//! Anonymous Theatre onboarding. No session, account ID or provider authority comes from JSON.
use super::{failure, AppState};
use crate::platform::theatre_registration::{
    application::VerificationOutcome,
    domain::{Email, RegistrationError},
    runtime::RegistrationRuntime,
};
use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Extension, Json, Router,
};
use cantos_contracts::{theatre_registration as dto, ErrorCode};
use std::net::SocketAddr;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/registration", post(start))
        .route("/auth/registration/verify", post(verify))
        .route("/auth/registration/cancel", post(cancel))
        .layer(DefaultBodyLimit::max(2048))
}

fn accepted() -> Response {
    (
        StatusCode::ACCEPTED,
        Json(dto::Accepted {
            status: dto::AcceptedStatus::Accepted,
        }),
    )
        .into_response()
}

fn origin_allowed(headers: &HeaderMap, runtime: &RegistrationRuntime, web_origin: &str) -> bool {
    runtime.web_origin() == web_origin
        && headers
            .get(header::ORIGIN)
            .is_some_and(|value| value.as_bytes() == web_origin.as_bytes())
        && !headers
            .get("sec-fetch-site")
            .is_some_and(|value| value == "cross-site")
}

async fn start(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Json<dto::RegistrationRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(runtime) = state.store.theatre_registration() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if !origin_allowed(&headers, runtime, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    if Email::parse(&body.email).is_err() {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    }
    // Only the socket peer is trusted. Forwarded headers never choose a quota identity.
    let Some(Extension(ConnectInfo(peer))) = peer else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(network) = runtime.network_key(peer.ip()) else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    if !runtime.admit(network) {
        return accepted();
    }
    let Some(permit) = runtime.acquire() else {
        return accepted();
    };
    match runtime.service().prepare(&body.email, network).await {
        Ok(Some(mail)) => {
            let service = runtime.service().clone();
            // Persisted challenge/quota precede dispatch. Network latency never reveals whether
            // an email was suppressed; crash recovery requests rotate proof after cooldown.
            tokio::spawn(async move {
                let _permit = permit;
                let _ = service.send_verification(mail).await;
            });
            accepted()
        }
        Ok(None) => accepted(),
        Err(RegistrationError::InvalidEmail | RegistrationError::InvalidToken) => {
            failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST)
        }
        Err(_) => failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn verify(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Json<dto::TokenRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(runtime) = state.store.theatre_registration() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if !origin_allowed(&headers, runtime, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let Some(Extension(ConnectInfo(peer))) = peer else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(network) = runtime.network_key(peer.ip()) else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    if !runtime.admit(network) {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    }

    let Some(_permit) = runtime.acquire() else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    let status = match runtime.service().verify(&body.token).await {
        Ok(VerificationOutcome::SetupSent) => dto::VerificationStatus::SetupLinkSent,
        Ok(VerificationOutcome::Unavailable) => dto::VerificationStatus::Unavailable,
        Err(_) => return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    };
    Json(dto::VerificationResponse { status }).into_response()
}

async fn cancel(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Json<dto::TokenRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(runtime) = state.store.theatre_registration() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if !origin_allowed(&headers, runtime, &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let Some(Extension(ConnectInfo(peer))) = peer else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(network) = runtime.network_key(peer.ip()) else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    if !runtime.admit(network) {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    }

    let Some(_permit) = runtime.acquire() else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    match runtime.service().cancel(&body.token).await {
        Ok(()) => accepted(),
        Err(_) => failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[cfg(test)]
mod tests;
