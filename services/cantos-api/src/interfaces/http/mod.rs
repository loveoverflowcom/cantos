//! Thin Axum adapter; all revision decisions live inward of this module.
//!
//! One host, three surfaces: `/api/v1/studio`, `/api/v1/theatre` and `/api/v1/admin`. This file
//! holds only what they share (state, authentication and origin middleware, error mapping).
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Json, Router,
};
use cantos_contracts::{routes, ApiError, ErrorCode};
use tower_http::services::ServeDir;

use crate::contexts::editorial::domain::diagnostics::validation_issues;
use crate::contexts::listening::{
    auth::{DenyListenerAuthentication, ListenerAuthenticator},
    LibraryService,
};
use crate::contexts::media::read::StorageError;
use crate::contexts::media::{
    ArtworkService, ListenerService, OriginPlayback, PlaybackDelivery, PublicationDelivery,
    PublicationService,
};
use crate::platform::postgres::{Store, StoreError};

mod admin;
pub(crate) mod audio_transport;
mod cookies;
mod cors;
pub use cors::with_browser_cors;
mod studio;
mod theatre;
mod theatre_auth;
mod theatre_library;
mod theatre_native_auth;
mod theatre_registration;
mod wire;

#[cfg(test)]
pub(crate) fn playback_contract_router(listener: ListenerService) -> Router {
    theatre::playback_contract_routes().layer(axum::extract::Extension(listener))
}

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub web_origin: String,
}

pub fn router(state: AppState, dist: &str) -> Router {
    let publication = PublicationService::new(state.store.clone());
    router_with_publication(state, Some(dist), publication)
}

/// Production host composition: the same surfaces as [`router`], with no static file fallback.
/// Web apps are served by their own hosting; every non-API path is a JSON `not_found`.
pub fn api_router(state: AppState) -> Router {
    let publication = PublicationService::new(state.store.clone());
    router_with_publication(state, None, publication)
}

/// Trusted host composition only. Requests cannot replace the media delivery consumer.
pub fn router_with_publication_delivery(
    state: AppState,
    dist: &str,
    delivery: std::sync::Arc<dyn PublicationDelivery>,
) -> Router {
    let publication =
        PublicationService::new(state.store.clone()).with_publication_delivery(delivery);
    router_with_publication(state, Some(dist), publication)
}

fn router_with_publication(
    state: AppState,
    dist: Option<&str>,
    publication: PublicationService,
) -> Router {
    // Unconfigured hosts grant no publication delivery or playback authority.
    let listener = ListenerService::new(state.store.clone());
    router_with_services(state, dist, publication, listener)
}

/// Trusted public-delivery host composition. Default host remains fail-closed.
pub fn router_with_theatre_delivery(
    state: AppState,
    dist: &str,
    delivery: std::sync::Arc<dyn PlaybackDelivery>,
) -> Router {
    let publication = PublicationService::new(state.store.clone());
    let listener = ListenerService::new(state.store.clone()).with_playback_delivery(delivery);
    router_with_services(state, Some(dist), publication, listener)
}

/// Trusted origin-delivery composition (ADR 0032 mode A). One value enables grant
/// issuance and the `/theatre/media/{grant}` reader together; the default host never does.
pub fn router_with_origin_playback(state: AppState, dist: &str, origin: OriginPlayback) -> Router {
    let publication = PublicationService::new(state.store.clone());
    let listener = ListenerService::new(state.store.clone()).with_origin_playback(origin);
    router_with_services(state, Some(dist), publication, listener)
}

/// Origin delivery and publication eligibility share the same separately configured byte class.
/// No static fallback is exposed by a media-enabled host, including in development.
pub fn router_with_public_media(
    state: AppState,
    publication: PublicationService,
    origin: OriginPlayback,
) -> Router {
    let listener = ListenerService::new(state.store.clone()).with_origin_playback(origin);
    router_with_services(state, None, publication, listener)
}

fn router_with_services(
    state: AppState,
    dist: Option<&str>,
    publication: PublicationService,
    listener: ListenerService,
) -> Router {
    let authenticator: std::sync::Arc<dyn ListenerAuthenticator> =
        if state.store.theatre_identity().is_some() {
            std::sync::Arc::new(
                crate::contexts::listening::auth::TheatreListenerAuthenticator::new(
                    state.store.clone(),
                ),
            )
        } else {
            std::sync::Arc::new(DenyListenerAuthentication)
        };
    router_with_identity_services(state, dist, publication, listener, authenticator)
}

