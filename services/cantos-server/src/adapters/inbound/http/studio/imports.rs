//! HTTP shells for private, immutable manuscript intake.
use crate::contract::studio::{ErrorCode, ImportRequest};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

use crate::adapters::inbound::http::{
    errors::error_response, errors::failure, studio::admission::cookie_token, AppState,
};
use crate::application::StoreError;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/imports", post(import))
        .route("/imports/{source}", get(load))
        .route("/imports/{source}/original", get(original))
        // A 1 MiB Vec<u8> occupies at most about 4 MiB in JSON integer transport.
        .layer(DefaultBodyLimit::max(5 * 1024 * 1024))
}

async fn import(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<ImportRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return failure(ErrorCode::InvalidRequest, StatusCode::PAYLOAD_TOO_LARGE)
        }
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.import_manuscript(&token, request).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => error_response(error),
    }
}

async fn load(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.load_import(&token, &id).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => error_response(error),
    }
}

async fn original(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.original_import(&token, &id).await {
        Ok(bytes) => {
            let mut response = bytes.into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"manuscript.bin\""),
            );
            response
        }
        Err(error) => error_response(error),
    }
}
