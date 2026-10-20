//! The deployed runtime grant file (`infra/vps/postgres/runtime-grants.sql`) against every
//! migration: it must apply as the database owner, enable real Studio and Theatre requests for
//! `cantos_app` on its own, and leave excluded surfaces without privileges.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_api::{
    interfaces::http::{router, AppState},
    platform::postgres::{local_config, migrate, token_hash, Store},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::env;
use tokio_postgres::{Client, Config, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

const GRANTS: &str = include_str!("../../../infra/vps/postgres/runtime-grants.sql");
const REGISTRATION_GRANTS: &str =
    include_str!("../../../infra/vps/postgres/theatre-registration-grants.sql");
const REGISTRATION_REVOKE: &str =
    include_str!("../../../infra/vps/postgres/theatre-registration-revoke.sql");
const ORIGIN: &str = "http://127.0.0.1:8080";
const TOKEN: &str = "5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e5a1e";

async fn connect(config: &Config) -> Client {
    let (client, io) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = io.await;
    });
    client
}

async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, format!("cantos_session={TOKEN}"));
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let body = body.map_or_else(Body::empty, |body| Body::from(body.to_string()));
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn work_request() -> Value {
    json!({
        "operation_id": Uuid::new_v4().to_string(), "id": Uuid::new_v4().to_string(), "kind": "novel",
        "details": {
            "title": "Tiếng đàn bên sông", "synopsis": "Tác phẩm kiểm tra quyền triển khai.",
            "language": "vi-VN", "visibility": "private", "rights_refs": []
        }
    })
}