/// Trusted host composition. A real issuer adapter must authenticate before it mints a principal.
pub fn router_with_listener_identity(
    state: AppState,
    dist: &str,
    authenticator: std::sync::Arc<dyn ListenerAuthenticator>,
) -> Router {
    let publication = PublicationService::new(state.store.clone());
    let listener = ListenerService::new(state.store.clone());
    router_with_identity_services(state, Some(dist), publication, listener, authenticator)
}

/// Trusted composition override for the private-artwork application capability.
pub fn router_with_private_artwork(state: AppState, dist: &str, artwork: ArtworkService) -> Router {
    let publication = PublicationService::new(state.store.clone());
    let listener = ListenerService::new(state.store.clone());
    router_with_identity_and_artwork(
        state,
        Some(dist),
        publication,
        listener,
        std::sync::Arc::new(DenyListenerAuthentication),
        Some(artwork),
    )
}

fn router_with_identity_services(
    state: AppState,
    dist: Option<&str>,
    publication: PublicationService,
    listener: ListenerService,
    authenticator: std::sync::Arc<dyn ListenerAuthenticator>,
) -> Router {
    // The existing operator-selected private storage topology is the activation boundary.
    // No new provider/configuration path or public artwork delivery is introduced.
    let artwork = state
        .store
        .audio_storage_is_enabled()
        .then(|| ArtworkService::new(state.store.clone()));
    router_with_identity_and_artwork(state, dist, publication, listener, authenticator, artwork)
}

fn router_with_identity_and_artwork(
    state: AppState,
    dist: Option<&str>,
    publication: PublicationService,
    listener: ListenerService,
    authenticator: std::sync::Arc<dyn ListenerAuthenticator>,
    artwork: Option<ArtworkService>,
) -> Router {
    let library = LibraryService::new(state.store.clone());
    // Surfaces are separate route groups with their own permission context; shared code is the
    // middleware below and the domain/store modules, never a handler.
    let api = Router::new()
        .nest(
            "/studio",
            studio::routes_with_artwork(state.clone(), studio::Mount::Current, artwork.is_some())
                .fallback(not_found),
        )
        .nest(
            "/theatre",
            theatre::routes()
                .merge(theatre_auth::routes(
                    state.store.theatre_identity().is_some(),
                ))
                .merge(theatre_native_auth::routes(
                    state.store.theatre_native().is_some(),
                ))
                .merge(theatre_library::routes())
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    theatre_auth::bff_identity,
                )),
        )
        .nest("/admin", admin::routes())
        // Deprecated pre-split Studio paths (`/api/v1/scripts/...`); same handlers as `/studio`.
        .merge(studio::routes(state.clone(), studio::Mount::Legacy))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            origin_and_cache,
        ));
    let host = Router::new().nest(routes::API_BASE, api);
    let host = match dist {
        // ServeDir follows nested symlinks. An intake-enabled process therefore serves no
        // static files at all, even if public and private root paths appear disjoint.
        // Every private artwork composition follows the same API-only rule.
        Some(dist) if !state.store.audio_storage_is_enabled() && artwork.is_none() => {
            host.fallback_service(ServeDir::new(dist))
        }
        _ => host.fallback(not_found),
    };
    let host = if let Some(artwork) = artwork {
        host.layer(axum::Extension(artwork))
    } else {
        host
    };
    host.with_state(state)
        .layer(axum::Extension(publication))
        .layer(axum::Extension(listener))
        .layer(axum::Extension(library))
        .layer(axum::Extension(authenticator))
}

async fn not_found() -> Response {
    failure(ErrorCode::NotFound, StatusCode::NOT_FOUND)
}

fn cookie_token(headers: &HeaderMap, store: &Store) -> Result<String, StoreError> {
    let name = store
        .studio_oidc()
        .map_or("cantos_session", |oidc| oidc.session_name());
    cookie_token_for_name(headers, name)
}

fn cookie_token_for_name(headers: &HeaderMap, expected: &str) -> Result<String, StoreError> {
    let mut found = None;
    for name in ["cantos_session", "__Secure-cantos_studio"] {
        let value =
            studio::auth::unique_cookie(headers, name).map_err(|_| StoreError::Unauthenticated)?;
        if let Some(value) = value {
            if name != expected || found.is_some() {
                return Err(StoreError::Unauthenticated);
            }
            found = Some(value);
        }
    }
    found.ok_or(StoreError::Unauthenticated)
}

