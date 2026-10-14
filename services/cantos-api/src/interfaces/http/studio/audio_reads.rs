//! Binary private audio delivery is independent of JSON/MessagePack metadata negotiation.
use super::StudioPath;
use crate::{
    contexts::media::read::{
        AudioReadScope, ByteRange, NormalizedByteRange, RangeError, StorageError, StorageStream,
    },
    interfaces::http::{cookie_token, error_response, failure, AppState},
    platform::postgres::StoreError,
};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use cantos_contracts::ErrorCode;
use futures_util::StreamExt;
use std::{
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

static READ_REQUESTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/scripts/{script}/audio-intents/{intent}/bytes", get(intent_bytes))
        .route("/scripts/{script}/audio-intents/{intent}/read-access", get(intent_access))
        .route("/content-units/{unit}/audio-submissions/{submission}/renditions/{rendition}/bytes", get(rendition_bytes))
        .route("/content-units/{unit}/audio-submissions/{submission}/renditions/{rendition}/read-access", get(rendition_access))
}
async fn intent_bytes(
    State(state): State<AppState>,
    StudioPath((script, intent)): StudioPath<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    read(
        state,
        AudioReadScope::ScriptIntent { script, intent },
        method,
        headers,
    )
    .await
}
async fn rendition_bytes(
    State(state): State<AppState>,
    StudioPath((unit, submission, rendition)): StudioPath<(String, String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    read(
        state,
        AudioReadScope::Rendition {
            unit,
            submission,
            rendition,
        },
        method,
        headers,
    )
    .await
}
async fn intent_access(
    State(state): State<AppState>,
    StudioPath((script, intent)): StudioPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/api/v1/studio/scripts/{script}/audio-intents/{intent}/bytes");
    access(
        state,
        AudioReadScope::ScriptIntent { script, intent },
        path,
        headers,
    )
    .await
}
async fn rendition_access(
    State(state): State<AppState>,
    StudioPath((unit, submission, rendition)): StudioPath<(String, String, String)>,
    headers: HeaderMap,
) -> Response {
    let path = format!("/api/v1/studio/content-units/{unit}/audio-submissions/{submission}/renditions/{rendition}/bytes");
    access(
        state,
        AudioReadScope::Rendition {
            unit,
            submission,
            rendition,
        },
        path,
        headers,
    )
    .await
}
async fn access(
    state: AppState,
    scope: AudioReadScope,
    path: String,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    let authorized = match state.store.authorize_audio_read(&token, scope).await {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let grant = match state.store.audio_read_grant(&token, &authorized).await {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let (url, expiry) = match grant {
        Some(grant) => {
            let (url, expiry) = grant.into_parts();
            (url, Some(expiry))
        }
        None => (path, None),
    };
    let location = match HeaderValue::from_str(&url) {
        Ok(value) => value,
        Err(_) => return error_response(StoreError::CorruptRevision),
    };
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(header::LOCATION, location);
    if let Some(expiry) = expiry {
        match HeaderValue::from_str(&expiry.to_string()) {
            Ok(value) => {
                response
                    .headers_mut()
                    .insert("x-cantos-read-expires-at", value);
            }
            Err(_) => return error_response(StoreError::CorruptRevision),
        }
    }
    response
}
async fn read(
    state: AppState,
    scope: AudioReadScope,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let permit = match READ_REQUESTS.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return error_response(StoreError::Unavailable),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    let authorized = match state.store.authorize_audio_read(&token, scope).await {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let length = authorized.byte_length();
    let etag = format!("\"{}\"", authorized.sha256());
    let range = match selected_range(&headers, &method, &etag, length) {
        Ok(range) => range,
        Err(RangeError::Unsatisfiable) => {
            let mut response =
                failure(ErrorCode::InvalidRequest, StatusCode::RANGE_NOT_SATISFIABLE);
            if insert_header(
                response.headers_mut(),
                header::CONTENT_RANGE,
                &format!("bytes */{length}"),
            )
            .is_err()
            {
                return error_response(StoreError::CorruptRevision);
            }
            return response;
        }
        Err(RangeError::Invalid | RangeError::MultipleRanges | RangeError::Overflow) => {
            return error_response(StoreError::InvalidRequest)
        }
    };
    let stored = match state
        .store
        .open_audio_read(&token, &authorized, range)
        .await
    {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    let expected = range.map_or(length, NormalizedByteRange::byte_length);
    if stored.byte_length != expected {
        return error_response(StoreError::CorruptRevision);
    }
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(bounded_stream(stored.stream, expected, permit))
    };
    let mut response = Response::new(body);
    *response.status_mut() = if range.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/ogg"));
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    for (name, value) in [
        (header::CONTENT_LENGTH, expected.to_string()),
        (header::ETAG, etag),
    ] {
        if insert_header(headers, name, &value).is_err() {
            return error_response(StoreError::CorruptRevision);
        }
    }
    if let Some(range) = range {
        if insert_header(
            headers,
            header::CONTENT_RANGE,
            &format!("bytes {}-{}/{length}", range.first(), range.last()),
        )
        .is_err()
        {
            return error_response(StoreError::CorruptRevision);
        }
    }
    response
}
fn insert_header(headers: &mut HeaderMap, name: header::HeaderName, value: &str) -> Result<(), ()> {
    headers.insert(name, HeaderValue::from_str(value).map_err(|_| ())?);
    Ok(())
}
fn selected_range(
    headers: &HeaderMap,
    method: &Method,
    etag: &str,
    length: u64,
) -> Result<Option<NormalizedByteRange>, RangeError> {
    if *method == Method::HEAD {
        return Ok(None);
    }
    if headers.get_all(header::RANGE).iter().count() > 1
        || headers.get_all(header::IF_RANGE).iter().count() > 1
    {
        return Err(RangeError::Invalid);
    }
    if headers
        .get(header::IF_RANGE)
        .is_some_and(|value| value.to_str().ok() != Some(etag))
    {
        return Ok(None);
    }
    let Some(value) = headers.get(header::RANGE) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| RangeError::Invalid)?;
    let (unit, specification) = value.split_once('=').ok_or(RangeError::Invalid)?;
    // RFC9110 range units are case-insensitive; unrecognized units must be ignored.
    if !unit.eq_ignore_ascii_case("bytes") {
        return Ok(None);
    }
    ByteRange::parse(&format!("bytes={specification}"))
        .and_then(|range| range.normalize(length))
        .map(Some)
}
fn bounded_stream(
    stream: StorageStream,
    length: u64,
    permit: OwnedSemaphorePermit,
) -> StorageStream {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let stream = futures_util::stream::unfold(
        (Some(stream), length, permit),
        move |(stream, remaining, permit)| async move {
            let mut stream = stream?;
            match tokio::time::timeout_at(deadline, stream.next()).await {
                Ok(Some(Ok(bytes))) if bytes.len() as u64 <= remaining => {
                    let remaining = remaining - bytes.len() as u64;
                    Some((Ok(bytes), (Some(stream), remaining, permit)))
                }
                Ok(Some(Ok(_))) => Some((Err(StorageError::LengthMismatch), (None, 0, permit))),
                Ok(Some(Err(error))) => Some((Err(error), (None, 0, permit))),
                Ok(None) if remaining == 0 => None,
                Ok(None) => Some((Err(StorageError::LengthMismatch), (None, 0, permit))),
                Err(_) => Some((Err(StorageError::Timeout), (None, 0, permit))),
            }
        },
    );
    Box::pin(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    #[test]
    fn http_ranges_follow_case_insensitive_units_if_range_and_head_semantics() {
        let etag = "\"content-sha\"";
        let mut headers = HeaderMap::new();
        headers.insert(header::RANGE, HeaderValue::from_static("Bytes=2-4"));
        let range = selected_range(&headers, &Method::GET, etag, 10)
            .unwrap()
            .unwrap();
        assert_eq!(
            (range.first(), range.last(), range.byte_length()),
            (2, 4, 3)
        );
        assert_eq!(selected_range(&headers, &Method::HEAD, etag, 10), Ok(None));
        headers.insert(header::IF_RANGE, HeaderValue::from_static("\"different\""));
        assert_eq!(selected_range(&headers, &Method::GET, etag, 10), Ok(None));
        headers.insert(
            header::IF_RANGE,
            HeaderValue::from_static("W/\"content-sha\""),
        );
        assert_eq!(selected_range(&headers, &Method::GET, etag, 10), Ok(None));
        headers.insert(
            header::IF_RANGE,
            HeaderValue::from_static("\"content-sha\""),
        );
        assert_eq!(
            selected_range(&headers, &Method::GET, etag, 10),
            Ok(Some(range))
        );
        headers.remove(header::IF_RANGE);
        headers.insert(header::RANGE, HeaderValue::from_static("items=0-2"));
        assert_eq!(selected_range(&headers, &Method::GET, etag, 10), Ok(None));
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=10-"));
        assert_eq!(
            selected_range(&headers, &Method::GET, etag, 10),
            Err(RangeError::Unsatisfiable)
        );
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=0-2,5-7"));
        assert_eq!(
            selected_range(&headers, &Method::GET, etag, 10),
            Err(RangeError::MultipleRanges)
        );
        headers.insert(header::RANGE, HeaderValue::from_static("bytes=0-2"));
        headers.append(header::RANGE, HeaderValue::from_static("bytes=5-7"));
        assert_eq!(
            selected_range(&headers, &Method::GET, etag, 10),
            Err(RangeError::Invalid)
        );
    }
    #[tokio::test]
    async fn stream_lengths_and_cancellation_never_release_a_live_stream_lane_early() {
        let lanes = Arc::new(Semaphore::new(1));
        let chunks: StorageStream =
            Box::pin(futures_util::stream::iter([Ok(Bytes::from_static(b"abc"))]));
        let mut stream = bounded_stream(chunks, 3, lanes.clone().try_acquire_owned().unwrap());
        assert_eq!(lanes.available_permits(), 0);
        assert_eq!(
            stream.next().await.unwrap().unwrap(),
            Bytes::from_static(b"abc")
        );
        assert_eq!(lanes.available_permits(), 0);
        assert!(stream.next().await.is_none());
        assert_eq!(lanes.available_permits(), 1);
        for (chunk, expected) in [(b"ab".as_slice(), 3), (b"abcd".as_slice(), 3)] {
            let source: StorageStream = Box::pin(futures_util::stream::iter([Ok(
                Bytes::copy_from_slice(chunk),
            )]));
            let mut stream =
                bounded_stream(source, expected, lanes.clone().try_acquire_owned().unwrap());
            let first = stream.next().await.unwrap();
            let error = if first.is_ok() {
                stream.next().await.unwrap()
            } else {
                first
            };
            assert_eq!(error, Err(StorageError::LengthMismatch));
            drop(stream);
            assert_eq!(lanes.available_permits(), 1);
        }
        let source: StorageStream = Box::pin(futures_util::stream::pending());
        let stream = bounded_stream(source, 3, lanes.clone().try_acquire_owned().unwrap());
        assert_eq!(lanes.available_permits(), 0);
        drop(stream);
        assert_eq!(lanes.available_permits(), 1);
    }
}
