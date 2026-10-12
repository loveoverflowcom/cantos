//! Thin private adaptation routes; all provider and transaction work belongs to the store.
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationContextRequest, AdaptationProviderResponse,
    CancelAdaptationRequest, ErrorCode, StartAdaptationRequest, SubmitAdaptationProposalRequest,
};

use crate::http::{cookie_token, error_response, failure, AppState};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/adaptations/provider", get(provider))
        .route("/adaptations", post(start))
        .route(
            "/adaptations/contexts",
            post(create_context).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/adaptations/{run}", get(load))
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
        .route("/adaptations/{run}/cancel", post(cancel))
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

async fn provider(State(state): State<AppState>) -> Response {
    Json(AdaptationProviderResponse {
        provider: state.store.adaptation_provider_metadata(),
    })
    .into_response()
}

async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<StartAdaptationRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match decode_body(body) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.start_adaptation(&token, request).await {
        Ok(run) => (StatusCode::ACCEPTED, Json(run)).into_response(),
        Err(error) => error_response(error),
    }
}

async fn load(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.load_adaptation(&token, &id).await {
        Ok(run) => Json(run).into_response(),
        Err(error) => error_response(error),
    }
}

async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<CancelAdaptationRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match decode_body(body) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.cancel_adaptation(&token, &id, request).await {
        Ok(run) => Json(run).into_response(),
        Err(error) => error_response(error),
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
