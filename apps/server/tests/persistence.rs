//! PostgreSQL boundary tests. Each case creates and drops its own disposable database.
//! Run with TEST_DATABASE_URL set to a PostgreSQL role with CREATEDB, then:
//! cargo test -p cantos-server --test persistence -- --ignored

use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "support/contracts.rs"]
mod contract_schema;

const OWNER: Uuid = Uuid::from_u128(1);
const OTHER: Uuid = Uuid::from_u128(2);

struct TestDatabase {
    admin: PgPool,
    pool: PgPool,
    name: String,
    url: String,
}

impl TestDatabase {
    async fn create() -> Self {
        let url = std::env::var("TEST_DATABASE_URL")
            .expect("set TEST_DATABASE_URL to a disposable PostgreSQL administrator URL");
        let options = PgConnectOptions::from_str(&url).expect("valid TEST_DATABASE_URL");
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options.clone())
            .await
            .expect("connect to PostgreSQL test administrator");
        let name = format!("cantos_test_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE {name}"))
            .execute(&admin)
            .await
            .expect("create isolated test database");
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect_with(options.database(&name).application_name(&name))
            .await
            .expect("connect to isolated test database");
        cantos_server::migrate(&pool)
            .await
            .expect("apply production migrations");
        for (creator, token) in [(OWNER, token('a')), (OTHER, token('b'))] {
            sqlx::query("INSERT INTO creators (id) VALUES ($1)")
                .bind(creator)
                .execute(&pool)
                .await
                .expect("seed creator");
            sqlx::query(
                "INSERT INTO creator_tokens (token_hash, creator_id, expires_at)
                 VALUES ($1, $2, now() + interval '1 hour')",
            )
            .bind(Sha256::digest(token.as_bytes()).to_vec())
            .bind(creator)
            .execute(&pool)
            .await
            .expect("seed hashed credential");
        }
        let mut database_url = reqwest::Url::parse(&url).expect("URL-form TEST_DATABASE_URL");
        database_url.set_path(&format!("/{name}"));
        Self {
            admin,
            pool,
            name,
            url: database_url.to_string(),
        }
    }

    fn app(&self) -> Router {
        cantos_server::router(self.pool.clone())
    }

    async fn destroy(self) {
        self.pool.close().await;
        sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .execute(&self.admin)
            .await
            .expect("drop isolated test database");
        self.admin.close().await;
    }

    async fn row_count(&self, table: &str) -> i64 {
        // Table names are fixed test literals, never HTTP or environment input.
        sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&self.pool)
            .await
            .expect("count persisted rows")
    }
}

fn token(character: char) -> String {
    character.to_string().repeat(64)
}

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
    ))
    .expect("committed valid Script IR fixture")
}

fn changed_content(text: &str) -> Value {
    let mut content = fixture();
    content["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] = json!(text);
    content
}

fn create_request() -> Value {
    let content = fixture();
    json!({
        "operation_id": Uuid::new_v4(),
        "source": {
            "id": content["provenance"][0]["source_record_id"],
            "kind": "original",
            "reference": "original fixture",
            "original_text": "Người dẫn chuyện\r\n  Bản thảo gốc.\n"
        },
        "rights": {
            "id": content["work"]["rights_record_id"],
            "holder": "Fixture creator",
            "evidence_reference": "fixture declaration"
        },
        "content": content
    })
}

fn save_request(base: i64, content: Value) -> Value {
    json!({"operation_id": Uuid::new_v4(), "base_version": base, "content": content})
}

fn accept_request(version: i64) -> Value {
    json!({"operation_id": Uuid::new_v4(), "version": version})
}

async fn request(
    app: Router,
    method: &str,
    path: &str,
    credential: Option<&str>,
    body: Option<&Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(credential) = credential {
        builder = builder.header("authorization", format!("Bearer {credential}"));
    }
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_vec(value).expect("serialize request"))
        }
        None => Body::empty(),
    };
    let response = app
        .oneshot(builder.body(body).expect("HTTP request"))
        .await
        .expect("infallible router");
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    if status == StatusCode::UNAUTHORIZED {
        assert_eq!(response.headers()["www-authenticate"], "Bearer");
    }
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("bounded response body");
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("response {status} is not JSON: {error}"));
    let definition = if !status.is_success() {
        "ErrorResponse"
    } else if path.contains("/history") {
        "History"
    } else if path.contains("/revisions") {
        "AcceptedRevision"
    } else if path.contains("/versions") {
        "SavedVersion"
    } else {
        "ScriptView"
    };
    assert_response_schema(definition, &value);
    (status, value)
}

