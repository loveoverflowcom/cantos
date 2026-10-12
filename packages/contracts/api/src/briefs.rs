//! Raw Production Brief v1 transport; domain admission and current access are authoritative.
use crate::{ContentRevisionResponse, ImportResponse, RevisionResponse};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProductionMode {
    Narration,
    Recording,
    AudioDrama,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ProducerCapability {
    Narration,
    Recording,
    ScriptIr,
    Pronunciation,
    Casting,
    SoundDesign,
    OggOpusMono,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EditorialIntent {
    pub mode: ProductionMode,
    pub notes: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PronunciationDirection {
    pub term: String,
    pub reading: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CastDirection {
    pub speaker: String,
    pub direction: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VoiceGuidance {
    pub revision: u64,
    pub notes: String,
    pub pronunciation: Vec<PronunciationDirection>,
    pub casting: Vec<CastDirection>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SoundDesignGuidance {
    pub revision: u64,
    pub notes: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeliveryConstraints {
    pub profile_id: String,
    pub permitted_use: String,
    pub attribution: String,
    pub naming: String,
    pub qa_instructions: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BriefGuidance {
    pub editorial: EditorialIntent,
    pub voice: Option<VoiceGuidance>,
    pub sound_design: Option<SoundDesignGuidance>,
    pub required_capabilities: Vec<ProducerCapability>,
    pub delivery: DeliveryConstraints,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateProductionBriefRequest {
    pub operation_id: String,
    pub id: String,
    pub expected_version: u64,
    pub content_revision_id: String,
    pub content_revision_digest: String,
    pub guidance: BriefGuidance,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProductionInstructions {
    Text {
        text: String,
    },
    Recording {},
    Source {
        receipt: ImportResponse,
        receipt_digest: String,
    },
    Script {
        revision: RevisionResponse,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BriefWork {
    pub id: String,
    pub owner_id: String,
    pub kind: String,
    pub title: String,
    pub rights_refs: Vec<String>,
    pub rights_digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionBriefDocument {
    pub schema_version: String,
    pub id: String,
    pub version: u64,
    pub work: BriefWork,
    pub content: ContentRevisionResponse,
    pub instructions: ProductionInstructions,
    pub guidance: BriefGuidance,
    pub created_by: String,
    pub created_at: i64,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BriefFreshness {
    Current,
    ContentChanged,
    RightsChanged,
    GuidanceChanged,
    Archived,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductionBriefResponse {
    pub brief: ProductionBriefDocument,
    pub freshness: BriefFreshness,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateProducerGrantRequest {
    pub operation_id: String,
    pub id: String,
    pub brief_digest: String,
    /// Caller-generated 32 random bytes as lowercase hex. Only a separated hash is persisted.
    pub token: String,
    pub expires_at: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProducerGrantResponse {
    pub id: String,
    pub work_id: String,
    pub unit_id: String,
    pub brief_id: String,
    pub brief_digest: String,
    pub permission: String,
    pub expires_at: i64,
}
