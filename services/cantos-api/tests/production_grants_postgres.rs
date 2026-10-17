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

    for (table, privilege) in [
        ("admin_audit", "SELECT"),
        ("admin_access_revocations", "INSERT"),
        ("actor_external_identities", "SELECT"),
        ("durable_jobs", "SELECT"),
        ("speech_clips", "SELECT"),
        ("theatre_registrations", "SELECT"),
        ("theatre_firebase_enrollments", "SELECT"),
        ("theatre_native_sessions", "SELECT"),
        ("actors", "INSERT"),
        ("sessions", "INSERT"),
        ("catalog_works", "DELETE"),
        ("catalog_works", "TRUNCATE"),
        ("script_revisions", "UPDATE"),
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