async fn authenticated(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let token = match cookie_token(request.headers(), &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    if let Err(error) = state.store.authenticate(&token).await {
        return error_response(error);
    }
    let session_id = match state.store.studio_session_id(&token).await {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    let mut query_fences = url::form_urlencoded::parse(
        request
            .uri()
            .path_and_query()
            .and_then(axum::http::uri::PathAndQuery::query)
            .unwrap_or("")
            .as_bytes(),
    )
    .filter(|(name, _)| name == "studio_session_id");
    let query_matches = match query_fences.next() {
        None => true,
        Some((_, value)) => {
            query_fences.next().is_none() && Some(value.as_ref()) == session_id.as_deref()
        }
    };
    if !query_matches || !studio_fence_matches(request.headers(), session_id.as_deref()) {
        return studio_fenced_response(
            failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
            session_id.as_deref(),
        );
    }
    studio_fenced_response(next.run(request).await, session_id.as_deref())
}

pub(super) const STUDIO_SESSION_HEADER: &str = "x-cantos-studio-session";

pub(super) fn studio_fence_matches(headers: &HeaderMap, actual: Option<&str>) -> bool {
    let mut values = headers.get_all(STUDIO_SESSION_HEADER).iter();
    match values.next() {
        None => true, // Legacy explicit clients remain compatible; the current UI always fences.
        Some(value) => {
            values.next().is_none()
                && value
                    .to_str()
                    .ok()
                    .is_some_and(|value| Some(value) == actual)
        }
    }
}

pub(super) fn studio_fenced_response(mut response: Response, session_id: Option<&str>) -> Response {
    if let Some(id) = session_id.and_then(|id| HeaderValue::from_str(id).ok()) {
        response.headers_mut().insert(STUDIO_SESSION_HEADER, id);
    }
    response
}

async fn origin_and_cache(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let read =
        request.method() == axum::http::Method::GET || request.method() == axum::http::Method::HEAD;
    let valid_config = state
        .store
        .studio_oidc()
        .is_none_or(|oidc| oidc.web_origin() == state.web_origin);
    let valid_origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
        == Some(state.web_origin.as_str());
    let original_path = request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map_or(request.uri().path(), |uri| uri.0.path());
    let theatre_request = original_path.starts_with("/api/v1/theatre/");
    let anonymous_playback = theatre::public_playback_post(request.method(), original_path);
    let bearer_mutation = theatre_library::bearer_mutation(request.method(), original_path);
    let native_mutation = theatre_native_auth::native_mutation(request.method(), original_path);
    let delegated_mutation = studio::delegation::bearer_mutation(request.method(), original_path);
    let mut response = if !valid_config
        || (!read
            && !valid_origin
            && !anonymous_playback
            && !bearer_mutation
            && !native_mutation
            && !delegated_mutation)
    {
        failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN)
    } else {
        match tokio::time::timeout(std::time::Duration::from_secs(15), next.run(request)).await {
            Ok(response) => response,
            Err(_) if theatre_request => {
                theatre::error(crate::contexts::media::ListenerError::Unavailable)
            }
            Err(_) => failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
        }
    };
    // Preserve the transport-owned policy, not a MIME heuristic: 304 has no Content-Type,
    // while conditional rejections and other audio errors carry the surface's JSON body.
    // Unrecognized/missing policies still cannot make a response cacheable.
    let cache_control = if response.headers().get(header::CACHE_CONTROL)
        == Some(&audio_transport::PRIVATE_CACHE_CONTROL)
    {
        audio_transport::PRIVATE_CACHE_CONTROL
    } else {
        HeaderValue::from_static("no-store")
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, cache_control);
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

fn failure(code: ErrorCode, status: StatusCode) -> Response {
    (
        status,
        Json(ApiError {
            code,
            current_revision: None,
            issues: vec![],
        }),
    )
        .into_response()
}

fn error_response(error: StoreError) -> Response {
    let (status, code, current_revision, issues) = match error {
        StoreError::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthenticated,
            None,
            vec![],
        ),
        StoreError::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound, None, vec![]),
        StoreError::Forbidden => (StatusCode::FORBIDDEN, ErrorCode::Forbidden, None, vec![]),
        StoreError::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            vec![],
        ),
        StoreError::InvalidRequestFields(issues) => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            issues,
        ),
        StoreError::EvidenceUnavailable => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::EvidenceUnavailable,
            None,
            vec![],
        ),
        StoreError::StaleRevision(revision) => (
            StatusCode::CONFLICT,
            ErrorCode::StaleRevision,
            Some(revision),
            vec![],
        ),
        StoreError::OperationReused => (
            StatusCode::CONFLICT,
            ErrorCode::OperationReused,
            None,
            vec![],
        ),
        StoreError::ProposalAlreadySubmitted => (
            StatusCode::CONFLICT,
            ErrorCode::ProposalAlreadySubmitted,
            None,
            vec![],
        ),
        StoreError::StaleProductionInputs => (
            StatusCode::CONFLICT,
            ErrorCode::StaleProductionInputs,
            None,
            vec![],
        ),
        StoreError::ProductionBlocked(issues) => (
            StatusCode::CONFLICT,
            ErrorCode::ProductionBlocked,
            None,
            issues,
        ),
        StoreError::Storage(error) => return storage_error_response(error),
        StoreError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            None,
            vec![],
        ),
        StoreError::CorruptRevision => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
            None,
            vec![],
        ),
        StoreError::AudioRejected(rule) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::AudioRejected,
            None,
            vec![cantos_contracts::FieldIssue {
                path: "audio".into(),
                rule: rule.into(),
            }],
        ),
        StoreError::AudioInspectionUnavailable(rule) => (
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            None,
            vec![cantos_contracts::FieldIssue {
                path: "audio".into(),
                rule: rule.into(),
            }],
        ),
        StoreError::InvalidScript(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidScript,
            None,
            validation_issues(error),
        ),
    };
    (
        status,
        Json(ApiError {
            code,
            current_revision,
            issues,
        }),
    )
        .into_response()
}

