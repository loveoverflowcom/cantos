//! Private, revision-pinned adaptation transport. Rights authorization is a recorded claim.
use serde::{Deserialize, Serialize};

use crate::{FieldIssue, RevisionResponse};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StartAdaptationRequest {
    pub operation_id: String,
    pub source_id: String,
    pub script_id: String,
    pub expected_revision: u64,
    /// The displayed, authorized destination and generation configuration must still match.
    pub expected_provider: AdaptationProviderMetadata,
    /// The creator explicitly authorizes processing this source by the displayed integration.
    /// This assertion never establishes production or publication eligibility.
    pub rights_authorization: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AcceptAdaptationRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub script_json: String,
    pub reviewed_findings: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CancelAdaptationRequest {
    pub operation_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptationStatus {
    AwaitingProposal,
    Queued,
    Running,
    Succeeded,
    InvalidOutput,
    Failed,
    Ambiguous,
    Cancelled,
    Accepted,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationConfig {
    pub temperature_milli: u16,
    pub seed: u32,
    pub num_context: u32,
    pub num_predict: u32,
    pub timeout_seconds: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationProviderMetadata {
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    /// Legacy recorded model evidence; retained verbatim for frozen v1/a1 record compatibility.
    /// This optional value does not attest the current integration or a cloud destination.
    pub local_model_digest: Option<String>,
    pub prompt_version: String,
    pub contract_version: String,
    pub config: AdaptationConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationProviderResponse {
    pub provider: Option<AdaptationProviderMetadata>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationUsage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationCost {
    /// An explicitly reported amount with its declared basis; absent records mean unknown.
    /// CallerDeclared values are unverified assertions in the host-owned workflow.
    pub currency: AdaptationCurrency,
    pub amount_minor: u64,
    pub basis: AdaptationCostBasis,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum AdaptationCurrency {
    /// ISO 4217 USD: two minor-unit decimal places.
    USD,
    /// ISO 4217 VND: zero minor-unit decimal places.
    VND,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptationCostBasis {
    CallerDeclared,
    ProviderReported,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationProblem {
    pub code: String,
    pub issues: Vec<FieldIssue>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptationFindingCode {
    SourceWarning,
    UncoveredSource,
    OmittedSource,
    TextChanged,
    UnresolvedSpeaker,
    NewSpeaker,
    PossibleNarrationConfusion,
    CueLikeMarkup,
    UnsupportedPerformanceControl,
    ProviderReviewNote,
    SourceCoverageRequiresReview,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationFinding {
    pub code: AdaptationFindingCode,
    pub block: Option<u32>,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptationCoverageDisposition {
    Represented,
    Omitted,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationCoverage {
    pub block: u32,
    pub dialogue_ids: Vec<String>,
    pub disposition: AdaptationCoverageDisposition,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationProposal {
    /// Canonical complete Script IR after structural and semantic admission.
    pub script_json: String,
    pub findings: Vec<AdaptationFinding>,
    pub coverage: Vec<AdaptationCoverage>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationRunResponse {
    pub id: String,
    pub created_by: String,
    pub recorded_at: String,
    pub updated_at: String,
    pub request: StartAdaptationRequest,
    pub source_sha256: String,
    pub extractor_version: String,
    pub input_content_digest: Option<String>,
    pub input_export_digest: Option<String>,
    pub generation_record_id: String,
    pub rights_record_id: String,
    pub provider: AdaptationProviderMetadata,
    pub status: AdaptationStatus,
    pub proposal: Option<AdaptationProposal>,
    pub problem: Option<AdaptationProblem>,
    pub usage: Option<AdaptationUsage>,
    pub cost: Option<AdaptationCost>,
    pub accepted_revision: Option<RevisionResponse>,
}

/// Caller-owned adaptation. Cantos exports data but does not invoke the caller's model.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationContextRequest {
    pub operation_id: String,
    pub source_id: String,
    pub source_sha256: String,
    pub extractor_version: String,
    pub script_id: String,
    pub expected_revision: u64,
    /// Recorded permission to export this frozen source to the requesting adaptation host.
    /// This does not establish legal eligibility or production/publication approval.
    pub rights_authorization: bool,
}

/// Every field is a caller declaration, not an attested provider or billing observation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CallerGenerationMetadata {
    pub host_tool: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub configuration_json: Option<String>,
    pub prompt_version: String,
    pub usage: Option<AdaptationUsage>,
    pub cost: Option<AdaptationCost>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SubmitAdaptationProposalRequest {
    pub operation_id: String,
    pub context_digest: String,
    pub proposal_json: String,
    pub generation: CallerGenerationMetadata,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdaptationSubmissionStatus {
    Valid,
    Invalid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationSubmissionReceipt {
    pub id: String,
    pub run_id: String,
    pub operation_id: String,
    pub context_digest: String,
    pub submitted_by: String,
    pub submitted_at: String,
    pub output_sha256: String,
    pub generation: CallerGenerationMetadata,
    pub status: AdaptationSubmissionStatus,
    pub problem: Option<AdaptationProblem>,
    pub proposal: Option<AdaptationProposal>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdaptationContextResponse {
    pub id: String,
    pub created_by: String,
    pub recorded_at: String,
    pub updated_at: String,
    pub request: AdaptationContextRequest,
    pub context_version: String,
    pub context_digest: String,
    pub prompt_version: String,
    pub contract_version: String,
    pub source: crate::ImportResponse,
    pub input_revision: Option<RevisionResponse>,
    pub generation_record_id: String,
    pub rights_record_id: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub proposal_schema_json: String,
    pub status: AdaptationStatus,
    pub proposal: Option<AdaptationProposal>,
    pub latest_submission: Option<AdaptationSubmissionReceipt>,
    pub accepted_revision: Option<RevisionResponse>,
}

/// The review surface distinguishes preserved legacy evidence from caller-owned contexts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "workflow", rename_all = "snake_case", deny_unknown_fields)]
pub enum AdaptationReviewResponse {
    Caller {
        context: Box<AdaptationContextResponse>,
    },
    Legacy {
        run: Box<AdaptationRunResponse>,
        source: Box<crate::ImportResponse>,
        input_revision: Option<Box<RevisionResponse>>,
    },
}
