use super::*;
use super::{errors::error_response, policy::origin_and_cache};
use crate::application::{Store, StoreError};
use crate::contexts::media::read::{StorageCapability, StorageError};
use crate::contract::studio::ErrorCode;
use axum::{body::Body, routing::get};
use axum::{
    extract::Request,
    http::{header, HeaderValue, StatusCode},
    middleware,
    response::IntoResponse,
    Router,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn middleware_state() -> AppState {
    AppState {
        store: crate::test_support::detached_store(),
        web_origin: "http://127.0.0.1:8080".into(),
    }
}

#[tokio::test]
async fn invalid_script_error_keeps_json_status_codes_paths_and_redaction() {
    let mut shape: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json"
    ))
    .unwrap();
    shape["work"]["id"] = serde_json::json!("private-id-sentinel!");
    for (input, issues) in [
        (
            "private-manuscript-sentinel".into(),
            serde_json::json!([{"path": "$", "rule": "InvalidDocument"}]),
        ),
        (
            r#"{"schema_version":"private-version-sentinel"}"#.into(),
            serde_json::json!([{"path": "$", "rule": "UnsupportedSchemaVersion"}]),
        ),
        (
            serde_json::to_string(&shape).unwrap(),
            serde_json::json!([{"path": "/work/id", "rule": "id_format"}]),
        ),
    ] {
        let error = crate::script_ir::read_script(input.as_bytes()).unwrap_err();
        let response = error_response(StoreError::InvalidScript(error));
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": "invalid_script", "current_revision": null, "issues": issues
            })
        );
    }
}

#[tokio::test]
async fn outer_middleware_preserves_private_audio_policy_for_non_audio_responses() {
    // Pool construction does not connect. These routes exercise only HTTP policy,
    // independently of authentication, storage and PostgreSQL acceptance.
    let state = middleware_state();
    for status in [
        StatusCode::OK,
        StatusCode::NOT_MODIFIED,
        StatusCode::BAD_REQUEST,
        StatusCode::PRECONDITION_FAILED,
        StatusCode::RANGE_NOT_SATISFIABLE,
        StatusCode::SERVICE_UNAVAILABLE,
    ] {
        let app = Router::new()
            .route(
                "/api/v1/studio/fixture",
                get(move || async move {
                    let mut response = status.into_response();
                    if status != StatusCode::NOT_MODIFIED {
                        response.headers_mut().insert(
                            header::CONTENT_TYPE,
                            HeaderValue::from_static(if status == StatusCode::OK {
                                "audio/ogg"
                            } else {
                                "application/json"
                            }),
                        );
                    }
                    audio_transport::private_response(response)
                }),
            )
            .layer(middleware::from_fn_with_state(
                state.clone(),
                origin_and_cache,
            ));
        for method in [axum::http::Method::GET, axum::http::Method::HEAD] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/api/v1/studio/fixture")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "private, no-store, no-transform",
                "{status}"
            );
        }
    }
}

