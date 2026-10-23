//! JSON boundary for Identity-backed email/password sessions (#335).
//!
//! Web carries the session in an HttpOnly cookie that no script can read; native carries opaque
//! Bearer secrets. The two never mix: a cookie request may not carry `Authorization`, and a
//! native request may not carry `Origin`, `Cookie` or `Sec-Fetch-*`. Every failure is generic.
use super::{cookies::unique_cookie, errors::failure, AppState};
use crate::contract::{
    auth::bearer::{RefreshRequest, Session, TokenPair, TokenType},
    auth::identity_auth as dto,
    studio::ErrorCode,
};
use crate::platform::identity_auth::{
    credential::{self, Channel, Surface},
    decision::unhex,
    AuthFailure, IdentityAuth, Login, SignupCommand,
};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use std::{net::SocketAddr, sync::Arc};

type Peer = Option<Extension<ConnectInfo<SocketAddr>>>;

/// A refusal already shaped as its response, boxed so `Result` stays small on the happy path.
struct Rejection(Box<Response>);

impl From<Response> for Rejection {
    fn from(response: Response) -> Self {
        Self(Box::new(response))
    }
}

impl IntoResponse for Rejection {
    fn into_response(self) -> Response {
        *self.0
    }
}
type Body<T> = Result<Json<T>, axum::extract::rejection::JsonRejection>;

const BODY_LIMIT: usize = 4096;
const THEATRE_FENCE: &str = "x-cantos-theatre-session";
const STUDIO_FENCE: &str = super::studio::admission::STUDIO_SESSION_HEADER;

pub(super) fn theatre_routes() -> Router<AppState> {
    let web = Router::new()
        .route("/identity/signup", post(signup_start))
        .route("/identity/signup/complete", post(signup_complete))
        .route("/identity/login", post(login_web))
        .route("/identity/session", get(session_web).delete(logout_web))
        .route("/identity/session/renew", post(renew_web))
        .layer(Extension(Channel::Web(Surface::Theatre)));
    let native = Router::new()
        .route("/native/identity/signup", post(signup_start))
        .route("/native/identity/signup/complete", post(signup_complete))
        .route("/native/identity/login", post(login_native))
        .route("/native/identity/refresh", post(refresh_native))
        .route("/native/identity/revoke", post(revoke_native))
        .route(
            "/native/identity/session",
            get(session_native).delete(logout_native),
        )
        .layer(Extension(Channel::Native));
    web.merge(native).layer(DefaultBodyLimit::max(BODY_LIMIT))
}

pub(super) fn studio_routes() -> Router<AppState> {
    Router::new()
        .route("/identity/login", post(login_web))
        .route("/identity/session", get(session_web).delete(logout_web))
        .route("/identity/session/renew", post(renew_web))
        .layer(Extension(Channel::Web(Surface::Studio)))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

/// Exactly the originless native mutations; never a prefix, never another surface.
pub(super) fn native_mutation(method: &Method, path: &str) -> bool {
    const BASE: &str = "/api/v1/theatre/native/identity";
    (*method == Method::POST
        && matches!(
            path.strip_prefix(BASE),
            Some("/signup" | "/signup/complete" | "/login" | "/refresh" | "/revoke")
        ))
        || (*method == Method::DELETE && path == "/api/v1/theatre/native/identity/session")
}

fn service(state: &AppState) -> Result<Arc<IdentityAuth>, Rejection> {
    state
        .store
        .identity_auth()
        .cloned()
        .ok_or_else(|| failure(ErrorCode::NotFound, StatusCode::NOT_FOUND).into())
}

fn refusal(failure_kind: AuthFailure) -> Response {
    match failure_kind {
        AuthFailure::Unauthenticated => {
            failure(ErrorCode::Unauthenticated, StatusCode::UNAUTHORIZED)
        }
        AuthFailure::Forbidden => failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN),
        AuthFailure::InvalidRequest => failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST),
        AuthFailure::Unavailable => {
            failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE)
        }
        AuthFailure::Throttled {
            retry_after_seconds,
        } => {
            let mut response = failure(ErrorCode::Unavailable, StatusCode::TOO_MANY_REQUESTS);
            if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
            response
        }
    }
}

fn unauthenticated() -> Response {
    refusal(AuthFailure::Unauthenticated)
}

// ---- header admission --------------------------------------------------------------------

