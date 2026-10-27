//! Local file-backed transaction tests; no real account or production data is touched.
use super::ServerFixture;
use crate::contexts::catalog::{
    domain::{
        ContentUnitId, ContentUnitKind, UnitDetails, Visibility, WorkDetails, WorkId, WorkKind,
    },
    CatalogCommand, CatalogStoreError,
};
use crate::contract::studio::SampleMetadataDisclosureRequest as Request;
use crate::platform::store::StoreError;
use axum::{
    body::Body,
    http::{header, Request as HttpRequest, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const WORK: &str = "b4630614-a59d-4a5f-8d34-ae9433f01d2a";
const UNIT: &str = "98e5f9c9-9add-4dad-b0b3-03dcf110f120";
const SHA: &str = "d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212";

fn request(op: String, work_revision: u64, unit_revision: Option<u64>) -> Request {
    Request::Disclose {
        operation_id: op,
        expected_work_revision: work_revision,
        expected_unit_revision: unit_revision,
        source_id: "sample-catalog-input-v1".into(),
        rights_id: "sample-rights".into(),
        confirm_metadata_rights: true,
        confirm_source: true,
    }
}

fn with_source(mut request: Request, source: &str) -> Request {
    if let Request::Disclose { source_id, .. } = &mut request {
        *source_id = source.into();
    }
    request
}

async fn work(fixture: &ServerFixture, title: &str, synopsis: &str, kind: WorkKind) {
    fixture
        .store()
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::CreateWork {
                id: WorkId::parse(WORK).unwrap(),
                kind,
                details: WorkDetails {
                    title: title.into(),
                    synopsis: synopsis.into(),
                    language: "vi-VN".into(),
                    visibility: Visibility::Public,
                    rights_refs: vec![],
                },
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn unowned_or_unsubstantiated_disclosure_has_no_effect() {
    let fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    fixture.studio_caller("bob", BOB).await;
    work(&fixture, "Synthetic", "Synthetic", WorkKind::Story).await;
    let op = Uuid::new_v4().to_string();
    assert!(matches!(
        fixture
            .store()
            .sample_metadata_decide(BOB, WORK, None, request(op.clone(), 1, None))
            .await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(matches!(
        fixture
            .store()
            .sample_metadata_decide(ALICE, WORK, None, request(op.clone(), 1, None))
            .await,
        Err(CatalogStoreError::Storage(StoreError::EvidenceUnavailable))
    ));
    fixture.exec("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights','sample-rights','synthetic')", &[]).await.unwrap();
    assert!(matches!(
        fixture
            .store()
            .sample_metadata_decide(ALICE, WORK, None, request(op.clone(), 1, None))
            .await,
        Err(CatalogStoreError::Storage(StoreError::EvidenceUnavailable))
    ));
    // A stored digest is not evidence that the bytes match it.
    fixture.exec("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','source','sample-catalog-input-v1','synthetic')", &[]).await.unwrap();
    fixture.exec("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice','sample-catalog-input-v1','synthetic','forged',?1,1)", &[&SHA]).await.unwrap();
    assert!(matches!(
        fixture
            .store()
            .sample_metadata_decide(ALICE, WORK, None, request(op, 1, None))
            .await,
        Err(CatalogStoreError::Storage(StoreError::CorruptRevision))
    ));
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_sample_metadata_disclosures",
                &[]
            )
            .await,
        0
    );
}

#[tokio::test]
async fn studio_disclosure_http_requires_cookie_and_valid_json() {
    let fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: fixture.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let path = format!("/api/v1/studio/works/{WORK}/sample-metadata-disclosures");
    let body = serde_json::to_string(&request(Uuid::new_v4().to_string(), 1, None)).unwrap();
    let missing_cookie = HttpRequest::post(&path)
        .header(header::ORIGIN, "https://studio.example.test")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(missing_cookie).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let malformed = HttpRequest::post(&path)
        .header(header::ORIGIN, "https://studio.example.test")
        .header(header::COOKIE, format!("cantos_session={ALICE}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let response = app.oneshot(malformed).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_sample_metadata_disclosures",
                &[]
            )
            .await,
        0
    );
}

#[tokio::test]
#[ignore = "requires the externally supplied sample-catalog.json"]
async fn owner_decision_is_pinned_replayable_and_withdrawn_after_restart() {
    let path = std::env::var_os("CANTOS_APPROVED_SAMPLE_INPUT").expect("sample input path");
    let text = std::fs::read_to_string(path).unwrap();
    let source: serde_json::Value = serde_json::from_str(&text).unwrap();
    let item = &source["works"][0];
    let chapter = &item["chapters"][0];
    let mut fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    fixture.studio_caller("bob", BOB).await;
    work(
        &fixture,
        item["title"].as_str().unwrap(),
        item["description"].as_str().unwrap(),
        WorkKind::parse(item["kind"].as_str().unwrap()).unwrap(),
    )
    .await;
    fixture
        .store()
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::CreateUnit {
                work_id: WorkId::parse(WORK).unwrap(),
                expected_work_revision: 1,
                id: ContentUnitId::parse(UNIT).unwrap(),
                kind: ContentUnitKind::Chapter,
                details: UnitDetails {
                    title: chapter["title"].as_str().unwrap().into(),
                    description: chapter["description"].as_str().unwrap().into(),
                    source: None,
                    script: None,
                    content_digest: None,
                },
            },
        )
        .await
        .unwrap();
    fixture.exec("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights','sample-rights','owner confirmed metadata rights')", &[]).await.unwrap();
    let imported = fixture
        .store()
        .import_manuscript(
            ALICE,
            crate::contract::studio::ImportRequest {
                metadata: crate::contract::studio::ImportMetadata {
                    operation_id: Uuid::new_v4().to_string(),
                    file_name: "sample-catalog.json".into(),
                    format: crate::contract::studio::ImportFormat::Txt,
                    reference: "approved sample".into(),
                    rights_holder: None,
                    permission_evidence: None,
                    usage_scope: None,
                },
                original_bytes: text.as_bytes().to_vec(),
            },
        )
        .await
        .unwrap();
    assert_eq!(imported.sha256, SHA);
    let op = Uuid::new_v4().to_string();
    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: fixture.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let http = HttpRequest::post(format!(
        "/api/v1/studio/works/{WORK}/sample-metadata-disclosures"
    ))
    .header(header::ORIGIN, "https://studio.example.test")
    .header(header::COOKIE, format!("cantos_session={ALICE}"))
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
        serde_json::to_vec(&with_source(request(op.clone(), 2, None), &imported.id)).unwrap(),
    ))
    .unwrap();
    let response = app.oneshot(http).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let first: crate::contract::studio::SampleMetadataDisclosureResponse =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!((first.sequence, first.decision.as_str()), (1, "disclosed"));
    assert_eq!(
        fixture
            .store()
            .sample_metadata_decide(
                ALICE,
                WORK,
                None,
                with_source(request(op.clone(), 2, None), &imported.id)
            )
            .await
            .unwrap(),
        first
    );
    assert!(matches!(
        fixture
            .store()
            .sample_metadata_decide(
                ALICE,
                WORK,
                None,
                with_source(request(op.clone(), 1, None), &imported.id)
            )
            .await,
        Err(CatalogStoreError::OperationReused)
    ));
    let unit = fixture
        .store()
        .sample_metadata_decide(
            ALICE,
            WORK,
            Some(UNIT),
            with_source(
                request(Uuid::new_v4().to_string(), 2, Some(1)),
                &imported.id,
            ),
        )
        .await
        .unwrap();
    assert_eq!((unit.sequence, unit.decision.as_str()), (1, "disclosed"));
    let withdrawal = Request::Withdraw {
        operation_id: Uuid::new_v4().to_string(),
    };
    let result = fixture
        .store()
        .sample_metadata_decide(ALICE, WORK, None, withdrawal.clone())
        .await
        .unwrap();
    assert_eq!(
        (result.sequence, result.decision.as_str()),
        (2, "withdrawn")
    );
    fixture.restart().await;
    assert_eq!(
        fixture
            .store()
            .sample_metadata_decide(ALICE, WORK, None, withdrawal)
            .await
            .unwrap(),
        result
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_sample_metadata_disclosures",
                &[]
            )
            .await,
        3
    );
}
