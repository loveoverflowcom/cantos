//! Real API router composition; the capability handler must not dial the unavailable database.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_api::adapters::inbound::http::with_browser_cors;
use cantos_contracts::web_api::WebSurface;
use tower::ServiceExt;

#[tokio::test]
async fn production_api_capability_handler_is_readable_but_other_surface_stays_closed() {
    use cantos_api::platform::postgres::{local_config, Store};
    let state = cantos_api::adapters::inbound::http::AppState {
        store: Store::new(
            local_config("host=127.0.0.1 port=1 user=cantos_test dbname=cantos_test").unwrap(),
        )
        .unwrap()
        .without_development_sessions(),
        web_origin: "https://cantoss-theatre.web.app".into(),
    };
    let app = with_browser_cors(
        cantos_api::adapters::inbound::http::api_router(state),
        "https://cantoss-theatre.web.app",
        WebSurface::Theatre,
    )
    .unwrap();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/theatre/auth/configuration")
                .header(header::ORIGIN, "https://cantoss-theatre.web.app")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 8192)
        .await
        .unwrap();
    let configuration: cantos_contracts::theatre_identity::Configuration =
        serde_json::from_slice(&body).unwrap();
    assert_eq!(
        configuration.login,
        cantos_contracts::theatre_identity::LoginMethod::Unconfigured
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/auth/configuration")
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
