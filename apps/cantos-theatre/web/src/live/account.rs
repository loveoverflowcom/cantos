//! Cantos API BFF transport. No provider token reaches the browser.
use crate::application::account::AccountScope;
pub use crate::application::account_view::AccountFailure as Failure;
use crate::application::account_view::CleanupIntent;
use cantos_contracts::theatre_identity::Session;
use gloo_net::http::Request;
use web_sys::{AbortController, RequestCredentials};
pub async fn session() -> Result<AccountScope, Failure> {
    let controller = AbortController::new().map_err(|_| Failure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| Failure::Unavailable)?;
    let signal = deadline.controller.signal();
    let response = Request::get(
        &crate::api_endpoint::endpoint("/api/v1/theatre/session")
            .map_err(|_| Failure::Unavailable)?,
    )
    .credentials(RequestCredentials::Include)
    .abort_signal(Some(&signal))
    .header("Accept", "application/json")
    .send()
    .await
    .map_err(|_| Failure::Unavailable)?;
    match response.status() {
        401 => return Err(Failure::SignedOut),
        200 => (),
        _ => return Err(Failure::Unavailable),
    }
    if !response.headers().get("content-type").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(Failure::Protocol);
    }
    let body = response.binary().await.map_err(|_| Failure::Unavailable)?;
    if body.len() > 8192 {
        return Err(Failure::Protocol);
    }
    let view: Session = serde_json::from_slice(&body).map_err(|_| Failure::Protocol)?;
    AccountScope::from_session(view.actor_id, view.session_id).map_err(|_| Failure::Protocol)
}
pub fn navigate_login(intent: &str) -> Result<(), Failure> {
    web_sys::window()
        .ok_or(Failure::Navigation)?
        .location()
        .set_href(
            &crate::api_endpoint::endpoint(&format!("/api/v1/theatre/auth/login?intent={intent}"))
                .map_err(|_| Failure::Navigation)?,
        )
        .map_err(|_| Failure::Navigation)
}
pub async fn delete_session(intent: &CleanupIntent) -> Result<(), Failure> {
    let controller = AbortController::new().map_err(|_| Failure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| Failure::Unavailable)?;
    let signal = deadline.controller.signal();
    let request = Request::delete(
        &crate::api_endpoint::endpoint("/api/v1/theatre/session")
            .map_err(|_| Failure::Unavailable)?,
    )
    .credentials(RequestCredentials::Include)
    .abort_signal(Some(&signal));
    let request = match intent {
        CleanupIntent::ExpectedSession(session) => {
            request.header("X-Cantos-Theatre-Session", session)
        }
        CleanupIntent::StaleOnly => request.header("X-Cantos-Theatre-Stale-Only", "1"),
    };
    let response = request.send().await.map_err(|_| Failure::Unavailable)?;
    if response.status() == 204 {
        Ok(())
    } else if response.status() == 409
        && response
            .headers()
            .get("X-Cantos-Theatre-Session")
            .is_some_and(|session| uuid::Uuid::parse_str(&session).is_ok())
    {
        Err(Failure::SessionChanged)
    } else {
        Err(Failure::Unavailable)
    }
}
