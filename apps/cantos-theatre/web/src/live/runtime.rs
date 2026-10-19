//! Leptos/browser shell for the pure listener account owner.
use crate::application::{
    account::AccountGeneration,
    account_view::{
        AccountFailure, AccountViewState, CleanupIntent, LibraryCompletion, LibraryRequest,
        SessionCompletion, SessionPhase,
    },
};
use leptos::prelude::*;
use web_sys::wasm_bindgen::{closure::Closure, JsCast};

async fn cleanup(
    state: RwSignal<AccountViewState>,
    issued: AccountGeneration,
    intent: CleanupIntent,
    session_change_budget: u8,
) {
    let result = super::account::delete_session(&intent).await;
    let session_changed = result == Err(AccountFailure::SessionChanged);
    let mut current_result = false;
    let _ = state.try_update(|current| {
        current_result =
            current.generation() == issued && current.phase() == SessionPhase::LogoutPending;
        current.finish_logout(issued, result);
    });
    if current_result && session_changed && session_change_budget > 0 {
        refresh_with_budget(state, session_change_budget - 1);
    }
}

pub fn refresh(state: RwSignal<AccountViewState>) {
    refresh_with_budget(state, 1);
}

fn refresh_with_budget(state: RwSignal<AccountViewState>, session_change_budget: u8) {
    let current = state.get_untracked();
    if matches!(
        current.phase(),
        SessionPhase::Checking | SessionPhase::LogoutPending
    ) {
        return;
    }
    let mut current = current;
    let Ok(issued) = current.begin_refresh() else {
        return;
    };
    state.set(current);
    leptos::task::spawn_local(async move {
        let result = super::account::session().await;
        finish_lookup(state, issued, result, session_change_budget).await;
    });
}
async fn finish_lookup(
    state: RwSignal<AccountViewState>,
    issued: AccountGeneration,
    result: Result<crate::application::account::AccountScope, AccountFailure>,
    session_change_budget: u8,
) {
    let mut completion = None;
    let mut cleanup_intent = CleanupIntent::StaleOnly;
    if state
        .try_update(|current| {
            completion = Some(current.finish_session(issued, result));
            cleanup_intent = current.cleanup_intent().clone();
        })
        .is_none()
    {
        return;
    }
    match completion {
        Some(Ok(SessionCompletion::Authenticated(_))) => {
            for section in crate::application::library::Section::ALL {
                load_library(state, section, false, session_change_budget, false);
            }
        }
        Some(Ok(SessionCompletion::Cleanup(issued))) => {
            cleanup(state, issued, cleanup_intent, session_change_budget).await
        }
        Some(Ok(SessionCompletion::Ignored)) | Some(Err(_)) | None => (),
    }
}

pub fn load_library(
    state: RwSignal<AccountViewState>,
    section: crate::application::library::Section,
    restart: bool,
    session_change_budget: u8,
    focus: bool,
) {
    let Some(Some(request)) = state.try_update(|s| s.begin_library(section, restart)) else {
        return;
    };
    leptos::task::spawn_local(finish_page(state, request, session_change_budget, focus));
}
async fn finish_page(
    state: RwSignal<AccountViewState>,
    request: LibraryRequest,
    session_change_budget: u8,
    focus: bool,
) {
    let result = super::library::load(&request).await;
    let section = request.page.section;
    let changed = result == Err(AccountFailure::SessionChanged);
    let Some(completion) = state.try_update(|s| s.finish_page(&request, result)) else {
        return;
    };
    match completion {
        Ok(LibraryCompletion::Cleanup(issued)) => {
            let intent = state.with_untracked(|s| s.cleanup_intent().clone());
            cleanup(state, issued, intent, session_change_budget).await;
        }
        Ok(LibraryCompletion::Applied) if changed && session_change_budget > 0 => {
            refresh_with_budget(state, session_change_budget - 1)
        }
        Ok(LibraryCompletion::Applied) if focus => {
            crate::platform::focus_by_id(crate::features::library::live::status_id(section))
        }
        Ok(LibraryCompletion::Applied | LibraryCompletion::Ignored) | Err(_) => (),
    }
}

