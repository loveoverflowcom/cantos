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
fn presentation_opt_in_fixtures_decode_with_available_and_missing_metadata() {
    for bytes in [
        fixture!("presentation/presented_continue"),
        fixture!("presentation/presented_continue_unavailable"),
    ] {
        check::<dto::PresentedContinuePage>("PresentedContinuePage", bytes);
    }
    for bytes in [
        fixture!("presentation/presented_saved"),
        fixture!("presentation/presented_saved_unavailable"),
    ] {
        check::<dto::PresentedSavedPage>("PresentedSavedPage", bytes);
    }
    for bytes in [
        fixture!("presentation/presented_history"),
        fixture!("presentation/presented_history_unavailable"),
    ] {
        check::<dto::PresentedHistoryPage>("PresentedHistoryPage", bytes);
    }
}
#[test]
fn library_presentation_requires_opt_in_and_explicit_nullable_member() {
    let continued: Value =
        serde_json::from_str(fixture!("presentation/presented_continue")).unwrap();
    let saved: Value = serde_json::from_str(fixture!("presentation/presented_saved")).unwrap();
    let history: Value = serde_json::from_str(fixture!("presentation/presented_history")).unwrap();
    assert!(serde_json::from_value::<dto::ContinuePage>(continued.clone()).is_err());
    assert!(serde_json::from_value::<dto::SavedPage>(saved.clone()).is_err());
    assert!(serde_json::from_value::<dto::HistoryPage>(history.clone()).is_err());
    let missing = |mut value: Value| {
        value["items"][0]
            .as_object_mut()
            .unwrap()
            .remove("presentation");
        value
    };
    assert!(serde_json::from_value::<dto::PresentedContinuePage>(missing(continued)).is_err());
    assert!(serde_json::from_value::<dto::PresentedSavedPage>(missing(saved)).is_err());
    assert!(serde_json::from_value::<dto::PresentedHistoryPage>(missing(history)).is_err());
}
#[test]
fn library_presentation_binds_released_audio_and_rejects_private_fields() {
    let continued: dto::PresentedContinuePage =
        serde_json::from_str(fixture!("presentation/presented_continue")).unwrap();
    let item = &continued.items[0];
    let presentation = item.presentation.as_ref().unwrap();
    assert_eq!(presentation.work.work_id, item.progress.audio.work_id);
    assert_eq!(
        presentation.work.publication_id,
        item.progress.audio.publication_id
    );
    assert_eq!(presentation.unit_id, item.progress.audio.unit_id);
    let root: Value = serde_json::from_str(SCHEMA).unwrap();
    for field in [
        "owner_id",
        "object_key",
        "url",
        "rights_refs",
        "provider",
        "draft_title",
    ] {
        let mut v: Value = serde_json::from_str(fixture!("presentation/presented_saved")).unwrap();
        v["items"][0]["presentation"][field] = Value::String("private".into());
        let mut schema = root.clone();
        schema["$ref"] = Value::String("#/$defs/PresentedSavedPage".into());
        schema.as_object_mut().unwrap().remove("anyOf");
        assert!(!jsonschema::validator_for(&schema).unwrap().is_valid(&v));
        assert!(serde_json::from_value::<dto::PresentedSavedPage>(v).is_err());
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

macro_rules! bookmark_fixture {
    ($file:literal) => {
        include_str!(concat!(
            "../../fixtures/theatre/library/v1/bookmarks/",
            $file,
            ".json"
        ))
    };
}
#[test]
fn bookmark_fixtures_decode_and_reencode_without_drift() {
    check::<dto::BookmarkWrite>("BookmarkWrite", bookmark_fixture!("bookmark_write"));
    check::<dto::BookmarkDelete>("BookmarkDelete", bookmark_fixture!("bookmark_delete"));
    for name in [
        bookmark_fixture!("bookmark_accepted"),
        bookmark_fixture!("bookmark_delete_accepted"),
        bookmark_fixture!("bookmark_deleted"),
    ] {
        check::<dto::Receipt>("Receipt", name);
    }
    check::<dto::BookmarkPage>("BookmarkPage", bookmark_fixture!("bookmarks"));
    check::<dto::BookmarkPage>("BookmarkPage", bookmark_fixture!("bookmarks_empty"));
}
#[test]
fn bookmark_pages_keep_unavailable_releases_without_metadata_and_never_remap() {
    let page: dto::BookmarkPage = serde_json::from_str(bookmark_fixture!("bookmarks")).unwrap();
    assert_eq!(page.items.len(), 2);
    let available = &page.items[0];
    let presentation = available.presentation.as_ref().unwrap();
    assert_eq!(
        presentation.work.publication_id,
        available.bookmark.audio.publication_id
    );
    let retained = &page.items[1];
    assert_eq!(retained.resume, dto::ResumeStatus::AccessUnavailable);
    assert!(retained.presentation.is_none());
    assert_eq!(retained.bookmark.audio, available.bookmark.audio);
}
#[test]
fn bookmark_requests_reject_raw_authority_missing_nullables_and_label_fields() {
    for member in ["actor_id", "listener_id", "label", "note", "revision"] {
        let mut v: Value = serde_json::from_str(bookmark_fixture!("bookmark_write")).unwrap();
        v[member] = Value::String("attacker".into());
        assert!(
            serde_json::from_value::<dto::BookmarkWrite>(v).is_err(),
            "{member}"
        );
    }
    let mut v: Value = serde_json::from_str(bookmark_fixture!("bookmark_write")).unwrap();
    v.as_object_mut().unwrap().remove("bookmark_id");
    assert!(serde_json::from_value::<dto::BookmarkWrite>(v).is_err());
    let mut v: Value = serde_json::from_str(bookmark_fixture!("bookmarks")).unwrap();
    v["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("presentation");
    assert!(serde_json::from_value::<dto::BookmarkPage>(v).is_err());
    let mut v: Value = serde_json::from_str(bookmark_fixture!("bookmark_delete")).unwrap();
    v["bookmark_id"] = Value::String("00000000-0000-4000-8000-0000000000b1".into());
    assert!(serde_json::from_value::<dto::BookmarkDelete>(v).is_err());
}
#[test]
fn bookmark_schema_bounds_page_size_ids_and_deleted_receipt_outcome() {
    let root: Value = serde_json::from_str(SCHEMA).unwrap();
    let valid = |definition: &str, value: &Value| {
        let mut schema = root.clone();
        schema["$ref"] = Value::String(format!("#/$defs/{definition}"));
        schema.as_object_mut().unwrap().remove("anyOf");
        jsonschema::validator_for(&schema).unwrap().is_valid(value)
    };
    let mut write: Value = serde_json::from_str(bookmark_fixture!("bookmark_write")).unwrap();
    assert!(valid("BookmarkWrite", &write));
    write["bookmark_id"] = Value::String("NOT-A-UUID".into());
    assert!(!valid("BookmarkWrite", &write));
    let mut page: Value = serde_json::from_str(bookmark_fixture!("bookmarks")).unwrap();
    let item = page["items"][0].clone();
    page["items"] = Value::Array(vec![item; 51]);
    assert!(!valid("BookmarkPage", &page));
    let deleted: Value = serde_json::from_str(bookmark_fixture!("bookmark_deleted")).unwrap();
    assert!(valid("Receipt", &deleted));
    let mut bad = deleted.clone();
    bad["outcome"] = Value::String("resurrected".into());
    assert!(!valid("Receipt", &bad));
}
