//! Owner-only metadata publication commands and state (ADR 0054 D01a/D02/D05/D06). One write
//! unit authenticates the current owner, replays or refuses a reused operation, compares the
//! pointer version, resolves every selected exact revision and writes the generation, its
//! sealing event (the receipt) and the pointer together. Saving a Catalog draft never reaches
//! here. Audio association reads only the release each Unit's audio pointer names now, through
//! the `ActiveAudioRelease` port Media implements; nothing searches for a latest release.
use std::future::Future;

use super::{
    owner_work::{owner_work, owner_work_now, recheck_owner_work, OwnerWork},
    CatalogStoreError,
};
use crate::contexts::{
    catalog::{
        domain::{
            metadata_publication::{
                admit_unit, admit_work, audio_compatibility, decide, public_search_text,
                AudioAssociation, AudioBasis, AudioCompatibility, Live, Pin, PublicationChange,
                PublicationError, PublicationState, Published, PublishedEntry, ReleasePin,
                MAX_PUBLISHED_UNITS,
            },
            ContentUnitId, OwnerId, Revision, Work, WorkId,
        },
        turso::{self, metadata_publication as store},
    },
    editorial,
};
use crate::contract::studio::{
    MetadataAudioBasis, MetadataGeneration, MetadataGenerationAudio, MetadataGenerationEntry,
    MetadataPublicationLifecycle as Lifecycle, MetadataPublicationRequest as Request,
    MetadataPublicationState as State,
};
use crate::platform::store::{normalized_uuid, Store, StoreError, Tx};

pub(crate) type AudioReleaseFuture<'a> =
    std::pin::Pin<Box<dyn Future<Output = Result<Option<ReleasePin>, StoreError>> + Send + 'a>>;

/// Media's answer, inside the settling transaction, to "which release does this Unit's audio
/// pointer name now?" for the Unit's owner. `None` when no release is active. Catalog never
/// reads Media tables and Media never writes generations except through `activate_audio`.
pub(crate) trait ActiveAudioRelease: Send + Sync {
    fn active_release<'a>(
        &'a self,
        tx: &'a Tx<'_>,
        owner: &'a OwnerId,
        unit: &'a ContentUnitId,
    ) -> AudioReleaseFuture<'a>;
}

/// The D06 comparison for one entry, read inside the settling transaction: the release's
/// pinned Work/Unit revisions against the generation's Work pin and the entry's Unit pin.
async fn compatibility(
    tx: &Tx<'_>,
    work: &Work,
    entry: &PublishedEntry,
    release: &ReleasePin,
) -> Result<AudioCompatibility, CatalogStoreError> {
    let id = work.id().as_str();
    let unit = entry.unit().as_str();
    let released_work = turso::load_work_revision(tx, id, release.work_revision)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let released_unit = turso::load_unit_revision(tx, unit, release.unit_revision)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let pinned = turso::load_unit_revision(tx, unit, entry.pin().revision())
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    Ok(audio_compatibility(
        &released_work,
        &released_unit,
        work,
        &pinned,
    ))
}

/// The exact association the Unit's active release offers for a newly pinned entry, if any.
/// A display-only drift is never associated without the owner's own command.
async fn offer(
    tx: &Tx<'_>,
    audio: &dyn ActiveAudioRelease,
    work: &Work,
    entry: PublishedEntry,
) -> Result<PublishedEntry, CatalogStoreError> {
    let Some(release) = audio
        .active_release(tx, work.owner_id(), entry.unit())
        .await?
    else {
        return Ok(entry);
    };
    let compatibility = compatibility(tx, work, &entry, &release).await?;
    let audio = AudioAssociation::admit(release.release, compatibility, false).ok();
    Ok(entry.with_audio(audio))
}

/// The canonical request bound: 1,000 selected Units with their IDs fit well within it.
const MAX_REQUEST_BYTES: usize = 256 * 1024;

fn refused(error: PublicationError) -> CatalogStoreError {
    CatalogStoreError::Publication(error)
}

