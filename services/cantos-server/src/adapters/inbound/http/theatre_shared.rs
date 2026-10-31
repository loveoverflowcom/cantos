//! Authenticated Theatre reads. No account/scope is taken from query or request JSON.
use super::{
    audio_transport,
    identity_auth::{theatre_credential, TheatreCredential},
    theatre, AppState,
};
use crate::contexts::media::{ListenerError, ListenerService};
use axum::{
    body::{Body, Bytes},
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, Method},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/shared/works", get(works))
        .route("/shared/search", get(works))
        .route("/shared/works/{work}", get(work))
        .route("/shared/works/{work}/units", get(units))
        .route("/shared/publications/{publication}", get(manifest))
        .route(
            "/shared/publications/{publication}/playback-authorizations",
            post(playback),
        )
        .route("/shared/media/{grant}", get(media))
}
fn credential(state: &AppState, headers: &HeaderMap) -> Result<String, ListenerError> {
    match theatre_credential(state, headers) {
        TheatreCredential::Present(_, token) => Ok(token),
        TheatreCredential::Absent | TheatreCredential::Refused => Err(ListenerError::NotFound),
    }
}
fn response<T: serde::Serialize>(result: Result<T, ListenerError>) -> Response {
    audio_transport::private_response(match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => theatre::error(error),
    })
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkQuery {
    q: Option<String>,
    kind: Option<String>,
    language: Option<String>,
    limit: Option<u16>,
    cursor: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitQuery {
    limit: Option<u16>,
    cursor: Option<String>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyQuery {}
async fn works(
    State(state): State<AppState>,
    Extension(service): Extension<ListenerService>,
    headers: HeaderMap,
    query: Result<Query<WorkQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    response(
        async {
            let token = credential(&state, &headers)?;
            let Query(q) = query.map_err(|_| ListenerError::InvalidRequest)?;
            service
                .shared_works(
                    &token,
                    q.q.as_deref().unwrap_or(""),
                    q.kind.as_deref(),
                    q.language.as_deref(),
                    q.limit.unwrap_or(20),
                    q.cursor.as_deref(),
                )
                .await
        }
        .await,
    )
}
async fn work(
    State(state): State<AppState>,
    Extension(service): Extension<ListenerService>,
    headers: HeaderMap,
    Path(work): Path<String>,
    query: Result<Query<EmptyQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    response(
        async {
            let token = credential(&state, &headers)?;
            query.map_err(|_| ListenerError::InvalidRequest)?;
            service.shared_work(&token, &work).await
        }
        .await,
    )
}
async fn units(
    State(state): State<AppState>,
    Extension(service): Extension<ListenerService>,
    headers: HeaderMap,
    Path(work): Path<String>,
    query: Result<Query<UnitQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    response(
        async {
            let token = credential(&state, &headers)?;
            let Query(q) = query.map_err(|_| ListenerError::InvalidRequest)?;
            service
                .shared_units(&token, &work, q.limit.unwrap_or(20), q.cursor.as_deref())
                .await
        }
        .await,
    )
}
async fn manifest(
    State(state): State<AppState>,
    Extension(service): Extension<ListenerService>,
    headers: HeaderMap,
    Path(publication): Path<String>,
) -> Response {
    response(
        async {
            let token = credential(&state, &headers)?;
            service.shared_manifest(&token, &publication).await
        }
        .await,
    )
}
async fn playback(
    State(state): State<AppState>,
    Extension(service): Extension<ListenerService>,
    headers: HeaderMap,
    Path(publication): Path<String>,
    body: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    response(
        async {
            let token = credential(&state, &headers)?;
            let body = body.map_err(|_| ListenerError::InvalidRequest)?;
            if !body.is_empty() && body.as_ref() != b"{}" {
                return Err(ListenerError::InvalidRequest);
            }
            service
                .shared_authorize_playback(&token, &publication)
                .await
        }
        .await,
    )
}
async fn media(
    State(state): State<AppState>,
    Extension(service): Extension<ListenerService>,
    headers: HeaderMap,
    method: Method,
    grant: Result<Path<String>, axum::extract::rejection::PathRejection>,
) -> Response {
    let mut result = match (credential(&state, &headers), grant) {
        (Ok(token), Ok(Path(grant))) => {
            theatre::media_response_in(&service, &grant, &method, &headers, Some(&token)).await
        }
        _ => theatre::error(ListenerError::NotFound),
    };
    if method == Method::HEAD {
        *result.body_mut() = Body::empty();
    }
    audio_transport::private_response(result)
}
/// The only new originless POST is still authenticated by the Theatre middleware. Its cookie
/// branch independently enforces exact Origin; the anonymous-public exception never matches it.
pub(super) fn bearer_mutation(method: &Method, path: &str) -> bool {
    method == Method::POST
        && path
            .strip_prefix("/api/v1/theatre/shared/publications/")
            .and_then(|v| v.strip_suffix("/playback-authorizations"))
            .is_some_and(|id| !id.is_empty() && !id.contains('/'))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_post_never_inherits_the_public_post_exception() {
        let private = "/api/v1/theatre/shared/publications/id/playback-authorizations";
        assert!(bearer_mutation(&Method::POST, private));
        assert!(!theatre::public_playback_post(&Method::POST, private));
        for path in [
            "/api/v1/theatre/shared/works/id",
            "/api/v1/theatre/shared/publications/a/b/playback-authorizations",
            "/api/v1/theatre/publications/id/playback-authorizations",
        ] {
            assert!(!bearer_mutation(&Method::POST, path));
        }
        assert!(!bearer_mutation(&Method::GET, private));
    }
}
