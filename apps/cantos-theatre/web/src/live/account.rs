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
    scope(view)
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