fn storage_error_response(error: StorageError) -> Response {
    let level = match error {
        StorageError::NotFound => "DEBUG",
        StorageError::Forbidden
        | StorageError::Conflict
        | StorageError::Timeout
        | StorageError::Cancelled => "WARN",
        StorageError::InvalidTarget
        | StorageError::InvalidKey
        | StorageError::InvalidIntegrity
        | StorageError::DigestMismatch
        | StorageError::LengthMismatch
        | StorageError::Unavailable
        | StorageError::UnsupportedCapability(_) => "ERROR",
    };
    eprintln!(
        "level={level} event=private_storage_operation_failed outcome={}",
        error.code()
    );
    let (status, code) = match error {
        StorageError::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
        StorageError::Forbidden => (StatusCode::FORBIDDEN, ErrorCode::Forbidden),
        StorageError::Conflict => (StatusCode::CONFLICT, ErrorCode::OperationReused),
        StorageError::InvalidTarget
        | StorageError::InvalidKey
        | StorageError::InvalidIntegrity
        | StorageError::DigestMismatch
        | StorageError::LengthMismatch => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
        ),
        StorageError::Timeout
        | StorageError::Cancelled
        | StorageError::Unavailable
        | StorageError::UnsupportedCapability(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable)
        }
    };
    failure(code, status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::media::read::StorageCapability;
    use axum::{body::Body, routing::get};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn middleware_state() -> AppState {
        AppState {
            store: Store::new(
                "host=127.0.0.1 port=1 user=unused dbname=unused"
                    .parse()
                    .unwrap(),
            )
            .unwrap(),
            web_origin: "http://127.0.0.1:8080".into(),
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
        let get = |uri: &str| axum::http::Request::get(uri).body(Body::empty()).unwrap();

        let (status, body) = send(&app, get("/api/v1/studio/auth/configuration")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body.contains("unavailable"), "{body}");

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

    #[tokio::test]
    async fn development_host_keeps_reporting_the_token_mode() {
        let app = router(middleware_state(), "unused-dist");
        let request = axum::http::Request::get("/api/v1/studio/auth/configuration")
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&app, request).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("development"), "{body}");
    }

    #[tokio::test]
    async fn api_router_serves_no_static_files() {
        let dist = std::env::temp_dir().join(format!("cantos-api-dist-{}", std::process::id()));
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
}
