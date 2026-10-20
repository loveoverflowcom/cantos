//! Studio owner management and separately authenticated one-submission upload capabilities.
use super::StudioPath as AudioPath;
use super::{
    audio_intake::limit_upload,
    catalog::{json_only, CatalogBody},
};
use crate::adapters::inbound::http::{
    errors::error_response, errors::failure, studio::admission::cookie_token, AppState,
};
use crate::application::{Store, StoreError};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use cantos_contracts as dto;
async fn method_rejected() -> Response {
    failure(
        dto::ErrorCode::InvalidRequest,
        StatusCode::METHOD_NOT_ALLOWED,
    )
}

pub(super) fn owner_routes() -> Router<AppState> {
    Router::new()
        .merge(super::publication::routes())
        .route("/content-units/{unit}/audio-submissions", post(create))
        .route("/content-units/{unit}/audio-submissions/{id}", get(read))
        .route(
            "/content-units/{unit}/audio-submissions/{id}/upload-permission",
            delete(revoke),
        )
        .route(
            "/content-units/{unit}/audio-submissions/{id}/reviews",
            post(review),
        )
        .route(
            "/content-units/{unit}/audio-submissions/{id}/release-candidates",
            post(candidate),
        )
        .route(
            "/content-units/{unit}/audio-submissions/{id}/release-candidates/{candidate}",
            get(candidate_read),
        )
        .method_not_allowed_fallback(method_rejected)
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn(json_only))
}
pub(super) fn producer_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/producer/audio-submissions/{id}",
            get(producer_read).layer(middleware::from_fn(json_only)),
        )
        .route(
            "/producer/audio-submissions/{id}/bytes",
            put(upload).layer(middleware::from_fn(limit_upload)),
        )
        .method_not_allowed_fallback(method_rejected)
        .layer(DefaultBodyLimit::max(Store::audio_upload_body_limit()))
}
fn bearer(headers: &HeaderMap) -> Result<String, StoreError> {
    if headers.contains_key(header::COOKIE)
        || headers.get_all(header::AUTHORIZATION).iter().count() != 1
    {
        return Err(StoreError::Unauthenticated);
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::to_owned)
        .ok_or(StoreError::Unauthenticated)
}
fn result<T: serde::Serialize>(value: Result<T, StoreError>, status: StatusCode) -> Response {
    match value {
        Ok(value) => (status, Json(value)).into_response(),
        Err(e) => error_response(e),
    }
}
async fn create(
    State(state): State<AppState>,
    AudioPath(unit): AudioPath<String>,
    headers: HeaderMap,
    CatalogBody(request): CatalogBody<dto::CreateAudioSubmissionRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    result(
        state
            .store
            .create_audio_submission(&token, &unit, request)
            .await,
        StatusCode::CREATED,
    )
}
async fn read(
    State(state): State<AppState>,
    AudioPath((unit, id)): AudioPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    result(
        state.store.load_audio_submission(&token, &unit, &id).await,
        StatusCode::OK,
    )
}
async fn revoke(
    State(state): State<AppState>,
    AudioPath((unit, id)): AudioPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    match state
        .store
        .revoke_audio_submission(&token, &unit, &id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(e),
    }
}
async fn review(
    State(state): State<AppState>,
    AudioPath((unit, id)): AudioPath<(String, String)>,
    headers: HeaderMap,
    CatalogBody(request): CatalogBody<dto::CreateAudioReviewRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    result(
        state
            .store
            .review_audio_submission(&token, &unit, &id, request)
            .await,
        StatusCode::CREATED,
    )
}
async fn candidate(
    State(state): State<AppState>,
    AudioPath((unit, id)): AudioPath<(String, String)>,
    headers: HeaderMap,
    CatalogBody(request): CatalogBody<dto::CreateReleaseCandidateRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    result(
        state
            .store
            .create_audio_release_candidate(&token, &unit, &id, request)
            .await,
        StatusCode::CREATED,
    )
}
async fn candidate_read(
    State(state): State<AppState>,
    AudioPath((unit, id, candidate)): AudioPath<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    result(
        state
            .store
            .load_audio_release_candidate(&token, &unit, &id, &candidate)
            .await,
        StatusCode::OK,
    )
}
async fn producer_read(
    State(state): State<AppState>,
    AudioPath(id): AudioPath<String>,
    headers: HeaderMap,
) -> Response {
    let token = match bearer(&headers) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    result(
        state.store.producer_audio_submission(&token, &id).await,
        StatusCode::OK,
    )
}
async fn upload(
    State(state): State<AppState>,
    AudioPath(id): AudioPath<String>,
    headers: HeaderMap,
    bytes: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let token = match bearer(&headers) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        != Some("application/octet-stream")
        || headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v != "application/json" && v != "*/*")
    {
        return failure(
            dto::ErrorCode::InvalidRequest,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        );
    }
    let bytes = match bytes {
        Ok(v) => v,
        Err(e) if e.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return failure(
                dto::ErrorCode::InvalidRequest,
                StatusCode::PAYLOAD_TOO_LARGE,
            )
        }
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    result(
        state
            .store
            .upload_audio_submission(&token, &id, bytes.to_vec())
            .await,
        StatusCode::OK,
    )
}
