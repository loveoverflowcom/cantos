//! Presentation-only viewport/document facts. Missing browser capability keeps motion off.
use leptos::prelude::*;

/// Unknown viewport/document facts keep continuous motion off until the host observes them.
pub(crate) fn loading_presence(node: NodeRef<leptos::html::Div>) -> Signal<bool> {
    let visible = RwSignal::new(false);
    let foreground = RwSignal::new(false);
    bind_loading_presence(node, visible, foreground);
    Signal::derive(move || visible.get() && foreground.get())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn bind_loading_presence(
    node: NodeRef<leptos::html::Div>,
    visible: RwSignal<bool>,
    foreground: RwSignal<bool>,
) {
    use leptos::wasm_bindgen::{closure::Closure, JsCast};
    let binding = StoredValue::new_local(
        None::<(
            web_sys::IntersectionObserver,
            Closure<dyn FnMut(web_sys::js_sys::Array)>,
        )>,
    );
    Effect::new(move |_| {
        let Some(node) = node.get() else {
            return;
        };
        let callback = Closure::wrap(Box::new(move |entries: web_sys::js_sys::Array| {
            let showing = entries
                .iter()
                .filter_map(|entry| entry.dyn_into::<web_sys::IntersectionObserverEntry>().ok())
                .any(|entry| entry.is_intersecting());
            let _ = visible.try_set(showing);
        }) as Box<dyn FnMut(web_sys::js_sys::Array)>);
        if let Ok(observer) = web_sys::IntersectionObserver::new(callback.as_ref().unchecked_ref())
        {
            observer.observe(&node);
            binding.set_value(Some((observer, callback)));
        }
    });
    let update = move || {
        let showing = web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|d| d.visibility_state() == web_sys::VisibilityState::Visible);
        let _ = foreground.try_set(showing);
    };
    update();
    let listener = StoredValue::new_local(web_sys::window().and_then(|w| w.document()).and_then(
        |document| {
            let callback = Closure::wrap(
                Box::new(move |_: web_sys::Event| update()) as Box<dyn FnMut(web_sys::Event)>
            );
            document
                .add_event_listener_with_callback(
                    "visibilitychange",
                    callback.as_ref().unchecked_ref(),
                )
                .ok()?;
            Some((document, callback))
        },
    ));
    on_cleanup(move || {
        binding.try_update_value(|slot| {
            if let Some((observer, _)) = slot.take() {
                observer.disconnect();
            }
        });
        listener.try_update_value(|slot| {
            if let Some((document, callback)) = slot.take() {
                let _ = document.remove_event_listener_with_callback(
                    "visibilitychange",
                    callback.as_ref().unchecked_ref(),
                );
            }
        });
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn bind_loading_presence(
    _: NodeRef<leptos::html::Div>,
    _: RwSignal<bool>,
    _: RwSignal<bool>,
) {
}
