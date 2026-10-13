//! Immutable audio admission and review values. No storage/HTTP/SQL/clock dependencies.
use super::audio_profile::DeliveryProfile;
use crate::contexts::catalog::domain::{
    content::ContentRevisionId, ContentUnitId, OwnerId, WorkId,
};
use crate::contexts::production::domain::brief::{
    fingerprint, BriefDigest, ProductionBrief, ProductionBriefId,
};
use crate::platform::ids::canonical_uuid;
use cantos_contracts as dto;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioError {
    InvalidId,
    InvalidDigest,
    InvalidDeclaration,
    InvalidExpiry,
    InvalidSize,
    BindingMismatch,
    DigestMismatch,
    OperationReused,
    StaleBrief,
    PermissionRevoked,
    PermissionExpired,
    InvalidTransition,
    Encoding,
}
macro_rules! identity {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(String);
        impl $name {
            pub fn parse(value: &str) -> Result<Self, AudioError> {
                if canonical_uuid(value).as_deref() != Some(value) {
                    return Err(AudioError::InvalidId);
                }
                Ok(Self(value.into()))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
identity!(AudioSubmissionId);
identity!(AudioRenditionId);
identity!(AudioReviewId);
identity!(ReleaseCandidateId);

pub fn sha256_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn digest<T: Serialize>(scheme: &str, value: &T) -> Result<String, AudioError> {
    fingerprint(scheme, value).map_err(|_| AudioError::Encoding)
}
fn bounded_snapshot<T: Serialize>(value: &T) -> Result<(), AudioError> {
    if serde_json::to_vec(value)
        .map_err(|_| AudioError::Encoding)?
        .len()
        > 32768
    {
        return Err(AudioError::InvalidSize);
    }
    Ok(())
}
fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
}
fn declarations(raw: &dto::ProducerDeclarations) -> Result<(), AudioError> {
    if !text(&raw.producer, 256)
        || !text(&raw.method, 256)
        || !text(&raw.provenance, 4096)
        || raw.quality_report.len() > 4096
        || raw
            .quality_report
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        || raw.rights_refs.is_empty()
        || raw.rights_refs.len() > 64
        || raw.rights_refs.iter().any(|s| !text(s, 128))
        || raw.rights_refs.iter().collect::<BTreeSet<_>>().len() != raw.rights_refs.len()
    {
        return Err(AudioError::InvalidDeclaration);
    }
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    work: WorkId,
    unit: ContentUnitId,
    content: ContentRevisionId,
    brief: ProductionBriefId,
    brief_digest: BriefDigest,
    profile: DeliveryProfile,
    raw: dto::AudioBinding,
}
impl Binding {
    pub fn from_brief(brief: &ProductionBrief) -> Result<Self, AudioError> {
        let d = brief.document();
        // A ProductionBrief is already admitted by its owner. No raw transport proves this.
        Self::admit(dto::AudioBinding {
            work_id: d.work.id.clone(),
            unit_id: d.content.unit_id.clone(),
            content_revision_id: d.content.id.clone(),
            content_revision_digest: d.content.revision_digest.clone(),
            brief_id: d.id.clone(),
            brief_version: d.version,
            brief_digest: d.digest.clone(),
            profile_id: d.guidance.delivery.profile_id.clone(),
        })
    }
    pub fn admit(raw: dto::AudioBinding) -> Result<Self, AudioError> {
        let work = WorkId::parse(&raw.work_id).map_err(|_| AudioError::InvalidId)?;
        let unit = ContentUnitId::parse(&raw.unit_id).map_err(|_| AudioError::InvalidId)?;
        let content = ContentRevisionId::parse(&raw.content_revision_id)
            .map_err(|_| AudioError::InvalidId)?;
        let brief = ProductionBriefId::parse(&raw.brief_id).map_err(|_| AudioError::InvalidId)?;
        let brief_digest =
            BriefDigest::parse(&raw.brief_digest).map_err(|_| AudioError::InvalidDigest)?;
        let profile =
            DeliveryProfile::from_id(&raw.profile_id).ok_or(AudioError::BindingMismatch)?;
        if work.as_str() != raw.work_id
            || unit.as_str() != raw.unit_id
            || content.as_str() != raw.content_revision_id
            || raw.brief_version == 0
            || raw.brief_version > i64::MAX as u64
            || !raw
                .content_revision_digest
                .strip_prefix("cc-r1:sha256:")
                .is_some_and(sha256_valid)
        {
            return Err(AudioError::BindingMismatch);
        }
        Ok(Self {
            work,
            unit,
            content,
            brief,
            brief_digest,
            profile,
            raw,
        })
    }
    pub fn raw(&self) -> &dto::AudioBinding {
        &self.raw
    }
    pub fn profile(&self) -> DeliveryProfile {
        self.profile
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSubmission {
    id: AudioSubmissionId,
    binding: Binding,
    raw: dto::AudioSubmissionDocument,
}
impl AudioSubmission {
    pub fn create(
        request: &dto::CreateAudioSubmissionRequest,
        brief: &ProductionBrief,
        owner: &OwnerId,
        now: i64,
    ) -> Result<Self, AudioError> {
        if request.binding != *Binding::from_brief(brief)?.raw()
            || owner.as_str() != brief.document().work.owner_id
        {
            return Err(AudioError::BindingMismatch);
        }
        if brief.document().work.rights_refs.is_empty()
            || request
                .declarations
                .rights_refs
                .iter()
                .collect::<BTreeSet<_>>()
                != brief
                    .document()
                    .work
                    .rights_refs
                    .iter()
                    .collect::<BTreeSet<_>>()
        {
            return Err(AudioError::InvalidDeclaration);
        }
        if now < 0 || request.expires_at <= now || request.expires_at - now > 86_400_000 {
            return Err(AudioError::InvalidExpiry);
        }
        let mut raw = dto::AudioSubmissionDocument {
            schema_version: "1".into(),
            id: request.id.clone(),
            operation_id: request.operation_id.clone(),
            binding: request.binding.clone(),
            expected_sha256: request.expected_sha256.clone(),
            expected_byte_length: request.expected_byte_length,
            declarations: request.declarations.clone(),
            owner_id: owner.as_str().into(),
            created_at: now,
            expires_at: request.expires_at,
            upload_path: format!(
                "/api/v1/studio/producer/audio-submissions/{}/bytes",
                request.id
            ),
            digest: String::new(),
        };
        raw.digest = digest("as-r1", &raw)?;
        Self::restore(raw)
    }
    pub fn restore(raw: dto::AudioSubmissionDocument) -> Result<Self, AudioError> {
        if serde_json::to_vec(&raw)
            .map_err(|_| AudioError::Encoding)?
            .len()
            > 32768
        {
            return Err(AudioError::InvalidSize);
        }
        let id = AudioSubmissionId::parse(&raw.id)?;
        AudioSubmissionId::parse(&raw.operation_id)?;
        OwnerId::parse(&raw.owner_id).map_err(|_| AudioError::InvalidId)?;
        let binding = Binding::admit(raw.binding.clone())?;
        declarations(&raw.declarations)?;
        if raw.schema_version != "1" || !sha256_valid(&raw.expected_sha256) {
            return Err(AudioError::InvalidDigest);
        }
        if raw.expected_byte_length == 0 || raw.expected_byte_length > 32 * 1024 * 1024 {
            return Err(AudioError::InvalidSize);
        }
        if raw.created_at < 0
            || raw.expires_at <= raw.created_at
            || raw.expires_at - raw.created_at > 86_400_000
        {
            return Err(AudioError::InvalidExpiry);
        }
        if raw.upload_path != format!("/api/v1/studio/producer/audio-submissions/{}/bytes", raw.id)
        {
            return Err(AudioError::BindingMismatch);
        }
        let mut check = raw.clone();
        check.digest.clear();
        if digest("as-r1", &check)? != raw.digest {
            return Err(AudioError::InvalidDigest);
        }
        Ok(Self { id, binding, raw })
    }
    pub fn id(&self) -> &AudioSubmissionId {
        &self.id
    }
    pub fn document(&self) -> &dto::AudioSubmissionDocument {
        &self.raw
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioRendition {
    id: AudioRenditionId,
    raw: dto::AudioRenditionDocument,
}
impl AudioRendition {
    pub(crate) fn create(
        submission: &AudioSubmission,
        verified: dto::AudioMediaMetadata,
        id: AudioRenditionId,
        now: i64,
    ) -> Result<Self, AudioError> {
        let mut raw = dto::AudioRenditionDocument {
            schema_version: "1".into(),
            id: id.as_str().into(),
            submission_id: submission.document().id.clone(),
            submission_digest: submission.document().digest.clone(),
            binding: submission.document().binding.clone(),
            declarations: submission.document().declarations.clone(),
            verified_media: verified,
            admitted_at: now,
            digest: String::new(),
        };
        raw.digest = digest("ar-r1", &raw)?;
        Self::restore(raw, submission)
    }
    pub(crate) fn restore(
        raw: dto::AudioRenditionDocument,
        submission: &AudioSubmission,
    ) -> Result<Self, AudioError> {
        bounded_snapshot(&raw)?;
        let id = AudioRenditionId::parse(&raw.id)?;
        let s = submission.document();
        let m = &raw.verified_media;
        let p = submission.binding().profile();
        if raw.schema_version != "1"
            || raw.submission_id != s.id
            || raw.submission_digest != s.digest
            || raw.binding != s.binding
            || raw.declarations != s.declarations
            || raw.admitted_at < s.created_at
            || raw.admitted_at >= s.expires_at
            || m.sha256 != s.expected_sha256
            || m.byte_length != s.expected_byte_length
            || m.container != p.container()
            || m.codec != p.codec()
            || m.profile_id != p.id()
            || m.channels != 1
            || m.sample_rate_hz != 48_000
            || m.duration_ms == 0
            || m.duration_ms > 3_600_000
            || m.audio_bitrate_bps == 0
            || m.audio_bitrate_bps > p.maximum_bitrate_bps()
        {
            return Err(AudioError::BindingMismatch);
        }
        let mut check = raw.clone();
        check.digest.clear();
        if digest("ar-r1", &check)? != raw.digest {
            return Err(AudioError::InvalidDigest);
        }
        Ok(Self { id, raw })
    }
    pub fn id(&self) -> &AudioRenditionId {
        &self.id
    }
    pub fn document(&self) -> &dto::AudioRenditionDocument {
        &self.raw
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionDecision {
    Inspect,
    Replay,
}
pub fn decide_admission(
    submission: &AudioSubmission,
    now: i64,
    revoked: bool,
    current: bool,
    sha256: &str,
    length: u64,
    settled: Option<&AudioRendition>,
) -> Result<AdmissionDecision, AudioError> {
    if revoked {
        return Err(AudioError::PermissionRevoked);
    }
    if now >= submission.document().expires_at {
        return Err(AudioError::PermissionExpired);
    }
    if !current {
        return Err(AudioError::StaleBrief);
    }
    if sha256 != submission.document().expected_sha256
        || length != submission.document().expected_byte_length
    {
        return Err(if settled.is_some() {
            AudioError::OperationReused
        } else {
            AudioError::DigestMismatch
        });
    }
    Ok(if settled.is_some() {
        AdmissionDecision::Replay
    } else {
        AdmissionDecision::Inspect
    })
}
pub fn submission_state(
    now: i64,
    expires: i64,
    revoked: bool,
    current: bool,
    admitted: bool,
) -> dto::AudioSubmissionState {
    if !current {
        dto::AudioSubmissionState::Stale
    } else if revoked {
        dto::AudioSubmissionState::Revoked
    } else if admitted {
        dto::AudioSubmissionState::Admitted
    } else if now >= expires {
        dto::AudioSubmissionState::Expired
    } else {
        dto::AudioSubmissionState::AwaitingBytes
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmissionEvent {
    Admit,
    Revoke,
    Expire,
    Invalidate,
}
pub fn transition(
    state: dto::AudioSubmissionState,
    event: SubmissionEvent,
) -> Result<dto::AudioSubmissionState, AudioError> {
    use dto::AudioSubmissionState as S;
    match (state, event) {
        (S::AwaitingBytes, SubmissionEvent::Admit) | (S::Admitted, SubmissionEvent::Admit) => {
            Ok(S::Admitted)
        }
        (S::AwaitingBytes | S::Admitted | S::Expired | S::Revoked, SubmissionEvent::Revoke) => {
            Ok(S::Revoked)
        }
        (S::AwaitingBytes | S::Expired, SubmissionEvent::Expire) => Ok(S::Expired),
        (_, SubmissionEvent::Invalidate) => Ok(S::Stale),
        (S::Stale, SubmissionEvent::Revoke) => Ok(S::Stale),
        (S::Stale, SubmissionEvent::Admit | SubmissionEvent::Expire)
        | (S::Revoked | S::Expired, SubmissionEvent::Admit)
        | (S::Admitted | S::Revoked, SubmissionEvent::Expire) => Err(AudioError::InvalidTransition),
    }
}
pub fn validate_review(
    request: &dto::CreateAudioReviewRequest,
    rendition: &AudioRendition,
) -> Result<(), AudioError> {
    AudioReviewId::parse(&request.operation_id)?;
    if request.rendition_digest != rendition.document().digest
        || request.sha256 != rendition.document().verified_media.sha256
    {
        return Err(AudioError::DigestMismatch);
    }
    if !text(&request.notes, 4096)
        || request.evidence_refs.is_empty()
        || request.evidence_refs.len() > 64
        || request.evidence_refs.iter().any(|s| !text(s, 256))
        || request.evidence_refs.iter().collect::<BTreeSet<_>>().len()
            != request.evidence_refs.len()
    {
        return Err(AudioError::InvalidDeclaration);
    }
    Ok(())
}
pub fn record_review(
    mut raw: dto::AudioReviewDocument,
    rendition: &AudioRendition,
) -> Result<dto::AudioReviewDocument, AudioError> {
    AudioReviewId::parse(&raw.id)?;
    AudioReviewId::parse(&raw.operation_id)?;
    if raw.operation_id != raw.request.operation_id {
        return Err(AudioError::BindingMismatch);
    }
    OwnerId::parse(&raw.reviewer_id).map_err(|_| AudioError::InvalidId)?;
    if raw.rendition_id != rendition.document().id
        || raw.sequence == 0
        || raw.sequence > i64::MAX as u64
        || raw.reviewed_at < rendition.document().admitted_at
    {
        return Err(AudioError::BindingMismatch);
    }
    validate_review(&raw.request, rendition)?;
    raw.digest.clear();
    raw.digest = digest("aq-r1", &raw)?;
    bounded_snapshot(&raw)?;
    Ok(raw)
}
pub fn latest_reviews(reviews: &[dto::AudioReviewDocument]) -> Vec<&dto::AudioReviewDocument> {
    [
        dto::AudioReviewKind::Qc,
        dto::AudioReviewKind::Rights,
        dto::AudioReviewKind::Editorial,
    ]
    .into_iter()
    .filter_map(|kind| {
        reviews
            .iter()
            .filter(|r| r.request.kind == kind)
            .max_by_key(|r| r.sequence)
    })
    .collect()
}
pub fn release_blockers(
    current: bool,
    revoked: bool,
    expired: bool,
    rights_recorded: bool,
    reviews: &[dto::AudioReviewDocument],
    candidate: Option<&dto::ReleaseCandidateDocument>,
    metadata: (u64, u64),
) -> Vec<dto::AudioReleaseBlocker> {
    use dto::AudioReleaseBlocker as B;
    let mut blockers = BTreeSet::from([B::PublicDeliveryNotImplemented]);
    if !current {
        blockers.insert(B::StaleBrief);
    }
    if revoked {
        blockers.insert(B::UploadPermissionRevoked);
    }
    if expired {
        blockers.insert(B::UploadPermissionExpired);
    }
    if !rights_recorded {
        blockers.insert(B::RightsUnrecorded);
    }
    let latest = latest_reviews(reviews);
    for (kind, blocker) in [
        (dto::AudioReviewKind::Qc, B::QcNotApproved),
        (dto::AudioReviewKind::Rights, B::RightsNotApproved),
        (dto::AudioReviewKind::Editorial, B::EditorialNotApproved),
    ] {
        if !latest.iter().any(|r| {
            r.request.kind == kind && r.request.decision == dto::AudioReviewDecision::Approved
        }) {
            blockers.insert(blocker);
        }
    }
    if let Some(c) = candidate {
        if c.review_ids != latest.iter().map(|r| r.id.clone()).collect::<Vec<_>>() {
            blockers.insert(B::CandidateReviewsChanged);
        }
        if (c.work_metadata_revision, c.unit_metadata_revision) != metadata {
            blockers.insert(B::MetadataChanged);
        }
    }
    blockers.into_iter().collect()
}
pub fn record_candidate(
    mut raw: dto::ReleaseCandidateDocument,
    rendition: &AudioRendition,
) -> Result<dto::ReleaseCandidateDocument, AudioError> {
    ReleaseCandidateId::parse(&raw.id)?;
    ReleaseCandidateId::parse(&raw.operation_id)?;
    OwnerId::parse(&raw.created_by).map_err(|_| AudioError::InvalidId)?;
    for id in &raw.review_ids {
        AudioReviewId::parse(id)?;
    }
    if raw.rendition_id != rendition.document().id
        || raw.rendition_digest != rendition.document().digest
        || raw.binding != rendition.document().binding
        || raw.sha256 != rendition.document().verified_media.sha256
        || raw.work_metadata_revision == 0
        || raw.work_metadata_revision > i64::MAX as u64
        || raw.unit_metadata_revision > i64::MAX as u64
        || raw.unit_metadata_revision == 0
        || raw.created_at < rendition.document().admitted_at
        || raw.review_ids.len() > 3
        || raw.review_ids.iter().collect::<BTreeSet<_>>().len() != raw.review_ids.len()
    {
        return Err(AudioError::BindingMismatch);
    }
    raw.digest.clear();
    raw.digest = digest("rc-r1", &raw)?;
    bounded_snapshot(&raw)?;
    Ok(raw)
}

#[cfg(test)]
#[path = "../lifecycle_tests.rs"]
mod tests;

/// Immutable admitted review value, distinct from raw transport and rendition identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioReview {
    id: AudioReviewId,
    raw: dto::AudioReviewDocument,
}
impl AudioReview {
    pub(crate) fn record(
        raw: dto::AudioReviewDocument,
        r: &AudioRendition,
    ) -> Result<Self, AudioError> {
        let raw = record_review(raw, r)?;
        Ok(Self {
            id: AudioReviewId::parse(&raw.id)?,
            raw,
        })
    }
    pub(crate) fn restore(
        raw: dto::AudioReviewDocument,
        r: &AudioRendition,
    ) -> Result<Self, AudioError> {
        let admitted = Self::record(raw.clone(), r)?;
        if admitted.raw != raw {
            return Err(AudioError::InvalidDigest);
        }
        Ok(admitted)
    }
    pub fn id(&self) -> &AudioReviewId {
        &self.id
    }
    pub fn document(&self) -> &dto::AudioReviewDocument {
        &self.raw
    }
}
/// Private candidate record only. This value is never a PublishableRelease witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseCandidate {
    id: ReleaseCandidateId,
    raw: dto::ReleaseCandidateDocument,
}
impl ReleaseCandidate {
    pub(crate) fn record(
        raw: dto::ReleaseCandidateDocument,
        r: &AudioRendition,
    ) -> Result<Self, AudioError> {
        let raw = record_candidate(raw, r)?;
        Ok(Self {
            id: ReleaseCandidateId::parse(&raw.id)?,
            raw,
        })
    }
    pub(crate) fn restore(
        raw: dto::ReleaseCandidateDocument,
        r: &AudioRendition,
    ) -> Result<Self, AudioError> {
        let admitted = Self::record(raw.clone(), r)?;
        if admitted.raw != raw {
            return Err(AudioError::InvalidDigest);
        }
        Ok(admitted)
    }
    pub fn id(&self) -> &ReleaseCandidateId {
        &self.id
    }
    pub fn document(&self) -> &dto::ReleaseCandidateDocument {
        &self.raw
    }
}
