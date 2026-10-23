//! Real Studio HTTP → application → run-owned native Turso; synthetic Vietnamese metadata.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_server::contract::studio::SaveRevisionRequest;
use cantos_server::platform::store::token_hash;
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    contexts::catalog::{domain::WorkId, CatalogCommand, MemberRole},
    platform::store::Store,
    script_ir::read_script,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    time::{Duration, Instant},
};
#[path = "support/native_sql.rs"]
mod native_sql;
use native_sql::NativeSql;
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CAROL: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const ORIGIN: &str = "http://127.0.0.1:8080";
const SCRIPT: &str =
    include_str!("../contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json");
const ORIGINAL: &str = "\u{feff}Ánh đèn cuối sân khấu\r\nNgười kể chuyện đã trở về.\r\n";

struct Harness {
    admin: NativeSql,
    store: Store,
    app: Router,
}

struct Reply {
    status: StatusCode,
    body: Value,
}

impl Reply {
    fn success(self, expected: StatusCode) -> Value {
        assert_eq!(self.status, expected, "{}", self.body);
        self.body
    }

    fn error(self, status: StatusCode, code: &str) -> Value {
        assert_eq!(self.status, status, "{}", self.body);
        assert_eq!(self.body["code"], code);
        self.body
    }
}

fn operation() -> String {
    Uuid::new_v4().to_string()
}

fn work_request(kind: &str) -> Value {
    json!({
        "operation_id": operation(), "id": operation(), "kind": kind,
        "details": {
            "title": "Ánh đèn cuối sân khấu", "synopsis": "Một tác phẩm thử nghiệm có dấu tiếng Việt.",
            "language": "vi-VN", "visibility": "private", "rights_refs": []
        }
    })
}

fn unit_request(work: &Value, title: &str) -> Value {
    json!({
        "operation_id": operation(), "id": operation(),
        "kind": if work["kind"] == "podcast" { "episode" } else { "chapter" },
        "expected_work_revision": work["revision"],
        "details": {"title": title, "description": "Nội dung thử nghiệm, chưa có âm thanh."}
    })
}

fn schema(definition: &str, body: &Value) {
    let root: Value = serde_json::from_str(
        if matches!(
            definition,
            "PublicationReadinessResponse"
                | "DistributionClearanceDocument"
                | "PublicationApprovalDocument"
                | "ActiveAudioReleaseResponse"
                | "AudioReleaseWithdrawalResponse"
        ) {
            include_str!("../contracts/studio/publication/v1/publication.schema.json")
        } else if definition == "ApiError"
            || definition.starts_with("Audio")
            || definition.starts_with("ReleaseCandidate")
        {
            include_str!("../contracts/studio/audio-lifecycle/v1/audio-lifecycle.schema.json")
        } else if matches!(
            definition,
            "ProductionBriefResponse" | "ProductionBriefDocument" | "ProducerGrantResponse"
        ) {
            include_str!("../contracts/studio/briefs/v1/briefs.schema.json")
        } else if matches!(
            definition,
            "AppendContentResponse" | "ContentRevisionResponse" | "ContentHistoryResponse"
        ) {
            include_str!("../contracts/studio/content/v1/content.schema.json")
        } else {
            include_str!("../contracts/studio/catalog/v1/catalog.schema.json")
        },
    )
    .unwrap();
    let selected = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": root["$defs"], "$ref": format!("#/$defs/{definition}")
    });
    let validator = jsonschema::validator_for(&selected).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(body)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{definition}: {errors:?}; {body}");
}

async fn request(
    app: Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Vec<u8>,
    headers: (&str, &str, &str),
) -> Reply {
    let (content_type, accept, origin) = headers;
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/v1/studio{path}"))
        .header("origin", origin)
        .header("content-type", content_type)
        .header("accept", accept);
    if let Some(token) = token {
        builder = builder.header("cookie", format!("cantos_session={token}"));
    }
    let response = app
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    if response.status() != StatusCode::NO_CONTENT {
        assert_eq!(response.headers()["content-type"], "application/json");
    }
    let status = response.status();
    if status == StatusCode::NO_CONTENT {
        return Reply {
            status,
            body: Value::Null,
        };
    }
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let definition = if !status.is_success() {
        "ApiError"
    } else if body.get("withdrawn_release_id").is_some() {
        "AudioReleaseWithdrawalResponse"
    } else if body.get("pointer_version").is_some() {
        if body.get("blockers").is_some() {
            "PublicationReadinessResponse"
        } else {
            "ActiveAudioReleaseResponse"
        }
    } else if body.get("actor_id").is_some() && body.get("candidate_id").is_some() {
        if body["request"].get("clearance_digest").is_some() {
            "PublicationApprovalDocument"
        } else {
            "DistributionClearanceDocument"
        }
    } else if body.get("submission").is_some() {
        "AudioSubmissionResponse"
    } else if body.get("candidate").is_some() {
        "ReleaseCandidateResponse"
    } else if body.get("reviewer_id").is_some() {
        "AudioReviewDocument"
    } else if body.get("brief").is_some() {
        "ProductionBriefResponse"
    } else if body.get("permission").is_some() {
        "ProducerGrantResponse"
    } else if body.get("content").is_some() {
        "AppendContentResponse"
    } else if body.get("unit_id").is_some() {
        "ContentRevisionResponse"
    } else if path.contains("/content/history") {
        "ContentHistoryResponse"
    } else if body.get("work").is_some() {
        "CatalogMutationResponse"
    } else if body.get("items").is_some() {
        if path.contains("content-units") {
            "CatalogContentUnitPage"
        } else {
            "CatalogWorkPage"
        }
    } else if body.get("work_id").is_some() {
        "CatalogContentUnit"
    } else {
        "CatalogWork"
    };
    schema(definition, &body);
    Reply { status, body }
}

