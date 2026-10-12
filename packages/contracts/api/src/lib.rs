//! Wire contracts shared by the Axum host and WASM/native clients. The crate root holds the
//! Studio v1 DTOs and the types every surface shares; `theatre` and `admin` are reserved for
//! their own DTOs and `routes` names the URL spaces.
//! These are raw DTOs, not permission or validation witnesses.
use serde::{Deserialize, Serialize};

mod imports;
pub use imports::*;
mod adaptations;
pub use adaptations::*;
mod production;
pub use production::*;
mod catalog;
pub use catalog::*;
mod content;
pub use content::*;
mod briefs;
pub use briefs::*;
mod audio_intake;
pub use audio_intake::*;
pub mod admin;
pub mod routes;
pub mod theatre;
pub mod wire;

/// Structural/semantic preview only. Acceptance independently checks access and evidence.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ValidateScriptRequest {
    pub script_json: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ScriptValidationResponse {
    pub issues: Vec<FieldIssue>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SaveRevisionRequest {
    pub expected_revision: u64,
    pub operation_id: String,
    /// Complete Script IR document; string transport preserves parser diagnostics.
    pub script_json: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RevisionResponse {
    pub script_id: String,
    pub revision: u64,
    pub accepted_by: String,
    pub accepted_at: String,
    pub content_digest: String,
    pub export_digest: String,
    pub script_json: String,
}

/// Bounded history omits manuscript bodies; use the existing pinned revision route to open one.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RevisionSummary {
    pub script_id: String,
    pub revision: u64,
    pub accepted_by: String,
    pub accepted_at: String,
    pub content_digest: String,
    pub export_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewRequest {
    pub operation_id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewResponse {
    pub id: String,
    pub script_id: String,
    pub revision: u64,
    pub operation_id: String,
    pub reviewed_by: String,
    pub reviewed_at: String,
    pub content_digest: String,
    pub export_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryResponse {
    pub revisions: Vec<RevisionSummary>,
    pub reviews: Vec<ReviewResponse>,
    pub next_after: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceResponse {
    pub id: String,
    pub reference: String,
    pub original_text: String,
    pub sha256: String,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthenticated,
    NotFound,
    Forbidden,
    InvalidRequest,
    InvalidScript,
    EvidenceUnavailable,
    StaleRevision,
    OperationReused,
    ProposalAlreadySubmitted,
    Unavailable,
    CorruptRevision,
    StaleProductionInputs,
    ProductionBlocked,
    InvalidCatalog,
    CatalogConflict,
    StaleCursor,
    AudioRejected,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FieldIssue {
    pub path: String,
    pub rule: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: ErrorCode,
    pub current_revision: Option<u64>,
    pub issues: Vec<FieldIssue>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRequest {
    pub token: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionResponse {
    pub actor_id: String,
}

mod audio_lifecycle;
pub use audio_lifecycle::*;