/// Browser admission: exact Origin for every mutation (the CSRF barrier; the JSON-only bodies
/// make a cross-site form post impossible), never a cross-site fetch, and no Bearer alongside
/// the cookie.
fn web_gate(state: &AppState, headers: &HeaderMap, mutation: bool) -> Result<(), Rejection> {
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(unauthenticated().into());
    }
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let origin = origins.next();
    let allowed = origins.next().is_none()
        && match origin {
            Some(value) => value.as_bytes() == state.web_origin.as_bytes(),
            None => !mutation,
        };
    if !allowed
        || headers
            .get("sec-fetch-site")
            .is_some_and(|value| value == "cross-site")
    {
        return Err(failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN).into());
    }
    Ok(())
}

/// Native admission: a real app sends none of these, so any of them marks a browser (or a
/// confused client) and is refused rather than interpreted.
fn native_gate(headers: &HeaderMap, bearer: bool) -> Result<(), Rejection> {
    if [header::ORIGIN, header::COOKIE]
        .iter()
        .any(|name| headers.contains_key(name))
        || headers
            .keys()
            .any(|name| name.as_str().starts_with("sec-fetch-"))
        || (!bearer && headers.contains_key(header::AUTHORIZATION))
    {
        return Err(unauthenticated().into());
    }
    Ok(())
}

fn bearer_token(headers: &HeaderMap) -> Result<String, Rejection> {
    native_gate(headers, true)?;
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let token = values
        .next()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| Rejection::from(unauthenticated()))?;
    if values.next().is_some() || credential::access_channel(token) != Some(Channel::Native) {
        return Err(unauthenticated().into());
    }
    Ok(token.to_owned())
}

/// The one cookie of this surface; the other secure/plain spelling or a duplicate is refused.
fn cookie_token(
    service: &IdentityAuth,
    headers: &HeaderMap,
    surface: Surface,
) -> Result<Option<String>, Rejection> {
    let expected = credential::cookie_name(surface, service.secure());
    let other = credential::cookie_name(surface, !service.secure());
    let read = |name| unique_cookie(headers, name).map_err(|_| Rejection::from(unauthenticated()));
    if read(other)?.is_some() {
        return Err(unauthenticated().into());
    }
    let token = read(expected)?;
    // A secret that belongs to another surface/channel never authenticates here.
    if token
        .as_deref()
        .is_some_and(|value| credential::access_channel(value) != Some(Channel::Web(surface)))
    {
        return Err(unauthenticated().into());
    }
    Ok(token)
}

fn client_ip(
    service: &IdentityAuth,
    peer: Peer,
    headers: &HeaderMap,
) -> Result<std::net::IpAddr, Rejection> {
    let Some(Extension(ConnectInfo(peer))) = peer else {
        return Err(refusal(AuthFailure::Unavailable).into());
    };
    let lines = headers
        .get_all("x-forwarded-for")
        .iter()
        .map(|line| line.to_str())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Rejection::from(refusal(AuthFailure::Unavailable)))?;
    service
        .client_ip(peer.ip(), &lines)
        .map_err(|e| refusal(e).into())
}

fn admit_anonymous(
    state: &AppState,
    channel: Channel,
    headers: &HeaderMap,
) -> Result<(), Rejection> {
    match channel {
        Channel::Web(_) => web_gate(state, headers, true),
        Channel::Native => native_gate(headers, false),
    }
}

// ---- signup (Theatre only) ---------------------------------------------------------------

