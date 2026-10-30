//! Theatre-scoped Cantos Identity v1 account admission: an API-issued HttpOnly cookie, never a
//! script-readable credential and no Bearer/provider fallback.
use crate::application::account::AccountScope;
pub use crate::application::account_view::AccountFailure as Failure;
use crate::application::account_view::CleanupIntent;

pub(crate) use super::failure::{auth_error, login_error};

fn scope(view: cantos_web_auth::core::SessionView) -> Result<AccountScope, Failure> {
    AccountScope::from_session(view.actor, view.session).map_err(|_| Failure::Protocol)
}

/// The authoritative lookup: the API decides who this browser is. An empty page state, a reload
/// or a cookie from another tab never decides signed-out on its own; only the API's 401 does.
/// A due idle renewal runs here, so the existing refresh lifecycle keeps a live session alive.
pub async fn session() -> Result<AccountScope, Failure> {
    let client = crate::api_endpoint::auth().map_err(auth_error)?;
    let Some(view) = client.read_session().await.map_err(auth_error)? else {
        return Err(Failure::SignedOut);
    };
    client
        .access(Some(&view.session))
        .await
        .map_err(auth_error)?;
    let scope = scope(view)?;
    // A display outage never logs the listener out. Session authority still belongs to the
    // existing auth owner; its lease refuses a cookie change before this answer is published.
    match presentation(scope.session()).await {
        Ok(presentation) => Ok(scope.with_presentation(presentation)),
        Err(Failure::SignedOut | Failure::SessionChanged) => Err(Failure::SessionChanged),
        Err(_) => Ok(scope),
    }
}

async fn presentation(
    expected: &str,
) -> Result<crate::contract::account_presentation::AccountPresentation, Failure> {
    use gloo_net::http::Request;
    let credential = crate::api_endpoint::auth()
        .map_err(auth_error)?
        .access(Some(expected))
        .await
        .map_err(auth_error)?;
    let controller = web_sys::AbortController::new().map_err(|_| Failure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| Failure::Unavailable)?;
    let url = crate::api_endpoint::endpoint("/api/v1/theatre/account/presentation")
        .map_err(|_| Failure::Unavailable)?;
    let response = credential
        .apply(Request::get(&url))
        .header("Accept", "application/json")
        .abort_signal(Some(&deadline.controller.signal()))
        .send()
        .await
        .map_err(|_| Failure::Unavailable)?;
    credential.receive(&response).map_err(auth_error)?;
    super::library::check_response(
        response.status(),
        response.headers().get("content-type").as_deref(),
        response
            .headers()
            .get("x-cantos-theatre-session")
            .as_deref(),
        expected,
    )?;
    let bytes = response.binary().await.map_err(|_| Failure::Unavailable)?;
    credential.check().map_err(auth_error)?;
    if bytes.len() > 4096 {
        return Err(Failure::Protocol);
    }
    let view: crate::contract::account_presentation::AccountPresentation =
        serde_json::from_slice(&bytes).map_err(|_| Failure::Protocol)?;
    if !view.valid() {
        return Err(Failure::Protocol);
    }
    Ok(view)
}

/// One email/password attempt. The reply is confirmed by an authoritative read, so a browser that
/// refuses to retain the cookie reports a fault instead of a session that cannot work.
pub async fn login(email: String, password: String) -> Result<AccountScope, Failure> {
    let client = crate::api_endpoint::auth().map_err(auth_error)?;
    client
        .login(email, password, None)
        .await
        .map_err(login_error)?;
    match session().await {
        Err(Failure::SignedOut) => Err(Failure::Diagnostic(
            crate::application::errors::ErrorKind::Configuration,
        )),
        other => other,
    }
}

pub async fn delete_session(intent: &CleanupIntent) -> Result<(), Failure> {
    let client = crate::api_endpoint::auth().map_err(auth_error)?;
    let expected = match intent {
        CleanupIntent::ExpectedSession(session) => Some(session.as_str()),
        CleanupIntent::StaleOnly => None,
    };
    client.logout(expected).await.map_err(auth_error)
}
