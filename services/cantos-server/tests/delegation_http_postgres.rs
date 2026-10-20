//! Real Studio HTTP → application → PostgreSQL evidence for Work-scoped delegation (ADR 0028),
//! enabled only by a disposable PostgreSQL cluster. Fixtures are synthetic Vietnamese text.
use std::{
    env,
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    contexts::catalog::{domain::WorkId, CatalogCommand, MemberRole},
    platform::postgres::{local_config, migrate, token_hash, Store},
};
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
const APPLICATION: &str = "cantos-delegation-http-test";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const ALL: [&str; 3] = ["work:read", "content:read", "content:write"];

struct Harness {
    admin: Client,
    config: Config,
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
        assert_eq!(self.body["code"], code, "{}", self.body);
        self.body
    }
}

/// One credential per request, exactly as a client would send it.
#[derive(Clone, Copy)]
enum Credential<'a> {
    None,
    Cookie(&'a str),
    Bearer(&'a str),
    Both(&'a str, &'a str),
}

fn operation() -> String {
    Uuid::new_v4().to_string()
}

fn schema(definition: &str, body: &Value) {
    let root: Value = serde_json::from_str(match definition {
        "IssueDelegationResponse" | "DelegationGrantResponse" | "DelegationGrantPage" => {
            include_str!("../../../packages/contracts/schema/delegation/v1.schema.json")
        }
        "AppendContentResponse" | "ContentRevisionResponse" | "ContentHistoryResponse" => {
            include_str!("../../../packages/contracts/schema/content/v1.schema.json")
        }
        "ApiError" => {
            include_str!("../../../packages/contracts/schema/audio-lifecycle/v1.schema.json")
        }
        _ => include_str!("../../../packages/contracts/schema/catalog/v1.schema.json"),
    })
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

async fn send(
    app: Router,
    method: &str,
    path: &str,
    credential: Credential<'_>,
    origin: bool,
    body: Option<Value>,
    definition: &str,
) -> Reply {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("/api/v1/studio{path}"))
        .header("content-type", "application/json")
        .header("accept", "application/json");
    if origin {
        builder = builder.header("origin", ORIGIN);
    }
    let (cookie, bearer) = match credential {
        Credential::None => (None, None),
        Credential::Cookie(token) => (Some(token), None),
        Credential::Bearer(token) => (None, Some(token)),
        Credential::Both(cookie, bearer) => (Some(cookie), Some(bearer)),
    };
    if let Some(token) = cookie {
        builder = builder.header("cookie", format!("cantos_session={token}"));
    }
    if let Some(token) = bearer {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let bytes = body.map_or_else(Vec::new, |body| serde_json::to_vec(&body).unwrap());
    let response = app
        .oneshot(builder.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status = response.status();
    if status == StatusCode::NO_CONTENT {
        return Reply {
            status,
            body: Value::Null,
        };
    }
    assert_eq!(response.headers()["content-type"], "application/json");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    schema(
        if status.is_success() {
            definition
        } else {
            "ApiError"
        },
        &body,
    );
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
        // Catalog/Content grants as in catalog_http_postgres, plus append-only delegation rows.
        admin.batch_execute(
            "GRANT USAGE ON SCHEMA public TO cantos_app;
             GRANT SELECT ON audio_submissions TO cantos_app;
             GRANT SELECT ON actors,sessions,scripts,script_revisions,script_members,script_evidence,source_records,revision_evidence,
             catalog_clock,catalog_works,catalog_work_revisions,catalog_units,catalog_unit_revisions,catalog_members,catalog_operations,catalog_content_revisions,catalog_content_heads TO cantos_app;
             GRANT INSERT ON catalog_works,catalog_work_revisions,catalog_units,catalog_unit_revisions,catalog_members,catalog_operations,catalog_content_revisions,catalog_content_heads TO cantos_app;
             GRANT UPDATE(revoked) ON sessions TO cantos_app;
             GRANT UPDATE(revision) ON catalog_clock TO cantos_app;
             GRANT UPDATE(revision_id,revision) ON catalog_content_heads TO cantos_app;
             GRANT UPDATE(head_revision,lifecycle) ON catalog_works TO cantos_app;
             GRANT UPDATE(head_revision,position,lifecycle) ON catalog_units TO cantos_app;
             GRANT UPDATE(role),DELETE ON catalog_members TO cantos_app;
             GRANT EXECUTE ON FUNCTION catalog_lock_actor(text),catalog_lock_member(text,text) TO cantos_app;
             GRANT SELECT,INSERT ON catalog_delegation_grants,catalog_delegation_revocations,catalog_delegated_operations TO cantos_app;
             INSERT INTO actors(id) VALUES('alice'),('bob'),('carol'),('agent-editor'),('agent-other');",
        ).await.unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB), ("carol", CAROL)] {
            admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,clock_timestamp()+interval '1 hour')", &[&token_hash(token), &actor]).await.unwrap();
            admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,'rights','catalog-rights','Synthetic catalog rights reference')", &[&actor]).await.unwrap();
        }
        let mut app_config = config.clone();
        app_config.user("cantos_app").application_name(APPLICATION);
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
        }
    }

    async fn call(
        &self,
        method: &str,
        path: &str,
        credential: Credential<'_>,
        body: Option<Value>,
        definition: &str,
    ) -> Reply {
        // Owner/browser calls carry the Studio Origin; delegate calls deliberately do not.
        let origin = matches!(credential, Credential::Cookie(_) | Credential::Both(..));
        send(
            self.app.clone(),
            method,
            path,
            credential,
            origin,
            body,
            definition,
        )
        .await
    }

    async fn now(&self) -> i64 {
        self.admin
            .query_one(
                "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
                &[],
            )
            .await
            .unwrap()
            .get(0)
    }

    /// Returns `(work, unit)` after creating a Work with one chapter as Alice.
    async fn work_with_unit(&self, title: &str) -> (Value, Value) {
        let work = self
            .call(
                "POST",
                "/works",
                Credential::Cookie(ALICE),
                Some(json!({
                    "operation_id": operation(), "id": operation(), "kind": "novel",
                    "details": {"title": title, "synopsis": "Tác phẩm thử nghiệm ủy quyền.",
                        "language": "vi-VN", "visibility": "private", "rights_refs": []}
                })),
                "CatalogMutationResponse",
            )
            .await
            .success(StatusCode::CREATED)["work"]
            .clone();
        let outcome = self
            .call(
                "POST",
                &format!("/works/{}/content-units", work["id"].as_str().unwrap()),
                Credential::Cookie(ALICE),
                Some(json!({
                    "operation_id": operation(), "id": operation(), "kind": "chapter",
                    "expected_work_revision": work["revision"],
                    "details": {"title": "Chương một", "description": "Chưa có âm thanh."}
                })),
                "CatalogMutationResponse",
            )
            .await
            .success(StatusCode::CREATED);
        (outcome["work"].clone(), outcome["units"][0].clone())
    }

    fn issue_request(&self, delegate: &str, actions: &[&str], expires_at: i64) -> Value {
        json!({"operation_id": operation(), "id": operation(), "delegate_actor_id": delegate,
            "actions": actions, "expires_at": expires_at})
    }

    async fn issue(&self, work: &Value, actions: &[&str]) -> (Value, String) {
        let request = self.issue_request("agent-editor", actions, self.now().await + 7 * DAY_MS);
        let reply = self
            .call(
                "POST",
                &format!("/works/{}/delegations", work["id"].as_str().unwrap()),
                Credential::Cookie(ALICE),
                Some(request),
                "IssueDelegationResponse",
            )
            .await
            .success(StatusCode::CREATED);
        let token = reply["token"].as_str().unwrap().to_owned();
        (reply["grant"].clone(), token)
    }

    async fn counts(&self) -> Value {
        let text: String = self.admin.query_one("SELECT json_build_object('grants',(SELECT count(*) FROM catalog_delegation_grants),'revocations',(SELECT count(*) FROM catalog_delegation_revocations),'lineage',(SELECT count(*) FROM catalog_delegated_operations),'operations',(SELECT count(*) FROM catalog_operations),'work_revisions',(SELECT count(*) FROM catalog_work_revisions),'content_revisions',(SELECT count(*) FROM catalog_content_revisions),'clock',(SELECT revision FROM catalog_clock))::text", &[]).await.unwrap().get(0);
        serde_json::from_str(&text).unwrap()
    }
}

