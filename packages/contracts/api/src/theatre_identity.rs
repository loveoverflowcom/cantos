//! Theatre Identity v1. Display/session fencing facts, never authentication evidence.
use serde::{Deserialize, Serialize};
use std::fmt;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub actor_id: String,
    pub session_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFlowRequest {
    pub state: String,
    pub nonce: String,
    pub code_challenge: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeFlow {
    pub flow_id: String,
    pub authorization_url: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeExchangeRequest {
    pub state: String,
    pub code: String,
    pub code_verifier: String,
}
/// Private native credential. Only OS secure storage may persist it; never a Web BFF cookie.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeExchange {
    pub actor_id: String,
    pub session_id: String,
    pub access_token: String,
    pub expires_at: u64,
}
impl fmt::Debug for NativeExchange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeExchange([redacted])")
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSession {
    pub actor_id: String,
    pub session_id: String,
    pub expires_at: u64,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_fixture_has_only_noncredential_fencing_facts() {
        let fixture = include_str!("../../fixtures/theatre-identity/v1/session.json");
        let session: Session = serde_json::from_str(fixture).unwrap();
        assert_eq!(session.actor_id, "00000000-0000-4000-8000-000000000001");
        assert_eq!(session.session_id, "00000000-0000-4000-8000-000000000002");
        let mut value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        value["access_token"] = serde_json::json!("must-never-be-returned");
        assert!(serde_json::from_value::<Session>(value).is_err());
    }
    #[test]
    fn native_session_contract_has_no_web_cookie_or_provider_token() {
        let fixture = include_str!("../../fixtures/theatre-identity/v1/native-session.json");
        let view: NativeSession = serde_json::from_str(fixture).unwrap();
        assert_eq!(view.expires_at, 1900000900);
        let mut raw: serde_json::Value = serde_json::from_str(fixture).unwrap();
        raw["access_token"] = serde_json::json!("not a session display field");
        assert!(serde_json::from_value::<NativeSession>(raw).is_err());
    }
}
