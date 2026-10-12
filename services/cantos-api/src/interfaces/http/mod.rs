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
use crate::platform::postgres::{Store, StoreError};

mod admin;
mod studio;
mod theatre;
mod wire;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub web_origin: String,
}

pub fn router(state: AppState, dist: &str) -> Router {
    // Surfaces are separate route groups with their own permission context; shared code is the
    // middleware below and the domain/store modules, never a handler.
    let api = Router::new()
        .nest(
            "/studio",
            studio::routes(state.clone(), studio::Mount::Current).fallback(not_found),
        )
        .nest("/theatre", theatre::routes())
        .nest("/admin", admin::routes())
        // Deprecated pre-split Studio paths (`/api/v1/scripts/...`); same handlers as `/studio`.
        .merge(studio::routes(state.clone(), studio::Mount::Legacy))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            origin_and_cache,
        ));
    Router::new()
        .nest(routes::API_BASE, api)
        .fallback_service(ServeDir::new(dist))
        .with_state(state)
}

async fn not_found() -> Response {
    failure(ErrorCode::NotFound, StatusCode::NOT_FOUND)
}

fn cookie_token(headers: &HeaderMap) -> Result<String, StoreError> {
    let cookies = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .ok_or(StoreError::Unauthenticated)?;
    let mut tokens = cookies
        .split(';')
        .filter_map(|part| part.trim().strip_prefix("cantos_session="));
    let token = tokens.next().ok_or(StoreError::Unauthenticated)?;
    if tokens.next().is_some() {
        return Err(StoreError::Unauthenticated);
    }
    Ok(token.to_owned())
}

async fn authenticated(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let result = match cookie_token(request.headers()) {
        Ok(token) => state.store.authenticate(&token).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(_) => next.run(request).await,
        Err(error) => error_response(error),
    }
}

async fn origin_and_cache(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let read =
        request.method() == axum::http::Method::GET || request.method() == axum::http::Method::HEAD;
    let valid_origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
        == Some(state.web_origin.as_str());
    let mut response = if !read && !valid_origin {
        failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN)
    } else {
        match tokio::time::timeout(std::time::Duration::from_secs(15), next.run(request)).await {
            Ok(response) => response,
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
