//! Thin publication shell. Mutable authority is read again under the Catalog mutation fence.
//! No network/storage promotion happens in the metadata commit transaction.
use super::{
    application::storage,
    domain::{
        lifecycle::{AudioRendition, AudioReview, ReleaseCandidate, ReleaseCandidateId},
        object_storage::{PrivateObjectRef, StorageError},
        publication::*,
    },
    lifecycle_application,
    postgres::{lifecycle as lifecycle_sql, publication as sql},
    publication_delivery::*,
    publication_storage::FilesystemPublicationStorage,
};
use crate::contract::studio as dto;
use crate::{
    contexts::{
        catalog::{
            self,
            domain::{content::ContentRevisionId, ContentUnitId, OwnerId},
            ProductionContent,
        },
        editorial,
        production::domain::brief::fingerprint,
    },
    platform::store::{authenticate, normalized_uuid, LockMode, Store, StoreError, Tx},
};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;
use uuid::Uuid;

#[derive(Debug)]
pub enum PublicationStoreError {
    Storage(StoreError),
    Domain(PublicationError),
    Blocked(Vec<dto::PublicationBlocker>),
    Conflict {
        version: u64,
        active_release_id: Option<String>,
    },
    Dependency(PublicationDependencyError),
}
impl From<crate::platform::store::Error> for PublicationStoreError {
    fn from(error: crate::platform::store::Error) -> Self {
        Self::Storage(StoreError::from(error))
    }
}
impl From<StoreError> for PublicationStoreError {
    fn from(e: StoreError) -> Self {
        Self::Storage(e)
    }
}
impl From<PublicationError> for PublicationStoreError {
    fn from(e: PublicationError) -> Self {
        Self::Domain(e)
    }
}
impl From<PublicationDependencyError> for PublicationStoreError {
    fn from(e: PublicationDependencyError) -> Self {
        Self::Dependency(e)
    }
}
struct Subject {
    s: super::domain::lifecycle::AudioSubmission,
    content: ProductionContent,
    r: AudioRendition,
    location: PrivateObjectRef,
    c: ReleaseCandidate,
    reviews: Vec<AudioReview>,
    current: bool,
    revoked: bool,
}
/// Ephemeral authority from authenticated owner resolution, not raw HTTP or a persisted row.
struct PublishPermit {
    owner: OwnerId,
    unit: ContentUnitId,
}
impl Subject {
    fn permit(&self) -> PublishPermit {
        PublishPermit {
            owner: self.content.work.owner_id().clone(),
            unit: self.content.unit.id().clone(),
        }
    }
    fn facts<'a>(
        &'a self,
        rights: Option<&'a DistributionClearance>,
        approval: Option<&'a PublicationApproval>,
        observations: &'a Observations,
        now: i64,
    ) -> PublicationFacts<'a> {
        PublicationFacts {
            candidate: &self.c,
            rendition: &self.r,
            work: &self.content.work,
            unit: &self.content.unit,
            content: &self.content.head,
            upload_expires_at: self.s.document().expires_at,
            actor: self.content.work.owner_id(),
            current_brief: self.current,
            upload_revoked: self.revoked,
            reviews: &self.reviews,
            rights,
            approval,
            object: &observations.object,
            delivery: &observations.delivery,
            now,
        }
    }
}
async fn subject(
    tx: &Tx<'_>,
    token: &str,
    unit: &str,
    candidate: &str,
    lock: LockMode,
) -> Result<Subject, PublicationStoreError> {
    ReleaseCandidateId::parse(candidate).map_err(|_| PublicationError::InvalidId)?;
    let submission = lifecycle_sql::candidate_submission(tx, candidate)
        .await?
        .ok_or(StoreError::NotFound)?;
    let (s, content, current) =
        lifecycle_application::owner(tx, token, unit, &submission, lock).await?;
    let r = lifecycle_sql::rendition(tx, &s)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    let location = lifecycle_sql::location(tx, &r).await?;
    let raw = lifecycle_sql::load_candidate(tx, candidate, &r)
        .await?
        .ok_or(StoreError::NotFound)?;
    let c = ReleaseCandidate::restore(raw, &r).map_err(|_| StoreError::CorruptRevision)?;
    let reviews = lifecycle_application::checked_reviews(tx, &s, &r)
        .await?
        .into_iter()
        .map(|raw| AudioReview::restore(raw, &r).map_err(|_| StoreError::CorruptRevision))
        .collect::<Result<Vec<_>, _>>()?;
    let revoked = lifecycle_sql::revoked(tx, &submission).await?;
    Ok(Subject {
        s,
        content,
        r,
        location,
        c,
        reviews,
        current,
        revoked,
    })
}
async fn bound_release(
    tx: &Tx<'_>,
    id: &str,
    subject: &Subject,
) -> Result<AudioRelease, PublicationStoreError> {
    let revision = ContentRevisionId::parse(&subject.r.document().binding.content_revision_id)
        .map_err(|_| StoreError::CorruptRevision)?;
    let content = catalog::pinned_content(tx, subject.content.unit.id(), &revision)
        .await
        .map_err(|e| match e {
            catalog::CatalogStoreError::Storage(e) => e,
            _ => StoreError::CorruptRevision,
        })?;
    Ok(sql::release(tx, id, &subject.c, &subject.r, &content).await?)
}
pub(super) struct Observations {
    pub object: ObjectObservation,
    pub delivery: DeliveryObservation,
}
struct PromotionAuthority {
    rights: Option<DistributionClearance>,
    approval: Option<PublicationApproval>,
    now: i64,
}
impl PromotionAuthority {
    async fn read(tx: &Tx<'_>, subject: &Subject) -> Result<Self, PublicationStoreError> {
        Ok(Self {
            rights: sql::latest_clearance(tx, &subject.c).await?,
            approval: sql::latest_approval(tx, &subject.c).await?,
            now: lifecycle_sql::now(tx).await?,
        })
    }
    fn allows(&self, subject: &Subject, object: &ObjectObservation) -> bool {
        let observations = Observations {
            object: object.clone(),
            delivery: DeliveryObservation::Unavailable,
        };
        // Only delivery may be missing before promotion. Every candidate/rights/QC/approval
        // gate already applies here; the final transaction independently repeats all gates.
        ready_for_promotion(&subject.facts(
            self.rights.as_ref(),
            self.approval.as_ref(),
            &observations,
            self.now,
        ))
    }
}
fn ready_for_promotion(facts: &PublicationFacts<'_>) -> bool {
    evaluate(facts) == vec![dto::PublicationBlocker::DeliveryUnavailable]
}
async fn observe(
    store: &Store,
    delivery: &dyn PublicationDelivery,
    subject: &Subject,
    promotion: Option<(&FilesystemPublicationStorage, &PromotionAuthority)>,
) -> Result<Observations, PublicationStoreError> {
    let object = observe_private(store, &subject.r, &subject.location).await?;
    if let Some((target, authority)) = promotion {
        if authority.allows(subject, &object) {
            promote_private(store, target, subject).await?;
        }
    }
    let delivery = observe_delivery(delivery, &subject.r).await?;
    Ok(Observations { object, delivery })
}

