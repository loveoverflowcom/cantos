//! Theatre-scoped Cantos Bearer account admission; no cookie fallback.
use crate::application::account::AccountScope;
pub use crate::application::account_view::AccountFailure as Failure;
use crate::application::account_view::CleanupIntent;
use cantos_contracts::bearer::Session;
use gloo_net::http::Request;
use web_sys::{AbortController, RequestCredentials};
pub(crate) fn auth_error(error: cantos_web_auth::Failure) -> Failure {
    match error {
        cantos_web_auth::Failure::SignedOut => Failure::SignedOut,
        cantos_web_auth::Failure::Changed | cantos_web_auth::Failure::AccountMismatch => {
            Failure::SessionChanged
        }
        cantos_web_auth::Failure::Protocol => Failure::Protocol,
        cantos_web_auth::Failure::Cancelled | cantos_web_auth::Failure::PopupBlocked => {
            Failure::Navigation
        }
        cantos_web_auth::Failure::Unavailable => Failure::Unavailable,
        cantos_web_auth::Failure::CleanupPending => Failure::SignedOut,
    }
}
pub async fn session() -> Result<AccountScope, Failure> {
    let credential = crate::api_endpoint::auth()
        .map_err(auth_error)?
        .access(None)
        .await
        .map_err(auth_error)?;
    let controller = AbortController::new().map_err(|_| Failure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| Failure::Unavailable)?;
    let signal = deadline.controller.signal();
    let response = Request::get(
        &crate::api_endpoint::endpoint("/api/v1/theatre/auth/bearer/session")
            .map_err(|_| Failure::Unavailable)?,
    )
    .credentials(RequestCredentials::Omit)
    .redirect(web_sys::RequestRedirect::Error)
    .abort_signal(Some(&signal))
    .header("Authorization", &credential.header())
    .header("Accept", "application/json")
    .send()
    .await
    .map_err(|_| Failure::Unavailable)?;
    credential.response(response.status()).map_err(auth_error)?;
    if response.status() != 200 {
        return Err(Failure::Unavailable);
    }
    if !response.headers().get("content-type").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(Failure::Protocol);
    }
    let body = response.binary().await.map_err(|_| Failure::Unavailable)?;
    if body.len() > 8192 {
        return Err(Failure::Protocol);
    }
    let view: Session = serde_json::from_slice(&body).map_err(|_| Failure::Protocol)?;
    credential.check().map_err(auth_error)?;
    if view.actor_id != credential.actor() || view.session_id != credential.session() {
        return Err(Failure::SessionChanged);
    }
    AccountScope::from_session(view.actor_id, view.session_id).map_err(|_| Failure::Protocol)
}
pub fn reserve_login() -> Result<cantos_web_auth::browser::Login, Failure> {
    crate::api_endpoint::auth()
        .map_err(auth_error)?
        .reserve_login(None, false)
        .map_err(auth_error)
}
pub async fn delete_session(intent: &CleanupIntent) -> Result<(), Failure> {
    let client = crate::api_endpoint::auth().map_err(auth_error)?;
    let expected = match intent {
        CleanupIntent::ExpectedSession(session) => Some(session.as_str()),
        CleanupIntent::StaleOnly => None,
    };
    client.logout(expected).await.map_err(auth_error)
}
