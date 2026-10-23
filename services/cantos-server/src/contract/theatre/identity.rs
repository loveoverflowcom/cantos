//! Theatre Identity v1. Display/session fencing facts, never authentication evidence.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Current Theatre Web mount capabilities. This is public display information, not authority.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    pub login: LoginMethod,
    pub registration: Registration,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoginMethod {
    Unconfigured,
    Oidc,
}

/// Mounted signup workflow availability, never issuer completion or session authority.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Registration {
    EmailVerification {},
    FirebaseEmailVerification {},
    Unavailable {
        reason: RegistrationUnavailableReason,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationUnavailableReason {
    IdentityUnconfigured,
    ProviderUnsupported,
    RegistrationUnconfigured,
}

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
    fn configuration_fixtures_distinguish_unconfigured_and_unsupported_registration() {
        let schema = serde_json::from_str(include_str!(
            "../../../contracts/theatre/identity/v1/auth-configuration.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        for (fixture, login, reason) in [
            (
                include_str!("../../../contracts/theatre/identity/v1/fixtures/configuration-unconfigured.json"),
                LoginMethod::Unconfigured,
                RegistrationUnavailableReason::IdentityUnconfigured,
            ),
            (
                include_str!("../../../contracts/theatre/identity/v1/fixtures/configuration-oidc.json"),
                LoginMethod::Oidc,
                RegistrationUnavailableReason::ProviderUnsupported,
            ),
        ] {
            let value: serde_json::Value = serde_json::from_str(fixture).unwrap();
            assert!(validator.is_valid(&value));
            assert_eq!(
                serde_json::from_value::<Configuration>(value).unwrap(),
                Configuration {
                    login,
                    registration: Registration::Unavailable { reason },
                }
            );
        }
    }

    #[test]
    fn email_registration_capability_requires_explicit_wire_status() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/theatre/identity/v1/auth-configuration.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        for (fixture,registration) in [
            (include_str!("../../../contracts/theatre/identity/v1/fixtures/configuration-email-verification.json"),Registration::EmailVerification {}),
            (include_str!("../../../contracts/theatre/identity/v1/fixtures/configuration-firebase-email-verification.json"),Registration::FirebaseEmailVerification {}),
            (include_str!("../../../contracts/theatre/identity/v1/fixtures/configuration-registration-unconfigured.json"),Registration::Unavailable { reason: RegistrationUnavailableReason::RegistrationUnconfigured }),
        ] {
            let value: serde_json::Value=serde_json::from_str(fixture).unwrap();
            assert!(validator.is_valid(&value));
            assert_eq!(serde_json::from_value::<Configuration>(value).unwrap(),Configuration { login:LoginMethod::Oidc,registration });
        }
        let value = serde_json::json!({"login":"oidc","registration":{"status":"email_verification","token":"not-public"}});
        assert!(!validator.is_valid(&value));
        assert!(serde_json::from_value::<Configuration>(value).is_err());
    }

    #[test]
    fn configuration_never_accepts_inferred_registration_or_credential_fields() {
        let schema = serde_json::from_str(include_str!(
            "../../../contracts/theatre/identity/v1/auth-configuration.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        for value in [
            serde_json::json!({"login":"oidc","registration":{"status":"supported","url":"https://idm.example/signup"}}),
            serde_json::json!({"login":"oidc","registration":{"status":"unavailable","reason":"provider_unsupported","url":"https://idm.example/signup"}}),
            serde_json::json!({"login":"oidc","registration":{"status":"unavailable","reason":"unknown"}}),
            serde_json::json!({"login":"oidc","registration":{"status":"unavailable","reason":"provider_unsupported"},"client_secret":"not-public"}),
        ] {
            assert!(!validator.is_valid(&value));
            assert!(serde_json::from_value::<Configuration>(value).is_err());
        }
    }

    #[test]
    fn session_fixture_has_only_noncredential_fencing_facts() {
        let fixture = include_str!("../../../contracts/theatre/identity/v1/fixtures/session.json");
        let session: Session = serde_json::from_str(fixture).unwrap();
        assert_eq!(session.actor_id, "00000000-0000-4000-8000-000000000001");
        assert_eq!(session.session_id, "00000000-0000-4000-8000-000000000002");
        let mut value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        value["access_token"] = serde_json::json!("must-never-be-returned");
        assert!(serde_json::from_value::<Session>(value).is_err());
    }
    #[test]
    fn native_session_contract_has_no_web_cookie_or_provider_token() {
        let fixture =
            include_str!("../../../contracts/theatre/identity/v1/fixtures/native-session.json");
        let view: NativeSession = serde_json::from_str(fixture).unwrap();
        assert_eq!(view.expires_at, 1900000900);
        let mut raw: serde_json::Value = serde_json::from_str(fixture).unwrap();
        raw["access_token"] = serde_json::json!("not a session display field");
        assert!(serde_json::from_value::<NativeSession>(raw).is_err());
    }
}
