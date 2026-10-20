//! Native-only public code exchange and session boundary. No Web cookies/client secret.
use super::{errors::failure, AppState};
use crate::{
    contexts::listening::auth::{
        ListenerAuthenticator, ListenerCredentials, NativeListenerAuthenticator,
    },
    platform::theatre_identity::AuthError,
};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{
    theatre_identity::{NativeExchangeRequest, NativeFlowRequest},
    ErrorCode,
};
pub(super) fn routes(enabled: bool) -> Router<AppState> {
    if !enabled {
        return Router::new();
    }
    Router::new()
        .route("/native/auth/flows", post(begin))
        .route("/native/auth/flows/{flow}/exchange", post(exchange))
        .route("/native/session", get(session).delete(logout))
        .layer(DefaultBodyLimit::max(16_384))
}
pub(super) fn native_mutation(method: &Method, path: &str) -> bool {
    (*method == Method::POST
        && (path == "/api/v1/theatre/native/auth/flows"
            || (path.starts_with("/api/v1/theatre/native/auth/flows/")
                && path.ends_with("/exchange"))))
        || (*method == Method::DELETE && path == "/api/v1/theatre/native/session")
}
pub(super) fn bearer(headers: &HeaderMap) -> Result<ListenerCredentials, AuthError> {
    if headers.contains_key(header::COOKIE) {
        return Err(AuthError::InvalidToken);
    }
    let mut headers = headers.get_all(header::AUTHORIZATION).iter();
    let value = headers
        .next()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AuthError::InvalidToken)?;
    if headers.next().is_some() {
        return Err(AuthError::InvalidToken);
    }
    ListenerCredentials::bearer(value).map_err(|_| AuthError::InvalidToken)
}
fn flow_headers(headers: &HeaderMap) -> bool {
    !headers.contains_key(header::COOKIE) && !headers.contains_key(header::AUTHORIZATION)
}
type Body<T> = Result<Json<T>, axum::extract::rejection::JsonRejection>;
async fn begin(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body<NativeFlowRequest>,
) -> Response {
    if !flow_headers(&headers) {
        return error(AuthError::InvalidToken);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let Some(native) = state.store.theatre_native() else {
        return error(AuthError::Configuration);
    };
    match native.begin(&state.store, body).await {
        Ok(flow) => Json(flow).into_response(),
        Err(e) => error(e),
    }
}
async fn exchange(
    State(state): State<AppState>,
    Path(flow): Path<String>,
    headers: HeaderMap,
    body: Body<NativeExchangeRequest>,
) -> Response {
    if !flow_headers(&headers) {
        return error(AuthError::InvalidToken);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let Some(native) = state.store.theatre_native() else {
        return error(AuthError::Configuration);
    };
    match native.exchange(&state.store, &flow, body).await {
        Ok(session) => Json(session).into_response(),
        Err(e) => error(e),
    }
}
async fn session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let token = match bearer(&headers) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let raw = token.expose_to_verifier().to_owned();
    let principal = match NativeListenerAuthenticator::new(state.store.clone())
        .authenticate(token)
        .await
    {
        Ok(v) => v,
        Err(crate::contexts::listening::auth::AuthenticationError::Unauthenticated) => {
            return error(AuthError::Inactive)
        }
        Err(crate::contexts::listening::auth::AuthenticationError::Unavailable) => {
            return error(AuthError::Unavailable)
        }
    };
    let Some(native) = state.store.theatre_native() else {
        return error(AuthError::Configuration);
    };
    match native
        .with_session_admission(
            &state.store,
            &raw,
            principal.listener().as_str(),
            |view| async { Json(view).into_response() },
        )
        .await
    {
        Ok(response) => response,
        Err(e) => error(e),
    }
}
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let token = match bearer(&headers) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let Some(native) = state.store.theatre_native() else {
        return error(AuthError::Configuration);
    };
    match native
        .logout(&state.store, token.expose_to_verifier())
        .await
    {
        Ok(done) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            response.headers_mut().insert(
                "cantos-provider-revocation",
                HeaderValue::from_static(if done { "completed" } else { "pending" }),
            );
            response
        }
        Err(e) => error(e),
    }
}
pub(super) fn error(error: AuthError) -> Response {
    let status = match error {
        AuthError::Configuration
        | AuthError::ProviderUnavailable
        | AuthError::InvalidMetadata
        | AuthError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        AuthError::InvalidToken
        | AuthError::Algorithm
        | AuthError::UnknownKey
        | AuthError::InvalidKey
        | AuthError::Signature
        | AuthError::Issuer
        | AuthError::Audience
        | AuthError::Expired
        | AuthError::NotYetValid
        | AuthError::Nonce
        | AuthError::Subject
        | AuthError::State
        | AuthError::Replay
        | AuthError::Inactive => StatusCode::UNAUTHORIZED,
    };
    failure(
        if status == StatusCode::SERVICE_UNAVAILABLE {
            ErrorCode::Unavailable
        } else {
            ErrorCode::Unauthenticated
        },
        status,
    )
}
