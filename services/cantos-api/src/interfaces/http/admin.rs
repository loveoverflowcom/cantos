//! Admin-only shell. Accepts a dedicated verified resource JWT, never cookies/identity headers.
use super::{failure, not_found, AppState};
use crate::platform::admin::AdminError;
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection},
        DefaultBodyLimit, Path, State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{admin::RevokeAccessRequest, ErrorCode};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/capabilities", get(capabilities))
        .route("/actors/{actor}/revoke-access", post(revoke))
        .fallback(not_found)
        .method_not_allowed_fallback(invalid_request)
        .layer(DefaultBodyLimit::max(1024))
}
fn bearer(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    value
        .strip_prefix("Bearer ")
        .filter(|v| !v.is_empty() && v.len() <= 16_384 && !v.chars().any(char::is_whitespace))
}
async fn capabilities(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(admin) = state.store.admin() else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    match admin
        .capabilities(&state.store, bearer(&headers), uuid::Uuid::new_v4())
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn revoke(
    State(state): State<AppState>,
    actor: Result<Path<String>, PathRejection>,
    headers: HeaderMap,
    request: Result<Json<RevokeAccessRequest>, JsonRejection>,
) -> Response {
    let (actor, request) = match (actor, request) {
        (Ok(Path(actor)), Ok(Json(request))) => (actor, request),
        _ => return invalid_request().await,
    };
    let Some(admin) = state.store.admin() else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    match admin
        .revoke_access(
            &state.store,
            bearer(&headers),
            &actor,
            &request.operation_id,
            uuid::Uuid::new_v4(),
        )
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn invalid_request() -> Response {
    failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST)
}
fn error(error: AdminError) -> Response {
    let (code, status) = match error {
        AdminError::Credential | AdminError::ActorMissing => {
            (ErrorCode::Unauthenticated, StatusCode::UNAUTHORIZED)
        }
        AdminError::MissingRole
        | AdminError::UnknownRole
        | AdminError::StaleRole
        | AdminError::InactiveActor
        | AdminError::Revoked
        | AdminError::Forbidden => (ErrorCode::Forbidden, StatusCode::FORBIDDEN),
        AdminError::InvalidRequest => (ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST),
        AdminError::TargetMissing => (ErrorCode::NotFound, StatusCode::NOT_FOUND),
        AdminError::OperationReused => (ErrorCode::OperationReused, StatusCode::CONFLICT),
        AdminError::Configuration
        | AdminError::ProviderUnavailable
        | AdminError::AuditUnavailable
        | AdminError::Unavailable
        | AdminError::CorruptRecord => (ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    };
    failure(code, status)
}
