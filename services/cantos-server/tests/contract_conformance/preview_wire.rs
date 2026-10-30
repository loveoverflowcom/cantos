//! Studio preview v1 (ADR 0054 D03, #650): fixtures match the strict schema and the Server's
//! Rust DTOs, and the allowlist stays closed in both readers.
use cantos_server::contract::studio::StudioPreview;
use serde_json::{json, Value};

fn schema_accepts(name: &str, v: &Value) -> bool {
    let root: Value = serde_json::from_str(include_str!(
        "../../contracts/studio/preview/v1/preview.schema.json"
    ))
    .unwrap();
    let schema =
        json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{name}")});
    jsonschema::validator_for(&schema).unwrap().is_valid(v)
}

const FIXTURES: [&str; 2] = [
    include_str!("../../contracts/studio/preview/v1/fixtures/first-page.json"),
    include_str!("../../contracts/studio/preview/v1/fixtures/last-page-empty-work.json"),
];

#[test]
fn preview_fixtures_match_strict_schema_and_round_trip_through_rust_types() {
    for text in FIXTURES {
        let v: Value = serde_json::from_str(text).unwrap();
        assert!(schema_accepts("StudioPreview", &v));
        let raw: StudioPreview = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(raw).unwrap(), v);
    }
}

#[test]
fn a_catalog_private_field_a_missing_field_or_a_bad_revision_is_refused_by_both_readers() {
    let base: Value = serde_json::from_str(FIXTURES[0]).unwrap();
    for (path, field, value) in [
        ("work", "owner_id", json!("alice")),
        ("work", "visibility", json!("public")),
        ("work", "rights_refs", json!([])),
        ("work", "lifecycle", json!("draft")),
        ("work", "created_at", json!(1)),
        ("unit", "position", json!(1)),
        ("unit", "content_digest", json!("a".repeat(64))),
        (
            "unit",
            "source",
            json!({"id": "s", "sha256": "a".repeat(64)}),
        ),
        ("unit", "cover", json!("https://example.invalid/c.avif")),
        ("unit", "audio", json!(null)),
    ] {
        let mut v = base.clone();
        let target = if path == "work" {
            &mut v["work"]
        } else {
            &mut v["units"][0]
        };
        target[field] = value;
        assert!(!schema_accepts("StudioPreview", &v), "{path}.{field}");
        assert!(
            serde_json::from_value::<StudioPreview>(v).is_err(),
            "{path}.{field}"
        );
    }
    for (path, bad) in [
        ("/work/revision", json!(0)),
        ("/units/0/ordinal", json!(0)),
        ("/units/0/revision", json!(-1)),
        ("/work/kind", json!("album")),
        ("/units/0/kind", json!("scene")),
        ("/work/id", json!("not-a-uuid")),
    ] {
        let mut v = base.clone();
        *v.pointer_mut(path).unwrap() = bad;
        assert!(!schema_accepts("StudioPreview", &v), "{path}");
    }
    // `next_cursor` is required, even when null.
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("next_cursor");
    assert!(!schema_accepts("StudioPreview", &missing));
}
