//! Work-scoped delegation transport (ADR 0028). Owner routes ride the Studio cookie session.
//! Delegate routes accept exactly one `Authorization: Bearer cantos_dlg_…` header and refuse
//! any request that also carries a cookie or Studio session fence: one credential, one purpose.
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use cantos_contracts::{
    AppendContentRequest, DelegationGrantPage, ErrorCode, FieldIssue, IssueDelegationRequest,
};
use serde::Deserialize;

use super::super::{
    errors::error_response, studio::admission::cookie_token,
    studio::admission::STUDIO_SESSION_HEADER, AppState,
};
use super::catalog::{catalog_error, failure, json_only, page_cursor, CatalogBody};
use super::StudioPath;
use crate::application::StoreError;
use crate::contexts::catalog::{
    wire::{
        self as input, content as content_input,
        delegation::{self as delegation_input, DelegationSecret},
    },
    CatalogStoreError,
};

const BODY_LIMIT: usize = 128 * 1024;
const APPEND_PREFIX: &str = "/api/v1/studio/delegated/content-units/";

pub(super) fn owner_routes() -> Router<AppState> {
    Router::new()
        .route("/works/{work_id}/delegations", get(list).post(issue))
        .route("/works/{work_id}/delegations/{grant_id}", delete(revoke))
}

pub(super) fn delegate_routes() -> Router<AppState> {
    Router::new()
        .route("/delegated/grant", get(grant))
        .route("/delegated/works/{work_id}", get(work))
        .route("/delegated/works/{work_id}/content-units", get(units))
        .route("/delegated/content-units/{unit_id}", get(unit))
        .route("/delegated/content-units/{unit_id}/content/head", get(head))
        .route(
            "/delegated/content-units/{unit_id}/content/revisions",
            post(append),
        )
        .route(
            "/delegated/content-units/{unit_id}/content/revisions/{revision_id}",
            get(revision),
        )
        .route(
            "/delegated/content-units/{unit_id}/content/history",
            get(history),
        )
        .method_not_allowed_fallback(|| async {
            failure(
                StatusCode::METHOD_NOT_ALLOWED,
                ErrorCode::InvalidRequest,
                None,
                vec![],
            )
        })
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
        .layer(middleware::from_fn(json_only))
}

/// Bearer-only and cookie-free, so the Origin CSRF barrier does not apply to this mutation.
pub(in crate::adapters::inbound::http) fn bearer_mutation(
    method: &axum::http::Method,
    path: &str,
) -> bool {
    method == axum::http::Method::POST
        && path
            .strip_prefix(APPEND_PREFIX)
            .and_then(|rest| rest.strip_suffix("/content/revisions"))
            .is_some_and(|unit| !unit.is_empty() && !unit.contains('/'))
}

/// Exactly one delegation bearer and no other Studio credential.
fn bearer(headers: &HeaderMap) -> Result<DelegationSecret, StoreError> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next();
    if values.next().is_some()
        || headers.contains_key(header::COOKIE)
        || headers.contains_key(STUDIO_SESSION_HEADER)
    {
        return Err(StoreError::Unauthenticated);
    }
    value
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(delegation_input::bearer)
        .ok_or(StoreError::Unauthenticated)
}

fn invalid(status: StatusCode, issues: Vec<FieldIssue>) -> Response {
    failure(status, ErrorCode::InvalidCatalog, None, issues)
}

fn issue_field(path: &str, rule: &str) -> FieldIssue {
    FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}

/// Field-level refusals are semantic catalog errors, like Content Identity's.
fn delegation_error(error: CatalogStoreError) -> Response {
    match error {
        CatalogStoreError::Storage(StoreError::InvalidRequestFields(issues)) => {
            invalid(StatusCode::UNPROCESSABLE_ENTITY, issues)
        }
        error => catalog_error(error),
    }
}

