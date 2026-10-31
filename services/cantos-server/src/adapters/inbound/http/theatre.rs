//! Public anonymous Theatre reads and playback authorization. No Studio session/cookie reuse.
use super::{audio_transport, AppState};
use crate::contexts::media::{ListenerError, ListenerService};
use crate::contract::theatre::discovery as dto;
use axum::{
    body::Body,
    extract::{rejection::PathRejection, DefaultBodyLimit, Extension, Path, Query},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/works", get(works))
        .route("/search", get(works))
        .route("/works/{work}", get(work))
        .route("/works/{work}/units", get(units))
        .route("/works/{work}/cover/{sha256}", get(cover))
        .route("/publications/{publication}", get(manifest))
        .route(
            "/publications/{publication}/playback-authorizations",
            post(playback),
        )
        .route("/media/{grant}", get(media))
        .fallback(theatre_not_found)
        .layer(DefaultBodyLimit::max(32 * 1024))
}
/// The crate-root fixture exercises these production handlers without a database or host switch.
#[cfg(test)]
pub(super) fn playback_contract_routes() -> Router {
    Router::new()
        .route("/publications/{publication}", get(manifest))
        .route(
            "/publications/{publication}/playback-authorizations",
            post(playback),
        )
        .route("/media/{grant}", get(media))
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
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WorkDetailQuery {}

#[cfg(test)]
mod generation_query_tests {
    use super::*;
    #[test]
    fn retired_presentation_selectors_are_refused() {
        for value in ["1", "2", "3", "4", ""] {
            let uri = format!("/works?presentation={value}").parse().unwrap();
            assert!(Query::<WorkQuery>::try_from_uri(&uri).is_err());
            assert!(Query::<UnitQuery>::try_from_uri(&uri).is_err());
            assert!(Query::<WorkDetailQuery>::try_from_uri(&uri).is_err());
        }
    }
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
    let search = q.q.as_deref().unwrap_or("");
    let (kind, language, limit) = (
        q.kind.as_deref(),
        q.language.as_deref(),
        q.limit.unwrap_or(20),
    );
    let cursor = q.cursor.as_deref();
    let result = service
        .metadata_works(search, kind, language, limit, cursor)
        .await
        .map(|v| Json(v).into_response());
    result.unwrap_or_else(error)
}
async fn work(
    Extension(service): Extension<ListenerService>,
    Path(work): Path<String>,
    query: Result<Query<WorkDetailQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(_)) = query else {
        return error(ListenerError::InvalidRequest);
    };
    let result = service
        .metadata_work(&work)
        .await
        .map(|v| Json(v).into_response());
    result.unwrap_or_else(error)
}
async fn units(
    Extension(service): Extension<ListenerService>,
    Path(work): Path<String>,
    query: Result<Query<UnitQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(q)) = query else {
        return error(ListenerError::InvalidRequest);
    };
    let (limit, cursor) = (q.limit.unwrap_or(20), q.cursor.as_deref());
    let result = service
        .metadata_units(&work, limit, cursor)
        .await
        .map(|v| Json(v).into_response());
    result.unwrap_or_else(error)
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
/// Origin-mode bytes (proposed ADR 0032). GET and HEAD share one path; every request is
/// re-admitted against current eligibility, and every response is private/no-store.
async fn media(
    Extension(service): Extension<ListenerService>,
    grant: Result<Path<String>, PathRejection>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let mut response = match grant {
        Ok(Path(grant)) => media_response(&service, &grant, &method, &headers).await,
        Err(_) => error(ListenerError::NotFound),
    };
    if method == Method::HEAD {
        *response.body_mut() = Body::empty();
    }
    audio_transport::private_response(response)
}
async fn media_response(
    service: &ListenerService,
    grant: &str,
    method: &Method,
    headers: &HeaderMap,
) -> Response {
    media_response_in(service, grant, method, headers, None).await
}
pub(super) async fn media_response_in(
    service: &ListenerService,
    grant: &str,
    method: &Method,
    headers: &HeaderMap,
    credential: Option<&str>,
) -> Response {
    let lane = match service.media_lane() {
        Ok(lane) => lane,
        Err(e) => return error(e),
    };
    let admission = match credential {
        Some(token) => service.authorize_shared_media(token, grant).await,
        None => service.authorize_media(grant).await,
    };
    let authorized = match admission {
        Ok(authorized) => authorized,
        Err(e) => return error(e),
    };
    let plan = match audio_transport::plan(
        headers,
        method,
        authorized.sha256(),
        authorized.byte_length(),
    ) {
        Ok(plan) => plan,
        Err(e) => return error(transport_error(e)),
    };
    if !plan.requires_storage() {
        let rejection = error(ListenerError::InvalidRequest);
        return audio_transport::metadata_response(plan, method, lane, rejection)
            .unwrap_or_else(|e| error(transport_error(e)));
    }
    let opened = match credential {
        Some(token) => {
            service
                .open_shared_media(token, &authorized, plan.range())
                .await
        }
        None => service.open_media(&authorized, plan.range()).await,
    };
    let stored = match opened {
        Ok(stored) => stored,
        Err(e) => return error(e),
    };
    audio_transport::response(stored, plan, method, lane)
        .unwrap_or_else(|e| error(transport_error(e)))
}
/// `public, no-cache`: any cache must revalidate before each use, so a revocation, expiry or
/// withdrawal takes effect on the next request (ADR 0033 rule 5). Errors stay `no-store`.
pub(super) const COVER_CACHE_CONTROL: HeaderValue = HeaderValue::from_static("public, no-cache");

