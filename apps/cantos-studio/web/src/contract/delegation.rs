//! Raw Work-scoped delegation v1 transport (ADR 0028). These DTOs are not permission
//! witnesses: the API admits them through domain constructors and decides every request from
//! current grant, actor and Work facts.
use serde::{Deserialize, Serialize};

/// Closed delegated operations. Review, publication, rights, membership, archive and grant
/// issuance are deliberately not representable.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum DelegationAction {
    #[serde(rename = "work:read")]
    WorkRead,
    #[serde(rename = "content:read")]
    ContentRead,
    #[serde(rename = "content:write")]
    ContentWrite,
}

/// The grantor is the authenticated Studio owner, never a request field.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IssueDelegationRequest {
    pub operation_id: String,
    pub id: String,
    pub delegate_actor_id: String,
    pub actions: Vec<DelegationAction>,
    /// Unix milliseconds; at most 30 days after the server's issuance time.
    pub expires_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DelegationGrantResponse {
    pub id: String,
    pub work_id: String,
    pub grantor_id: String,
    pub delegate_actor_id: String,
    pub actions: Vec<DelegationAction>,
    pub issued_at: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
}

/// `token` is present exactly once, on the response that created the grant. A retry of the
/// same operation returns `null`: a lost secret is replaced by revoking and issuing anew.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IssueDelegationResponse {
    pub grant: DelegationGrantResponse,
    pub token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DelegationGrantPage {
    pub items: Vec<DelegationGrantResponse>,
    pub next_after: Option<String>,
}
