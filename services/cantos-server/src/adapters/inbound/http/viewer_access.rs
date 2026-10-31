//! Separate control-plane routes. Theatre claim accepts no actor, mailbox, or session field.
use super::{
    errors::error_response,
    identity_auth::{theatre_credential, TheatreCredential},
    studio::admission::cookie_token,
    AppState,
};
use crate::contract::theatre::viewer_access::*;
use crate::{
    application::{ViewerAccessError, ViewerAccessRefusal as AccessError},
    contract::studio::*,
    platform::store::StoreError,
};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::de::DeserializeOwned;
use serde_json::json;

pub(super) fn studio_routes() -> Router<AppState> {
    Router::new()
        .route("/works/{work}/viewer-access", get(state).post(command))
        .route("/viewer-groups", get(groups))
        .route("/viewer-groups/{group}", post(group_command))
        .layer(DefaultBodyLimit::max(4096))
}
pub(super) fn theatre_routes() -> Router<AppState> {
    Router::new()
        .route("/shared/claim", post(claim))
        .layer(DefaultBodyLimit::max(4096))
}
fn private(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
fn error(e: ViewerAccessError) -> Response {
    let (status, code, current) = match e {
        ViewerAccessError::Storage(e) => return private(error_response(e)),
        ViewerAccessError::Access(AccessError::Invalid) => {
            (StatusCode::BAD_REQUEST, "invalid_request", None)
        }
        ViewerAccessError::Access(AccessError::Stale { current }) => {
            (StatusCode::CONFLICT, "stale_policy", Some(current))
        }
        ViewerAccessError::Access(AccessError::NotFound) => {
            (StatusCode::NOT_FOUND, "not_found", None)
        }
        ViewerAccessError::Access(AccessError::Rights) => {
            (StatusCode::CONFLICT, "rights_unavailable", None)
        }
        ViewerAccessError::Access(AccessError::Unavailable) => {
            (StatusCode::SERVICE_UNAVAILABLE, "unavailable", None)
        }
        ViewerAccessError::Access(AccessError::Reused) => {
            (StatusCode::CONFLICT, "operation_reused", None)
        }
        ViewerAccessError::Access(AccessError::Bound) => {
            (StatusCode::CONFLICT, "access_bound", None)
        }
        ViewerAccessError::Access(AccessError::Duplicate) => {
            (StatusCode::CONFLICT, "duplicate", None)
        }
        ViewerAccessError::Access(AccessError::Exhausted) => {
            (StatusCode::CONFLICT, "version_limit", None)
        }
    };
    private((status, Json(json!({"code":code,"current_version":current}))).into_response())
}
fn body<T: DeserializeOwned>(
    raw: Result<Json<T>, axum::extract::rejection::JsonRejection>,
) -> Result<T, ViewerAccessError> {
    raw.map(|Json(v)| v)
        .map_err(|_| AccessError::Invalid.into())
}
async fn state(
    State(s): State<AppState>,
    Path(work): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &s.store) {
        Ok(t) => t,
        Err(e) => return error(e.into()),
    };
    match s.store.viewer_access_state(&token, &work).await {
        Ok(v) => private(Json(v).into_response()),
        Err(e) => error(e),
    }
}
async fn command(
    State(s): State<AppState>,
    Path(work): Path<String>,
    headers: HeaderMap,
    raw: Result<Json<ViewerAccessRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let token = match cookie_token(&headers, &s.store) {
        Ok(t) => t,
        Err(e) => return error(e.into()),
    };
    let request = match body(raw) {
        Ok(r) => r,
        Err(e) => return error(e),
    };
    match s.store.viewer_access_execute(&token, &work, request).await {
        Ok(v) => private(Json(v).into_response()),
        Err(e) => error(e),
    }
}
async fn groups(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let token = match cookie_token(&headers, &s.store) {
        Ok(t) => t,
        Err(e) => return error(e.into()),
    };
    match s.store.viewer_groups(&token).await {
        Ok(v) => private(Json(v).into_response()),
        Err(e) => error(e),
    }
}
async fn group_command(
    State(s): State<AppState>,
    Path(group): Path<String>,
    headers: HeaderMap,
    raw: Result<Json<ViewerGroupRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let token = match cookie_token(&headers, &s.store) {
        Ok(t) => t,
        Err(e) => return error(e.into()),
    };
    let request = match body(raw) {
        Ok(r) => r,
        Err(e) => return error(e),
    };
    match s.store.viewer_group_execute(&token, &group, request).await {
        Ok(v) => private(Json(v).into_response()),
        Err(e) => error(e),
    }
}
async fn claim(
    State(s): State<AppState>,
    headers: HeaderMap,
    raw: Result<Json<ClaimViewerSharesRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let token = match theatre_credential(&s, &headers) {
        TheatreCredential::Present(_, token) => token,
        _ => return error(StoreError::Unauthenticated.into()),
    };
    let request = match body(raw) {
        Ok(r) => r,
        Err(e) => return error(e),
    };
    match s.store.claim_viewer_shares(&token, request).await {
        Ok(v) => private(Json(v).into_response()),
        Err(e) => error(e),
    }
}