async fn rights_resolved(
    tx: &Tx<'_>,
    owner: &str,
    refs: &[String],
) -> Result<bool, CatalogStoreError> {
    for id in refs {
        if !editorial::evidence_exists(tx, owner, "rights", id).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The owner's chosen saved Work revision, admitted against the current Work and rights.
async fn work_pin(
    tx: &Tx<'_>,
    owner: &OwnerWork,
    requested: u64,
) -> Result<Pin, CatalogStoreError> {
    let unknown = || refused(PublicationError::UnknownRevision);
    let revision = Revision::new(requested).map_err(|_| unknown())?;
    let id = owner.work.id().as_str();
    let pinned = turso::load_work_revision(tx, id, revision)
        .await?
        .ok_or_else(unknown)?;
    let digest = store::work_revision_digest(tx, id, revision)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let resolved = rights_resolved(tx, owner.actor.as_str(), &pinned.details().rights_refs).await?;
    admit_work(&pinned, &owner.work, &digest, resolved).map_err(refused)
}

/// Republication of the active Work pin still passes today's archive and rights gates.
pub(super) async fn recheck_work_pin(
    tx: &Tx<'_>,
    owner: &OwnerWork,
    pin: &Pin,
) -> Result<(), CatalogStoreError> {
    let pinned = store::pinned_work(tx, owner.work.id().as_str(), pin).await?;
    let resolved = rights_resolved(tx, owner.actor.as_str(), &pinned.details().rights_refs).await?;
    admit_work(&pinned, &owner.work, pin.digest(), resolved).map_err(refused)?;
    Ok(())
}

async fn unit_entry(
    tx: &Tx<'_>,
    work: &WorkId,
    unit: &str,
    requested: u64,
) -> Result<PublishedEntry, CatalogStoreError> {
    let id = ContentUnitId::parse(unit).map_err(|_| StoreError::InvalidRequest)?;
    let unknown = || refused(PublicationError::UnknownUnit { unit: id.clone() });
    let revision = Revision::new(requested).map_err(|_| unknown())?;
    let current = turso::load_unit(tx, id.as_str())
        .await?
        .ok_or_else(unknown)?;
    // A Unit of any other Work, including another owner's, is indistinguishable from absent.
    if current.work_id() != work {
        return Err(unknown());
    }
    let pinned = turso::load_unit_revision(tx, id.as_str(), revision)
        .await?
        .ok_or_else(unknown)?;
    let digest = store::unit_revision_digest(tx, id.as_str(), revision)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    admit_unit(work, &pinned, &current, &digest).map_err(refused)
}

async fn published_state(
    tx: &Tx<'_>,
    work: &WorkId,
    published: &Published,
) -> Result<State, CatalogStoreError> {
    let generation = published.generation();
    let recorded_at = store::recorded_at(tx, work.as_str(), generation.number()).await?;
    let entries = generation
        .entries()
        .iter()
        .zip(1_u32..)
        .map(|(entry, position)| MetadataGenerationEntry {
            position,
            unit_id: entry.unit().as_str().to_owned(),
            revision: entry.pin().revision().get(),
            digest: entry.pin().digest().to_owned(),
            audio: entry.audio().map(|audio| MetadataGenerationAudio {
                release_id: audio.release().release().to_owned(),
                release_digest: audio.release().digest().to_owned(),
                basis: match audio.basis() {
                    AudioBasis::Exact => MetadataAudioBasis::Exact,
                    AudioBasis::DisplayOnly => MetadataAudioBasis::DisplayOnly,
                },
            }),
        })
        .collect();
    Ok(State {
        work_id: work.as_str().to_owned(),
        lifecycle: match published.lifecycle() {
            Live::Active => Lifecycle::Active,
            Live::Withdrawn => Lifecycle::Withdrawn,
        },
        version: published.version().get(),
        first_visible_at_ms: Some(published.first_visible_at()),
        generation: Some(MetadataGeneration {
            number: generation.number().get(),
            parent: generation.parent().map(|parent| parent.get()),
            work_revision: generation.work().revision().get(),
            work_digest: generation.work().digest().to_owned(),
            recorded_at_ms: recorded_at,
            entries,
        }),
    })
}

async fn current_state(
    tx: &Tx<'_>,
    work: &WorkId,
    state: &PublicationState,
) -> Result<State, CatalogStoreError> {
    match state {
        PublicationState::NeverPublished => Ok(State {
            work_id: work.as_str().to_owned(),
            lifecycle: Lifecycle::NeverPublished,
            version: 0,
            first_visible_at_ms: None,
            generation: None,
        }),
        PublicationState::Published(published) => published_state(tx, work, published).await,
    }
}

impl Store {
    /// The owner's view of one Work's publication pointer and active or last generation.
    pub async fn metadata_publication_state(
        &self,
        token: &str,
        work: &str,
    ) -> Result<State, CatalogStoreError> {
        let work_id = WorkId::parse(work).map_err(|_| StoreError::NotFound)?;
        let connection = self.connection().await?;
        let tx = connection.read().await.map_err(StoreError::from)?;
        let owner = owner_work(&tx, token, &work_id).await?;
        let state = store::state(&tx, work_id.as_str()).await?;
        let reply = current_state(&tx, &work_id, &state).await?;
        recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(reply)
    }

    /// Settle one owner command. The same operation ID and payload replays the outcome it
    /// settled (also after a lost response); a changed payload is refused before any read.
    pub(crate) async fn metadata_publication_execute(
        &self,
        token: &str,
        work: &str,
        request: Request,
        audio: &dyn ActiveAudioRelease,
    ) -> Result<State, CatalogStoreError> {
        let work_id = WorkId::parse(work).map_err(|_| StoreError::NotFound)?;
        let operation = normalized_uuid(request.operation_id())?;
        match &request {
            Request::Publish { units, .. } if units.len() > MAX_PUBLISHED_UNITS => {
                return Err(refused(PublicationError::TooManyUnits));
            }
            Request::Publish {
                confirm_metadata_rights: false,
                ..
            }
            | Request::PublishUnit {
                confirm_metadata_rights: false,
                ..
            } => return Err(StoreError::InvalidRequest.into()),
            _ => {}
        }
        let encoded = serde_json::to_vec(&(work_id.as_str(), &request))
            .map_err(|_| StoreError::InvalidRequest)?;
        if encoded.len() > MAX_REQUEST_BYTES {
            return Err(CatalogStoreError::Limit);
        }
        let fingerprint = turso::digest(&encoded);
        let mut connection = self.connection().await?;
        let tx = connection.transaction().await.map_err(StoreError::from)?;
        let owner = owner_work(&tx, token, &work_id).await?;
        let actor = owner.actor.as_str();
        if let Some(receipt) = store::operation(&tx, actor, &operation).await? {
            if receipt.fingerprint != fingerprint || receipt.work_id != work_id.as_str() {
                return Err(CatalogStoreError::OperationReused);
            }
            let settled = store::at_version(&tx, work_id.as_str(), receipt.version).await?;
            let reply = published_state(&tx, &work_id, &settled).await?;
            recheck_owner_work(&tx, token, &owner).await?;
            tx.commit().await.map_err(StoreError::from)?;
            return Ok(reply);
        }
        let state = store::state(&tx, work_id.as_str()).await?;
        // A stale command resolves no selected revision.
        if request.expected_version() != state.version() {
            return Err(refused(PublicationError::StaleVersion {
                current: state.version(),
            }));
        }
        let now = owner_work_now(&tx).await?;
        let (action, change) = match &request {
            Request::Publish {
                work_revision,
                units,
                ..
            } => {
                let work = work_pin(&tx, &owner, *work_revision).await?;
                let pinned = store::pinned_work(&tx, work_id.as_str(), &work).await?;
                let mut entries = Vec::with_capacity(units.len());
                for unit in units {
                    let entry = unit_entry(&tx, &work_id, &unit.unit_id, unit.revision).await?;
                    entries.push(offer(&tx, audio, &pinned, entry).await?);
                }
                ("publish", PublicationChange::Publish { work, entries })
            }
            Request::PublishUnit {
                unit_id,
                revision,
                position,
                ..
            } => {
                let published = state.active().map_err(refused)?;
                let pinned =
                    store::pinned_work(&tx, work_id.as_str(), published.generation().work())
                        .await?;
                recheck_work_pin(&tx, &owner, published.generation().work()).await?;
                let entry = unit_entry(&tx, &work_id, unit_id, *revision).await?;
                let entry = offer(&tx, audio, &pinned, entry).await?;
                (
                    "publish_unit",
                    PublicationChange::PublishUnit {
                        entry,
                        position: *position,
                    },
                )
            }
            Request::WithdrawUnit { unit_id, .. } => (
                "withdraw_unit",
                PublicationChange::WithdrawUnit {
                    unit: ContentUnitId::parse(unit_id).map_err(|_| StoreError::InvalidRequest)?,
                },
            ),
            Request::Withdraw { .. } => ("withdraw", PublicationChange::Withdraw),
            Request::AssociateAudio {
                unit_id,
                release_id,
                confirm_display_only_drift,
                ..
            } => {
                let unit = ContentUnitId::parse(unit_id).map_err(|_| StoreError::InvalidRequest)?;
                let published = state.active().map_err(refused)?;
                let entry = published.generation().entry(&unit).ok_or_else(|| {
                    refused(PublicationError::UnitNotPublished { unit: unit.clone() })
                })?;
                let pinned =
                    store::pinned_work(&tx, work_id.as_str(), published.generation().work())
                        .await?;
                // Only the release the Unit's audio pointer names now; never a substitute.
                let release = audio
                    .active_release(&tx, &owner.actor, &unit)
                    .await?
                    .filter(|release| release.release.release() == release_id)
                    .ok_or_else(|| refused(PublicationError::ReleaseNotActive))?;
                let compatibility = compatibility(&tx, &pinned, entry, &release).await?;
                let audio = AudioAssociation::admit(
                    release.release,
                    compatibility,
                    *confirm_display_only_drift,
                )
                .map_err(refused)?;
                (
                    "associate_audio",
                    PublicationChange::AssociateAudio { unit, audio },
                )
            }
        };
        let settled = decide(&state, request.expected_version(), change, now).map_err(refused)?;
        if settled.created {
            let generation = settled.published.generation();
            let pinned = store::pinned_work(&tx, work_id.as_str(), generation.work()).await?;
            let text = public_search_text(&format!(
                "{} {}",
                pinned.details().title,
                pinned.details().synopsis
            ));
            store::insert_generation(&tx, &pinned, generation, &text, now).await?;
        }
        store::settle(
            &tx,
            work_id.as_str(),
            actor,
            action,
            &settled.published,
            &operation,
            &fingerprint,
            now,
        )
        .await?;
        let reply = published_state(&tx, &work_id, &settled.published).await?;
        recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(reply)
    }
}

/// What an audio activation did to the Unit's metadata publication (D06c).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActivationAssociation {
    /// Generation N+1 associates the activated release with the active entry.
    Associated,
    /// The active entry already names this release.
    AlreadyAssociated,
    /// "Audio ready, not associated": the active entry's pin differs from the release's; the
    /// owner may associate a display-only difference explicitly.
    NotAssociated,
    /// No ACTIVE generation publishes the Unit; metadata publication will offer the release.
    NotPublished,
}

/// D06c: inside the audio activation's own write unit (the caller has already authorized the
/// owner, written the release and moved the Unit's audio pointer), associate the release with
/// the ACTIVE generation's entry when it pins exactly the same Unit revision. The pointer
/// version read here is compared by the same single writer, so no concurrent publisher can
/// interleave. Nothing is written when the release is not exact.
pub(crate) async fn activate_audio(
    tx: &Tx<'_>,
    owner: &OwnerId,
    unit: &ContentUnitId,
    release: ReleasePin,
    now: i64,
) -> Result<ActivationAssociation, CatalogStoreError> {
    let current = turso::load_unit(tx, unit.as_str())
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let work_id = current.work_id().clone();
    let state = store::state(tx, work_id.as_str()).await?;
    let Ok(published) = state.active() else {
        return Ok(ActivationAssociation::NotPublished);
    };
    let Some(entry) = published.generation().entry(unit) else {
        return Ok(ActivationAssociation::NotPublished);
    };
    let pinned = store::pinned_work(tx, work_id.as_str(), published.generation().work()).await?;
    if pinned.owner_id() != owner {
        return Err(StoreError::CorruptRevision.into());
    }
    let compatibility = compatibility(tx, &pinned, entry, &release).await?;
    let fingerprint = turso::digest(
        format!(
            "activate_audio\n{}\n{}\n{}",
            unit.as_str(),
            release.release.release(),
            release.release.digest()
        )
        .as_bytes(),
    );
    let Ok(audio) = AudioAssociation::admit(release.release, compatibility, false) else {
        return Ok(ActivationAssociation::NotAssociated);
    };
    let change = PublicationChange::AssociateAudio {
        unit: unit.clone(),
        audio,
    };
    let settled = match decide(&state, state.version(), change, now) {
        Ok(settled) => settled,
        Err(PublicationError::Unchanged) => return Ok(ActivationAssociation::AlreadyAssociated),
        Err(error) => return Err(refused(error)),
    };
    let text = public_search_text(&format!(
        "{} {}",
        pinned.details().title,
        pinned.details().synopsis
    ));
    store::insert_generation(tx, &pinned, settled.published.generation(), &text, now).await?;
    // The receipt's operation ID is fresh: the audio operation already has its own receipt, and
    // reusing its ID here could collide with an unrelated metadata command of the same owner.
    store::settle(
        tx,
        work_id.as_str(),
        owner.as_str(),
        "activate_audio",
        &settled.published,
        &uuid::Uuid::new_v4().to_string(),
        &fingerprint,
        now,
    )
    .await?;
    Ok(ActivationAssociation::Associated)
}
