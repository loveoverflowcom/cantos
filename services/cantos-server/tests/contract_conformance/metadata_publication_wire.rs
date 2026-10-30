//! Studio metadata publication v1 (ADR 0054, #648, #649): fixtures match the strict schema and
//! the Server's Rust DTOs; the action shapes, the never-published state, explicit nulls and the
//! exact audio association are closed in both readers.
use cantos_server::contract::studio::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

fn schema_accepts(name: &str, v: &Value) -> bool {
    let root: Value = serde_json::from_str(include_str!(
        "../../contracts/studio/metadata-publication/v1/metadata-publication.schema.json"
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

macro_rules! fixture_text {
    ($name:literal) => {
        include_str!(concat!(
            "../../contracts/studio/metadata-publication/v1/fixtures/",
            $name
        ))
    };
}

const REQUESTS: [&str; 5] = [
    fixture_text!("publish-request.json"),
    fixture_text!("publish-unit-request.json"),
    fixture_text!("withdraw-unit-request.json"),
    fixture_text!("withdraw-request.json"),
    fixture_text!("associate-audio-request.json"),
];
const STATES: [&str; 3] = [
    fixture_text!("never-published.json"),
    fixture_text!("active.json"),
    fixture_text!("withdrawn.json"),
];

#[test]
fn publication_fixtures_match_strict_schema_and_rust_types() {
    for text in REQUESTS {
        fixture::<MetadataPublicationRequest>("MetadataPublicationRequest", text);
    }
    for text in STATES {
        fixture::<MetadataPublicationState>("MetadataPublicationState", text);
    }
}

#[test]
fn unknown_fields_missing_nulls_and_unconfirmed_rights_are_refused() {
    let publish: Value = serde_json::from_str(REQUESTS[0]).unwrap();
    // A Discovery presentation level or any other extra field is not part of this contract.
    let mut extra = publish.clone();
    extra["presentation"] = json!(3);
    assert!(!schema_accepts("MetadataPublicationRequest", &extra));
    assert!(serde_json::from_value::<MetadataPublicationRequest>(extra).is_err());
    let mut unconfirmed = publish.clone();
    unconfirmed["confirm_metadata_rights"] = json!(false);
    // The DTO is raw; the service refuses the unconfirmed command before any read.
    assert!(!schema_accepts("MetadataPublicationRequest", &unconfirmed));
    for (field, value) in [
        ("action", json!("archive")),
        ("operation_id", json!("not-a-uuid")),
        ("work_revision", json!(0)),
        ("expected_version", json!(-1)),
    ] {
        let mut v = publish.clone();
        v[field] = value;
        assert!(!schema_accepts("MetadataPublicationRequest", &v), "{field}");
    }
    let mut position = serde_json::from_str::<Value>(REQUESTS[1]).unwrap();
    position["position"] = json!(0);
    assert!(!schema_accepts("MetadataPublicationRequest", &position));

    let never: Value = serde_json::from_str(STATES[0]).unwrap();
    // A never-published state is exactly version 0 with explicit nulls; omission is invalid.
    for field in ["generation", "first_visible_at_ms"] {
        let mut missing = never.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            !schema_accepts("MetadataPublicationState", &missing),
            "{field}"
        );
        assert!(serde_json::from_value::<MetadataPublicationState>(missing).is_err());
    }
    let mut versioned = never.clone();
    versioned["version"] = json!(1);
    assert!(!schema_accepts("MetadataPublicationState", &versioned));
    let mut active: Value = serde_json::from_str(STATES[1]).unwrap();
    active["generation"] = Value::Null;
    assert!(!schema_accepts("MetadataPublicationState", &active));
    let mut leaked: Value = serde_json::from_str(STATES[1]).unwrap();
    leaked["generation"]["owner_id"] = json!("private-sentinel");
    assert!(!schema_accepts("MetadataPublicationState", &leaked));
    assert!(serde_json::from_value::<MetadataPublicationState>(leaked).is_err());
}

#[test]
fn audio_association_is_an_exact_release_pin_with_an_explicit_null() {
    let active: Value = serde_json::from_str(STATES[1]).unwrap();
    // Metadata-only is an explicit null; omission is invalid in both readers.
    let mut missing = active.clone();
    missing["generation"]["entries"][2]
        .as_object_mut()
        .unwrap()
        .remove("audio");
    assert!(!schema_accepts("MetadataPublicationState", &missing));
    assert!(serde_json::from_value::<MetadataPublicationState>(missing).is_err());
    for (field, value) in [
        ("basis", json!("latest")),
        (
            "release_digest",
            json!("rm-r1:sha256:".to_owned() + &"e".repeat(64)),
        ),
        ("release_id", json!("not-a-uuid")),
        ("playable", json!(true)),
    ] {
        let mut v = active.clone();
        v["generation"]["entries"][0]["audio"][field] = value;
        assert!(!schema_accepts("MetadataPublicationState", &v), "{field}");
    }
    let mut extra = active.clone();
    extra["generation"]["entries"][0]["audio"]["playable"] = json!(true);
    assert!(serde_json::from_value::<MetadataPublicationState>(extra).is_err());
    // The association request names one exact release; no "latest" selector exists.
    let request: Value = serde_json::from_str(REQUESTS[4]).unwrap();
    for (field, value) in [
        ("release_id", json!("latest")),
        ("confirm_display_only_drift", json!("yes")),
    ] {
        let mut v = request.clone();
        v[field] = value;
        assert!(!schema_accepts("MetadataPublicationRequest", &v), "{field}");
    }
    let mut missing = request.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("confirm_display_only_drift");
    assert!(!schema_accepts("MetadataPublicationRequest", &missing));
    assert!(serde_json::from_value::<MetadataPublicationRequest>(missing).is_err());
}
