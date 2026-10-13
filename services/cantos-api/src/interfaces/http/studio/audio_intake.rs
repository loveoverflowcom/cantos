//! Current Studio owner-session intake only. External producers never receive or reuse cookies.
use super::StudioPath as AudioPath;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use cantos_contracts::{CreateAudioIntentRequest, ErrorCode};
use tokio::sync::Semaphore;

use crate::interfaces::http::{cookie_token, error_response, failure, AppState};
use crate::platform::postgres::{Store, StoreError};

static UPLOAD_REQUESTS: Semaphore = Semaphore::const_new(4);

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/scripts/{script}/audio-intents",
            post(prepare).layer(DefaultBodyLimit::max(16 * 1024)),
        )
        .route("/scripts/{script}/audio-intents/{intent}", get(status))
        .route(
            "/scripts/{script}/audio-intents/{intent}/bytes",
            put(upload).layer(middleware::from_fn(limit_upload)),
        )
        .method_not_allowed_fallback(|| async {
            failure(ErrorCode::InvalidRequest, StatusCode::METHOD_NOT_ALLOWED)
        })
        .layer(DefaultBodyLimit::max(Store::audio_upload_body_limit()))
}

// This runs before the Bytes extractor, so authenticated floods cannot buffer arbitrary
// numbers of 32MiB bodies while waiting for the two inspection/storage lanes.
pub(super) async fn limit_upload(request: Request, next: Next) -> Response {
    let Ok(_permit) = UPLOAD_REQUESTS.try_acquire() else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    next.run(request).await
}

async fn prepare(
    State(state): State<AppState>,
    AudioPath(script): AudioPath<String>,
    headers: HeaderMap,
    body: Result<Json<CreateAudioIntentRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return failure(ErrorCode::InvalidRequest, StatusCode::PAYLOAD_TOO_LARGE)
        }
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let token = match cookie_token(&headers) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .create_audio_intent(&token, &script, request)
        .await
    {
        Ok(intent) => Json(intent).into_response(),
        Err(error) => error_response(error),
    }
}

async fn status(
    State(state): State<AppState>,
    AudioPath((script, intent)): AudioPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .load_audio_intent(&token, &script, &intent)
        .await
    {
        Ok(status) => Json(status).into_response(),
        Err(error) => error_response(error),
    }
}

async fn upload(
    State(state): State<AppState>,
    AudioPath((script, intent)): AudioPath<(String, String)>,
    headers: HeaderMap,
    body: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        != Some("application/octet-stream")
        || headers.contains_key(header::CONTENT_ENCODING)
    {
        return failure(
            ErrorCode::InvalidRequest,
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        );
    }
    let bytes = match body {
        Ok(bytes) => bytes,
        Err(error) if error.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            return failure(ErrorCode::InvalidRequest, StatusCode::PAYLOAD_TOO_LARGE)
        }
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let token = match cookie_token(&headers) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .upload_audio_intent(&token, &script, &intent, bytes.to_vec())
        .await
    {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => error_response(error),
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request, middleware, routing::put, Router};
    use tower::ServiceExt;

    use super::{limit_upload, Bytes, StatusCode, UPLOAD_REQUESTS};

    #[tokio::test]
    async fn occupied_request_lanes_reject_before_a_binary_handler_runs() {
        let _permits: Vec<_> = (0..4)
            .map(|_| UPLOAD_REQUESTS.try_acquire().unwrap())
            .collect();
        let app = Router::new().route(
            "/bytes",
            put(|_: Bytes| async {
                panic!("a full upload gate must not invoke the body extractor or handler");
                #[allow(unreachable_code)]
                StatusCode::OK
            })
            .layer(middleware::from_fn(limit_upload)),
        );
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/bytes")
                    .body(Body::from("untrusted bytes"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
