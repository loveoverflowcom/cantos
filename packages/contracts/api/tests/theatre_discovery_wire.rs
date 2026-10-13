use cantos_contracts::theatre::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
fn fixture<T: DeserializeOwned + Serialize>(name: &str, text: &str) {
    let root: Value = serde_json::from_str(include_str!(
        "../../schema/theatre/discovery/v1.schema.json"
    ))
    .unwrap();
    let schema =
        json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{name}")});
    let v: Value = serde_json::from_str(text).unwrap();
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&v));
    let raw: T = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(serde_json::to_value(raw).unwrap(), v);
}
#[test]
fn public_fixtures_match_strict_schema_and_shared_rust_types() {
    fixture::<WorkPage>(
        "WorkPage",
        include_str!("../../fixtures/theatre/discovery/v1/works.json"),
    );
    fixture::<WorkPage>(
        "WorkPage",
        include_str!("../../fixtures/theatre/discovery/v1/empty.json"),
    );
    fixture::<UnitPage>(
        "UnitPage",
        include_str!("../../fixtures/theatre/discovery/v1/units.json"),
    );
    fixture::<PublicationManifest>(
        "PublicationManifest",
        include_str!("../../fixtures/theatre/discovery/v1/manifest.json"),
    );
    for text in [
        include_str!("../../fixtures/theatre/discovery/v1/not_found.json"),
        include_str!("../../fixtures/theatre/discovery/v1/invalid_request.json"),
        include_str!("../../fixtures/theatre/discovery/v1/invalid_cursor.json"),
        include_str!("../../fixtures/theatre/discovery/v1/unavailable.json"),
        include_str!("../../fixtures/theatre/discovery/v1/rendition_unavailable.json"),
    ] {
        fixture::<ListenerError>("ListenerError", text);
    }
}
#[test]
fn public_manifest_rejects_audit_and_disposable_grant_fields() {
    let mut v: Value = serde_json::from_str(include_str!(
        "../../fixtures/theatre/discovery/v1/manifest.json"
    ))
    .unwrap();
    for name in [
        "owner_id",
        "review_evidence",
        "url",
        "object_key",
        "candidate_id",
    ] {
        v[name] = json!("private");
        assert!(serde_json::from_value::<PublicationManifest>(v.clone()).is_err());
        v.as_object_mut().unwrap().remove(name);
    }
}
