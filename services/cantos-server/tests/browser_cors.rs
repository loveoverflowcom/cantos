//! Real API router composition; the capability handler must not dial the unavailable database.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_contracts::web_api::WebSurface;
use cantos_server::adapters::inbound::http::with_browser_cors;
use tower::ServiceExt;

#[tokio::test]
async fn each_cookie_surface_exposes_retry_after_without_exposing_credentials() {
    for (surface, base, origin, own_fence, other_fence) in [
        (
            WebSurface::Theatre,
            "/api/v1/theatre",
            "http://127.0.0.1:8082",
            "x-cantos-theatre-session",
            "x-cantos-studio-session",
        ),
        (
            WebSurface::Studio,
            "/api/v1/studio",
            "http://127.0.0.1:8081",
            "x-cantos-studio-session",
            "x-cantos-theatre-session",
        ),
    ] {
        let path = format!("{base}/identity/login");
        // Isolate the production CORS adapter from the limiter's clock/budget. This is an
        // HTTP boundary regression, not a successful Identity or browser journey.
        let router = axum::Router::new().route(
            &path,
            axum::routing::post(|| async {
                (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "73")])
            }),
        );
        let app = with_browser_cors(router, origin, surface).unwrap();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&path)
                    .header(header::ORIGIN, origin)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[header::RETRY_AFTER], "73");
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            origin
        );
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_CREDENTIALS],
            "true"
        );
        let exposed: Vec<_> = response.headers()[header::ACCESS_CONTROL_EXPOSE_HEADERS]
            .to_str()
            .unwrap()
            .split(',')
            .map(str::trim)
            .collect();
        assert!(exposed.contains(&"retry-after"), "{surface:?}: {exposed:?}");
        assert!(exposed.contains(&own_fence));
        for secret_or_foreign in ["*", "set-cookie", "authorization", other_fence] {
            assert!(!exposed.contains(&secret_or_foreign));
        }
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&path)
                    .header(header::ORIGIN, "http://localhost:8082")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
        assert!(!response
            .headers()
            .contains_key(header::ACCESS_CONTROL_EXPOSE_HEADERS));
    }
}

#[tokio::test]
async fn production_api_theatre_handler_is_readable_but_other_surface_stays_closed() {
    let state = cantos_server::adapters::inbound::http::AppState {
        store: cantos_server::test_support::detached_store().without_development_sessions(),
        web_origin: "https://cantoss-theatre.web.app".into(),
    };
    let app = with_browser_cors(
        cantos_server::adapters::inbound::http::api_router(state),
        "https://cantoss-theatre.web.app",
        WebSurface::Theatre,
    )
    .unwrap();
    // The Library refuses a request with no Identity credential before any database access, so
    // this is a real handler answer on the unavailable pool, with the surface's CORS headers.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/theatre/library/continue")
                .header(header::ORIGIN, "https://cantoss-theatre.web.app")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://cantoss-theatre.web.app"
    );
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let error: cantos_contracts::listener_library::LibraryError =
        serde_json::from_slice(&body).unwrap();
    assert_eq!(
        error.code,
        cantos_contracts::listener_library::ErrorCode::Unauthenticated
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/works")
                .header(header::ORIGIN, "https://cantoss-theatre.web.app")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(!response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
}
