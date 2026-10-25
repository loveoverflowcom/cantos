//! Studio cover disclosure v1 (ADR 0033 rule 2): fixtures match the strict schema and the
//! Server's Rust DTOs; the decision shape and expiry presence are closed in both.
use cantos_server::contract::studio::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

fn schema_accepts(name: &str, v: &Value) -> bool {
    let root: Value = serde_json::from_str(include_str!(
        "../../contracts/studio/cover-disclosure/v1/cover-disclosure.schema.json"
    ))
    .unwrap();
    let schema =
        json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{name}")});
    jsonschema::validator_for(&schema).unwrap().is_valid(v)
}

fn fixture<T: DeserializeOwned + Serialize>(name: &str, text: &str) -> Value {
    let v: Value = serde_json::from_str(text).unwrap();
    assert!(schema_accepts(name, &v), "{name}");
    let raw: T = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(raw).unwrap(), v);
    v
}

const CLEAR: &str =
    include_str!("../../contracts/studio/cover-disclosure/v1/fixtures/clear-request.json");
const REVOKE: &str =
    include_str!("../../contracts/studio/cover-disclosure/v1/fixtures/revoke-request.json");
const CLEARED: &str =
    include_str!("../../contracts/studio/cover-disclosure/v1/fixtures/cleared.json");
const REVOKED: &str =
    include_str!("../../contracts/studio/cover-disclosure/v1/fixtures/revoked.json");

#[test]
fn disclosure_fixtures_match_strict_schema_and_rust_types() {
    fixture::<RecordCoverDisclosureRequest>("RecordCoverDisclosureRequest", CLEAR);
    fixture::<RecordCoverDisclosureRequest>("RecordCoverDisclosureRequest", REVOKE);
    fixture::<CoverDisclosureResponse>("CoverDisclosureResponse", CLEARED);
    fixture::<CoverDisclosureResponse>("CoverDisclosureResponse", REVOKED);
}

#[test]
fn expiry_presence_is_explicit_and_bound_to_the_decision() {
    let clear: Value = serde_json::from_str(CLEAR).unwrap();
    // Omission never means "perpetual" or "revoked": both readers refuse it.
    let mut missing = clear.clone();
    missing.as_object_mut().unwrap().remove("valid_until_ms");
    assert!(!schema_accepts("RecordCoverDisclosureRequest", &missing));
    assert!(serde_json::from_value::<RecordCoverDisclosureRequest>(missing).is_err());
    // A clearance without a window and a revocation with one are schema-invalid; the Rust DTO
    // is a raw shape and the domain refuses them (see `cover_disclosure_tests`).
    let mut perpetual = clear.clone();
    perpetual["valid_until_ms"] = Value::Null;
    assert!(!schema_accepts("RecordCoverDisclosureRequest", &perpetual));
    let mut windowed = serde_json::from_str::<Value>(REVOKE).unwrap();
    windowed["valid_until_ms"] = json!(1_900_003_600_000_u64);
    assert!(!schema_accepts("RecordCoverDisclosureRequest", &windowed));
    for (field, value) in [
        ("decision", json!("withdrawn")),
        ("image_asset_digest", json!("a".repeat(64))),
        ("operation_id", json!("not-a-uuid")),
        ("valid_until_ms", json!(0)),
        ("valid_until_ms", json!(9_007_199_254_740_992_u64)),
        (
            "image_asset_id",
            json!("10000000-0000-4000-8000-000000000003"),
        ),
    ] {
        let mut v = clear.clone();
        v[field] = value;
        assert!(
            !schema_accepts("RecordCoverDisclosureRequest", &v),
            "{field}"
        );
    }
    let mut extra: Value = serde_json::from_str(CLEARED).unwrap();
    extra["owner_id"] = json!("private-sentinel");
    assert!(!schema_accepts("CoverDisclosureResponse", &extra));
    assert!(serde_json::from_value::<CoverDisclosureResponse>(extra).is_err());
}