/// Whether a path is exactly the Theatre cover route, for the response cache policy.
pub(super) fn cover_path(path: &str) -> bool {
    path.strip_prefix("/api/v1/theatre/works/")
        .and_then(|rest| rest.split_once("/cover/"))
        .is_some_and(|(work, sha)| {
            !work.is_empty() && !work.contains('/') && !sha.is_empty() && !sha.contains('/')
        })
}

/// Origin cover bytes (accepted ADR 0033). GET and HEAD re-project on every request and serve
/// only the image the projection shows now; every denial is the same anonymous `not_found`.
/// Ranges are not served: a `Range` header receives the full representation.
async fn cover(
    Extension(service): Extension<ListenerService>,
    path: Result<Path<(String, String)>, PathRejection>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let Ok(Path((work, sha256))) = path else {
        return error(ListenerError::NotFound);
    };
    let admitted = match service.admit_cover(&work, &sha256).await {
        Ok(admitted) => admitted,
        Err(e) => return error(e),
    };
    let etag = format!("\"{}\"", admitted.sha256());
    let Ok(etag) = HeaderValue::from_str(&etag) else {
        return error(ListenerError::Corrupt);
    };
    let not_modified = headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .any(|value| etag_matches(value, &etag));
    let mut response = if not_modified {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let stream = match service.open_cover(&admitted).await {
            Ok(stream) => stream,
            Err(e) => return error(e),
        };
        let body = if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from_stream(stream)
        };
        let mut response = body.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static(admitted.content_type()),
        );
        response.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from(admitted.byte_length()),
        );
        response
    };
    let headers = response.headers_mut();
    headers.insert(header::ETAG, etag);
    headers.insert(header::CACHE_CONTROL, COVER_CACHE_CONTROL);
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("none"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// `If-None-Match` uses weak comparison (RFC 9110 13.1.2): `*`, or any listed tag whose opaque
/// value equals the strong cover tag.
fn etag_matches(value: &HeaderValue, etag: &HeaderValue) -> bool {
    let (Ok(value), Ok(etag)) = (value.to_str(), etag.to_str()) else {
        return false;
    };
    value.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

fn transport_error(e: audio_transport::AudioTransportError) -> ListenerError {
    use audio_transport::AudioTransportError;
    match e {
        AudioTransportError::MalformedRange
        | AudioTransportError::MalformedIfRange
        | AudioTransportError::MalformedIfMatch
        | AudioTransportError::MalformedIfNoneMatch
        | AudioTransportError::UnsupportedMethod => ListenerError::InvalidRequest,
        AudioTransportError::InvalidRepresentation
        | AudioTransportError::InconsistentPlan
        | AudioTransportError::DeclaredLengthMismatch => ListenerError::Corrupt,
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
