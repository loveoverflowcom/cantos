//! This package's own representation of the two Identity auth v1 shapes the session owner
//! reads or sends. The Server owns the canonical schema and fixtures
//! (`services/cantos-server/contracts/auth/`); the test below checks this representation
//! against them, so the package needs no shared contract crate.
use serde::{Deserialize, Serialize};
use std::fmt;

/// Nonsecret session facts answered by the cookie-session endpoints. Never a credential.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub actor_id: String,
    pub session_id: String,
    pub expires_at: u64,
    pub absolute_expires_at: u64,
    pub authenticated_at: Option<u64>,
}

#[derive(Serialize)]
pub struct Login {
    pub email: String,
    pub password: String,
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Login([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const SCHEMA: &str = include_str!(
        "../../../services/cantos-server/contracts/auth/identity-auth/v1/identity-auth.schema.json"
    );
    const LEGACY_SESSION: &str = include_str!(
        "../../../services/cantos-server/contracts/auth/bearer/v1/fixtures/session.json"
    );

    fn v1_session() -> Value {
        json!({
            "actor_id": "00000000-0000-4000-8000-000000000001",
            "session_id": "00000000-0000-4000-8000-000000000002",
            "expires_at": 1_900_000_120,
            "absolute_expires_at": 1_900_000_900,
            "authenticated_at": 1_900_000_000
        })
    }

    #[test]
    fn a_schema_valid_v1_session_round_trips_unchanged() {
        let schema = jsonschema::validator_for(&serde_json::from_str(SCHEMA).unwrap()).unwrap();
        assert!(schema.is_valid(&v1_session()));
        let session: Session = serde_json::from_value(v1_session()).unwrap();
        assert_eq!(serde_json::to_value(&session).unwrap(), v1_session());
    }

    #[test]
    fn the_canonical_legacy_session_fixture_still_decodes_without_an_auth_time() {
        let fixture: Value = serde_json::from_str(LEGACY_SESSION).unwrap();
        let session: Session = serde_json::from_value(fixture.clone()).unwrap();
        assert_eq!(session.authenticated_at, None);
        assert_eq!(serde_json::to_value(&session).unwrap(), fixture);
    }

    #[test]
    fn session_refuses_unknown_and_missing_required_fields() {
        let mut extra = v1_session();
        extra["access_token"] = json!("secret");
        assert!(serde_json::from_value::<Session>(extra).is_err());
        let mut missing = v1_session();
        missing.as_object_mut().unwrap().remove("actor_id");
        assert!(serde_json::from_value::<Session>(missing).is_err());
    }

    #[test]
    fn login_sends_exactly_email_and_password_and_never_prints_them() {
        let login = Login {
            email: "an@example.vn".into(),
            password: "mật khẩu".into(),
        };
        assert_eq!(
            serde_json::to_value(&login).unwrap(),
            json!({"email": "an@example.vn", "password": "mật khẩu"})
        );
        assert_eq!(format!("{login:?}"), "Login([redacted])");
    }
}
