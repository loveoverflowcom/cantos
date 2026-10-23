//! Studio credential selection, request fences and admission-lease lifetime.
use super::super::{
    cookies::unique_cookie,
    errors::{error_response, failure},
    AppState,
};
use crate::application::{Store, StoreError};
use crate::contract::studio::ErrorCode;
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};

pub(in crate::adapters::inbound::http) fn cookie_token(
    headers: &HeaderMap,
    store: &Store,
) -> Result<String, StoreError> {
    // Studio has no Bearer credential: an Authorization header is never a Studio session.
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(StoreError::Unauthenticated);
    }
    // An Identity-issued cookie is its own credential kind: found first, and never together
    // with a development-token cookie.
    if let Some(token) = identity_cookie(headers, store)? {
        return Ok(token);
    }
    let token = cookie_token_for_name(headers, DEV_COOKIE)?;
    // Identity-issued secrets (and the retired provider family) have their own cookie; neither may
    // ride the development-token name, where the `__Secure-`/host-only guarantees are absent.
    if token.starts_with("ca1_") || token.starts_with("ci1_") {
        return Err(StoreError::Unauthenticated);
    }
    Ok(token)
}

fn identity_cookie(headers: &HeaderMap, store: &Store) -> Result<Option<String>, StoreError> {
    use crate::platform::identity_auth::credential::{self, Channel, Surface};
    let Some(service) = store.identity_auth() else {
        return Ok(None);
    };
    let (expected, other) = (
        credential::cookie_name(Surface::Studio, service.secure()),
        credential::cookie_name(Surface::Studio, !service.secure()),
    );
    let read = |name| unique_cookie(headers, name).map_err(|_| StoreError::Unauthenticated);
    if read(other)?.is_some() {
        return Err(StoreError::Unauthenticated);
    }
    let Some(token) = read(expected)? else {
        return Ok(None);
    };
    let legacy = DEV_COOKIE_NAMES
        .iter()
        .any(|name| unique_cookie(headers, name).map_or(true, |value| value.is_some()));
    if legacy || credential::access_channel(&token) != Some(Channel::Web(Surface::Studio)) {
        return Err(StoreError::Unauthenticated);
    }
    Ok(Some(token))
}

pub(in crate::adapters::inbound::http) fn cookie_token_for_name(
    headers: &HeaderMap,
    expected: &str,
) -> Result<String, StoreError> {
    let mut found = None;
    for name in DEV_COOKIE_NAMES {
        let value = unique_cookie(headers, name).map_err(|_| StoreError::Unauthenticated)?;
        if let Some(value) = value {
            if name != expected || found.is_some() {
                return Err(StoreError::Unauthenticated);
            }
            found = Some(value);
        }
    }
    found.ok_or(StoreError::Unauthenticated)
}

pub(in crate::adapters::inbound::http) async fn authenticated(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let token = match cookie_token(request.headers(), &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    if let Err(error) = state.store.authenticate(&token).await {
        return error_response(error);
    }
    let session_id = match state.store.studio_session_id(&token).await {
        Ok(id) => id,
        Err(error) => return error_response(error),
    };
    if !studio_query_fence_matches(request.uri(), session_id.as_deref())
        || !studio_fence_matches(request.headers(), session_id.as_deref())
    {
        return studio_fenced_response(
            failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
            session_id.as_deref(),
        );
    }
    studio_fenced_response(next.run(request).await, session_id.as_deref())
}

pub(in crate::adapters::inbound::http) fn studio_query_fence_matches(
    uri: &axum::http::Uri,
    session_id: Option<&str>,
) -> bool {
    let mut query_fences = url::form_urlencoded::parse(
        uri.path_and_query()
            .and_then(axum::http::uri::PathAndQuery::query)
            .unwrap_or("")
            .as_bytes(),
    )
    .filter(|(name, _)| name == "studio_session_id");
    match query_fences.next() {
        None => true,
        Some((_, value)) => query_fences.next().is_none() && Some(value.as_ref()) == session_id,
    }
}

pub(in crate::adapters::inbound::http) const STUDIO_SESSION_HEADER: &str =
    "x-cantos-studio-session";

/// The development-token cookie the loopback Studio mount issues, and the `__Secure-` spelling a
/// Studio session must never use. Issuing (`studio/mod.rs`) and reading/rejecting (here) both
/// name them from this one owner, so a rename cannot leave the collision check stale.
pub(in crate::adapters::inbound::http) const DEV_COOKIE: &str = "cantos_session";
const DEV_SECURE_COOKIE: &str = "__Secure-cantos_studio";
const DEV_COOKIE_NAMES: [&str; 2] = [DEV_COOKIE, DEV_SECURE_COOKIE];

pub(in crate::adapters::inbound::http) fn studio_fence_matches(
    headers: &HeaderMap,
    actual: Option<&str>,
) -> bool {
    let mut values = headers.get_all(STUDIO_SESSION_HEADER).iter();
    match values.next() {
        None => true, // Legacy explicit clients remain compatible; the current UI always fences.
        Some(value) => {
            values.next().is_none()
                && value
                    .to_str()
                    .ok()
                    .is_some_and(|value| Some(value) == actual)
        }
    }
}

pub(in crate::adapters::inbound::http) fn studio_fenced_response(
    mut response: Response,
    session_id: Option<&str>,
) -> Response {
    if let Some(id) = session_id.and_then(|id| HeaderValue::from_str(id).ok()) {
        response.headers_mut().insert(STUDIO_SESSION_HEADER, id);
    }
    response
}

// Session parser tests execute the production parser, including multiple Cookie headers.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_cookie_headers_and_mixed_session_names_are_rejected() {
        let mut headers = HeaderMap::new();
        headers.append(header::COOKIE, HeaderValue::from_static("cantos_session=a"));
        headers.append(header::COOKIE, HeaderValue::from_static("cantos_session=b"));
        assert!(matches!(
            cookie_token_for_name(&headers, "cantos_session"),
            Err(StoreError::Unauthenticated)
        ));
        assert!(unique_cookie(&headers, "cantos_session").is_err());
        headers.clear();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("cantos_session=attacker"),
        );
        assert!(matches!(
            cookie_token_for_name(&headers, "__Secure-cantos_studio"),
            Err(StoreError::Unauthenticated)
        ));
        headers.clear();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("cantos_session=a; __Secure-cantos_studio=b"),
        );
        assert!(matches!(
            cookie_token_for_name(&headers, "cantos_session"),
            Err(StoreError::Unauthenticated)
        ));
    }

    #[test]
    fn studio_never_accepts_an_authorization_header_as_a_session() {
        let store = crate::test_support::detached_store();
        for value in ["Bearer ci1_s_x", "Bearer ca1_s_x", "Bearer 00"] {
            let mut headers = HeaderMap::new();
            headers.insert(header::AUTHORIZATION, HeaderValue::from_static(value));
            assert!(matches!(
                cookie_token(&headers, &store),
                Err(StoreError::Unauthenticated)
            ));
        }
    }
}
