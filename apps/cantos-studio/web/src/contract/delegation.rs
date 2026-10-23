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
#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
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

/// `token` is the delegate's bearer secret: a `{:?}` of the raw response (a parse error path, a
/// failed assertion) must not print it before it is converted into the redacting `DelegationToken`.
impl std::fmt::Debug for IssueDelegationResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssueDelegationResponse")
            .field("grant", &self.grant)
            .field("token", &self.token.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_the_delegation_token() {
        let response: IssueDelegationResponse = serde_json::from_value(serde_json::json!({
            "grant": {"id": "g", "work_id": "w", "grantor_id": "a", "delegate_actor_id": "d",
                "actions": [], "issued_at": 1, "expires_at": 2, "revoked_at": null},
            "token": "cantos_dlg_SECRET"
        }))
        .unwrap();
        let shown = format!("{response:?}");
        assert!(!shown.contains("cantos_dlg_SECRET"), "{shown}");
        assert!(shown.contains("[redacted]"));
    }
}