fn assert_response_schema(definition: &str, value: &Value) {
    static SCHEMAS: OnceLock<Vec<(&str, jsonschema::Validator)>> = OnceLock::new();
    let schemas = SCHEMAS.get_or_init(|| {
        [
            "ScriptView",
            "SavedVersion",
            "AcceptedRevision",
            "History",
            "ErrorResponse",
        ]
        .into_iter()
        .map(|name| (name, contract_schema::validator(name)))
        .collect()
    });
    let validator = &schemas
        .iter()
        .find(|(name, _)| *name == definition)
        .unwrap()
        .1;
    let errors: Vec<_> = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "HTTP response violates {definition}: {errors:?}"
    );
}

async fn owner_request(
    database: &TestDatabase,
    method: &str,
    path: &str,
    body: Option<&Value>,
    expected: StatusCode,
) -> Value {
    let (status, value) = request(database.app(), method, path, Some(&token('a')), body).await;
    assert_eq!(status, expected, "{method} {path}: {value}");
    value
}

async fn create_script(database: &TestDatabase) -> Value {
    owner_request(
        database,
        "POST",
        "/v1/scripts",
        Some(&create_request()),
        StatusCode::CREATED,
    )
    .await
}

fn script_path(script: &Value) -> String {
    format!("/v1/scripts/{}", script["id"].as_str().expect("script ID"))
}

