//! App-owned placement and request facts over the admitted shared primitives.
use crate::messages::{text, Key, Locale};
use cantos_ui::{AccessibleName, EdgeProgress, SurfacePulse};
use leptos::prelude::*;

#[component]
pub(crate) fn RequestEdge(
    #[prop(into)] active: Signal<bool>,
    label: Key,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Div>::new();
    let visible = RwSignal::new(false);
    let foreground = RwSignal::new(false);
    crate::platform::bind_loading_presence(node, visible, foreground);
    view! { <div node_ref=node class="theatre-request-edge">
        {move || AccessibleName::new(text(label, locale.get())).ok().map(|label| view! {
            <EdgeProgress active visible foreground label/>
        })}
    </div> }
}

/// Production card geometry: one cover slot and line boxes; no separate spinner/status.
#[component]
pub(crate) fn PendingRows(#[prop(default = false)] compact: bool) -> impl IntoView {
    let node = NodeRef::<leptos::html::Div>::new();
    let active = crate::platform::loading_presence(node);
    view! { <div node_ref=node class="theatre-pending-rows" data-compact=compact aria-hidden="true">
        {(0..3).map(|_| view! { <div class="theatre-pending-row">
            <div class="theatre-pending-cover"><SurfacePulse active/></div>
            <div class="theatre-pending-lines"><div><SurfacePulse active/></div><div><SurfacePulse active/></div><div><SurfacePulse active/></div></div>
        </div> }).collect_view()}
    </div> }
}
