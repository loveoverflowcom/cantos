//! Thin account-owned Theatre HTTP boundary. Bearer identity is separate from Studio cookies.
use super::AppState;
use crate::contexts::listening::{
    auth::{
        AuthenticatedListener, AuthenticationError, ListenerAuthenticator, ListenerCredentials,
    },
    LibraryError, LibraryService,
};
use crate::contract::theatre::library as dto;
use axum::{
    extract::{Extension, Path, Query},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/account/presentation", get(account_presentation))
        .route("/library/continue", get(continue_listening))
        .route("/library/saved", get(saved_works))
        .route("/library/saved/{work}", get(saved).put(set_saved))
        .route("/library/history", get(history))
        .route("/library/history/playback-starts", post(playback_started))
        .route("/library/history/clear", post(clear_history))
        .route("/library/bookmarks", get(bookmarks))
        .route("/library/bookmarks/{bookmark}", put(create_bookmark))
        .route(
            "/library/bookmarks/{bookmark}/delete",
            post(delete_bookmark),
        )
        .route("/progress/{publication}", get(progress).put(put_progress))
        .route("/operations/{operation}", get(receipt))
}
async fn account_presentation(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
) -> Response {
    match principal(auth.as_ref(), &headers, trusted).await {
        Ok(principal) => {
            let mut response = Json(service.account_presentation(&principal)).into_response();
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static("no-store"),
            );
            response
        }
        Err(failure) => error(failure),
    }
}
async fn principal(
    auth: &dyn ListenerAuthenticator,
    headers: &HeaderMap,
    trusted: Option<Extension<AuthenticatedListener>>,
) -> Result<AuthenticatedListener, LibraryError> {
    if let Some(Extension(principal)) = trusted {
        return Ok(principal);
    }
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let one = values.next().ok_or(LibraryError::Unauthenticated)?;
    if values.next().is_some() {
        return Err(LibraryError::Unauthenticated);
    }
    let bearer = one
        .to_str()
        .ok()
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(LibraryError::Unauthenticated)?;
    let credentials =
        ListenerCredentials::bearer(bearer).map_err(|_| LibraryError::Unauthenticated)?;
    auth.authenticate(credentials).await.map_err(|e| match e {
        AuthenticationError::Unauthenticated => LibraryError::Unauthenticated,
        AuthenticationError::Unavailable => LibraryError::Unavailable,
    })
}
pub(super) fn error(e: LibraryError) -> Response {
    let (status, code, retryable) = match e {
        LibraryError::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            dto::ErrorCode::InvalidRequest,
            false,
        ),
        LibraryError::InvalidPosition => (
            StatusCode::UNPROCESSABLE_ENTITY,
            dto::ErrorCode::InvalidPosition,
            false,
        ),
        LibraryError::InvalidCursor => (
            StatusCode::BAD_REQUEST,
            dto::ErrorCode::InvalidCursor,
            false,
        ),
        LibraryError::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            dto::ErrorCode::Unauthenticated,
            false,
        ),
        LibraryError::NotFound => (StatusCode::NOT_FOUND, dto::ErrorCode::NotFound, false),
        LibraryError::OperationReused => (
            StatusCode::UNPROCESSABLE_ENTITY,
            dto::ErrorCode::IdempotencyKeyReused,
            false,
        ),
        LibraryError::StaleSequence => (
            StatusCode::CONFLICT,
            dto::ErrorCode::StaleDeviceSequence,
            false,
        ),
        LibraryError::RevisionExhausted => (
            StatusCode::CONFLICT,
            dto::ErrorCode::RevisionExhausted,
            false,
        ),
        LibraryError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            dto::ErrorCode::Unavailable,
            true,
        ),
        LibraryError::Corrupt => (
            StatusCode::INTERNAL_SERVER_ERROR,
            dto::ErrorCode::CorruptState,
            false,
        ),
    };
    (status, Json(dto::LibraryError { code, retryable })).into_response()
}
fn write_response(result: Result<dto::Receipt, LibraryError>) -> Response {
    match result {
        Ok(r) => {
            let status = match r.outcome {
                dto::WriteOutcome::Accepted | dto::WriteOutcome::AlreadyCurrent => StatusCode::OK,
                dto::WriteOutcome::RevisionConflict
                | dto::WriteOutcome::HistoryEpochConflict
                | dto::WriteOutcome::BookmarkDeleted => StatusCode::CONFLICT,
            };
            (status, Json(r)).into_response()
        }
        Err(e) => error(e),
    }
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<u16>,
    cursor: Option<String>,
    presentation: Option<Presentation>,
}
#[derive(Deserialize)]
enum Presentation {
    #[serde(rename = "1")]
    V1,
}
type PageInput = Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>;
type RawJson<T> = Result<Json<T>, axum::extract::rejection::JsonRejection>;
async fn continue_listening(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    query: PageInput,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Query(q)) = query else {
        return error(LibraryError::InvalidRequest);
    };
    if q.presentation.is_some() {
        return match service
            .presented_continue_listening(&p, q.limit.unwrap_or(20), q.cursor.as_deref())
            .await
        {
            Ok(v) => Json(v).into_response(),
            Err(e) => error(e),
        };
    }
    match service
        .continue_listening(&p, q.limit.unwrap_or(20), q.cursor.as_deref())
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn saved_works(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    query: PageInput,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Query(q)) = query else {
        return error(LibraryError::InvalidRequest);
    };
    if q.presentation.is_some() {
        return match service
            .presented_saved_works(&p, q.limit.unwrap_or(20), q.cursor.as_deref())
            .await
        {
            Ok(v) => Json(v).into_response(),
            Err(e) => error(e),
        };
    }
    match service
        .saved_works(&p, q.limit.unwrap_or(20), q.cursor.as_deref())
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn history(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    query: PageInput,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Query(q)) = query else {
        return error(LibraryError::InvalidRequest);
    };
    if q.presentation.is_some() {
        return match service
            .presented_history(&p, q.limit.unwrap_or(20), q.cursor.as_deref())
            .await
        {
            Ok(v) => Json(v).into_response(),
            Err(e) => error(e),
        };
    }
    match service
        .history(&p, q.limit.unwrap_or(20), q.cursor.as_deref())
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn bookmarks(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    query: PageInput,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    // Bookmark pages always carry their metadata, so the opt-in query of older pages is refused.
    let Ok(Query(q)) = query else {
        return error(LibraryError::InvalidRequest);
    };
    if q.presentation.is_some() {
        return error(LibraryError::InvalidRequest);
    }
    match service
        .bookmarks(&p, q.limit.unwrap_or(20), q.cursor.as_deref())
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn create_bookmark(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    Path(bookmark): Path<String>,
    body: RawJson<dto::BookmarkWrite>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Json(v)) = body else {
        return error(LibraryError::InvalidRequest);
    };
    write_response(service.create_bookmark(&p, &bookmark, v).await)
}
async fn delete_bookmark(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    Path(bookmark): Path<String>,
    body: RawJson<dto::BookmarkDelete>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Json(v)) = body else {
        return error(LibraryError::InvalidRequest);
    };
    write_response(service.delete_bookmark(&p, &bookmark, v).await)
}
async fn progress(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    Path(publication): Path<String>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    match service.progress(&p, &publication).await {
        Ok(current) => Json(dto::MutationState::Progress { current }).into_response(),
        Err(e) => error(e),
    }
}
async fn saved(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    Path(work): Path<String>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    match service.saved(&p, &work).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn receipt(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    Path(operation): Path<String>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    match service.receipt(&p, &operation).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn put_progress(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    Path(publication): Path<String>,
    body: RawJson<dto::ProgressWrite>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Json(v)) = body else {
        return error(LibraryError::InvalidRequest);
    };
    write_response(service.put_progress(&p, &publication, v).await)
}
async fn set_saved(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    Path(work): Path<String>,
    body: RawJson<dto::SavedWrite>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Json(v)) = body else {
        return error(LibraryError::InvalidRequest);
    };
    write_response(service.set_saved(&p, &work, v).await)
}
async fn playback_started(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    body: RawJson<dto::HistoryStart>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Json(v)) = body else {
        return error(LibraryError::InvalidRequest);
    };
    write_response(service.playback_started(&p, v).await)
}
async fn clear_history(
    Extension(service): Extension<LibraryService>,
    Extension(auth): Extension<Arc<dyn ListenerAuthenticator>>,
    trusted: Option<Extension<AuthenticatedListener>>,
    headers: HeaderMap,
    body: RawJson<dto::HistoryClear>,
) -> Response {
    let p = match principal(auth.as_ref(), &headers, trusted).await {
        Ok(p) => p,
        Err(e) => return error(e),
    };
    let Ok(Json(v)) = body else {
        return error(LibraryError::InvalidRequest);
    };
    write_response(service.clear_history(&p, v).await)
}
/// These exact bearer-only account mutations have no ambient cookie authority or Studio CSRF.
pub(super) fn bearer_mutation(method: &axum::http::Method, path: &str) -> bool {
    if method == axum::http::Method::POST {
        return matches!(
            path,
            "/api/v1/theatre/library/history/clear"
                | "/api/v1/theatre/library/history/playback-starts"
        ) || path
            .strip_prefix("/api/v1/theatre/library/bookmarks/")
            .and_then(|rest| rest.strip_suffix("/delete"))
            .is_some_and(|id| !id.is_empty() && !id.contains('/'));
    }
    if method != axum::http::Method::PUT {
        return false;
    }
    [
        "/api/v1/theatre/progress/",
        "/api/v1/theatre/library/saved/",
        "/api/v1/theatre/library/bookmarks/",
    ]
    .iter()
    .any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    #[test]
    fn library_presentation_query_requires_the_exact_supported_opt_in() {
        use axum::http::Uri;
        let q = Query::<PageQuery>::try_from_uri(&Uri::from_static("/?presentation=1&limit=1"));
        assert!(q.unwrap().presentation.is_some());
        assert!(
            Query::<PageQuery>::try_from_uri(&Uri::from_static("/?limit=1"))
                .unwrap()
                .presentation
                .is_none()
        );
        for raw in [
            "/?presentation=0",
            "/?presentation=true",
            "/?presentation=2",
            "/?presentation=",
            "/?private=true",
        ] {
            assert!(Query::<PageQuery>::try_from_uri(&raw.parse::<Uri>().unwrap()).is_err());
        }
    }
    #[test]
    fn bookmark_mutations_are_bearer_only_in_exactly_their_shapes() {
        use axum::http::Method;
        let id = "00000000-0000-4000-8000-0000000000b1";
        let create = format!("/api/v1/theatre/library/bookmarks/{id}");
        let delete = format!("{create}/delete");
        assert!(bearer_mutation(&Method::PUT, &create));
        assert!(bearer_mutation(&Method::POST, &delete));
        for (method, path) in [
            (Method::PUT, "/api/v1/theatre/library/bookmarks/".to_owned()),
            (Method::PUT, format!("{create}/delete")),
            (Method::PUT, format!("{create}/x")),
            (Method::POST, create.clone()),
            (
                Method::POST,
                "/api/v1/theatre/library/bookmarks//delete".to_owned(),
            ),
            (Method::POST, format!("{delete}/x")),
            (Method::DELETE, create.clone()),
            (Method::GET, create.clone()),
            (
                Method::PUT,
                format!("/api/v1/studio/library/bookmarks/{id}"),
            ),
        ] {
            assert!(!bearer_mutation(&method, &path), "{method} {path}");
        }
    }
    #[tokio::test]
    async fn error_projection_is_total_and_never_leaks_private_or_sql_fields() {
        for (e, status, code, retryable) in [
            (LibraryError::InvalidRequest, 400, "invalid_request", false),
            (
                LibraryError::InvalidPosition,
                422,
                "invalid_position",
                false,
            ),
            (LibraryError::InvalidCursor, 400, "invalid_cursor", false),
            (LibraryError::Unauthenticated, 401, "unauthenticated", false),
            (LibraryError::NotFound, 404, "not_found", false),
            (
                LibraryError::OperationReused,
                422,
                "idempotency_key_reused",
                false,
            ),
            (
                LibraryError::StaleSequence,
                409,
                "stale_device_sequence",
                false,
            ),
            (
                LibraryError::RevisionExhausted,
                409,
                "revision_exhausted",
                false,
            ),
            (LibraryError::Unavailable, 503, "unavailable", true),
            (LibraryError::Corrupt, 500, "corrupt_state", false),
        ] {
            let response = error(e);
            assert_eq!(response.status().as_u16(), status);
            let v: serde_json::Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(v, serde_json::json!({"code":code,"retryable":retryable}));
        }
    }
}
