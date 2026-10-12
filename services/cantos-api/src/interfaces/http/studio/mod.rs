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
    routes, wire::WireFormat, ReviewRequest, SaveRevisionRequest, ScriptValidationResponse,
    SessionRequest, SessionResponse, ValidateScriptRequest,
};

use super::wire::{respond, Accepts, WireBody};
use super::{authenticated, cookie_token, error_response, AppState};
use crate::platform::postgres::StoreError;

mod adaptations;
mod catalog;
mod content;
mod imports;
mod production;

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
            .route_layer(middleware::from_fn_with_state(state, authenticated))
            .route("/session", post(login).delete(logout)),
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
    body: Result<Json<SessionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    start_session(state, body, Mount::Current).await
}

async fn legacy_login(
    state: State<AppState>,
    body: Result<Json<SessionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    start_session(state, body, Mount::Legacy).await
}

async fn logout(state: State<AppState>, headers: HeaderMap) -> Response {
    end_session(state, headers, Mount::Current).await
}

async fn legacy_logout(state: State<AppState>, headers: HeaderMap) -> Response {
    end_session(state, headers, Mount::Legacy).await
}

async fn start_session(
    State(state): State<AppState>,
    body: Result<Json<SessionRequest>, axum::extract::rejection::JsonRejection>,
    mount: Mount,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    match state.store.authenticate(&request.token).await {
        Ok(actor_id) => {
            let mut response = Json(SessionResponse { actor_id }).into_response();
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
                    response.headers_mut().insert(header::SET_COOKIE, value);
                    response
                }
                Err(_) => error_response(StoreError::InvalidRequest),
            }
        }
        Err(error) => error_response(error),
    }
}

async fn end_session(State(state): State<AppState>, headers: HeaderMap, mount: Mount) -> Response {
    let result = match cookie_token(&headers) {
        Ok(token) => state.store.revoke(&token).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
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

// Head, pinned revision, save and validation negotiate JSON or MessagePack (see `wire`); the
// representation is settled by the extractors before the store is called.
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
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
    let token = match cookie_token(&headers) {
        Ok(token) => token,
        Err(error) => return error_response(error),
    };
    match state.store.source(&token, &script, &source).await {
        Ok(source) => Json(source).into_response(),
        Err(error) => error_response(error),
    }
}
