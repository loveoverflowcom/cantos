//! Contextual help primitive: a plain label tooltip following WCAG 2.2 SC 1.4.13.
//!
//! Dismissible by Escape without moving focus, hoverable so moving pointer onto the tooltip
//! keeps it visible, persistent across pointer and keyboard focus until explicitly dismissed.
use leptos::{ev, prelude::*};

use std::sync::atomic::{AtomicUsize, Ordering};

static TOOLTIP_COUNTER: AtomicUsize = AtomicUsize::new(1);

macro_rules! ui_primitive {
    ($($item:tt)*) => {
        #[component]
        $($item)*
    };
}

ui_primitive! {
    /// A plain label tooltip wrapper. Provides hover, keyboard focus, and Escape dismissal.
    pub fn Tooltip(
        children: Children,
        #[prop(into)] text: Signal<String>,
        #[prop(optional, into)] id: Option<String>,
        #[prop(optional, into)] class: Signal<String>,
    ) -> impl IntoView {
        let open = RwSignal::new(false);
        let tooltip_id = id.unwrap_or_else(|| {
            format!("cui-tooltip-{}", TOOLTIP_COUNTER.fetch_add(1, Ordering::Relaxed))
        });

        view! {
            <div class="cui-tooltip-anchor"
                on:mouseenter=move |_| open.set(true)
                on:mouseleave=move |_| open.set(false)
                on:focusin=move |_| open.set(true)
                on:focusout=move |_| open.set(false)
                on:keydown=move |event: ev::KeyboardEvent| {
                    if event.key() == "Escape" && open.get_untracked() {
                        event.prevent_default();
                        open.set(false);
                    }
                }>
                {children()}
                <span id=tooltip_id.clone()
                    role="tooltip"
                    class=move || {
                        let extra = class.get();
                        if extra.is_empty() {
                            "cui-tooltip".to_string()
                        } else {
                            format!("cui-tooltip {extra}")
                        }
                    }
                    data-state=move || if open.get() { "visible" } else { "hidden" }
                    hidden=move || !open.get()
                    on:mouseenter=move |_| open.set(true)
                    on:mouseleave=move |_| open.set(false)>
                    {move || text.get()}
                </span>
            </div>
        }
    }
}
