//! Cantos Studio surface: script editing, import, adaptation and production planning.
//!
//! Mounted at `/api/v1/studio`. The same routes stay reachable at the pre-split `/api/v1/*` paths
//! as a deprecated compatibility alias that shares these handlers, so behavior cannot diverge.
//! Nothing here serves listeners; Theatre and Admin have their own modules and permission context.
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{
    routes, wire::WireFormat, ErrorCode, ReviewRequest, SaveRevisionRequest,
    ScriptValidationResponse, SessionRequest, SessionResponse, ValidateScriptRequest,
};

use super::wire::{respond, Accepts, WireBody};
use super::{authenticated, cookie_token, error_response, AppState};
use crate::platform::postgres::StoreError;

mod adaptations;
mod audio_intake;
mod audio_lifecycle;
pub(super) mod auth;
mod briefs;
mod catalog;
mod content;
mod imports;
mod production;
mod publication;

/// Which URL space a Studio route set is mounted under.
#[derive(Clone, Copy)]
pub(super) enum Mount {
    /// `/api/v1/studio`
    Current,
    /// Pre-split `/api/v1`; a deprecated alias with identical behavior.
    Legacy,
}

impl Mount {
    fn cookie_path(self) -> &'static str {
        match self {
            Mount::Current => routes::STUDIO_BASE,
            Mount::Legacy => routes::API_BASE,
        }
    }

    fn other_cookie_path(self) -> &'static str {
        match self {
            Mount::Current => routes::API_BASE,
            Mount::Legacy => routes::STUDIO_BASE,
        }
    }
}

pub(super) fn routes(state: AppState, mount: Mount) -> Router<AppState> {
    let scripts = Router::new()
        .route("/scripts/{script}/head", get(head))
        .route("/scripts/{script}/revisions/{revision}", get(revision))
        .route("/scripts/{script}/revisions", post(save))
        .route("/scripts/{script}/history", get(history))
        .route("/scripts/{script}/reviews", post(review))
        .route("/scripts/{script}/sources/{source}", get(source))
        .route("/validation", post(validate_script))
        .merge(imports::routes())
        .merge(adaptations::routes())
        .merge(production::routes());
    match mount {
        Mount::Current => scripts
            .merge(catalog::routes())
            // New Catalog/intake routes have no deprecated aliases.
            .merge(audio_intake::routes())
            .merge(audio_lifecycle::owner_routes())
            .route_layer(middleware::from_fn_with_state(state, authenticated))
            .merge(briefs::producer_routes())
            .merge(audio_lifecycle::producer_routes())
            .route("/session", post(login).get(current_session).delete(logout))
            .merge(auth::routes()),
        Mount::Legacy => scripts
            .route_layer(middleware::from_fn_with_state(state, authenticated))
            .route("/session", post(legacy_login).delete(legacy_logout))
            .layer(middleware::map_response(deprecated)),
    }
}

async fn deprecated(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert("deprecation", HeaderValue::from_static("true"));
    response.headers_mut().insert(
        header::LINK,
        HeaderValue::from_static("</api/v1/studio>; rel=\"successor-version\""),
    );
    response
}

async fn login(
    state: State<AppState>,
    headers: HeaderMap,
    body: Result<Json<SessionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    start_session(state, headers, body, Mount::Current).await
}

/// Confirm the cookie that the browser actually sends, independently of a token exchange.
async fn current_session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.authenticate(&token).await {
        Ok(actor_id) => match state.store.studio_session_id(&token).await {
            Ok(id) => super::studio_fenced_response(
                Json(SessionResponse { actor_id }).into_response(),
                id.as_deref(),
            ),
            Err(error) => error_response(error),
        },
        Err(error) => error_response(error),
    }
}

async fn legacy_login(
    state: State<AppState>,
    headers: HeaderMap,
    body: Result<Json<SessionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    start_session(state, headers, body, Mount::Legacy).await
}

async fn logout(state: State<AppState>, headers: HeaderMap) -> Response {
    end_session(state, headers, Mount::Current).await
}

async fn legacy_logout(state: State<AppState>, headers: HeaderMap) -> Response {
    end_session(state, headers, Mount::Legacy).await
}

async fn start_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<SessionRequest>, axum::extract::rejection::JsonRejection>,
    mount: Mount,
) -> Response {
    if state.store.studio_oidc().is_some() {
        return error_response(StoreError::Forbidden);
    }
    if headers.contains_key(super::STUDIO_SESSION_HEADER) {
        let actual = match cookie_token(&headers, &state.store) {
            Ok(token) => match state.store.studio_session_id(&token).await {
                Ok(id) => id,
                Err(error) => return error_response(error),
            },
            Err(_) => None,
        };
        if !super::studio_fence_matches(&headers, actual.as_deref()) {
            return super::studio_fenced_response(
                super::failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                actual.as_deref(),
            );
        }
    }
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    match state.store.authenticate(&request.token).await {
        Ok(actor_id) => {
            let mut response = Json(SessionResponse { actor_id }).into_response();
            let session_id = match state.store.studio_session_id(&request.token).await {
                Ok(id) => id,
                Err(error) => return error_response(error),
            };
            response = super::studio_fenced_response(response, session_id.as_deref());
            // Only loopback development HTTP is supported. Production requires HTTPS/Secure.
            // The cookie is scoped to the mount so a Studio session is never sent to Theatre or
            // Admin routes.
            let cookie = format!(
                "cantos_session={}; HttpOnly; SameSite=Strict; Path={}",
                request.token,
                mount.cookie_path()
            );
            match HeaderValue::from_str(&cookie) {
                Ok(value) => {
                    // Retire only the other known Studio mount. Otherwise a browser sends both
                    // same-name cookies at /studio and the fail-closed parser rejects the session.
                    let retired = format!(
                        "cantos_session=; HttpOnly; SameSite=Strict; Path={}; Max-Age=0",
                        mount.other_cookie_path()
                    );
                    let Ok(retired) = HeaderValue::from_str(&retired) else {
                        return error_response(StoreError::InvalidRequest);
                    };
                    response.headers_mut().append(header::SET_COOKIE, value);
                    response.headers_mut().append(header::SET_COOKIE, retired);
                    response
                }
                Err(_) => error_response(StoreError::InvalidRequest),
            }
        }
        Err(error) => error_response(error),
    }
}

