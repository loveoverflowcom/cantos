//! Binary private audio delivery is independent of JSON/MessagePack metadata negotiation.
use super::StudioPath;
use crate::contract::studio::ErrorCode;
use crate::{
    adapters::inbound::http::{
        audio_transport, errors::error_response, errors::failure, studio::admission::cookie_token,
        AppState,
    },
    application::StoreError,
    contexts::media::read::AudioReadScope,
};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

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
    audio_transport::private_response(read_private(state, scope, method, headers).await)
}
async fn read_private(
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
    let plan = match audio_transport::plan(
        &headers,
        &method,
        authorized.sha256(),
        authorized.byte_length(),
    ) {
        Ok(plan) => plan,
        Err(error) => return transport_error_response(error, &method),
    };
    if !plan.requires_storage() {
        let rejection = failure(ErrorCode::InvalidRequest, plan.status());
        return match audio_transport::metadata_response(plan, &method, permit, rejection) {
            Ok(response) => response,
            Err(error) => transport_error_response(error, &method),
        };
    }
    // The owning application opens only this authorized immutable object and then rechecks
    // the same owner/resource binding. Shared HTTP mechanics supply no read authority.
    let stored = match state
        .store
        .open_audio_read(&token, &authorized, plan.range())
        .await
    {
        Ok(value) => value,
        Err(error) => return error_response(error),
    };
    match audio_transport::response(stored, plan, &method, permit) {
        Ok(response) => response,
        Err(error) => transport_error_response(error, &method),
    }
}

fn transport_error_response(
    error: audio_transport::AudioTransportError,
    method: &Method,
) -> Response {
    use audio_transport::AudioTransportError;
    let error = match error {
        AudioTransportError::MalformedRange
        | AudioTransportError::MalformedIfRange
        | AudioTransportError::MalformedIfMatch
        | AudioTransportError::MalformedIfNoneMatch
        | AudioTransportError::UnsupportedMethod => StoreError::InvalidRequest,
        AudioTransportError::InvalidRepresentation
        | AudioTransportError::InconsistentPlan
        | AudioTransportError::DeclaredLengthMismatch => StoreError::CorruptRevision,
    };
    let mut response = error_response(error);
    if *method == Method::HEAD {
        *response.body_mut() = Body::empty();
    }
    audio_transport::private_response(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_transport::AudioTransportError;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn studio_projects_every_transport_error_to_its_own_safe_wire_contract() {
        for (error, status, code) in [
            (
                AudioTransportError::MalformedRange,
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                AudioTransportError::MalformedIfRange,
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                AudioTransportError::MalformedIfMatch,
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                AudioTransportError::MalformedIfNoneMatch,
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                AudioTransportError::UnsupportedMethod,
                StatusCode::BAD_REQUEST,
                "invalid_request",
            ),
            (
                AudioTransportError::InvalidRepresentation,
                StatusCode::INTERNAL_SERVER_ERROR,
                "corrupt_revision",
            ),
            (
                AudioTransportError::InconsistentPlan,
                StatusCode::INTERNAL_SERVER_ERROR,
                "corrupt_revision",
            ),
            (
                AudioTransportError::DeclaredLengthMismatch,
                StatusCode::INTERNAL_SERVER_ERROR,
                "corrupt_revision",
            ),
        ] {
            let response = transport_error_response(error, &Method::GET);
            assert_eq!(response.status(), status);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "private, no-store, no-transform"
            );
            assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                value,
                serde_json::json!({"code": code, "current_revision": null, "issues": []})
            );
        }
    }

    #[tokio::test]
    async fn studio_head_transport_errors_have_no_body() {
        let response =
            transport_error_response(AudioTransportError::MalformedIfMatch, &Method::HEAD);
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(to_bytes(response.into_body(), 1024)
            .await
            .unwrap()
            .is_empty());
    }
}
