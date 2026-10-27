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

#[test]
fn metadata_wire_requires_explicit_nullable_playback_and_stays_distinct_from_v1() {
    use crate::contract::theatre::discovery::{MetadataUnitSummary, PresentedUnitSummary};
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/discovery/v1/metadata.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": schema["$defs"], "$ref": "#/$defs/unit"
    }))
    .unwrap();
    for raw in [
        include_str!("../../contracts/theatre/discovery/v1/fixtures/metadata/unreleased.json"),
        include_str!("../../contracts/theatre/discovery/v1/fixtures/metadata/released.json"),
    ] {
        let value: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert!(validator.is_valid(&value));
        let parsed: MetadataUnitSummary = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        assert!(serde_json::from_value::<PresentedUnitSummary>(value.clone()).is_err());
        let mut missing = value;
        missing.as_object_mut().unwrap().remove("playback");
        assert!(!validator.is_valid(&missing));
        assert!(serde_json::from_value::<MetadataUnitSummary>(missing).is_err());
    }
}

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
fn with_rights(mut request: Request, rights: &str) -> Request {
    if let Request::Disclose { rights_id, .. } = &mut request {
        *rights_id = rights.into();
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

async fn disclosed(
    fixture: &ServerFixture,
) -> Option<crate::contexts::catalog::SampleDisclosedWork> {
    let connection = fixture.store().connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    let result = crate::contexts::catalog::sample_disclosed_work(&tx, WORK)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    result
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
            .sample_metadata_decide(ALICE, WORK, None, request(op.clone(), 1, None))
            .await,
        Err(CatalogStoreError::Storage(StoreError::EvidenceUnavailable))
    ));
    // A syntactically bound claim still cannot make forged source bytes valid.
    let rights_id = "sample_metadata_00000000000000000000000000000001";
    let claim = serde_json::json!({"version":"sample-metadata-rights-v1", "operation_id":"00000000-0000-0000-0000-000000000001", "source_id":"sample-catalog-input-v1", "source_sha256":SHA, "evidence_reference":"synthetic", "rights_statement":"synthetic", "confirm_metadata_rights":true, "confirm_source":true}).to_string();
    fixture.exec("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights',?1,?2)", &[&rights_id,&claim]).await.unwrap();
    assert!(matches!(
        fixture
            .store()
            .sample_metadata_decide(
                ALICE,
                WORK,
                None,
                with_rights(request(op, 1, None), rights_id)
            )
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
    let rights_request = crate::contract::studio::SampleMetadataRightsRequest {
        operation_id: Uuid::new_v4().to_string(),
        source_id: imported.id.clone(),
        evidence_reference: "synthetic fixture evidence".into(),
        rights_statement: "synthetic owner declaration".into(),
        confirm_metadata_rights: true,
        confirm_source: true,
    };
    let rights_http = HttpRequest::post("/api/v1/studio/sample-metadata-rights")
        .header(header::ORIGIN, "https://studio.example.test")
        .header(header::COOKIE, format!("cantos_session={ALICE}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&rights_request).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(rights_http).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let rights: crate::contract::studio::SampleMetadataRightsResponse =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        fixture
            .store()
            .record_sample_metadata_rights(ALICE, rights_request.clone())
            .await
            .unwrap(),
        rights
    );
    assert!(matches!(
        fixture
            .store()
            .record_sample_metadata_rights(BOB, rights_request.clone())
            .await,
        Err(CatalogStoreError::Storage(StoreError::EvidenceUnavailable))
    ));
    let mut changed = rights_request.clone();
    changed.rights_statement = "different claim".into();
    assert!(matches!(
        fixture
            .store()
            .record_sample_metadata_rights(ALICE, changed)
            .await,
        Err(CatalogStoreError::OperationReused)
    ));
    let http = HttpRequest::post(format!(
        "/api/v1/studio/works/{WORK}/sample-metadata-disclosures"
    ))
    .header(header::ORIGIN, "https://studio.example.test")
    .header(header::COOKIE, format!("cantos_session={ALICE}"))
    .header(header::CONTENT_TYPE, "application/json")
    .body(Body::from(
        serde_json::to_vec(&with_rights(
            with_source(request(op.clone(), 2, None), &imported.id),
            &rights.rights_id,
        ))
        .unwrap(),
    ))
    .unwrap();
    let response = app.clone().oneshot(http).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let first: crate::contract::studio::SampleMetadataDisclosureResponse =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!((first.sequence, first.decision.as_str()), (1, "disclosed"));
    let visible = disclosed(&fixture).await.expect("owner-disclosed Work");
    assert_eq!(visible.work.id().as_str(), WORK);
    assert!(
        visible.units.is_empty(),
        "a Work decision does not disclose units"
    );
    assert!(visible.first_at >= 0);
    assert_eq!(
        fixture
            .store()
            .sample_metadata_decide(
                ALICE,
                WORK,
                None,
                with_rights(
                    with_source(request(op.clone(), 2, None), &imported.id),
                    &rights.rights_id
                )
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
                with_rights(
                    with_source(request(op.clone(), 1, None), &imported.id),
                    &rights.rights_id
                )
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
            with_rights(
                with_source(
                    request(Uuid::new_v4().to_string(), 2, Some(1)),
                    &imported.id,
                ),
                &rights.rights_id,
            ),
        )
        .await
        .unwrap();
    assert_eq!((unit.sequence, unit.decision.as_str()), (1, "disclosed"));
    let visible = disclosed(&fixture).await.expect("owner-disclosed Work");
    assert_eq!(visible.units.len(), 1);
    assert_eq!(visible.units[0].id().as_str(), UNIT);
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get("/api/v1/theatre/works?presentation=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let old: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(old["items"].as_array().unwrap().is_empty());
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get("/api/v1/theatre/works?presentation=3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: crate::contract::theatre::discovery::MetadataWorkPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].work_id, WORK);
    assert_eq!(page.items[0].unit_count, 1);
    assert!(page.items[0].cover.is_none());
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get(format!("/api/v1/theatre/works/{WORK}/units?presentation=3"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: crate::contract::theatre::discovery::MetadataUnitPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].unit_id, UNIT);
    assert!(page.items[0].playback.is_none());
    // Real eligible release plus the disclosed-only sample, across a one-item walk.
    let released = fixture
        .seed_work(
            None,
            &super::WorkSeed::podcast(71, "Published synthetic", &["Published episode"]),
        )
        .await;
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get("/api/v1/theatre/works?presentation=3&limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let first_page: crate::contract::theatre::discovery::MetadataWorkPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let cursor = first_page
        .next_cursor
        .expect("two distinct Works require a continuation");
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get(format!(
                "/api/v1/theatre/works?presentation=3&limit=1&cursor={cursor}"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let second_page: crate::contract::theatre::discovery::MetadataWorkPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let mut ids = vec![
        first_page.items[0].work_id.clone(),
        second_page.items[0].work_id.clone(),
    ];
    ids.sort();
    let mut expected = vec![WORK.to_string(), released.work_id.clone()];
    expected.sort();
    assert_eq!(ids, expected);
    assert!(second_page.next_cursor.is_none());
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get(format!(
                "/api/v1/theatre/works/{}/units?presentation=3",
                released.work_id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let page: crate::contract::theatre::discovery::MetadataUnitPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        page.items[0].playback.as_ref().unwrap().publication_id,
        released.units[0]
            .chain
            .release_id
            .as_ref()
            .unwrap()
            .as_str()
    );
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get(format!(
                "/api/v1/theatre/works?presentation=2&limit=1&cursor={cursor}"
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "cursors cannot cross presentation boundaries"
    );
    fixture
        .store()
        .sample_metadata_decide(
            ALICE,
            WORK,
            Some(UNIT),
            Request::Withdraw {
                operation_id: Uuid::new_v4().to_string(),
            },
        )
        .await
        .unwrap();
    let visible = disclosed(&fixture).await.expect("Work remains disclosed");
    assert!(visible.units.is_empty(), "unit withdrawal is independent");
    fixture
        .store()
        .sample_metadata_decide(
            ALICE,
            WORK,
            Some(UNIT),
            with_rights(
                with_source(
                    request(Uuid::new_v4().to_string(), 2, Some(1)),
                    &imported.id,
                ),
                &rights.rights_id,
            ),
        )
        .await
        .unwrap();
    assert_eq!(disclosed(&fixture).await.unwrap().units.len(), 1);
    fixture
        .exec("UPDATE actors SET active=0 WHERE id='alice'", &[])
        .await
        .unwrap();
    assert!(
        disclosed(&fixture).await.is_none(),
        "disabled owner cannot be public"
    );
    fixture
        .exec("UPDATE actors SET active=1 WHERE id='alice'", &[])
        .await
        .unwrap();
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
    assert!(
        disclosed(&fixture).await.is_none(),
        "withdrawn parent hides its units"
    );
    let response = app
        .clone()
        .oneshot(
            HttpRequest::get(format!("/api/v1/theatre/works/{WORK}?presentation=3"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    drop(app);
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
        5
    );
}

/// Exercises the production seed code against the real local HTTP router and disposable Turso.
#[tokio::test]
#[ignore = "requires the externally supplied sample-catalog.json and python3"]
async fn full_sample_seed_uses_real_http_and_replays_idempotently() {
    let input = std::env::var_os("CANTOS_APPROVED_SAMPLE_INPUT").expect("sample input path");
    let fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: fixture.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let script_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts");
    let program = r#"
import os, sys
sys.path.insert(0, os.environ['SEED_SCRIPT_DIR'])
import seed_approved_sample_metadata as seed
raw, works = seed.load_input(os.environ['SAMPLE_INPUT'])
client = seed.Studio('http://127.0.0.1:' + os.environ['LOCAL_PORT'], 'https://studio.example.test', 'cantos_session=' + os.environ['FIXTURE_TOKEN'])
for _ in range(2):
    result = seed.seed(client, 'alice', raw, works, 'synthetic fixture source', 'synthetic owner statement')
    assert (result['works'], result['units']) == (6, 12)
"#;
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new("python3")
            .arg("-c")
            .arg(program)
            .env("SEED_SCRIPT_DIR", script_dir)
            .env("SAMPLE_INPUT", input)
            .env("LOCAL_PORT", addr.port().to_string())
            .env("FIXTURE_TOKEN", ALICE)
            .output()
            .expect("python3 executable")
    })
    .await
    .unwrap();
    server.abort();
    assert!(
        result.status.success(),
        "seed failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