async fn end_session(State(state): State<AppState>, headers: HeaderMap, mount: Mount) -> Response {
    if state.store.studio_oidc().is_some() {
        if matches!(mount, Mount::Legacy) {
            return error_response(StoreError::Forbidden);
        }
        return auth::logout(state, headers).await;
    }
    if let Ok(token) = cookie_token(&headers, &state.store) {
        let session_id = match state.store.studio_session_id(&token).await {
            Ok(id) => id,
            Err(error) => return error_response(error),
        };
        if !super::studio_fence_matches(&headers, session_id.as_deref()) {
            return super::studio_fenced_response(
                super::failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                session_id.as_deref(),
            );
        }
        if headers
            .get("x-cantos-studio-stale-only")
            .is_some_and(|value| value == "1")
        {
            match state.store.authenticate(&token).await {
                Ok(_) => {
                    return super::studio_fenced_response(
                        super::failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                        session_id.as_deref(),
                    )
                }
                Err(StoreError::Unauthenticated) => {}
                Err(error) => return error_response(error),
            }
        }
    }
    let result = match cookie_token(&headers, &state.store) {
        Ok(token) => state.store.revoke(&token).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            if headers.contains_key(super::STUDIO_SESSION_HEADER)
                || headers.contains_key("x-cantos-studio-stale-only")
            {
                return response;
            }
            let cookie = format!(
                "cantos_session=; HttpOnly; SameSite=Strict; Path={}; Max-Age=0",
                mount.cookie_path()
            );
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                response.headers_mut().insert(header::SET_COOKIE, value);
            }
            response
        }
        Err(error) => error_response(error),
    }
}

// Head, pinned revision, save and validation negotiate JSON, legacy MessagePack or Protobuf.
// The representation is settled by the `wire` extractors before the store is called.
async fn head(
    State(state): State<AppState>,
    Path(script): Path<String>,
    Accepts(format): Accepts,
    headers: HeaderMap,
) -> Response {
    read_revision(state, script, None, headers, format).await
}

async fn revision(
    State(state): State<AppState>,
    path: Result<Path<(String, u64)>, axum::extract::rejection::PathRejection>,
    Accepts(format): Accepts,
    headers: HeaderMap,
) -> Response {
    let (script, revision) = match path {
        Ok(Path(path)) => path,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    read_revision(state, script, Some(revision), headers, format).await
}

async fn read_revision(
    state: AppState,
    script: String,
    revision: Option<u64>,
    headers: HeaderMap,
    format: WireFormat,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.load(&token, &script, revision).await {
        Ok(saved) => respond(format, saved),
        Err(error) => error_response(error),
    }
}

async fn save(
    State(state): State<AppState>,
    Path(script): Path<String>,
    Accepts(format): Accepts,
    headers: HeaderMap,
    WireBody(request): WireBody<SaveRevisionRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.save(&token, &script, request).await {
        Ok(saved) => respond(format, saved),
        Err(error) => error_response(error),
    }
}

/// Draft preview uses the same bounded reader as storage, without creating a revision.
/// Authentication and Origin are checked by the shared middleware; this route reveals no
/// target/evidence facts and does not establish permission to accept, produce or publish.
async fn validate_script(
    Accepts(format): Accepts,
    WireBody(request): WireBody<ValidateScriptRequest>,
) -> Response {
    match crate::script_ir::read_script(request.script_json.as_bytes()) {
        Ok(_) => respond(format, ScriptValidationResponse { issues: vec![] }),
        Err(error) => error_response(StoreError::InvalidScript(error)),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryQuery {
    #[serde(default)]
    after_revision: u64,
    #[serde(default = "history_limit")]
    limit: u64,
}

fn history_limit() -> u64 {
    20
}

async fn history(
    State(state): State<AppState>,
    Path(script): Path<String>,
    headers: HeaderMap,
    query: Result<Query<HistoryQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let query = match query {
        Ok(Query(query)) => query,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state
        .store
        .history(&token, &script, query.after_revision, query.limit)
        .await
    {
        Ok(history) => Json(history).into_response(),
        Err(error) => error_response(error),
    }
}

async fn review(
    State(state): State<AppState>,
    Path(script): Path<String>,
    headers: HeaderMap,
    body: Result<Json<ReviewRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.review(&token, &script, request).await {
        Ok(review) => Json(review).into_response(),
        Err(error) => error_response(error),
    }
}

async fn source(
    State(state): State<AppState>,
    Path((script, source)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.source(&token, &script, &source).await {
        Ok(source) => Json(source).into_response(),
        Err(error) => error_response(error),
    }
}

/// Shared Studio transport boundary: malformed route parameters use ApiError JSON.
pub(super) struct StudioPath<T>(pub(super) T);
impl<S: Send + Sync, T: serde::de::DeserializeOwned + Send> axum::extract::FromRequestParts<S>
    for StudioPath<T>
{
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Response> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(value)| Self(value))
            .map_err(|_| error_response(StoreError::InvalidRequest))
    }
}