impl Harness {
    async fn new() -> Self {
        Self::with_fixture(NativeSql::new().await).await
    }
    async fn with_fixture(admin: NativeSql) -> Self {
        for (actor, token) in [("alice", ALICE), ("bob", BOB), ("carol", CAROL)] {
            admin.0.studio_caller(actor, token).await;
            for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
                admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,?2,?3,'Synthetic reference; no clearance')", &[&actor,&kind,&id]).await.unwrap();
            }
            admin.execute("INSERT INTO script_evidence VALUES(?1,'rights','catalog-rights','Synthetic rights')", &[&actor]).await.unwrap();
        }
        let store = admin.store();
        let app = router(
            AppState {
                store: store.clone(),
                web_origin: ORIGIN.into(),
            },
            "apps/cantos-studio/web/dist",
        );
        Self { admin, store, app }
    }
    async fn restart_with_audio(&mut self, root: &std::path::Path) {
        let replacement = NativeSql::new().await;
        let old = std::mem::replace(&mut self.store, replacement.store());
        self.app = router(
            AppState {
                store: replacement.store(),
                web_origin: ORIGIN.into(),
            },
            "unused-public-root",
        );
        drop(old);
        self.admin.0.restart().await;
        self.store = self.admin.store().with_audio_storage(root).unwrap();
        self.app = router(
            AppState {
                store: self.store.clone(),
                web_origin: ORIGIN.into(),
            },
            "unused-public-root",
        );
    }
    async fn json(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        request(
            self.app.clone(),
            method,
            path,
            token,
            body.map_or_else(Vec::new, |body| serde_json::to_vec(&body).unwrap()),
            ("application/json", "application/json", ORIGIN),
        )
        .await
    }

    async fn create(&self, token: &str, kind: &str) -> Value {
        self.json("POST", "/works", Some(token), Some(work_request(kind)))
            .await
            .success(StatusCode::CREATED)["work"]
            .clone()
    }

    async fn unit(&self, work: &Value, title: &str) -> Value {
        self.json(
            "POST",
            &format!("/works/{}/content-units", work["id"].as_str().unwrap()),
            Some(ALICE),
            Some(unit_request(work, title)),
        )
        .await
        .success(StatusCode::CREATED)
    }

    async fn counts(&self) -> Value {
        let text: String = self.admin.query_one("SELECT json_object('works',(SELECT count(*) FROM catalog_works),'work_revisions',(SELECT count(*) FROM catalog_work_revisions),'units',(SELECT count(*) FROM catalog_units),'unit_revisions',(SELECT count(*) FROM catalog_unit_revisions),'members',(SELECT count(*) FROM catalog_members),'operations',(SELECT count(*) FROM catalog_operations),'clock',(SELECT revision FROM catalog_clock),'content_revisions',(SELECT count(*) FROM catalog_content_revisions),'content_heads',(SELECT count(*) FROM catalog_content_heads))", &[]).await.unwrap().get(0);
        serde_json::from_str(&text).unwrap()
    }

    async fn member(&self, work: &Value, role: MemberRole) -> Value {
        let outcome = self
            .store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::SetMember {
                    work_id: WorkId::parse(work["id"].as_str().unwrap()).unwrap(),
                    expected_revision: work["revision"].as_u64().unwrap(),
                    actor_id: cantos_server::contexts::catalog::domain::OwnerId::parse("bob")
                        .unwrap(),
                    role: Some(role),
                },
            )
            .await
            .unwrap();
        serde_json::to_value(outcome.work).unwrap()
    }
}

#[tokio::test]
async fn http_catalog_creates_all_categories_without_script_cast_and_rejects_wrong_unit_kind() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    for kind in ["novel", "story", "podcast"] {
        let work = h.create(ALICE, kind).await;
        assert_eq!(work["owner_id"], "alice");
        let unit = h.unit(&work, "Chương một — Người kể trở về").await;
        let id = unit["units"][0]["id"].as_str().unwrap();
        assert_eq!(unit["units"][0]["details"]["script"], Value::Null);
        assert_eq!(
            unit["units"][0]["kind"],
            if kind == "podcast" {
                "episode"
            } else {
                "chapter"
            }
        );
        assert_eq!(
            h.json("GET", &format!("/content-units/{id}"), Some(ALICE), None)
                .await
                .success(StatusCode::OK),
            unit["units"][0]
        );
        let mut invalid = unit_request(&unit["work"], "Sai loại");
        invalid["kind"] = json!(if kind == "podcast" {
            "chapter"
        } else {
            "episode"
        });
        let before = h.counts().await;
        h.json(
            "POST",
            &format!("/works/{}/content-units", work["id"].as_str().unwrap()),
            Some(ALICE),
            Some(invalid),
        )
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_catalog");
        assert_eq!(h.counts().await, before);
    }
    assert_eq!(h.counts().await["works"], 3);
    assert_eq!(h.counts().await["units"], 3);
}

