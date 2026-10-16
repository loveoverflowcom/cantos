//! Browser navigation, focus and presentation geometry. No media or network adapter.
use crate::app::routes::Route;
use leptos::prelude::*;

/// Restore a native range after canceled input, including when its controlled value is unchanged.
pub(crate) fn restore_range_value(input: NodeRef<leptos::html::Input>, position_ms: u32) {
    #[cfg(target_arch = "wasm32")]
    if let Some(input) = input.get_untracked() {
        input.set_value(&position_ms.to_string());
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (input, position_ms);
}

/// Reserve the measured overlay height for wrapped titles, text scaling and focus scrolling.
/// CSS retains a conservative fallback if ResizeObserver is unavailable.
#[cfg(target_arch = "wasm32")]
pub(crate) fn bind_mini_player_clearance(bar: NodeRef<leptos::html::Aside>) {
    use leptos::wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::{HtmlElement, ResizeObserver};

    let binding =
        StoredValue::new_local(None::<(ResizeObserver, Closure<dyn FnMut()>, HtmlElement)>);
    Effect::new(move |_| {
        let Some(bar) = bar.get() else {
            return;
        };
        let Some(parent) = bar.parent_element() else {
            return;
        };
        let Ok(shell) = parent.dyn_into::<HtmlElement>() else {
            return;
        };
        binding.update_value(|current| {
            if let Some((observer, _, shell)) = current.take() {
                observer.disconnect();
                let _ = shell.style().remove_property("--theatre-player-clearance");
            }
        });
        let measured_bar = bar.clone();
        let measured_shell = shell.clone();
        let measure = move || {
            let height = measured_bar.get_bounding_client_rect().height();
            if height.is_finite() && height > 0.0 {
                let _ = measured_shell.style().set_property(
                    "--theatre-player-clearance",
                    &format!("calc({height}px + var(--cantos-space-xl))"),
                );
            }
        };
        measure();
        let callback = Closure::wrap(Box::new(measure) as Box<dyn FnMut()>);
        if let Ok(observer) = ResizeObserver::new(callback.as_ref().unchecked_ref()) {
            observer.observe(&bar);
            binding.set_value(Some((observer, callback, shell)));
        } else {
            let _ = shell.style().remove_property("--theatre-player-clearance");
        }
    });
    on_cleanup(move || {
        binding.try_update_value(|current| {
            if let Some((observer, _, shell)) = current.take() {
                observer.disconnect();
                let _ = shell.style().remove_property("--theatre-player-clearance");
            }
        });
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn bind_mini_player_clearance(_bar: NodeRef<leptos::html::Aside>) {}

#[cfg(target_arch = "wasm32")]
pub fn bind_navigation(route: RwSignal<Route>, main: NodeRef<leptos::html::Main>) {
    use leptos::{leptos_dom::helpers::window_event_listener_untyped, web_sys};
    let read_route = move || {
        web_sys::window()
            .and_then(|window| window.location().hash().ok())
            .map(|hash| Route::from_mock_hash(&hash))
            .unwrap_or(Route::Discover)
    };
    route.set(read_route());
    let listener = window_event_listener_untyped("hashchange", move |_| {
        route.set(read_route());
        // A new route starts at its heading, not at the previous page's scroll offset.
        if let Some(window) = web_sys::window() {
            window.scroll_to_with_x_and_y(0.0, 0.0);
        }
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
pub fn focus_input(input: NodeRef<leptos::html::Input>) {
    if let Some(element) = input.get_untracked() {
        let _ = element.focus();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn focus_input(_input: NodeRef<leptos::html::Input>) {}

#[cfg(target_arch = "wasm32")]
pub fn focus_button(button: NodeRef<leptos::html::Button>) {
    if let Some(element) = button.get_untracked() {
        let _ = element.focus();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn focus_button(_button: NodeRef<leptos::html::Button>) {}

/// Focus an element rendered by the current frame, after Leptos has applied the change.
#[cfg(target_arch = "wasm32")]
pub(crate) fn focus_by_id(id: String) {
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        use leptos::wasm_bindgen::JsCast;
        if let Some(element) = leptos::web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(&id))
            .and_then(|element| element.dyn_into::<leptos::web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    });
}

/// Mirrors the interface locale onto `<html lang>` so the whole document, including browser UI
/// and content outside the app root, is announced in the chosen language.
#[cfg(target_arch = "wasm32")]
pub fn set_document_language(tag: &str) {
    if let Some(root) = leptos::web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("lang", tag);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_document_language(_tag: &str) {}

#[cfg(target_arch = "wasm32")]
pub fn set_title(title: &str) {
    if let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) {
        document.set_title(title);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_title(_title: &str) {}

pub(crate) mod auth_browser;
pub(crate) mod signup_browser;