async fn signup_start(
    State(state): State<AppState>,
    Extension(channel): Extension<Channel>,
    peer: Peer,
    headers: HeaderMap,
    body: Body<dto::SignupStart>,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = admit_anonymous(&state, channel, &headers) {
        return e.into_response();
    }
    let Ok(Json(body)) = body else {
        return refusal(AuthFailure::InvalidRequest);
    };
    let ip = match client_ip(&service, peer, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match service.start_signup(&body.email, ip).await {
        Ok(ttl) => (
            StatusCode::ACCEPTED,
            Json(dto::SignupAccepted {
                accepted: true,
                proof_ttl_seconds: ttl,
            }),
        )
            .into_response(),
        Err(e) => refusal(e),
    }
}

async fn signup_complete(
    State(state): State<AppState>,
    Extension(channel): Extension<Channel>,
    peer: Peer,
    headers: HeaderMap,
    body: Body<dto::SignupComplete>,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = admit_anonymous(&state, channel, &headers) {
        return e.into_response();
    }
    let Ok(Json(body)) = body else {
        return refusal(AuthFailure::InvalidRequest);
    };
    let (Some(operation), Some(proof), true) = (
        unhex::<16>(&body.operation_id),
        unhex::<16>(&body.proof_id),
        body.explicit_confirmation,
    ) else {
        return refusal(AuthFailure::InvalidRequest);
    };
    let secret = match (
        unhex::<32>(&body.proof_secret),
        body.firebase_oob_code.as_deref(),
    ) {
        (Some(secret), None) => secret,
        (None, Some(code))
            if body.proof_secret.is_empty()
                && !code.is_empty()
                && code.len() <= 4096
                && code.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
                }) =>
        {
            [0; 32]
        }
        _ => return refusal(AuthFailure::InvalidRequest),
    };
    let ip = match client_ip(&service, peer, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let command = SignupCommand {
        operation,
        proof,
        secret,
        firebase_oob_code: body.firebase_oob_code,
        password: body.password,
    };
    match service.complete_signup(&state.store, command, ip).await {
        Ok(replayed) => Json(dto::SignupCompleted {
            status: if replayed {
                dto::SignupStatus::Replayed
            } else {
                dto::SignupStatus::Completed
            },
        })
        .into_response(),
        Err(e) => refusal(e),
    }
}

// ---- Web: HttpOnly cookie ----------------------------------------------------------------

fn fence(surface: Surface) -> HeaderName {
    HeaderName::from_static(match surface {
        Surface::Theatre => THEATRE_FENCE,
        Surface::Studio => STUDIO_FENCE,
    })
}

fn stamped(mut response: Response, surface: Surface, session: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(session) {
        response.headers_mut().insert(fence(surface), value);
    }
    response
}

fn with_cookie(mut response: Response, cookie: &str) -> Response {
    match HeaderValue::from_str(cookie) {
        Ok(value) => {
            response.headers_mut().append(header::SET_COOKIE, value);
            response
        }
        Err(_) => refusal(AuthFailure::Unavailable),
    }
}

fn seconds_until(deadline: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    deadline.saturating_sub(now)
}

fn view(actor: &str, session: &str, expires: u64, absolute: u64, authenticated_at: u64) -> Session {
    Session {
        actor_id: actor.to_owned(),
        session_id: session.to_owned(),
        expires_at: expires,
        absolute_expires_at: absolute,
        authenticated_at: Some(authenticated_at),
    }
}

