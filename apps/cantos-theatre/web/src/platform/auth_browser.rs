//! Browser-only history and nonsecret return context. No credential is persisted here.
#[cfg(target_arch = "wasm32")]
use crate::application::auth_navigation::ProviderReturn;
use crate::{app::routes::Route, application::auth_navigation::AuthRoute};
use leptos::prelude::*;

#[derive(Clone, Default)]
pub(crate) struct ReturnSnapshot {
    pub scroll_y: f64,
    pub focus_id: String,
}

pub(crate) fn capture_return() -> ReturnSnapshot {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        return ReturnSnapshot {
            scroll_y: window.scroll_y().unwrap_or(0.0),
            focus_id: window
                .document()
                .and_then(|d| d.active_element())
                .map(|e| e.id())
                .unwrap_or_default(),
        };
    }
    ReturnSnapshot::default()
}

pub(crate) fn restore_return(snapshot: ReturnSnapshot) {
    #[cfg(target_arch = "wasm32")]
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        use web_sys::wasm_bindgen::JsCast;
        if let Some(window) = web_sys::window() {
            let captured = window
                .document()
                .and_then(|document| document.get_element_by_id(&snapshot.focus_id))
                .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok());
            if let Some(element) = captured {
                let _ = element.focus();
                window.scroll_to_with_x_and_y(0.0, snapshot.scroll_y);
            } else if let Some(main) = window
                .document()
                .and_then(|document| document.get_element_by_id("theatre-main"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = main.focus();
                window.scroll_to_with_x_and_y(0.0, 0.0);
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (snapshot.scroll_y, snapshot.focus_id);
}

pub(crate) fn go(route: RwSignal<Route>, destination: Route, replace: bool) {
    if route.get_untracked() == destination && !replace {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        if let Ok(history) = window.history() {
            let previous = web_sys::wasm_bindgen::JsValue::from_str(&route.get_untracked().hash());
            let result = if replace {
                history.replace_state_with_url(
                    &web_sys::wasm_bindgen::JsValue::NULL,
                    "",
                    Some(&destination.hash()),
                )
            } else {
                history.push_state_with_url(&previous, "", Some(&destination.hash()))
            };
            if result.is_err() {
                return;
            }
        }
    }
    route.set(destination);
}

pub(crate) fn back(route: RwSignal<Route>, destination: Route) {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        if let Ok(history) = window.history() {
            // Only an entry created by this shell can authorize history traversal. A deep link
            // instead replaces itself with its typed fallback, avoiding a Back loop.
            if history.state().ok().and_then(|s| s.as_string()).as_deref()
                == Some(&destination.hash())
                && history.back().is_ok()
            {
                return;
            }
        }
    }
    go(route, destination, true);
}

pub(crate) fn clear_provider_return() {
    #[cfg(target_arch = "wasm32")]
    if let Some(storage) = web_sys::window().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = storage.remove_item("cantos.auth-return.v1");
        let _ = storage.remove_item("cantos.auth-focus.v1");
    }
}

/// Consume once. Only a canonical closed route written by this shell survives reload.
pub(crate) fn take_provider_return(callback: &str) -> Option<AuthRoute> {
    #[cfg(target_arch = "wasm32")]
    {
        let storage = web_sys::window()?.session_storage().ok()??;
        let value = storage.get_item("cantos.auth-return.v1").ok()??;
        let context = ProviderReturn::parse(&value)?;
        // An abandoned flow must not consume a newer flow's return intent.
        context.response(callback)?;
        storage.remove_item("cantos.auth-return.v1").ok()?;
        Some(context.route())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = callback;
        None
    }
}

pub(crate) fn hash() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().hash().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    String::new()
}

pub(crate) fn take_return_snapshot() -> Option<(Route, ReturnSnapshot)> {
    #[cfg(target_arch = "wasm32")]
    {
        let storage = web_sys::window()?.session_storage().ok()??;
        let data = storage.get_item("cantos.auth-focus.v1").ok()??;
        storage.remove_item("cantos.auth-focus.v1").ok()?;
        let values: (String, f64, String) = serde_json::from_str(&data).ok()?;
        let route = Route::from_hash(&values.0);
        if route.hash() != values.0
            || !values.1.is_finite()
            || values.1 < 0.0
            || values.1 > 10_000_000.0
            || values.2.len() > 96
        {
            return None;
        }
        Some((
            route,
            ReturnSnapshot {
                scroll_y: values.1,
                focus_id: values.2,
            },
        ))
    }
    #[cfg(not(target_arch = "wasm32"))]
    None
}
