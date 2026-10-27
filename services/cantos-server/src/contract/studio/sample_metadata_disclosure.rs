//! Raw owner request for one explicitly scoped sample metadata decision.
use serde::{Deserialize, Serialize};

/// Owner's explicit, source-bound metadata display claim. This is not an independent legal
/// determination or a clearance for audio, covers or anything outside the approved sample.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SampleMetadataRightsRequest {
    pub operation_id: String,
    pub source_id: String,
    pub evidence_reference: String,
    pub rights_statement: String,
    pub confirm_metadata_rights: bool,
    pub confirm_source: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SampleMetadataRightsResponse {
    pub rights_id: String,
    pub source_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum SampleMetadataDisclosureRequest {
    Disclose {
        operation_id: String,
        expected_work_revision: u64,
        /// Required for a unit, forbidden for a Work.
        expected_unit_revision: Option<u64>,
        /// The owner's imported source record; its actual bytes must match the approved sample.
        source_id: String,
        /// Reference to existing owner-bound evidence, accompanied by the owner's confirmation.
        rights_id: String,
        /// Owner explicitly confirms metadata display rights for this exact pinned source.
        confirm_metadata_rights: bool,
        /// Owner confirms the source and identity binding being disclosed.
        confirm_source: bool,
    },
    Withdraw {
        operation_id: String,
    },
}

impl SampleMetadataDisclosureRequest {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Disclose { operation_id, .. } | Self::Withdraw { operation_id } => operation_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SampleMetadataDisclosureResponse {
    pub work_id: String,
    pub subject_kind: String,
    pub subject_id: String,
    pub sequence: u64,
    pub decision: String,
    pub recorded_at_ms: i64,
}
