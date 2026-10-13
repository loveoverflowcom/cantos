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
use crate::contexts::media::{
    ListenerService, PlaybackDelivery, PublicationDelivery, PublicationService,
};
use crate::platform::postgres::{Store, StoreError};

mod admin;
mod studio;
mod theatre;
mod theatre_library;
mod wire;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub web_origin: String,
}

pub fn router(state: AppState, dist: &str) -> Router {
    let publication = PublicationService::new(state.store.clone());
    router_with_publication(state, dist, publication)
}

/// Trusted host composition only. Requests cannot replace the media delivery consumer.
pub fn router_with_publication_delivery(
    state: AppState,
    dist: &str,
    delivery: std::sync::Arc<dyn PublicationDelivery>,
) -> Router {
    let publication =
        PublicationService::new(state.store.clone()).with_publication_delivery(delivery);
    router_with_publication(state, dist, publication)
}

fn router_with_publication(state: AppState, dist: &str, publication: PublicationService) -> Router {
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
    router_with_services(state, dist, publication, listener)
}

fn router_with_services(
    state: AppState,
    dist: &str,
    publication: PublicationService,
    listener: ListenerService,
) -> Router {
    router_with_identity_services(
        state,
        dist,
        publication,
        listener,
        std::sync::Arc::new(DenyListenerAuthentication),
    )
}

/// Trusted host composition. A real issuer adapter must authenticate before it mints a principal.
pub fn router_with_listener_identity(
    state: AppState,
    dist: &str,
    authenticator: std::sync::Arc<dyn ListenerAuthenticator>,
) -> Router {
    let publication = PublicationService::new(state.store.clone());
    let listener = ListenerService::new(state.store.clone());
    router_with_identity_services(state, dist, publication, listener, authenticator)
}

fn router_with_identity_services(
    state: AppState,
    dist: &str,
    publication: PublicationService,
    listener: ListenerService,
    authenticator: std::sync::Arc<dyn ListenerAuthenticator>,
) -> Router {
    let library = LibraryService::new(state.store.clone());
    // Surfaces are separate route groups with their own permission context; shared code is the
    // middleware below and the domain/store modules, never a handler.
    let api = Router::new()
        .nest(
            "/studio",
            studio::routes(state.clone(), studio::Mount::Current).fallback(not_found),
        )
        .nest(
            "/theatre",
            theatre::routes().merge(theatre_library::routes()),
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
    let host = if state.store.audio_storage_is_enabled() {
        // ServeDir follows nested symlinks. An intake-enabled process therefore serves no
        // static files at all, even if public and private root paths appear disjoint.
        host.fallback(not_found)
    } else {
        host.fallback_service(ServeDir::new(dist))
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
    let mut query_fences =
        url::form_urlencoded::parse(request.uri().query().unwrap_or("").as_bytes())
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
    let mut response = if !valid_config
        || (!read && !valid_origin && !anonymous_playback && !bearer_mutation)
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
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
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
