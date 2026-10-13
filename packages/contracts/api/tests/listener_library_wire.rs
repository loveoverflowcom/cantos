use cantos_contracts::listener_library as dto;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
const SCHEMA: &str = include_str!("../../schema/theatre/library/v1.schema.json");
fn check<T: DeserializeOwned + Serialize>(definition: &str, bytes: &str) {
    let value: Value = serde_json::from_str(bytes).unwrap();
    let mut schema: Value = serde_json::from_str(SCHEMA).unwrap();
    schema["$ref"] = Value::String(format!("#/$defs/{definition}"));
    schema.as_object_mut().unwrap().remove("anyOf");
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&value)
        .unwrap();
    let decoded: T = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
}
macro_rules! fixture {
    ($file:literal) => {
        include_str!(concat!(
            "../../fixtures/theatre/library/v1/",
            $file,
            ".json"
        ))
    };
}
#[test]
fn strict_library_v1_fixtures_decode_and_reencode_without_drift() {
    check::<dto::ProgressWrite>("ProgressWrite", fixture!("progress_write"));
    check::<dto::SavedWrite>("SavedWrite", fixture!("saved_write"));
    check::<dto::HistoryStart>("HistoryStart", fixture!("history_start"));
    check::<dto::HistoryClear>("HistoryClear", fixture!("history_clear"));
    check::<dto::ContinuePage>("ContinuePage", fixture!("continue"));
    check::<dto::SavedPage>("SavedPage", fixture!("saved"));
    check::<dto::HistoryPage>("HistoryPage", fixture!("history"));
    check::<dto::Receipt>("Receipt", fixture!("progress_conflict"));
    check::<dto::MutationState>("MutationState", fixture!("progress_read"));
    check::<dto::Receipt>("Receipt", fixture!("accepted"));
    check::<dto::Receipt>("Receipt", fixture!("epoch_conflict"));
    check::<dto::ContinuePage>("ContinuePage", fixture!("empty"));
    for bytes in [
        fixture!("invalid_request"),
        fixture!("invalid_position"),
        fixture!("invalid_cursor"),
        fixture!("unauthenticated"),
        fixture!("not_found"),
        fixture!("idempotency_key_reused"),
        fixture!("stale_device_sequence"),
        fixture!("revision_exhausted"),
        fixture!("unavailable"),
        fixture!("corrupt_state"),
    ] {
        check::<dto::LibraryError>("LibraryError", bytes);
    }
}
#[test]
fn raw_authority_and_unconfirmed_history_events_never_decode() {
    let mut v: Value = serde_json::from_str(fixture!("progress_write")).unwrap();
    v["actor_id"] = Value::String("alice".into());
    assert!(serde_json::from_value::<dto::ProgressWrite>(v).is_err());
    for event in ["play_requested", "detail_viewed", "play_failed"] {
        let mut v: Value = serde_json::from_str(fixture!("history_start")).unwrap();
        v["event"] = Value::String(event.into());
        assert!(serde_json::from_value::<dto::HistoryStart>(v).is_err());
    }
    let mut v: Value = serde_json::from_str(fixture!("history_clear")).unwrap();
    v.as_object_mut().unwrap().remove("confirmation");
    assert!(serde_json::from_value::<dto::HistoryClear>(v).is_err());
}

#[test]
fn required_nullable_members_must_be_present_for_wire_schema_conformance() {
    let mut value: Value = serde_json::from_str(fixture!("progress_write")).unwrap();
    value["operation"]
        .as_object_mut()
        .unwrap()
        .remove("device_reported_at_ms");
    assert!(serde_json::from_value::<dto::ProgressWrite>(value).is_err());
    let mut value: Value = serde_json::from_str(fixture!("continue")).unwrap();
    value.as_object_mut().unwrap().remove("next_cursor");
    assert!(serde_json::from_value::<dto::ContinuePage>(value).is_err());
    let mut value: Value = serde_json::from_str(fixture!("progress_conflict")).unwrap();
    value["state"].as_object_mut().unwrap().remove("current");
    assert!(serde_json::from_value::<dto::Receipt>(value).is_err());
}
