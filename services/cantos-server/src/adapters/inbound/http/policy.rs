//! Cross-surface Origin admission, deadline and response cache/security policy.
use super::{
    audio_transport, bearer, errors::failure, studio, theatre, theatre_library,
    theatre_native_auth, theatre_registration, AppState,
};
use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use cantos_contracts::ErrorCode;

#[derive(Clone, Copy)]
pub(super) enum Surface {
    Studio,
    Theatre,
    Admin,
}

#[derive(Clone)]
pub(super) struct Boundary {
    pub state: AppState,
    pub surface: Surface,
}

pub(super) async fn origin_and_cache(
    State(boundary): State<Boundary>,
    request: Request,
    next: Next,
) -> Response {
    let state = boundary.state;
    let read =
        request.method() == axum::http::Method::GET || request.method() == axum::http::Method::HEAD;
    let valid_config = match boundary.surface {
        Surface::Studio => state
            .store
            .studio_oidc()
            .is_none_or(|oidc| oidc.web_origin() == state.web_origin),
        Surface::Theatre => state
            .store
            .theatre_identity()
            .is_none_or(|oidc| oidc.web_origin() == state.web_origin),
        Surface::Admin => true,
    };
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
    let anonymous_playback = theatre::public_playback_post(request.method(), original_path);
    let bearer_mutation = theatre_library::bearer_mutation(request.method(), original_path);
    let native_mutation = theatre_native_auth::native_mutation(request.method(), original_path)
        || theatre_registration::native_mutation(request.method(), original_path);
    let scoped_bearer = bearer::mutation(
        request.method(),
        original_path,
        request.headers(),
        &state.web_origin,
    );
    let delegated_mutation = studio::delegation::bearer_mutation(request.method(), original_path);
    let mut response = if !valid_config
        || (!read
            && !valid_origin
            && !anonymous_playback
            && !bearer_mutation
            && !native_mutation
            && !delegated_mutation
            && !scoped_bearer)
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
    let cache_control = if response.headers().get(header::CACHE_CONTROL)
        == Some(&audio_transport::PRIVATE_CACHE_CONTROL)
    {
        audio_transport::PRIVATE_CACHE_CONTROL
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
