use axum::{
    Extension, Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query, Request, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::{self, Actor, AuthError},
    editorial::{AcceptRequest, CreateRequest, EditError, SaveRequest},
    store::{self, StoreError},
};

pub fn router(pool: PgPool) -> Router {
    let private = Router::new()
        .route("/v1/scripts", post(create))
        .route("/v1/scripts/{id}", get(read))
        .route("/v1/scripts/{id}/versions", post(save))
        .route("/v1/scripts/{id}/versions/{version}", get(read_version))
        .route("/v1/scripts/{id}/revisions", post(accept))
        .route("/v1/scripts/{id}/revisions/{revision}", get(read_revision))
        .route("/v1/scripts/{id}/history", get(history))
        .route_layer(middleware::from_fn_with_state(pool.clone(), authenticate));
    Router::new()
        .merge(private)
        .route("/healthz", get(health))
        .fallback(|| async { ApiError::code(StatusCode::NOT_FOUND, "not_found") })
        .method_not_allowed_fallback(|| async {
            ApiError::code(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed")
        })
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn(no_store))
        .with_state(pool)
}

async fn no_store(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn authenticate(
    State(pool): State<PgPool>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let mut headers = request.headers().get_all(header::AUTHORIZATION).iter();
    let value = headers.next().and_then(|value| value.to_str().ok());
    if headers.next().is_some() {
        return Err(AuthError::Unauthenticated.into());
    }
    let actor = auth::authenticate(&pool, value).await?;
    request.extensions_mut().insert(actor);
    Ok(next.run(request).await)
}

async fn health(State(pool): State<PgPool>) -> Response {
    match sqlx::query("SELECT 1").execute(&pool).await {
        Ok(_) => (StatusCode::OK, Json(json!({"status":"ok"}))).into_response(),
        Err(_) => {
            ApiError::code(StatusCode::SERVICE_UNAVAILABLE, "storage_unavailable").into_response()
        }
    }
}

struct ApiError {
    status: StatusCode,
    detail: Value,
}
impl ApiError {
    fn code(status: StatusCode, code: &str) -> Self {
        Self {
            status,
            detail: json!({"code":code}),
        }
    }
    fn invalid() -> Self {
        Self::code(StatusCode::BAD_REQUEST, "invalid_request")
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(json!({"error":self.detail}))).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}
impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Unauthenticated => Self::code(StatusCode::UNAUTHORIZED, "unauthenticated"),
            AuthError::Storage(_) => Self::code(StatusCode::INTERNAL_SERVER_ERROR, "storage_error"),
        }
    }
}
impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => Self::code(StatusCode::NOT_FOUND, "not_found"),
            StoreError::Database(_) | StoreError::Edit(EditError::Corrupt) => {
                Self::code(StatusCode::INTERNAL_SERVER_ERROR, "storage_error")
            }
            StoreError::Edit(EditError::InvalidRequest) => Self::invalid(),
            StoreError::Edit(EditError::IdentityChanged) => {
                Self::code(StatusCode::UNPROCESSABLE_ENTITY, "identity_changed")
            }
            StoreError::Edit(EditError::OperationReused) => {
                Self::code(StatusCode::CONFLICT, "operation_reused")
            }
            StoreError::Edit(EditError::Conflict { head_version }) => Self {
                status: StatusCode::CONFLICT,
                detail: json!({"code":"conflict", "head_version":head_version}),
            },
            StoreError::Edit(EditError::InvalidScript(report)) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                detail: json!({"code":"invalid_script", "issues":report.issues}),
            },
        }
    }
}

fn body<T>(result: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    result.map(|Json(value)| value).map_err(|rejection| {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError::code(StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large")
        } else {
            ApiError::invalid()
        }
    })
}
fn path<T>(result: Result<Path<T>, PathRejection>) -> Result<T, ApiError> {
    result
        .map(|Path(value)| value)
        .map_err(|_| ApiError::invalid())
}

async fn create(
    State(pool): State<PgPool>,
    Extension(actor): Extension<Actor>,
    request: Result<Json<CreateRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(store::create(&pool, &actor, body(request)?).await?),
    ))
}
async fn read(
    State(pool): State<PgPool>,
    Extension(actor): Extension<Actor>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(store::get(&pool, &actor, path(id)?).await?))
}
async fn save(
    State(pool): State<PgPool>,
    Extension(actor): Extension<Actor>,
    id: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<SaveRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        store::save(&pool, &actor, path(id)?, body(request)?).await?,
    ))
}
async fn read_version(
    State(pool): State<PgPool>,
    Extension(actor): Extension<Actor>,
    ids: Result<Path<(Uuid, i64)>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let (id, version) = path(ids)?;
    Ok(Json(store::get_version(&pool, &actor, id, version).await?))
}
async fn accept(
    State(pool): State<PgPool>,
    Extension(actor): Extension<Actor>,
    id: Result<Path<Uuid>, PathRejection>,
    request: Result<Json<AcceptRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        store::accept(&pool, &actor, path(id)?, body(request)?).await?,
    ))
}
async fn read_revision(
    State(pool): State<PgPool>,
    Extension(actor): Extension<Actor>,
    ids: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let (script, revision) = path(ids)?;
    Ok(Json(
        store::get_revision(&pool, &actor, script, revision).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    #[serde(default)]
    after: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 {
    50
}
async fn history(
    State(pool): State<PgPool>,
    Extension(actor): Extension<Actor>,
    id: Result<Path<Uuid>, PathRejection>,
    query: Result<Query<Page>, QueryRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Query(page) = query.map_err(|_| ApiError::invalid())?;
    Ok(Json(
        store::history(&pool, &actor, path(id)?, page.after, page.limit).await?,
    ))
}
