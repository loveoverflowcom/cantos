//! Only the current Studio mount exposes OIDC. Handlers never inspect provider claims.
use crate::{
    interfaces::http::{
        error_response, failure, studio_fence_matches, studio_fenced_response, AppState,
    },
    platform::studio_oidc::{AuthError, BrowserInventory, StudioOidc, MAX_BROWSER_FLOWS},
};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use cantos_contracts::{ErrorCode, StudioAuthenticationConfiguration, StudioAuthenticationMode};
use serde::Deserialize;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/configuration", get(configuration))
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
}

pub(in crate::interfaces::http) use crate::interfaces::http::cookies::unique_cookie;

/// A production host without Studio OIDC has no Studio credential; it never reports the
/// development token mode it cannot honor.
async fn configuration(State(state): State<AppState>) -> Response {
    let mode = if state.store.studio_oidc().is_some() {
        StudioAuthenticationMode::Oidc
    } else if state.store.development_sessions_enabled() {
        StudioAuthenticationMode::Development
    } else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    Json(StudioAuthenticationConfiguration { mode }).into_response()
}

fn cookie_count(headers: &HeaderMap, name: &str) -> usize {
    let prefix = format!("{name}=");
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter(|part| part.trim().starts_with(&prefix))
        .count()
}

/// These historical name/path pairs can never address the current Secure credential. On HTTP
/// only the deprecated alias path may be retired; the current Studio path remains untouched.
fn safe_legacy_retirement(
    oidc: &crate::platform::studio_oidc::StudioOidc,
    headers: &HeaderMap,
) -> Vec<String> {
    let mut cookies = Vec::new();
    if oidc.session_name().starts_with("__Secure-") {
        if cookie_count(headers, "cantos_session") > 0 {
            cookies.push(
                "cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1/studio; Max-Age=0".into(),
            );
            cookies
                .push("cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1; Max-Age=0".into());
        }
        if cookie_count(headers, "cantos_oidc") > 0 {
            cookies.push(
                "cantos_oidc=; HttpOnly; SameSite=Lax; Path=/api/v1/studio; Max-Age=0".into(),
            );
        }
    } else if cookie_count(headers, "cantos_session") > 1 {
        cookies.push("cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1; Max-Age=0".into());
    }
    cookies
}

fn browser_inventory(
    headers: &HeaderMap,
    oidc: &StudioOidc,
) -> Result<BrowserInventory, AuthError> {
    let prefix = format!("{}_", oidc.flow_name());
    let mut values = Vec::new();
    for header in headers.get_all(header::COOKIE) {
        for part in header.to_str().map_err(|_| AuthError::State)?.split(';') {
            if let Some((name, value)) = part.trim().split_once('=') {
                if let Some(state) = name.strip_prefix(&prefix) {
                    values.push((state.into(), value.into()));
                }
            }
        }
    }
    BrowserInventory::new(values)
}

#[derive(Deserialize)]
struct LoginQuery {
    studio_session_id: Option<String>,
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LoginQuery>,
) -> Response {
    let Some(oidc) = state.store.studio_oidc() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    match browser_inventory(&headers, oidc) {
        Ok(inventory) if inventory.len() < MAX_BROWSER_FLOWS => {}
        _ => return error(AuthError::State),
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
    let previous = match unique_cookie(&headers, oidc.session_name()) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    if previous.is_some()
        || headers.contains_key(super::super::STUDIO_SESSION_HEADER)
        || query.studio_session_id.is_some()
    {
        let session_id = match previous.as_deref() {
            Some(previous) => match state.store.studio_session_id(previous).await {
                Ok(id) => id,
                Err(error) => return error_response(error),
            },
            None => None,
        };
        if !studio_fence_matches(&headers, session_id.as_deref())
            || query
                .studio_session_id
                .as_deref()
                .is_some_and(|expected| Some(expected) != session_id.as_deref())
        {
            return studio_fenced_response(
                failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                session_id.as_deref(),
            );
        }
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
    let Some(oidc) = state.store.studio_oidc() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    let Ok(Query(query)) = query else {
        return error(AuthError::State);
    };
    if browser_inventory(&headers, oidc).is_err() {
        return error(AuthError::State);
    }
    let name = match oidc.flow_cookie_name(&query.state) {
        Ok(name) => name,
        Err(e) => return error(e),
    };
    let browser = match unique_cookie(&headers, &name) {
        Ok(Some(v)) => v,
        _ => return error(AuthError::State),
    };
    match oidc
        .callback(&state.store, &query.state, &browser, &query.code)
        .await
    {
        // A consumed flow is already unusable. A delayed callback must not expire a newer
        // browser flow cookie shared by another tab; its binder ages out after 300 seconds.
        Ok(complete) => redirect("/", &[complete.cookie]),
        Err(e) => error(e),
    }
}

pub(super) async fn logout(state: AppState, headers: HeaderMap) -> Response {
    let Some(oidc) = state.store.studio_oidc() else {
        return error(AuthError::Configuration);
    };
    let retirement = safe_legacy_retirement(oidc, &headers);
    let token = match unique_cookie(&headers, oidc.session_name()) {
        Ok(v) => v,
        Err(e) => {
            return if retirement.is_empty() {
                error(e)
            } else {
                // Duplicate HTTP names cannot select an authenticated token. Retire only the
                // deprecated path and require a fresh observation; no logout is acknowledged.
                cookies(
                    failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                    &retirement,
                )
            };
        }
    };
    let expected = headers.contains_key(super::super::STUDIO_SESSION_HEADER);
    let stale_only = headers
        .get("x-cantos-studio-stale-only")
        .is_some_and(|value| value == "1");
    if !expected && !stale_only {
        return failure(ErrorCode::Unauthenticated, StatusCode::UNAUTHORIZED);
    }
    if let Some(token) = token.as_deref() {
        let session_id = match state.store.studio_session_id(token).await {
            Ok(id) => id,
            Err(error) => return error_response(error),
        };
        if !studio_fence_matches(&headers, session_id.as_deref()) {
            return cookies(
                studio_fenced_response(
                    failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                    session_id.as_deref(),
                ),
                &retirement,
            );
        }
        if headers
            .get("x-cantos-studio-stale-only")
            .is_some_and(|value| value == "1")
        {
            match state.store.authenticate(token).await {
                Ok(_) => {
                    return cookies(
                        studio_fenced_response(
                            failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                            session_id.as_deref(),
                        ),
                        &retirement,
                    )
                }
                Err(crate::platform::postgres::StoreError::Unauthenticated) => {}
                Err(error) => return error_response(error),
            }
        }
    }
    let inventory = match browser_inventory(&headers, oidc) {
        Ok(inventory) => inventory,
        Err(e) => return error(e),
    };
    let result = if expected && !stale_only {
        match token.as_deref() {
            Some(token) => {
                oidc.logout_in_browser(&state.store, token, &inventory)
                    .await
            }
            None => return failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
        }
    } else {
        // Cleanup has no authority to cancel another tab's fresh guest login.
        oidc.logout(&state.store, token.as_deref(), None).await
    };
    match result {
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
            // Browser cookies cannot condition an expiry header on the session value. A late
            // A response must not erase B's newly installed session/flow. The guarded UI leaves
            // only an inert locally revoked cookie, which the next successful login overwrites.
            if headers.contains_key(super::super::STUDIO_SESSION_HEADER)
                || headers.contains_key("x-cantos-studio-stale-only")
            {
                cookies(response, &retirement)
            } else {
                cookies(response, &complete.cookies)
            }
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

// Session parser tests execute the production parser, including multiple Cookie headers.
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
