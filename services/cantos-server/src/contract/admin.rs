//! Independent Admin v1 responses. No role/session deserialization constructs authority.
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct CapabilityResponse {
    pub actor_id: String,
    pub role: String,
    pub source: String,
    pub valid_until: u64,
    pub audit_id: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokeAccessRequest {
    pub operation_id: String,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevokeAccessResponse {
    pub operation_id: String,
    pub target_actor_id: String,
    pub revoked: bool,
    pub audit_id: String,
}
