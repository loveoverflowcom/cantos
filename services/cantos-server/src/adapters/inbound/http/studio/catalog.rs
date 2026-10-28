//! JSON-only Catalog adapter. Admission maps raw fields to typed commands; the application
//! owns current access, locked PATCH resolution, immutable history and atomic receipts.
use crate::contract::{
    codec::WireFormat,
    studio::{
        ApiError, ArchiveContentUnitRequest, ArchiveWorkRequest, CreateContentUnitRequest,
        CreateWorkRequest, ErrorCode, FieldIssue, PatchContentUnitRequest, PatchWorkRequest,
        ReorderContentUnitsRequest, SampleMetadataDisclosureRequest, SampleMetadataRightsRequest,
    },
};
use axum::{
    extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Query, Request, State},
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{de::DeserializeOwned, Deserialize};

use super::super::{errors::error_response, studio::admission::cookie_token, wire, AppState};
use crate::contexts::catalog::{
    domain::CatalogError, wire as input, CatalogCommand, CatalogCursor, CatalogStoreError,
};

const BODY_LIMIT: usize = 128 * 1024;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/works", get(list_works).post(create_work))
        .route(
            "/works/{work_id}",
            get(get_work).patch(patch_work).delete(archive_work),
        )
        .route(
            "/works/{work_id}/content-units",
            get(list_units).post(create_unit),
        )
        .route("/works/{work_id}/content-units/reorder", post(reorder))
        .route("/works/{work_id}/history", get(history))
        .route("/sample-metadata-rights", post(record_sample_rights))
        .route(
            "/works/{work_id}/sample-metadata-disclosures",
            post(record_sample_work),
        )
        .route(
            "/works/{work_id}/content-units/{unit_id}/sample-metadata-disclosures",
            post(record_sample_unit),
        )
        .route(
            "/content-units/{unit_id}",
            get(get_unit).patch(patch_unit).delete(archive_unit),
        )
        .merge(super::content::routes())
        .merge(super::briefs::owner_routes())
        .merge(super::delegation::owner_routes())
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

pub(super) async fn json_only(request: Request, next: Next) -> Response {
    let accept = wire::accept_header(request.headers());
    let mut response = if wire::json_response_format(accept.as_deref()).is_err() {
        failure(
            StatusCode::NOT_ACCEPTABLE,
            ErrorCode::InvalidRequest,
            None,
            vec![issue("accept", "not_acceptable")],
        )
    } else {
        next.run(request).await
    };
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("accept"));
    response
}

/// Path syntax failures also use the shared JSON envelope.
struct CatalogId(String);
impl<S: Send + Sync> FromRequestParts<S> for CatalogId {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Response> {
        Path::<String>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| Self(value))
            .map_err(|_| {
                failure(
                    StatusCode::BAD_REQUEST,
                    ErrorCode::InvalidRequest,
                    None,
                    vec![],
                )
            })
    }
}

/// Map decoder/body-limit failures to the shared JSON error envelope before mutation.
pub(super) struct CatalogBody<T>(pub(super) T);
impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for CatalogBody<T> {
    type Rejection = Response;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok());
        if !matches!(wire::request_format(content_type), Ok(WireFormat::Json)) {
            return Err(failure(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ErrorCode::InvalidRequest,
                None,
                vec![issue("content-type", "unsupported_media_type")],
            ));
        }
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|error| {
                let status = if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                };
                failure(status, ErrorCode::InvalidRequest, None, vec![])
            })
    }
}

async fn mutate(
    state: AppState,
    headers: HeaderMap,
    admitted: Result<(String, CatalogCommand), Vec<FieldIssue>>,
    status: StatusCode,
) -> Response {
    let (operation, command) = match admitted {
        Ok(command) => command,
        Err(issues) => {
            return failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidCatalog,
                None,
                issues,
            )
        }
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .catalog_execute(&token, &operation, command)
        .await
    {
        Ok(result) => (status, Json(input::outcome(result))).into_response(),
        Err(error) => catalog_error(error),
    }
}

async fn create_work(
    State(state): State<AppState>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<CreateWorkRequest>,
) -> Response {
    mutate(
        state,
        headers,
        input::create_work(body),
        StatusCode::CREATED,
    )
    .await
}

async fn sample_decision(
    state: AppState,
    headers: HeaderMap,
    work: String,
    unit: Option<String>,
    body: SampleMetadataDisclosureRequest,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .sample_metadata_decide(&token, &work, unit.as_deref(), body)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => catalog_error(error),
    }
}

async fn record_sample_rights(
    State(state): State<AppState>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<SampleMetadataRightsRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .record_sample_metadata_rights(&token, body)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => catalog_error(error),
    }
}

