//! Thin Studio owner and independently authenticated producer JSON adapters.
use super::super::{errors::error_response, studio::admission::cookie_token, AppState};
use super::catalog::CatalogBody;
use crate::application::StoreError;

fn brief_error(error: StoreError) -> Response {
    match error {
        StoreError::InvalidRequestFields(issues) => super::catalog::failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            cantos_contracts::ErrorCode::InvalidRequest,
            None,
            issues,
        ),
        error => error_response(error),
    }
}
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use cantos_contracts::{CreateProducerGrantRequest, CreateProductionBriefRequest};

pub(super) fn owner_routes() -> Router<AppState> {
    Router::new()
        .route("/content-units/{unit}/production-briefs", post(create))
        .route(
            "/content-units/{unit}/production-briefs/{version}",
            get(read),
        )
        .route(
            "/content-units/{unit}/production-briefs/{version}/producer-grants",
            post(grant),
        )
        .route(
            "/content-units/{unit}/producer-grants/{grant}",
            delete(revoke),
        )
}
pub(super) fn producer_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/producer/content-units/{unit}/production-briefs/{version}",
            get(export),
        )
        .method_not_allowed_fallback(|| async {
            super::catalog::failure(
                StatusCode::METHOD_NOT_ALLOWED,
                cantos_contracts::ErrorCode::InvalidRequest,
                None,
                vec![],
            )
        })
        .layer(middleware::from_fn(super::catalog::json_only))
}
async fn create(
    State(state): State<AppState>,
    path: Result<Path<String>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<CreateProductionBriefRequest>,
) -> Response {
    let Ok(Path(unit)) = path else {
        return error_response(StoreError::InvalidRequest);
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    match state
        .store
        .create_production_brief(&token, &unit, body)
        .await
    {
        Ok(reply) => (StatusCode::CREATED, Json(reply)).into_response(),
        Err(error) => brief_error(error),
    }
}
async fn read(
    State(state): State<AppState>,
    path: Result<Path<(String, u64)>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
) -> Response {
    let Ok(Path((unit, version))) = path else {
        return error_response(StoreError::InvalidRequest);
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    match state.store.production_brief(&token, &unit, version).await {
        Ok(reply) => Json(reply).into_response(),
        Err(error) => brief_error(error),
    }
}
async fn grant(
    State(state): State<AppState>,
    path: Result<Path<(String, u64)>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<CreateProducerGrantRequest>,
) -> Response {
    let Ok(Path((unit, version))) = path else {
        return error_response(StoreError::InvalidRequest);
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    match state
        .store
        .create_producer_grant(&token, &unit, version, body)
        .await
    {
        Ok(reply) => (StatusCode::CREATED, Json(reply)).into_response(),
        Err(error) => brief_error(error),
    }
}
async fn revoke(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
) -> Response {
    let Ok(Path((unit, grant))) = path else {
        return error_response(StoreError::InvalidRequest);
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(v) => v,
        Err(e) => return error_response(e),
    };
    match state
        .store
        .revoke_producer_grant(&token, &unit, &grant)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => brief_error(error),
    }
}
async fn export(
    State(state): State<AppState>,
    path: Result<Path<(String, u64)>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
) -> Response {
    let Ok(Path((unit, version))) = path else {
        return error_response(StoreError::InvalidRequest);
    };
    let mut authorizations = headers.get_all(header::AUTHORIZATION).iter();
    let bearer = authorizations
        .next()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    let Some(bearer) = bearer.filter(|_| authorizations.next().is_none()) else {
        return error_response(StoreError::Unauthenticated);
    };
    match state
        .store
        .producer_instructions(bearer, &unit, version)
        .await
    {
        Ok(reply) => Json(reply).into_response(),
        Err(error) => brief_error(error),
    }
}
