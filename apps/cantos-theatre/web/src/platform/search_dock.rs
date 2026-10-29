//! Search presentation lifetime only. Visual viewport measurements include an onscreen keyboard;
//! no query, request or navigation is triggered by focus, blur, resize or dismissal.
use leptos::prelude::*;

/// Space kept above the field when the page scrolls to make room for the results layer.
#[cfg(target_arch = "wasm32")]
const KEEP_FIELD_VISIBLE_PX: f64 = 8.0;
/// How long the browser's own scroll to a newly focused field is given before room is rechecked.
#[cfg(target_arch = "wasm32")]
const SETTLE_MS: u64 = 250;

/// Bottom edge of what the reader can see: the visual viewport includes an onscreen keyboard.
#[cfg(target_arch = "wasm32")]
fn visible_bottom(viewport: Option<&web_sys::VisualViewport>) -> f64 {
    viewport
        .map(|v| v.offset_top() + v.height())
        .or_else(|| web_sys::window()?.inner_height().ok()?.as_f64())
        .unwrap_or(0.0)
}

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
    let observer = StoredValue::new_local(None::<(web_sys::ResizeObserver, Closure<dyn FnMut()>)>);
    let make_room = StoredValue::new_local(None::<Rc<dyn Fn()>>);
    Effect::new(move |_| {
        let Some(panel) = panel.get() else {
            return;
        };
        let Some(window) = web_sys::window() else {
            return;
        };
        // The field (header and its messages) is measured independently: the results layer floats
        // below its bottom edge, so growing or showing it cannot move the anchor.
        let measured = panel.clone();
        let viewport = window.visual_viewport();
        let measure: Rc<dyn Fn()> = Rc::new(move || {
            let Some(field) = measured.query_selector(".cui-field").ok().flatten() else {
                return;
            };
            let available = (visible_bottom(viewport.as_ref())
                - field.get_bounding_client_rect().bottom())
            .max(0.0);
            let _ = web_sys::HtmlElement::style(&measured)
                .set_property("--cantos-web-search-available", &format!("{available}px"));
        });
        measure();
        // The field also moves without a scroll or resize: a locale change, a banner, late fonts
        // or loaded rows above it. A stale clearance would let the layer hang below the fold.
        let on_shift = {
            let measure = measure.clone();
            Closure::wrap(Box::new(move || measure()) as Box<dyn FnMut()>)
        };
        if let Ok(resizes) = web_sys::ResizeObserver::new(on_shift.as_ref().unchecked_ref()) {
            resizes.observe(&panel);
            if let Some(body) = window.document().and_then(|document| document.body()) {
                resizes.observe(&body);
            }
            observer.set_value(Some((resizes, on_shift)));
        }
        // Opening (or a keyboard shrinking the visual viewport) scrolls the page just enough for
        // the layer to sit above the fold, never past the point where the field leaves the top.
        // A manual page scroll never asks for this, so it cannot fight the reader.
        let room: Rc<dyn Fn()> = {
            let measure = measure.clone();
            let panel = panel.clone();
            let viewport = window.visual_viewport();
            Rc::new(move || {
                measure();
                let (Some(window), Some(field), Some(dock)) = (
                    web_sys::window(),
                    panel
                        .query_selector(".cui-field")
                        .ok()
                        .flatten()
                        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()),
                    panel
                        .query_selector(".cui-search-dock")
                        .ok()
                        .flatten()
                        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()),
                ) else {
                    return;
                };
                let bottom = visible_bottom(viewport.as_ref());
                let field_rect = field.get_bounding_client_rect();
                let gap = f64::from(dock.offset_top() - field.offset_height()).max(0.0);
                let overshoot =
                    field_rect.bottom() + gap + f64::from(dock.offset_height()) - bottom;
                let headroom = (field_rect.top() - KEEP_FIELD_VISIBLE_PX).max(0.0);
                let by = overshoot.min(headroom);
                if by >= 1.0 {
                    window.scroll_by_with_x_and_y(0.0, by);
                }
            })
        };
        make_room.set_value(Some(room.clone()));
        let mut targets: Vec<(EventTarget, &'static str)> = vec![
            (window.clone().into(), "resize"),
            (window.clone().into(), "scroll"),
        ];
        if let Some(viewport) = window.visual_viewport() {
            let keyboard = Closure::wrap(Box::new({
                let room = room.clone();
                move |_: web_sys::Event| {
                    if expanded.get_untracked() {
                        room();
                    }
                }
            }) as Box<dyn FnMut(web_sys::Event)>);
            let target: EventTarget = viewport.clone().into();
            if target
                .add_event_listener_with_callback("resize", keyboard.as_ref().unchecked_ref())
                .is_ok()
            {
                bindings.update_value(|list| list.push((target, "resize", keyboard)));
            }
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
                    // The field owns both the header and its results layer, so a press on a
                    // result, its scrollbar or the layer's padding is never an outside press.
                    panel
                        .query_selector(".cui-field")
                        .ok()
                        .flatten()
                        .is_some_and(|field| field.contains(Some(&node)))
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
    // The browser's own scroll-to-focused-field can still be moving when the layer opens, so the
    // room is made once next frame and once more after it has settled. Never on a plain page scroll.
    Effect::new(move |_| {
        if expanded.get() {
            if let Some(room) = make_room.with_value(Clone::clone) {
                let later = room.clone();
                leptos::leptos_dom::helpers::request_animation_frame(move || room());
                leptos::leptos_dom::helpers::set_timeout(
                    move || {
                        if expanded.get_untracked() {
                            later();
                        }
                    },
                    std::time::Duration::from_millis(SETTLE_MS),
                );
            }
        }
    });
    on_cleanup(move || {
        observer.try_update_value(|current| {
            if let Some((resizes, _)) = current.take() {
                resizes.disconnect();
            }
        });
        make_room.try_update_value(|room| *room = None);
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
/// Focus moved from inside the search field (header or results layer) to another element outside
/// it, e.g. Tab past the last result onto a filter chip the open layer would otherwise cover.
/// A press on the layer's own padding blurs to nothing (`relatedTarget` null) and stays open.
pub(crate) fn focus_left_search_field(event: &leptos::ev::FocusEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    let element = |target: Option<leptos::web_sys::EventTarget>| {
        target.and_then(|t| t.dyn_into::<leptos::web_sys::Element>().ok())
    };
    let Some(field) = element(event.target()).and_then(|e| e.closest(".cui-field").ok().flatten())
    else {
        return false;
    };
    element(event.related_target()).is_some_and(|next| !field.contains(Some(&next)))
}
/// The key was pressed on one of the layer's result buttons.
pub(crate) fn key_is_on_result(event: &leptos::ev::KeyboardEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    event
        .target()
        .and_then(|t| t.dyn_into::<leptos::web_sys::Element>().ok())
        .is_some_and(|e| {
            e.closest(".theatre-search-result-list button")
                .ok()
                .flatten()
                .is_some()
        })
}

/// A press on the layer's own background (padding, gaps, empty space), not on a control in it.
/// The browser would move focus to the nearest focusable ancestor, taking the caret from the
/// input and the dock's keys with it; the caller cancels that default so focus stays where it was.
pub(crate) fn press_on_layer_background(event: &leptos::ev::MouseEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    let Some(target) = event
        .target()
        .and_then(|t| t.dyn_into::<leptos::web_sys::Element>().ok())
    else {
        return false;
    };
    let Some(layer) = target.closest(".cui-search-dock").ok().flatten() else {
        return false;
    };
    match target
        .closest("button, a[href], input, select, textarea, summary, [tabindex]")
        .ok()
        .flatten()
    {
        Some(control) => !layer.contains(Some(&control)),
        None => true,
    }
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

/// Arrow keys step through the result buttons: down to the next row, up to the previous one and,
/// above the first, back to the input. The ends do not wrap, so a reader always knows where they are.
pub(crate) fn step_search_result(panel: NodeRef<leptos::html::Div>, forward: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsCast;
        let row = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .and_then(|e| e.closest("li").ok().flatten());
        let next = row.and_then(|row| {
            if forward {
                row.next_element_sibling()
            } else {
                row.previous_element_sibling()
            }
        });
        match next {
            Some(row) => {
                if let Some(button) = row
                    .query_selector("button")
                    .ok()
                    .flatten()
                    .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
                {
                    let _ = button.focus();
                }
            }
            None if !forward => focus_search_input(panel),
            None => {}
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (panel, forward);
}
