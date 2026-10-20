//! Private planning endpoints; persistence and policy stay in the application/domain shell.
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{
    ApproveProductionRequest, ErrorCode, FreezeProductionRequest, SaveProductionRightsRequest,
    SaveProductionSettingsRequest,
};

use crate::adapters::inbound::http::{cookie_token, error_response, failure, AppState};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/production/catalog", get(catalog))
        .route("/scripts/{script}/production", get(state))
        .route(
            "/scripts/{script}/production/settings",
            post(save_settings).layer(DefaultBodyLimit::max(256 * 1024)),
        )
        .route(
            "/scripts/{script}/production/rights",
            post(save_rights).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/scripts/{script}/production/review", get(preview))
        .route(
            "/scripts/{script}/production/snapshots",
            post(freeze).layer(DefaultBodyLimit::max(64 * 1024)),
        )
        .route(
            "/scripts/{script}/production/snapshots/{snapshot}",
            get(snapshot),
        )
        .route(
            "/scripts/{script}/production/snapshots/{snapshot}/eligibility",
            get(eligibility),
        )
        .route(
            "/scripts/{script}/production/snapshots/{snapshot}/approvals",
            post(approve).layer(DefaultBodyLimit::max(64 * 1024)),
        )
}

fn body<T>(
    body: Result<Json<T>, axum::extract::rejection::JsonRejection>,
) -> Result<T, StatusCode> {
    match body {
        Ok(Json(value)) => Ok(value),
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            Err(StatusCode::PAYLOAD_TOO_LARGE)
        }
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

async fn catalog() -> Response {
    Json(crate::contexts::production::production_catalog()).into_response()
}

async fn state(
    State(state): State<AppState>,
    Path(script): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.production_state(&token, &script).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

async fn save_settings(
    State(state): State<AppState>,
    Path(script): Path<String>,
    headers: HeaderMap,
    request: Result<Json<SaveProductionSettingsRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body(request) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .save_production_settings(&token, &script, request)
        .await
    {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

async fn save_rights(
    State(state): State<AppState>,
    Path(script): Path<String>,
    headers: HeaderMap,
    request: Result<Json<SaveProductionRightsRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body(request) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .save_production_rights(&token, &script, request)
        .await
    {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

async fn preview(
    State(state): State<AppState>,
    Path(script): Path<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state.store.production_preview(&token, &script).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

async fn freeze(
    State(state): State<AppState>,
    Path(script): Path<String>,
    headers: HeaderMap,
    request: Result<Json<FreezeProductionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body(request) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .freeze_production(&token, &script, request)
        .await
    {
        Ok(value) => (StatusCode::CREATED, Json(value)).into_response(),
        Err(error) => error_response(error),
    }
}

async fn snapshot(
    State(state): State<AppState>,
    Path((script, snapshot)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .production_snapshot(&token, &script, &snapshot)
        .await
    {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

async fn eligibility(
    State(state): State<AppState>,
    Path((script, snapshot)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .production_eligibility(&token, &script, &snapshot)
        .await
    {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}

async fn approve(
    State(state): State<AppState>,
    Path((script, snapshot)): Path<(String, String)>,
    headers: HeaderMap,
    request: Result<Json<ApproveProductionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body(request) {
        Ok(value) => value,
        Err(status) => return failure(ErrorCode::InvalidRequest, status),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .approve_production(&token, &script, &snapshot, request)
        .await
    {
        Ok(value) => Json(value).into_response(),
        Err(error) => error_response(error),
    }
}
