//! Thin caller-authored adaptation routes; transaction work belongs to the store.
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationContextRequest, CancelAdaptationRequest, ErrorCode,
    SubmitAdaptationProposalRequest,
};

use crate::adapters::inbound::http::{cookie_token, error_response, failure, AppState};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/adaptations/contexts",
            post(create_context).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/adaptations/{run}/context", get(context))
        .route("/adaptations/{run}/review", get(review))
        .route(
            "/adaptations/{run}/proposals",
            post(submit).layer(DefaultBodyLimit::max(1024 * 1024)),
        )
        .route(
            "/adaptations/{run}/context/cancel",
            post(cancel_context).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/adaptations/{run}/accept", post(accept))
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
}

fn decode_body<T>(
    body: Result<Json<T>, axum::extract::rejection::JsonRejection>,
) -> Result<T, StatusCode> {
    match body {
        Ok(Json(request)) => Ok(request),
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            Err(StatusCode::PAYLOAD_TOO_LARGE)
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn accept(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<AcceptAdaptationRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match decode_body(body) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.accept_adaptation(&token, &id, request).await {
        Ok(saved) => Json(saved).into_response(),
        Err(error) => error_response(error),
    }
}

async fn create_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<AdaptationContextRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match decode_body(body) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.create_adaptation_context(&token, request).await {
        Ok(context) => (StatusCode::CREATED, Json(context)).into_response(),
        Err(error) => error_response(error),
    }
}

async fn context(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.load_adaptation_context(&token, &id).await {
        Ok(context) => Json(context).into_response(),
        Err(error) => error_response(error),
    }
}

async fn review(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.load_adaptation_review(&token, &id).await {
        Ok(review) => Json(review).into_response(),
        Err(error) => error_response(error),
    }
}

async fn submit(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<SubmitAdaptationProposalRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match decode_body(body) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .submit_adaptation_proposal(&token, &id, request)
        .await
    {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => error_response(error),
    }
}

async fn cancel_context(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CancelAdaptationRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match decode_body(body) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .cancel_adaptation_context(&token, &id, request)
        .await
    {
        Ok(context) => Json(context).into_response(),
        Err(error) => error_response(error),
    }
}
