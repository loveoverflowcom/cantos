//! Email proof is scoped to onboarding, never authentication or a password credential.
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationRequest {
    pub email: String,
}

/// Neither raw proof nor applicant email implements Debug or is echoed in a response.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TokenRequest {
    pub token: String,
}

/// Firebase action proof is bound to one server operation; no caller identity/issuer is accepted.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FirebaseConfirmRequest {
    pub operation_id: String,
    pub oob_code: String,
}

/// Private one-shot delivery to the explicitly confirming browser. Never a login receipt.
/// No Debug implementation: the setup capability must not enter logs.
#[derive(Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum FirebaseConfirmation {
    SetupReady { login: String, setup_url: String },
    Unavailable {},
}

/// Public metadata for the separately enabled native admission surface; never an issuer probe.
#[derive(Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeRegistrationConfiguration {
    FirebaseEmailVerification { action_origin: String },
    Unavailable {},
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcceptedStatus {
    Accepted,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Accepted {
    pub status: AcceptedStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    SetupLinkSent,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VerificationResponse {
    pub status: VerificationStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_receipts_cannot_claim_account_or_session_completion() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../schemas/theatre-registration-v1.schema.json"
        ))
        .unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        for literal in [
            r#"{"status":"accepted"}"#,
            r#"{"status":"setup_link_sent"}"#,
            r#"{"status":"unavailable"}"#,
        ] {
            let value: serde_json::Value = serde_json::from_str(literal).unwrap();
            assert!(validator.is_valid(&value));
        }
        assert_eq!(
            serde_json::from_str::<Accepted>(r#"{"status":"accepted"}"#)
                .unwrap()
                .status,
            AcceptedStatus::Accepted
        );
        assert_eq!(
            serde_json::from_str::<VerificationResponse>(r#"{"status":"setup_link_sent"}"#)
                .unwrap()
                .status,
            VerificationStatus::SetupLinkSent
        );
        for literal in [
            r#"{"status":"complete"}"#,
            r#"{"status":"accepted","actor_id":"caller"}"#,
            r#"{"status":"setup_link_sent","token":"secret"}"#,
        ] {
            let value: serde_json::Value = serde_json::from_str(literal).unwrap();
            assert!(!validator.is_valid(&value));
        }
        assert!(serde_json::from_str::<VerificationResponse>(r#"{"status":"complete"}"#).is_err());
    }

    #[test]
    fn request_shapes_match_independent_schema_fixtures() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../schemas/theatre-registration-v1.schema.json"
        ))
        .unwrap();
        let fixtures: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../fixtures/theatre-registration/v1/cases.json"
        ))
        .unwrap();
        for case in fixtures {
            let kind = case["kind"].as_str().unwrap();
            let valid = case["valid"].as_bool().unwrap();
            let value = &case["value"];
            assert_eq!(
                jsonschema::validator_for(&schema["$defs"][kind])
                    .unwrap()
                    .is_valid(value),
                valid
            );
            let admitted = match kind {
                "email_request" => {
                    serde_json::from_value::<RegistrationRequest>(value.clone()).is_ok()
                }
                "proof_request" => serde_json::from_value::<TokenRequest>(value.clone()).is_ok(),
                "firebase_confirm_request" => {
                    serde_json::from_value::<FirebaseConfirmRequest>(value.clone()).is_ok()
                }
                "native_configuration" => {
                    serde_json::from_value::<NativeRegistrationConfiguration>(value.clone()).is_ok()
                }
                "accepted" => serde_json::from_value::<Accepted>(value.clone()).is_ok(),
                "firebase_confirmation" => {
                    serde_json::from_value::<FirebaseConfirmation>(value.clone()).is_ok()
                }
                _ => panic!("unknown request fixture"),
            };
            assert_eq!(admitted, valid);
        }
    }

    #[test]
    fn firebase_setup_delivery_has_a_separate_strict_non_session_shape() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../schemas/theatre-registration-v1.schema.json"
        ))
        .unwrap();
        let validator =
            jsonschema::validator_for(&schema["$defs"]["firebase_confirmation"]).unwrap();
        for (literal, valid) in [
            (
                r#"{"status":"setup_ready","login":"listener_fixture","setup_url":"https://idm.example.invalid/ui/reset?token=fixture"}"#,
                true,
            ),
            (r#"{"status":"unavailable"}"#, true),
            (r#"{"status":"setup_ready"}"#, false),
            (
                r#"{"status":"setup_ready","login":"fixture","setup_url":"fixture","session_id":"forged"}"#,
                false,
            ),
            (r#"{"status":"unavailable","setup_url":"fixture"}"#, false),
        ] {
            let value: serde_json::Value = serde_json::from_str(literal).unwrap();
            assert_eq!(validator.is_valid(&value), valid);
            assert_eq!(
                serde_json::from_value::<FirebaseConfirmation>(value).is_ok(),
                valid
            );
        }
    }

    #[test]
    fn client_cannot_select_existing_person_or_collect_passwords() {
        for literal in [
            r#"{"email":"listener@example.invalid","person_id":"studio"}"#,
            r#"{"email":"listener@example.invalid","password":"forbidden"}"#,
            r#"{"email":"listener@example.invalid","issuer":"https://attacker.invalid"}"#,
        ] {
            assert!(serde_json::from_str::<RegistrationRequest>(literal).is_err());
        }
        assert!(serde_json::from_str::<TokenRequest>(
            r#"{"token":"proof","email":"other@example.invalid"}"#
        )
        .is_err());
    }
}
