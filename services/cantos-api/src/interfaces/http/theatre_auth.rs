//! Theatre-only OIDC HTTP boundary.
use crate::{
    interfaces::http::{failure, AppState},
    platform::theatre_identity::AuthError,
};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use cantos_contracts::ErrorCode;
use serde::Deserialize;

pub(super) fn routes(enabled: bool) -> Router<AppState> {
    if !enabled {
        return Router::new();
    }
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/session", get(session).delete(logout_handler))
}

use crate::interfaces::http::cookies::unique_cookie;

async fn login(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(oidc) = state.store.theatre_identity() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if oidc.web_origin() != state.web_origin {
        return error(AuthError::Configuration);
    }
    if headers
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site")
        || headers
            .get(header::ORIGIN)
            .is_some_and(|v| v.as_bytes() != oidc.web_origin().as_bytes())
    {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let previous = match session_cookie(&headers, oidc.session_name()) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match oidc.begin(&state.store, previous.as_deref()).await {
        Ok(start) => redirect(&start.url, &[start.cookie]),
        Err(e) => error(e),
    }
}

#[derive(Deserialize)]
struct Callback {
    state: String,
    code: String,
}

async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<Callback>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Some(oidc) = state.store.theatre_identity() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    let Ok(Query(query)) = query else {
        return error(AuthError::State);
    };
    let browser = match flow_cookie(&headers, oidc.flow_name()) {
        Ok(Some(v)) => v,
        _ => return error(AuthError::State),
    };
    match oidc
        .callback(&state.store, &query.state, &browser, &query.code)
        .await
    {
        Ok(complete) => redirect("/", &[complete.cookie, complete.clear_flow]),
        Err(e) => error(e),
    }
}

async fn logout_handler(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(oidc) = state.store.theatre_identity() else {
        return error(AuthError::Configuration);
    };
    let token = match session_cookie(&headers, oidc.session_name()) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let browser = match flow_cookie(&headers, oidc.flow_name()) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    match oidc
        .logout(&state.store, token.as_deref(), browser.as_deref())
        .await
    {
        Ok(complete) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            response.headers_mut().insert(
                "cantos-provider-revocation",
                HeaderValue::from_static(if complete.provider_revoked {
                    "completed"
                } else {
                    "pending"
                }),
            );
            cookies(response, &complete.cookies)
        }
        Err(e) => error(e),
    }
}

fn redirect(location: &str, values: &[String]) -> Response {
    let Ok(location) = HeaderValue::from_str(location) else {
        return error(AuthError::Configuration);
    };
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(header::LOCATION, location);
    cookies(response, values)
}
fn cookies(mut response: Response, values: &[String]) -> Response {
    for cookie in values {
        let Ok(cookie) = HeaderValue::from_str(cookie) else {
            return error(AuthError::Configuration);
        };
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}
fn error(error: AuthError) -> Response {
    match error {
        AuthError::Unavailable | AuthError::ProviderUnavailable => {
            failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE)
        }
        AuthError::Configuration | AuthError::InvalidMetadata => {
            failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE)
        }
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
        | AuthError::Inactive => failure(ErrorCode::Unauthenticated, StatusCode::UNAUTHORIZED),
    }
}

use crate::contexts::listening::auth::TheatreListenerAuthenticator;
use crate::contexts::listening::{
    auth::{AuthenticatedListener, ListenerAuthenticator, ListenerCredentials},
    LibraryError,
};
use axum::{
    extract::{Extension, Request},
    middleware::Next,
};

fn session_cookie(headers: &HeaderMap, expected: &str) -> Result<Option<String>, AuthError> {
    let mut found = None;
    for name in ["cantos_theatre", "__Secure-cantos_theatre"] {
        if let Some(value) = unique_cookie(headers, name)? {
            if name != expected || found.is_some() {
                return Err(AuthError::State);
            }
            found = Some(value);
        }
    }
    Ok(found)
}