async fn issue(
    State(state): State<AppState>,
    StudioPath(work_id): StudioPath<String>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<IssueDelegationRequest>,
) -> Response {
    let (operation, request) = match delegation_input::admit_issue(&work_id, body) {
        Ok(admitted) => admitted,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .issue_delegation(&token, &operation, request)
        .await
    {
        Ok(issued) => {
            // Only the creating response carries the secret; a retry reports it as unavailable.
            let (created, reply) = delegation_input::issued_response(issued);
            let status = if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            };
            (status, Json(reply)).into_response()
        }
        Err(error) => delegation_error(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListQuery {
    #[serde(default = "page_limit")]
    limit: u64,
    after: Option<String>,
}
fn page_limit() -> u64 {
    20
}

async fn list(
    State(state): State<AppState>,
    StudioPath(work_id): StudioPath<String>,
    headers: HeaderMap,
    query: Result<Query<ListQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error_response(StoreError::InvalidRequest);
    };
    let work_id = match input::validate_work_id(&work_id) {
        Ok(id) => id,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    if !(1..=50).contains(&query.limit) {
        return error_page(issue_field("limit", "out_of_range"));
    }
    if query
        .after
        .as_deref()
        .is_some_and(|after| delegation_input::grant_id(after).is_none())
    {
        return error_page(issue_field("after", "invalid_cursor"));
    }
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .delegations(&token, &work_id, query.after.as_deref(), query.limit)
        .await
    {
        Ok((items, next_after)) => Json(DelegationGrantPage {
            items: items.iter().map(delegation_input::grant_response).collect(),
            next_after,
        })
        .into_response(),
        Err(error) => delegation_error(error),
    }
}
fn error_page(issue: FieldIssue) -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidRequest,
        None,
        vec![issue],
    )
}

async fn revoke(
    State(state): State<AppState>,
    StudioPath((work_id, grant_id)): StudioPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let work_id = match input::validate_work_id(&work_id) {
        Ok(id) => id,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    let Some(grant_id) = delegation_input::grant_id(&grant_id) else {
        return invalid(
            StatusCode::UNPROCESSABLE_ENTITY,
            vec![issue_field("grant_id", "invalid_uuid")],
        );
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .revoke_delegation(&token, &work_id, &grant_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => delegation_error(error),
    }
}

async fn grant(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let secret = match bearer(&headers) {
        Ok(secret) => secret,
        Err(error) => return error_response(error),
    };
    match state.store.delegated_grant(&secret).await {
        Ok(view) => Json(delegation_input::grant_response(&view)).into_response(),
        Err(error) => delegation_error(error),
    }
}

async fn work(
    State(state): State<AppState>,
    StudioPath(id): StudioPath<String>,
    headers: HeaderMap,
) -> Response {
    let secret = match bearer(&headers) {
        Ok(secret) => secret,
        Err(error) => return error_response(error),
    };
    let id = match input::validate_work_id(&id) {
        Ok(id) => id,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    match state.store.delegated_work(&secret, &id).await {
        Ok(work) => Json(input::work(&work)).into_response(),
        Err(error) => delegation_error(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitsQuery {
    #[serde(default = "page_limit")]
    limit: u64,
    #[serde(default)]
    include_archived: bool,
    cursor: Option<String>,
}

async fn units(
    State(state): State<AppState>,
    StudioPath(id): StudioPath<String>,
    headers: HeaderMap,
    query: Result<Query<UnitsQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let secret = match bearer(&headers) {
        Ok(secret) => secret,
        Err(error) => return error_response(error),
    };
    let Ok(Query(query)) = query else {
        return error_response(StoreError::InvalidRequest);
    };
    let id = match input::validate_work_id(&id) {
        Ok(id) => id,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    let cursor = match page_cursor(query.limit, query.cursor.as_deref()) {
        Ok(cursor) => cursor,
        Err(issues) => {
            return failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::InvalidRequest,
                None,
                issues,
            )
        }
    };
    match state
        .store
        .delegated_units(
            &secret,
            &id,
            query.limit,
            query.include_archived,
            cursor.as_ref(),
        )
        .await
    {
        Ok(page) => match input::unit_page(page) {
            Ok(page) => Json(page).into_response(),
            Err(error) => delegation_error(error),
        },
        Err(error) => delegation_error(error),
    }
}

async fn unit(
    State(state): State<AppState>,
    StudioPath(id): StudioPath<String>,
    headers: HeaderMap,
) -> Response {
    let secret = match bearer(&headers) {
        Ok(secret) => secret,
        Err(error) => return error_response(error),
    };
    let id = match input::validate_unit_id(&id) {
        Ok(id) => id,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    match state.store.delegated_unit(&secret, &id).await {
        Ok(unit) => Json(input::unit(&unit)).into_response(),
        Err(error) => delegation_error(error),
    }
}

async fn head(
    State(state): State<AppState>,
    StudioPath(unit): StudioPath<String>,
    headers: HeaderMap,
) -> Response {
    read(state, unit, None, headers).await
}

async fn revision(
    State(state): State<AppState>,
    StudioPath((unit, revision)): StudioPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    read(state, unit, Some(revision), headers).await
}

async fn read(
    state: AppState,
    unit: String,
    revision: Option<String>,
    headers: HeaderMap,
) -> Response {
    let secret = match bearer(&headers) {
        Ok(secret) => secret,
        Err(error) => return error_response(error),
    };
    let unit = match input::validate_unit_id(&unit) {
        Ok(id) => id,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    let revision = match revision
        .as_deref()
        .map(content_input::validate_revision_id)
        .transpose()
    {
        Ok(id) => id,
        Err(error) => return delegation_error(error),
    };
    match state
        .store
        .delegated_content(&secret, &unit, revision.as_ref())
        .await
    {
        Ok(value) => match content_input::response(&value) {
            Ok(reply) => Json(reply).into_response(),
            Err(error) => delegation_error(error),
        },
        Err(error) => delegation_error(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryQuery {
    #[serde(default)]
    after_revision: u64,
    #[serde(default = "page_limit")]
    limit: u64,
}

async fn history(
    State(state): State<AppState>,
    StudioPath(unit): StudioPath<String>,
    headers: HeaderMap,
    query: Result<Query<HistoryQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let secret = match bearer(&headers) {
        Ok(secret) => secret,
        Err(error) => return error_response(error),
    };
    let Ok(Query(query)) = query else {
        return error_response(StoreError::InvalidRequest);
    };
    let unit = match input::validate_unit_id(&unit) {
        Ok(id) => id,
        Err(issues) => return invalid(StatusCode::UNPROCESSABLE_ENTITY, issues),
    };
    match state
        .store
        .delegated_content_history(&secret, &unit, query.after_revision, query.limit)
        .await
    {
        Ok(reply) => Json(reply).into_response(),
        Err(error) => delegation_error(error),
    }
}

async fn append(
    State(state): State<AppState>,
    path: Result<Path<String>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<AppendContentRequest>,
) -> Response {
    let secret = match bearer(&headers) {
        Ok(secret) => secret,
        Err(error) => return error_response(error),
    };
    let Ok(Path(unit)) = path else {
        return error_response(StoreError::InvalidRequest);
    };
    let (operation, command) = match content_input::admit_append(&unit, body) {
        Ok(admitted) => admitted,
        Err(error) => return delegation_error(error),
    };
    match state
        .store
        .delegated_append_content(&secret, &operation, command)
        .await
    {
        Ok(reply) => (StatusCode::CREATED, Json(reply)).into_response(),
        Err(error) => delegation_error(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderValue, Method};

    const TOKEN: &str =
        "cantos_dlg_000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn exactly_one_delegation_bearer_and_no_other_studio_credential_is_accepted() {
        let bearer_value = format!("Bearer {TOKEN}");
        assert_eq!(
            bearer(&headers(&[("authorization", &bearer_value)]))
                .map(|secret| secret.encode())
                .ok(),
            Some(TOKEN.to_owned())
        );
        let session = "a".repeat(64);
        for (case, pairs) in [
            ("missing", vec![]),
            (
                "duplicate",
                vec![
                    ("authorization", bearer_value.as_str()),
                    ("authorization", bearer_value.as_str()),
                ],
            ),
            (
                "cookie",
                vec![
                    ("authorization", bearer_value.as_str()),
                    ("cookie", "cantos_session=x"),
                ],
            ),
            (
                "fence",
                vec![
                    ("authorization", bearer_value.as_str()),
                    ("x-cantos-studio-session", "s"),
                ],
            ),
            ("scheme", vec![("authorization", "bearer cantos_dlg_00")]),
            ("basic", vec![("authorization", "Basic dXNlcjpwYXNz")]),
            (
                "session token",
                vec![("authorization", &format!("Bearer {session}"))],
            ),
            (
                "producer token",
                vec![("authorization", &format!("Bearer {}", "b".repeat(64)))],
            ),
        ] {
            assert!(
                matches!(bearer(&headers(&pairs)), Err(StoreError::Unauthenticated)),
                "{case}"
            );
        }
    }

    #[test]
    fn only_the_exact_delegated_append_route_bypasses_the_origin_barrier() {
        let unit = "20000000-0000-4000-8000-000000000001";
        let append = format!("{APPEND_PREFIX}{unit}/content/revisions");
        assert!(bearer_mutation(&Method::POST, &append));
        for (method, path) in [
            (Method::PUT, append.clone()),
            (Method::DELETE, append.clone()),
            (Method::POST, format!("{APPEND_PREFIX}/content/revisions")),
            (
                Method::POST,
                format!("{APPEND_PREFIX}{unit}/x/content/revisions"),
            ),
            (
                Method::POST,
                format!("/api/v1/studio/content-units/{unit}/content/revisions"),
            ),
            (
                Method::POST,
                "/api/v1/studio/works/w/delegations".to_owned(),
            ),
            (Method::POST, format!("{append}/extra")),
        ] {
            assert!(!bearer_mutation(&method, &path), "{method} {path}");
        }
    }
}