static PROMOTION_LANES: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));
async fn promote_private(
    store: &Store,
    target: &FilesystemPublicationStorage,
    subject: &Subject,
) -> Result<(), PublicationStoreError> {
    use futures_util::StreamExt;
    let lane = PROMOTION_LANES
        .clone()
        .try_acquire_owned()
        .map_err(|_| StoreError::Unavailable)?;
    let metadata = &subject.r.document().verified_media;
    let mut read = storage(store)?
        .open_verified(&subject.location, None)
        .await?;
    if read.byte_length != metadata.byte_length {
        return Err(StoreError::CorruptRevision.into());
    }
    let expected = metadata.byte_length;
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(60), async move {
        let mut bytes = Vec::with_capacity(expected as usize);
        while let Some(chunk) = read.stream.next().await {
            let chunk = chunk?;
            if chunk.len() > (expected as usize).saturating_sub(bytes.len()) {
                return Err(StorageError::LengthMismatch);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok::<_, StorageError>(bytes)
    })
    .await
    .map_err(|_| StoreError::Storage(StorageError::Timeout))?
    .map_err(StoreError::Storage)?;
    subject
        .location
        .integrity()
        .verify_bytes(&bytes)
        .map_err(StoreError::Storage)?;
    let profile = super::audio_intake::DeliveryProfile::from_id(&metadata.profile_id)
        .ok_or(StoreError::CorruptRevision)?;
    let verified = tokio::task::spawn_blocking(move || {
        let _lane = lane;
        super::audio_intake::inspect_audio(&bytes, profile)
    })
    .await
    .map_err(|_| StoreError::Unavailable)?
    .map_err(|error| {
        if error.is_unavailable() {
            StoreError::AudioInspectionUnavailable(error.code())
        } else {
            StoreError::AudioRejected(error.code())
        }
    })?;
    target
        .promote(&subject.r.document().id, verified)
        .await
        .map_err(StoreError::Storage)?;
    Ok(())
}
async fn observe_private(
    store: &Store,
    r: &AudioRendition,
    location: &PrivateObjectRef,
) -> Result<ObjectObservation, PublicationStoreError> {
    let metadata = &r.document().verified_media;
    let object = match storage(store) {
        Ok(objects) => match objects.verify(location).await {
            Ok(()) => ObjectObservation::Verified {
                sha256: Sha256::parse(&metadata.sha256)?,
                byte_length: metadata.byte_length,
            },
            Err(e) => private_failure_observation(e)?,
        },
        Err(StoreError::Unavailable) => ObjectObservation::Unverified,
        Err(e) => return Err(e.into()),
    };
    Ok(object)
}
fn private_failure_observation(error: StoreError) -> Result<ObjectObservation, StoreError> {
    match error {
        StoreError::CorruptRevision => Ok(ObjectObservation::IntegrityMismatch),
        StoreError::Storage(StorageError::NotFound) => Ok(ObjectObservation::Missing),
        StoreError::Storage(
            StorageError::InvalidKey
            | StorageError::InvalidIntegrity
            | StorageError::DigestMismatch
            | StorageError::LengthMismatch
            | StorageError::Conflict,
        ) => Ok(ObjectObservation::IntegrityMismatch),
        StoreError::Unavailable
        | StoreError::Storage(
            StorageError::InvalidTarget
            | StorageError::Forbidden
            | StorageError::Timeout
            | StorageError::Cancelled
            | StorageError::Unavailable
            | StorageError::UnsupportedCapability(_),
        ) => Ok(ObjectObservation::Unverified),
        error => Err(error),
    }
}
fn recheck_observed_object(initial: &Subject, fresh: &Subject) -> Result<(), StoreError> {
    if initial.r != fresh.r || initial.location != fresh.location {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}
pub(super) async fn observe_delivery(
    delivery: &dyn PublicationDelivery,
    r: &AudioRendition,
) -> Result<DeliveryObservation, PublicationStoreError> {
    let metadata = &r.document().verified_media;
    Ok(delivery
        .observe(PublicationObject {
            rendition_id: r.document().id.clone(),
            sha256: Sha256::parse(&metadata.sha256)?,
            byte_length: metadata.byte_length,
            profile_id: metadata.profile_id.clone(),
        })
        .await?)
}
fn request_digest<T: serde::Serialize>(request: &T) -> Result<String, PublicationStoreError> {
    fingerprint("publication-request1", request).map_err(|_| StoreError::CorruptRevision.into())
}
fn conflict(pointer: &sql::Pointer) -> PublicationStoreError {
    PublicationStoreError::Conflict {
        version: pointer.version,
        active_release_id: pointer.active.clone(),
    }
}
/// Production preparation shared by the database shell and deterministic tests. A failed gate
/// returns before constructing a release or proposing any pointer change.
pub(super) fn prepare(
    facts: &PublicationFacts<'_>,
    pointer: &sql::Pointer,
    request: &dto::PublishAudioReleaseRequest,
    id: ReleaseId,
) -> Result<(AudioRelease, u64), PublicationStoreError> {
    ReleaseId::parse(&request.operation_id)?;
    if request.candidate_digest != facts.candidate.document().digest {
        return Err(PublicationError::BindingMismatch.into());
    }
    if request.expected_pointer_version != pointer.version
        || request.expected_active_release_id != pointer.active
    {
        return Err(conflict(pointer));
    }
    let next = advance_pointer(pointer.version, request.expected_pointer_version)?;
    let witness = decide(facts).map_err(PublicationStoreError::Blocked)?;
    let prior = pointer
        .active
        .as_deref()
        .map(ReleaseId::parse)
        .transpose()?;
    let release = AudioRelease::publish(id, facts.actor, facts.now, prior, witness)?;
    Ok((release, next))
}
async fn commit(
    tx: &Tx<'_>,
    permit: PublishPermit,
    pointer: &sql::Pointer,
    release: &AudioRelease,
    next: u64,
    operation: &str,
    digest: &str,
) -> Result<(), PublicationStoreError> {
    let d = release.document();
    if d.published_by != permit.owner.as_str() || d.manifest.unit_id != permit.unit.as_str() {
        return Err(StoreError::CorruptRevision.into());
    }
    sql::insert_release(tx, release).await?;
    // The discovery index is written in the same unit as the release row and the pointer.
    super::turso::listener::index_release(tx, release).await?;
    sql::set_pointer(
        tx,
        permit.unit.as_str(),
        permit.owner.as_str(),
        pointer,
        Some(&d.id),
        next,
    )
    .await?;
    sql::event(
        tx,
        sql::Event {
            id: &Uuid::new_v4().to_string(),
            unit: permit.unit.as_str(),
            version: next,
            kind: "activated",
            release: &d.id,
            owner: permit.owner.as_str(),
            now: d.published_at,
        },
    )
    .await?;
    sql::record(
        tx,
        permit.owner.as_str(),
        operation,
        "publish",
        &sql::Operation {
            request_digest: digest.into(),
            unit: permit.unit.as_str().into(),
            release: d.id.clone(),
            version: next,
        },
    )
    .await?;
    Ok(())
}
/// Media-owned composition of metadata authority and a publication delivery consumer.
/// Platform PostgreSQL plumbing never owns or imports this business port.
#[derive(Clone)]
pub struct PublicationService {
    store: Store,
    delivery: Arc<dyn PublicationDelivery>,
    promotion: Option<Arc<FilesystemPublicationStorage>>,
}
impl PublicationService {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            delivery: Arc::new(UnavailableDelivery),
            promotion: None,
        }
    }
    /// Explicit injection by the trusted host, never by a request/producer.
    pub fn with_publication_delivery(mut self, delivery: Arc<dyn PublicationDelivery>) -> Self {
        self.delivery = delivery;
        self.promotion = None;
        self
    }
    /// Trusted origin host: readiness/publish may copy only the authenticated, fully approved
    /// private rendition into this separate byte class. Listener requests never promote.
    pub fn with_origin_publication(mut self, storage: Arc<FilesystemPublicationStorage>) -> Self {
        self.delivery = storage.clone();
        self.promotion = Some(storage);
        self
    }
    pub async fn audio_publication_readiness(
        &self,
        token: &str,
        unit: &str,
        candidate: &str,
    ) -> Result<dto::PublicationReadinessResponse, PublicationStoreError> {
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let initial = subject(&tx, token, unit, candidate, LockMode::Share).await?;
        let promotion = if self.promotion.is_some() {
            Some(PromotionAuthority::read(&tx, &initial).await?)
        } else {
            None
        };
        tx.commit().await?;
        drop(client);
        let observations = observe(
            &self.store,
            self.delivery.as_ref(),
            &initial,
            self.promotion.as_deref().zip(promotion.as_ref()),
        )
        .await?;
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let fresh = subject(&tx, token, unit, candidate, LockMode::Share).await?;
        recheck_observed_object(&initial, &fresh)?;
        let rights = sql::latest_clearance(&tx, &fresh.c).await?;
        let approval = sql::latest_approval(&tx, &fresh.c).await?;
        let pointer =
            sql::pointer(&tx, unit, fresh.content.work.owner_id().as_str(), false).await?;
        let blockers = evaluate(&fresh.facts(
            rights.as_ref(),
            approval.as_ref(),
            &observations,
            lifecycle_sql::now(&tx).await?,
        ));
        let response = dto::PublicationReadinessResponse {
            candidate_id: candidate.into(),
            candidate_digest: fresh.c.document().digest.clone(),
            pointer_version: pointer.version,
            active_release_id: pointer.active,
            blockers,
            clearance: rights.map(|r| r.document().clone()),
            approval: approval.map(|a| a.document().clone()),
        };
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(response)
    }
    pub async fn record_audio_distribution_rights(
        &self,
        token: &str,
        unit: &str,
        candidate: &str,
        mut request: dto::RecordDistributionRightsRequest,
    ) -> Result<dto::DistributionClearanceDocument, PublicationStoreError> {
        normalized_uuid(&request.operation_id)?;
        request.rights_refs.sort();
        request.evidence_refs.sort();
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let s = subject(&tx, token, unit, candidate, LockMode::Update).await?;
        let actor = s.content.work.owner_id().as_str();
        if let Some(prior) = sql::prior_clearance(&tx, actor, &request.operation_id, &s.c).await? {
            if prior.document().request != request {
                return Err(StoreError::OperationReused.into());
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(prior.document().clone());
        }
        // These are named owner's explicit distribution decisions. Producer claims are never used
        // as evidence. Every required rights reference must exist in the owner's rights registry.
        if request.decision == dto::DistributionDecision::Cleared {
            for id in request
                .rights_refs
                .iter()
                .chain(request.evidence_refs.iter())
            {
                if !editorial::evidence_exists(&tx, actor, "rights", id).await? {
                    return Err(StoreError::EvidenceUnavailable.into());
                }
            }
        }
        let sequence = sql::latest_clearance(&tx, &s.c).await?.map_or(Ok(1), |r| {
            advance_pointer(r.document().sequence, r.document().sequence)
        })?;
        let value = DistributionClearance::record(
            dto::DistributionClearanceDocument {
                id: Uuid::new_v4().to_string(),
                candidate_id: candidate.into(),
                actor_id: actor.into(),
                sequence,
                recorded_at: lifecycle_sql::now(&tx).await?,
                request,
                digest: String::new(),
            },
            &s.c,
        )?;
        sql::insert_clearance(&tx, &value).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(value.document().clone())
    }
    pub async fn record_audio_publication_approval(
        &self,
        token: &str,
        unit: &str,
        candidate: &str,
        request: dto::RecordPublicationApprovalRequest,
    ) -> Result<dto::PublicationApprovalDocument, PublicationStoreError> {
        normalized_uuid(&request.operation_id)?;
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let initial = subject(&tx, token, unit, candidate, LockMode::Share).await?;
        let actor = initial.content.work.owner_id().as_str();
        if let Some(prior) =
            sql::prior_approval(&tx, actor, &request.operation_id, &initial.c).await?
        {
            if prior.document().request != request {
                return Err(StoreError::OperationReused.into());
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(prior.document().clone());
        }
        authenticate(&tx, token).await?;
        tx.commit().await?;
        drop(client);
        let object = if request.decision == dto::PublicationApprovalDecision::Approved {
            Some(observe_private(&self.store, &initial.r, &initial.location).await?)
        } else {
            None
        };
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let s = subject(&tx, token, unit, candidate, LockMode::Update).await?;
        recheck_observed_object(&initial, &s)?;
        let actor = s.content.work.owner_id().as_str();
        if let Some(prior) = sql::prior_approval(&tx, actor, &request.operation_id, &s.c).await? {
            if prior.document().request != request {
                return Err(StoreError::OperationReused.into());
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(prior.document().clone());
        }
        let rights = sql::latest_clearance(&tx, &s.c).await?.ok_or_else(|| {
            PublicationStoreError::Blocked(vec![dto::PublicationBlocker::RightsUnrecorded])
        })?;
        if request.clearance_digest != rights.document().digest {
            return Err(PublicationStoreError::Blocked(vec![
                dto::PublicationBlocker::ApprovalStale,
            ]));
        }
        let now = lifecycle_sql::now(&tx).await?;
        if let Some(object) = object {
            let observations = Observations {
                object,
                delivery: DeliveryObservation::Unavailable,
            };
            let blockers = approval_blockers(&s.facts(Some(&rights), None, &observations, now));
            if !blockers.is_empty() {
                return Err(PublicationStoreError::Blocked(blockers));
            }
        }
        let sequence = sql::latest_approval(&tx, &s.c).await?.map_or(Ok(1), |a| {
            advance_pointer(a.document().sequence, a.document().sequence)
        })?;
        let value = PublicationApproval::record(
            dto::PublicationApprovalDocument {
                id: Uuid::new_v4().to_string(),
                candidate_id: candidate.into(),
                actor_id: actor.into(),
                sequence,
                recorded_at: now,
                request,
                digest: String::new(),
            },
            &s.c,
        )?;
        sql::insert_approval(&tx, &value).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(value.document().clone())
    }
    pub async fn publish_audio_release(
        &self,
        token: &str,
        unit: &str,
        candidate: &str,
        request: dto::PublishAudioReleaseRequest,
    ) -> Result<dto::ActiveAudioReleaseResponse, PublicationStoreError> {
        normalized_uuid(&request.operation_id)?;
        let digest = request_digest(&(unit, candidate, &request))?;
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let initial = subject(&tx, token, unit, candidate, LockMode::Share).await?;
        // Replays are durable receipts, never a second activation, even if later rights change.
        if let Some(prior) = sql::prior(
            &tx,
            initial.content.work.owner_id().as_str(),
            &request.operation_id,
            "publish",
        )
        .await?
        {
            if prior.request_digest != digest || prior.unit != unit {
                return Err(StoreError::OperationReused.into());
            }
            let release = bound_release(&tx, &prior.release, &initial).await?;
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(dto::ActiveAudioReleaseResponse {
                pointer_version: prior.version,
                release: Some(release.document().clone()),
            });
        }
        let promotion = if self.promotion.is_some() {
            Some(PromotionAuthority::read(&tx, &initial).await?)
        } else {
            None
        };
        tx.commit().await?;
        drop(client);
        let observations = observe(
            &self.store,
            self.delivery.as_ref(),
            &initial,
            self.promotion.as_deref().zip(promotion.as_ref()),
        )
        .await?;
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let fresh = subject(&tx, token, unit, candidate, LockMode::Update).await?;
        let actor = fresh.content.work.owner_id().as_str();
        recheck_observed_object(&initial, &fresh)?;
        if let Some(prior) = sql::prior(&tx, actor, &request.operation_id, "publish").await? {
            if prior.request_digest != digest || prior.unit != unit {
                return Err(StoreError::OperationReused.into());
            }
            let release = bound_release(&tx, &prior.release, &fresh).await?;
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(dto::ActiveAudioReleaseResponse {
                pointer_version: prior.version,
                release: Some(release.document().clone()),
            });
        }
        let rights = sql::latest_clearance(&tx, &fresh.c).await?;
        let approval = sql::latest_approval(&tx, &fresh.c).await?;
        let pointer = sql::pointer(&tx, unit, actor, true).await?;
        let (release, next) = prepare(
            &fresh.facts(
                rights.as_ref(),
                approval.as_ref(),
                &observations,
                lifecycle_sql::now(&tx).await?,
            ),
            &pointer,
            &request,
            ReleaseId::parse(&Uuid::new_v4().to_string())?,
        )?;
        commit(
            &tx,
            fresh.permit(),
            &pointer,
            &release,
            next,
            &request.operation_id,
            &digest,
        )
        .await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(dto::ActiveAudioReleaseResponse {
            pointer_version: next,
            release: Some(release.document().clone()),
        })
    }
    pub async fn active_audio_release(
        &self,
        token: &str,
        unit: &str,
    ) -> Result<dto::ActiveAudioReleaseResponse, PublicationStoreError> {
        let unit_id = ContentUnitId::parse(unit).map_err(|_| PublicationError::InvalidId)?;
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let content =
            catalog::production_content(&tx, Some(token), None, &unit_id, LockMode::Share)
                .await
                .map_err(|e| match e {
                    catalog::CatalogStoreError::Storage(e) => e,
                    _ => StoreError::CorruptRevision,
                })?;
        let actor = content.work.owner_id().as_str();
        let pointer = sql::pointer(&tx, unit, actor, false).await?;
        let Some(id) = &pointer.active else {
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(dto::ActiveAudioReleaseResponse {
                pointer_version: pointer.version,
                release: None,
            });
        };
        let candidate = sql::release_candidate(&tx, id, unit, actor)
            .await?
            .ok_or(StoreError::CorruptRevision)?;
        let initial = subject(&tx, token, unit, &candidate, LockMode::Share).await?;
        let release = bound_release(&tx, id, &initial).await?;
        tx.commit().await?;
        drop(client);
        let observations = observe(&self.store, self.delivery.as_ref(), &initial, None).await?;
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let fresh = subject(&tx, token, unit, &candidate, LockMode::Share).await?;
        recheck_observed_object(&initial, &fresh)?;
        let current =
            sql::pointer(&tx, unit, fresh.content.work.owner_id().as_str(), false).await?;
        if current != pointer {
            return Err(conflict(&current));
        }
        let rights = sql::latest_clearance(&tx, &fresh.c).await?;
        let approval = sql::latest_approval(&tx, &fresh.c).await?;
        let now = lifecycle_sql::now(&tx).await?;
        let eligible = retained_release_eligible(
            &release,
            rights.as_ref(),
            approval.as_ref(),
            sql::withdrawn(&tx, id).await? || fresh.revoked,
            now,
        );
        if eligible
            && !delivery_matches_manifest(&observations.delivery, &release.document().manifest, now)
        {
            return Err(PublicationStoreError::Blocked(vec![
                dto::PublicationBlocker::DeliveryUnavailable,
            ]));
        }
        if eligible && !matches!(observations.object, ObjectObservation::Verified { .. }) {
            return Err(PublicationStoreError::Blocked(vec![
                dto::PublicationBlocker::ObjectUnverified,
            ]));
        }
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(dto::ActiveAudioReleaseResponse {
            pointer_version: pointer.version,
            release: eligible.then(|| release.document().clone()),
        })
    }
    pub async fn withdraw_audio_release(
        &self,
        token: &str,
        unit: &str,
        request: dto::WithdrawAudioReleaseRequest,
    ) -> Result<dto::AudioReleaseWithdrawalResponse, PublicationStoreError> {
        normalized_uuid(&request.operation_id)?;
        ReleaseId::parse(&request.release_id)?;
        if request.reason.trim().is_empty()
            || request.reason.len() > 4096
            || request.reason.chars().any(char::is_control)
        {
            return Err(PublicationError::InvalidEvidence.into());
        }
        let digest = request_digest(&(unit, &request))?;
        let unit_id = ContentUnitId::parse(unit).map_err(|_| PublicationError::InvalidId)?;
        let mut client = self.store.connection().await?;
        let tx = client.transaction().await?;
        let content =
            catalog::production_content(&tx, Some(token), None, &unit_id, LockMode::Update)
                .await
                .map_err(|e| match e {
                    catalog::CatalogStoreError::Storage(e) => e,
                    _ => StoreError::CorruptRevision,
                })?;
        let actor = content.work.owner_id().as_str();
        if let Some(prior) = sql::prior(&tx, actor, &request.operation_id, "withdraw").await? {
            if prior.request_digest != digest || prior.unit != unit {
                return Err(StoreError::OperationReused.into());
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(dto::AudioReleaseWithdrawalResponse {
                pointer_version: prior.version,
                withdrawn_release_id: prior.release,
            });
        }
        if sql::release_candidate(&tx, &request.release_id, unit, actor)
            .await?
            .is_none()
        {
            return Err(StoreError::NotFound.into());
        }
        let pointer = sql::pointer(&tx, unit, actor, true).await?;
        if pointer.version != request.expected_pointer_version {
            return Err(conflict(&pointer));
        }
        let now = lifecycle_sql::now(&tx).await?;
        let next = if pointer.active.as_deref() == Some(request.release_id.as_str()) {
            advance_pointer(pointer.version, pointer.version)?
        } else {
            pointer.version
        };
        if !sql::withdrawn(&tx, &request.release_id).await? {
            sql::withdraw(&tx, &request.release_id, actor, now, &request.reason).await?;
            sql::event(
                &tx,
                sql::Event {
                    id: &Uuid::new_v4().to_string(),
                    unit,
                    version: next,
                    kind: "withdrawn",
                    release: &request.release_id,
                    owner: actor,
                    now,
                },
            )
            .await?;
        }
        if next != pointer.version {
            sql::set_pointer(&tx, unit, actor, &pointer, None, next).await?;
        }
        sql::record(
            &tx,
            actor,
            &request.operation_id,
            "withdraw",
            &sql::Operation {
                request_digest: digest,
                unit: unit.into(),
                release: request.release_id.clone(),
                version: next,
            },
        )
        .await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(dto::AudioReleaseWithdrawalResponse {
            pointer_version: next,
            withdrawn_release_id: request.release_id,
        })
    }
}

#[cfg(test)]
#[path = "publication_application_tests.rs"]
mod tests;