#[tokio::test]
async fn http_retry_returns_original_snapshot_duplicate_ids_and_changed_intents_do_not_commit() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let body = work_request("story");
    let created = h
        .json("POST", "/works", Some(ALICE), Some(body.clone()))
        .await
        .success(StatusCode::CREATED);
    let id = created["work"]["id"].as_str().unwrap();
    let patch = json!({"operation_id": operation(), "expected_revision": 1, "details": {"title": "Tên mới sau khi sửa"}});
    let edited = h
        .json(
            "PATCH",
            &format!("/works/{id}"),
            Some(ALICE),
            Some(patch.clone()),
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(
        edited["work"]["details"]["synopsis"],
        created["work"]["details"]["synopsis"]
    );
    let before = h.counts().await;
    assert_eq!(
        h.json("POST", "/works", Some(ALICE), Some(body.clone()))
            .await
            .success(StatusCode::CREATED),
        created
    );
    assert_eq!(
        h.json(
            "PATCH",
            &format!("/works/{id}"),
            Some(ALICE),
            Some(patch.clone())
        )
        .await
        .success(StatusCode::OK),
        edited
    );
    let mut reused = body.clone();
    reused["details"]["title"] = json!("Ý định khác");
    h.json("POST", "/works", Some(ALICE), Some(reused))
        .await
        .error(StatusCode::CONFLICT, "operation_reused");
    let mut duplicate = body;
    duplicate["operation_id"] = json!(operation());
    h.json("POST", "/works", Some(ALICE), Some(duplicate))
        .await
        .error(StatusCode::CONFLICT, "catalog_conflict");
    let mut stale = patch;
    stale["operation_id"] = json!(operation());
    let error = h
        .json("PATCH", &format!("/works/{id}"), Some(ALICE), Some(stale))
        .await
        .error(StatusCode::CONFLICT, "stale_revision");
    assert_eq!(error["current_revision"], 2);
    assert_eq!(h.counts().await, before);
    let unit_body = unit_request(&edited["work"], "Chương được gửi lại");
    let unit_path = format!("/works/{id}/content-units");
    let unit = h
        .json("POST", &unit_path, Some(ALICE), Some(unit_body.clone()))
        .await
        .success(StatusCode::CREATED);
    let before = h.counts().await;
    assert_eq!(
        h.json("POST", &unit_path, Some(ALICE), Some(unit_body.clone()))
            .await
            .success(StatusCode::CREATED),
        unit
    );
    let mut duplicate = unit_body.clone();
    duplicate["operation_id"] = json!(operation());
    duplicate["expected_work_revision"] = json!(3);
    h.json("POST", &unit_path, Some(ALICE), Some(duplicate))
        .await
        .error(StatusCode::CONFLICT, "catalog_conflict");
    let mut reused = unit_body;
    reused["details"]["description"] = json!("Payload khác với cùng operation");
    h.json("POST", &unit_path, Some(ALICE), Some(reused))
        .await
        .error(StatusCode::CONFLICT, "operation_reused");
    let uid = unit["units"][0]["id"].as_str().unwrap();
    for (unit_revision, work_revision) in [(1, 2), (2, 3)] {
        h.json("PATCH", &format!("/content-units/{uid}"), Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":unit_revision,"expected_work_revision":work_revision,"details":{"title":"Bản đọc đã cũ"}}))).await.error(StatusCode::CONFLICT,"stale_revision");
    }
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
async fn http_authorizes_every_work_and_unit_route_and_keeps_owner_only_mutations_private() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let work = h.create(ALICE, "story").await;
    let created = h.unit(&work, "Một").await;
    let uid = created["units"][0]["id"].as_str().unwrap();
    let work = h.member(&created["work"], MemberRole::Reader).await;
    let wid = work["id"].as_str().unwrap();
    for path in [
        format!("/works/{wid}"),
        format!("/works/{wid}/history"),
        format!("/works/{wid}/content-units"),
        format!("/content-units/{uid}"),
    ] {
        h.json("GET", &path, Some(BOB), None)
            .await
            .success(StatusCode::OK);
        h.json("GET", &path, Some(CAROL), None)
            .await
            .error(StatusCode::NOT_FOUND, "not_found");
        h.json("GET", &path, None, None)
            .await
            .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let before = h.counts().await;
    h.json("PATCH", &format!("/works/{wid}"), Some(BOB), Some(json!({"operation_id": operation(), "expected_revision": work["revision"], "details":{"title":"Người đọc sửa"}}))).await.error(StatusCode::FORBIDDEN, "forbidden");
    h.json(
        "POST",
        &format!("/works/{wid}/content-units"),
        Some(BOB),
        Some(unit_request(&work, "Người đọc thêm")),
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
    for (method, path, body) in [
        (
            "PATCH",
            format!("/content-units/{uid}"),
            json!({"operation_id":operation(),"expected_revision":1,"expected_work_revision":work["revision"],"details":{"title":"Người đọc sửa chương"}}),
        ),
        (
            "DELETE",
            format!("/content-units/{uid}"),
            json!({"operation_id":operation(),"expected_revision":1,"expected_work_revision":work["revision"]}),
        ),
        (
            "DELETE",
            format!("/works/{wid}"),
            json!({"operation_id":operation(),"expected_revision":work["revision"]}),
        ),
        (
            "POST",
            format!("/works/{wid}/content-units/reorder"),
            json!({"operation_id":operation(),"expected_revision":work["revision"],"ordered":[uid]}),
        ),
    ] {
        h.json(method, &path, Some(BOB), Some(body.clone()))
            .await
            .error(StatusCode::FORBIDDEN, "forbidden");
        h.json(method, &path, Some(CAROL), Some(body.clone()))
            .await
            .error(StatusCode::NOT_FOUND, "not_found");
        h.json(method, &path, None, Some(body))
            .await
            .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    assert_eq!(h.counts().await, before);
    let work = h.member(&work, MemberRole::Editor).await;
    let edited = h.json("PATCH", &format!("/works/{wid}"), Some(BOB), Some(json!({"operation_id": operation(), "expected_revision":work["revision"], "details":{"title":"Biên tập viên sửa"}}))).await.success(StatusCode::OK);
    let work = &edited["work"];
    let before = h.counts().await;
    for details in [
        json!({"visibility":"public"}),
        json!({"rights_refs":["catalog-rights"]}),
    ] {
        h.json("PATCH", &format!("/works/{wid}"), Some(BOB), Some(json!({"operation_id":operation(), "expected_revision":work["revision"], "details":details}))).await.error(StatusCode::FORBIDDEN, "forbidden");
    }
    h.json(
        "DELETE",
        &format!("/works/{wid}"),
        Some(BOB),
        Some(json!({"operation_id":operation(),"expected_revision":work["revision"]})),
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
    h.json("DELETE", &format!("/content-units/{uid}"), Some(BOB), Some(json!({"operation_id":operation(),"expected_revision":1,"expected_work_revision":work["revision"]}))).await.error(StatusCode::FORBIDDEN, "forbidden");
    h.json("PATCH", &format!("/works/{wid}"), Some(CAROL), Some(json!({"operation_id":operation(),"expected_revision":work["revision"],"details":{"title":"Tài khoản khác"}}))).await.error(StatusCode::NOT_FOUND, "not_found");
    assert_eq!(h.counts().await, before);
    let mut forged = work_request("novel");
    forged["owner_id"] = json!("alice");
    h.json("POST", "/works", Some(CAROL), Some(forged))
        .await
        .error(StatusCode::BAD_REQUEST, "invalid_request");
    assert_eq!(h.create(CAROL, "podcast").await["owner_id"], "carol");
    let listed = h
        .json("GET", "/works", Some(CAROL), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed["items"][0]["owner_id"], "carol");
}

#[tokio::test]
async fn http_catalog_validates_fields_query_body_and_supported_representation_before_mutation() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let invalid = json!({"operation_id":"bad","id":"bad","kind":"audio","details":{"title":"  ","synopsis":"","language":"bad tag","visibility":"world","rights_refs":["bad ref","bad ref"]}});
    let error = h
        .json("POST", "/works", Some(ALICE), Some(invalid))
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_catalog");
    assert!(error["issues"].as_array().unwrap().len() >= 4, "{error}");
    let body = serde_json::to_vec(&work_request("story")).unwrap();
    request(
        h.app.clone(),
        "POST",
        "/works",
        Some(ALICE),
        body.clone(),
        ("application/msgpack", "application/json", ORIGIN),
    )
    .await
    .error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "invalid_request");
    request(
        h.app.clone(),
        "GET",
        "/works",
        Some(ALICE),
        vec![],
        ("application/json", "application/msgpack", ORIGIN),
    )
    .await
    .error(StatusCode::NOT_ACCEPTABLE, "invalid_request");
    request(
        h.app.clone(),
        "POST",
        "/works",
        Some(ALICE),
        body,
        (
            "application/json",
            "application/json",
            "https://not-cantos.invalid",
        ),
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
    request(
        h.app.clone(),
        "POST",
        "/works",
        Some(ALICE),
        vec![b' '; 128 * 1024 + 1],
        ("application/json", "application/json", ORIGIN),
    )
    .await
    .error(StatusCode::PAYLOAD_TOO_LARGE, "invalid_request");
    for query in [
        "limit=0",
        "limit=51",
        "limit=no",
        "cursor=xyz",
        "include_archived=no",
    ] {
        h.json("GET", &format!("/works?{query}"), Some(ALICE), None)
            .await
            .error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    h.json("GET", "/works/%FF", Some(ALICE), None)
        .await
        .error(StatusCode::BAD_REQUEST, "invalid_request");
    h.json("PUT", "/works", Some(ALICE), Some(json!({})))
        .await
        .error(StatusCode::METHOD_NOT_ALLOWED, "invalid_request");
    h.json("POST", "/works", None, Some(work_request("novel")))
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    assert_eq!(h.counts().await["operations"], 0);
}

#[tokio::test]
async fn http_pagination_is_bounded_deterministic_scoped_and_stale_after_catalog_changes() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut works = Vec::new();
    for _ in 0..3 {
        works.push(h.create(ALICE, "novel").await);
    }
    works.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let first = h
        .json("GET", "/works?limit=1", Some(ALICE), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(first["items"], json!([works[0]]));
    assert_eq!(
        h.json("GET", "/works?limit=1", Some(ALICE), None)
            .await
            .success(StatusCode::OK),
        first
    );
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = h
        .json(
            "GET",
            &format!("/works?limit=1&cursor={cursor}"),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(second["items"], json!([works[1]]));
    h.json(
        "GET",
        &format!("/works?limit=1&cursor={cursor}"),
        Some(BOB),
        None,
    )
    .await
    .error(StatusCode::BAD_REQUEST, "invalid_request");
    h.json(
        "GET",
        &format!("/works?limit=1&include_archived=true&cursor={cursor}"),
        Some(ALICE),
        None,
    )
    .await
    .error(StatusCode::BAD_REQUEST, "invalid_request");
    h.create(BOB, "podcast").await;
    h.json(
        "GET",
        &format!("/works?limit=1&cursor={cursor}"),
        Some(ALICE),
        None,
    )
    .await
    .error(StatusCode::CONFLICT, "stale_cursor");
    let mut work = works[0].clone();
    let mut units = Vec::new();
    for title in ["Một", "Hai", "Ba"] {
        let outcome = h.unit(&work, title).await;
        units.push(outcome["units"][0].clone());
        work = outcome["work"].clone();
    }
    let path = format!(
        "/works/{}/content-units?limit=1",
        work["id"].as_str().unwrap()
    );
    let first = h
        .json("GET", &path, Some(ALICE), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(first["items"], json!([units[0]]));
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = h
        .json("GET", &format!("{path}&cursor={cursor}"), Some(ALICE), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(second["items"], json!([units[1]]));
    h.unit(&work, "Bốn").await;
    h.json("GET", &format!("{path}&cursor={cursor}"), Some(ALICE), None)
        .await
        .error(StatusCode::CONFLICT, "stale_cursor");
}

#[tokio::test]
async fn http_archive_preserves_history_and_tombstone_slots_and_reorder_is_atomic() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create(ALICE, "story").await;
    let original = work.clone();
    let mut units = Vec::new();
    for title in ["Một", "Hai", "Ba"] {
        let outcome = h.unit(&work, title).await;
        units.push(outcome["units"][0].clone());
        work = outcome["work"].clone();
    }
    let wid = work["id"].as_str().unwrap().to_owned();
    let uid = units[1]["id"].as_str().unwrap();
    let archived = h.json("DELETE", &format!("/content-units/{uid}"), Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":1,"expected_work_revision":4}))).await.success(StatusCode::OK);
    assert_eq!(archived["units"][0]["lifecycle"], "archived");
    let path = format!("/works/{wid}/content-units/reorder");
    let before = h.counts().await;
    h.json("POST", &path, Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":5,"ordered":[units[0]["id"],units[0]["id"]]}))).await.error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_catalog");
    for ordered in [
        json!([units[0]["id"]]),
        json!([units[0]["id"], operation()]),
        json!([units[0]["id"], units[1]["id"], units[2]["id"]]),
    ] {
        h.json(
            "POST",
            &path,
            Some(ALICE),
            Some(json!({"operation_id":operation(),"expected_revision":5,"ordered":ordered})),
        )
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_catalog");
    }
    assert_eq!(h.counts().await, before);
    let reordered = h.json("POST", &path, Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":5,"ordered":[units[2]["id"],units[0]["id"]]}))).await.success(StatusCode::OK);
    assert_eq!(reordered["work"]["revision"], 6);
    let page = h
        .json(
            "GET",
            &format!("/works/{wid}/content-units?include_archived=true"),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(page["items"][0]["id"], units[2]["id"]);
    assert_eq!(page["items"][1]["id"], units[1]["id"]);
    assert_eq!(page["items"][1]["position"], 1);
    assert_eq!(page["items"][2]["id"], units[0]["id"]);
    assert_eq!(
        h.json(
            "GET",
            &format!("/works/{wid}/content-units"),
            Some(ALICE),
            None
        )
        .await
        .success(StatusCode::OK)["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let archived = h
        .json(
            "DELETE",
            &format!("/works/{wid}"),
            Some(ALICE),
            Some(json!({"operation_id":operation(),"expected_revision":6})),
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(archived["work"]["lifecycle"], "archived");
    assert_eq!(
        h.json("GET", &format!("/works/{wid}"), Some(ALICE), None)
            .await
            .success(StatusCode::OK),
        archived["work"]
    );
    assert_eq!(
        h.json("GET", "/works", Some(ALICE), None)
            .await
            .success(StatusCode::OK)["items"],
        json!([])
    );
    assert_eq!(
        h.json("GET", "/works?include_archived=true", Some(ALICE), None)
            .await
            .success(StatusCode::OK)["items"],
        json!([archived["work"]])
    );
    let history = h
        .json("GET", &format!("/works/{wid}/history"), Some(ALICE), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(history["items"].as_array().unwrap().len(), 7);
    assert_eq!(history["items"][0], original);
    assert_eq!(history["items"][6], archived["work"]);
    let before = h.counts().await;
    h.json("PATCH", &format!("/works/{wid}"), Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":7,"details":{"title":"Không thể sửa bản lưu"}}))).await.error(StatusCode::CONFLICT, "catalog_conflict");
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
async fn http_optional_links_pin_exact_owned_historical_bytes_and_patch_omission_differs_from_null()
{
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let source_id = "synthetic-source";
    let checksum = format!("{:x}", Sha256::digest(ORIGINAL.as_bytes()));
    h.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','source',?1,'Synthetic manuscript')", &[&source_id]).await.unwrap();
    h.admin.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice',?1,'synthetic://http-catalog',?2,?3,0)", &[&source_id,&ORIGINAL,&checksum]).await.unwrap();
    let script_id = operation();
    let saved = h
        .store
        .save(
            ALICE,
            &script_id,
            SaveRevisionRequest {
                operation_id: operation(),
                expected_revision: 0,
                script_json: SCRIPT.into(),
            },
        )
        .await
        .unwrap();
    let work = h.create(ALICE, "story").await;
    let mut body = unit_request(&work, "Liên kết phiên bản gốc");
    body["details"]["source"] = json!({"id":source_id,"sha256":checksum});
    body["details"]["script"] =
        json!({"id":script_id,"revision":1,"export_digest":saved.export_digest});
    body["details"]["content_digest"] = json!(saved.content_digest);
    let path = format!("/works/{}/content-units", work["id"].as_str().unwrap());
    let created = h
        .json("POST", &path, Some(ALICE), Some(body))
        .await
        .success(StatusCode::CREATED);
    let uid = created["units"][0]["id"].as_str().unwrap();
    let patch = json!({"operation_id":operation(),"expected_revision":1,"expected_work_revision":2,"details":{"title":"Chỉ sửa tên"}});
    h.store
        .save(
            ALICE,
            &script_id,
            SaveRevisionRequest {
                operation_id: operation(),
                expected_revision: 1,
                script_json: SCRIPT.into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        h.json("GET", &format!("/content-units/{uid}"), Some(ALICE), None)
            .await
            .success(StatusCode::OK)["details"]["script"],
        created["units"][0]["details"]["script"]
    );
    let edited = h
        .json(
            "PATCH",
            &format!("/content-units/{uid}"),
            Some(ALICE),
            Some(patch.clone()),
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(
        edited["units"][0]["details"]["source"],
        created["units"][0]["details"]["source"]
    );
    assert_eq!(
        edited["units"][0]["details"]["script"],
        created["units"][0]["details"]["script"]
    );
    let before = h.counts().await;
    let mut different = patch;
    different["details"]["source"] = Value::Null;
    h.json(
        "PATCH",
        &format!("/content-units/{uid}"),
        Some(ALICE),
        Some(different),
    )
    .await
    .error(StatusCode::CONFLICT, "operation_reused");
    assert_eq!(h.counts().await, before);
    let cleared = h.json("PATCH",&format!("/content-units/{uid}"),Some(ALICE),Some(json!({"operation_id":operation(),"expected_revision":2,"expected_work_revision":3,"details":{"source":null,"script":null,"content_digest":null}}))).await.success(StatusCode::OK);
    assert_eq!(cleared["units"][0]["details"]["source"], Value::Null);
    let other = h.create(BOB, "podcast").await;
    let before = h.counts().await;
    for link in [
        json!({"source":{"id":source_id,"sha256":checksum}}),
        json!({"script":{"id":script_id,"revision":1,"export_digest":saved.export_digest}}),
    ] {
        let mut foreign = unit_request(&other, "Tài liệu tài khoản khác");
        for (key, value) in link.as_object().unwrap() {
            foreign["details"][key] = value.clone();
        }
        h.json(
            "POST",
            &format!("/works/{}/content-units", other["id"].as_str().unwrap()),
            Some(BOB),
            Some(foreign),
        )
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "evidence_unavailable");
    }
    for details in [
        json!({"source":{"id":source_id,"sha256":"0".repeat(64)}}),
        json!({"script":{"id":script_id,"revision":1,"export_digest":format!("sir-e1:sha256:{}","0".repeat(64))}}),
        json!({"source":{"id":source_id,"sha256":checksum},"content_digest":"0".repeat(64)}),
    ] {
        h.json("PATCH", &format!("/content-units/{uid}"), Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":3,"expected_work_revision":4,"details":details}))).await.error(StatusCode::UNPROCESSABLE_ENTITY,"evidence_unavailable");
    }
    assert_eq!(h.counts().await, before);
    let source: (String, String) = {
        let row = h
            .admin
            .query_one(
                "SELECT original_text,sha256 FROM source_records WHERE owner_id='alice' AND id=?1",
                &[&source_id],
            )
            .await
            .unwrap();
        (row.get(0), row.get(1))
    };
    assert_eq!(source, (ORIGINAL.into(), checksum));
    let row = h.admin.query_one("SELECT canonical_export,content_digest,export_digest FROM script_revisions WHERE script_id=?1 AND revision=1", &[&script_id]).await.unwrap();
    assert_eq!(row.get::<_, Vec<u8>>(0), saved.script_json.as_bytes());
    assert_eq!(row.get::<_, String>(1), saved.content_digest);
    assert_eq!(row.get::<_, String>(2), saved.export_digest);
}

fn content_request(expected: u64, work: u64, body: Value) -> Value {
    json!({"operation_id":operation(),"id":operation(),"expected_revision":expected,"expected_work_revision":work,
        "details":{"language":"vi-VN","body":body,"rights_refs":[],"provenance_note":"Synthetic authored/imported/recorded test content; no publication clearance"}})
}

#[tokio::test]
async fn content_categories_allow_text_and_recordings_without_script_ir_and_keep_metadata_separate()
{
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    for kind in ["novel", "story", "podcast"] {
        let w = h.create(ALICE, kind).await;
        let catalog = h.unit(&w, "Chương hoặc tập thử nghiệm").await;
        let uid = catalog["units"][0]["id"].as_str().unwrap();
        let path = format!("/content-units/{uid}/content/revisions");
        let body = if kind == "podcast" {
            json!({"kind":"recording"})
        } else {
            json!({"kind":"text","text":"Người kể chuyện\r\nÁnh đèn.","source":null})
        };
        let submitted = content_request(0, 2, body);
        let original_operation = submitted["operation_id"].clone();
        let first = h
            .json("POST", &path, Some(ALICE), Some(submitted.clone()))
            .await
            .success(StatusCode::CREATED);
        assert_eq!(first["content"]["revision"], 1);
        assert_eq!(first["work_revision"], 3);
        assert_eq!(first["content"]["details"]["body"]["script"], Value::Null);
        if kind != "podcast" {
            assert_eq!(
                first["content"]["details"]["body"]["text"],
                "Người kể chuyện\nÁnh đèn."
            );
        }
        let pinned = format!("{path}/{}", first["content"]["id"].as_str().unwrap());
        assert_eq!(
            h.json("GET", &pinned, Some(ALICE), None)
                .await
                .success(StatusCode::OK),
            first["content"]
        );
        // Metadata edits and legacy link changes have no authority over managed content.
        let patch = json!({"operation_id":operation(),"expected_revision":1,"expected_work_revision":3,"details":{"title":"Tên đã sửa","content_digest":"0".repeat(64)}});
        let metadata = h
            .json(
                "PATCH",
                &format!("/content-units/{uid}"),
                Some(ALICE),
                Some(patch),
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(metadata["units"][0]["revision"], 2);
        assert_eq!(
            h.json(
                "GET",
                &format!("/content-units/{uid}/content/head"),
                Some(ALICE),
                None
            )
            .await
            .success(StatusCode::OK),
            first["content"]
        );
        let mut next = content_request(1, 4, first["content"]["details"]["body"].clone());
        next["details"]["provenance_note"] = json!("Corrected caller attribution");
        let second = h
            .json("POST", &path, Some(ALICE), Some(next))
            .await
            .success(StatusCode::CREATED);
        assert_eq!(
            second["content"]["content_digest"],
            first["content"]["content_digest"]
        );
        assert_ne!(
            second["content"]["revision_digest"],
            first["content"]["revision_digest"]
        );
        assert_ne!(second["content"]["id"], first["content"]["id"]);
        assert_eq!(second["content"]["unit_id"], first["content"]["unit_id"]);
        let before = h.counts().await;
        assert_eq!(
            h.json("POST", &path, Some(ALICE), Some(submitted.clone()))
                .await
                .success(StatusCode::CREATED),
            first
        );
        let mut changed = submitted;
        changed["details"]["provenance_note"] = json!("Different intent under old ID");
        h.json("POST", &path, Some(ALICE), Some(changed))
            .await
            .error(StatusCode::CONFLICT, "operation_reused");
        // A content operation cannot be reused by the old Catalog API either.
        h.json("PATCH",&format!("/content-units/{uid}"),Some(ALICE),Some(json!({"operation_id":original_operation,"expected_revision":1,"expected_work_revision":2,"details":{"title":"Stale metadata"}}))).await.error(StatusCode::CONFLICT,"operation_reused");
        assert_eq!(h.counts().await, before);
        let history = h
            .json(
                "GET",
                &format!("/content-units/{uid}/content/history?limit=1"),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(history["revisions"][0]["id"], first["content"]["id"]);
        assert_eq!(history["next_after"], 1);
        assert_eq!(history["revisions"][0]["details"], Value::Null);
        let page = h
            .json(
                "GET",
                &format!("/content-units/{uid}/content/history?after_revision=1&limit=1"),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(page["revisions"][0]["id"], second["content"]["id"]);
        assert_eq!(page["next_after"], Value::Null);
        h.json("DELETE",&format!("/content-units/{uid}"),Some(ALICE),Some(json!({"operation_id":operation(),"expected_revision":2,"expected_work_revision":5}))).await.success(StatusCode::OK);
        let before = h.counts().await;
        h.json(
            "POST",
            &path,
            Some(ALICE),
            Some(content_request(2, 6, json!({"kind":"recording"}))),
        )
        .await
        .error(StatusCode::CONFLICT, "catalog_conflict");
        assert_eq!(h.counts().await, before);
        assert_eq!(
            h.json("GET", &pinned, Some(ALICE), None)
                .await
                .success(StatusCode::OK),
            first["content"]
        );
    }
}

#[tokio::test]
async fn content_authorization_replays_and_validation_never_bypass_current_access() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let w = h.create(ALICE, "podcast").await;
    let catalog = h.unit(&w, "Tập thử nghiệm").await;
    let uid = catalog["units"][0]["id"].as_str().unwrap();
    let path = format!("/content-units/{uid}/content/revisions");
    h.member(&catalog["work"], MemberRole::Editor).await;
    let submitted = content_request(0, 3, json!({"kind":"recording"}));
    let first = h
        .json("POST", &path, Some(BOB), Some(submitted.clone()))
        .await
        .success(StatusCode::CREATED);
    let before = h.counts().await;
    let mut rights = content_request(1, 4, json!({"kind":"recording"}));
    rights["details"]["rights_refs"] = json!(["catalog-rights"]);
    h.json("POST", &path, Some(BOB), Some(rights))
        .await
        .error(StatusCode::FORBIDDEN, "forbidden");
    h.json(
        "POST",
        &path,
        Some(CAROL),
        Some(content_request(1, 4, json!({"kind":"recording"}))),
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    for (key, value, code) in [
        ("owner_id", json!("bob"), "invalid_request"),
        (
            "details",
            json!({"language":"vi-VN","body":{"kind":"text","text":" \t\n"},"rights_refs":[],"provenance_note":"Synthetic"}),
            "invalid_catalog",
        ),
    ] {
        let mut invalid = content_request(1, 4, json!({"kind":"recording"}));
        invalid[key] = value;
        h.json("POST", &path, Some(ALICE), Some(invalid))
            .await
            .error(
                if code == "invalid_request" {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::UNPROCESSABLE_ENTITY
                },
                code,
            );
    }
    assert_eq!(h.counts().await, before);
    let current = h
        .json(
            "GET",
            &format!("/works/{}", w["id"].as_str().unwrap()),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    h.member(&current, MemberRole::Reader).await;
    assert_eq!(
        h.json(
            "GET",
            &format!("/content-units/{uid}/content/head"),
            Some(BOB),
            None
        )
        .await
        .success(StatusCode::OK),
        first["content"]
    );
    let before = h.counts().await;
    h.json("POST", &path, Some(BOB), Some(submitted))
        .await
        .error(StatusCode::FORBIDDEN, "forbidden");
    assert_eq!(h.counts().await, before);
    h.admin
        .execute(
            "DELETE FROM catalog_members WHERE work_id=?1 AND actor_id='bob'",
            &[&w["id"].as_str().unwrap()],
        )
        .await
        .unwrap();
    h.json(
        "GET",
        &format!("/content-units/{uid}/content/head"),
        Some(BOB),
        None,
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    h.json(
        "GET",
        &format!("/content-units/{uid}/content/head"),
        None,
        None,
    )
    .await
    .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    // Boundaries inherited from Catalog apply to every Content route.
    request(
        h.app.clone(),
        "POST",
        &path,
        Some(ALICE),
        vec![b' '; 128 * 1024 + 1],
        ("application/json", "application/json", ORIGIN),
    )
    .await
    .error(StatusCode::PAYLOAD_TOO_LARGE, "invalid_request");
    request(
        h.app.clone(),
        "POST",
        &path,
        Some(ALICE),
        vec![],
        ("application/msgpack", "application/json", ORIGIN),
    )
    .await
    .error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "invalid_request");
    request(
        h.app.clone(),
        "GET",
        &format!("/content-units/{uid}/content/head"),
        Some(ALICE),
        vec![],
        ("application/json", "application/msgpack", ORIGIN),
    )
    .await
    .error(StatusCode::NOT_ACCEPTABLE, "invalid_request");
    request(
        h.app.clone(),
        "POST",
        &path,
        Some(ALICE),
        serde_json::to_vec(&content_request(1, 5, json!({"kind":"recording"}))).unwrap(),
        (
            "application/json",
            "application/json",
            "https://untrusted.example",
        ),
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
}

#[tokio::test]
async fn content_links_retain_authoritative_sources_and_exact_owned_script_exports() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let checksum = format!("{:x}", Sha256::digest(ORIGINAL.as_bytes()));
    h.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','source','content-source','Synthetic source for Content Identity')",&[]).await.unwrap();
    h.admin.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice','content-source','synthetic://content',?1,?2,0)",&[&ORIGINAL,&checksum]).await.unwrap();
    let sid = operation();
    let script = h
        .store
        .save(
            ALICE,
            &sid,
            SaveRevisionRequest {
                operation_id: operation(),
                expected_revision: 0,
                script_json: SCRIPT.into(),
            },
        )
        .await
        .unwrap();
    let w = h.create(ALICE, "story").await;
    let catalog = h.unit(&w, "Chương có nguồn").await;
    let uid = catalog["units"][0]["id"].as_str().unwrap();
    let path = format!("/content-units/{uid}/content/revisions");
    let source = json!({"id":"content-source","sha256":checksum});
    let reference = json!({"id":sid,"revision":1,"export_digest":script.export_digest});
    let submitted = content_request(
        0,
        2,
        json!({"kind":"script","script":reference,"source":source}),
    );
    let first = h
        .json("POST", &path, Some(ALICE), Some(submitted.clone()))
        .await
        .success(StatusCode::CREATED);
    let mut changed: Value = serde_json::from_str(SCRIPT).unwrap();
    changed["episode"]["title"] = json!("Tên khác");
    h.store
        .save(
            ALICE,
            &sid,
            SaveRevisionRequest {
                operation_id: operation(),
                expected_revision: 1,
                script_json: serde_json::to_string(&changed).unwrap(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        h.json(
            "GET",
            &format!("/content-units/{uid}/content/head"),
            Some(ALICE),
            None
        )
        .await
        .success(StatusCode::OK),
        first["content"]
    );
    let before = h.counts().await;
    for body in [
        json!({"kind":"source","source":{"id":"content-source","sha256":"0".repeat(64)}}),
        json!({"kind":"script","script":{"id":sid,"revision":1,"export_digest":format!("sir-e1:sha256:{}","0".repeat(64))}}),
    ] {
        h.json(
            "POST",
            &path,
            Some(ALICE),
            Some(content_request(1, 3, body)),
        )
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "evidence_unavailable");
    }
    assert_eq!(h.counts().await, before);
    let other = h.create(BOB, "podcast").await;
    let unit = h
        .json(
            "POST",
            &format!("/works/{}/content-units", other["id"].as_str().unwrap()),
            Some(BOB),
            Some(unit_request(&other, "Tài khoản khác")),
        )
        .await
        .success(StatusCode::CREATED);
    let other_path = format!(
        "/content-units/{}/content/revisions",
        unit["units"][0]["id"].as_str().unwrap()
    );
    for body in [
        json!({"kind":"source","source":source}),
        json!({"kind":"script","script":reference}),
    ] {
        h.json(
            "POST",
            &other_path,
            Some(BOB),
            Some(content_request(0, 2, body)),
        )
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "evidence_unavailable");
    }
    // A valid revision ID under a different unit must reveal nothing.
    h.json(
        "GET",
        &format!("{other_path}/{}", first["content"]["id"].as_str().unwrap()),
        Some(BOB),
        None,
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    let row = h.admin.query_one("SELECT original_text,sha256 FROM source_records WHERE owner_id='alice' AND id='content-source'",&[]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), ORIGINAL);
    assert_eq!(row.get::<_, String>(1), checksum);
    let row = h.admin.query_one("SELECT canonical_export,export_digest FROM script_revisions WHERE script_id=?1 AND revision=1",&[&sid]).await.unwrap();
    assert_eq!(row.get::<_, Vec<u8>>(0), script.script_json.as_bytes());
    assert_eq!(row.get::<_, String>(1), script.export_digest);
}

#[path = "support/audio_lifecycle_http_turso.rs"]
mod audio_lifecycle_http;
#[path = "support/brief_http_turso.rs"]
mod brief_http;

#[tokio::test]
async fn http_waiting_mutation_rechecks_session_and_membership_at_transaction_boundary() {
    let _media_lane = native_sql::media_lane().await;
    for expire in [false, true] {
        let clock = cantos_store_turso::testing::FixedClock::new(1_800_000_000_000_000);
        let h = Harness::with_fixture(NativeSql(
            cantos_server::test_support::ServerFixture::with_clock(clock.clone()).await,
        ))
        .await;
        let initial = h.create(ALICE, "story").await;
        let work = h.member(&initial, MemberRole::Editor).await;
        let path = format!("/works/{}", work["id"].as_str().unwrap());
        let before = h.counts().await;
        let blocker = h.admin.0.writer().await;
        let mut waiting=Box::pin(h.json("PATCH",&path,Some(BOB),Some(json!({"operation_id":operation(),"expected_revision":2,"details":{"title":"Sau khi mất quyền"}}))));
        assert!(futures_util::poll!(&mut waiting).is_pending());
        if expire {
            clock.advance(3_600_000_001);
        } else {
            blocker
                .execute("DELETE FROM catalog_members WHERE actor_id='bob'", ())
                .await
                .unwrap();
        }
        blocker.commit().await.unwrap();
        waiting.await.error(
            if expire {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::NOT_FOUND
            },
            if expire {
                "unauthenticated"
            } else {
                "not_found"
            },
        );
        let mut expected = before;
        if !expire {
            expected["members"] = json!(0);
        }
        assert_eq!(h.counts().await, expected);
    }
}
#[tokio::test]
async fn concurrent_http_reorders_have_one_winner_with_exact_revision_and_no_partial_commit() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let w = h.create(ALICE, "story").await;
    let a = h.unit(&w, "Một").await;
    let b = h.unit(&a["work"], "Hai").await;
    let wid = w["id"].as_str().unwrap();
    let path = format!("/works/{wid}/content-units/reorder");
    let ordered = json!([b["units"][0]["id"], a["units"][0]["id"]]);
    let request =
        |order: Value| json!({"operation_id":operation(),"expected_revision":3,"ordered":order});
    let blocker = h.admin.0.writer().await;
    let mut first = Box::pin(h.json("POST", &path, Some(ALICE), Some(request(ordered.clone()))));
    let mut second = Box::pin(h.json("POST", &path, Some(ALICE), Some(request(ordered))));
    assert!(futures_util::poll!(&mut first).is_pending());
    assert!(futures_util::poll!(&mut second).is_pending());
    blocker.commit().await.unwrap();
    let (a, b) = tokio::join!(first, second);
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|s| **s == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1
    );
    assert_eq!(
        h.json("GET", &format!("/works/{wid}"), Some(ALICE), None)
            .await
            .success(StatusCode::OK)["revision"],
        4
    );
    assert_eq!(h.counts().await["unit_revisions"], 4);
}
#[tokio::test]
async fn content_concurrent_append_rollback_and_database_immutability_are_real_constraints() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let w = h.create(ALICE, "story").await;
    let u = h.unit(&w, "Một").await;
    let uid = u["units"][0]["id"].as_str().unwrap();
    let path = format!("/content-units/{uid}/content/revisions");
    h.admin.execute("CREATE TRIGGER content_fault BEFORE INSERT ON catalog_content_heads BEGIN SELECT RAISE(ABORT,'synthetic fault'); END",&[]).await.unwrap();
    let body = content_request(0, 2, json!({"kind":"text","text":"Bản thử","source":null}));
    h.json("POST", &path, Some(ALICE), Some(body.clone()))
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    assert_eq!(h.counts().await["content_revisions"], 0);
    assert_eq!(h.counts().await["work_revisions"], 2);
    h.admin
        .execute("DROP TRIGGER content_fault", &[])
        .await
        .unwrap();
    let blocker = h.admin.0.writer().await;
    let mut a = Box::pin(h.json("POST", &path, Some(ALICE), Some(body)));
    let mut b = Box::pin(h.json(
        "POST",
        &path,
        Some(ALICE),
        Some(content_request(
            0,
            2,
            json!({"kind":"text","text":"Bản khác","source":null}),
        )),
    ));
    assert!(futures_util::poll!(&mut a).is_pending());
    assert!(futures_util::poll!(&mut b).is_pending());
    blocker.commit().await.unwrap();
    let (a, b) = tokio::join!(a, b);
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|s| **s == StatusCode::CREATED)
            .count(),
        1
    );
    let rejected = if a.status == StatusCode::CONFLICT {
        a
    } else {
        b
    };
    rejected.error(StatusCode::CONFLICT, "stale_revision");
    assert_eq!(h.counts().await["content_revisions"], 1);
    assert_eq!(h.counts().await["content_heads"], 1);
    for sql in [
        "UPDATE catalog_content_revisions SET snapshot=snapshot",
        "DELETE FROM catalog_content_revisions",
        "INSERT OR REPLACE INTO catalog_content_revisions SELECT * FROM catalog_content_revisions",
    ] {
        assert!(h
            .admin
            .0
            .try_exec(sql, &[])
            .await
            .unwrap_err()
            .guard()
            .is_some());
    }
    h.admin
        .execute("DROP TRIGGER catalog_content_revisions_no_update", &[])
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE catalog_content_revisions SET snapshot=?1",
            &[&b"{}".to_vec()],
        )
        .await
        .unwrap();
    h.json(
        "GET",
        &format!("/content-units/{uid}/content/head"),
        Some(ALICE),
        None,
    )
    .await
    .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
}
#[tokio::test]
async fn committed_http_catalog_preserves_exact_native_restart_receipt() {
    let _media_lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
    let w = h.create(ALICE, "story").await;
    let u = h.unit(&w, "Giữ nguyên").await;
    let uid = u["units"][0]["id"].as_str().unwrap();
    let path = format!("/content-units/{uid}/content/revisions");
    let intent = content_request(
        0,
        2,
        json!({"kind":"text","text":"Người kể đã về.","source":null}),
    );
    let saved = h
        .json("POST", &path, Some(ALICE), Some(intent.clone()))
        .await
        .success(StatusCode::CREATED);
    let before = h.counts().await;
    let root = std::fs::canonicalize(h.admin.0.dir())
        .unwrap()
        .join("private-audio");
    h.restart_with_audio(&root).await;
    assert_eq!(
        h.json("POST", &path, Some(ALICE), Some(intent))
            .await
            .success(StatusCode::CREATED),
        saved
    );
    assert_eq!(
        h.json(
            "GET",
            &format!("/content-units/{uid}/content/head"),
            Some(ALICE),
            None
        )
        .await
        .success(StatusCode::OK),
        saved["content"]
    );
    assert_eq!(h.counts().await, before);
}
#[tokio::test]
async fn bounded_content_head_and_history_use_native_indexes_and_preserve_snapshots() {
    let _lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let work = h.create(ALICE, "story").await;
    let unit = h.unit(&work, "Chương đo cục bộ").await;
    let id = unit["units"][0]["id"].as_str().unwrap();
    let path = format!("/content-units/{id}/content/revisions");
    let mut last = Value::Null;
    for revision in 0..100 {
        last=h.json("POST",&path,Some(ALICE),Some(content_request(revision,revision+2,json!({"kind":"text","text":format!("Bản thử nghiệm {revision}"),"source":null})))).await.success(StatusCode::CREATED);
    }
    let mut samples = Vec::new();
    for i in 0..35 {
        let began = Instant::now();
        assert_eq!(
            h.json(
                "GET",
                &format!("/content-units/{id}/content/head"),
                Some(ALICE),
                None
            )
            .await
            .success(StatusCode::OK),
            last["content"]
        );
        let page = h
            .json(
                "GET",
                &format!("/content-units/{id}/content/history?after_revision=50&limit=20"),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(page["revisions"].as_array().unwrap().len(), 20);
        assert_eq!(page["next_after"], 70);
        if i >= 5 {
            samples.push(began.elapsed().as_secs_f64() * 1000.);
        }
    }
    for sql in ["EXPLAIN QUERY PLAN SELECT * FROM catalog_content_heads WHERE unit_id=?1","EXPLAIN QUERY PLAN SELECT * FROM catalog_content_revisions WHERE unit_id=?1 AND revision>50 ORDER BY revision LIMIT 20"] {let plan:Vec<String>=h.admin.query(sql,&[&id]).await.unwrap().iter().map(|r|r.get(3)).collect();assert!(plan.iter().any(|p|p.contains("SEARCH")&&p.contains("INDEX")),"{plan:?}");}
    samples.sort_by(f64::total_cmp);
    assert!(samples[28] < 500., "{samples:?}");
    eprintln!("Turso0.8.3 100 actual revisions;5 warmup/30 authorized head/history pairs ms={samples:?};warm synthetic Router only");
}
