//! Authenticated Theatre viewer claim v1.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimViewerSharesRequest {
    pub operation_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClaimedViewerShare {
    pub work_id: String,
    pub grant_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClaimViewerSharesReceipt {
    pub operation_id: String,
    pub grants: Vec<ClaimedViewerShare>,
    pub replayed: bool,
}
