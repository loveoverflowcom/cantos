//! Browser navigation, focus and presentation geometry, plus the one audio element in `audio`.
#[cfg(feature = "preview")]
use crate::app::routes::Route;
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
pub(crate) mod audio;

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

#[cfg(feature = "preview")]
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

#[cfg(feature = "preview")]
#[cfg(not(target_arch = "wasm32"))]
pub fn bind_navigation(_route: RwSignal<Route>, _main: NodeRef<leptos::html::Main>) {}

#[cfg(feature = "preview")]
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

#[cfg(feature = "preview")]
#[cfg(target_arch = "wasm32")]
pub fn focus_input(input: NodeRef<leptos::html::Input>) {
    if let Some(element) = input.get_untracked() {
        let _ = element.focus();
    }
}

#[cfg(feature = "preview")]
#[cfg(not(target_arch = "wasm32"))]
pub fn focus_input(_input: NodeRef<leptos::html::Input>) {}

#[cfg(feature = "preview")]
#[cfg(target_arch = "wasm32")]
pub fn focus_button(button: NodeRef<leptos::html::Button>) {
    if let Some(element) = button.get_untracked() {
        let _ = element.focus();
    }
}

#[cfg(feature = "preview")]
#[cfg(not(target_arch = "wasm32"))]
pub fn focus_button(_button: NodeRef<leptos::html::Button>) {}

