use cantos_contracts::{
    routes, ApiError, ArchiveContentUnitRequest, ArchiveWorkRequest, CatalogContentUnit,
    CatalogMutationResponse, CatalogPage, CatalogWork, CreateContentUnitRequest, CreateWorkRequest,
    ErrorCode, PatchContentUnitRequest, PatchField, PatchWorkRequest, ReorderContentUnitsRequest,
    UnitPatchDto, WorkPatchDto,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const CREATE: &str = include_str!("../../fixtures/catalog/v1/create-novel.json");
const CREATE_UNIT: &str = include_str!("../../fixtures/catalog/v1/create-linked-chapter.json");
const WORK: &str = include_str!("../../fixtures/catalog/v1/work.json");
const UNIT: &str = include_str!("../../fixtures/catalog/v1/unit.json");

fn round_trip<T: DeserializeOwned + Serialize>(fixture: &str) {
    let decoded: T = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        serde_json::to_value(decoded).unwrap(),
        serde_json::from_str::<Value>(fixture).unwrap()
    );
}

#[test]
fn independent_request_fixtures_round_trip_with_plain_story_and_unscripted_podcast() {
    for fixture in [
        CREATE,
        include_str!("../../fixtures/catalog/v1/create-story.json"),
        include_str!("../../fixtures/catalog/v1/create-podcast.json"),
    ] {
        round_trip::<CreateWorkRequest>(fixture);
    }
    round_trip::<PatchWorkRequest>(include_str!("../../fixtures/catalog/v1/patch-work.json"));
    round_trip::<ArchiveWorkRequest>(include_str!("../../fixtures/catalog/v1/archive-work.json"));
    for fixture in [
        CREATE_UNIT,
        include_str!("../../fixtures/catalog/v1/create-chapter.json"),
        include_str!("../../fixtures/catalog/v1/create-episode.json"),
    ] {
        round_trip::<CreateContentUnitRequest>(fixture);
    }
    let episode: CreateContentUnitRequest = serde_json::from_str(include_str!(
        "../../fixtures/catalog/v1/create-episode.json"
    ))
    .unwrap();
    assert_eq!(episode.kind, "episode");
    assert_eq!(episode.details.script, None);
    assert_eq!(episode.details.source, None);
    round_trip::<PatchContentUnitRequest>(include_str!(
        "../../fixtures/catalog/v1/patch-unit.json"
    ));
    round_trip::<ArchiveContentUnitRequest>(include_str!(
        "../../fixtures/catalog/v1/archive-unit.json"
    ));
    round_trip::<ReorderContentUnitsRequest>(include_str!(
        "../../fixtures/catalog/v1/reorder.json"
    ));
}

#[test]
fn response_fixtures_preserve_immutable_identity_revision_and_millisecond_timestamps() {
    round_trip::<CatalogWork>(WORK);
    round_trip::<CatalogContentUnit>(UNIT);
    round_trip::<CatalogMutationResponse>(include_str!("../../fixtures/catalog/v1/mutation.json"));
    round_trip::<CatalogPage<CatalogWork>>(include_str!(
        "../../fixtures/catalog/v1/work-page.json"
    ));
    round_trip::<CatalogPage<CatalogWork>>(include_str!(
        "../../fixtures/catalog/v1/work-page-with-cursor.json"
    ));
    round_trip::<CatalogPage<CatalogContentUnit>>(include_str!(
        "../../fixtures/catalog/v1/unit-page.json"
    ));
    round_trip::<CatalogPage<CatalogWork>>(include_str!(
        "../../fixtures/catalog/v1/history-page.json"
    ));
    let history: CatalogPage<CatalogWork> =
        serde_json::from_str(include_str!("../../fixtures/catalog/v1/history-page.json")).unwrap();
    assert_eq!(history.items[0].id, history.items[1].id);
    assert_eq!(history.items[0].revision, 1);
    assert_eq!(history.items[1].revision, 2);
    assert_eq!(history.items[0].created_at, 1_791_800_000_000);
    assert_eq!(history.items[1].updated_at, 1_791_800_000_001);
    assert_eq!(history.next_cursor, None);
    let page: CatalogPage<CatalogWork> = serde_json::from_str(include_str!(
        "../../fixtures/catalog/v1/work-page-with-cursor.json"
    ))
    .unwrap();
    assert_eq!(page.items[0].id, history.items[0].id);
    assert!(page.next_cursor.unwrap().starts_with("7b22"));
}

#[test]
fn patch_preserves_omission_null_and_value_as_three_distinct_inputs() {
    let patch: UnitPatchDto = serde_json::from_value(json!({
        "title":"Chương mới", "source":null, "content_digest":null
    }))
    .unwrap();
    assert_eq!(patch.title, PatchField::Value("Chương mới".into()));
    assert_eq!(patch.description, PatchField::Missing);
    assert_eq!(patch.source, PatchField::Null);
    assert_eq!(patch.script, PatchField::Missing);
    assert_eq!(patch.content_digest, PatchField::Null);
    assert_eq!(
        serde_json::to_value(patch).unwrap(),
        json!({"title":"Chương mới", "source":null, "content_digest":null})
    );
    let omitted: UnitPatchDto = serde_json::from_value(json!({})).unwrap();
    assert_eq!(omitted, UnitPatchDto::default());
    assert_eq!(serde_json::to_value(omitted).unwrap(), json!({}));
    let null: WorkPatchDto = serde_json::from_value(json!({"title":null})).unwrap();
    assert_eq!(null.title, PatchField::Null);
    // Required-field null is retained for the server's independent field diagnostics.
    assert_eq!(serde_json::to_value(null).unwrap(), json!({"title":null}));
}

