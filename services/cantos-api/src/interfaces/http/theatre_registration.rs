//! Anonymous Theatre onboarding. No session, account ID or provider authority comes from JSON.
use super::{failure, AppState};
use crate::platform::theatre_registration::{
    application::VerificationOutcome,
    domain::{Email, RegistrationError},
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
        .route(
            "/auth/registration/firebase/confirm",
            post(firebase_confirm),
        )
        .route("/auth/registration/firebase/cancel", post(firebase_cancel))
        .layer(DefaultBodyLimit::max(8192))
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

fn same_origin(headers: &HeaderMap, configured: &str, web_origin: &str) -> bool {
    configured == web_origin
        && headers.get_all(header::ORIGIN).iter().count() == 1
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
    if state.store.theatre_firebase_enrollment().is_some() {
        return firebase_start(state, peer, headers, body).await;
    }
    let Some(runtime) = state.store.theatre_registration() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if !same_origin(&headers, runtime.web_origin(), &state.web_origin) {
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
    if !same_origin(&headers, runtime.web_origin(), &state.web_origin) {
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
    if !same_origin(&headers, runtime.web_origin(), &state.web_origin) {
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

async fn firebase_start(
    state: AppState,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Json<dto::RegistrationRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(runtime) = state.store.theatre_firebase_enrollment() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if !same_origin(&headers, runtime.web_origin(), &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    if Email::parse(&body.email).is_err() {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    }
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
        Ok(Some(work)) => {
            let service = runtime.service().clone();
            tokio::spawn(async move {
                let _permit = permit;
                let _ = service.dispatch(work).await;
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

async fn firebase_confirm(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Json<dto::FirebaseConfirmRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    use crate::platform::theatre_registration::firebase_enrollment::application::ConfirmationOutcome;
    let Some(runtime) = state.store.theatre_firebase_enrollment() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if !same_origin(&headers, runtime.web_origin(), &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let Ok(operation) = uuid::Uuid::parse_str(&body.operation_id) else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    if operation.is_nil() {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    }
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
    let outcome = match runtime.service().confirm(operation, &body.oob_code).await {
        Ok(ConfirmationOutcome::SetupReady(setup)) => dto::FirebaseConfirmation::SetupReady {
            login: setup.login().to_owned(),
            setup_url: setup.link().as_str().to_owned(),
        },
        Ok(ConfirmationOutcome::Unavailable) => dto::FirebaseConfirmation::Unavailable {},
        Err(_) => return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    };
    let mut response = Json(outcome).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        axum::http::HeaderValue::from_static("no-referrer"),
    );
    response
}

async fn firebase_cancel(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Json<dto::FirebaseConfirmRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Some(runtime) = state.store.theatre_firebase_enrollment() else {
        return failure(ErrorCode::NotFound, StatusCode::NOT_FOUND);
    };
    if !same_origin(&headers, runtime.web_origin(), &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let Ok(Json(body)) = body else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    let Ok(operation) = uuid::Uuid::parse_str(&body.operation_id) else {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    };
    if operation.is_nil() {
        return failure(ErrorCode::InvalidRequest, StatusCode::BAD_REQUEST);
    }
    let Some(Extension(ConnectInfo(peer))) = peer else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    let Ok(network) = runtime.network_key(peer.ip()) else {
        return failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE);
    };
    if !runtime.admit(network) {
        return accepted();
    }
    let Some(_permit) = runtime.acquire() else {
        return accepted();
    };
    match runtime.service().cancel(operation, &body.oob_code).await {
        Ok(()) => accepted(),
        Err(_) => failure(ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[cfg(test)]
mod firebase_tests;
#[cfg(test)]
mod tests;
