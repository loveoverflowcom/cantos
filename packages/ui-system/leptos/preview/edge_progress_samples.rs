//! Two placements, one host-controlled request and primary indicator. No service or debounce.
use super::{messages::edge, recipe::Locale};
use cantos_ui::{AccessibleName, Announcement, EdgeProgress, Select, TextField, TextFieldKind};
use leptos::prelude::*;

#[component]
pub fn EdgeProgressSamples(
    id: String,
    initial_state: &'static str,
    #[prop(into)] visible: Signal<bool>,
    #[prop(into)] locale: Signal<Locale>,
) -> impl IntoView {
    let state = RwSignal::new(initial_state.to_owned());
    let list = RwSignal::new(initial_state == "list-pending");
    let query = RwSignal::new(edge::TITLE.to_owned());
    let foreground = document_foreground();
    let pending = Signal::derive(move || {
        matches!(
            state.get().as_str(),
            "pending" | "list-pending" | "hidden" | "background"
        )
    });
    let showing = Signal::derive(move || visible.get() && state.get() != "hidden");
    let running_host = Signal::derive(move || foreground.get() && state.get() != "background");
    let search_ref = NodeRef::<leptos::html::Div>::new();
    let list_ref = NodeRef::<leptos::html::Div>::new();
    let search_visible = slot_visibility(search_ref, showing);
    let list_visible = slot_visibility(list_ref, showing);
    let label = Signal::derive(move || {
        AccessibleName::new(edge::pending(locale.get())).expect("nonblank preview status")
    });
    let field_id = format!("{id}-query");
    let state_id = format!("{id}-state");
    let placement_id = format!("{id}-placement");
    view! {
        <div class="cui-edge-specimen" data-preview-scenario="edge-progress" data-request-state=move || state.get()>
            <div lang="en" class="cui-edge-diagnostics">
                <Select id=state_id label=edge::STATE value=state on_input=Callback::new(move |value| state.set(value))>
                    {edge::STATES.iter().map(|value| view! { <option value=*value>{*value}</option> }).collect_view()}
                </Select>
                <Select id=placement_id label=edge::PLACEMENT value=Signal::derive(move || if list.get() { "list".to_owned() } else { "search".to_owned() })
                    on_input=Callback::new(move |value: String| list.set(value == "list"))>
                    <option value="search">{edge::SEARCH_PLACEMENT}</option><option value="list">{edge::LIST_PLACEMENT}</option>
                </Select>
            </div>
            <TextField id=field_id kind=TextFieldKind::Search label=Signal::derive(move || edge::search(locale.get()).to_owned())
                value=query on_input=Callback::new(move |value| query.set(value))/>
            <div class="cui-edge-slot" data-edge-placement="search" node_ref=search_ref>
                <Show when=move || state.get() != "disposed">
                    <EdgeProgress active=Signal::derive(move || pending.get() && !list.get()) visible=search_visible foreground=running_host label=label announcement=Announcement::Polite/>
                </Show>
            </div>
            <p class="cui-edge-outcome">{move || edge::outcome(locale.get(), &state.get())}</p>
            <div class="cui-edge-slot" data-edge-placement="list" node_ref=list_ref>
                <Show when=move || state.get() != "disposed">
                    <EdgeProgress active=Signal::derive(move || pending.get() && list.get()) visible=list_visible foreground=running_host label=label announcement=Announcement::Polite/>
                </Show>
            </div>
            <div class="cui-edge-retained" aria-busy=move || pending.get().then_some("true")>
                <a href="#edge-retained" class="cui-edge-retained-action">{edge::TITLE}</a>
                <p>{move || edge::retained(locale.get())}</p>
            </div>
        </div>
    }
}

/// Preview composition root resolves the document lifecycle; the reusable primitive receives facts.
fn document_foreground() -> RwSignal<bool> {
    let foreground = RwSignal::new(true);
    #[cfg(target_arch = "wasm32")]
    if let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) {
        use leptos::wasm_bindgen::{closure::Closure, JsCast};
        foreground.set(document.visibility_state() == web_sys::VisibilityState::Visible);
        let observed = document.clone();
        let listener = Closure::wrap(Box::new(move || {
            let _ = foreground
                .try_set(observed.visibility_state() == web_sys::VisibilityState::Visible);
        }) as Box<dyn FnMut()>);
        if document
            .add_event_listener_with_callback("visibilitychange", listener.as_ref().unchecked_ref())
            .is_ok()
        {
            let binding = StoredValue::new_local(Some((document, listener)));
            on_cleanup(move || {
                binding.try_update_value(|current| {
                    if let Some((document, listener)) = current.take() {
                        let _ = document.remove_event_listener_with_callback(
                            "visibilitychange",
                            listener.as_ref().unchecked_ref(),
                        );
                    }
                });
            });
        }
    }
    foreground
}

/// Preview-only viewport adapter. Scroll/resize/slot reflow measure visibility; no polling or frame
/// timer. A hidden recipe clears the borrowed fact and disposal disconnects every listener.
fn slot_visibility(
    reference: NodeRef<leptos::html::Div>,
    host_visible: Signal<bool>,
) -> Signal<bool> {
    let in_view = RwSignal::new(true);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = reference;
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::{closure::Closure, JsCast};
        use web_sys::{ResizeObserver, Window};
        type Binding = (Window, Closure<dyn FnMut()>, ResizeObserver);
        fn disconnect(binding: Option<Binding>) {
            if let Some((window, listener, resize)) = binding {
                for event in ["scroll", "resize"] {
                    let _ = window.remove_event_listener_with_callback_and_bool(
                        event,
                        listener.as_ref().unchecked_ref(),
                        true,
                    );
                }
                resize.disconnect();
            }
        }
        let binding = StoredValue::new_local(None::<Binding>);
        Effect::new(move |_| {
            let enabled = host_visible.get();
            let Some(element) = reference.get() else {
                return;
            };
            let Some(window) = leptos::web_sys::window() else {
                return;
            };
            binding.update_value(|current| disconnect(current.take()));
            let measured_window = window.clone();
            let measured_element = element.clone();
            let measure = move || {
                let bounds = measured_element.get_bounding_client_rect();
                let width = measured_window
                    .inner_width()
                    .ok()
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                let height = measured_window
                    .inner_height()
                    .ok()
                    .and_then(|value| value.as_f64())
                    .unwrap_or(0.0);
                let visible = enabled
                    && bounds.width() > 0.0
                    && bounds.height() > 0.0
                    && bounds.bottom() > 0.0
                    && bounds.top() < height
                    && bounds.right() > 0.0
                    && bounds.left() < width;
                let _ = in_view.try_set(visible);
            };
            measure();
            let callback = Closure::wrap(Box::new(measure) as Box<dyn FnMut()>);
            if let Ok(resize) = ResizeObserver::new(callback.as_ref().unchecked_ref()) {
                resize.observe(&element);
                for event in ["scroll", "resize"] {
                    let _ = window.add_event_listener_with_callback_and_bool(
                        event,
                        callback.as_ref().unchecked_ref(),
                        true,
                    );
                }
                binding.set_value(Some((window, callback, resize)));
            }
        });
        on_cleanup(move || {
            binding.try_update_value(|current| disconnect(current.take()));
        });
    }
    Signal::derive(move || host_visible.get() && in_view.get())
}
