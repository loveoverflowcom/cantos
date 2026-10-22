//! Real API router composition; the capability handler must not dial the unavailable database.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_contracts::web_api::WebSurface;
use cantos_server::adapters::inbound::http::with_browser_cors;
use tower::ServiceExt;

#[tokio::test]
async fn production_api_theatre_handler_is_readable_but_other_surface_stays_closed() {
    use cantos_server::platform::postgres::{local_config, Store};
    let state = cantos_server::adapters::inbound::http::AppState {
        store: Store::new(
            local_config("host=127.0.0.1 port=1 user=cantos_test dbname=cantos_test").unwrap(),
        )
        .unwrap()
        .without_development_sessions(),
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