async fn login_web(
    State(state): State<AppState>,
    Extension(channel): Extension<Channel>,
    peer: Peer,
    headers: HeaderMap,
    body: Body<dto::Login>,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let surface = channel.surface();
    if surface == Surface::Studio && !service.studio_enabled() {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    }
    if let Err(e) = web_gate(&state, &headers, true) {
        return e.into_response();
    }
    // A previous cookie of this surface is replaced and retired; a duplicate or foreign one is
    // refused before any credential is checked.
    let previous = match cookie_token(&service, &headers, surface) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let Ok(Json(body)) = body else {
        return refusal(AuthFailure::InvalidRequest);
    };
    let ip = match client_ip(&service, peer, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let login = match service
        .login(&state.store, channel, &body.email, &body.password, ip)
        .await
    {
        Ok(v) => v,
        Err(e) => return refusal(e),
    };
    if let Some(previous) = previous {
        // Best effort: the new session is already durable, and the old one dies on its own.
        let _ = service.logout(&state.store, &previous, channel).await;
    }
    let Login {
        actor_id,
        session_id,
        lifetime,
        authenticated_at,
        secrets,
    } = login;
    let cookie = credential::set_cookie(
        surface,
        service.secure(),
        &secrets.access,
        seconds_until(lifetime.idle),
    );
    let response = Json(view(
        &actor_id,
        &session_id,
        lifetime.idle,
        lifetime.absolute,
        authenticated_at,
    ))
    .into_response();
    stamped(with_cookie(response, &cookie), surface, &session_id)
}

async fn session_web(
    State(state): State<AppState>,
    Extension(channel): Extension<Channel>,
    headers: HeaderMap,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = web_gate(&state, &headers, false) {
        return e.into_response();
    }
    let surface = channel.surface();
    let token = match cookie_token(&service, &headers, surface) {
        Ok(Some(v)) => v,
        Ok(None) => return unauthenticated(),
        Err(e) => return e.into_response(),
    };
    match service.authenticate(&state.store, &token, channel).await {
        Ok(record) => stamped(
            Json(view(
                &record.actor_id,
                &record.session_id,
                record.token_expires,
                record.absolute_expires,
                record.authenticated_at,
            ))
            .into_response(),
            surface,
            &record.session_id,
        ),
        Err(e) => refusal(e),
    }
}

async fn renew_web(
    State(state): State<AppState>,
    Extension(channel): Extension<Channel>,
    headers: HeaderMap,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = web_gate(&state, &headers, true) {
        return e.into_response();
    }
    let surface = channel.surface();
    let token = match cookie_token(&service, &headers, surface) {
        Ok(Some(v)) => v,
        Ok(None) => return unauthenticated(),
        Err(e) => return e.into_response(),
    };
    if let Some(refusal_response) = fence_refusal(&service, &state, &headers, &token, channel).await
    {
        return refusal_response;
    }
    match service.renew_web(&state.store, &token, channel).await {
        Ok((record, lifetime)) => {
            let cookie = credential::set_cookie(
                surface,
                service.secure(),
                &token,
                seconds_until(lifetime.idle),
            );
            stamped(
                with_cookie(
                    Json(view(
                        &record.actor_id,
                        &record.session_id,
                        lifetime.idle,
                        record.absolute_expires,
                        record.authenticated_at,
                    ))
                    .into_response(),
                    &cookie,
                ),
                surface,
                &record.session_id,
            )
        }
        Err(e) => refusal(e),
    }
}

/// The client names the session it believes it holds. A mismatch means another tab switched
/// account: refuse and say which session is current, never act on the wrong one.
async fn fence_refusal(
    service: &IdentityAuth,
    state: &AppState,
    headers: &HeaderMap,
    token: &str,
    channel: Channel,
) -> Option<Response> {
    let mut values = headers.get_all(fence(channel.surface())).iter();
    let declared = values.next()?;
    let actual = match service.session_id_of(&state.store, token, channel).await {
        Ok(actual) => actual,
        Err(e) => return Some(refusal(e)),
    };
    if values.next().is_none() && declared.to_str().ok() == actual.as_deref() {
        return None;
    }
    let response = failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT);
    Some(match actual {
        Some(id) => stamped(response, channel.surface(), &id),
        None => response,
    })
}

async fn logout_web(
    State(state): State<AppState>,
    Extension(channel): Extension<Channel>,
    headers: HeaderMap,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = web_gate(&state, &headers, true) {
        return e.into_response();
    }
    let surface = channel.surface();
    let token = match cookie_token(&service, &headers, surface) {
        Ok(Some(v)) => v,
        Ok(None) => return unauthenticated(),
        Err(e) => return e.into_response(),
    };
    if let Some(refusal_response) = fence_refusal(&service, &state, &headers, &token, channel).await
    {
        return refusal_response;
    }
    match service.logout(&state.store, &token, channel).await {
        Ok(()) => with_cookie(
            StatusCode::NO_CONTENT.into_response(),
            &credential::set_cookie(surface, service.secure(), "", 0),
        ),
        Err(e) => refusal(e),
    }
}

// ---- Native: opaque Bearer ---------------------------------------------------------------

fn token_pair(
    actor: String,
    session: String,
    lifetime: crate::platform::bearer::core::Lifetime,
    authenticated_at: u64,
    secrets: credential::Secrets,
) -> Response {
    let Some(refresh) = secrets.refresh else {
        return refusal(AuthFailure::Unavailable);
    };
    Json(TokenPair {
        token_type: TokenType::Bearer,
        access_token: secrets.access,
        refresh_token: refresh,
        actor_id: actor,
        session_id: session,
        expires_at: lifetime.access,
        refresh_expires_at: lifetime.idle,
        absolute_expires_at: lifetime.absolute,
        authenticated_at: Some(authenticated_at),
    })
    .into_response()
}

