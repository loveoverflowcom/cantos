//! Raw Audio Lifecycle v1 JSON transport. Declarations are never server media facts.
use super::AudioMediaMetadata;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioBinding {
    pub work_id: String,
    pub unit_id: String,
    pub content_revision_id: String,
    pub content_revision_digest: String,
    pub brief_id: String,
    pub brief_version: u64,
    pub brief_digest: String,
    pub profile_id: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProducerDeclarations {
    pub producer: String,
    pub method: String,
    pub provenance: String,
    pub rights_refs: Vec<String>,
    pub quality_report: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateAudioSubmissionRequest {
    pub operation_id: String,
    pub id: String,
    pub binding: AudioBinding,
    pub expected_sha256: String,
    pub expected_byte_length: u64,
    pub declarations: ProducerDeclarations,
    /// Fresh caller-generated random32bytes, lower hex; stored only as a separated hash.
    pub upload_token: String,
    pub expires_at: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioSubmissionDocument {
    pub schema_version: String,
    pub id: String,
    pub operation_id: String,
    pub binding: AudioBinding,
    pub expected_sha256: String,
    pub expected_byte_length: u64,
    pub declarations: ProducerDeclarations,
    pub owner_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub upload_path: String,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioRenditionDocument {
    pub schema_version: String,
    pub id: String,
    pub submission_id: String,
    pub submission_digest: String,
    pub binding: AudioBinding,
    pub declarations: ProducerDeclarations,
    pub verified_media: AudioMediaMetadata,
    pub admitted_at: i64,
    pub digest: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioSubmissionState {
    AwaitingBytes,
    Admitted,
    Revoked,
    Expired,
    Stale,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AudioReviewKind {
    Qc,
    Rights,
    Editorial,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioReviewDecision {
    Approved,
    Rejected,
    Withdrawn,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateAudioReviewRequest {
    pub operation_id: String,
    pub rendition_digest: String,
    pub sha256: String,
    pub kind: AudioReviewKind,
    pub decision: AudioReviewDecision,
    pub notes: String,
    pub evidence_refs: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioReviewDocument {
    pub id: String,
    pub rendition_id: String,
    pub operation_id: String,
    pub sequence: u64,
    pub reviewer_id: String,
    pub reviewed_at: i64,
    pub request: CreateAudioReviewRequest,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioSubmissionResponse {
    pub submission: AudioSubmissionDocument,
    pub state: AudioSubmissionState,
    pub rendition: Option<AudioRenditionDocument>,
    pub reviews: Vec<AudioReviewDocument>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateReleaseCandidateRequest {
    pub operation_id: String,
    pub rendition_digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCandidateDocument {
    pub id: String,
    pub rendition_id: String,
    pub rendition_digest: String,
    pub binding: AudioBinding,
    pub sha256: String,
    pub work_metadata_revision: u64,
    pub unit_metadata_revision: u64,
    pub review_ids: Vec<String>,
    pub created_by: String,
    pub created_at: i64,
    pub operation_id: String,
    pub digest: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AudioReleaseBlocker {
    StaleBrief,
    UploadPermissionRevoked,
    UploadPermissionExpired,
    RightsUnrecorded,
    QcNotApproved,
    RightsNotApproved,
    EditorialNotApproved,
    CandidateReviewsChanged,
    MetadataChanged,
    PublicDeliveryNotImplemented,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCandidateResponse {
    pub candidate: ReleaseCandidateDocument,
    pub blockers: Vec<AudioReleaseBlocker>,
}
