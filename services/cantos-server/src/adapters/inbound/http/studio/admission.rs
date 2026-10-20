//! Studio credential selection, request fences and admission-lease lifetime.
use super::super::{
    bearer,
    errors::{error_response, failure},
    AppState,
};
use crate::application::{Store, StoreError};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use cantos_contracts::ErrorCode;

pub(in crate::adapters::inbound::http) fn cookie_token(
    headers: &HeaderMap,
    store: &Store,
) -> Result<String, StoreError> {
    if headers.contains_key(header::AUTHORIZATION) {
        return bearer::access(headers)
            .map(str::to_owned)
            .map_err(|_| StoreError::Unauthenticated);
    }
    let name = store
        .studio_oidc()
        .map_or("cantos_session", |oidc| oidc.session_name());
    let token = cookie_token_for_name(headers, name)?;
    if token.starts_with("ca1_") {
        return Err(StoreError::Unauthenticated);
    }
    Ok(token)
}

pub(in crate::adapters::inbound::http) fn cookie_token_for_name(
    headers: &HeaderMap,
    expected: &str,
) -> Result<String, StoreError> {
    let mut found = None;
    for name in ["cantos_session", "__Secure-cantos_studio"] {
        let value =
            super::auth::unique_cookie(headers, name).map_err(|_| StoreError::Unauthenticated)?;
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
    if request.headers().contains_key(header::AUTHORIZATION) {
        return bearer_studio(state, request, next).await;
    }
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

async fn bearer_studio(state: AppState, request: Request, next: Next) -> Response {
    if !bearer::browser_origin_allowed(request.headers(), &state.web_origin) {
        return failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN);
    }
    let token = match bearer::access(request.headers()) {
        Ok(v) => v.to_owned(),
        Err(e) => return bearer::error(e),
    };
    if crate::platform::bearer::credential::access_client(&token)
        != Some(crate::platform::bearer::client::Client::Studio)
    {
        return bearer::error(crate::platform::studio_oidc::AuthError::InvalidToken);
    }
    let Some(service) = state
        .store
        .bearer(crate::platform::bearer::client::Client::Studio)
    else {
        return bearer::error(crate::platform::studio_oidc::AuthError::Configuration);
    };
    if let Err(e) = service.authenticate(&state.store, &token).await {
        return bearer::error(e);
    }
    match service
        .with_admission(&state.store, &token, |session| async move {
            if !studio_query_fence_matches(request.uri(), Some(&session.session_id))
                || !studio_fence_matches(request.headers(), Some(&session.session_id))
            {
                return studio_fenced_response(
                    failure(ErrorCode::Unauthenticated, StatusCode::CONFLICT),
                    Some(&session.session_id),
                );
            }
            if bearer::sensitive(request.method(), request.uri().path())
                && !bearer::recent(&session)
            {
                return studio_fenced_response(
                    failure(ErrorCode::Forbidden, StatusCode::FORBIDDEN),
                    Some(&session.session_id),
                );
            }
            studio_fenced_response(next.run(request).await, Some(&session.session_id))
        })
        .await
    {
        Ok(v) => v,
        Err(e) => bearer::error(e),
    }
}

pub(in crate::adapters::inbound::http) const STUDIO_SESSION_HEADER: &str =
    "x-cantos-studio-session";

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