async fn record_sample_work(
    State(state): State<AppState>,
    CatalogId(work): CatalogId,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<SampleMetadataDisclosureRequest>,
) -> Response {
    sample_decision(state, headers, work, None, body).await
}

async fn record_sample_unit(
    State(state): State<AppState>,
    path: Result<Path<(String, String)>, axum::extract::rejection::PathRejection>,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<SampleMetadataDisclosureRequest>,
) -> Response {
    let Ok(Path((work, unit))) = path else {
        return failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            vec![],
        );
    };
    sample_decision(state, headers, work, Some(unit), body).await
}
async fn patch_work(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<PatchWorkRequest>,
) -> Response {
    mutate(state, headers, input::patch_work(&id, body), StatusCode::OK).await
}
async fn archive_work(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<ArchiveWorkRequest>,
) -> Response {
    mutate(
        state,
        headers,
        input::archive_work(&id, body),
        StatusCode::OK,
    )
    .await
}
async fn create_unit(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<CreateContentUnitRequest>,
) -> Response {
    mutate(
        state,
        headers,
        input::create_unit(&id, body),
        StatusCode::CREATED,
    )
    .await
}
async fn patch_unit(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<PatchContentUnitRequest>,
) -> Response {
    mutate(state, headers, input::patch_unit(&id, body), StatusCode::OK).await
}
async fn archive_unit(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<ArchiveContentUnitRequest>,
) -> Response {
    mutate(
        state,
        headers,
        input::archive_unit(&id, body),
        StatusCode::OK,
    )
    .await
}
async fn reorder(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    CatalogBody(body): CatalogBody<ReorderContentUnitsRequest>,
) -> Response {
    mutate(state, headers, input::reorder(&id, body), StatusCode::OK).await
}

async fn get_work(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
) -> Response {
    let id = match input::validate_work_id(&id) {
        Ok(id) => id,
        Err(issues) => {
            return failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidCatalog,
                None,
                issues,
            )
        }
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.catalog_work(&token, &id).await {
        Ok(work) => Json(input::work(&work)).into_response(),
        Err(error) => catalog_error(error),
    }
}
async fn get_unit(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
) -> Response {
    let id = match input::validate_unit_id(&id) {
        Ok(id) => id,
        Err(issues) => {
            return failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidCatalog,
                None,
                issues,
            )
        }
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.catalog_unit(&token, &id).await {
        Ok(unit) => Json(input::unit(&unit)).into_response(),
        Err(error) => catalog_error(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    #[serde(default = "page_limit")]
    limit: u64,
    #[serde(default)]
    include_archived: bool,
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryQuery {
    #[serde(default = "page_limit")]
    limit: u64,
    cursor: Option<String>,
}
fn page_limit() -> u64 {
    20
}
pub(super) fn page_cursor(
    limit: u64,
    cursor: Option<&str>,
) -> Result<Option<CatalogCursor>, Vec<FieldIssue>> {
    if !(1..=50).contains(&limit) {
        return Err(vec![issue("limit", "out_of_range")]);
    }
    cursor
        .map(CatalogCursor::decode)
        .transpose()
        .map_err(|_| vec![issue("cursor", "invalid_cursor")])
}

async fn list_works(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return super::super::errors::error_response(
                crate::application::StoreError::InvalidRequest,
            )
        }
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
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .catalog_works(&token, query.limit, query.include_archived, cursor.as_ref())
        .await
    {
        Ok(page) => match input::work_page(page) {
            Ok(page) => Json(page).into_response(),
            Err(error) => catalog_error(error),
        },
        Err(error) => catalog_error(error),
    }
}
async fn list_units(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    query: Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return super::super::errors::error_response(
                crate::application::StoreError::InvalidRequest,
            )
        }
    };
    let id = match input::validate_work_id(&id) {
        Ok(id) => id,
        Err(issues) => {
            return failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidCatalog,
                None,
                issues,
            )
        }
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
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .catalog_units(
            &token,
            &id,
            query.limit,
            query.include_archived,
            cursor.as_ref(),
        )
        .await
    {
        Ok(page) => match input::unit_page(page) {
            Ok(page) => Json(page).into_response(),
            Err(error) => catalog_error(error),
        },
        Err(error) => catalog_error(error),
    }
}
async fn history(
    State(state): State<AppState>,
    CatalogId(id): CatalogId,
    headers: HeaderMap,
    query: Result<Query<HistoryQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => {
            return super::super::errors::error_response(
                crate::application::StoreError::InvalidRequest,
            )
        }
    };
    let id = match input::validate_work_id(&id) {
        Ok(id) => id,
        Err(issues) => {
            return failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidCatalog,
                None,
                issues,
            )
        }
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
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .catalog_work_history(&token, &id, query.limit, cursor.as_ref())
        .await
    {
        Ok(page) => match input::work_page(page) {
            Ok(page) => Json(page).into_response(),
            Err(error) => catalog_error(error),
        },
        Err(error) => catalog_error(error),
    }
}

