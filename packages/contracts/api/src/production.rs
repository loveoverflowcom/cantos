//! Production input planning only. None of these DTOs is permission to send a paid request.
use serde::{Deserialize, Serialize};

use crate::RevisionResponse;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ProductionCurrency {
    USD,
    VND,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionRate {
    /// Caller-recorded planning basis, never a provider receipt or attested price.
    pub reference: String,
    pub version: String,
    /// Billing units in this bounded slice are Unicode scalar values of effective text.
    pub units_per_charge: u64,
    pub amount_minor: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionBudget {
    pub currency: ProductionCurrency,
    pub limit_minor: u64,
    pub rate: Option<ProductionRate>,
    /// Exact caller-recorded production scope; no implicit unlimited policy.
    pub scope: String,
    pub territory: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionPronunciation {
    pub surface: String,
    pub replacement: String,
}

/// Response projection of admitted Script IR overrides and smaller character presets.
/// Inherited overrides retain Script IR's 10,000 normalized Unicode scalar bound.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPronunciation {
    pub surface: String,
    pub replacement: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SynthesisPerformance {
    pub rate_permille: u16,
    pub pitch_semitones: i16,
    /// None preserves the accepted Script IR delivery, never substitutes neutral.
    pub emotion: Option<String>,
    pub intensity_permille: Option<u16>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CharacterCasting {
    pub character_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub voice_id: String,
    pub language: String,
    pub voice_rights_record_id: String,
    pub performance: SynthesisPerformance,
    pub pronunciation: Vec<ProductionPronunciation>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionSettings {
    pub bindings: Vec<CharacterCasting>,
    pub budget: ProductionBudget,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SaveProductionSettingsRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub expected_settings_version: u64,
    pub settings: ProductionSettings,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionSettingsResponse {
    pub script_id: String,
    pub version: u64,
    pub script_revision: u64,
    pub operation_id: String,
    pub recorded_by: String,
    pub recorded_at: String,
    pub settings: ProductionSettings,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductionRightsStatus {
    Pending,
    Granted,
    Revoked,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductionRightsScope {
    ProductionSynthesis,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProductionRightsSubject {
    /// The existing rights-kind evidence referenced by the complete Script IR export.
    Evidence { record_id: String },
    Voice {
        provider_id: String,
        model_id: String,
        voice_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionRightsDeclaration {
    pub record_id: String,
    pub subject: ProductionRightsSubject,
    pub scope: ProductionRightsScope,
    pub rights_holder: String,
    /// Exact supported language tags, or the explicitly recorded literal `all`.
    pub languages: Vec<String>,
    pub territory: String,
    pub attribution: String,
    pub restrictions: String,
    pub permitted_scope: String,
    pub status: ProductionRightsStatus,
    /// A recorded claim reference; Cantos does not legally verify its contents.
    pub reference: String,
    pub valid_from_unix: i64,
    /// None records an explicitly open-ended term; Pending still blocks authorization.
    pub valid_until_unix: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SaveProductionRightsRequest {
    pub operation_id: String,
    pub expected_version: u64,
    pub claim: ProductionRightsDeclaration,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionRightsClaim {
    pub version: u64,
    pub declaration: ProductionRightsDeclaration,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionRightsClaimResponse {
    pub claim: ProductionRightsClaim,
    pub operation_id: String,
    pub recorded_by: String,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCapability {
    pub provider_id: String,
    pub model_id: String,
    pub voice_ids: Vec<String>,
    pub languages: Vec<String>,
    pub emotions: Vec<String>,
    pub rate_min_permille: u16,
    pub rate_max_permille: u16,
    pub pitch_min_semitones: i16,
    pub pitch_max_semitones: i16,
    pub intensity_supported: bool,
    pub pronunciation_supported: bool,
    pub reference_only: bool,
    pub billable_dispatch_available: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionCatalogResponse {
    pub version: String,
    pub capabilities: Vec<ProductionCapability>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResolvedSynthesisInput {
    pub dialogue_id: String,
    pub character_id: String,
    pub text: String,
    pub language: String,
    pub provider_id: String,
    pub model_id: String,
    pub voice_id: String,
    pub voice_rights_record_id: String,
    pub adapter_version: String,
    /// This reference-only slice pins `reference-only/no-audio`, not an audio format.
    pub output_contract: String,
    pub performance: SynthesisPerformance,
    pub pronunciation: Vec<ResolvedPronunciation>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProductionEstimate {
    Unavailable {
        reason: String,
    },
    Known {
        currency: ProductionCurrency,
        amount_minor: u64,
        units: u64,
        rate_reference: String,
        rate_version: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductionFindingCode {
    ScriptUnreviewed,
    RightsMissing,
    RightsPending,
    RightsRevoked,
    RightsNotYetValid,
    RightsExpired,
    RightsSubjectMismatch,
    RightsLanguageMismatch,
    RightsTerritoryMismatch,
    RightsScopeMismatch,
    RightsRestrictionsRequireReview,
    EstimateUnavailable,
    BudgetExceeded,
    RevisionChanged,
    SettingsChanged,
    RightsChanged,
    ApprovalMissing,
    ApprovalScopeMismatch,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionFinding {
    pub code: ProductionFindingCode,
    pub path: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionPreviewResponse {
    pub input_digest: String,
    pub script_revision: u64,
    pub settings_version: u64,
    pub resolved: Vec<ResolvedSynthesisInput>,
    pub estimate: ProductionEstimate,
    pub findings: Vec<ProductionFinding>,
    pub inputs_eligible: bool,
    pub billable_dispatch_available: bool,
}

/// Immutable candidate. A frozen candidate is not an approved or dispatchable request.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrozenProductionDocument {
    pub owner_id: String,
    pub catalog_version: String,
    pub revision: RevisionResponse,
    pub settings: ProductionSettingsResponse,
    pub rights: Vec<ProductionRightsClaimResponse>,
    pub resolved: Vec<ResolvedSynthesisInput>,
    pub estimate: ProductionEstimate,
    pub input_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FreezeProductionRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub settings_version: u64,
    pub input_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApproveProductionRequest {
    pub operation_id: String,
    pub input_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionApprovalResponse {
    pub id: String,
    pub snapshot_id: String,
    pub approved_by: String,
    pub approved_at: String,
    pub operation_id: String,
    pub input_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionEligibility {
    pub inputs_eligible: bool,
    pub approval_current: bool,
    pub billable_dispatch_available: bool,
    pub findings: Vec<ProductionFinding>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionSnapshotResponse {
    pub id: String,
    pub script_id: String,
    pub created_by: String,
    pub recorded_at: String,
    pub operation_id: String,
    pub document: FrozenProductionDocument,
    pub approval: Option<ProductionApprovalResponse>,
    pub eligibility: ProductionEligibility,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionStateResponse {
    pub settings: Option<ProductionSettingsResponse>,
    pub rights: Vec<ProductionRightsClaimResponse>,
    /// Bounded newest-first history. Fetch a known snapshot separately for full inspection.
    pub snapshots: Vec<ProductionSnapshotSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionSnapshotSummary {
    pub id: String,
    pub script_id: String,
    pub created_by: String,
    pub recorded_at: String,
    pub input_digest: String,
    pub script_revision: u64,
    pub settings_version: u64,
}