/// Admitted account requests hold a current session/actor share lock until the response.
/// Logout/disable linearize after already admitted requests; no network runs under this lock.
pub(super) async fn bff_identity(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let account = path.starts_with("/library/")
        || path.starts_with("/progress/")
        || path.starts_with("/operations/")
        || path == "/session";
    if !account
        || (path == "/session" && request.method() == axum::http::Method::DELETE)
        || state.store.theatre_identity().is_none()
    {
        return next.run(request).await;
    }
    let Some(oidc) = state.store.theatre_identity() else {
        return error(AuthError::Configuration);
    };
    if oidc.web_origin() != state.web_origin {
        return error(AuthError::Configuration);
    }
    // Web BFF does not admit direct bearer/native tokens; native resource verification is deferred.
    if request.headers().contains_key(header::AUTHORIZATION) {
        return super::theatre_library::error(LibraryError::Unauthenticated);
    }
    let read = matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    if !read
        && request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            != Some(oidc.web_origin())
    {
        return super::theatre_library::error(LibraryError::Unauthenticated);
    }
    let token = match session_cookie(request.headers(), oidc.session_name()) {
        Ok(Some(token)) => token,
        _ => return super::theatre_library::error(LibraryError::Unauthenticated),
    };
    let credential = match ListenerCredentials::bearer(&token) {
        Ok(v) => v,
        Err(_) => return super::theatre_library::error(LibraryError::Unauthenticated),
    };
    let principal = match TheatreListenerAuthenticator::new(state.store.clone())
        .authenticate(credential)
        .await
    {
        Ok(v) => v,
        Err(crate::contexts::listening::auth::AuthenticationError::Unauthenticated) => {
            return super::theatre_library::error(LibraryError::Unauthenticated)
        }
        Err(crate::contexts::listening::auth::AuthenticationError::Unavailable) => {
            return super::theatre_library::error(LibraryError::Unavailable)
        }
    };
    let actor = principal.listener().as_str().to_owned();
    match oidc
        .with_session_admission(&state.store, &token, &actor, move |session_id| async move {
            request
                .extensions_mut()
                .insert(cantos_contracts::theatre_identity::Session {
                    actor_id: principal.listener().as_str().into(),
                    session_id,
                });
            request.extensions_mut().insert(principal);
            next.run(request).await
        })
        .await
    {
        Ok(response) => response,
        Err(AuthError::Inactive) => super::theatre_library::error(LibraryError::Unauthenticated),
        Err(error_value) => error(error_value),
    }
}

async fn session(
    trusted: Option<Extension<AuthenticatedListener>>,
    view: Option<Extension<cantos_contracts::theatre_identity::Session>>,
) -> Response {
    match (trusted, view) {
        (Some(_), Some(Extension(view))) => axum::Json(view).into_response(),
        _ => super::theatre_library::error(LibraryError::Unauthenticated),
    }
}

// Session parser tests execute the production parser, including multiple Cookie headers.

fn flow_cookie(headers: &HeaderMap, expected: &str) -> Result<Option<String>, AuthError> {
    let mut found = None;
    for name in ["cantos_theatre_flow", "__Secure-cantos_theatre_flow"] {
        if let Some(value) = unique_cookie(headers, name)? {
            if name != expected || found.is_some() {
                return Err(AuthError::State);
            }
            found = Some(value);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::http::cookie_token_for_name;
    #[test]
    fn duplicate_cookie_headers_and_mixed_session_names_are_rejected() {
        let mut headers = HeaderMap::new();
        headers.append(header::COOKIE, HeaderValue::from_static("cantos_session=a"));
        headers.append(header::COOKIE, HeaderValue::from_static("cantos_session=b"));
        assert!(matches!(
            cookie_token_for_name(&headers, "cantos_session"),
            Err(crate::platform::postgres::StoreError::Unauthenticated)
        ));
        assert_eq!(
            unique_cookie(&headers, "cantos_session"),
            Err(AuthError::State)
        );
        headers.clear();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("cantos_session=attacker"),
        );
        assert!(matches!(
            cookie_token_for_name(&headers, "__Secure-cantos_studio"),
            Err(crate::platform::postgres::StoreError::Unauthenticated)
        ));
        headers.clear();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("cantos_session=a; __Secure-cantos_studio=b"),
        );
        assert!(matches!(
            cookie_token_for_name(&headers, "cantos_session"),
            Err(crate::platform::postgres::StoreError::Unauthenticated)
        ));
    }
}
