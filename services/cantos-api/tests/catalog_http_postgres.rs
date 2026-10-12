//! Real Studio HTTP → application → PostgreSQL catalog evidence, enabled only by xtask's
//! disposable-cluster runner. Fixtures are synthetic Vietnamese metadata, never story data.
use std::{
    env, fs,
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_api::{
    contexts::catalog::{domain::WorkId, CatalogCommand, MemberRole},
    interfaces::http::{router, AppState},
    platform::postgres::{local_config, migrate, token_hash, Store},
    script_ir::read_script,
};
use cantos_contracts::SaveRevisionRequest;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_postgres::{Client, Config, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CAROL: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const ORIGIN: &str = "http://127.0.0.1:8080";
const EVIDENCE: &str = "../../target/catalog-evidence";
const SCRIPT: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const ORIGINAL: &str = "\u{feff}Ánh đèn cuối sân khấu\r\nNgười kể chuyện đã trở về.\r\n";

struct Harness {
    admin: Client,
    config: Config,
    store: Store,
    app: Router,
    database: String,
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
    let root: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/catalog/v1.schema.json"
    ))
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
    assert_eq!(response.headers()["content-type"], "application/json");
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let definition = if !status.is_success() {
        "ApiError"
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

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

impl Harness {
    async fn new() -> Self {
        let mut config = local_config(
            &env::var("CANTOS_TEST_CLUSTER_URL").expect("use cargo xtask verify postgres"),
        )
        .unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let database = format!("cantos_test_{}", Uuid::new_v4().simple());
        connect(&config)
            .await
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await
            .unwrap();
        config.dbname(&database);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin.batch_execute(
            "GRANT USAGE ON SCHEMA public TO cantos_app;
             GRANT SELECT ON actors,sessions,scripts,script_revisions,script_members,script_evidence,source_records,revision_evidence,
             catalog_clock,catalog_works,catalog_work_revisions,catalog_units,catalog_unit_revisions,catalog_members,catalog_operations TO cantos_app;
             GRANT INSERT ON scripts,script_revisions,revision_evidence,catalog_works,catalog_work_revisions,catalog_units,catalog_unit_revisions,catalog_members,catalog_operations TO cantos_app;
             GRANT UPDATE(head_revision) ON scripts TO cantos_app;
             GRANT UPDATE(revoked) ON sessions TO cantos_app;
             GRANT UPDATE(revision) ON catalog_clock TO cantos_app;
             GRANT UPDATE(head_revision,lifecycle) ON catalog_works TO cantos_app;
             GRANT UPDATE(head_revision,position,lifecycle) ON catalog_units TO cantos_app;
             GRANT UPDATE(role),DELETE ON catalog_members TO cantos_app;
             GRANT EXECUTE ON FUNCTION catalog_lock_actor(text),catalog_lock_member(text,text) TO cantos_app;
             INSERT INTO actors(id) VALUES('alice'),('bob'),('carol');",
        ).await.unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB), ("carol", CAROL)] {
            admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,clock_timestamp()+interval '1 hour')", &[&token_hash(token), &actor]).await.unwrap();
            for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
                admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,$2,$3,'Synthetic test reference; no publication clearance')", &[&actor,&kind,&id]).await.unwrap();
            }
            admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,'rights','catalog-rights','Synthetic catalog rights reference')", &[&actor]).await.unwrap();
        }
        Self::from_connections(config, database, admin)
    }

    fn from_connections(config: Config, database: String, admin: Client) -> Self {
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-catalog-http-test");
        let store = Store::new(app_config).unwrap();
        let app = router(
            AppState {
                store: store.clone(),
                web_origin: ORIGIN.into(),
            },
            "apps/cantos-studio/web/dist",
        );
        Self {
            admin,
            config,
            store,
            app,
            database,
        }
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
        let text: String = self.admin.query_one("SELECT json_build_object('works',(SELECT count(*) FROM catalog_works),'work_revisions',(SELECT count(*) FROM catalog_work_revisions),'units',(SELECT count(*) FROM catalog_units),'unit_revisions',(SELECT count(*) FROM catalog_unit_revisions),'members',(SELECT count(*) FROM catalog_members),'operations',(SELECT count(*) FROM catalog_operations),'clock',(SELECT revision FROM catalog_clock))::text", &[]).await.unwrap().get(0);
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
                    actor_id: cantos_api::contexts::catalog::domain::OwnerId::parse("bob").unwrap(),
                    role: Some(role),
                },
            )
            .await
            .unwrap();
        serde_json::to_value(outcome.work).unwrap()
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_catalog_creates_all_categories_without_script_cast_and_rejects_wrong_unit_kind() {
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_retry_returns_original_snapshot_duplicate_ids_and_changed_intents_do_not_commit() {
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_authorizes_every_work_and_unit_route_and_keeps_owner_only_mutations_private() {
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_catalog_validates_fields_query_body_and_supported_representation_before_mutation() {
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_pagination_is_bounded_deterministic_scoped_and_stale_after_catalog_changes() {
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_archive_preserves_history_and_tombstone_slots_and_reorder_is_atomic() {
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

async fn blocked(admin: &Client, count: i64) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let waiting: i64 = admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name='cantos-catalog-http-test' AND wait_event_type='Lock'", &[]).await.unwrap().get(0);
        if waiting >= count {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "HTTP operation did not reach the lock barrier"
        );
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_http_reorders_have_one_winner_with_exact_revision_and_no_partial_commit() {
    let h = Harness::new().await;
    let work = h.create(ALICE, "novel").await;
    let first = h.unit(&work, "Một").await;
    let second = h.unit(&first["work"], "Hai").await;
    let a = first["units"][0]["id"].clone();
    let b = second["units"][0]["id"].clone();
    let wid = work["id"].as_str().unwrap();
    let path = format!("/works/{wid}/content-units/reorder");
    let mut blocker = connect(&h.config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one(
        "SELECT revision FROM catalog_clock WHERE singleton FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    let mut handles = Vec::new();
    for (index, ordered) in [json!([b, a]), json!([a, b])].into_iter().enumerate() {
        let app = h.app.clone();
        let path = path.clone();
        let body = serde_json::to_vec(
            &json!({"operation_id":operation(),"expected_revision":3,"ordered":ordered}),
        )
        .unwrap();
        handles.push(tokio::spawn(async move {
            request(
                app,
                "POST",
                &path,
                Some(ALICE),
                body,
                ("application/json", "application/json", ORIGIN),
            )
            .await
        }));
        blocked(&h.admin, index as i64 + 1).await;
    }
    tx.commit().await.unwrap();
    let results = [
        handles.remove(0).await.unwrap(),
        handles.remove(0).await.unwrap(),
    ];
    assert_eq!(
        results
            .iter()
            .filter(|reply| reply.status == StatusCode::OK)
            .count(),
        1
    );
    let conflict = results
        .iter()
        .find(|reply| reply.status == StatusCode::CONFLICT)
        .unwrap();
    assert_eq!(conflict.body["code"], "stale_revision");
    assert_eq!(conflict.body["current_revision"], 4);
    let page = h
        .json(
            "GET",
            &format!("/works/{wid}/content-units"),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    let ids = json!([page["items"][0]["id"], page["items"][1]["id"]]);
    assert!(ids == json!([a, b]) || ids == json!([b, a]));
    assert_eq!(page["items"][0]["position"], 0);
    assert_eq!(page["items"][1]["position"], 1);
    assert_eq!(h.counts().await["operations"], 4);
    assert_eq!(h.counts().await["work_revisions"], 4);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_waiting_mutation_rechecks_session_and_membership_at_transaction_boundary() {
    for expire in [false, true] {
        let h = Harness::new().await;
        let initial = h.create(ALICE, "story").await;
        let work = h.member(&initial, MemberRole::Editor).await;
        let wid = work["id"].as_str().unwrap();
        let before = h.counts().await;
        let mut blocker = connect(&h.config).await;
        let tx = blocker.transaction().await.unwrap();
        tx.query_one(
            "SELECT revision FROM catalog_clock WHERE singleton FOR UPDATE",
            &[],
        )
        .await
        .unwrap();
        if expire {
            h.admin.execute("UPDATE sessions SET expires_at=clock_timestamp()+interval '200 milliseconds' WHERE actor_id='bob'", &[]).await.unwrap();
        }
        let app = h.app.clone();
        let path = format!("/works/{wid}");
        let body = serde_json::to_vec(&json!({"operation_id":operation(),"expected_revision":2,"details":{"title":"Sau khi mất quyền"}})).unwrap();
        let waiting = tokio::spawn(async move {
            request(
                app,
                "PATCH",
                &path,
                Some(BOB),
                body,
                ("application/json", "application/json", ORIGIN),
            )
            .await
        });
        blocked(&h.admin, 1).await;
        if expire {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !h
                .admin
                .query_one(
                    "SELECT expires_at<=clock_timestamp() FROM sessions WHERE actor_id='bob'",
                    &[],
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
            {
                assert!(
                    Instant::now() < deadline,
                    "session did not expire at the clock barrier"
                );
                tokio::task::yield_now().await;
            }
        } else {
            tx.execute(
                "DELETE FROM catalog_members WHERE work_id=$1 AND actor_id='bob'",
                &[&wid],
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();
        waiting.await.unwrap().error(
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
        assert_eq!(
            h.json("GET", &format!("/works/{wid}"), Some(ALICE), None)
                .await
                .success(StatusCode::OK),
            work
        );
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn http_optional_links_pin_exact_owned_historical_bytes_and_patch_omission_differs_from_null()
{
    let h = Harness::new().await;
    let source_id = "synthetic-source";
    let checksum = format!("{:x}", Sha256::digest(ORIGINAL.as_bytes()));
    h.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','source',$1,'Synthetic manuscript')", &[&source_id]).await.unwrap();
    h.admin.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256) VALUES('alice',$1,'synthetic://http-catalog',$2,$3)", &[&source_id,&ORIGINAL,&checksum]).await.unwrap();
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
                "SELECT original_text,sha256 FROM source_records WHERE owner_id='alice' AND id=$1",
                &[&source_id],
            )
            .await
            .unwrap();
        (row.get(0), row.get(1))
    };
    assert_eq!(source, (ORIGINAL.into(), checksum));
    let row = h.admin.query_one("SELECT canonical_export,content_digest,export_digest FROM script_revisions WHERE script_id=$1 AND revision=1", &[&script_id]).await.unwrap();
    assert_eq!(row.get::<_, Vec<u8>>(0), saved.script_json.as_bytes());
    assert_eq!(row.get::<_, String>(1), saved.content_digest);
    assert_eq!(row.get::<_, String>(2), saved.export_digest);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn committed_http_catalog_emits_exact_postgres_restart_oracle() {
    let h = Harness::new().await;
    let create_body = work_request("podcast");
    let original = h
        .json("POST", "/works", Some(ALICE), Some(create_body.clone()))
        .await
        .success(StatusCode::CREATED);
    let wid = original["work"]["id"].as_str().unwrap();
    let unit_body = unit_request(&original["work"], "Tập vẫn còn nguyên sau khởi động lại");
    let unit_path = format!("/works/{wid}/content-units");
    let current = h
        .json("POST", &unit_path, Some(ALICE), Some(unit_body.clone()))
        .await
        .success(StatusCode::CREATED);
    let uid = current["units"][0]["id"].as_str().unwrap();
    fs::create_dir_all(EVIDENCE).unwrap();
    fs::write(format!("{EVIDENCE}/http-restart.json"),serde_json::to_vec_pretty(&json!({
        "database":h.database,"create_body":create_body,"unit_body":unit_body,
        "work_id":wid,"unit_id":uid,"original":original,"current":current,"counts":h.counts().await,
        "provenance":"Real Router/application/app-role PostgreSQL; synthetic metadata; no provider or media"
    })).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "runner enables this exact test only after its PostgreSQL restart"]
async fn catalog_http_recovery_after_server_restart() {
    let fixture: Value =
        serde_json::from_slice(&fs::read(format!("{EVIDENCE}/http-restart.json")).unwrap())
            .unwrap();
    let database = fixture["database"].as_str().unwrap();
    assert!(
        database.starts_with("cantos_test_")
            && database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    assert_eq!(config.get_user(), Some("cantos_test_admin"));
    config.dbname(database);
    migrate(&config).await.unwrap();
    let h = Harness::from_connections(config.clone(), database.into(), connect(&config).await);
    let wid = fixture["work_id"].as_str().unwrap();
    let uid = fixture["unit_id"].as_str().unwrap();
    assert_eq!(
        h.json("GET", &format!("/works/{wid}"), Some(ALICE), None)
            .await
            .success(StatusCode::OK),
        fixture["current"]["work"]
    );
    assert_eq!(
        h.json("GET", &format!("/content-units/{uid}"), Some(ALICE), None)
            .await
            .success(StatusCode::OK),
        fixture["current"]["units"][0]
    );
    assert_eq!(
        h.json(
            "POST",
            "/works",
            Some(ALICE),
            Some(fixture["create_body"].clone())
        )
        .await
        .success(StatusCode::CREATED),
        fixture["original"]
    );
    assert_eq!(
        h.json(
            "POST",
            &format!("/works/{wid}/content-units"),
            Some(ALICE),
            Some(fixture["unit_body"].clone())
        )
        .await
        .success(StatusCode::CREATED),
        fixture["current"]
    );
    assert_eq!(h.counts().await, fixture["counts"]);
}