async fn login_native(
    State(state): State<AppState>,
    peer: Peer,
    headers: HeaderMap,
    body: Body<dto::Login>,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = native_gate(&headers, false) {
        return e.into_response();
    }
    let Ok(Json(body)) = body else {
        return refusal(AuthFailure::InvalidRequest);
    };
    let ip = match client_ip(&service, peer, &headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match service
        .login(
            &state.store,
            Channel::Native,
            &body.email,
            &body.password,
            ip,
        )
        .await
    {
        Ok(login) => token_pair(
            login.actor_id,
            login.session_id,
            login.lifetime,
            login.authenticated_at,
            login.secrets,
        ),
        Err(e) => refusal(e),
    }
}

async fn refresh_native(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body<RefreshRequest>,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = native_gate(&headers, false) {
        return e.into_response();
    }
    let Ok(Json(body)) = body else {
        return refusal(AuthFailure::InvalidRequest);
    };
    match service
        .refresh_native(&state.store, &body.refresh_token, &body.session_id)
        .await
    {
        Ok((actor, authenticated_at, lifetime, secrets)) => {
            token_pair(actor, body.session_id, lifetime, authenticated_at, secrets)
        }
        Err(e) => refusal(e),
    }
}

async fn revoke_native(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body<RefreshRequest>,
) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = native_gate(&headers, false) {
        return e.into_response();
    }
    let Ok(Json(body)) = body else {
        return refusal(AuthFailure::InvalidRequest);
    };
    match service
        .revoke_refresh(&state.store, &body.refresh_token, &body.session_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => refusal(e),
    }
}

async fn session_native(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let token = match bearer_token(&headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match service
        .authenticate(&state.store, &token, Channel::Native)
        .await
    {
        Ok(record) => stamped(
            Json(view(
                &record.actor_id,
                &record.session_id,
                record.token_expires,
                record.absolute_expires,
                record.authenticated_at,
            ))
            .into_response(),
            Surface::Theatre,
            &record.session_id,
        ),
        Err(e) => refusal(e),
    }
}

async fn logout_native(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let service = match service(&state) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let token = match bearer_token(&headers) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match service.logout(&state.store, &token, Channel::Native).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => refusal(e),
    }
}

// ---- protected admission: Theatre library and Studio -----------------------------------------

/// What a Theatre account request presents, decided from headers alone.
pub(super) enum TheatreCredential {
    /// Nothing from this family. The request continues to the Library's listener port, which
    /// admits nothing by default (a trusted host override may supply its own verifier).
    Absent,
    /// Mixed, duplicate, wrong-surface or wrong-kind: refused, never retried against another kind.
    Refused,
    Present(Channel, String),
}

pub(super) fn theatre_credential(state: &AppState, headers: &HeaderMap) -> TheatreCredential {
    let Some(service) = state.store.identity_auth() else {
        return TheatreCredential::Absent;
    };
    let authorization = headers.get(header::AUTHORIZATION);
    let cookie = |name| unique_cookie(headers, name);
    let (expected, other) = (
        credential::cookie_name(Surface::Theatre, service.secure()),
        credential::cookie_name(Surface::Theatre, !service.secure()),
    );
    let (mine, wrong_spelling) = match (cookie(expected), cookie(other)) {
        (Ok(mine), Ok(wrong)) => (mine, wrong),
        _ => return TheatreCredential::Refused,
    };
    let bearer = authorization
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| value.starts_with("ci1_"));
    match (bearer, mine, wrong_spelling) {
        // Any other Authorization is not an Identity credential; the listener port decides.
        (None, None, None) => TheatreCredential::Absent,
        (Some(token), None, None) => {
            if headers.get_all(header::AUTHORIZATION).iter().count() == 1
                && !headers.contains_key(header::COOKIE)
                && credential::access_channel(token) == Some(Channel::Native)
            {
                TheatreCredential::Present(Channel::Native, token.to_owned())
            } else {
                TheatreCredential::Refused
            }
        }
        (None, Some(token), None) => {
            if authorization.is_none()
                && credential::access_channel(&token) == Some(Channel::Web(Surface::Theatre))
                && !legacy_theatre_cookie(headers)
            {
                TheatreCredential::Present(Channel::Web(Surface::Theatre), token)
            } else {
                TheatreCredential::Refused
            }
        }
        _ => TheatreCredential::Refused,
    }
}

/// The retired provider-bound Web session cookie; a browser still holding one beside ours is an
/// ambiguous request and is refused, never retried against another credential kind.
fn legacy_theatre_cookie(headers: &HeaderMap) -> bool {
    ["cantos_theatre", "__Secure-cantos_theatre"]
        .iter()
        .any(|name| unique_cookie(headers, name).map_or(true, |value| value.is_some()))
}

