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
    match flow_inventory(&headers, oidc.flow_name()) {
        Ok(inventory) if inventory.len() < MAX_FLOW_INVENTORY => {}
        _ => return error(AuthError::State),
    }
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
    let browser = match flow_inventory(&headers, oidc.flow_name()) {
        Ok(inventory) => match inventory
            .into_iter()
            .find(|(state, _)| state == &query.state)
        {
            Some((_, browser)) => browser,
            None => return error(AuthError::State),
        },
        _ => return error(AuthError::State),
    };
    match oidc
        .callback(&state.store, &query.state, &browser, &query.code)
        .await
    {
        // Consumed/completed SQL flow remains one-time. A global expiry header here could
        // erase a newer flow cookie created by another tab while exchange was in flight.
        Ok(complete) => redirect("/", &[complete.cookie]),
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
    let inventory = match flow_inventory(&headers, oidc.flow_name()) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let mut expected = headers.get_all("x-cantos-theatre-session").iter();
    let expected_value = match expected.next() {
        Some(v) => match v.to_str() {
            Ok(value) => Some(value),
            Err(_) => return error(AuthError::State),
        },
        None => None,
    };
    if expected.next().is_some()
        || expected_value.is_some_and(|v| uuid::Uuid::parse_str(v).is_err())
    {
        return error(AuthError::State);
    }
    let mut stale = headers.get_all("x-cantos-theatre-stale-only").iter();
    let stale_value = stale.next();
    if stale.next().is_some() || stale_value.is_some_and(|v| v.as_bytes() != b"1") {
        return error(AuthError::State);
    }
    match oidc
        .guarded_logout(
            &state.store,
            token.as_deref(),
            &inventory,
            expected_value,
            stale_value.is_some(),
        )
        .await
    {
        Ok(crate::platform::theatre_identity::GuardedLogout::SessionChanged(session)) => {
            stamp_session(
                failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                &session,
            )
        }
        Ok(crate::platform::theatre_identity::GuardedLogout::Complete(complete)) => {
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

use crate::contexts::listening::auth::{NativeListenerAuthenticator, TheatreListenerAuthenticator};
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
        || (state.store.theatre_identity().is_none() && state.store.theatre_native().is_none())
    {
        return next.run(request).await;
    }
    if request.headers().contains_key(header::AUTHORIZATION) {
        // Native bearer and Web cookie purposes have separate admission paths. Web /session
        // never accepts native credentials; mixed cookie/bearer requests are refused.
        if path == "/session" || request.headers().contains_key(header::COOKIE) {
            return super::theatre_library::error(LibraryError::Unauthenticated);
        }
        let Some(native) = state.store.theatre_native() else {
            return super::theatre_library::error(LibraryError::Unauthenticated);
        };
        let credentials = match super::theatre_native_auth::bearer(request.headers()) {
            Ok(v) => v,
            Err(e) => return error(e),
        };
        let token = credentials.expose_to_verifier().to_owned();
        let principal = match NativeListenerAuthenticator::new(state.store.clone())
            .authenticate(credentials)
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
        return match native
            .with_session_admission(&state.store, &token, &actor, move |view| async move {
                if let Some(response) = session_fence_refusal(request.headers(), &view.session_id) {
                    return response;
                }
                request.extensions_mut().insert(principal);
                stamp_session(next.run(request).await, &view.session_id)
            })
            .await
        {
            Ok(response) => response,
            Err(e) => error(e),
        };
    }
    let Some(oidc) = state.store.theatre_identity() else {
        // A missing Web purpose is not an unavailable provider. Native-only deployments
        // still deny absent credentials and Web cookies as unauthenticated.
        return super::theatre_library::error(LibraryError::Unauthenticated);
    };
    let token = match session_cookie(request.headers(), oidc.session_name()) {
        Ok(Some(token)) => token,
        _ => return super::theatre_library::error(LibraryError::Unauthenticated),
    };
    let credential = match ListenerCredentials::bearer(&token) {
        Ok(v) => v,
        Err(_) => return super::theatre_library::error(LibraryError::Unauthenticated),
    };
    if oidc.web_origin() != state.web_origin {
        return error(AuthError::Configuration);
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
            if let Some(response) = session_fence_refusal(request.headers(), &session_id) {
                return response;
            }
            request
                .extensions_mut()
                .insert(cantos_contracts::theatre_identity::Session {
                    actor_id: principal.listener().as_str().into(),
                    session_id: session_id.clone(),
                });
            request.extensions_mut().insert(principal);
            stamp_session(next.run(request).await, &session_id)
        })
        .await
    {
        Ok(response) => response,
        Err(AuthError::Inactive) => super::theatre_library::error(LibraryError::Unauthenticated),
        Err(error_value) => error(error_value),
    }
}

/// The client declares which nonsecret session owns its intent. The header grants no authority;
/// it rejects a cookie/bearer account switch after dispatch, before any resource mutation.
fn session_fence_refusal(headers: &HeaderMap, actual: &str) -> Option<Response> {
    let mut values = headers.get_all("x-cantos-theatre-session").iter();
    let value = values.next()?;
    if values.next().is_some() || value.to_str().ok() != Some(actual) {
        Some(stamp_session(
            failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
            actual,
        ))
    } else {
        None
    }
}
fn stamp_session(mut response: Response, session: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(session) {
        response
            .headers_mut()
            .insert("x-cantos-theatre-session", value);
    }
    response
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

const MAX_FLOW_INVENTORY: usize = 8;
fn flow_inventory(headers: &HeaderMap, expected: &str) -> Result<Vec<(String, String)>, AuthError> {
    let mut found = std::collections::BTreeMap::<String, String>::new();
    let mut binders = std::collections::BTreeSet::<String>::new();
    let mut bytes = 0usize;
    for value in headers.get_all(header::COOKIE) {
        bytes += value.as_bytes().len();
        if bytes > 16_384 {
            return Err(AuthError::State);
        }
        for part in value.to_str().map_err(|_| AuthError::State)?.split(';') {
            let part = part.trim();
            let Some((name, value)) = part.split_once('=') else {
                if part.starts_with("cantos_theatre_flow")
                    || part.starts_with("__Secure-cantos_theatre_flow")
                {
                    return Err(AuthError::State);
                }
                continue;
            };
            if name.starts_with("cantos_theatre_flow")
                || name.starts_with("__Secure-cantos_theatre_flow")
            {
                let state = name
                    .strip_prefix(&format!("{expected}_"))
                    .ok_or(AuthError::State)?;
                if !crate::platform::theatre_identity::valid_secret(state)
                    || !crate::platform::theatre_identity::valid_secret(value)
                    || found.insert(state.into(), value.into()).is_some()
                    || !binders.insert(value.into())
                    || found.len() > MAX_FLOW_INVENTORY
                {
                    return Err(AuthError::State);
                }
            }
        }
    }
    Ok(found.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interfaces::http::cookie_token_for_name;
    #[test]
    fn immutable_flow_inventory_is_exact_bounded_and_rejects_mutable_fallback() {
        let mut headers = HeaderMap::new();
        let inventory = (0..8)
            .map(|index| format!("cantos_theatre_flow_{index:064x}={:064x}", index + 20))
            .collect::<Vec<_>>()
            .join("; ");
        headers.insert(header::COOKIE, HeaderValue::from_str(&inventory).unwrap());
        assert_eq!(
            flow_inventory(&headers, "cantos_theatre_flow")
                .unwrap()
                .len(),
            8
        );
        for cookie in [
            format!("{inventory}; cantos_theatre_flow_{:064x}={:064x}", 8, 28),
            format!("{inventory}; cantos_theatre_flow_{:064x}={:064x}", 0, 20),
            format!(
                "cantos_theatre_flow_{:064x}={:064x}; cantos_theatre_flow_{:064x}={:064x}",
                0, 20, 1, 20
            ),
            format!("cantos_theatre_flow={:064x}", 20),
            format!("__Secure-cantos_theatre_flow_{:064x}={:064x}", 0, 20),
            format!("cantos_theatre_flow_bad={:064x}", 20),
        ] {
            headers.insert(header::COOKIE, HeaderValue::from_str(&cookie).unwrap());
            assert_eq!(
                flow_inventory(&headers, "cantos_theatre_flow"),
                Err(AuthError::State)
            );
        }
    }
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