#[tokio::test]
#[ignore = "requires cargo xtask verify postgres"]
async fn deployed_runtime_grants_alone_serve_studio_and_theatre_requests() {
    let mut config =
        local_config(&env::var("CANTOS_TEST_CLUSTER_URL").expect("cargo xtask verify postgres"))
            .unwrap();
    let database = format!("cantos_test_{}", Uuid::new_v4().simple());
    connect(&config)
        .await
        .batch_execute(&format!("CREATE DATABASE {database}"))
        .await
        .unwrap();
    config.dbname(&database);
    migrate(&config).await.unwrap();
    let owner = connect(&config).await;
    owner
        .execute("INSERT INTO actors(id) VALUES('grant-probe')", &[])
        .await
        .unwrap();
    owner
        .execute(
            "INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,'grant-probe',clock_timestamp()+interval '1 hour')",
            &[&token_hash(TOKEN)],
        )
        .await
        .unwrap();

    let mut app_config = config.clone();
    app_config
        .user("cantos_app")
        .application_name("cantos-production-grants-test");
    let app = router(
        AppState {
            store: Store::new(app_config).unwrap(),
            web_origin: ORIGIN.into(),
        },
        "unused-dist",
    );

    // Non-vacuity: without the file the runtime role cannot even authenticate a session.
    let (status, _) = call(&app, "POST", "/api/v1/studio/works", Some(work_request())).await;
    assert_ne!(status, StatusCode::CREATED);

    owner.batch_execute(GRANTS).await.unwrap();
    owner
        .batch_execute(GRANTS)
        .await
        .expect("grants are idempotent");

    let (status, created) = call(&app, "POST", "/api/v1/studio/works", Some(work_request())).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let work = created["work"].clone();
    let unit = json!({
        "operation_id": Uuid::new_v4().to_string(), "id": Uuid::new_v4().to_string(), "kind": "chapter",
        "expected_work_revision": work["revision"],
        "details": {"title": "Chương một", "description": "Chưa có âm thanh."}
    });
    let path = format!(
        "/api/v1/studio/works/{}/content-units",
        work["id"].as_str().unwrap()
    );
    let (status, body) = call(&app, "POST", &path, Some(unit)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, listed) = call(&app, "GET", "/api/v1/studio/works", None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert!(
        listed.to_string().contains("Tiếng đàn bên sông"),
        "{listed}"
    );

    // Anonymous public discovery reads the listener projection; a private Work never appears.
    let (status, public) = call(&app, "GET", "/api/v1/theatre/works", None).await;
    assert_eq!(status, StatusCode::OK, "{public}");
    assert!(
        !public.to_string().contains("Tiếng đàn bên sông"),
        "{public}"
    );

    for retired in [
        "durable_jobs",
        "speech_clips",
        "adaptation_attempts",
        "adaptation_observations",
    ] {
        assert!(
            owner
                .query_one("SELECT to_regclass($1)::text IS NULL", &[&retired])
                .await
                .unwrap()
                .get::<_, bool>(0),
            "retired relation {retired} must be absent"
        );
    }

    for (table, privilege) in [
        ("admin_audit", "SELECT"),
        ("admin_access_revocations", "INSERT"),
        ("actor_external_identities", "SELECT"),
        ("theatre_registrations", "SELECT"),
        ("theatre_firebase_enrollments", "SELECT"),
        ("theatre_native_sessions", "SELECT"),
        ("actors", "INSERT"),
        ("sessions", "INSERT"),
        ("catalog_works", "DELETE"),
        ("catalog_works", "TRUNCATE"),
        ("script_revisions", "UPDATE"),
        ("listener_bookmarks", "DELETE"),
        ("listener_bookmarks", "TRUNCATE"),
    ] {
        let granted: bool = owner
            .query_one(
                "SELECT has_table_privilege('cantos_app', $1, $2)",
                &[&table, &privilege],
            )
            .await
            .unwrap()
            .get(0);
        assert!(!granted, "cantos_app must not hold {privilege} on {table}");
    }
    // Release-bound bookmarks (migration 0003): create-once rows whose only change is the final tombstone.
    for privilege in ["SELECT", "INSERT"] {
        assert!(
            table_privilege(&owner, "listener_bookmarks", privilege).await,
            "the deployed grants must give cantos_app {privilege} on listener_bookmarks"
        );
    }
    for column in [
        "deleted",
        "listener_order",
        "accepted_at",
        "snapshot",
        "snapshot_digest",
    ] {
        assert!(
            column_update(&owner, "listener_bookmarks", column).await,
            "UPDATE({column}) on listener_bookmarks"
        );
    }
    for column in [
        "listener_id",
        "bookmark_id",
        "publication_id",
        "work_id",
        "unit_id",
        "rendition_id",
        "sha256",
        "duration_ms",
        "position_ms",
    ] {
        assert!(
            !column_update(&owner, "listener_bookmarks", column).await,
            "the immutable column {column} is never updatable"
        );
    }
    let creates: bool = owner
        .query_one(
            "SELECT has_schema_privilege('cantos_app','public','CREATE')",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(!creates, "cantos_app must not create schema objects");
}

async fn table_privilege(owner: &Client, table: &str, privilege: &str) -> bool {
    owner
        .query_one(
            "SELECT has_table_privilege('cantos_app', $1, $2)",
            &[&table, &privilege],
        )
        .await
        .unwrap()
        .get(0)
}

async fn column_update(owner: &Client, table: &str, column: &str) -> bool {
    owner
        .query_one(
            "SELECT has_column_privilege('cantos_app', $1, $2, 'UPDATE')",
            &[&table, &column],
        )
        .await
        .unwrap()
        .get(0)
}

/// The optional registration block is separate from the base file, grants only what the
/// Firebase enrollment repository executes, and is withdrawn by the revoke counterpart.
#[tokio::test]
#[ignore = "requires cargo xtask verify postgres"]
async fn registration_grants_are_separate_minimal_idempotent_and_revocable() {
    let mut config =
        local_config(&env::var("CANTOS_TEST_CLUSTER_URL").expect("cargo xtask verify postgres"))
            .unwrap();
    let database = format!("cantos_test_{}", Uuid::new_v4().simple());
    connect(&config)
        .await
        .batch_execute(&format!("CREATE DATABASE {database}"))
        .await
        .unwrap();
    config.dbname(&database);
    migrate(&config).await.unwrap();
    let owner = connect(&config).await;
    owner.batch_execute(GRANTS).await.unwrap();

    let enrollments = "theatre_firebase_enrollments";
    let starts = "theatre_firebase_send_starts";
    let receipts = "theatre_firebase_enrollment_receipts";
    // Non-vacuity: the base file alone leaves registration without any privilege.
    assert!(!table_privilege(&owner, enrollments, "SELECT").await);
    assert!(!table_privilege(&owner, starts, "INSERT").await);

    owner.batch_execute(REGISTRATION_GRANTS).await.unwrap();
    owner
        .batch_execute(REGISTRATION_GRANTS)
        .await
        .expect("registration grants are idempotent");
    for retired in [
        "durable_jobs",
        "speech_clips",
        "adaptation_attempts",
        "adaptation_observations",
    ] {
        assert!(
            owner
                .query_one("SELECT to_regclass($1)::text IS NULL", &[&retired])
                .await
                .unwrap()
                .get::<_, bool>(0),
            "retired relation {retired} must be absent"
        );
    }

    for (table, privilege) in [
        (enrollments, "SELECT"),
        (enrollments, "INSERT"),
        (starts, "SELECT"),
        (starts, "INSERT"),
    ] {
        assert!(
            table_privilege(&owner, table, privilege).await,
            "{privilege} on {table}"
        );
    }
    for column in ["phase", "last_sent_at", "person_id", "block_reason"] {
        assert!(column_update(&owner, enrollments, column).await, "{column}");
    }
    // Binding, lifetime and identity columns stay immutable for the runtime role.
    for column in [
        "namespace",
        "id",
        "project",
        "uid",
        "email",
        "email_key",
        "accepted_at",
        "expires_at",
    ] {
        assert!(
            !column_update(&owner, enrollments, column).await,
            "{column}"
        );
    }
    for retired in [
        "durable_jobs",
        "speech_clips",
        "adaptation_attempts",
        "adaptation_observations",
    ] {
        assert!(
            owner
                .query_one("SELECT to_regclass($1)::text IS NULL", &[&retired])
                .await
                .unwrap()
                .get::<_, bool>(0),
            "retired relation {retired} must be absent"
        );
    }

    for (table, privilege) in [
        (enrollments, "DELETE"),
        (enrollments, "TRUNCATE"),
        (enrollments, "UPDATE"),
        (starts, "UPDATE"),
        (starts, "DELETE"),
        (starts, "TRUNCATE"),
        (receipts, "SELECT"),
        (receipts, "INSERT"),
        (receipts, "UPDATE"),
        (receipts, "DELETE"),
        ("theatre_registrations", "SELECT"),
        ("theatre_registration_mail_starts", "INSERT"),
        ("admin_audit", "SELECT"),
        ("actors", "INSERT"),
    ] {
        assert!(
            !table_privilege(&owner, table, privilege).await,
            "cantos_app must not hold {privilege} on {table}"
        );
    }
    for (sequence, expected) in [
        ("theatre_firebase_send_starts_receipt_id_seq", true),
        ("theatre_firebase_enrollment_receipts_receipt_id_seq", false),
    ] {
        let usage: bool = owner
            .query_one(
                "SELECT has_sequence_privilege('cantos_app', $1, 'USAGE')",
                &[&sequence],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(usage, expected, "{sequence}");
    }
    for function in [
        "theatre_firebase_enrollment_guard()",
        "theatre_firebase_enrollment_receipt()",
    ] {
        let execute: bool = owner
            .query_one(
                "SELECT has_function_privilege('cantos_app', $1, 'EXECUTE')",
                &[&function],
            )
            .await
            .unwrap()
            .get(0);
        assert!(!execute, "{function}");
    }
    let creates: bool = owner
        .query_one(
            "SELECT has_schema_privilege('cantos_app','public','CREATE')",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(!creates);

    owner.batch_execute(REGISTRATION_REVOKE).await.unwrap();
    owner
        .batch_execute(REGISTRATION_REVOKE)
        .await
        .expect("registration revoke is idempotent");
    for retired in [
        "durable_jobs",
        "speech_clips",
        "adaptation_attempts",
        "adaptation_observations",
    ] {
        assert!(
            owner
                .query_one("SELECT to_regclass($1)::text IS NULL", &[&retired])
                .await
                .unwrap()
                .get::<_, bool>(0),
            "retired relation {retired} must be absent"
        );
    }

    for (table, privilege) in [(enrollments, "SELECT"), (starts, "INSERT")] {
        assert!(!table_privilege(&owner, table, privilege).await);
    }
    assert!(!column_update(&owner, enrollments, "phase").await);
    // The base grants are untouched by the revoke counterpart.
    assert!(table_privilege(&owner, "catalog_works", "INSERT").await);
}
