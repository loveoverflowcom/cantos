use cantos_server::contract::theatre::discovery::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
fn schema_accepts(name: &str, v: &Value) -> bool {
    let root: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/discovery.schema.json"
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
        include_str!("../../contracts/theatre/discovery/v1/fixtures/works.json"),
    );
    fixture::<WorkPage>(
        "WorkPage",
        include_str!("../../contracts/theatre/discovery/v1/fixtures/empty.json"),
    );
    fixture::<UnitPage>(
        "UnitPage",
        include_str!("../../contracts/theatre/discovery/v1/fixtures/units.json"),
    );
    for text in [
        include_str!("../../contracts/theatre/discovery/v1/fixtures/units_empty_continuation.json"),
        include_str!("../../contracts/theatre/discovery/v1/fixtures/units_end_empty.json"),
        include_str!("../../contracts/theatre/discovery/v1/fixtures/units_republished.json"),
    ] {
        fixture::<UnitPage>("UnitPage", text);
    }
    fixture::<PublicationManifest>(
        "PublicationManifest",
        include_str!("../../contracts/theatre/discovery/v1/fixtures/manifest.json"),
    );
    for text in [
        include_str!("../../contracts/theatre/discovery/v1/fixtures/not_found.json"),
        include_str!("../../contracts/theatre/discovery/v1/fixtures/invalid_request.json"),
        include_str!("../../contracts/theatre/discovery/v1/fixtures/invalid_cursor.json"),
        include_str!("../../contracts/theatre/discovery/v1/fixtures/unavailable.json"),
        include_str!("../../contracts/theatre/discovery/v1/fixtures/rendition_unavailable.json"),
    ] {
        fixture::<ListenerError>("ListenerError", text);
    }
}
#[test]
fn presentation_opt_in_fixtures_match_separate_strict_wire_types() {
    fixture::<PresentedWorkPage>(
        "PresentedWorkPage",
        include_str!("../../contracts/theatre/discovery/v1/fixtures/presented_works.json"),
    );
    fixture::<PresentedUnitPage>(
        "PresentedUnitPage",
        include_str!("../../contracts/theatre/discovery/v1/fixtures/presented_units.json"),
    );
}
#[test]
fn presentation_fields_are_negotiated_and_positive_safe_integers() {
    let work: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/presented_works.json"
    ))
    .unwrap();
    let unit: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/presented_units.json"
    ))
    .unwrap();
    assert!(!schema_accepts("WorkPage", &work));
    assert!(serde_json::from_value::<WorkPage>(work.clone()).is_err());
    assert!(!schema_accepts("UnitPage", &unit));
    assert!(serde_json::from_value::<UnitPage>(unit.clone()).is_err());
    for (name, field, mut value) in [
        (
            "PresentedWorkSummary",
            "unit_count",
            work["items"][0].clone(),
        ),
        ("PresentedUnitSummary", "ordinal", unit["items"][0].clone()),
    ] {
        for invalid in [json!(0), json!(-1), json!(1.5), json!(9007199254740992_u64)] {
            value[field] = invalid;
            assert!(
                !schema_accepts(name, &value),
                "{name} admits invalid {field}"
            );
            let rust_rejects = match name {
                "PresentedWorkSummary" => {
                    serde_json::from_value::<PresentedWorkSummary>(value.clone()).is_err()
                }
                "PresentedUnitSummary" => {
                    serde_json::from_value::<PresentedUnitSummary>(value.clone()).is_err()
                }
                _ => unreachable!(),
            };
            assert!(rust_rejects, "{name} reader admits invalid {field}");
        }
        value.as_object_mut().unwrap().remove(field);
        assert!(
            !schema_accepts(name, &value),
            "{name} admits missing {field}"
        );
    }
}
#[test]
fn presented_summaries_reject_private_and_undecided_metadata() {
    let work: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/presented_works.json"
    ))
    .unwrap();
    let unit: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/presented_units.json"
    ))
    .unwrap();
    for field in [
        "owner_id",
        "object_key",
        "url",
        "genre",
        "season",
        "episode_number",
    ] {
        let mut w = work["items"][0].clone();
        w[field] = json!("injected");
        assert!(!schema_accepts("PresentedWorkSummary", &w));
        assert!(serde_json::from_value::<PresentedWorkSummary>(w).is_err());
        let mut u = unit["items"][0].clone();
        u[field] = json!("injected");
        assert!(!schema_accepts("PresentedUnitSummary", &u));
        assert!(serde_json::from_value::<PresentedUnitSummary>(u).is_err());
    }
}
#[test]
fn public_manifest_rejects_audit_and_disposable_grant_fields() {
    let mut v: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/manifest.json"
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
            "../../contracts/theatre/discovery/v1/fixtures/works.json"
        )),
    );
    assert_rejects::<UnitSummary>(
        "UnitSummary",
        first_item(include_str!(
            "../../contracts/theatre/discovery/v1/fixtures/units.json"
        )),
    );
}
/// The state samples a client integrates against, as wire facts: an empty page is not the end
/// while `next_cursor` is set, and a republish changes the exact audio of the same unit.
#[test]
fn unit_state_fixtures_pin_empty_continuation_and_version_change() {
    let page = |text: &str| serde_json::from_str::<UnitPage>(text).unwrap();
    let listed = page(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/units.json"
    ));
    let skipped = page(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/units_empty_continuation.json"
    ));
    let ended = page(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/units_end_empty.json"
    ));
    let republished = page(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/units_republished.json"
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
/// Accepted Discovery v1.2 (ADR 0033): `presentation=2` adds a required nullable cover.
#[test]
fn cover_opt_in_fixtures_match_strict_schema_and_shared_rust_types() {
    fixture::<CoveredWorkPage>(
        "CoveredWorkPage",
        include_str!("../../contracts/theatre/discovery/v1/fixtures/cover/covered_works.json"),
    );
    fixture::<CoveredWorkSummary>(
        "CoveredWorkSummary",
        include_str!("../../contracts/theatre/discovery/v1/fixtures/cover/covered_work.json"),
    );
}
#[test]
fn cover_fixtures_bind_url_to_their_work_and_bytes_and_cover_both_states() {
    let page: CoveredWorkPage = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/cover/covered_works.json"
    ))
    .unwrap();
    let detail: CoveredWorkSummary = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/cover/covered_work.json"
    ))
    .unwrap();
    let mut shown = 0;
    for work in page.items.iter().chain([&detail]) {
        let Some(cover) = &work.cover else { continue };
        shown += 1;
        let suffix = format!(
            "/api/v1/theatre/works/{}/cover/{}",
            work.work_id, cover.sha256
        );
        assert!(cover.url.ends_with(&suffix), "{}", cover.url);
    }
    assert_eq!(shown, 2);
    assert!(page.items.iter().any(|work| work.cover.is_none()));
}
#[test]
fn old_readers_reject_cover_responses_and_cover_requires_explicit_null() {
    let page: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/cover/covered_works.json"
    ))
    .unwrap();
    // Default v1 and presentation=1 readers must never receive presentation=2 responses.
    assert!(!schema_accepts("WorkPage", &page));
    assert!(serde_json::from_value::<WorkPage>(page.clone()).is_err());
    assert!(!schema_accepts("PresentedWorkPage", &page));
    assert!(serde_json::from_value::<PresentedWorkPage>(page.clone()).is_err());
    // Omission is not a no-cover state: `null` is the only one.
    let mut missing = page["items"][1].clone();
    missing.as_object_mut().unwrap().remove("cover");
    assert!(!schema_accepts("CoveredWorkSummary", &missing));
    assert!(serde_json::from_value::<CoveredWorkSummary>(missing).is_err());
}
#[test]
fn cover_rejects_private_fields_and_out_of_contract_values() {
    let page: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/fixtures/cover/covered_works.json"
    ))
    .unwrap();
    let cover = page["items"][0]["cover"].clone();
    assert!(schema_accepts("CoverImage", &cover));
    for field in [
        "image_asset_id",
        "image_asset_digest",
        "rights_refs",
        "provenance",
        "owner_id",
        "object_key",
        "receipt",
        "disclosure",
        "valid_until_ms",
    ] {
        let mut v = cover.clone();
        v[field] = json!("private-sentinel");
        assert!(!schema_accepts("CoverImage", &v), "schema admits {field}");
        assert!(
            serde_json::from_value::<CoverImage>(v).is_err(),
            "reader admits {field}"
        );
    }
    let sha = cover["sha256"].as_str().unwrap().to_owned();
    for (field, invalid) in [
        ("content_type", json!("image/webp")),
        ("content_type", json!("image/svg+xml")),
        ("width_px", json!(0)),
        ("width_px", json!(4097)),
        ("height_px", json!(0)),
        ("height_px", json!(4097)),
        ("byte_length", json!(0)),
        ("byte_length", json!(8_388_609)),
    ] {
        let mut v = cover.clone();
        v[field] = invalid;
        assert!(
            !schema_accepts("CoverImage", &v),
            "schema admits {field}={}",
            v[field]
        );
        assert!(
            serde_json::from_value::<CoverImage>(v).is_err(),
            "reader admits {field}"
        );
    }
    for (field, invalid) in [
        ("sha256", "A".repeat(64)),
        ("alt_text", String::new()),
        ("url", format!("http://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-000000000001/cover/{sha}")),
        ("url", format!("https://api.cantos.example/api/v1/theatre/media/pg1_{sha}")),
        ("url", format!("https://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-000000000001/cover/{sha}?sig=x")),
    ] {
        let mut v = cover.clone();
        v[field] = json!(invalid);
        assert!(!schema_accepts("CoverImage", &v), "schema admits {field}={invalid}");
    }
}
