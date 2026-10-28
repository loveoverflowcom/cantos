//! Reviewable #574 application input. No HTTP route is mounted pending policy acceptance.
use super::RecordCoverDisclosureRequest;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecordMetadataCoverDisclosureRequest {
    pub expected_work_revision: u64,
    /// Private reference to the explicit metadata-image public-display authorization.
    pub evidence_reference: String,
    pub disclosure: RecordCoverDisclosureRequest,
}
