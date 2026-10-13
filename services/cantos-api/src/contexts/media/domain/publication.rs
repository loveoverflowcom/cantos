//! Immutable release rules. No database, HTTP, clock, object SDK or producer execution.
use super::{
    audio_profile::DeliveryProfile,
    lifecycle::{sha256_valid, AudioRendition, AudioReview, Binding, ReleaseCandidate},
};
use crate::contexts::{
    catalog::domain::{
        content::{ContentBody, ContentDetails, ContentRevision},
        ContentUnit, Lifecycle, OwnerId, Work,
    },
    production::domain::brief::fingerprint,
};
use crate::platform::ids::canonical_uuid;
use cantos_contracts as dto;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationError {
    InvalidId,
    InvalidDigest,
    InvalidEvidence,
    InvalidTime,
    InvalidSequence,
    BindingMismatch,
    Encoding,
    PointerConflict,
    VersionExhausted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseId(String);
impl ReleaseId {
    pub fn parse(raw: &str) -> Result<Self, PublicationError> {
        if canonical_uuid(raw).as_deref() != Some(raw) {
            return Err(PublicationError::InvalidId);
        }
        Ok(Self(raw.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sha256(String);
impl Sha256 {
    pub fn parse(raw: &str) -> Result<Self, PublicationError> {
        if !sha256_valid(raw) {
            return Err(PublicationError::InvalidDigest);
        }
        Ok(Self(raw.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
fn digest<T: serde::Serialize>(scheme: &str, raw: &T) -> Result<String, PublicationError> {
    fingerprint(scheme, raw).map_err(|_| PublicationError::Encoding)
}
fn check_digest(raw: &str, scheme: &str) -> Result<(), PublicationError> {
    let hex = raw
        .strip_prefix(&format!("{scheme}:sha256:"))
        .ok_or(PublicationError::InvalidDigest)?;
    Sha256::parse(hex)?;
    Ok(())
}
fn text(raw: &str, max: usize) -> bool {
    !raw.trim().is_empty() && raw.len() <= max && !raw.chars().any(char::is_control)
}
fn evidence(raw: &[String]) -> bool {
    !raw.is_empty()
        && raw.len() <= 64
        && raw.iter().all(|s| text(s, 256))
        && raw.iter().collect::<BTreeSet<_>>().len() == raw.len()
}
fn record_identity(
    id: &str,
    actor: &str,
    operation: &str,
    sequence: u64,
    at: i64,
) -> Result<(), PublicationError> {
    ReleaseId::parse(id)?;
    ReleaseId::parse(operation)?;
    OwnerId::parse(actor).map_err(|_| PublicationError::InvalidId)?;
    if sequence == 0 || sequence > i64::MAX as u64 {
        return Err(PublicationError::InvalidSequence);
    }
    if at < 0 {
        return Err(PublicationError::InvalidTime);
    }
    Ok(())
}
/// An owner-recorded distribution decision, never inferred from producer declarations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistributionClearance(dto::DistributionClearanceDocument);
impl DistributionClearance {
    pub fn record(
        mut raw: dto::DistributionClearanceDocument,
        c: &ReleaseCandidate,
    ) -> Result<Self, PublicationError> {
        record_identity(
            &raw.id,
            &raw.actor_id,
            &raw.request.operation_id,
            raw.sequence,
            raw.recorded_at,
        )?;
        let d = c.document();
        if raw.candidate_id != d.id
            || raw.actor_id != d.created_by
            || raw.request.candidate_digest != d.digest
        {
            return Err(PublicationError::BindingMismatch);
        }
        if raw.recorded_at < d.created_at || raw.request.valid_until <= raw.recorded_at {
            return Err(PublicationError::InvalidTime);
        }
        if !evidence(&raw.request.rights_refs)
            || !evidence(&raw.request.evidence_refs)
            || !text(&raw.request.notes, 4096)
        {
            return Err(PublicationError::InvalidEvidence);
        }
        raw.request.rights_refs.sort();
        raw.request.evidence_refs.sort();
        raw.digest.clear();
        raw.digest = digest("dc-r1", &raw)?;
        if serde_json::to_vec(&raw)
            .map_err(|_| PublicationError::Encoding)?
            .len()
            > 32768
        {
            return Err(PublicationError::InvalidEvidence);
        }
        Ok(Self(raw))
    }
    pub fn restore(
        raw: dto::DistributionClearanceDocument,
        c: &ReleaseCandidate,
    ) -> Result<Self, PublicationError> {
        let admitted = Self::record(raw.clone(), c)?;
        if admitted.0 != raw {
            return Err(PublicationError::InvalidDigest);
        }
        Ok(admitted)
    }
    pub fn document(&self) -> &dto::DistributionClearanceDocument {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationApproval(dto::PublicationApprovalDocument);
impl PublicationApproval {
    pub fn record(
        mut raw: dto::PublicationApprovalDocument,
        c: &ReleaseCandidate,
    ) -> Result<Self, PublicationError> {
        record_identity(
            &raw.id,
            &raw.actor_id,
            &raw.request.operation_id,
            raw.sequence,
            raw.recorded_at,
        )?;
        if raw.candidate_id != c.document().id
            || raw.actor_id != c.document().created_by
            || raw.request.candidate_digest != c.document().digest
        {
            return Err(PublicationError::BindingMismatch);
        }
        if raw.recorded_at < c.document().created_at {
            return Err(PublicationError::InvalidTime);
        }
        check_digest(&raw.request.clearance_digest, "dc-r1")?;
        if !text(&raw.request.notes, 4096) {
            return Err(PublicationError::InvalidEvidence);
        }
        raw.digest.clear();
        raw.digest = digest("pa-r1", &raw)?;
        if serde_json::to_vec(&raw)
            .map_err(|_| PublicationError::Encoding)?
            .len()
            > 8192
        {
            return Err(PublicationError::InvalidEvidence);
        }
        Ok(Self(raw))
    }
    pub fn restore(
        raw: dto::PublicationApprovalDocument,
        c: &ReleaseCandidate,
    ) -> Result<Self, PublicationError> {
        let admitted = Self::record(raw.clone(), c)?;
        if admitted.0 != raw {
            return Err(PublicationError::InvalidDigest);
        }
        Ok(admitted)
    }
    pub fn document(&self) -> &dto::PublicationApprovalDocument {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectObservation {
    Verified { sha256: Sha256, byte_length: u64 },
    Missing,
    Unverified,
    IntegrityMismatch,
}
/// A provider-neutral delivery observation after bytes are ready through the intended path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryEvidence {
    object_id: String,
    sha256: Sha256,
    byte_length: u64,
    verified_at: i64,
    valid_until: i64,
}
impl DeliveryEvidence {
    pub fn new(
        object_id: String,
        sha256: Sha256,
        byte_length: u64,
        verified_at: i64,
        valid_until: i64,
    ) -> Result<Self, PublicationError> {
        // No path, URL, credentials or content-addressed staging identifier is admitted.
        if !object_id.starts_with("publication-")
            || object_id.len() > 128
            || !object_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(PublicationError::InvalidId);
        }
        if byte_length == 0 || byte_length > 32 * 1024 * 1024 {
            return Err(PublicationError::InvalidEvidence);
        }
        if verified_at < 0 || valid_until <= verified_at {
            return Err(PublicationError::InvalidTime);
        }
        Ok(Self {
            object_id,
            sha256,
            byte_length,
            verified_at,
            valid_until,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeliveryObservation {
    Verified(DeliveryEvidence),
    Unavailable,
    Missing,
    IntegrityMismatch,
}

pub struct PublicationFacts<'a> {
    pub candidate: &'a ReleaseCandidate,
    pub rendition: &'a AudioRendition,
    pub work: &'a Work,
    pub unit: &'a ContentUnit,
    pub content: &'a ContentRevision,
    pub upload_expires_at: i64,
    pub actor: &'a OwnerId,
    pub current_brief: bool,
    pub upload_revoked: bool,
    pub reviews: &'a [AudioReview],
    pub rights: Option<&'a DistributionClearance>,
    pub approval: Option<&'a PublicationApproval>,
    pub object: &'a ObjectObservation,
    pub delivery: &'a DeliveryObservation,
    pub now: i64,
}
/// Only the evaluator may create this ephemeral witness. It cannot deserialize or default.
/// ```compile_fail,E0277
/// use cantos_api::contexts::media::domain::publication::PublishableRelease;
/// let forged = serde_json::from_str::<PublishableRelease>("{}");
/// ```
/// ```compile_fail,E0451
/// use cantos_api::contexts::media::domain::publication::PublishableRelease;
/// let forged = PublishableRelease { manifest: todo!() };
/// ```
pub struct PublishableRelease {
    manifest: dto::ReleaseManifestDocument,
    evaluated_at: i64,
}
impl PublishableRelease {
    pub fn manifest(&self) -> &dto::ReleaseManifestDocument {
        &self.manifest
    }
}

pub fn evaluate(f: &PublicationFacts<'_>) -> Vec<dto::PublicationBlocker> {
    use dto::PublicationBlocker as B;
    let mut b = BTreeSet::new();
    let c = f.candidate.document();
    let r = f.rendition.document();
    let m = &r.verified_media;
    if c.rendition_id != r.id
        || c.rendition_digest != r.digest
        || c.binding != r.binding
        || c.sha256 != m.sha256
        || c.binding.work_id != f.work.id().as_str()
        || c.binding.unit_id != f.unit.id().as_str()
        || f.unit.work_id() != f.work.id()
        || c.binding.content_revision_id != f.content.id().as_str()
        || f.content
            .revision_digest()
            .map(|d| d.as_str().to_owned())
            .as_deref()
            != Ok(c.binding.content_revision_digest.as_str())
    {
        b.insert(B::CandidateMismatch);
    }
    if f.actor != f.work.owner_id() || c.created_by != f.actor.as_str() {
        b.insert(B::OwnerScopeLost);
    }
    if f.work.lifecycle() == Lifecycle::Archived || f.unit.lifecycle() == Lifecycle::Archived {
        b.insert(B::Archived);
    }
    if !f.current_brief {
        b.insert(B::StaleBrief);
    }
    if (c.work_metadata_revision, c.unit_metadata_revision)
        != (f.work.revision(), f.unit.revision())
    {
        b.insert(B::MetadataChanged);
    }
    if f.now >= f.upload_expires_at {
        b.insert(B::UploadPermissionExpired);
    }
    if f.upload_revoked {
        b.insert(B::UploadPermissionRevoked);
    }
    if DeliveryProfile::from_id(&m.profile_id).is_none() || m.profile_id != c.binding.profile_id {
        b.insert(B::ProfileUnsupported);
    }
    match f.object {
        ObjectObservation::Verified {
            sha256,
            byte_length,
        } if sha256.as_str() == m.sha256 && *byte_length == m.byte_length => {}
        ObjectObservation::Verified { .. } | ObjectObservation::IntegrityMismatch => {
            b.insert(B::ObjectIntegrityMismatch);
        }
        ObjectObservation::Missing => {
            b.insert(B::ObjectMissing);
        }
        ObjectObservation::Unverified => {
            b.insert(B::ObjectUnverified);
        }
    }
    let latest: Vec<_> = [
        dto::AudioReviewKind::Qc,
        dto::AudioReviewKind::Rights,
        dto::AudioReviewKind::Editorial,
    ]
    .into_iter()
    .filter_map(|kind| {
        f.reviews
            .iter()
            .filter(|r| r.document().request.kind == kind)
            .max_by_key(|r| r.document().sequence)
    })
    .collect();
    for (kind, blocker) in [
        (dto::AudioReviewKind::Qc, B::QcNotApproved),
        (dto::AudioReviewKind::Rights, B::RightsReviewNotApproved),
        (dto::AudioReviewKind::Editorial, B::EditorialNotApproved),
    ] {
        if !latest.iter().any(|v| {
            let v = v.document();
            v.request.kind == kind
                && v.rendition_id == r.id
                && v.request.rendition_digest == r.digest
                && v.request.sha256 == m.sha256
                && v.reviewer_id == f.work.owner_id().as_str()
                && v.reviewed_at <= f.now
                && v.request.decision == dto::AudioReviewDecision::Approved
        }) {
            b.insert(blocker);
        }
    }
    if c.review_ids
        != latest
            .iter()
            .map(|r| r.document().id.clone())
            .collect::<Vec<_>>()
    {
        b.insert(B::CandidateReviewsChanged);
    }
    match f.rights {
        None => {
            b.insert(B::RightsUnrecorded);
        }
        Some(rights) => {
            let d = rights.document();
            if d.request.decision == dto::DistributionDecision::Revoked {
                b.insert(B::RightsRevoked);
            }
            if !rights_window_valid(d.recorded_at, d.request.valid_until, f.now) {
                b.insert(B::RightsExpired);
            }
            let required: BTreeSet<_> = r
                .declarations
                .rights_refs
                .iter()
                .chain(f.work.details().rights_refs.iter())
                .chain(f.content.details().rights_refs().iter())
                .collect();
            if d.candidate_id != c.id
                || d.request.candidate_digest != c.digest
                || d.actor_id != f.work.owner_id().as_str()
                || required.is_empty()
                || required != d.request.rights_refs.iter().collect::<BTreeSet<_>>()
            {
                b.insert(B::RightsScopeMismatch);
            }
        }
    }
    match f.approval {
        None => {
            b.insert(B::ApprovalMissing);
        }
        Some(approval) => {
            let a = approval.document();
            if a.request.decision == dto::PublicationApprovalDecision::Withdrawn {
                b.insert(B::ApprovalWithdrawn);
            }
            if a.recorded_at > f.now
                || a.candidate_id != c.id
                || a.request.candidate_digest != c.digest
                || a.actor_id != f.work.owner_id().as_str()
                || f.rights.is_none_or(|r| {
                    r.document().digest != a.request.clearance_digest
                        || r.document().recorded_at > a.recorded_at
                })
            {
                b.insert(B::ApprovalStale);
            }
        }
    }
    match f.delivery {
        DeliveryObservation::Unavailable => {
            b.insert(B::DeliveryUnavailable);
        }
        DeliveryObservation::Missing => {
            b.insert(B::DeliveryMissing);
        }
        DeliveryObservation::IntegrityMismatch => {
            b.insert(B::DeliveryIntegrityMismatch);
        }
        DeliveryObservation::Verified(d) => {
            if d.sha256.as_str() != m.sha256 || d.byte_length != m.byte_length {
                b.insert(B::DeliveryIntegrityMismatch);
            }
            if !rights_window_valid(d.verified_at, d.valid_until, f.now) {
                b.insert(B::DeliveryEvidenceExpired);
            }
        }
    }
    b.into_iter().collect()
}
/// The half-open validity kernel is shared by commit-time rights and delivery checks.
pub fn rights_window_valid(start: i64, end: i64, now: i64) -> bool {
    start >= 0 && start <= now && now < end
}
/// Checked optimistic pointer advancement, also used by the persistence shell.
pub fn advance_pointer(actual: u64, expected: u64) -> Result<u64, PublicationError> {
    if actual != expected {
        return Err(PublicationError::PointerConflict);
    }
    actual
        .checked_add(1)
        .filter(|v| *v <= i64::MAX as u64)
        .ok_or(PublicationError::VersionExhausted)
}
pub fn decide(
    f: &PublicationFacts<'_>,
) -> Result<PublishableRelease, Vec<dto::PublicationBlocker>> {
    let blockers = evaluate(f);
    if !blockers.is_empty() {
        return Err(blockers);
    }
    // Exhaustive observations: no stored witness/producer boolean can bypass the evaluator.
    let (Some(rights), Some(approval), DeliveryObservation::Verified(delivery)) =
        (f.rights, f.approval, f.delivery)
    else {
        return Err(vec![dto::PublicationBlocker::DeliveryUnavailable]);
    };
    let c = f.candidate.document();
    let r = f.rendition.document();
    let mut manifest = dto::ReleaseManifestDocument {
        schema_version: "1".into(),
        work_id: f.work.id().as_str().into(),
        unit_id: f.unit.id().as_str().into(),
        work_metadata_revision: f.work.revision(),
        unit_metadata_revision: f.unit.revision(),
        work_title: f.work.details().title.clone(),
        unit_title: f.unit.details().title.clone(),
        language: f.content.details().language().into(),
        rights_refs: r
            .declarations
            .rights_refs
            .iter()
            .chain(f.work.details().rights_refs.iter())
            .chain(f.content.details().rights_refs().iter())
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        candidate_id: c.id.clone(),
        candidate_digest: c.digest.clone(),
        rendition_id: r.id.clone(),
        rendition_digest: r.digest.clone(),
        binding: c.binding.clone(),
        media: r.verified_media.clone(),
        review_evidence: [
            dto::AudioReviewKind::Qc,
            dto::AudioReviewKind::Rights,
            dto::AudioReviewKind::Editorial,
        ]
        .into_iter()
        .filter_map(|kind| {
            f.reviews
                .iter()
                .filter(|r| r.document().request.kind == kind)
                .max_by_key(|r| r.document().sequence)
        })
        .map(|r| r.document().clone())
        .collect(),
        distribution_clearance: rights.document().clone(),
        publication_approval: approval.document().clone(),
        delivery_object_id: delivery.object_id.clone(),
        retention: dto::ReleaseRetentionPolicy::RetainUntilRightsExpiryOrWithdrawal,
        digest: String::new(),
    };
    manifest.review_evidence.sort_by_key(|r| r.request.kind);
    manifest.digest =
        digest("rm-r1", &manifest).map_err(|_| vec![dto::PublicationBlocker::CandidateMismatch])?;
    Ok(PublishableRelease {
        manifest,
        evaluated_at: f.now,
    })
}
/// Immutable settled release. Restoration checks the same digest and intrinsic bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioRelease(dto::AudioReleaseDocument);
impl AudioRelease {
    pub fn publish(
        id: ReleaseId,
        actor: &OwnerId,
        at: i64,
        supersedes: Option<ReleaseId>,
        witness: PublishableRelease,
    ) -> Result<Self, PublicationError> {
        if at != witness.evaluated_at {
            return Err(PublicationError::InvalidTime);
        }
        let mut raw = dto::AudioReleaseDocument {
            id: id.0,
            published_by: actor.as_str().into(),
            published_at: at,
            supersedes: supersedes.map(|id| id.0),
            manifest: witness.manifest,
            digest: String::new(),
        };
        raw.digest = digest("rl-r1", &raw)?;
        Self::restore_intrinsic(raw)
    }
    pub fn restore(
        raw: dto::AudioReleaseDocument,
        candidate: &ReleaseCandidate,
        rendition: &AudioRendition,
        content: &ContentRevision,
    ) -> Result<Self, PublicationError> {
        let m = &raw.manifest;
        let c = candidate.document();
        let r = rendition.document();
        if m.language != content.details().language()
            || m.binding.content_revision_id != content.id().as_str()
            || content
                .revision_digest()
                .map(|d| d.as_str().to_owned())
                .as_deref()
                != Ok(m.binding.content_revision_digest.as_str())
            || content
                .details()
                .rights_refs()
                .iter()
                .chain(r.declarations.rights_refs.iter())
                .any(|id| !m.rights_refs.contains(id))
            || c.rendition_id != r.id
            || c.rendition_digest != r.digest
            || c.binding != r.binding
            || c.sha256 != r.verified_media.sha256
            || m.candidate_id != c.id
            || m.candidate_digest != c.digest
            || m.rendition_id != r.id
            || m.rendition_digest != r.digest
            || m.binding != c.binding
            || m.media != r.verified_media
            || m.work_metadata_revision != c.work_metadata_revision
            || m.unit_metadata_revision != c.unit_metadata_revision
        {
            return Err(PublicationError::BindingMismatch);
        }
        let rights = DistributionClearance::restore(m.distribution_clearance.clone(), candidate)?;
        let approval = PublicationApproval::restore(m.publication_approval.clone(), candidate)?;
        if rights.document().request.decision != dto::DistributionDecision::Cleared
            || approval.document().request.decision != dto::PublicationApprovalDecision::Approved
            || approval.document().request.clearance_digest != rights.document().digest
            || approval.document().recorded_at < rights.document().recorded_at
            || approval.document().recorded_at > raw.published_at
            || m.rights_refs != rights.document().request.rights_refs
        {
            return Err(PublicationError::BindingMismatch);
        }
        if m.review_evidence.len() != 3
            || m.review_evidence
                .iter()
                .map(|r| r.id.clone())
                .collect::<Vec<_>>()
                != c.review_ids
        {
            return Err(PublicationError::BindingMismatch);
        }
        for (review, kind) in m.review_evidence.iter().zip([
            dto::AudioReviewKind::Qc,
            dto::AudioReviewKind::Rights,
            dto::AudioReviewKind::Editorial,
        ]) {
            AudioReview::restore(review.clone(), rendition)
                .map_err(|_| PublicationError::BindingMismatch)?;
            if review.request.kind != kind
                || review.request.decision != dto::AudioReviewDecision::Approved
                || review.reviewer_id != raw.published_by
                || review.reviewed_at > raw.published_at
            {
                return Err(PublicationError::BindingMismatch);
            }
        }
        Self::restore_intrinsic(raw)
    }
    fn restore_intrinsic(raw: dto::AudioReleaseDocument) -> Result<Self, PublicationError> {
        ReleaseId::parse(&raw.id)?;
        OwnerId::parse(&raw.published_by).map_err(|_| PublicationError::InvalidId)?;
        if let Some(id) = &raw.supersedes {
            ReleaseId::parse(id)?;
            if id == &raw.id {
                return Err(PublicationError::BindingMismatch);
            }
        }
        let m = &raw.manifest;
        ContentDetails::new(
            m.language.clone(),
            ContentBody::Recording,
            vec![],
            "Release snapshot".into(),
        )
        .map_err(|_| PublicationError::BindingMismatch)?;
        if !text(&m.work_title, 512) || !text(&m.unit_title, 512) {
            return Err(PublicationError::BindingMismatch);
        }
        DeliveryEvidence::new(
            m.delivery_object_id.clone(),
            Sha256::parse(&m.media.sha256)?,
            m.media.byte_length,
            raw.published_at,
            raw.published_at
                .checked_add(1)
                .ok_or(PublicationError::InvalidTime)?,
        )?;
        Binding::admit(m.binding.clone()).map_err(|_| PublicationError::BindingMismatch)?;
        Sha256::parse(&m.media.sha256)?;
        ReleaseId::parse(&m.candidate_id)?;
        ReleaseId::parse(&m.rendition_id)?;
        if m.schema_version != "1"
            || m.work_id != m.binding.work_id
            || m.unit_id != m.binding.unit_id
            || m.media.profile_id != m.binding.profile_id
            || m.work_metadata_revision == 0
            || m.unit_metadata_revision == 0
            || raw.published_by != m.publication_approval.actor_id
            || !rights_window_valid(
                m.distribution_clearance.recorded_at,
                m.distribution_clearance.request.valid_until,
                raw.published_at,
            )
        {
            return Err(PublicationError::BindingMismatch);
        }
        let mut manifest = m.clone();
        manifest.digest.clear();
        if digest("rm-r1", &manifest)? != m.digest {
            return Err(PublicationError::InvalidDigest);
        }
        let mut check = raw.clone();
        check.digest.clear();
        if digest("rl-r1", &check)? != raw.digest {
            return Err(PublicationError::InvalidDigest);
        }
        if serde_json::to_vec(&raw)
            .map_err(|_| PublicationError::Encoding)?
            .len()
            > 262144
        {
            return Err(PublicationError::InvalidEvidence);
        }
        Ok(Self(raw))
    }
    pub fn document(&self) -> &dto::AudioReleaseDocument {
        &self.0
    }
}

/// Retained manifests are never re-evaluated against newer editorial/metadata revisions.
/// Exact approval/rights withdrawal or expiry stops resolution immediately; historical bytes
/// are never deleted and a rollback must create a new gated release.
pub fn retained_release_eligible(
    release: &AudioRelease,
    rights: Option<&DistributionClearance>,
    approval: Option<&PublicationApproval>,
    withdrawn: bool,
    now: i64,
) -> bool {
    let m = &release.document().manifest;
    !withdrawn
        && rights.is_some_and(|r| {
            r.document().digest == m.distribution_clearance.digest
                && r.document().request.decision == dto::DistributionDecision::Cleared
                && rights_window_valid(
                    r.document().recorded_at,
                    r.document().request.valid_until,
                    now,
                )
        })
        && approval.is_some_and(|a| {
            a.document().digest == m.publication_approval.digest
                && a.document().request.decision == dto::PublicationApprovalDecision::Approved
                && a.document().recorded_at <= now
        })
}
pub fn delivery_matches_manifest(
    delivery: &DeliveryObservation,
    manifest: &dto::ReleaseManifestDocument,
    now: i64,
) -> bool {
    match delivery {
        DeliveryObservation::Verified(e) => {
            e.object_id == manifest.delivery_object_id
                && e.sha256.as_str() == manifest.media.sha256
                && e.byte_length == manifest.media.byte_length
                && rights_window_valid(e.verified_at, e.valid_until, now)
        }
        DeliveryObservation::Unavailable
        | DeliveryObservation::Missing
        | DeliveryObservation::IntegrityMismatch => false,
    }
}

#[cfg(test)]
#[path = "../publication_tests.rs"]
pub(crate) mod tests;
#[cfg(kani)]
mod kani_proofs {
    use super::{advance_pointer, rights_window_valid, PublicationError};
    /// Full u64 pointers and i64 clocks, no assumptions/stubs, unwind2, production kernels.
    /// Proves exact conflict/exhaustion/half-open expiry; not SQL/storage/QC/authorization.
    #[kani::proof]
    #[kani::unwind(2)]
    fn publication_pointer_and_rights_fail_closed() {
        let actual: u64 = kani::any();
        let expected: u64 = kani::any();
        match advance_pointer(actual, expected) {
            Ok(next) => {
                assert_eq!(actual, expected);
                assert!(actual < (i64::MAX as u64));
                assert_eq!(next, actual + 1);
            }
            Err(PublicationError::PointerConflict) => assert_ne!(actual, expected),
            Err(PublicationError::VersionExhausted) => {
                assert_eq!(actual, expected);
                assert!(actual >= i64::MAX as u64);
            }
            Err(_) => assert!(false, "unexpected pointer error"),
        }
        let start: i64 = kani::any();
        let end: i64 = kani::any();
        let now: i64 = kani::any();
        let expected_validity = start >= 0 && now >= start && end > now;
        assert_eq!(rights_window_valid(start, end, now), expected_validity);
        kani::cover!(actual != expected, "conflict rejected");
        kani::cover!(
            actual == expected && actual == i64::MAX as u64,
            "exhaustion rejected"
        );
        kani::cover!(
            actual == expected && actual == 0,
            "initial pointer advances"
        );
        kani::cover!(
            start >= 0 && start == now && end > now,
            "inclusive validity start"
        );
        kani::cover!(start >= 0 && end == now, "exclusive expiry rejected");
        kani::cover!(start < 0, "negative start rejected");
    }
}
