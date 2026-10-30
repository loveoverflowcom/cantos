//! Media's side of exact audio association on metadata publication generations (ADR 0054 D06).
//! Catalog owns the association rows and the compatibility decision; Media owns every audio
//! fact. This module answers Catalog's `ActiveAudioRelease` port inside a settling transaction
//! and applies the live listener gate to an associated release. Nothing here searches for a
//! latest release: the only release considered is the one an entry pins or the one the Unit's
//! audio pointer names now.
use super::{
    domain::{
        listening::{associated_release_playable, AssociatedReleaseFacts},
        publication::AudioRelease,
    },
    turso::{lifecycle as lifecycle_sql, publication as sql},
};
use crate::contexts::catalog::{
    domain::{
        metadata_publication::{AudioAssociation, AudioRef, ReleasePin},
        ContentUnitId, OwnerId, Revision,
    },
    ActiveAudioRelease, AudioReleaseFuture, CatalogStoreError,
};
use crate::contract::studio::{MetadataPublicationRequest, MetadataPublicationState};
use crate::platform::store::{Store, StoreError, Tx};

/// The exact metadata revisions a settled release's manifest pins.
pub(super) fn release_pin(release: &AudioRelease) -> Result<ReleasePin, StoreError> {
    let d = release.document();
    let revision = |raw| Revision::new(raw).map_err(|_| StoreError::CorruptRevision);
    Ok(ReleasePin {
        release: AudioRef::new(&d.id, &d.digest).map_err(|_| StoreError::CorruptRevision)?,
        work_revision: revision(d.manifest.work_metadata_revision)?,
        unit_revision: revision(d.manifest.unit_metadata_revision)?,
    })
}

/// The production `ActiveAudioRelease`: the release the Unit's audio pointer names now for its
/// owner. A withdrawn release is never named (withdrawal clears the pointer); the live gate
/// still rechecks it at every read.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ActiveAudio;

impl ActiveAudioRelease for ActiveAudio {
    fn active_release<'a>(
        &'a self,
        tx: &'a Tx<'_>,
        owner: &'a OwnerId,
        unit: &'a ContentUnitId,
    ) -> AudioReleaseFuture<'a> {
        Box::pin(async move {
            sql::active_release(tx, unit.as_str(), owner.as_str())
                .await?
                .as_ref()
                .map(release_pin)
                .transpose()
        })
    }
}

/// The Studio use case for owner metadata publication commands: Catalog's settling command
/// composed with Media's audio facts, so exact releases are offered and `associate_audio`
/// names only the Unit's active release.
#[derive(Clone)]
pub struct MetadataPublicationService {
    store: Store,
}

impl MetadataPublicationService {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// Settle one owner command (Studio metadata publication v1).
    pub async fn settle(
        &self,
        token: &str,
        work: &str,
        request: MetadataPublicationRequest,
    ) -> Result<MetadataPublicationState, CatalogStoreError> {
        self.store
            .metadata_publication_execute(token, work, request, &ActiveAudio)
            .await
    }
}

/// An associated release that passed every live gate at the read's instant. Only
/// `playable_releases` constructs it.
#[derive(Clone, Debug)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "S05 composes generation playback into Discovery with every current client"
    )
)]
pub(crate) struct PlayableRelease(AudioRelease);

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "S05 composes generation playback into Discovery with every current client"
    )
)]
impl PlayableRelease {
    pub(crate) fn release(&self) -> &AudioRelease {
        &self.0
    }
}

/// CanPlay's release half for each `(unit, association)` of one visible page, in order and in
/// the caller's read snapshot. `None` means metadata-only for this read: no association, or an
/// associated release that is withdrawn, superseded, revoked, expired or no longer approved.
/// The caller has already applied the Work/Unit visibility gates.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "S05 composes generation playback into Discovery with every current client"
    )
)]
pub(crate) async fn playable_releases(
    tx: &Tx<'_>,
    entries: &[(&ContentUnitId, Option<&AudioAssociation>)],
) -> Result<Vec<Option<PlayableRelease>>, StoreError> {
    let asked: Vec<_> = entries
        .iter()
        .filter_map(|(unit, audio)| audio.map(|a| (unit.as_str(), a.release().release())))
        .collect();
    let mut live = sql::live_releases(tx, &asked).await?.into_iter();
    let now = lifecycle_sql::now(tx).await?;
    entries
        .iter()
        .map(|(_, audio)| {
            let Some(audio) = audio else {
                return Ok(None);
            };
            let fact = live.next().ok_or(StoreError::CorruptRevision)?;
            let playable = associated_release_playable(&AssociatedReleaseFacts {
                release: &fact.release,
                pinned_digest: audio.release().digest(),
                active: fact.active,
                withdrawn: fact.withdrawn,
                submission_revoked: fact.submission_revoked,
                latest_clearance: fact.latest_clearance.as_deref(),
                latest_approval: fact.latest_approval.as_deref(),
                now,
            });
            Ok(playable.then_some(PlayableRelease(fact.release)))
        })
        .collect()
}
