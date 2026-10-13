//! Only the current Studio mount exposes OIDC. Handlers never inspect provider claims.
use crate::{
    interfaces::http::{failure, AppState},
    platform::studio_oidc::AuthError,
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

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
}

pub(in crate::interfaces::http) fn unique_cookie(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<String>, AuthError> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let value = value.to_str().map_err(|_| AuthError::State)?;
        for part in value.split(';') {
            if let Some(value) = part.trim().strip_prefix(&format!("{name}=")) {
                if found.is_some() {
                    return Err(AuthError::State);
                }
                found = Some(value.into());
            }
        }
    }
    Ok(found)
}

async fn login(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(oidc) = state.store.studio_oidc() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
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
    let browser = match unique_cookie(&headers, oidc.flow_name()) {
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

pub(super) async fn logout(state: AppState, headers: HeaderMap) -> Response {
    let Some(oidc) = state.store.studio_oidc() else {
        return error(AuthError::Configuration);
    };
    let token = match unique_cookie(&headers, oidc.session_name()) {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let browser = match unique_cookie(&headers, oidc.flow_name()) {
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