/// Signup phase changes may remove the initiating control. Resolve within this mounted panel
/// after rendering; a disposed/removed panel cannot focus a new route with the same field IDs.
pub(crate) fn focus_in_signup(panel: NodeRef<leptos::html::Div>, id: &'static str) {
    #[cfg(target_arch = "wasm32")]
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        use leptos::wasm_bindgen::JsCast;
        let Some(Some(panel)) = panel.try_get_untracked() else {
            return;
        };
        if !panel.is_connected() {
            return;
        }
        if let Some(element) = panel
            .query_selector(&format!("#{id}"))
            .ok()
            .flatten()
            .and_then(|element| element.dyn_into::<leptos::web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (panel, id);
}

/// A cleared search removes its Clear action. Restore a useful target only if that removal
/// lost focus; a listener who already moved elsewhere keeps it. Resolve within the same mount
/// so a delayed frame cannot focus another route or a newly mounted search with the same ID.
pub(crate) fn restore_disappearing_action_focus(
    panel: NodeRef<leptos::html::Div>,
    id: &'static str,
) {
    #[cfg(target_arch = "wasm32")]
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        use leptos::wasm_bindgen::JsCast;
        let Some(Some(panel)) = panel.try_get_untracked() else {
            return;
        };
        if !panel.is_connected() {
            return;
        }
        let Some(document) = panel.owner_document() else {
            return;
        };
        if document
            .active_element()
            .is_some_and(|active| active.tag_name() != "BODY")
        {
            return;
        }
        if let Some(element) = panel
            .query_selector(&format!("#{id}"))
            .ok()
            .flatten()
            .and_then(|element| element.dyn_into::<leptos::web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (panel, id);
}

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

/// A startup error has no pressed opener. Focus a stable shell return target synchronously,
/// before Dialog captures the active element; animation-frame focus would run after showModal.
#[cfg(target_arch = "wasm32")]
pub(crate) fn prepare_error_focus() -> Result<(), ()> {
    use leptos::wasm_bindgen::JsCast;
    let document = leptos::web_sys::window()
        .and_then(|window| window.document())
        .ok_or(())?;
    if document
        .active_element()
        .is_some_and(|element| element.tag_name() != "BODY")
    {
        return Ok(());
    }
    let main = document
        .get_element_by_id("theatre-main")
        .and_then(|element| element.dyn_into::<leptos::web_sys::HtmlElement>().ok())
        .ok_or(())?;
    main.focus().map_err(|_| ())
}

/// Mirrors the interface locale onto `<html lang>` so the whole document, including browser UI
/// and content outside the app root, is announced in the chosen language.
#[cfg(target_arch = "wasm32")]
pub fn set_document_locale(tag: &str, direction: &str) {
    if let Some(root) = leptos::web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("lang", tag);
        let _ = root.set_attribute("dir", direction);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_document_locale(_tag: &str, _direction: &str) {}

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

/// Pinned keyed rendering moves retained nodes with insertBefore, which can clear browser
/// focus/selection. Snapshot before that synchronous diff; restore in its next microtask,
/// before the existing page-completion RAF. Never carry interaction across removed owners.
pub(crate) fn preserve_library_row_interaction(list: NodeRef<leptos::html::Ul>) {
    #[cfg(target_arch = "wasm32")]
    if let Some(list) = list.get_untracked() {
        preserve_library_interaction(&list);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = list;
}

// Native Selection bindings stay at the browser boundary. Reuse Leptos's wasm-bindgen;
// no new dependency/features or production test hook is needed.
#[cfg(target_arch = "wasm32")]
#[leptos::wasm_bindgen::prelude::wasm_bindgen(
    wasm_bindgen = leptos::wasm_bindgen,
    inline_js = r#"
export function preserve_library_interaction(list) {
    const doc = list.ownerDocument;
    const active = list.contains(doc.activeElement) ? doc.activeElement : null;
    const selection = doc.getSelection();
    const inside = node => node && list.contains(node);
    const saved = selection && inside(selection.anchorNode) && inside(selection.focusNode)
        ? {anchor: selection.anchorNode, anchorOffset: selection.anchorOffset,
           focus: selection.focusNode, focusOffset: selection.focusOffset,
           anchorText: selection.anchorNode.textContent, focusText: selection.focusNode.textContent}
        : null;
    if (!active && !saved) return;
    queueMicrotask(() => {
        if (!list.isConnected) return;
        if (active && inside(active) && active.isConnected &&
            (!doc.activeElement || doc.activeElement === doc.body)) {
            active.focus({preventScroll: true});
        }
        if (saved && inside(saved.anchor) && inside(saved.focus) &&
            saved.anchor.isConnected && saved.focus.isConnected &&
            saved.anchor.textContent === saved.anchorText && saved.focus.textContent === saved.focusText) {
            selection.setBaseAndExtent(saved.anchor, saved.anchorOffset, saved.focus, saved.focusOffset);
        }
    });
}
"#
)]
extern "C" {
    fn preserve_library_interaction(list: &leptos::web_sys::HtmlUListElement);
}

/// Keep keyboard focus on the retained row when its action subtree becomes unavailable.
/// Call synchronously at the conditional boundary, while the focused node is still connected.
pub(crate) fn focus_before_unit_actions_removed(
    actions: NodeRef<leptos::html::Div>,
    heading: NodeRef<leptos::html::H3>,
) {
    #[cfg(target_arch = "wasm32")]
    if let (Some(actions), Some(heading)) = (actions.get_untracked(), heading.get_untracked()) {
        let active = leptos::web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element());
        if active
            .as_ref()
            .is_some_and(|active| actions.contains(Some(active)))
        {
            let _ = heading.focus();
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (actions, heading);
}

/// A disappearing unit action has already handed focus to its retained heading during render.
/// Preserve that handoff when the page request's usual status/new-row focus runs afterward.
#[cfg(target_arch = "wasm32")]
pub(crate) fn focus_after_unit_page(id: String) {
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        use leptos::wasm_bindgen::JsCast;
        let Some(document) = leptos::web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let handed_off = document.active_element().is_some_and(|active| {
            active.tag_name() == "H3"
                && active.id().starts_with("theatre-public-unit-")
                && active
                    .parent_element()
                    .and_then(|copy| copy.parent_element())
                    .is_some_and(|row| {
                        row.get_attribute("data-unavailable").as_deref() == Some("true")
                    })
        });
        if handed_off {
            return;
        }
        if let Some(element) = document
            .get_element_by_id(&id)
            .and_then(|e| e.dyn_into::<leptos::web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    });
}

mod search_dock;
pub(crate) use search_dock::{
    bind_search_dock, click_is_search_input, focus_is_search_input, focus_search_input,
    focus_search_result,
};

mod loading_presence;
pub(crate) use loading_presence::{bind_loading_presence, loading_presence};
