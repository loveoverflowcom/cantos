//! Browser-only history and nonsecret return context. No credential or session is persisted here.
use crate::app::routes::Route;
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

/// Push a discovery link carrying `query`. The entry is skipped when the address already is that
/// link, so repeating a search never stacks identical entries. `route` becomes the shown route.
pub(crate) fn push_discovery(
    route: RwSignal<Route>,
    screen: Route,
    query: &crate::application::public_discovery::DiscoveryQuery,
) {
    let destination = screen.discovery_hash(query);
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = web_sys::window() {
        if let Ok(history) = window.history() {
            if hash() != destination {
                let previous =
                    web_sys::wasm_bindgen::JsValue::from_str(&route.get_untracked().hash());
                if history
                    .push_state_with_url(&previous, "", Some(&destination))
                    .is_err()
                {
                    return;
                }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = &destination;
    if route.get_untracked() != screen {
        route.set(screen);
    }
}

/// Rewrite the current address to the canonical discovery link for `query`, without a new history
/// entry. Used when an older link carried a filter this page no longer has.
pub(crate) fn replace_discovery(
    screen: Route,
    query: &crate::application::public_discovery::DiscoveryQuery,
) {
    let destination = screen.discovery_hash(query);
    #[cfg(target_arch = "wasm32")]
    if let Some(history) = web_sys::window().and_then(|window| window.history().ok()) {
        let _ = history.replace_state_with_url(
            &web_sys::wasm_bindgen::JsValue::NULL,
            "",
            Some(&destination),
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = &destination;
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
