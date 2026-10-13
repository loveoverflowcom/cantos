//! Opt-in same-origin BFF port; the existing mock runtime is not silently upgraded.
use crate::application::account::{AccountScope, AccountState};
use cantos_contracts::theatre_identity::Session;
use gloo_net::http::Request;
use std::{cell::RefCell, rc::Rc};
use web_sys::{AbortController, RequestCredentials};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    SignedOut,
    Unavailable,
    Protocol,
    Navigation,
    LogoutPending,
}
async fn session() -> Result<AccountScope, Failure> {
    let controller = AbortController::new().map_err(|_| Failure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| Failure::Unavailable)?;
    let signal = deadline.controller.signal();
    let response = Request::get("/api/v1/theatre/session")
        .credentials(RequestCredentials::SameOrigin)
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
            .is_some_and(|value| value.trim() == "application/json")
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
/// Both success and failure are fenced. No mutable signal/state borrow survives network I/O.
pub async fn refresh(owner: Rc<RefCell<AccountState>>) -> Result<bool, Failure> {
    if owner.borrow().logout_pending() {
        return Err(Failure::LogoutPending);
    }
    let issued = owner.borrow().generation();
    let result = session().await;
    let mut current = owner.borrow_mut();
    if current.generation() != issued {
        return Ok(false);
    }
    match result {
        Ok(scope) => current
            .finish_session(issued, scope)
            .map_err(|_| Failure::Protocol),
        Err(Failure::SignedOut) => {
            current.leave().map_err(|_| Failure::Protocol)?;
            Err(Failure::SignedOut)
        }
        Err(error) => Err(error),
    }
}
pub fn begin_login(owner: &mut AccountState) -> Result<(), Failure> {
    if owner.logout_pending() {
        return Err(Failure::LogoutPending);
    }
    owner.leave().map_err(|_| Failure::Protocol)?;
    web_sys::window()
        .ok_or(Failure::Navigation)?
        .location()
        .set_href("/api/v1/theatre/auth/login")
        .map_err(|_| Failure::Navigation)
}
pub async fn logout(owner: Rc<RefCell<AccountState>>) -> Result<(), Failure> {
    let issued = owner
        .borrow_mut()
        .begin_logout()
        .map_err(|_| Failure::Protocol)?;
    let controller = AbortController::new().map_err(|_| Failure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| Failure::Unavailable)?;
    let signal = deadline.controller.signal();
    let response = Request::delete("/api/v1/theatre/session")
        .credentials(RequestCredentials::SameOrigin)
        .abort_signal(Some(&signal))
        .send()
        .await
        .map_err(|_| Failure::Unavailable)?;
    if response.status() == 204 {
        owner.borrow_mut().finish_logout(issued);
        Ok(())
    } else {
        Err(Failure::Unavailable)
    }
}