#[tokio::test]
async fn outer_middleware_does_not_trust_an_upstream_cacheable_policy() {
    let state = middleware_state();
    for policy in [None, Some("public, max-age=3600")] {
        let app = Router::new()
            .route(
                "/api/v1/studio/fixture",
                get(move || async move {
                    let mut response = StatusCode::OK.into_response();
                    if let Some(policy) = policy {
                        response
                            .headers_mut()
                            .insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
                    }
                    response
                }),
            )
            .layer(middleware::from_fn_with_state(
                state.clone(),
                origin_and_cache,
            ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/fixture")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
}

#[tokio::test]
async fn private_storage_errors_map_to_safe_existing_api_codes_without_internal_facts() {
    for (error, expected_status, expected_code) in [
        (
            StorageError::NotFound,
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
        ),
        (
            StorageError::Forbidden,
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
        ),
        (
            StorageError::Conflict,
            StatusCode::CONFLICT,
            ErrorCode::OperationReused,
        ),
        (
            StorageError::InvalidTarget,
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
        ),
        (
            StorageError::InvalidKey,
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
        ),
        (
            StorageError::InvalidIntegrity,
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
        ),
        (
            StorageError::DigestMismatch,
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
        ),
        (
            StorageError::LengthMismatch,
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
        ),
        (
            StorageError::Timeout,
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
        ),
        (
            StorageError::Cancelled,
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
        ),
        (
            StorageError::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
        ),
        (
            StorageError::UnsupportedCapability(StorageCapability::ConditionalCreate),
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
        ),
        (
            StorageError::UnsupportedCapability(StorageCapability::RangeRead),
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
        ),
        (
            StorageError::UnsupportedCapability(StorageCapability::TemporaryRead),
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
        ),
    ] {
        let response = error_response(StoreError::Storage(error));
        assert_eq!(response.status(), expected_status, "{error:?}");
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let actual: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            actual,
            serde_json::json!({
                "code": expected_code,
                "current_revision": null,
                "issues": [],
            }),
            "{error:?}"
        );
    }
}

fn production_state(store: Store) -> AppState {
    AppState {
        store,
        web_origin: "https://studio.cantos.example".into(),
    }
}

async fn send(app: &Router, request: axum::http::Request<Body>) -> (StatusCode, String) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

const DEV_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn production_host_offers_no_development_studio_credential() {
    // Every refusal precedes PostgreSQL: the unreachable pool proves no session lookup ran.
    let store = middleware_state().store.without_development_sessions();
    let app = api_router(production_state(store));

    let login = axum::http::Request::post("/api/v1/studio/session")
        .header(header::ORIGIN, "https://studio.cantos.example")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(r#"{{"token":"{DEV_TOKEN}"}}"#)))
        .unwrap();
    assert_eq!(send(&app, login).await.0, StatusCode::FORBIDDEN);

    let legacy = axum::http::Request::post("/api/v1/session")
        .header(header::ORIGIN, "https://studio.cantos.example")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!(r#"{{"token":"{DEV_TOKEN}"}}"#)))
        .unwrap();
    assert_eq!(send(&app, legacy).await.0, StatusCode::FORBIDDEN);

    let cookie = |uri: &str| {
        axum::http::Request::get(uri)
            .header(header::COOKIE, format!("cantos_session={DEV_TOKEN}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        send(&app, cookie("/api/v1/studio/session")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, cookie("/api/v1/studio/works")).await.0,
        StatusCode::UNAUTHORIZED
    );
}

/// Every route of the retired provider authentication (Kanidm/OIDC login and callback, the
/// provider-bound Bearer family, the legacy native public client, Firebase/SMTP registration and
/// the provider session views) is gone: a router that never configured any of them answers the
/// surface's `not_found`, never an `unavailable`/`unauthenticated` that implies a mounted path.
#[tokio::test]
async fn retired_provider_authentication_routes_are_not_mounted() {
    let app = api_router(production_state(middleware_state().store));
    let origin = "https://studio.cantos.example";
    for (method, path) in [
        ("GET", "/api/v1/studio/auth/configuration"),
        ("GET", "/api/v1/studio/auth/login"),
        ("GET", "/api/v1/studio/auth/callback?state=s&code=c"),
        ("POST", "/api/v1/studio/auth/bearer/flows"),
        ("POST", "/api/v1/studio/auth/bearer/refresh"),
        ("GET", "/api/v1/studio/auth/bearer/session"),
        ("GET", "/api/v1/theatre/auth/configuration"),
        ("GET", "/api/v1/theatre/auth/login"),
        ("GET", "/api/v1/theatre/auth/callback?state=s&code=c"),
        ("GET", "/api/v1/theatre/session"),
        ("DELETE", "/api/v1/theatre/session"),
        ("POST", "/api/v1/theatre/auth/bearer/flows"),
        ("GET", "/api/v1/theatre/auth/registration/configuration"),
        ("POST", "/api/v1/theatre/auth/registration/start"),
        ("POST", "/api/v1/theatre/native/auth/flows"),
        ("POST", "/api/v1/theatre/native/auth/bearer/flows"),
        ("GET", "/api/v1/theatre/native/session"),
        (
            "GET",
            "/api/v1/theatre/native/auth/registration/configuration",
        ),
    ] {
        let request = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header(header::ORIGIN, origin)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let (status, body) = send(&app, request).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {body}");
        assert!(body.contains("not_found"), "{method} {path}: {body}");
    }
}

#[tokio::test]
async fn api_router_serves_no_static_files() {
    let dist = std::env::temp_dir().join(format!("cantos-server-dist-{}", std::process::id()));
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(dist.join("index.html"), "<!doctype html>static").unwrap();
    let request = || {
        axum::http::Request::get("/index.html")
            .body(Body::empty())
            .unwrap()
    };

    let development = router(middleware_state(), dist.to_str().unwrap());
    let (status, body) = send(&development, request()).await;
    assert_eq!(
        (status, body.as_str()),
        (StatusCode::OK, "<!doctype html>static")
    );

    let production = api_router(production_state(middleware_state().store));
    let (status, body) = send(&production, request()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!body.contains("static"), "{body}");
    std::fs::remove_dir_all(dist).unwrap();
}
