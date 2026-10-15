//! Theatre-only OIDC HTTP boundary.
use crate::{
    interfaces::http::{failure, AppState},
    platform::theatre_identity::{AuthError, TheatreIdentity},
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

#[derive(Deserialize)]
struct LoginQuery {
    intent: Option<String>,
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<LoginQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Some(oidc) = state.store.theatre_identity() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if oidc.web_origin() != state.web_origin {
        return error(AuthError::Configuration);
    }
    let Ok(Query(query)) = query else {
        return error(AuthError::State);
    };
    if query.intent.as_deref().is_some_and(|v| !valid_ui_intent(v)) {
        return error(AuthError::State);
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
        Ok(start) => {
            let mut values = vec![start.cookie];
            if let Some(intent) = query.intent {
                let flow_state = match authorization_state(&start.url) {
                    Ok(value) => value,
                    Err(e) => return error(e),
                };
                values.push(return_cookie(oidc, &flow_state, &intent, 600));
            }
            redirect(&start.url, &values)
        }
        Err(e) => error(e),
    }
}

fn valid_ui_intent(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

fn authorization_state(value: &str) -> Result<String, AuthError> {
    let url = url::Url::parse(value).map_err(|_| AuthError::Configuration)?;
    let mut states = url.query_pairs().filter(|(name, _)| name == "state");
    let state = states
        .next()
        .ok_or(AuthError::Configuration)?
        .1
        .into_owned();
    if states.next().is_some() || !crate::platform::theatre_identity::valid_secret(&state) {
        return Err(AuthError::Configuration);
    }
    Ok(state)
}

fn return_cookie_name(oidc: &TheatreIdentity, state: &str) -> String {
    let prefix = if oidc.session_name().starts_with("__Secure-") {
        "__Secure-"
    } else {
        ""
    };
    format!("{prefix}cantos_theatre_return_{state}")
}

fn return_cookie(oidc: &TheatreIdentity, state: &str, intent: &str, age: u64) -> String {
    let secure = if oidc.session_name().starts_with("__Secure-") {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{}={intent}; HttpOnly; SameSite=Lax; Path=/api/v1/theatre/auth; Max-Age={age}{secure}",
        return_cookie_name(oidc, state)
    )
}

fn callback_intent(
    headers: &HeaderMap,
    oidc: &TheatreIdentity,
    state: &str,
) -> Result<Option<String>, AuthError> {
    let expected = return_cookie_name(oidc, state);
    let mut found = None;
    for name in [
        format!("cantos_theatre_return_{state}"),
        format!("__Secure-cantos_theatre_return_{state}"),
    ] {
        if let Some(value) = unique_cookie(headers, &name)? {
            if name != expected || !valid_ui_intent(&value) {
                return Err(AuthError::State);
            }
            found = Some(value);
        }
    }
    Ok(found)
}

fn callback_location(cancelled: bool, intent: Option<&str>) -> String {
    let hash = if cancelled {
        "auth-cancelled"
    } else {
        "auth-return"
    };
    match intent {
        Some(intent) => format!("/#{hash}?intent={intent}"),
        None => format!("/#{hash}"),
    }
}

#[derive(Deserialize)]
struct Callback {
    state: String,
    code: Option<String>,
    error: Option<ProviderCallbackError>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProviderCallbackError {
    AccessDenied,
    InvalidRequest,
    UnauthorizedClient,
    UnsupportedResponseType,
    InvalidScope,
    ServerError,
    TemporarilyUnavailable,
    InteractionRequired,
    LoginRequired,
    AccountSelectionRequired,
    ConsentRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallbackResponse<'a> {
    Code(&'a str),
    Cancelled,
}

impl Callback {
    fn response(&self) -> Result<CallbackResponse<'_>, AuthError> {
        if !crate::platform::theatre_identity::valid_secret(&self.state) {
            return Err(AuthError::State);
        }
        match (&self.code, self.error) {
            (Some(code), None)
                if !code.is_empty()
                    && code.len() <= 4096
                    && !code.chars().any(char::is_control) =>
            {
                Ok(CallbackResponse::Code(code))
            }
            (None, Some(_)) => Ok(CallbackResponse::Cancelled),
            _ => Err(AuthError::State),
        }
    }
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
    let response = match query.response() {
        Ok(response) => response,
        Err(e) => return error(e),
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
    let intent = match callback_intent(&headers, oidc, &query.state) {
        Ok(intent) => intent,
        Err(e) => return error(e),
    };
    let cleared = intent
        .as_ref()
        .map(|_| return_cookie(oidc, &query.state, "", 0))
        .into_iter()
        .collect::<Vec<_>>();
    if response == CallbackResponse::Cancelled {
        // Only a browser-bound provider failure returns to the app. It grants no session,
        // exchanges no code and never reflects provider text or a caller's return URL.
        return match oidc
            .cancel_callback(&state.store, &query.state, &browser)
            .await
        {
            Ok(()) => redirect(&callback_location(true, intent.as_deref()), &cleared),
            Err(e) => error(e),
        };
    }
    let CallbackResponse::Code(code) = response else {
        return error(AuthError::State);
    };
    match oidc
        .callback(&state.store, &query.state, &browser, code)
        .await
    {
        // Consumed/completed SQL flow remains one-time. A global expiry header here could
        // erase a newer flow cookie created by another tab while exchange was in flight.
        // The fixed hash is a completion intent only. The Web client restores its internal
        // return context and confirms /session; no arbitrary redirect is accepted here.
        Ok(complete) => {
            let mut values = cleared;
            values.push(complete.cookie);
            redirect(&callback_location(false, intent.as_deref()), &values)
        }
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

    fn callback_query(
        query: &str,
    ) -> Result<Query<Callback>, axum::extract::rejection::QueryRejection> {
        let uri = format!("/auth/callback?{query}").parse().unwrap();
        Query::<Callback>::try_from_uri(&uri)
    }

    #[test]
    fn ui_return_nonce_is_bounded_and_exactly_bound_to_one_flow_cookie() {
        let oidc = TheatreIdentity::new(
            "http://127.0.0.1:9000/oauth2/openid/cantos-theatre-web-bff",
            "http://127.0.0.1:8080",
            "fixture".into(),
        )
        .unwrap();
        let state_a = "a".repeat(64);
        let state_b = "b".repeat(64);
        let intent_a = "1".repeat(32);
        let intent_b = "2".repeat(32);
        assert!(valid_ui_intent(&intent_a));
        for value in [
            "",
            "abc",
            &"a".repeat(33),
            &"A".repeat(32),
            "https://example.com",
        ] {
            assert!(!valid_ui_intent(value));
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!(
            "cantos_theatre_return_{state_a}={intent_a}; cantos_theatre_return_{state_b}={intent_b}"
        ))
            .unwrap(),
        );
        assert_eq!(
            callback_intent(&headers, &oidc, &state_a),
            Ok(Some(intent_a.clone()))
        );
        assert_eq!(
            callback_intent(&headers, &oidc, &state_b),
            Ok(Some(intent_b.clone()))
        );
        assert_eq!(
            callback_location(false, Some(&intent_a)),
            format!("/#auth-return?intent={intent_a}")
        );
        assert_eq!(
            callback_location(true, Some(&intent_b)),
            format!("/#auth-cancelled?intent={intent_b}")
        );
        let cleared = return_cookie(&oidc, &state_a, "", 0);
        assert!(cleared.starts_with(&format!("cantos_theatre_return_{state_a}=")));
        assert!(cleared.contains("HttpOnly; SameSite=Lax; Path=/api/v1/theatre/auth; Max-Age=0"));
        assert!(!cleared.contains(&state_b));
        for value in [
            format!("cantos_theatre_return_{state_a}=bad"),
            format!("cantos_theatre_return_{state_a}={intent_a}; cantos_theatre_return_{state_a}={intent_a}"),
            format!("__Secure-cantos_theatre_return_{state_a}={intent_a}"),
        ] {
            headers.insert(header::COOKIE, HeaderValue::from_str(&value).unwrap());
            assert_eq!(callback_intent(&headers, &oidc, &state_a), Err(AuthError::State));
        }
        assert_eq!(
            authorization_state(&format!("https://example.com/authorize?state={state_a}")),
            Ok(state_a.clone())
        );
        assert_eq!(
            authorization_state(&format!(
                "https://example.com/authorize?state={state_a}&state={state_b}"
            )),
            Err(AuthError::Configuration)
        );
        let secure = TheatreIdentity::new(
            "https://id.example.com/oauth2/openid/cantos-theatre-web-bff",
            "https://cantos.example.com",
            "fixture".into(),
        )
        .unwrap();
        let cookie = return_cookie(&secure, &state_a, &intent_a, 600);
        assert!(cookie.starts_with("__Secure-cantos_theatre_return_"));
        assert!(cookie.ends_with("; Secure"));
    }

    #[test]
    fn callback_code_and_provider_error_are_exclusive_and_state_is_required() {
        let state = "a".repeat(64);
        let accepted = callback_query(&format!("state={state}&code=provider-code")).unwrap();
        assert_eq!(
            accepted.response(),
            Ok(CallbackResponse::Code("provider-code"))
        );
        for query in [
            format!("state={state}"),
            format!("state={state}&code="),
            format!("state={state}&code=%0A"),
            format!("state={state}&code=code&error=access_denied"),
            format!("state={state}&code=&error=access_denied"),
            "state=&code=code".into(),
            "state=not-a-flow&error=access_denied".into(),
        ] {
            assert_eq!(
                callback_query(&query).unwrap().response(),
                Err(AuthError::State),
                "unexpected acceptance: {query}"
            );
        }
        for query in [
            "code=code".into(),
            format!("state={state}&error="),
            format!("state={state}&error=unknown_provider_error"),
            format!("state={state}&code=first&code=second"),
            format!("state={state}&error=access_denied&error=server_error"),
            format!("state={state}&state={state}&code=code"),
        ] {
            assert!(
                callback_query(&query).is_err(),
                "unexpected parsing: {query}"
            );
        }
    }

    #[test]
    fn standard_provider_failures_are_non_authorizing_cancellation_intents() {
        let state = "a".repeat(64);
        for provider_error in [
            "access_denied",
            "invalid_request",
            "unauthorized_client",
            "unsupported_response_type",
            "invalid_scope",
            "server_error",
            "temporarily_unavailable",
            "interaction_required",
            "login_required",
            "account_selection_required",
            "consent_required",
        ] {
            let query = callback_query(&format!(
                "state={state}&error={provider_error}&error_description=ignored&error_uri=https://example.com&return_to=https://example.com"
            ))
            .unwrap();
            assert_eq!(query.response(), Ok(CallbackResponse::Cancelled));
        }
        let oversized =
            callback_query(&format!("state={state}&code={}", "a".repeat(4097))).unwrap();
        assert_eq!(oversized.response(), Err(AuthError::State));
    }

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