pub fn logout(state: RwSignal<AccountViewState>) {
    let current = state.get_untracked();
    if current.cleanup_in_flight() {
        return;
    }
    let mut current = current;
    let Ok(issued) = current.begin_logout() else {
        return;
    };
    let intent = current.cleanup_intent().clone();
    state.set(current);
    let expected = match &intent {
        CleanupIntent::ExpectedSession(session) => Some(session.as_str()),
        CleanupIntent::StaleOnly => None,
    };
    if let Ok(client) = crate::api_endpoint::auth() {
        let _ = client.begin_logout(expected);
    }
    leptos::task::spawn_local(cleanup(state, issued, intent, 1));
}

pub fn login(
    state: RwSignal<AccountViewState>,
    configuration: crate::application::registration::RegistrationState,
) {
    if !state.get_untracked().can_login(configuration) {
        return;
    }
    let login = match super::account::reserve_login() {
        Ok(login) => login,
        Err(error) => {
            state.update(|s| s.navigation_failed(error));
            return;
        }
    };
    let mut current = state.get_untracked();
    let Ok(issued) = current.begin_refresh() else {
        return;
    };
    state.set(current);
    leptos::task::spawn_local(async move {
        let result = match login.run().await {
            Ok(()) => super::account::session().await,
            Err(cantos_web_auth::Failure::Cancelled | cantos_web_auth::Failure::Changed) => {
                Err(AccountFailure::SignedOut)
            }
            Err(error) => Err(super::account::auth_error(error)),
        };
        finish_lookup(state, issued, result, 1).await;
    });
}
pub fn cancel_login() {
    if let Ok(client) = crate::api_endpoint::auth() {
        if client.cancel_authorization().is_ok() {
            let _ = client.invalidate();
            leptos::task::spawn_local(async move {
                let _ = client.settled_cleanup().await;
            });
        }
    }
}

fn credentials_lost(state: RwSignal<AccountViewState>) {
    let Some(mut current) = state.try_get_untracked() else {
        return;
    };
    if current.phase() != SessionPhase::SignedIn {
        return;
    }
    let Ok(issued) = current.begin_logout() else {
        return;
    };
    state.set(current);
    leptos::task::spawn_local(async move {
        let result = match crate::api_endpoint::auth() {
            Ok(client) => client
                .settled_cleanup()
                .await
                .map_err(super::account::auth_error),
            Err(error) => Err(super::account::auth_error(error)),
        };
        let _ = state.try_update(|current| {
            current.finish_logout(issued, result);
        });
    });
}
pub fn mount(state: RwSignal<AccountViewState>) {
    use leptos::leptos_dom::helpers::window_event_listener_untyped;

    let credential_loss = window_event_listener_untyped(
        cantos_web_auth::browser::invalidation_event(cantos_web_auth::Surface::Theatre),
        move |_| credentials_lost(state),
    );
    let focus = window_event_listener_untyped("focus", move |_| refresh(state));
    let timer = StoredValue::new_local(None::<(web_sys::Window, i32, Closure<dyn FnMut()>)>);
    if let Some(window) = web_sys::window() {
        let observed_window = window.clone();
        let callback = Closure::wrap(Box::new(move || {
            if observed_window
                .document()
                .is_some_and(|document| document.hidden())
            {
                return;
            }
            if state.get_untracked().phase() == SessionPhase::SignedIn {
                refresh(state);
            }
        }) as Box<dyn FnMut()>);
        if let Ok(handle) = window.set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            60_000,
        ) {
            timer.set_value(Some((window, handle, callback)));
        }
    }
    on_cleanup(move || {
        focus.remove();
        credential_loss.remove();
        timer.update_value(|timer| {
            if let Some((window, handle, callback)) = timer.take() {
                window.clear_interval_with_handle(handle);
                drop(callback);
            }
        });
    });
    refresh(state);
}
