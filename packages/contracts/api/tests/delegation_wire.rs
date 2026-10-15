use cantos_contracts::*;
use serde_json::{json, Value};

const REQUEST: &str = include_str!("../../fixtures/delegation/v1/issue-request.json");
const CREATED: &str = include_str!("../../fixtures/delegation/v1/issue-created.json");
const REPLAY: &str = include_str!("../../fixtures/delegation/v1/issue-replay.json");
const PAGE: &str = include_str!("../../fixtures/delegation/v1/grant-page.json");
const SCHEMA: &str = include_str!("../../schema/delegation/v1.schema.json");

fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(fixture: &str) -> T {
    let value: T = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        serde_json::to_value(&value).unwrap(),
        serde_json::from_str::<Value>(fixture).unwrap()
    );
    value
}

#[test]
fn fixtures_round_trip_and_only_the_creating_response_carries_a_token() {
    let request: IssueDelegationRequest = round_trip(REQUEST);
    assert_eq!(
        request.actions,
        [
            DelegationAction::WorkRead,
            DelegationAction::ContentRead,
            DelegationAction::ContentWrite
        ]
    );
    let created: IssueDelegationResponse = round_trip(CREATED);
    let replay: IssueDelegationResponse = round_trip(REPLAY);
    assert!(created.token.is_some());
    assert_eq!(replay.token, None);
    assert_eq!(created.grant, replay.grant);
    let page: DelegationGrantPage = round_trip(PAGE);
    assert_eq!(page.items[0].revoked_at, None);
    assert!(page.items[1].revoked_at.is_some());
}

#[test]
fn closed_fields_and_action_vocabulary_are_transport_barriers() {
    for label in [
        "publication:stage",
        "script:read",
        "work:*",
        "WORK:READ",
        "",
    ] {
        assert!(
            serde_json::from_value::<DelegationAction>(json!(label)).is_err(),
            "{label}"
        );
    }
    let request: Value = serde_json::from_str(REQUEST).unwrap();
    for field in ["grantor_id", "token", "token_hash", "owner_id"] {
        let mut value = request.clone();
        value[field] = json!("alice");
        assert!(
            serde_json::from_value::<IssueDelegationRequest>(value).is_err(),
            "{field}"
        );
    }
    let created: Value = serde_json::from_str(CREATED).unwrap();
    let mut value = created.clone();
    value["grant"]["token_hash"] = json!("00");
    assert!(serde_json::from_value::<IssueDelegationResponse>(value).is_err());
    // The schema requires the key; a lenient reader still treats absence as "no secret".
    let mut value = created;
    value.as_object_mut().unwrap().remove("token");
    assert_eq!(
        serde_json::from_value::<IssueDelegationResponse>(value)
            .unwrap()
            .token,
        None
    );
}

#[test]
fn schema_declares_every_wire_type_and_the_same_closed_actions() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    let defs = schema["$defs"].as_object().unwrap();
    for name in [
        "DelegationAction",
        "IssueDelegationRequest",
        "DelegationGrantResponse",
        "IssueDelegationResponse",
        "DelegationGrantPage",
    ] {
        assert!(defs.contains_key(name), "{name}");
    }
    let labels: Vec<_> = defs["DelegationAction"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|label| serde_json::from_value::<DelegationAction>(label.clone()).unwrap())
        .collect();
    assert_eq!(
        labels,
        [
            DelegationAction::WorkRead,
            DelegationAction::ContentRead,
            DelegationAction::ContentWrite
        ]
    );
}
