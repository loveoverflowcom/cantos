//! Fallible browser hash navigation only. No storage, auth, media element or network adapter.
use crate::app::routes::Route;
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
pub fn bind_navigation(route: RwSignal<Route>, main: NodeRef<leptos::html::Main>) {
    use leptos::{leptos_dom::helpers::window_event_listener_untyped, web_sys};
    let read_route = move || {
        web_sys::window()
            .and_then(|window| window.location().hash().ok())
            .map(|hash| Route::from_hash(&hash))
            .unwrap_or(Route::Discover)
    };
    route.set(read_route());
    let listener = window_event_listener_untyped("hashchange", move |_| {
        route.set(read_route());
        if let Some(element) = main.get_untracked() {
            let _ = element.focus();
        }
    });
    on_cleanup(move || listener.remove());
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bind_navigation(_route: RwSignal<Route>, _main: NodeRef<leptos::html::Main>) {}

pub fn navigate(route: RwSignal<Route>, destination: Route) {
    #[cfg(target_arch = "wasm32")]
    {
        // Hash navigation produces browser history entries and native Back/Forward behavior.
        if leptos::web_sys::window()
            .is_some_and(|window| window.location().set_hash(&destination.hash()).is_ok())
        {
            return;
        }
    }
    // Native rendering and a missing browser get an explicit in-memory fallback.
    route.set(destination);
}

#[cfg(target_arch = "wasm32")]
pub fn focus_main(main: NodeRef<leptos::html::Main>) {
    if let Some(element) = main.get_untracked() {
        let _ = element.focus();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn focus_main(_main: NodeRef<leptos::html::Main>) {}

#[cfg(target_arch = "wasm32")]
pub fn set_title(title: &str) {
    if let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) {
        document.set_title(title);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_title(_title: &str) {}
