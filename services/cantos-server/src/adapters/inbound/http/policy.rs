//! Cross-surface Origin admission, deadline and response cache/security policy.
use super::{
    audio_transport, errors::failure, identity_auth, studio, theatre, theatre_library, AppState,
};
use crate::contract::studio::ErrorCode;
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};

#[derive(Clone, Copy)]
pub(super) enum Surface {
    Studio,
    Theatre,
    Admin,
}

pub(super) async fn origin_and_cache(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let read =
        request.method() == axum::http::Method::GET || request.method() == axum::http::Method::HEAD;
    let valid_origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
        == Some(state.web_origin.as_str());
    let original_path = request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map_or(request.uri().path(), |uri| uri.0.path());
    let theatre_request = original_path.starts_with("/api/v1/theatre/");
    let cover_request = read && theatre::cover_path(original_path);
    let anonymous_playback = theatre::public_playback_post(request.method(), original_path);
    let shared_request = original_path.starts_with("/api/v1/theatre/shared/");
    let bearer_mutation = theatre_library::bearer_mutation(request.method(), original_path)
        || super::theatre_shared::bearer_mutation(request.method(), original_path);
    let native_mutation = identity_auth::native_mutation(request.method(), original_path);
    let delegated_mutation = studio::delegation::bearer_mutation(request.method(), original_path);
    let mut response = if !read
        && !valid_origin
        && !anonymous_playback
        && !bearer_mutation
        && !native_mutation
        && !delegated_mutation
    {
        failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN)
    } else {
        match tokio::time::timeout(std::time::Duration::from_secs(15), next.run(request)).await {
            Ok(response) => response,
            Err(_) if theatre_request => {
                theatre::error(crate::contexts::media::ListenerError::Unavailable)
            }
            Err(_) => failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
        }
    };
    // Preserve the transport-owned policy, not a MIME heuristic: 304 has no Content-Type,
    // while conditional rejections and other audio errors carry the surface's JSON body.
    // Unrecognized/missing policies still cannot make a response cacheable.
    let cache_control = if shared_request
        || response.headers().get(header::CACHE_CONTROL)
            == Some(&audio_transport::PRIVATE_CACHE_CONTROL)
    {
        audio_transport::PRIVATE_CACHE_CONTROL
    } else if cover_request
        && matches!(response.status(), StatusCode::OK | StatusCode::NOT_MODIFIED)
        && response.headers().get(header::CACHE_CONTROL) == Some(&theatre::COVER_CACHE_CONTROL)
    {
        // Only the exact cover route's shown bytes or revalidation are publicly revalidatable.
        theatre::COVER_CACHE_CONTROL
    } else {
        HeaderValue::from_static("no-store")
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, cache_control);
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}
