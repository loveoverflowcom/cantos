//! Search presentation lifetime only. Visual viewport measurements include an onscreen keyboard;
//! no query, request or navigation is triggered by focus, blur, resize or dismissal.
use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
pub(crate) fn bind_search_dock(panel: NodeRef<leptos::html::Div>, expanded: RwSignal<bool>) {
    use leptos::wasm_bindgen::{closure::Closure, JsCast};
    use std::rc::Rc;
    use web_sys::EventTarget;

    let bindings = StoredValue::new_local(Vec::<(
        EventTarget,
        &'static str,
        Closure<dyn FnMut(web_sys::Event)>,
    )>::new());
    Effect::new(move |_| {
        let Some(panel) = panel.get() else {
            return;
        };
        let Some(window) = web_sys::window() else {
            return;
        };
        // The input header is measured independently: growing the body cannot move the anchor.
        let measured = panel.clone();
        let viewport = window.visual_viewport();
        let measure: Rc<dyn Fn()> = Rc::new(move || {
            let Some(field) = measured.query_selector(".cui-field-control").ok().flatten() else {
                return;
            };
            let bottom = viewport
                .as_ref()
                .map(|v| v.offset_top() + v.height())
                .or_else(|| web_sys::window()?.inner_height().ok()?.as_f64())
                .unwrap_or(0.0);
            let available = (bottom - field.get_bounding_client_rect().bottom()).max(0.0);
            let _ = web_sys::HtmlElement::style(&measured)
                .set_property("--cantos-web-search-available", &format!("{available}px"));
        });
        measure();
        let mut targets: Vec<(EventTarget, &'static str)> = vec![
            (window.clone().into(), "resize"),
            (window.clone().into(), "scroll"),
        ];
        if let Some(viewport) = window.visual_viewport() {
            targets.push((viewport.clone().into(), "resize"));
            targets.push((viewport.into(), "scroll"));
        }
        for (target, event) in targets {
            let measure = measure.clone();
            let callback = Closure::wrap(
                Box::new(move |_: web_sys::Event| measure()) as Box<dyn FnMut(web_sys::Event)>
            );
            if target
                .add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())
                .is_ok()
            {
                bindings.update_value(|list| list.push((target, event, callback)));
            }
        }
        let target: EventTarget = window.into();
        let callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let inside = event
                .target()
                .and_then(|node| node.dyn_into::<web_sys::Node>().ok())
                .is_some_and(|node| {
                    panel
                        .query_selector(".cui-field-shell")
                        .ok()
                        .flatten()
                        .is_some_and(|shell| shell.contains(Some(&node)))
                });
            if !inside {
                expanded.set(false);
            }
        }) as Box<dyn FnMut(web_sys::Event)>);
        if target
            .add_event_listener_with_callback("pointerdown", callback.as_ref().unchecked_ref())
            .is_ok()
        {
            bindings.update_value(|list| list.push((target, "pointerdown", callback)));
        }
    });
    on_cleanup(move || {
        bindings.try_update_value(|list| {
            for (target, event, callback) in list.drain(..) {
                let _ = target
                    .remove_event_listener_with_callback(event, callback.as_ref().unchecked_ref());
            }
        });
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn bind_search_dock(_: NodeRef<leptos::html::Div>, _: RwSignal<bool>) {}

pub(crate) fn focus_is_search_input(event: &leptos::ev::FocusEvent) -> bool {
    is_search_input(event.target())
}
pub(crate) fn click_is_search_input(event: &leptos::ev::MouseEvent) -> bool {
    is_search_input(event.target())
}
fn is_search_input(target: Option<leptos::web_sys::EventTarget>) -> bool {
    use leptos::wasm_bindgen::JsCast;
    target
        .and_then(|t| t.dyn_into::<leptos::web_sys::Element>().ok())
        .is_some_and(|e| e.id() == "theatre-discovery-search")
}

/// The caller suppresses its focus-open intent while restoring after collapse.
pub(crate) fn focus_search_input(panel: NodeRef<leptos::html::Div>) {
    #[cfg(target_arch = "wasm32")]
    if let Some(panel) = panel.get_untracked() {
        use leptos::wasm_bindgen::JsCast;
        if let Some(input) = panel
            .query_selector("#theatre-discovery-search")
            .ok()
            .flatten()
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = input.focus();
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = panel;
}

/// Results are real navigation buttons, not invented autocomplete options. Standard Tab remains
/// available; Up/Down from the input provides a keyboard shortcut into the current result list.
pub(crate) fn focus_search_result(panel: NodeRef<leptos::html::Div>, last: bool) {
    #[cfg(target_arch = "wasm32")]
    if let Some(panel) = panel.get_untracked() {
        use leptos::wasm_bindgen::JsCast;
        let selector = if last {
            ".theatre-search-result-list li:last-child button"
        } else {
            ".theatre-search-result-list li:first-child button"
        };
        if let Some(button) = panel
            .query_selector(selector)
            .ok()
            .flatten()
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = button.focus();
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (panel, last);
}
