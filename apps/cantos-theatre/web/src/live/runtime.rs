//! Leptos/browser shell for the pure listener account owner.
use crate::application::{
    account::AccountGeneration,
    account_view::{
        AccountFailure, AccountViewState, CleanupIntent, LibraryCompletion, LibraryPhase, SessionCompletion, SessionPhase,
    },
};
use leptos::prelude::*;
use web_sys::wasm_bindgen::{closure::Closure, JsCast};

async fn cleanup(state: RwSignal<AccountViewState>, issued: AccountGeneration, intent: CleanupIntent, session_change_budget: u8) {
    let result = super::account::delete_session(&intent).await;
    let session_changed = result == Err(AccountFailure::SessionChanged);
    let mut current_result = false;
    let _ = state.try_update(|current| {
        current_result = current.generation() == issued && current.phase() == SessionPhase::LogoutPending;
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
    if matches!(current.phase(), SessionPhase::Checking | SessionPhase::LogoutPending)
        || current.library_phase() == LibraryPhase::Loading
    {
        return;
    }
    let mut current = current;
    let Ok(issued) = current.begin_refresh() else {
        return;
    };
    state.set(current);
    leptos::task::spawn_local(async move {
        let result = super::account::session().await;
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
            Some(Ok(SessionCompletion::Authenticated(fence))) => {
                let result = super::library::load(fence.session()).await;
                let session_changed = matches!(&result, Err(AccountFailure::SessionChanged));
                let mut completed = None;
                let _ = state.try_update(|current| {
                    completed = Some(current.finish_library(&fence, result));
                    cleanup_intent = current.cleanup_intent().clone();
                });
                match completed {
                    Some(Ok(LibraryCompletion::Cleanup(issued))) => cleanup(state, issued, cleanup_intent, session_change_budget).await,
                    Some(Ok(LibraryCompletion::Applied)) if session_changed && session_change_budget > 0 => {
                        refresh_with_budget(state, session_change_budget - 1);
                    }
                    Some(Ok(LibraryCompletion::Applied | LibraryCompletion::Ignored))
                    | Some(Err(_)) | None => (),
                }
            }
            Some(Ok(SessionCompletion::Cleanup(issued))) => cleanup(state, issued, cleanup_intent, session_change_budget).await,
            Some(Ok(SessionCompletion::Ignored)) | Some(Err(_)) | None => (),
        }
    });
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
    leptos::task::spawn_local(cleanup(state, issued, intent, 1));
}

pub fn login(state: RwSignal<AccountViewState>) {
    if !state.get_untracked().can_login() {
        return;
    }
    if super::account::navigate_login().is_err() {
        state.update(AccountViewState::navigation_failed);
    }
}

pub fn mount(state: RwSignal<AccountViewState>) {
    use leptos::leptos_dom::helpers::window_event_listener_untyped;

    let focus = window_event_listener_untyped("focus", move |_| refresh(state));
    let timer = StoredValue::new_local(None::<(web_sys::Window, i32, Closure<dyn FnMut()>)>);
    if let Some(window) = web_sys::window() {
        let observed_window = window.clone();
        let callback = Closure::wrap(Box::new(move || {
            if observed_window.document().is_some_and(|document| document.hidden()) {
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
        timer.update_value(|timer| {
            if let Some((window, handle, callback)) = timer.take() {
                window.clear_interval_with_handle(handle);
                drop(callback);
            }
        });
    });
    refresh(state);
}