fn assert_error(value: &Value, code: &str) {
    assert_eq!(value["error"]["code"], code, "{value}");
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn creator_saves_reads_and_audits_versions_with_preserved_source_and_pending_rights() {
    let database = TestDatabase::create().await;
    let create = create_request();
    let script = owner_request(
        &database,
        "POST",
        "/v1/scripts",
        Some(&create),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(script["owner_id"], OWNER.to_string());
    assert_eq!(script["head"]["version"], 1);
    assert_eq!(script["head"]["base_version"], 0);
    assert_eq!(script["head"]["actor_id"], OWNER.to_string());
    assert_eq!(script["head"]["operation_id"], create["operation_id"]);
    assert_eq!(
        script["source"]["original_text"],
        create["source"]["original_text"]
    );
    assert_eq!(script["source"]["reference"], "original fixture");
    assert_eq!(script["rights"]["status"], "pending_review");
    assert_eq!(
        script["rights"]["evidence_reference"],
        "fixture declaration"
    );
    let source_digest = format!(
        "{:x}",
        Sha256::digest(
            create["source"]["original_text"]
                .as_str()
                .unwrap()
                .as_bytes()
        )
    );
    assert_eq!(script["source"]["sha256"], source_digest);
    assert!(script["head"]["created_at"].is_string());
    let path = script_path(&script);
    let reopened = owner_request(&database, "GET", &path, None, StatusCode::OK).await;
    assert_eq!(reopened, script);
    let edit = save_request(1, changed_content("Ngày mai, sân khấu lại sáng đèn."));
    let saved = owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&edit),
        StatusCode::OK,
    )
    .await;
    assert_eq!(saved["version"], 2);
    assert_eq!(saved["base_version"], 1);
    assert_eq!(saved["actor_id"], OWNER.to_string());
    assert_eq!(saved["operation_id"], edit["operation_id"]);
    assert_ne!(saved["digest"], script["head"]["digest"]);
    let original = owner_request(
        &database,
        "GET",
        &format!("{path}/versions/1"),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(original, script["head"]);
    let history = owner_request(
        &database,
        "GET",
        &format!("{path}/history?after=0&limit=50"),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(history["versions"], json!([original, saved]));
    assert_eq!(history["revisions"], json!([]));
    let page = owner_request(
        &database,
        "GET",
        &format!("{path}/history?after=1&limit=1"),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(page["versions"], json!([saved]));
    for (table, expected) in [
        ("scripts", 1),
        ("source_records", 1),
        ("rights_records", 1),
        ("script_versions", 2),
    ] {
        assert_eq!(database.row_count(table).await, expected, "{table}");
    }
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn every_script_route_rejects_missing_identity_and_hides_other_creators_content() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let path = script_path(&script);
    let accepted = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept_request(1)),
        StatusCode::OK,
    )
    .await;
    let routes = [
        ("GET", path.clone(), None),
        ("GET", format!("{path}/versions/1"), None),
        ("GET", format!("{path}/history?after=0&limit=50"), None),
        (
            "GET",
            format!("{path}/revisions/{}", accepted["id"].as_str().unwrap()),
            None,
        ),
        (
            "POST",
            format!("{path}/versions"),
            Some(save_request(1, changed_content("Không được ghi dòng này."))),
        ),
        ("POST", format!("{path}/revisions"), Some(accept_request(1))),
    ];
    for (method, route, body) in routes {
        for (credential, status, code) in [
            (None, StatusCode::UNAUTHORIZED, "unauthenticated"),
            (Some(token('b')), StatusCode::NOT_FOUND, "not_found"),
        ] {
            let (actual, value) = request(
                database.app(),
                method,
                &route,
                credential.as_deref(),
                body.as_ref(),
            )
            .await;
            assert_eq!(actual, status, "{method} {route}: {value}");
            assert_error(&value, code);
            assert!(value["error"]["head_version"].is_null());
            assert!(!value.to_string().contains("original fixture"));
        }
    }
    let (status, body) = request(
        database.app(),
        "POST",
        "/v1/scripts",
        None,
        Some(&create_request()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_error(&body, "unauthenticated");
    assert_eq!(database.row_count("scripts").await, 1);
    assert_eq!(database.row_count("script_versions").await, 1);
    assert_eq!(database.row_count("script_revisions").await, 1);
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn request_fields_cannot_forge_owner_actor_or_publication_permission() {
    let database = TestDatabase::create().await;
    for (field, value) in [("owner_id", json!(OTHER)), ("actor_id", json!(OTHER))] {
        let mut body = create_request();
        body[field] = value;
        let error = owner_request(
            &database,
            "POST",
            "/v1/scripts",
            Some(&body),
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_error(&error, "invalid_request");
    }
    let mut body = create_request();
    body["rights"]["status"] = json!("approved");
    let error = owner_request(
        &database,
        "POST",
        "/v1/scripts",
        Some(&body),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_error(&error, "invalid_request");
    assert_eq!(database.row_count("scripts").await, 0);
    let script = create_script(&database).await;
    let path = script_path(&script);
    for (suffix, mut body) in [
        ("versions", save_request(1, fixture())),
        ("revisions", accept_request(1)),
    ] {
        body["actor_id"] = json!(OTHER);
        let error = owner_request(
            &database,
            "POST",
            &format!("{path}/{suffix}"),
            Some(&body),
            StatusCode::BAD_REQUEST,
        )
        .await;
        assert_error(&error, "invalid_request");
    }
    assert_eq!(database.row_count("script_versions").await, 1);
    assert_eq!(database.row_count("script_revisions").await, 0);
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn revoked_expired_disabled_unknown_and_ambiguous_credentials_fail_closed() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let path = script_path(&script);
    for character in ['c', 'd'] {
        sqlx::query(
            "INSERT INTO creator_tokens (token_hash, creator_id, expires_at, revoked_at)
             VALUES ($1, $2,
               CASE WHEN $3 THEN now() - interval '1 second' ELSE now() + interval '1 hour' END,
               CASE WHEN $3 THEN NULL ELSE now() END)",
        )
        .bind(Sha256::digest(token(character).as_bytes()).to_vec())
        .bind(OWNER)
        .bind(character == 'c')
        .execute(&database.pool)
        .await
        .unwrap();
    }
    for credential in [token('c'), token('d'), token('f'), "malformed".to_owned()] {
        let (status, value) = request(database.app(), "GET", &path, Some(&credential), None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_error(&value, "unauthenticated");
    }
    let response = database
        .app()
        .oneshot(
            Request::builder()
                .uri(&path)
                .header("authorization", format!("Bearer {}", token('a')))
                .header("authorization", format!("Bearer {}", token('b')))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    sqlx::query("UPDATE creators SET disabled = true WHERE id = $1")
        .bind(OWNER)
        .execute(&database.pool)
        .await
        .unwrap();
    let error = owner_request(&database, "GET", &path, None, StatusCode::UNAUTHORIZED).await;
    assert_error(&error, "unauthenticated");
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn competing_edits_wait_on_the_same_head_and_exactly_one_commits() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let script_id = Uuid::parse_str(script["id"].as_str().unwrap()).unwrap();
    let path = format!("{}/versions", script_path(&script));
    let mut held_lock = database.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM scripts WHERE id = $1 FOR UPDATE")
        .bind(script_id)
        .fetch_one(&mut *held_lock)
        .await
        .unwrap();
    let first_body = save_request(1, changed_content("Bản chỉnh sửa từ cửa sổ thứ nhất."));
    let second_body = save_request(1, changed_content("Bản chỉnh sửa từ cửa sổ thứ hai."));
    let mut tasks = Vec::new();
    for body in [first_body.clone(), second_body.clone()] {
        let app = database.app();
        let path = path.clone();
        tasks.push(tokio::spawn(async move {
            request(app, "POST", &path, Some(&token('a')), Some(&body)).await
        }));
    }
    // Both real transactions must reach the database lock before either can commit.
    // This proves the interleaving instead of relying on scheduler timing or sleeps.
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity
                 WHERE datname = $1 AND wait_event_type = 'Lock'",
            )
            .bind(&database.name)
            .fetch_one(&database.admin)
            .await
            .unwrap();
            if waiting >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("both competing writes blocked on the explicitly held script lock");
    held_lock.commit().await.unwrap();
    let first = tasks.remove(0).await.unwrap();
    let second = tasks.remove(0).await.unwrap();
    let (winner, conflict) = match (first.0, second.0) {
        (StatusCode::OK, StatusCode::CONFLICT) => (&first.1, &second.1),
        (StatusCode::CONFLICT, StatusCode::OK) => (&second.1, &first.1),
        statuses => panic!("one commit and one conflict required, got {statuses:?}"),
    };
    assert_error(conflict, "conflict");
    assert_eq!(conflict["error"]["head_version"], 2);
    assert_eq!(winner["version"], 2);
    assert_eq!(winner["base_version"], 1);
    assert_eq!(database.row_count("script_versions").await, 2);
    let reopened = owner_request(
        &database,
        "GET",
        &script_path(&script),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(&reopened["head"], winner);
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn retried_create_and_save_return_original_results_after_head_advances() {
    let database = TestDatabase::create().await;
    let create = create_request();
    let script = owner_request(
        &database,
        "POST",
        "/v1/scripts",
        Some(&create),
        StatusCode::CREATED,
    )
    .await;
    let path = script_path(&script);
    let edit = save_request(1, changed_content("Bản chỉnh sửa được gửi lại."));
    let saved = owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&edit),
        StatusCode::OK,
    )
    .await;
    owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&save_request(
            2,
            changed_content("Bản chỉnh sửa tiếp theo."),
        )),
        StatusCode::OK,
    )
    .await;
    let retried_save = owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&edit),
        StatusCode::OK,
    )
    .await;
    assert_eq!(retried_save, saved);
    let retried_create = owner_request(
        &database,
        "POST",
        "/v1/scripts",
        Some(&create),
        StatusCode::CREATED,
    )
    .await;
    assert_eq!(retried_create["id"], script["id"]);
    assert_eq!(retried_create["head"], script["head"]);
    let mut altered_create = create;
    altered_create["source"]["original_text"] = json!("different source");
    let error = owner_request(
        &database,
        "POST",
        "/v1/scripts",
        Some(&altered_create),
        StatusCode::CONFLICT,
    )
    .await;
    assert_error(&error, "operation_reused");
    for field in ["content", "base_version"] {
        let mut altered = edit.clone();
        altered[field] = if field == "content" {
            changed_content("Nội dung khác cùng operation ID.")
        } else {
            json!(3)
        };
        let error = owner_request(
            &database,
            "POST",
            &format!("{path}/versions"),
            Some(&altered),
            StatusCode::CONFLICT,
        )
        .await;
        assert_error(&error, "operation_reused");
    }
    assert_eq!(database.row_count("scripts").await, 1);
    assert_eq!(database.row_count("script_versions").await, 3);
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn accepted_revision_stays_pinned_and_acceptance_retries_do_not_create_duplicates() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let path = script_path(&script);
    let accept = accept_request(1);
    let revision = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept),
        StatusCode::OK,
    )
    .await;
    assert_eq!(revision["version"], 1);
    assert_eq!(revision["content"], script["head"]["content"]);
    assert_eq!(revision["digest"], script["head"]["digest"]);
    assert_eq!(revision["actor_id"], OWNER.to_string());
    assert_eq!(revision["operation_id"], accept["operation_id"]);
    let same_version = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept_request(1)),
        StatusCode::OK,
    )
    .await;
    assert_eq!(same_version, revision);
    owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&save_request(
            1,
            changed_content("Lời mới không thay đổi bản đã chấp nhận."),
        )),
        StatusCode::OK,
    )
    .await;
    let retry = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept),
        StatusCode::OK,
    )
    .await;
    assert_eq!(retry, revision);
    let existing_old_revision = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept_request(1)),
        StatusCode::OK,
    )
    .await;
    assert_eq!(existing_old_revision, revision);
    owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&save_request(
            2,
            changed_content("Bản thứ ba chưa được chấp nhận."),
        )),
        StatusCode::OK,
    )
    .await;
    let stale = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept_request(2)),
        StatusCode::CONFLICT,
    )
    .await;
    assert_error(&stale, "conflict");
    assert_eq!(stale["error"]["head_version"], 3);
    let mut reused = accept;
    reused["version"] = json!(2);
    let error = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&reused),
        StatusCode::CONFLICT,
    )
    .await;
    assert_error(&error, "operation_reused");
    let pinned = owner_request(
        &database,
        "GET",
        &format!("{path}/revisions/{}", revision["id"].as_str().unwrap()),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(pinned, revision);
    let history = owner_request(
        &database,
        "GET",
        &format!("{path}/history?after=0&limit=50"),
        None,
        StatusCode::OK,
    )
    .await;
    assert_eq!(history["revisions"], json!([revision]));
    assert_eq!(database.row_count("script_revisions").await, 1);
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn database_rejects_update_delete_and_truncate_of_settled_records() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let path = script_path(&script);
    owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept_request(1)),
        StatusCode::OK,
    )
    .await;
    for table in [
        "source_records",
        "rights_records",
        "script_versions",
        "script_revisions",
        "revision_operations",
    ] {
        let before = database.row_count(table).await;
        assert_eq!(before, 1, "seeded settled table {table}");
        for statement in [
            format!("UPDATE {table} SET script_id = script_id"),
            format!("DELETE FROM {table}"),
            format!("TRUNCATE {table} CASCADE"),
        ] {
            let error = sqlx::query(&statement)
                .execute(&database.pool)
                .await
                .expect_err("settled rows must reject mutation even by the migration owner");
            assert_eq!(
                error
                    .as_database_error()
                    .and_then(|error| error.code())
                    .as_deref(),
                Some("23514"),
                "{statement}: {error}"
            );
            assert_eq!(database.row_count(table).await, before);
        }
    }
    let id = Uuid::parse_str(script["id"].as_str().unwrap()).unwrap();
    for statement in [
        "UPDATE scripts SET owner_id = $2 WHERE id = $1",
        "UPDATE scripts SET id = $2 WHERE id = $1",
    ] {
        let error = sqlx::query(statement)
            .bind(id)
            .bind(OTHER)
            .execute(&database.pool)
            .await
            .expect_err("script identity and owner must remain immutable");
        assert_eq!(
            error
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref(),
            Some("23514")
        );
    }
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn invalid_scripts_and_changed_source_identity_do_not_write_or_advance_head() {
    let database = TestDatabase::create().await;
    let mut invalid = create_request();
    invalid["content"]["schema_version"] = json!("999.0.0");
    let error = owner_request(
        &database,
        "POST",
        "/v1/scripts",
        Some(&invalid),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    assert_error(&error, "invalid_script");
    for table in [
        "scripts",
        "source_records",
        "rights_records",
        "script_versions",
    ] {
        assert_eq!(database.row_count(table).await, 0, "{table}");
    }
    let script = create_script(&database).await;
    let path = script_path(&script);
    let mut invalid_content = fixture();
    invalid_content["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["speaker_id"] =
        json!("unknown-speaker");
    let error = owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&save_request(1, invalid_content)),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    assert_error(&error, "invalid_script");
    let mut changed_source = fixture();
    changed_source["provenance"][0]["source_record_id"] = json!("different-source");
    let error = owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&save_request(1, changed_source)),
        StatusCode::UNPROCESSABLE_ENTITY,
    )
    .await;
    assert_error(&error, "identity_changed");
    let reopened = owner_request(&database, "GET", &path, None, StatusCode::OK).await;
    assert_eq!(reopened, script);
    assert_eq!(database.row_count("script_versions").await, 1);
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn injected_version_insert_failure_rolls_back_and_the_same_operation_can_retry() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let path = script_path(&script);
    sqlx::raw_sql(
        "CREATE FUNCTION test_reject_version_insert() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'test fault before version insert'; END; $$;
         CREATE TRIGGER test_insert_failure BEFORE INSERT ON script_versions
         FOR EACH ROW EXECUTE FUNCTION test_reject_version_insert();",
    )
    .execute(&database.pool)
    .await
    .unwrap();
    let edit = save_request(
        1,
        changed_content("Lần thử lại được giữ nguyên operation ID."),
    );
    let error = owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&edit),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await;
    assert_error(&error, "storage_error");
    assert!(!error.to_string().contains("test fault"));
    assert_eq!(database.row_count("script_versions").await, 1);
    let reopened = owner_request(&database, "GET", &path, None, StatusCode::OK).await;
    assert_eq!(reopened, script);
    sqlx::query("DROP TRIGGER test_insert_failure ON script_versions")
        .execute(&database.pool)
        .await
        .unwrap();
    let saved = owner_request(
        &database,
        "POST",
        &format!("{path}/versions"),
        Some(&edit),
        StatusCode::OK,
    )
    .await;
    assert_eq!(saved["version"], 2);
    assert_eq!(saved["operation_id"], edit["operation_id"]);
    assert_eq!(database.row_count("script_versions").await, 2);
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn stored_content_corruption_is_rejected_when_loading_any_version_or_revision() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let path = script_path(&script);
    let revision = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept_request(1)),
        StatusCode::OK,
    )
    .await;
    // Only the disposable database administrator bypasses the trigger here.
    // This models corrupted storage, never an available application operation.
    sqlx::query("ALTER TABLE script_versions DISABLE TRIGGER immutable_versions")
        .execute(&database.pool)
        .await
        .unwrap();
    // Well-formed, so only re-verification against the document can expose it.
    let tampered = format!("sir-c1:sha256:{}", "0".repeat(64));
    sqlx::query("UPDATE script_versions SET digest = $1")
        .bind(&tampered)
        .execute(&database.pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE script_versions ENABLE TRIGGER immutable_versions")
        .execute(&database.pool)
        .await
        .unwrap();
    for route in [
        path.clone(),
        format!("{path}/versions/1"),
        format!("{path}/history?after=0&limit=50"),
        format!("{path}/revisions/{}", revision["id"].as_str().unwrap()),
    ] {
        let error = owner_request(
            &database,
            "GET",
            &route,
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .await;
        assert_error(&error, "storage_error");
        assert!(!error.to_string().contains(&tampered));
    }
    database.destroy().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn corrupted_source_and_rights_references_fail_even_when_performance_digest_is_unchanged() {
    let database = TestDatabase::create().await;
    let script = create_script(&database).await;
    let path = script_path(&script);
    let revision = owner_request(
        &database,
        "POST",
        &format!("{path}/revisions"),
        Some(&accept_request(1)),
        StatusCode::OK,
    )
    .await;
    let original = script["head"]["content"].clone();
    let before: (Vec<u8>, String, Vec<u8>) =
        sqlx::query_as("SELECT canonical, digest, document_digest FROM script_versions")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    for reference in ["/work/rights_record_id", "/provenance/0/source_record_id"] {
        let mut corrupted = original.clone();
        *corrupted.pointer_mut(reference).unwrap() = json!("corrupted-reference");
        // A canonical document, so only the recorded document digest can expose it.
        let domain =
            cantos_server::script_ir::read_script(&serde_json::to_vec(&corrupted).unwrap())
                .unwrap();
        let corrupted = domain.export_bytes();
        assert_eq!(domain.canonical_bytes(), before.0);
        assert_eq!(domain.content_digest().to_string(), before.1);
        // Inject corruption through the disposable administrator only. The
        // semantic checksum cannot detect these envelope reference changes.
        sqlx::query("ALTER TABLE script_versions DISABLE TRIGGER immutable_versions")
            .execute(&database.pool)
            .await
            .unwrap();
        sqlx::query("UPDATE script_versions SET document = $1")
            .bind(corrupted)
            .execute(&database.pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE script_versions ENABLE TRIGGER immutable_versions")
            .execute(&database.pool)
            .await
            .unwrap();
        let after: (Vec<u8>, String, Vec<u8>) =
            sqlx::query_as("SELECT canonical, digest, document_digest FROM script_versions")
                .fetch_one(&database.pool)
                .await
                .unwrap();
        assert_eq!(after, before);
        for route in [
            path.clone(),
            format!("{path}/versions/1"),
            format!("{path}/history?after=0&limit=50"),
            format!("{path}/revisions/{}", revision["id"].as_str().unwrap()),
        ] {
            let error = owner_request(
                &database,
                "GET",
                &route,
                None,
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .await;
            assert_error(&error, "storage_error");
            assert!(!error.to_string().contains("corrupted-reference"));
        }
    }
    database.destroy().await;
}

fn cli(database_url: &str, arguments: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_cantos-server"))
        .env("DATABASE_URL", database_url)
        .env("TOKIO_WORKER_THREADS", "2")
        .args(arguments)
        .output()
        .expect("execute actual operator CLI");
    // Never include captured output in diagnostics: issue-token writes a bearer
    // credential to stdout as its explicit operator-facing result.
    assert!(output.status.success(), "operator CLI must succeed");
    output
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn operator_cli_issues_expiring_hashed_credentials_and_revocation_blocks_http_access() {
    let database = TestDatabase::create().await;
    let created = cli(&database.url, &["create-creator"]);
    let creator_text = String::from_utf8(created.stdout).unwrap();
    let creator = Uuid::parse_str(creator_text.trim()).unwrap();
    let before: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    let issued = cli(&database.url, &["issue-token", creator_text.trim(), "1"]);
    let credential_output = String::from_utf8(issued.stdout).unwrap();
    let credential = credential_output.trim();
    assert_eq!(credential.len(), 64);
    assert!(
        credential
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    let after: chrono::DateTime<chrono::Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&database.pool)
        .await
        .unwrap();
    let (stored_hash, expires_in_one_hour, not_revoked): (Vec<u8>, bool, bool) = sqlx::query_as(
        "SELECT token_hash,
                expires_at BETWEEN $2::timestamptz + interval '1 hour'
                               AND $3::timestamptz + interval '1 hour',
                revoked_at IS NULL
         FROM creator_tokens WHERE creator_id = $1",
    )
    .bind(creator)
    .bind(before)
    .bind(after)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(stored_hash.len(), 32);
    assert!(stored_hash == Sha256::digest(credential.as_bytes()).to_vec());
    assert!(expires_in_one_hour);
    assert!(not_revoked);
    let (status, script) = request(
        database.app(),
        "POST",
        "/v1/scripts",
        Some(credential),
        Some(&create_request()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(script["owner_id"], creator.to_string());
    assert_eq!(script["head"]["actor_id"], creator.to_string());
    let path = script_path(&script);
    let (status, reopened) = request(database.app(), "GET", &path, Some(credential), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reopened, script);
    cli(&database.url, &["revoke-tokens", creator_text.trim()]);
    let (status, error) = request(database.app(), "GET", &path, Some(credential), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_error(&error, "unauthenticated");
    let revoked: bool = sqlx::query_scalar(
        "SELECT revoked_at IS NOT NULL FROM creator_tokens WHERE creator_id = $1",
    )
    .bind(creator)
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert!(revoked);
    database.destroy().await;
}

struct RunningServer {
    child: Child,
    origin: String,
}

impl RunningServer {
    async fn start(database_url: &str) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let child = Command::new(env!("CARGO_BIN_EXE_cantos-server"))
            .env("DATABASE_URL", database_url)
            .env("CANTOS_BIND_ADDRESS", address.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start actual backend process");
        let mut server = Self {
            child,
            origin: format!("http://{address}"),
        };
        let client = reqwest::Client::new();
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                assert!(
                    server.child.try_wait().unwrap().is_none(),
                    "backend exited before readiness"
                );
                if let Ok(response) = client
                    .get(format!("{}/healthz", server.origin))
                    .send()
                    .await
                    && response.status().is_success()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("backend reaches its real HTTP readiness endpoint");
        server
    }

    fn stop(&mut self) {
        self.child.kill().expect("terminate backend process");
        self.child.wait().expect("reap terminated backend process");
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        // Also avoid orphaning a child when an assertion fails.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run --test persistence -- --ignored"]
async fn committed_script_revision_source_and_audit_survive_a_real_backend_restart() {
    let database = TestDatabase::create().await;
    let client = reqwest::Client::new();
    let mut first = RunningServer::start(&database.url).await;
    let response = client
        .post(format!("{}/v1/scripts", first.origin))
        .bearer_auth(token('a'))
        .json(&create_request())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let script: Value = response.json().await.unwrap();
    let path = script_path(&script);
    let response = client
        .post(format!("{}{path}/revisions", first.origin))
        .bearer_auth(token('a'))
        .json(&accept_request(1))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let revision: Value = response.json().await.unwrap();
    first.stop();
    let mut restarted = RunningServer::start(&database.url).await;
    let reopened: Value = client
        .get(format!("{}{path}", restarted.origin))
        .bearer_auth(token('a'))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(reopened, script);
    let history: Value = client
        .get(format!(
            "{}{path}/history?after=0&limit=50",
            restarted.origin
        ))
        .bearer_auth(token('a'))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history["versions"], json!([script["head"]]));
    assert_eq!(history["revisions"], json!([revision]));
    restarted.stop();
    database.destroy().await;
}