#[test]
fn optional_link_absence_on_creation_serializes_as_explicit_unknown_facts() {
    let mut value: Value = serde_json::from_str(CREATE_UNIT).unwrap();
    for field in ["source", "script", "content_digest"] {
        value["details"].as_object_mut().unwrap().remove(field);
    }
    let request: CreateContentUnitRequest = serde_json::from_value(value).unwrap();
    assert_eq!(request.details.source, None);
    assert_eq!(request.details.script, None);
    assert_eq!(request.details.content_digest, None);
    let encoded = serde_json::to_value(request).unwrap();
    for field in ["source", "script", "content_digest"] {
        assert_eq!(encoded["details"][field], Value::Null);
    }
}

#[test]
fn request_transport_refuses_forged_authority_at_every_nested_boundary() {
    for pointer in ["", "/details"] {
        let mut value: Value = serde_json::from_str(CREATE).unwrap();
        value.pointer_mut(pointer).unwrap()["owner_id"] = json!("stranger");
        let error = serde_json::from_value::<CreateWorkRequest>(value).unwrap_err();
        assert!(error.to_string().contains("unknown field `owner_id`"));
    }
    for pointer in ["", "/details", "/details/source", "/details/script"] {
        let mut value: Value = serde_json::from_str(CREATE_UNIT).unwrap();
        value.pointer_mut(pointer).unwrap()["authorized"] = json!(true);
        let error = serde_json::from_value::<CreateContentUnitRequest>(value).unwrap_err();
        assert!(error.to_string().contains("unknown field `authorized`"));
    }
    for patch in [json!({"owner_id":"stranger"}), json!({"kind":"podcast"})] {
        let error = serde_json::from_value::<WorkPatchDto>(patch).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }
    let forged = include_str!("../../fixtures/catalog/v1/reject-owner.json");
    let error = serde_json::from_str::<CreateWorkRequest>(forged).unwrap_err();
    assert!(error.to_string().contains("unknown field `owner_id`"));
}

#[test]
fn response_transport_is_closed_without_publication_or_rights_assertions() {
    for pointer in ["", "/details"] {
        let mut value: Value = serde_json::from_str(WORK).unwrap();
        value.pointer_mut(pointer).unwrap()["published"] = json!(true);
        let error = serde_json::from_value::<CatalogWork>(value).unwrap_err();
        assert!(error.to_string().contains("unknown field `published`"));
    }
    for pointer in ["", "/details"] {
        let mut value: Value = serde_json::from_str(UNIT).unwrap();
        value.pointer_mut(pointer).unwrap()["rights_verified"] = json!(true);
        let error = serde_json::from_value::<CatalogContentUnit>(value).unwrap_err();
        assert!(error
            .to_string()
            .contains("unknown field `rights_verified`"));
    }
    let error = serde_json::from_value::<CatalogPage<CatalogWork>>(
        json!({"items":[],"next_cursor":null,"total":100}),
    )
    .unwrap_err();
    assert!(error.to_string().contains("unknown field `total`"));
}

#[test]
fn raw_transport_retains_semantic_errors_but_refuses_wrong_numeric_types() {
    let unknown: CreateWorkRequest =
        serde_json::from_str(include_str!("../../fixtures/catalog/v1/reject-kind.json")).unwrap();
    assert_eq!(unknown.kind, "audio_book");
    for value in [json!(-1), json!(1.5), json!("1"), Value::Null] {
        let mut request: Value =
            serde_json::from_str(include_str!("../../fixtures/catalog/v1/patch-work.json"))
                .unwrap();
        request["expected_revision"] = value;
        let error = serde_json::from_value::<PatchWorkRequest>(request).unwrap_err();
        assert!(
            error.to_string().contains("invalid type")
                || error.to_string().contains("invalid value")
        );
    }
    for value in [json!(-1), json!(4_294_967_296_u64), json!(1.5), json!("1")] {
        let mut unit: Value = serde_json::from_str(UNIT).unwrap();
        unit["position"] = value;
        let error = serde_json::from_value::<CatalogContentUnit>(unit).unwrap_err();
        assert!(
            error.to_string().contains("invalid type")
                || error.to_string().contains("invalid value")
        );
    }
}

#[test]
fn catalog_error_codes_use_the_shared_stable_json_envelope() {
    for (fixture, code) in [
        (
            include_str!("../../fixtures/catalog/v1/invalid-catalog.json"),
            ErrorCode::InvalidCatalog,
        ),
        (
            include_str!("../../fixtures/catalog/v1/catalog-conflict.json"),
            ErrorCode::CatalogConflict,
        ),
        (
            include_str!("../../fixtures/catalog/v1/stale-cursor.json"),
            ErrorCode::StaleCursor,
        ),
    ] {
        round_trip::<ApiError>(fixture);
        let error: ApiError = serde_json::from_str(fixture).unwrap();
        assert_eq!(error.code, code);
        assert_eq!(error.current_revision, None);
    }
}

#[test]
fn every_catalog_route_constant_has_the_current_studio_prefix() {
    for path in [
        routes::STUDIO_WORKS,
        routes::STUDIO_WORK,
        routes::STUDIO_WORK_HISTORY,
        routes::STUDIO_CONTENT_UNITS,
        routes::STUDIO_CONTENT_UNITS_REORDER,
        routes::STUDIO_CONTENT_UNIT,
    ] {
        assert!(path.starts_with("/api/v1/studio/"), "{path}");
    }
}
