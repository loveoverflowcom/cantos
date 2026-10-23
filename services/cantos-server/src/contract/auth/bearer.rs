//! Cantos Bearer v1. JSON-only identity wire, independent of business Protobuf negotiation.
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FlowRequest {
    pub state: String,
    pub nonce: String,
    pub code_challenge: String,
    #[serde(default)]
    pub reauthenticate: bool,
    /// Theatre native only: selects one operator-admitted callback. Absent means production.
    /// It names a closed profile, never a URI; Studio and Web clients must omit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callback_profile: Option<CallbackProfile>,
}
/// Closed set of native callbacks; the API owns each exact URI and its operator admission.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CallbackProfile {
    Production,
    Development,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Flow {
    pub flow_id: String,
    pub authorization_url: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExchangeRequest {
    pub state: String,
    pub code: String,
    pub code_verifier: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CancelRequest {
    pub state: String,
    pub code_verifier: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub session_id: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TokenPair {
    pub token_type: TokenType,
    pub access_token: String,
    pub refresh_token: String,
    pub actor_id: String,
    pub session_id: String,
    pub expires_at: u64,
    pub refresh_expires_at: u64,
    pub absolute_expires_at: u64,
    /// Signed issuer auth_time only; refresh never changes it. None is not recent auth.
    pub authenticated_at: Option<u64>,
}
impl std::fmt::Debug for TokenPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenPair([redacted])")
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum TokenType {
    Bearer,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub actor_id: String,
    pub session_id: String,
    pub expires_at: u64,
    pub absolute_expires_at: u64,
    pub authenticated_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_json_fixtures_have_only_cantos_credentials_and_redacted_debug() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/auth/bearer/v1/bearer.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/auth/bearer/v1/fixtures/token-pair.json"
        ))
        .unwrap();
        assert!(validator.is_valid(&value));
        let pair: TokenPair = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(pair.token_type, TokenType::Bearer);
        assert_eq!(format!("{pair:?}"), "TokenPair([redacted])");
        for field in ["provider_token", "client_secret", "cookie"] {
            let mut wrong = value.clone();
            wrong[field] = serde_json::json!("forbidden");
            assert!(!validator.is_valid(&wrong));
            assert!(serde_json::from_value::<TokenPair>(wrong).is_err());
        }
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/auth/bearer/v1/fixtures/session.json"
        ))
        .unwrap();
        assert!(validator.is_valid(&value));
        let _: Session = serde_json::from_value(value.clone()).unwrap();
        let mut wrong = value;
        wrong["access_token"] = serde_json::json!("forbidden");
        assert!(serde_json::from_value::<Session>(wrong).is_err());
    }
}