fn content(expected: u64, work_revision: &Value, body: Value, rights: &[&str]) -> Value {
    json!({"operation_id": operation(), "id": operation(), "expected_revision": expected,
        "expected_work_revision": work_revision,
        "details": {"language": "vi-VN", "body": body, "rights_refs": rights,
            "provenance_note": "Bản thảo tổng hợp do tác tử biên tập; chưa được duyệt phát hành."}})
}
fn text(value: &str) -> Value {
    json!({"kind": "text", "text": value, "source": null})
}

/// The independent oracle for the stored lookup: domain separator plus the decoded secret.
fn expected_lookup(token: &str) -> Vec<u8> {
    let hex = token.strip_prefix("cantos_dlg_").unwrap();
    let secret: Vec<u8> = (0..32)
        .map(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
        .collect();
    let mut hash = Sha256::new();
    hash.update(b"cantos/work-delegation/v1\n");
    hash.update(&secret);
    hash.finalize().to_vec()
}

async fn blocked(admin: &Client, count: i64) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let waiting: i64 = admin
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock'",
                &[&APPLICATION],
            )
            .await
            .unwrap()
            .get(0);
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
async fn delegate_reads_and_appends_only_inside_the_granted_work_under_its_own_identity() {
    let h = Harness::new().await;
    let (work_a, unit_a) = h.work_with_unit("Ánh đèn cuối sân khấu").await;
    let (work_b, unit_b) = h.work_with_unit("Mùa trăng qua hiên").await;
    let (a, a1) = (
        work_a["id"].as_str().unwrap(),
        unit_a["id"].as_str().unwrap(),
    );
    let (b, b1) = (
        work_b["id"].as_str().unwrap(),
        unit_b["id"].as_str().unwrap(),
    );

    // Issuance reveals the secret once and stores only its purpose-separated lookup.
    let request = h.issue_request("agent-editor", &ALL, h.now().await + 7 * DAY_MS);
    let created = h
        .call(
            "POST",
            &format!("/works/{a}/delegations"),
            Credential::Cookie(ALICE),
            Some(request.clone()),
            "IssueDelegationResponse",
        )
        .await
        .success(StatusCode::CREATED);
    let grant = created["grant"].clone();
    let token = created["token"].as_str().unwrap().to_owned();
    assert_eq!(grant["work_id"], a);
    assert_eq!(grant["grantor_id"], "alice");
    assert_eq!(grant["delegate_actor_id"], "agent-editor");
    assert_eq!(grant["actions"], json!(ALL));
    assert_eq!(grant["revoked_at"], Value::Null);
    let stored: Vec<u8> = h
        .admin
        .query_one(
            "SELECT token_hash FROM catalog_delegation_grants WHERE id=$1",
            &[&grant["id"].as_str().unwrap()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(stored, expected_lookup(&token));
    assert_ne!(stored, token_hash(&token), "never the Studio session hash");

    // A lost response is retried: same grant, no second secret, no second row.
    let before = h.counts().await;
    let replay = h
        .call(
            "POST",
            &format!("/works/{a}/delegations"),
            Credential::Cookie(ALICE),
            Some(request.clone()),
            "IssueDelegationResponse",
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(replay, json!({"grant": grant, "token": null}));
    let mut changed = request.clone();
    changed["actions"] = json!(["work:read"]);
    h.call(
        "POST",
        &format!("/works/{a}/delegations"),
        Credential::Cookie(ALICE),
        Some(changed),
        "IssueDelegationResponse",
    )
    .await
    .error(StatusCode::CONFLICT, "operation_reused");
    assert_eq!(h.counts().await, before);

    let bearer = Credential::Bearer(&token);
    assert_eq!(
        h.call(
            "GET",
            "/delegated/grant",
            bearer,
            None,
            "DelegationGrantResponse"
        )
        .await
        .success(StatusCode::OK),
        grant
    );
    assert_eq!(
        h.call(
            "GET",
            &format!("/delegated/works/{a}"),
            bearer,
            None,
            "CatalogWork"
        )
        .await
        .success(StatusCode::OK)["id"],
        a
    );
    let page = h
        .call(
            "GET",
            &format!("/delegated/works/{a}/content-units"),
            bearer,
            None,
            "CatalogContentUnitPage",
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(page["items"][0]["id"], a1);
    // Work B is indistinguishable from a missing Work for every route.
    for path in [
        format!("/delegated/works/{b}"),
        format!("/delegated/works/{b}/content-units"),
        format!("/delegated/content-units/{b1}"),
        format!("/delegated/content-units/{b1}/content/head"),
        format!("/delegated/content-units/{b1}/content/history"),
        format!("/delegated/works/{}", operation()),
    ] {
        h.call("GET", &path, bearer, None, "CatalogWork")
            .await
            .error(StatusCode::NOT_FOUND, "not_found");
    }

    // The append needs no browser Origin and is recorded by the delegate, never by Alice.
    let append = content(
        0,
        &work_a["revision"],
        text("Người kể chuyện\r\nđã trở về."),
        &[],
    );
    let before = h.counts().await;
    let appended = h
        .call(
            "POST",
            &format!("/delegated/content-units/{a1}/content/revisions"),
            bearer,
            Some(append.clone()),
            "AppendContentResponse",
        )
        .await
        .success(StatusCode::CREATED);
    assert_eq!(appended["content"]["recorded_by"], "agent-editor");
    assert_eq!(appended["content"]["unit_id"], a1);
    let operation_id = append["operation_id"].as_str().unwrap();
    let row = h
        .admin
        .query_one(
            "SELECT (SELECT recorded_by FROM catalog_content_revisions WHERE id=$1),
                    (SELECT recorded_by FROM catalog_work_revisions WHERE work_id=$2 ORDER BY revision DESC LIMIT 1),
                    (SELECT grant_id FROM catalog_delegated_operations WHERE actor_id='agent-editor' AND operation_id=$3),
                    (SELECT count(*) FROM catalog_operations WHERE actor_id='alice' AND operation_id=$3)",
            &[&append["id"].as_str().unwrap(), &a, &operation_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "agent-editor");
    assert_eq!(row.get::<_, String>(1), "agent-editor");
    assert_eq!(row.get::<_, String>(2), grant["id"].as_str().unwrap());
    assert_eq!(row.get::<_, i64>(3), 0);
    let mut expected = before.clone();
    for key in [
        "lineage",
        "operations",
        "work_revisions",
        "content_revisions",
        "clock",
    ] {
        expected[key] = json!(before[key].as_i64().unwrap() + 1);
    }
    let after = h.counts().await;
    assert_eq!(after, expected);

    // Retry returns the receipt without a new effect.
    assert_eq!(
        h.call(
            "POST",
            &format!("/delegated/content-units/{a1}/content/revisions"),
            bearer,
            Some(append.clone()),
            "AppendContentResponse",
        )
        .await
        .success(StatusCode::CREATED),
        appended
    );
    assert_eq!(h.counts().await, after);

    // Foreign units, source/Script links and rights changes never commit.
    let work_revision = appended["work_revision"].clone();
    h.call(
        "POST",
        &format!("/delegated/content-units/{b1}/content/revisions"),
        bearer,
        Some(content(
            0,
            &work_b["revision"],
            text("Không thuộc quyền."),
            &[],
        )),
        "AppendContentResponse",
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    for (body, rights) in [
        (
            json!({"kind":"text","text":"Có nguồn.","source":{"id":"source-vi","sha256":"0".repeat(64)}}),
            vec![],
        ),
        (
            json!({"kind":"source","source":{"id":"source-vi","sha256":"0".repeat(64)}}),
            vec![],
        ),
        (text("Đổi quyền sử dụng."), vec!["catalog-rights"]),
    ] {
        h.call(
            "POST",
            &format!("/delegated/content-units/{a1}/content/revisions"),
            bearer,
            Some(content(1, &work_revision, body, &rights)),
            "AppendContentResponse",
        )
        .await
        .error(StatusCode::FORBIDDEN, "forbidden");
    }
    assert_eq!(h.counts().await, after);

    let head = h
        .call(
            "GET",
            &format!("/delegated/content-units/{a1}/content/head"),
            bearer,
            None,
            "ContentRevisionResponse",
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(head, appended["content"]);
    let history = h
        .call(
            "GET",
            &format!("/delegated/content-units/{a1}/content/history"),
            bearer,
            None,
            "ContentHistoryResponse",
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(history["revisions"][0]["recorded_by"], "agent-editor");
    // The owner sees the delegate's attribution through the ordinary Studio route.
    assert_eq!(
        h.call(
            "GET",
            &format!("/content-units/{a1}/content/head"),
            Credential::Cookie(ALICE),
            None,
            "ContentRevisionResponse",
        )
        .await
        .success(StatusCode::OK)["recorded_by"],
        "agent-editor"
    );

    // One credential, one purpose: no mixing, no cross-surface reuse.
    for (path, credential) in [
        (
            format!("/delegated/works/{a}"),
            Credential::Both(ALICE, &token),
        ),
        (format!("/delegated/works/{a}"), Credential::Cookie(ALICE)),
        (format!("/delegated/works/{a}"), Credential::Bearer(ALICE)),
        (format!("/delegated/works/{a}"), Credential::None),
        (format!("/works/{a}"), Credential::Bearer(&token)),
        (format!("/works/{a}"), Credential::Cookie(&token)),
        (
            format!("/works/{a}/delegations"),
            Credential::Bearer(&token),
        ),
    ] {
        h.call("GET", &path, credential, None, "CatalogWork")
            .await
            .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }

    let listed = h
        .call(
            "GET",
            &format!("/works/{a}/delegations"),
            Credential::Cookie(ALICE),
            None,
            "DelegationGrantPage",
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(listed, json!({"items": [grant], "next_after": null}));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn revocation_expiry_deactivation_and_archive_end_delegated_access() {
    let h = Harness::new().await;
    let (work, unit) = h.work_with_unit("Ánh đèn cuối sân khấu").await;
    let (a, a1) = (work["id"].as_str().unwrap(), unit["id"].as_str().unwrap());

    // A read-only grant refuses content reads and writes but serves the Work.
    let (read_only, token) = h.issue(&work, &["work:read"]).await;
    let bearer = Credential::Bearer(&token);
    h.call(
        "GET",
        &format!("/delegated/works/{a}"),
        bearer,
        None,
        "CatalogWork",
    )
    .await
    .success(StatusCode::OK);
    h.call(
        "GET",
        &format!("/delegated/content-units/{a1}/content/history"),
        bearer,
        None,
        "ContentHistoryResponse",
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
    h.call(
        "POST",
        &format!("/delegated/content-units/{a1}/content/revisions"),
        bearer,
        Some(content(0, &work["revision"], text("Không được ghi."), &[])),
        "AppendContentResponse",
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");

    // Revocation is idempotent, keeps its first record and ends every later request.
    let grant_path = format!(
        "/works/{a}/delegations/{}",
        read_only["id"].as_str().unwrap()
    );
    for _ in 0..2 {
        h.call("DELETE", &grant_path, Credential::Cookie(ALICE), None, "")
            .await
            .success(StatusCode::NO_CONTENT);
    }
    let revoked: (String, i64) = {
        let row = h
            .admin
            .query_one(
                "SELECT revoked_by,count(*) OVER () FROM catalog_delegation_revocations",
                &[],
            )
            .await
            .unwrap();
        (row.get(0), row.get(1))
    };
    assert_eq!(revoked, ("alice".into(), 1));
    for path in [
        "/delegated/grant".to_owned(),
        format!("/delegated/works/{a}"),
    ] {
        h.call("GET", &path, bearer, None, "CatalogWork")
            .await
            .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    let listed = h
        .call(
            "GET",
            &format!("/works/{a}/delegations"),
            Credential::Cookie(ALICE),
            None,
            "DelegationGrantPage",
        )
        .await
        .success(StatusCode::OK);
    assert!(listed["items"][0]["revoked_at"].is_i64());
    h.call(
        "DELETE",
        &format!("/works/{a}/delegations/{}", operation()),
        Credential::Cookie(ALICE),
        None,
        "",
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");

    // Delegate or grantor deactivation suspends the grant; reactivation restores it.
    let (live, token) = h.issue(&work, &["work:read"]).await;
    let bearer = Credential::Bearer(&token);
    // A grant is revocable only through its own Work, even by the same owner.
    let (other, _) = h.work_with_unit("Mùa trăng qua hiên").await;
    h.call(
        "DELETE",
        &format!(
            "/works/{}/delegations/{}",
            other["id"].as_str().unwrap(),
            live["id"].as_str().unwrap()
        ),
        Credential::Cookie(ALICE),
        None,
        "",
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    for actor in ["agent-editor", "alice"] {
        h.admin
            .execute("UPDATE actors SET active=false WHERE id=$1", &[&actor])
            .await
            .unwrap();
        h.call(
            "GET",
            &format!("/delegated/works/{a}"),
            bearer,
            None,
            "CatalogWork",
        )
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
        h.admin
            .execute("UPDATE actors SET active=true WHERE id=$1", &[&actor])
            .await
            .unwrap();
        h.call(
            "GET",
            &format!("/delegated/works/{a}"),
            bearer,
            None,
            "CatalogWork",
        )
        .await
        .success(StatusCode::OK);
    }

    // Expiry is decided by the database clock, not by the token.
    let now = h.now().await;
    let expired_secret = [7_u8; 32];
    let expired_token = format!(
        "cantos_dlg_{}",
        expired_secret
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    h.admin
        .execute(
            "INSERT INTO catalog_delegation_grants(id,work_id,grantor_id,delegate_id,work_read,content_read,content_write,token_hash,issued_at,expires_at,operation_id,request_digest) VALUES($1,$2,'alice','agent-editor',true,false,false,$3,$4,$5,$6,$7)",
            &[&operation(), &a, &expected_lookup(&expired_token), &(now - 2000), &(now - 1000), &operation(), &format!("dg-request1:sha256:{}", "0".repeat(64))],
        )
        .await
        .unwrap();
    h.call(
        "GET",
        &format!("/delegated/works/{a}"),
        Credential::Bearer(&expired_token),
        None,
        "CatalogWork",
    )
    .await
    .error(StatusCode::UNAUTHORIZED, "unauthenticated");

    // Archiving the Work ends delegation; the owner can no longer issue for it.
    h.call(
        "DELETE",
        &format!("/works/{a}"),
        Credential::Cookie(ALICE),
        Some(json!({"operation_id": operation(), "expected_revision": work["revision"]})),
        "CatalogMutationResponse",
    )
    .await
    .success(StatusCode::OK);
    h.call(
        "GET",
        &format!("/delegated/works/{a}"),
        bearer,
        None,
        "CatalogWork",
    )
    .await
    .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    h.call(
        "POST",
        &format!("/works/{a}/delegations"),
        Credential::Cookie(ALICE),
        Some(h.issue_request("agent-editor", &ALL, h.now().await + DAY_MS)),
        "IssueDelegationResponse",
    )
    .await
    .error(StatusCode::CONFLICT, "catalog_conflict");

    // Settled delegation rows are immutable even to the migration owner.
    for statement in [
        "UPDATE catalog_delegation_grants SET content_write=true",
        "DELETE FROM catalog_delegation_revocations",
        "UPDATE catalog_delegation_grants SET expires_at=expires_at+1",
    ] {
        assert!(
            h.admin.execute(statement, &[]).await.is_err(),
            "{statement}"
        );
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn only_the_owner_issues_and_every_rejected_issuance_commits_nothing() {
    let h = Harness::new().await;
    let (work, _) = h.work_with_unit("Ánh đèn cuối sân khấu").await;
    let a = work["id"].as_str().unwrap();
    let path = format!("/works/{a}/delegations");
    let work = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: WorkId::parse(a).unwrap(),
                expected_revision: work["revision"].as_u64().unwrap(),
                actor_id: cantos_server::contexts::catalog::domain::OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Editor),
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(work.revision(), 3);
    let now = h.now().await;
    let before = h.counts().await;

    // Editors cannot delegate; strangers cannot learn that the Work exists.
    for (token, status, code) in [
        (BOB, StatusCode::FORBIDDEN, "forbidden"),
        (CAROL, StatusCode::NOT_FOUND, "not_found"),
    ] {
        h.call(
            "POST",
            &path,
            Credential::Cookie(token),
            Some(h.issue_request("agent-editor", &ALL, now + DAY_MS)),
            "IssueDelegationResponse",
        )
        .await
        .error(status, code);
        h.call(
            "GET",
            &path,
            Credential::Cookie(token),
            None,
            "DelegationGrantPage",
        )
        .await
        .error(status, code);
    }

    h.admin
        .execute("UPDATE actors SET active=false WHERE id='agent-other'", &[])
        .await
        .unwrap();
    let mut duplicate = h.issue_request("agent-editor", &["work:read", "work:read"], now + DAY_MS);
    duplicate["actions"] = json!(["work:read", "work:read"]);
    for (request, field, rule) in [
        (
            h.issue_request("alice", &ALL, now + DAY_MS),
            "delegate_actor_id",
            "self_delegation",
        ),
        (
            h.issue_request("ghost", &ALL, now + DAY_MS),
            "delegate_actor_id",
            "unavailable_actor",
        ),
        (
            h.issue_request("agent-other", &ALL, now + DAY_MS),
            "delegate_actor_id",
            "unavailable_actor",
        ),
        (
            h.issue_request("Agent", &ALL, now + DAY_MS),
            "delegate_actor_id",
            "invalid_actor",
        ),
        (
            h.issue_request("agent-editor", &ALL, now + 31 * DAY_MS),
            "expires_at",
            "invalid_expiry",
        ),
        (
            h.issue_request("agent-editor", &ALL, now - 1),
            "expires_at",
            "invalid_expiry",
        ),
        (
            h.issue_request("agent-editor", &[], now + DAY_MS),
            "actions",
            "empty",
        ),
        (duplicate, "actions", "duplicate_action"),
    ] {
        let error = h
            .call(
                "POST",
                &path,
                Credential::Cookie(ALICE),
                Some(request),
                "IssueDelegationResponse",
            )
            .await
            .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_catalog");
        assert_eq!(error["issues"], json!([{"path": field, "rule": rule}]));
    }
    let mut unknown = h.issue_request("agent-editor", &ALL, now + DAY_MS);
    unknown["actions"] = json!(["publication:stage"]);
    let mut extra = h.issue_request("agent-editor", &ALL, now + DAY_MS);
    extra["grantor_id"] = json!("bob");
    for request in [unknown, extra] {
        h.call(
            "POST",
            &path,
            Credential::Cookie(ALICE),
            Some(request),
            "IssueDelegationResponse",
        )
        .await
        .error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    // Owner mutations still require the Studio Origin.
    send(
        h.app.clone(),
        "POST",
        &path,
        Credential::Cookie(ALICE),
        false,
        Some(h.issue_request("agent-editor", &ALL, now + DAY_MS)),
        "IssueDelegationResponse",
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
    assert_eq!(h.counts().await, before);

    // A grant ID names one grant: reuse under another operation is a conflict.
    let first = h.issue_request("agent-editor", &ALL, now + DAY_MS);
    h.call(
        "POST",
        &path,
        Credential::Cookie(ALICE),
        Some(first.clone()),
        "IssueDelegationResponse",
    )
    .await
    .success(StatusCode::CREATED);
    let mut reused = first;
    reused["operation_id"] = json!(operation());
    h.call(
        "POST",
        &path,
        Credential::Cookie(ALICE),
        Some(reused),
        "IssueDelegationResponse",
    )
    .await
    .error(StatusCode::CONFLICT, "catalog_conflict");
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn an_admitted_write_finishes_before_revoke_and_a_waiting_write_sees_the_revocation() {
    let h = Harness::new().await;
    let (work, unit) = h.work_with_unit("Ánh đèn cuối sân khấu").await;
    let (a, a1) = (
        work["id"].as_str().unwrap().to_owned(),
        unit["id"].as_str().unwrap().to_owned(),
    );
    let append_path = format!("/delegated/content-units/{a1}/content/revisions");

    // 1. The write holds the Catalog clock and waits on the Work row; revoke queues behind it.
    let (grant, token) = h.issue(&work, &ALL).await;
    let mut blocker = connect(&h.config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one("SELECT id FROM catalog_works WHERE id=$1 FOR UPDATE", &[&a])
        .await
        .unwrap();
    let write = {
        let (app, path, token) = (h.app.clone(), append_path.clone(), token.clone());
        let body = content(0, &work["revision"], text("Ghi trước khi thu hồi."), &[]);
        tokio::spawn(async move {
            send(
                app,
                "POST",
                &path,
                Credential::Bearer(&token),
                false,
                Some(body),
                "AppendContentResponse",
            )
            .await
        })
    };
    blocked(&h.admin, 1).await;
    let revoke = {
        let app = h.app.clone();
        let path = format!("/works/{a}/delegations/{}", grant["id"].as_str().unwrap());
        tokio::spawn(async move {
            send(
                app,
                "DELETE",
                &path,
                Credential::Cookie(ALICE),
                true,
                None,
                "",
            )
            .await
        })
    };
    blocked(&h.admin, 2).await;
    tx.commit().await.unwrap();
    let written = write.await.unwrap().success(StatusCode::CREATED);
    revoke.await.unwrap().success(StatusCode::NO_CONTENT);
    assert_eq!(written["content"]["recorded_by"], "agent-editor");
    h.call(
        "POST",
        &append_path,
        Credential::Bearer(&token),
        Some(content(
            1,
            &written["work_revision"],
            text("Sau khi thu hồi."),
            &[],
        )),
        "AppendContentResponse",
    )
    .await
    .error(StatusCode::UNAUTHORIZED, "unauthenticated");

    // 2. Revoke holds the clock and waits on Alice's actor row; the queued write then sees it.
    let (grant, token) = h.issue(&work, &ALL).await;
    let before = h.counts().await;
    let mut blocker = connect(&h.config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one("SELECT id FROM actors WHERE id='alice' FOR UPDATE", &[])
        .await
        .unwrap();
    let revoke = {
        let app = h.app.clone();
        let path = format!("/works/{a}/delegations/{}", grant["id"].as_str().unwrap());
        tokio::spawn(async move {
            send(
                app,
                "DELETE",
                &path,
                Credential::Cookie(ALICE),
                true,
                None,
                "",
            )
            .await
        })
    };
    blocked(&h.admin, 1).await;
    let write = {
        let (app, path, token) = (h.app.clone(), append_path.clone(), token.clone());
        let body = content(1, &written["work_revision"], text("Chờ sau thu hồi."), &[]);
        tokio::spawn(async move {
            send(
                app,
                "POST",
                &path,
                Credential::Bearer(&token),
                false,
                Some(body),
                "AppendContentResponse",
            )
            .await
        })
    };
    blocked(&h.admin, 2).await;
    tx.commit().await.unwrap();
    revoke.await.unwrap().success(StatusCode::NO_CONTENT);
    write
        .await
        .unwrap()
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    let mut expected = before;
    expected["revocations"] = json!(expected["revocations"].as_i64().unwrap() + 1);
    assert_eq!(h.counts().await, expected);
}
