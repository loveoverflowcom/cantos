//! Studio Web representation of Publication v1. Server-owned facts never grant client authority.
use super::{AudioBinding, AudioMediaMetadata, AudioReviewDocument};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DistributionDecision {
    Cleared,
    Revoked,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordDistributionRightsRequest {
    pub operation_id: String,
    pub candidate_digest: String,
    pub decision: DistributionDecision,
    pub rights_refs: Vec<String>,
    pub valid_until: i64,
    pub evidence_refs: Vec<String>,
    pub notes: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DistributionClearanceDocument {
    pub id: String,
    pub candidate_id: String,
    pub actor_id: String,
    pub sequence: u64,
    pub recorded_at: i64,
    pub request: RecordDistributionRightsRequest,
    pub digest: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PublicationApprovalDecision {
    Approved,
    Withdrawn,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordPublicationApprovalRequest {
    pub operation_id: String,
    pub candidate_digest: String,
    pub clearance_digest: String,
    pub decision: PublicationApprovalDecision,
    pub notes: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublicationApprovalDocument {
    pub id: String,
    pub candidate_id: String,
    pub actor_id: String,
    pub sequence: u64,
    pub recorded_at: i64,
    pub request: RecordPublicationApprovalRequest,
    pub digest: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PublicationBlocker {
    CandidateMismatch,
    OwnerScopeLost,
    Archived,
    StaleBrief,
    MetadataChanged,
    UploadPermissionRevoked,
    UploadPermissionExpired,
    ProfileUnsupported,
    ObjectMissing,
    ObjectUnverified,
    ObjectIntegrityMismatch,
    QcNotApproved,
    RightsReviewNotApproved,
    EditorialNotApproved,
    CandidateReviewsChanged,
    RightsUnrecorded,
    RightsRevoked,
    RightsExpired,
    RightsScopeMismatch,
    ApprovalMissing,
    ApprovalWithdrawn,
    ApprovalStale,
    DeliveryUnavailable,
    DeliveryMissing,
    DeliveryIntegrityMismatch,
    DeliveryEvidenceExpired,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublicationReadinessResponse {
    pub candidate_id: String,
    pub candidate_digest: String,
    pub pointer_version: u64,
    pub active_release_id: Option<String>,
    pub blockers: Vec<PublicationBlocker>,
    pub clearance: Option<DistributionClearanceDocument>,
    pub approval: Option<PublicationApprovalDocument>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PublishAudioReleaseRequest {
    pub operation_id: String,
    pub candidate_digest: String,
    pub expected_pointer_version: u64,
    pub expected_active_release_id: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRetentionPolicy {
    RetainUntilRightsExpiryOrWithdrawal,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifestDocument {
    pub schema_version: String,
    pub work_id: String,
    pub unit_id: String,
    pub work_metadata_revision: u64,
    pub unit_metadata_revision: u64,
    pub work_title: String,
    pub unit_title: String,
    pub language: String,
    pub rights_refs: Vec<String>,
    pub candidate_id: String,
    pub candidate_digest: String,
    pub rendition_id: String,
    pub rendition_digest: String,
    pub binding: AudioBinding,
    pub media: AudioMediaMetadata,
    pub review_evidence: Vec<AudioReviewDocument>,
    pub distribution_clearance: DistributionClearanceDocument,
    pub publication_approval: PublicationApprovalDocument,
    /// Opaque publication-class identity. Never a URL or a staging object key.
    pub delivery_object_id: String,
    pub retention: ReleaseRetentionPolicy,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioReleaseDocument {
    pub id: String,
    pub published_by: String,
    pub published_at: i64,
    pub supersedes: Option<String>,
    pub manifest: ReleaseManifestDocument,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActiveAudioReleaseResponse {
    pub pointer_version: u64,
    pub release: Option<AudioReleaseDocument>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WithdrawAudioReleaseRequest {
    pub operation_id: String,
    pub release_id: String,
    pub expected_pointer_version: u64,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioReleaseWithdrawalResponse {
    pub pointer_version: u64,
    pub withdrawn_release_id: String,
}
