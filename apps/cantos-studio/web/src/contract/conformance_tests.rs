//! Studio Web's own representation against the Server-owned canonical fixtures: every fixture
//! this client sends or receives decodes into its DTO and re-encodes to the same JSON value. The
//! Server checks its representation and the schemas in its own `contract_conformance` tests;
//! this proves only that the client's separate copy has not drifted from the same bytes.
use super::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

macro_rules! studio {
    ($path:literal) => {
        include_str!(concat!(
            "../../../../../services/cantos-server/contracts/studio/",
            $path
        ))
    };
}

fn same<T: DeserializeOwned + Serialize>(json: &str) {
    let expected: Value = serde_json::from_str(json).unwrap();
    let decoded: T = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
}

fn refuses_unknown_field<T: DeserializeOwned>(json: &str) {
    let mut value: Value = serde_json::from_str(json).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unreviewed".into(), Value::Bool(true));
    assert!(serde_json::from_value::<T>(value).is_err());
}

#[test]
fn editorial_fixtures_round_trip_through_the_studio_representation() {
    same::<ValidateScriptRequest>(studio!("editorial/v1/fixtures/validate-script.json"));
    same::<ScriptValidationResponse>(studio!("editorial/v1/fixtures/validation.json"));
    same::<SaveRevisionRequest>(studio!("editorial/v1/fixtures/save-revision.json"));
    same::<RevisionResponse>(studio!("editorial/v1/fixtures/revision.json"));
    same::<ReviewResponse>(studio!("editorial/v1/fixtures/review.json"));
    same::<HistoryResponse>(studio!("editorial/v1/fixtures/history.json"));
    same::<ApiError>(studio!("editorial/v1/fixtures/stale-revision.json"));
    refuses_unknown_field::<RevisionResponse>(studio!("editorial/v1/fixtures/revision.json"));
}

#[test]
fn catalog_fixtures_round_trip_including_patch_omission_and_null() {
    for json in [
        studio!("catalog/v1/fixtures/create-novel.json"),
        studio!("catalog/v1/fixtures/create-story.json"),
        studio!("catalog/v1/fixtures/create-podcast.json"),
    ] {
        same::<CreateWorkRequest>(json);
    }
    for json in [
        studio!("catalog/v1/fixtures/create-chapter.json"),
        studio!("catalog/v1/fixtures/create-episode.json"),
        studio!("catalog/v1/fixtures/create-linked-chapter.json"),
    ] {
        same::<CreateContentUnitRequest>(json);
    }
    same::<PatchWorkRequest>(studio!("catalog/v1/fixtures/patch-work.json"));
    same::<PatchContentUnitRequest>(studio!("catalog/v1/fixtures/patch-unit.json"));
    same::<ArchiveWorkRequest>(studio!("catalog/v1/fixtures/archive-work.json"));
    same::<ArchiveContentUnitRequest>(studio!("catalog/v1/fixtures/archive-unit.json"));
    same::<ReorderContentUnitsRequest>(studio!("catalog/v1/fixtures/reorder.json"));
    same::<CatalogWork>(studio!("catalog/v1/fixtures/work.json"));
    same::<CatalogContentUnit>(studio!("catalog/v1/fixtures/unit.json"));
    same::<CatalogMutationResponse>(studio!("catalog/v1/fixtures/mutation.json"));
    for json in [
        studio!("catalog/v1/fixtures/work-page.json"),
        studio!("catalog/v1/fixtures/work-page-with-cursor.json"),
        studio!("catalog/v1/fixtures/history-page.json"),
    ] {
        same::<CatalogPage<CatalogWork>>(json);
    }
    same::<CatalogPage<CatalogContentUnit>>(studio!("catalog/v1/fixtures/unit-page.json"));
    for json in [
        studio!("catalog/v1/fixtures/catalog-conflict.json"),
        studio!("catalog/v1/fixtures/invalid-catalog.json"),
        studio!("catalog/v1/fixtures/stale-cursor.json"),
    ] {
        same::<ApiError>(json);
    }
    refuses_unknown_field::<CatalogWork>(studio!("catalog/v1/fixtures/work.json"));
}

#[test]
fn content_delegation_brief_and_lifecycle_fixtures_round_trip() {
    same::<AppendContentRequest>(studio!("content/v1/fixtures/text-request.json"));
    same::<AppendContentRequest>(studio!("content/v1/fixtures/recording-request.json"));
    same::<ContentRevisionResponse>(studio!("content/v1/fixtures/text-revision.json"));
    same::<IssueDelegationRequest>(studio!("delegation/v1/fixtures/issue-request.json"));
    same::<IssueDelegationResponse>(studio!("delegation/v1/fixtures/issue-created.json"));
    same::<IssueDelegationResponse>(studio!("delegation/v1/fixtures/issue-replay.json"));
    same::<DelegationGrantPage>(studio!("delegation/v1/fixtures/grant-page.json"));
    same::<CreateProductionBriefRequest>(studio!("briefs/v1/fixtures/create-narration.json"));
    same::<CreateProductionBriefRequest>(studio!("briefs/v1/fixtures/create-recording.json"));
    same::<ProductionBriefDocument>(studio!("briefs/v1/fixtures/narration-document.json"));
    same::<CreateAudioSubmissionRequest>(studio!(
        "audio-lifecycle/v1/fixtures/prepare-recording.json"
    ));
    same::<AudioRenditionDocument>(studio!(
        "audio-lifecycle/v1/fixtures/rendition-response.json"
    ));
    same::<AudioMediaMetadata>(studio!("audio-intake/v1/fixtures/media-metadata.json"));
}

#[test]
fn production_fixtures_round_trip() {
    same::<ProductionCatalogResponse>(studio!("production/v1/fixtures/catalog-response.json"));
    same::<SaveProductionSettingsRequest>(studio!("production/v1/fixtures/settings-request.json"));
    same::<SaveProductionRightsRequest>(studio!("production/v1/fixtures/rights-request.json"));
}

#[test]
fn publication_requests_round_trip_through_the_studio_representation() {
    same::<RecordDistributionRightsRequest>(studio!("publication/v1/fixtures/clearance.json"));
    same::<RecordPublicationApprovalRequest>(studio!("publication/v1/fixtures/approval.json"));
    same::<PublishAudioReleaseRequest>(studio!("publication/v1/fixtures/publish.json"));
    same::<WithdrawAudioReleaseRequest>(studio!("publication/v1/fixtures/withdraw.json"));
    refuses_unknown_field::<PublishAudioReleaseRequest>(studio!(
        "publication/v1/fixtures/publish.json"
    ));
}

#[test]
fn preview_fixtures_round_trip_and_refuse_catalog_private_fields() {
    for json in [
        studio!("preview/v1/fixtures/first-page.json"),
        studio!("preview/v1/fixtures/last-page-empty-work.json"),
    ] {
        same::<StudioPreview>(json);
        refuses_unknown_field::<StudioPreview>(json);
        let mut value: Value = serde_json::from_str(json).unwrap();
        value["work"]["owner_id"] = Value::String("alice".into());
        assert!(serde_json::from_value::<StudioPreview>(value).is_err());
    }
}
