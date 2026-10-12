//! Content Identity is only mounted through the current Studio Catalog JSON router.
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use cantos_contracts::{AppendContentRequest, ErrorCode};
use serde::Deserialize;

use super::super::{cookie_token, error_response, AppState};
use super::catalog::{catalog_error, failure, CatalogBody};
use crate::contexts::catalog::{
    content::{wire, AppendContent},
    domain::{content::ContentRevisionId, ContentUnitId},
    wire::validate_unit_id,
    CatalogStoreError,
};
use crate::platform::postgres::StoreError;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/content-units/{unit_id}/content/head", get(head))
        .route(
            "/content-units/{unit_id}/content/revisions",
            axum::routing::post(append),
        )
        .route(
            "/content-units/{unit_id}/content/revisions/{revision_id}",
            get(revision),
        )
        .route("/content-units/{unit_id}/content/history", get(history))
}

fn content_error(error: CatalogStoreError) -> Response {
    match error {
        CatalogStoreError::Storage(StoreError::InvalidRequestFields(issues)) => failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            None,
            issues,
        ),
        error => catalog_error(error),
    }
}
fn unit_id(id: &str) -> Result<ContentUnitId, Response> {
    validate_unit_id(id).map_err(|issues| {
        failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            None,
            issues,
        )
    })
}
fn revision_id(id: &str) -> Result<ContentRevisionId, Response> {
    ContentRevisionId::parse(id).map_err(|_| {
        failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            None,
            vec![cantos_contracts::FieldIssue {
                path: "id".into(),
                rule: "invalid_uuid".into(),
            }],
        )
    })
}

async fn append(
    State(state): State<AppState>,
    path: Result<Path<String>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<AppendContentRequest>,
) -> Response {
    let Path(id) = match path {
        Ok(path) => path,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let unit_id = match unit_id(&id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let id = match revision_id(&body.id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let details = match wire::admit_details(body.details) {
        Ok(details) => details,
        Err(error) => return content_error(error),
    };
    let token = match cookie_token(&headers) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .append_content(
            &token,
            &body.operation_id,
            AppendContent {
                id,
                unit_id,
                expected_revision: body.expected_revision,
                expected_work_revision: body.expected_work_revision,
                details,
            },
        )
        .await
    {
        Ok(reply) => (StatusCode::CREATED, Json(reply)).into_response(),
        Err(error) => content_error(error),
    }
}
async fn head(
    State(state): State<AppState>,
    path: Result<Path<String>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
) -> Response {
    let Path(id) = match path {
        Ok(path) => path,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    read(state, id, None, headers).await
}
async fn revision(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
) -> Response {
    let Path((unit, revision)) = match path {
        Ok(path) => path,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let revision = match revision_id(&revision) {
        Ok(id) => id,
        Err(response) => return response,
    };
    read(state, unit, Some(revision), headers).await
}
async fn read(
    state: AppState,
    unit: String,
    revision: Option<ContentRevisionId>,
    headers: HeaderMap,
) -> Response {
    let unit = match unit_id(&unit) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let token = match cookie_token(&headers) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .content_revision(&token, &unit, revision.as_ref())
        .await
    {
        Ok(value) => match wire::response(&value) {
            Ok(reply) => Json(reply).into_response(),
            Err(error) => content_error(error),
        },
        Err(error) => content_error(error),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryQuery {
    #[serde(default)]
    after_revision: u64,
    #[serde(default = "history_limit")]
    limit: u64,
}
fn history_limit() -> u64 {
    20
}
async fn history(
    State(state): State<AppState>,
    path: Result<Path<String>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
    query: Result<Query<HistoryQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Path(id) = match path {
        Ok(path) => path,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let unit = match unit_id(&id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let token = match cookie_token(&headers) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .content_history(&token, &unit, query.after_revision, query.limit)
        .await
    {
        Ok(reply) => Json(reply).into_response(),
        Err(error) => content_error(error),
    }
}
