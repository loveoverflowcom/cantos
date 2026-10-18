//! Cookie-free Cantos credential transport. Identity routes deliberately remain JSON-only.
use super::{failure, AppState};
use crate::platform::{
    bearer::{
        core::{self, Client},
        BearerIdentity,
    },
    studio_oidc::AuthError,
};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{bearer as dto, ErrorCode};
use serde::Deserialize;

type Body<T> = Result<Json<T>, axum::extract::rejection::JsonRejection>;
pub(super) fn routes(client: Client) -> Router<AppState> {
    let prefix = if client == Client::TheatreNative {
        "/native/auth/bearer"
    } else {
        "/auth/bearer"
    };
    let router = Router::new()
        .route(&format!("{prefix}/flows"), post(begin))
        .route(&format!("{prefix}/flows/{{flow}}/exchange"), post(exchange))
        .route(&format!("{prefix}/flows/{{flow}}/cancel"), post(cancel))
        .route(&format!("{prefix}/refresh"), post(refresh))
        .route(&format!("{prefix}/revoke"), post(revoke))
        .route(&format!("{prefix}/session"), get(session).delete(logout));
    let router = if client == Client::TheatreNative {
        router
    } else {
        router.route(&format!("{prefix}/callback"), get(callback))
    };
    router
        .layer(axum::Extension(client))
        .layer(DefaultBodyLimit::max(16384))
}
pub(super) fn access(headers: &HeaderMap) -> Result<&str, AuthError> {
    if headers.contains_key(header::COOKIE) {
        return Err(AuthError::InvalidToken);
    }
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let token = values
        .next()
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AuthError::InvalidToken)?;
    if values.next().is_some() || core::access_client(token).is_none() {
        return Err(AuthError::InvalidToken);
    }
    Ok(token)
}
fn anonymous(headers: &HeaderMap) -> Result<(), AuthError> {
    if headers.contains_key(header::COOKIE) || headers.contains_key(header::AUTHORIZATION) {
        return Err(AuthError::InvalidToken);
    }
    Ok(())
}
fn origin_allowed(headers: &HeaderMap, origin: &str) -> bool {
    !headers.contains_key(header::ORIGIN)
        || (headers.get_all(header::ORIGIN).iter().count() == 1
            && headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) == Some(origin))
}
fn identity(state: &AppState, client: Client) -> Result<&BearerIdentity, AuthError> {
    state.store.bearer(client).ok_or(AuthError::Configuration)
}
async fn begin(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    headers: HeaderMap,
    body: Body<dto::FlowRequest>,
) -> Response {
    if !origin_allowed(&headers, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    if let Err(e) = anonymous(&headers) {
        return error(e);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match service.begin(&state.store, body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Callback {
    state: String,
    code: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
    iss: Option<String>,
}
async fn callback(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    query: Result<Query<Callback>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return error(AuthError::State);
    };
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    if !service.accepts_callback_issuer(query.iss.as_deref()) {
        return error(AuthError::Issuer);
    }
    let result = match (query.code, query.error, query.error_description) {
        (Some(code), None, None) => service.callback(&state.store, &query.state, &code).await,
        (None, Some(reason), description)
            if !reason.is_empty()
                && reason.len() <= 128
                && reason.bytes().all(|b| b.is_ascii_graphic())
                && description.as_ref().is_none_or(|d| d.len() <= 1024) =>
        {
            service
                .authorization_failed(&state.store, &query.state)
                .await
        }
        _ => Err(AuthError::State),
    };
    match result {
        Ok(url) => match HeaderValue::from_str(&url) {
            Ok(url) => {
                let mut response = StatusCode::SEE_OTHER.into_response();
                response.headers_mut().insert(header::LOCATION, url);
                response
            }
            Err(_) => error(AuthError::Configuration),
        },
        Err(e) => error(e),
    }
}
async fn exchange(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    Path(flow): Path<String>,
    headers: HeaderMap,
    body: Body<dto::ExchangeRequest>,
) -> Response {
    if !origin_allowed(&headers, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    if let Err(e) = anonymous(&headers) {
        return error(e);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match service.exchange(&state.store, &flow, body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn cancel(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    Path(flow): Path<String>,
    headers: HeaderMap,
    body: Body<dto::CancelRequest>,
) -> Response {
    if !origin_allowed(&headers, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    if let Err(e) = anonymous(&headers) {
        return error(e);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match service.cancel(&state.store, &flow, body).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error(e),
    }
}
async fn refresh(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    headers: HeaderMap,
    body: Body<dto::RefreshRequest>,
) -> Response {
    if !origin_allowed(&headers, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    if let Err(e) = anonymous(&headers) {
        return error(e);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match service.refresh(&state.store, body).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
async fn revoke(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    headers: HeaderMap,
    body: Body<dto::RefreshRequest>,
) -> Response {
    if !origin_allowed(&headers, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    if let Err(e) = anonymous(&headers) {
        return error(e);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match service.revoke_refresh(&state.store, body).await {
        Ok(done) => revoked(done),
        Err(e) => error(e),
    }
}
async fn session(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    headers: HeaderMap,
) -> Response {
    if !origin_allowed(&headers, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let token = match access(&headers) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    if core::access_client(token) != Some(client) {
        return error(AuthError::InvalidToken);
    }
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    if let Err(e) = service.authenticate(&state.store, token).await {
        return error(e);
    }
    match service
        .with_admission(&state.store, token, |session| async {
            Json(session).into_response()
        })
        .await
    {
        Ok(v) => v,
        Err(e) => error(e),
    }
}
async fn logout(
    State(state): State<AppState>,
    axum::Extension(client): axum::Extension<Client>,
    headers: HeaderMap,
) -> Response {
    if !origin_allowed(&headers, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let token = match access(&headers) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    if core::access_client(token) != Some(client) {
        return error(AuthError::InvalidToken);
    }
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match service.logout(&state.store, token).await {
        Ok(done) => revoked(done),
        Err(e) => error(e),
    }
}
fn revoked(done: bool) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        "cantos-provider-revocation",
        HeaderValue::from_static(if done { "completed" } else { "pending" }),
    );
    response
}
pub(super) fn error(error: AuthError) -> Response {
    super::theatre_native_auth::error(error)
}
/// Only cookie-free bearer mutations and these explicit protocol endpoints bypass Origin CSRF.
/// Browser Origin, when supplied, must still match the configured surface exactly.
pub(super) fn mutation(method: &Method, path: &str, headers: &HeaderMap, origin: &str) -> bool {
    if headers.contains_key(header::COOKIE) {
        return false;
    }
    if headers.contains_key(header::ORIGIN)
        && (headers.get_all(header::ORIGIN).iter().count() != 1
            || headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(origin))
    {
        return false;
    }
    if path.contains("/auth/bearer/") {
        return (*method == Method::POST || *method == Method::DELETE)
            && !headers.contains_key(header::AUTHORIZATION)
            || (*method == Method::DELETE && access(headers).is_ok());
    }
    path.starts_with("/api/v1/studio/") && access(headers).is_ok()
}
/// Sensitive owner effects require signed recent issuer auth_time, not refresh/iat/SSO time.
pub(super) fn sensitive(method: &Method, path: &str) -> bool {
    !matches!(*method, Method::GET | Method::HEAD)
        && (path.contains("/delegations")
            || path.contains("/producer-grants")
            || path.ends_with("/publication-approvals")
            || path.ends_with("/publish")
            || path.ends_with("/publication/withdraw"))
}
pub(super) fn recent(session: &dto::Session) -> bool {
    crate::platform::bearer::now().is_ok_and(|now| {
        session
            .authenticated_at
            .is_some_and(|time| time <= now && now - time < 300)
    })
}
pub(super) async fn studio_session(state: AppState, headers: HeaderMap, end: bool) -> Response {
    if end {
        logout(State(state), axum::Extension(Client::Studio), headers).await
    } else {
        session(State(state), axum::Extension(Client::Studio), headers).await
    }
}

pub(super) async fn theatre(
    state: AppState,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if !origin_allowed(request.headers(), &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let token = match access(request.headers()) {
        Ok(v) => v.to_owned(),
        Err(e) => return error(e),
    };
    let Some(client) = core::access_client(&token).filter(|c| c.surface() == "theatre") else {
        return error(AuthError::InvalidToken);
    };
    let service = match identity(&state, client) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    if request.uri().path() == "/session" && request.method() == Method::DELETE {
        return match service.logout(&state.store, &token).await {
            Ok(done) => revoked(done),
            Err(e) => error(e),
        };
    }
    use crate::contexts::listening::auth::{
        AuthenticationError, BearerListenerAuthenticator, ListenerAuthenticator,
        ListenerCredentials,
    };
    let credentials = match ListenerCredentials::bearer(&token) {
        Ok(v) => v,
        Err(_) => return error(AuthError::InvalidToken),
    };
    let principal = match BearerListenerAuthenticator::new(state.store.clone())
        .authenticate(credentials)
        .await
    {
        Ok(v) => v,
        Err(AuthenticationError::Unauthenticated) => return error(AuthError::Inactive),
        Err(AuthenticationError::Unavailable) => return error(AuthError::Unavailable),
    };
    match service
        .with_admission(&state.store, &token, |view| async move {
            if let Some(response) =
                super::theatre_auth::session_fence_refusal(request.headers(), &view.session_id)
            {
                return response;
            }
            if request.uri().path() == "/session" {
                return super::theatre_auth::stamp_session(
                    Json(view.clone()).into_response(),
                    &view.session_id,
                );
            }
            request.extensions_mut().insert(principal);
            super::theatre_auth::stamp_session(next.run(request).await, &view.session_id)
        })
        .await
    {
        Ok(v) => v,
        Err(e) => error(e),
    }
}

pub(super) fn browser_origin_allowed(headers: &HeaderMap, origin: &str) -> bool {
    origin_allowed(headers, origin)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_fences_survive_the_bearer_migration() {
        for (uri, expected) in [
            ("/scripts/id/head", true),
            ("/scripts/id/head?studio_session_id=a", true),
            ("/scripts/id/head?studio_session_id=b", false),
            (
                "/scripts/id/head?studio_session_id=a&studio_session_id=a",
                false,
            ),
        ] {
            assert_eq!(
                super::super::studio_query_fence_matches(&uri.parse().unwrap(), Some("a")),
                expected
            );
        }
    }
    #[test]
    fn one_credential_and_no_cross_site_origin_bypass() {
        let token = format!("Bearer ca1_s_{}", "a".repeat(64));
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, token.parse().unwrap());
        assert!(access(&headers).is_ok());
        assert!(mutation(
            &Method::PUT,
            "/api/v1/studio/works/id",
            &headers,
            "https://studio.web.app"
        ));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example"),
        );
        assert!(!mutation(
            &Method::PUT,
            "/api/v1/studio/works/id",
            &headers,
            "https://studio.web.app"
        ));
        headers.remove(header::ORIGIN);
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("cantos_session=old"),
        );
        assert!(matches!(access(&headers), Err(AuthError::InvalidToken)));
        headers.remove(header::COOKIE);
        headers.append(header::AUTHORIZATION, token.parse().unwrap());
        assert!(matches!(access(&headers), Err(AuthError::InvalidToken)));
    }
    #[test]
    fn refresh_time_is_never_recent_authentication() {
        let session = dto::Session {
            actor_id: "alice".into(),
            session_id: "session".into(),
            expires_at: u64::MAX,
            absolute_expires_at: u64::MAX,
            authenticated_at: None,
        };
        assert!(!recent(&session));
        assert!(sensitive(&Method::POST, "/works/id/delegations"));
        assert!(!sensitive(&Method::GET, "/works/id/delegations"));
    }
}