/// Theatre account routes (`/library/*`, `/progress/*`, `/operations/*`) admit only an Identity v1
/// session: a mixed, duplicate, wrong-surface or wrong-kind credential is refused here and never
/// retried against another verifier. A request without an Identity credential continues to the
/// Library's listener port, which is [`DenyListenerAuthentication`](crate::contexts::listening::auth::DenyListenerAuthentication)
/// unless a trusted host supplied its own.
pub(super) async fn bff_identity(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path();
    let account = path.starts_with("/library/")
        || path.starts_with("/progress/")
        || path.starts_with("/operations/");
    if !account {
        return next.run(request).await;
    }
    match theatre_credential(&state, request.headers()) {
        TheatreCredential::Absent => next.run(request).await,
        TheatreCredential::Refused => {
            super::theatre_library::error(crate::contexts::listening::LibraryError::Unauthenticated)
        }
        TheatreCredential::Present(channel, token) => {
            admit_theatre(state, request, next, channel, token).await
        }
    }
}

/// The client declares which nonsecret session owns its intent. The header grants no authority;
/// it rejects a cookie/bearer account switch after dispatch, before any resource mutation.
pub(super) fn session_fence_refusal(headers: &HeaderMap, actual: &str) -> Option<Response> {
    let mut values = headers.get_all(THEATRE_FENCE).iter();
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

fn stamp_session(response: Response, session: &str) -> Response {
    stamped(response, Surface::Theatre, session)
}

/// Runs `next` as the account's listener while the session family and actor stay locked.
/// A cookie request is browser-originated, so the exact-Origin CSRF rule is enforced here too:
/// the Library's originless-mutation exemption belongs to Bearer requests only.
pub(super) async fn admit_theatre(
    state: AppState,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
    channel: Channel,
    token: String,
) -> Response {
    use crate::contexts::listening::{
        auth::{
            AuthenticationError, ListenerAuthenticator, ListenerCredentials,
            PasswordListenerAuthenticator,
        },
        LibraryError,
    };
    let Some(service) = state.store.identity_auth().cloned() else {
        return super::theatre_library::error(LibraryError::Unauthenticated);
    };
    if matches!(channel, Channel::Web(_)) {
        let read = matches!(*request.method(), Method::GET | Method::HEAD);
        if web_gate(&state, request.headers(), !read).is_err() {
            return super::theatre_library::error(LibraryError::Unauthenticated);
        }
    } else if request.headers().contains_key(header::ORIGIN)
        && request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            != Some(state.web_origin.as_str())
    {
        // A supplied Origin must be this surface's, as for every Theatre mutation.
        return super::theatre_library::error(LibraryError::Unauthenticated);
    }
    let store = state.store.clone();
    let credentials = match ListenerCredentials::bearer(&token) {
        Ok(v) => v,
        Err(_) => return super::theatre_library::error(LibraryError::Unauthenticated),
    };
    // The principal needs a fresh Identity read; the lease below then only locks the session.
    let principal = match PasswordListenerAuthenticator::new(store.clone())
        .authenticate(credentials)
        .await
    {
        Ok(v) => v,
        Err(AuthenticationError::Unauthenticated) => {
            return super::theatre_library::error(LibraryError::Unauthenticated)
        }
        Err(AuthenticationError::Unavailable) => {
            return super::theatre_library::error(LibraryError::Unavailable)
        }
    };
    let result = service
        .lease(&store, &token, channel, |record| async move {
            if let Some(response) = session_fence_refusal(request.headers(), &record.session_id) {
                return response;
            }
            request
                .extensions_mut()
                .insert(crate::contract::theatre::identity::Session {
                    actor_id: record.actor_id.clone(),
                    session_id: record.session_id.clone(),
                });
            request.extensions_mut().insert(principal);
            stamp_session(next.run(request).await, &record.session_id)
        })
        .await;
    match result {
        Ok(response) => response,
        Err(AuthFailure::Unavailable | AuthFailure::Throttled { .. }) => {
            super::theatre_library::error(LibraryError::Unavailable)
        }
        Err(_) => super::theatre_library::error(LibraryError::Unauthenticated),
    }
}

#[cfg(test)]
mod tests;
