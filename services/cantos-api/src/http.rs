//! Thin Axum adapter; all revision decisions live inward of this module.
use axum::{
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts::{
    wire::WireFormat, ApiError, ErrorCode, ReviewRequest, SaveRevisionRequest,
    ScriptValidationResponse, SessionRequest, SessionResponse, ValidateScriptRequest,
};
use tower_http::services::ServeDir;

use crate::diagnostics::validation_issues;
use crate::postgres::{Store, StoreError};
use wire::{respond, Accepts, WireBody};

mod adaptations;
mod imports;
mod production;
mod wire;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub web_origin: String,
}

pub fn router(state: AppState, dist: &str) -> Router {
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
        .merge(production::routes())
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticated));
    let api = scripts
        .route("/session", post(login).delete(logout))
        .fallback(|| async { failure(ErrorCode::NotFound, StatusCode::NOT_FOUND) })
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            origin_and_cache,
        ));
    Router::new()
        .nest("/api/v1", api)
        .fallback_service(ServeDir::new(dist))
        .with_state(state)
}

fn cookie_token(headers: &HeaderMap) -> Result<String, StoreError> {
    let cookies = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .ok_or(StoreError::Unauthenticated)?;
    let mut tokens = cookies
        .split(';')
        .filter_map(|part| part.trim().strip_prefix("cantos_session="));
    let token = tokens.next().ok_or(StoreError::Unauthenticated)?;
    if tokens.next().is_some() {
        return Err(StoreError::Unauthenticated);
    }
    Ok(token.to_owned())
}

async fn authenticated(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let result = match cookie_token(request.headers()) {
        Ok(token) => state.store.authenticate(&token).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(_) => next.run(request).await,
        Err(error) => error_response(error),
    }
}

async fn origin_and_cache(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let read =
        request.method() == axum::http::Method::GET || request.method() == axum::http::Method::HEAD;
    let valid_origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
        == Some(state.web_origin.as_str());
    let mut response = if !read && !valid_origin {
        failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN)
    } else {
        match tokio::time::timeout(std::time::Duration::from_secs(15), next.run(request)).await {
            Ok(response) => response,
            Err(_) => failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
        }
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn login(
    State(state): State<AppState>,
    body: Result<Json<SessionRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let request = match body {
        Ok(Json(request)) => request,
        Err(_) => return error_response(StoreError::InvalidRequest),
    };
    match state.store.authenticate(&request.token).await {
        Ok(actor_id) => {
            let mut response = Json(SessionResponse { actor_id }).into_response();
            // Only loopback development HTTP is supported. Production requires HTTPS/Secure.
            let cookie = format!(
                "cantos_session={}; HttpOnly; SameSite=Strict; Path=/api/v1",
                request.token
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

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let result = match cookie_token(&headers) {
        Ok(token) => state.store.revoke(&token).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            response.headers_mut().insert(
                header::SET_COOKIE,
                HeaderValue::from_static(
                    "cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1; Max-Age=0",
                ),
            );
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

fn failure(code: ErrorCode, status: StatusCode) -> Response {
    (
        status,
        Json(ApiError {
            code,
            current_revision: None,
            issues: vec![],
        }),
    )
        .into_response()
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

fn error_response(error: StoreError) -> Response {
    let (status, code, current_revision, issues) = match error {
        StoreError::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthenticated,
            None,
            vec![],
        ),
        StoreError::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound, None, vec![]),
        StoreError::Forbidden => (StatusCode::FORBIDDEN, ErrorCode::Forbidden, None, vec![]),
        StoreError::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            vec![],
        ),
        StoreError::InvalidRequestFields(issues) => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            issues,
        ),
        StoreError::EvidenceUnavailable => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::EvidenceUnavailable,
            None,
            vec![],
        ),
        StoreError::StaleRevision(revision) => (
            StatusCode::CONFLICT,
            ErrorCode::StaleRevision,
            Some(revision),
            vec![],
        ),
        StoreError::OperationReused => (
            StatusCode::CONFLICT,
            ErrorCode::OperationReused,
            None,
            vec![],
        ),
        StoreError::ProposalAlreadySubmitted => (
            StatusCode::CONFLICT,
            ErrorCode::ProposalAlreadySubmitted,
            None,
            vec![],
        ),
        StoreError::StaleProductionInputs => (
            StatusCode::CONFLICT,
            ErrorCode::StaleProductionInputs,
            None,
            vec![],
        ),
        StoreError::ProductionBlocked(issues) => (
            StatusCode::CONFLICT,
            ErrorCode::ProductionBlocked,
            None,
            issues,
        ),
        StoreError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            None,
            vec![],
        ),
        StoreError::CorruptRevision => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
            None,
            vec![],
        ),
        StoreError::InvalidScript(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidScript,
            None,
            validation_issues(error),
        ),
    };
    (
        status,
        Json(ApiError {
            code,
            current_revision,
            issues,
        }),
    )
        .into_response()
}
