//! Theatre Identity v1. Display/session fencing facts, never authentication evidence.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub actor_id: String,
    pub session_id: String,
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
}
