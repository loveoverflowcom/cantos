//! Public anonymous Theatre reads and playback authorization. No Studio session/cookie reuse.
use super::AppState;
use crate::contexts::media::{ListenerError, ListenerService};
use axum::{
    extract::{DefaultBodyLimit, Extension, Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::theatre as dto;
use serde::Deserialize;
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/works", get(works))
        .route("/search", get(works))
        .route("/works/{work}", get(work))
        .route("/works/{work}/units", get(units))
        .route("/publications/{publication}", get(manifest))
        .route(
            "/publications/{publication}/playback-authorizations",
            post(playback),
        )
        .fallback(theatre_not_found)
        .layer(DefaultBodyLimit::max(32 * 1024))
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WorkQuery {
    q: Option<String>,
    kind: Option<String>,
    language: Option<String>,
    limit: Option<u16>,
    cursor: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct UnitQuery {
    limit: Option<u16>,
    cursor: Option<String>,
}
pub(super) fn error(e: ListenerError) -> Response {
    let (status, code, retryable) = match e {
        ListenerError::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            dto::ListenerErrorCode::InvalidRequest,
            false,
        ),
        ListenerError::InvalidCursor => (
            StatusCode::BAD_REQUEST,
            dto::ListenerErrorCode::InvalidCursor,
            false,
        ),
        ListenerError::NotFound => (
            StatusCode::NOT_FOUND,
            dto::ListenerErrorCode::NotFound,
            false,
        ),
        ListenerError::RenditionUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            dto::ListenerErrorCode::RenditionUnavailable,
            true,
        ),
        ListenerError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            dto::ListenerErrorCode::Unavailable,
            true,
        ),
        ListenerError::Corrupt => (
            StatusCode::INTERNAL_SERVER_ERROR,
            dto::ListenerErrorCode::Unavailable,
            false,
        ),
    };
    (status, Json(dto::ListenerError { code, retryable })).into_response()
}
async fn works(
    Extension(service): Extension<ListenerService>,
    query: Result<Query<WorkQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(q)) = query else {
        return error(ListenerError::InvalidRequest);
    };
    match service
        .works(
            q.q.as_deref().unwrap_or(""),
            q.kind.as_deref(),
            q.language.as_deref(),
            q.limit.unwrap_or(20),
            q.cursor.as_deref(),
        )
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn work(
    Extension(service): Extension<ListenerService>,
    Path(work): Path<String>,
) -> Response {
    match service.work(&work).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn units(
    Extension(service): Extension<ListenerService>,
    Path(work): Path<String>,
    query: Result<Query<UnitQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(q)) = query else {
        return error(ListenerError::InvalidRequest);
    };
    match service
        .units(&work, q.limit.unwrap_or(20), q.cursor.as_deref())
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn manifest(
    Extension(service): Extension<ListenerService>,
    Path(id): Path<String>,
) -> Response {
    match service.manifest(&id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn playback(
    Extension(service): Extension<ListenerService>,
    Path(id): Path<String>,
    body: Result<axum::body::Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let Ok(body) = body else {
        return error(ListenerError::InvalidRequest);
    };
    if !body.is_empty() && body.as_ref() != b"{}" {
        return error(ListenerError::InvalidRequest);
    }
    match service.authorize_playback(&id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn theatre_not_found() -> Response {
    error(ListenerError::NotFound)
}
/// Anonymous public playback has no ambient cookie authority, so Studio's CSRF origin rule
/// must not block CMP/native callers. This exception names only this exact public POST shape.
pub(super) fn public_playback_post(method: &axum::http::Method, path: &str) -> bool {
    method == axum::http::Method::POST
        && path
            .strip_prefix("/api/v1/theatre/publications/")
            .and_then(|v| v.strip_suffix("/playback-authorizations"))
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}
#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    #[tokio::test]
    async fn exhaustive_safe_error_mapping() {
        for (e, status, code, retry) in [
            (ListenerError::InvalidRequest, 400, "invalid_request", false),
            (ListenerError::InvalidCursor, 400, "invalid_cursor", false),
            (ListenerError::NotFound, 404, "not_found", false),
            (
                ListenerError::RenditionUnavailable,
                503,
                "rendition_unavailable",
                true,
            ),
            (ListenerError::Unavailable, 503, "unavailable", true),
            (ListenerError::Corrupt, 500, "unavailable", false),
        ] {
            let r = error(e);
            assert_eq!(r.status().as_u16(), status);
            let v: serde_json::Value =
                serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
            assert_eq!(v, serde_json::json!({"code":code,"retryable":retry}));
        }
    }
}
