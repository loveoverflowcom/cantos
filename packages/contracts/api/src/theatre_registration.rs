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
                _ => panic!("unknown request fixture"),
            };
            assert_eq!(admitted, valid);
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
