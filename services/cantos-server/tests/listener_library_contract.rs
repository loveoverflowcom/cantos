//! Actual Router/default-auth regressions with unavailable lazy PG. No account-sync claim.
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
#[tokio::test]
async fn every_account_route_rejects_guest_studio_cookie_raw_actor_and_dev_bearer_before_pg() {
    for (method, path) in [
        ("GET", "/library/continue"),
        ("GET", "/library/saved"),
        ("GET", "/library/saved/not-an-id"),
        ("PUT", "/library/saved/not-an-id"),
        ("GET", "/library/history"),
        ("POST", "/library/history/playback-starts"),
        ("POST", "/library/history/clear"),
        ("GET", "/progress/not-an-id"),
        ("PUT", "/progress/not-an-id"),
        ("GET", "/operations/not-an-id"),
    ] {
        for bearer in [
            None,
            Some("alice"),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            Some("fake.oidc.token"),
        ] {
            let mut r = Request::builder()
                .method(method)
                .uri(format!("/api/v1/theatre{path}"))
                .header("cookie", "cantos_session=studio-development-token")
                .header("content-type", "application/json");
            if let Some(b) = bearer {
                r = r.header("authorization", format!("Bearer {b}"));
            }
            let response = app()
                .oneshot(r.body(Body::from("{}")).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {path}"
            );
            assert_eq!(response.headers()["cache-control"], "no-store");
            let v: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(v, json!({"code":"unauthenticated","retryable":false}));
            let root: Value = serde_json::from_str(include_str!(
                "../../../services/cantos-server/contracts/theatre/library/v1/library.schema.json"
            ))
            .unwrap();
            assert!(jsonschema::validator_for(&root).unwrap().is_valid(&v));
        }
    }
}
#[tokio::test]
async fn library_has_no_studio_or_legacy_alias_and_native_posts_do_not_need_studio_origin() {
    for base in ["/api/v1/studio", "/api/v1"] {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri(format!("{base}/library/saved"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    for origin in [None, Some("https://listener-client.invalid")] {
        let mut r = Request::builder()
            .method("POST")
            .uri("/api/v1/theatre/library/history/clear");
        if let Some(o) = origin {
            r = r.header("origin", o);
        }
        let response = app().oneshot(r.body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/studio/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn duplicate_authorization_headers_and_oversized_json_never_create_identity() {
    for size in [2, 32769] {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/theatre/library/history/playback-starts")
                    .header("authorization", "Bearer alice")
                    .header("authorization", "Bearer bob")
                    .header("content-type", "application/json")
                    .body(Body::from(vec![b'x'; size]))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
