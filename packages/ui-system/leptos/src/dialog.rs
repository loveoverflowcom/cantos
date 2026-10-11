use crate::{Icon, IconName};
use leptos::{ev, html, prelude::*, web_sys::HtmlElement};

/// User intent is separate from the caller-owned open state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogCloseReason {
    Escape,
    CloseButton,
    /// A form with method="dialog" or another native caller has already closed the dialog.
    NativeClose,
}

/// Browser failures contain no raw JavaScript message; apps supply localized recovery text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogError {
    NotConnected,
    OpenRejected,
    InitialFocusUnavailable,
    InitialFocusOutsideDialog,
    InitialFocusRejected,
    ReturnFocusRejected,
}

/// Native modal anatomy with caller-owned state, localized content and effects.
///
/// Supply a unique stable `id`, non-empty `title` and visible `close_label`. The component
/// reserves `<id>-title`, `<id>-description` and `<id>-close`. Escape and the close button
/// request dismissal; the caller accepts by setting `open` to false. `on_error` must expose
/// browser failures and reconcile the caller's state; a failed showModal is never a modal.
///
/// The default autofocus is the close button. An optional `initial_focus` resolves a caller's
/// NodeRef to a safe, focusable descendant after mounting. Choose a non-destructive target;
/// an unavailable, outside or unfocusable target reports an error and keeps the safe fallback.
/// Put scrolling content in `children` and bounded confirmation controls in `actions`.
/// No background click dismissal, application decision or focus selector is installed.
///
/// Native focus containment, Escape and restoration require real-browser verification.
/// String rendering intentionally leaves the dialog closed, even when open is requested.
#[component]
pub fn Dialog(
    #[prop(into)] id: String,
    #[prop(into)] title: String,
    #[prop(into)] close_label: String,
    #[prop(into)] open: Signal<bool>,
    on_close_request: Callback<DialogCloseReason>,
    on_error: Callback<DialogError>,
    children: Children,
    #[prop(optional, into)] description: Option<String>,
    #[prop(optional, into)] actions: Option<ViewFn>,
    #[prop(optional)] initial_focus: Option<Callback<(), Option<HtmlElement>>>,
    #[prop(into, default = Signal::derive(String::new))] class: Signal<String>,
) -> impl IntoView {
    let title_id = format!("{id}-title");
    let description_id = format!("{id}-description");
    let close_id = format!("{id}-close");
    let labelled_by = title_id.clone();
    let described_by = description.as_ref().map(|_| description_id.clone());
    let dialog_ref = NodeRef::<html::Dialog>::new();
    let close_ref = NodeRef::<html::Button>::new();

    #[cfg(target_arch = "wasm32")]
    let browser_state = platform::bind(dialog_ref, close_ref, open, initial_focus, on_error);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (initial_focus, on_error);

    view! {
        <dialog id=id node_ref=dialog_ref class=move || format!("cui-dialog {}", class.get())
            aria-labelledby=labelled_by aria-describedby=described_by
            on:close=move |_| {
                #[cfg(target_arch = "wasm32")]
                platform::native_closed(dialog_ref, browser_state, open, on_close_request, on_error);
            }>
            <header class="cui-dialog-header">
                <h2 id=title_id class="cui-dialog-title">{title}</h2>
                <button id=close_id node_ref=close_ref type="button" autofocus=true
                    class="cui-button cui-dialog-close" data-variant="text"
                    on:click=move |_| {
                        if open.get_untracked() {
                            on_close_request.run(DialogCloseReason::CloseButton);
                        }
                    }>
                    <Icon name=IconName::Close/>
                    {close_label}
                </button>
            </header>
            <div class="cui-dialog-body">
                {description.map(|text| view! {
                    <p id=description_id class="cui-dialog-description">{text}</p>
                })}
                {children()}
            </div>
            {actions.map(|render| view! {
                <footer class="cui-dialog-actions">{render.run()}</footer>
            })}
        </dialog>
    }
    .add_any_attr(ev::on(ev::Custom::<leptos::web_sys::Event>::new("cancel"), move |event| {
        // Keep native state controlled: a caller may need to preserve an unsaved draft.
        event.prevent_default();
        if open.get_untracked() {
            on_close_request.run(DialogCloseReason::Escape);
        }
    }))
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{DialogCloseReason, DialogError};
    use leptos::{
        html,
        prelude::*,
        wasm_bindgen::JsCast,
        web_sys::{HtmlDialogElement, HtmlElement, Node},
    };

    type ReturnFocus = StoredValue<Option<HtmlElement>, LocalStorage>;

    pub(super) fn bind(
        dialog_ref: NodeRef<html::Dialog>,
        close_ref: NodeRef<html::Button>,
        open: Signal<bool>,
        initial_focus: Option<Callback<(), Option<HtmlElement>>>,
        on_error: Callback<DialogError>,
    ) -> ReturnFocus {
        // The local arena keeps browser handles on their mounting thread and releases them.
        let return_focus = StoredValue::new_local(None::<HtmlElement>);
        Effect::new(move |_| {
            let requested_open = open.get();
            let Some(dialog) = dialog_ref.get() else {
                return;
            };
            if requested_open && !dialog.open() {
                match show_modal(&dialog) {
                    Ok(trigger) => {
                        return_focus.set_value(trigger);
                        if let Err(error) = focus_initial(&dialog, close_ref, initial_focus) {
                            on_error.run(error);
                        }
                    }
                    Err(error) => on_error.run(error),
                }
            } else if !requested_open {
                close_modal(&dialog);
                if let Err(error) = restore_focus(return_focus) {
                    on_error.run(error);
                }
            }
        });
        on_cleanup(move || {
            if let Some(dialog) = dialog_ref.try_get_untracked().flatten() {
                close_modal(&dialog);
            }
            // Cleanup may run while callbacks are being disposed; it performs no app dispatch.
            let _ = restore_focus(return_focus);
        });
        return_focus
    }

    fn show_modal(dialog: &HtmlDialogElement) -> Result<Option<HtmlElement>, DialogError> {
        if !dialog.is_connected() {
            return Err(DialogError::NotConnected);
        }
        let trigger = dialog
            .owner_document()
            .and_then(|document| document.active_element())
            .and_then(|element| element.dyn_into::<HtmlElement>().ok());
        dialog.show_modal().map_err(|_| DialogError::OpenRejected)?;
        Ok(trigger)
    }

    fn close_modal(dialog: &HtmlDialogElement) {
        if dialog.open() {
            dialog.close();
        }
    }

    fn focus_initial(
        dialog: &HtmlDialogElement,
        close_ref: NodeRef<html::Button>,
        initial_focus: Option<Callback<(), Option<HtmlElement>>>,
    ) -> Result<(), DialogError> {
        let fallback = close_ref
            .get_untracked()
            .map(|button| button.unchecked_into::<HtmlElement>())
            .ok_or(DialogError::InitialFocusUnavailable)?;
        // Focus a safe control before attempting a caller-selected target.
        focus_element(&fallback).map_err(|_| DialogError::InitialFocusRejected)?;
        let Some(resolve) = initial_focus else {
            return Ok(());
        };
        let target = resolve
            .run(())
            .ok_or(DialogError::InitialFocusUnavailable)?;
        if !dialog.contains(Some(target.unchecked_ref::<Node>())) {
            return Err(DialogError::InitialFocusOutsideDialog);
        }
        focus_element(&target).map_err(|_| DialogError::InitialFocusRejected)
    }

    fn focus_element(element: &HtmlElement) -> Result<(), ()> {
        element.focus().map_err(|_| ())?;
        let is_focused = element
            .owner_document()
            .and_then(|document| document.active_element())
            .is_some_and(|active| active.is_same_node(Some(element.unchecked_ref::<Node>())));
        if is_focused {
            Ok(())
        } else {
            Err(())
        }
    }

    fn restore_focus(return_focus: ReturnFocus) -> Result<(), DialogError> {
        let trigger = return_focus.try_update_value(Option::take).flatten();
        // A removed trigger requires application-owned navigation focus, not a stale DOM call.
        if let Some(trigger) = trigger.filter(|element| element.is_connected()) {
            focus_element(&trigger).map_err(|_| DialogError::ReturnFocusRejected)?;
        }
        Ok(())
    }

    pub(super) fn native_closed(
        dialog_ref: NodeRef<html::Dialog>,
        return_focus: ReturnFocus,
        open: Signal<bool>,
        on_close_request: Callback<DialogCloseReason>,
        on_error: Callback<DialogError>,
    ) {
        // close events are queued; a rapid reopen must not dismiss the new modal.
        if dialog_ref
            .get_untracked()
            .is_none_or(|dialog| dialog.open())
        {
            return;
        }
        if let Err(error) = restore_focus(return_focus) {
            on_error.run(error);
        }
        if open.get_untracked() {
            on_close_request.run(DialogCloseReason::NativeClose);
        }
    }
}
