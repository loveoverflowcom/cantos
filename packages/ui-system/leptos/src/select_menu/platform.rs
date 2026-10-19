//! Top-layer positioning and lifecycle. No OS preferences, app state or services.
use super::model::{popup_width, MenuState};
use leptos::{
    html,
    prelude::*,
    wasm_bindgen::{closure::Closure, JsCast},
};
use web_sys::{Event, HtmlElement};

pub(super) fn focus(trigger: NodeRef<html::Button>) {
    if let Some(trigger) = trigger.get_untracked() {
        let _ = trigger.focus();
    }
}

pub(super) fn is_open(popup: NodeRef<html::Div>) -> bool {
    popup
        .get_untracked()
        .is_some_and(|popup| popup.matches(":popover-open").unwrap_or(false))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PositionError {
    GeometryUnavailable,
    AnchorOutsideViewport,
}

fn position(trigger: &HtmlElement, popup: &HtmlElement) -> Result<(), PositionError> {
    let window = web_sys::window().ok_or(PositionError::GeometryUnavailable)?;
    let viewport = window.visual_viewport();
    let (left, top, width, height) = if let Some(viewport) = viewport {
        (
            viewport.offset_left(),
            viewport.offset_top(),
            viewport.width(),
            viewport.height(),
        )
    } else {
        (
            0.0,
            0.0,
            window
                .inner_width()
                .map_err(|_| PositionError::GeometryUnavailable)?
                .as_f64()
                .ok_or(PositionError::GeometryUnavailable)?,
            window
                .inner_height()
                .map_err(|_| PositionError::GeometryUnavailable)?
                .as_f64()
                .ok_or(PositionError::GeometryUnavailable)?,
        )
    };
    let computed = window
        .get_computed_style(popup)
        .map_err(|_| PositionError::GeometryUnavailable)?
        .ok_or(PositionError::GeometryUnavailable)?;
    // The menu's token-based padding is also its viewport inset and trigger offset.
    let inset = computed
        .get_property_value("padding-top")
        .map_err(|_| PositionError::GeometryUnavailable)?
        .trim_end_matches("px")
        .parse::<f64>()
        .map_err(|_| PositionError::GeometryUnavailable)?;
    let anchor = trigger.get_bounding_client_rect();
    if anchor.bottom() <= top
        || anchor.top() >= top + height
        || anchor.right() <= left
        || anchor.left() >= left + width
    {
        return Err(PositionError::AnchorOutsideViewport);
    }
    let style = popup.style();
    // Measure intrinsic option/check anatomy before bounding it. Compact triggers need not
    // force localized options into a narrower column than their natural text width.
    style
        .set_property("width", "max-content")
        .map_err(|_| PositionError::GeometryUnavailable)?;
    let menu_width = popup_width(
        anchor.width(),
        popup.get_bounding_client_rect().width(),
        width - inset * 2.0,
    );
    let rtl = computed
        .get_property_value("direction")
        .map_err(|_| PositionError::GeometryUnavailable)?
        == "rtl";
    let preferred_x = if rtl {
        anchor.right() - menu_width
    } else {
        anchor.left()
    };
    let x = preferred_x
        .max(left + inset)
        .min((left + width - menu_width - inset).max(left + inset));
    let below = (top + height - anchor.bottom() - inset * 2.0).max(0.0);
    let above = (anchor.top() - top - inset * 2.0).max(0.0);
    style
        .set_property("width", &format!("{menu_width}px"))
        .map_err(|_| PositionError::GeometryUnavailable)?;
    let below_anchor = popup.scroll_height() as f64 <= below || below >= above;
    let available = if below_anchor { below } else { above };
    style
        .set_property("max-height", &format!("{available}px"))
        .map_err(|_| PositionError::GeometryUnavailable)?;
    let y = if below_anchor {
        anchor.bottom() + inset
    } else {
        anchor.top() - inset - popup.get_bounding_client_rect().height()
    };
    style
        .set_property("left", &format!("{x}px"))
        .map_err(|_| PositionError::GeometryUnavailable)?;
    let lowest_top =
        (top + height - popup.get_bounding_client_rect().height() - inset).max(top + inset);
    style
        .set_property("top", &format!("{}px", y.max(top + inset).min(lowest_top)))
        .map_err(|_| PositionError::GeometryUnavailable)?;
    Ok(())
}

fn reveal_active(popup: &HtmlElement, active: Option<usize>) {
    let Some(index) = active else {
        return;
    };
    let Some(option) = popup
        .children()
        .item(index as u32)
        .and_then(|item| item.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    let top = option.offset_top();
    let bottom = top + option.offset_height();
    if top < popup.scroll_top() {
        popup.set_scroll_top(top);
    } else if bottom > popup.scroll_top() + popup.client_height() {
        popup.set_scroll_top(bottom - popup.client_height());
    }
}

type Binding = (
    web_sys::Window,
    Closure<dyn FnMut(Event)>,
    Option<web_sys::VisualViewport>,
    Option<(web_sys::ResizeObserver, Closure<dyn FnMut()>)>,
);

pub(super) fn bind(
    trigger_ref: NodeRef<html::Button>,
    popup_ref: NodeRef<html::Div>,
    menu: RwSignal<MenuState>,
    enhanced: RwSignal<bool>,
) {
    Effect::new(move |_| {
        let Some(popup) = popup_ref.get() else {
            return;
        };
        // Unsupported engines retain the visible native select and its keyboard/form behavior.
        let supports =
            web_sys::js_sys::Reflect::has(popup.as_ref(), &"popover".into()).unwrap_or(false);
        enhanced.set(supports);
    });
    Effect::new(move |_| {
        let state = menu.get();
        if !enhanced.get() {
            return;
        }
        let (Some(trigger), Some(popup)) = (trigger_ref.get(), popup_ref.get()) else {
            return;
        };
        if state.open {
            // Popover escapes clipping/stacking ancestors while preserving scoped CSS inheritance.
            let outcome = popup
                .show_popover()
                .map_err(|_| PositionError::GeometryUnavailable)
                .and_then(|_| position(&trigger, &popup));
            if let Err(error) = outcome {
                let _ = popup.hide_popover();
                if error == PositionError::GeometryUnavailable {
                    enhanced.set(false);
                }
                menu.set(MenuState::default());
                return;
            }
            reveal_active(&popup, state.active);
        } else {
            let _ = popup.hide_popover();
        }
    });
    // Reposition on scroll (including nested scrollers), resize, zoom and font/trigger reflow.
    let binding = StoredValue::new_local(None::<Binding>);
    Effect::new(move |_| {
        let (Some(trigger), Some(popup)) = (trigger_ref.get(), popup_ref.get()) else {
            return;
        };
        if binding.with_value(|binding| binding.is_some()) {
            return;
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let update = move || {
            if !menu.get_untracked().open {
                return;
            }
            if !trigger.is_connected() || trigger.get_bounding_client_rect().width() == 0.0 {
                menu.set(MenuState::default());
                return;
            }
            if position(&trigger, &popup).is_err() {
                menu.set(MenuState::default());
            }
        };
        let event_update = update.clone();
        let callback =
            Closure::wrap(Box::new(move |_: Event| event_update()) as Box<dyn FnMut(Event)>);
        let handler = callback.as_ref().unchecked_ref();
        let _ = window.add_event_listener_with_callback("resize", handler);
        let _ = window.add_event_listener_with_callback_and_bool("scroll", handler, true);
        let viewport = window.visual_viewport();
        if let Some(viewport) = &viewport {
            let _ = viewport.add_event_listener_with_callback("resize", handler);
            let _ = viewport.add_event_listener_with_callback("scroll", handler);
        }
        let resize_callback = Closure::wrap(Box::new(update) as Box<dyn FnMut()>);
        let observer = web_sys::ResizeObserver::new(resize_callback.as_ref().unchecked_ref())
            .ok()
            .map(|observer| {
                if let Some(trigger) = trigger_ref.get_untracked() {
                    observer.observe(&trigger);
                }
                (observer, resize_callback)
            });
        binding.set_value(Some((window, callback, viewport, observer)));
    });
    on_cleanup(move || {
        let mut registered = None;
        binding.try_update_value(|value| registered = value.take());
        if let Some((window, callback, viewport, observer)) = registered {
            let handler = callback.as_ref().unchecked_ref();
            let _ = window.remove_event_listener_with_callback("resize", handler);
            let _ = window.remove_event_listener_with_callback_and_bool("scroll", handler, true);
            if let Some(viewport) = viewport {
                let _ = viewport.remove_event_listener_with_callback("resize", handler);
                let _ = viewport.remove_event_listener_with_callback("scroll", handler);
            }
            if let Some((observer, _callback)) = observer {
                observer.disconnect();
            }
        }
    });
}
