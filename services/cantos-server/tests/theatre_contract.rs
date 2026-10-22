//! Actual HTTP host protocol tests. Lazy unavailable PG is not database acceptance.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use cantos_server::adapters::inbound::http::{router, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
fn app() -> axum::Router {
    router(
        AppState {
            store: cantos_server::test_support::detached_store(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "unused-public-root",
    )
}
fn schema(name: &str, v: &Value) {
    let root: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/theatre/discovery/v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(
        &json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{name}")}),
    )
    .unwrap();
    assert!(validator.is_valid(v), "{v}");
}
#[tokio::test]
async fn public_routes_use_theatre_errors_and_do_not_reuse_studio_cookies() {
    for path in [
        "/works?limit=0",
        "/works?kind=fantasy",
        "/search?unknown=1",
        "/works?cursor=plaintext-private-id",
        "/works/not-an-id",
        "/works/not-an-id/units",
        "/publications/not-an-id",
    ] {
        for cookie in [false, true] {
            let mut req = Request::builder().uri(format!("/api/v1/theatre{path}"));
            if cookie {
                req = req.header("cookie", "cantos_session=x; cantos_session=y");
            }
            let r = app()
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::BAD_REQUEST, "{path}");
            assert_eq!(r.headers()["cache-control"], "no-store");
            let v: Value =
                serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
            schema("ListenerError", &v);
            assert_eq!(v["retryable"], false);
        }
    }
}
#[tokio::test]
async fn unavailable_database_never_returns_mock_catalog_or_a_signed_url() {
    use cantos_server::test_support::ServerFixture;
    let list = |app: axum::Router| async move {
        app.oneshot(
            Request::builder()
                .uri("/api/v1/theatre/works")
                .header("cookie", "cantos_session=ignored")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    };
    let fixture = ServerFixture::new().await;
    let app = || {
        router(
            AppState {
                store: fixture.store().clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "unused-public-root",
        )
    };
    // A healthy, empty `server.db` is an honest empty catalog, never sample data.
    let r = list(app()).await;
    assert_eq!(r.status(), StatusCode::OK);
    let v: Value =
        serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v, json!({"items":[],"next_cursor":null}));
    schema("WorkPage", &v);
    // A database that cannot answer is a typed, retryable unavailability: no empty-success
    // fallback, no mock catalog, no signed URL.
    fixture
        .exec("DROP TABLE audio_listener_releases", &[])
        .await
        .unwrap();
    let r = list(app()).await;
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    let v: Value =
        serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v, json!({"code":"unavailable","retryable":true}));
    schema("ListenerError", &v);
    let r = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/theatre/publications/00000000-0000-4000-8000-000000000001/playback-authorizations")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = r.into_body().collect().await.unwrap().to_bytes();
    assert!(
        !String::from_utf8_lossy(&body).contains("https://"),
        "no signed URL"
    );
}
#[tokio::test]
async fn playback_rejects_raw_authority_and_has_no_studio_or_legacy_alias() {
    for base in ["/api/v1/theatre", "/api/v1/studio", "/api/v1"] {
        let r = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "{base}/publications/not-an-id/playback-authorizations"
                    ))
                    .header("origin", "http://127.0.0.1:8080")
                    .body(Body::from("{\"owner_id\":\"alice\",\"passed\":true}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        if base == "/api/v1/theatre" {
            assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        } else {
            assert_eq!(r.status(), StatusCode::NOT_FOUND);
        }
    }
}

#[tokio::test]
async fn native_public_playback_needs_no_studio_origin_and_raw_body_stays_bounded() {
    for origin in [None, Some("https://another-listener.invalid")] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/theatre/publications/not-an-id/playback-authorizations");
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        let r = app()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        let v: Value =
            serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(v, json!({"code":"invalid_request","retryable":false}));
    }
    let r = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/theatre/publications/not-an-id/playback-authorizations")
                .body(Body::from(vec![b'a'; 32769]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let v: Value =
        serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
    schema("ListenerError", &v);
}