fn issue(path: &str, rule: &str) -> FieldIssue {
    FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}
pub(super) fn failure(
    status: StatusCode,
    code: ErrorCode,
    current_revision: Option<u64>,
    issues: Vec<FieldIssue>,
) -> Response {
    (
        status,
        Json(ApiError {
            code,
            current_revision,
            issues,
        }),
    )
        .into_response()
}
pub(super) fn catalog_error(error: CatalogStoreError) -> Response {
    match error {
        CatalogStoreError::Storage(error) => error_response(error),
        CatalogStoreError::OperationReused => failure(
            StatusCode::CONFLICT,
            ErrorCode::OperationReused,
            None,
            vec![],
        ),
        CatalogStoreError::DuplicateId => failure(
            StatusCode::CONFLICT,
            ErrorCode::CatalogConflict,
            None,
            vec![issue("id", "duplicate_id")],
        ),
        CatalogStoreError::StaleCursor => {
            failure(StatusCode::CONFLICT, ErrorCode::StaleCursor, None, vec![])
        }
        CatalogStoreError::InvalidCursor => failure(
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            vec![issue("cursor", "invalid_cursor")],
        ),
        CatalogStoreError::Limit => failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            None,
            vec![issue("catalog", "limit_exceeded")],
        ),
        CatalogStoreError::Domain(CatalogError::RevisionConflict {
            current_revision, ..
        }) => failure(
            StatusCode::CONFLICT,
            ErrorCode::StaleRevision,
            Some(current_revision),
            vec![],
        ),
        CatalogStoreError::Domain(error) => domain_error(error),
    }
}
fn domain_error(error: CatalogError) -> Response {
    use CatalogError::*;
    let (status, code, path, rule) = match error {
        WorkArchived | UnitArchived | UnexpectedLifecycle { .. } => (
            StatusCode::CONFLICT,
            ErrorCode::CatalogConflict,
            "lifecycle",
            "archived",
        ),
        RevisionLimit => (
            StatusCode::CONFLICT,
            ErrorCode::CatalogConflict,
            "revision",
            "revision_limit",
        ),
        WrongContentUnitKind { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "kind",
            "wrong_content_unit_kind",
        ),
        WrongWork { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "work_id",
            "wrong_work",
        ),
        DuplicateContentUnit { .. } | DuplicateReorderUnit { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "ordered",
            "duplicate_unit",
        ),
        UnknownReorderUnit { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "ordered",
            "unknown_unit",
        ),
        ArchivedReorderUnit { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "ordered",
            "archived_unit",
        ),
        MissingReorderUnit { .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "ordered",
            "missing_unit",
        ),
        DuplicatePosition { .. } | InvalidOrder => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "ordered",
            "invalid_order",
        ),
        InvalidId(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "id",
            "invalid_id",
        ),
        UnknownWorkKind | UnknownContentUnitKind => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "kind",
            "unknown_kind",
        ),
        UnknownVisibility => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "details.visibility",
            "unknown_visibility",
        ),
        BlankField { field } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            field,
            "blank",
        ),
        FieldTooLong { field, .. } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            field,
            "too_long",
        ),
        ControlCharacter { field } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            field,
            "control_character",
        ),
        InvalidLanguage => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "details.language",
            "invalid_language",
        ),
        TooManyRightsRefs | InvalidRightsRef | DuplicateRightsRef => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "details.rights_refs",
            "invalid_rights_refs",
        ),
        InvalidSourceId | InvalidSourceDigest => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "details.source",
            "invalid_source",
        ),
        InvalidScriptId | InvalidScriptDigest => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "details.script",
            "invalid_script",
        ),
        InvalidContentDigest => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "details.content_digest",
            "invalid_digest",
        ),
        InvalidRevision => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidCatalog,
            "expected_revision",
            "invalid_revision",
        ),
        UnknownLifecycle | InvalidTimestamp | TimestampRegression => {
            return failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::CorruptRevision,
                None,
                vec![],
            )
        }
        RevisionConflict {
            current_revision, ..
        } => {
            return failure(
                StatusCode::CONFLICT,
                ErrorCode::StaleRevision,
                Some(current_revision),
                vec![],
            )
        }
    };
    failure(status, code, None, vec![issue(path, rule)])
}
