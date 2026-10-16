use cantos_contracts::theatre::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
fn schema_accepts(name: &str, v: &Value) -> bool {
    let root: Value = serde_json::from_str(include_str!(
        "../../schema/theatre/discovery/v1.schema.json"
    ))
    .unwrap();
    let schema =
        json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{name}")});
    jsonschema::validator_for(&schema).unwrap().is_valid(v)
}
fn fixture<T: DeserializeOwned + Serialize>(name: &str, text: &str) {
    let v: Value = serde_json::from_str(text).unwrap();
    assert!(schema_accepts(name, &v));
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
    for text in [
        include_str!("../../fixtures/theatre/discovery/v1/units_empty_continuation.json"),
        include_str!("../../fixtures/theatre/discovery/v1/units_end_empty.json"),
        include_str!("../../fixtures/theatre/discovery/v1/units_republished.json"),
    ] {
        fixture::<UnitPage>("UnitPage", text);
    }
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
/// Anonymous catalog rows carry no private evidence and no per-row availability. Adding a state
/// field (an `availability` projection, ADR 0035 alternative) is a deliberate v1 review, so
/// strict readers and the schema must reject it until that decision lands.
#[test]
fn public_list_summaries_reject_private_and_row_state_fields() {
    fn first_item(text: &str) -> Value {
        let page: Value = serde_json::from_str(text).unwrap();
        page["items"][0].clone()
    }
    fn assert_rejects<T: DeserializeOwned>(name: &str, mut v: Value) {
        for field in [
            "owner_id",
            "visibility",
            "rights_refs",
            "candidate_id",
            "review_evidence",
            "object_key",
            "url",
            "availability",
            "state",
            "status",
        ] {
            v[field] = json!("injected");
            assert!(!schema_accepts(name, &v), "{name} schema admits {field}");
            assert!(
                serde_json::from_value::<T>(v.clone()).is_err(),
                "{name} reader admits {field}"
            );
            v.as_object_mut().unwrap().remove(field);
        }
        assert!(schema_accepts(name, &v) && serde_json::from_value::<T>(v).is_ok());
    }
    assert_rejects::<WorkSummary>(
        "WorkSummary",
        first_item(include_str!(
            "../../fixtures/theatre/discovery/v1/works.json"
        )),
    );
    assert_rejects::<UnitSummary>(
        "UnitSummary",
        first_item(include_str!(
            "../../fixtures/theatre/discovery/v1/units.json"
        )),
    );
}
/// The state samples a client integrates against, as wire facts: an empty page is not the end
/// while `next_cursor` is set, and a republish changes the exact audio of the same unit.
#[test]
fn unit_state_fixtures_pin_empty_continuation_and_version_change() {
    let page = |text: &str| serde_json::from_str::<UnitPage>(text).unwrap();
    let listed = page(include_str!(
        "../../fixtures/theatre/discovery/v1/units.json"
    ));
    let skipped = page(include_str!(
        "../../fixtures/theatre/discovery/v1/units_empty_continuation.json"
    ));
    let ended = page(include_str!(
        "../../fixtures/theatre/discovery/v1/units_end_empty.json"
    ));
    let republished = page(include_str!(
        "../../fixtures/theatre/discovery/v1/units_republished.json"
    ));

    assert!(skipped.items.is_empty() && skipped.next_cursor.is_some());
    assert!(ended.items.is_empty() && ended.next_cursor.is_none());
    assert_eq!(
        [&listed.work_id, &skipped.work_id, &ended.work_id],
        [&republished.work_id; 3]
    );

    let (before, after) = (&listed.items[0], &republished.items[0]);
    assert_eq!(
        (&before.unit_id, before.position),
        (&after.unit_id, after.position)
    );
    assert_ne!(before.publication_id, after.publication_id);
    assert!(after.published_at_ms > before.published_at_ms);
    assert!(before.duration_ms > 0 && after.duration_ms > 0);
}
