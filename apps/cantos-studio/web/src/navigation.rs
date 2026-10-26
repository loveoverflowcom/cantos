//! Browser-only pane return state. No text, credentials or durable identity is copied here.
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, HtmlInputElement, HtmlTextAreaElement};

#[derive(Clone)]
pub struct FieldReturn {
    id: String,
    selection: Option<(u32, u32, String)>,
    scroll_top: i32,
    page_x: f64,
    page_y: f64,
}

impl FieldReturn {
    pub fn capture(target: Element) -> Option<Self> {
        target
            .closest("#studio-private-workspaces")
            .ok()
            .flatten()?;
        let id = target.id();
        if id.is_empty() {
            return None;
        }
        let selection = if let Some(field) = target.dyn_ref::<HtmlTextAreaElement>() {
            Some((
                field.selection_start().ok()??,
                field.selection_end().ok()??,
                field
                    .selection_direction()
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "none".into()),
            ))
        } else {
            let field = target.dyn_ref::<HtmlInputElement>()?;
            field
                .selection_start()
                .ok()
                .flatten()
                .zip(field.selection_end().ok().flatten())
                .map(|(start, end)| {
                    (
                        start,
                        end,
                        field
                            .selection_direction()
                            .ok()
                            .flatten()
                            .unwrap_or_else(|| "none".into()),
                    )
                })
        };
        let window = web_sys::window()?;
        Some(Self {
            id,
            selection,
            scroll_top: target.scroll_top(),
            page_x: window.scroll_x().ok()?,
            page_y: window.scroll_y().ok()?,
        })
    }

    pub fn restore(&self) -> bool {
        let Some(target) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(&self.id))
        else {
            return false;
        };
        let Some(element) = target.dyn_ref::<HtmlElement>() else {
            return false;
        };
        if element.focus().is_err() {
            return false;
        }
        if web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .as_ref()
            != Some(&target)
        {
            return false;
        }
        if let Some((start, end, direction)) = &self.selection {
            if let Some(field) = target.dyn_ref::<HtmlTextAreaElement>() {
                let _ = field.set_selection_range_with_direction(*start, *end, direction);
            } else if let Some(field) = target.dyn_ref::<HtmlInputElement>() {
                let _ = field.set_selection_range_with_direction(*start, *end, direction);
            }
        }
        target.set_scroll_top(self.scroll_top);
        if let Some(window) = web_sys::window() {
            window.scroll_to_with_x_and_y(self.page_x, self.page_y);
        }
        true
    }
}

pub fn focus_heading(id: &str) {
    if let Some(element) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}

/// Keeps the current workspace link visible when the narrow strip scrolls horizontally.
/// Where the list does not overflow the container has no scroll range, so this does nothing.
pub fn reveal_in_strip(id: &str) {
    let Some(item) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    let Some(strip) = item
        .parent_element()
        .and_then(|e| e.dyn_into::<HtmlElement>().ok())
    else {
        return;
    };
    let centered = item.offset_left() - (strip.client_width() - item.offset_width()) / 2;
    strip.set_scroll_left(centered.max(0));
}

/// A destination-specific CTA may already have focused its form after the pane became visible.
/// Do not replace that more precise intent with the shell's generic heading/caret fallback.
pub fn pane_has_focus(id: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
        .is_some_and(|element| {
            element.closest(&format!("#{id}")).ok().flatten().is_some()
                && element
                    .closest("[hidden], [inert]")
                    .ok()
                    .flatten()
                    .is_none()
        })
}
