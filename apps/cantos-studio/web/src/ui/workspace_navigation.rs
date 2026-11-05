//! Studio-owned rail/drawer composition over the shared modal contract.
use super::{WorkspaceNavigation, WorkspacePane};
use crate::messages::workspace::{text, Key};
use cantos_ui::{Button, ButtonVariant, Dialog, DialogError};
use leptos::prelude::*;

#[component]
pub fn WorkspaceNavigationShell(
    panel: RwSignal<WorkspacePane>,
    locale: Signal<crate::localization::Locale>,
    available: Signal<bool>,
    open: RwSignal<bool>,
    /// #582 can inject one preferences/account composition into both presentations.
    #[prop(optional_no_strip)]
    footer: Option<Callback<&'static str, AnyView>>,
) -> impl IntoView {
    let failure = RwSignal::new(false);
    let generation = StoredValue::new(0_u64);
    let chrome_locale = RwSignal::new(locale.get_untracked());
    Effect::new(move |_| {
        let next = locale.get();
        if chrome_locale.get_untracked() != next {
            let reopen = open.get_untracked();
            open.set(false);
            chrome_locale.set(next);
            // New static-label chrome must mount closed. showModal on the replacing node
            // before it is connected is rejected by the shared Dialog's safety contract.
            #[cfg(target_arch = "wasm32")]
            if reopen {
                leptos::leptos_dom::helpers::request_animation_frame(move || {
                    if available.try_get_untracked() == Some(true)
                        && chrome_locale.try_get_untracked() == Some(next)
                    {
                        let _ = open.try_set(true);
                    }
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _ = reopen;
        }
    });
    let menu = NodeRef::<leptos::html::Div>::new();
    #[cfg(target_arch = "wasm32")]
    platform::close_at_rail(menu, open, panel);
    Effect::new(move |_| {
        if !available.get() {
            open.set(false);
        }
    });
    let rail_footer = footer;
    view! {
        <div class="workspace-navigation">
            <div class="workspace-navigation-rail">
                <WorkspaceNavigation panel locale footer=rail_footer/>
            </div>
            <div class="workspace-navigation-menu" node_ref=menu>
                <Button id="studio-navigation-menu" variant=ButtonVariant::Tonal
                    focus_on_press=true aria_controls="studio-navigation-drawer"
                    aria_expanded=Signal::derive(move || open.get())
                    on_press=Callback::new(move |_| {
                        if available.get_untracked() { failure.set(false); open.set(true); }
                    })>
                    {move || text(Key::OpenNavigation, locale.get())}
                </Button>
                <span class="workspace-navigation-context">{move || panel.get().label(locale.get())}</span>
            </div>
            <p role="alert" hidden=move || !failure.get()>{move || text(Key::DialogFailure, locale.get())}</p>
            // Only modal chrome is recreated for translated static Dialog labels, never workspaces.
            {move || {
                let language = chrome_locale.get();
                // Replacing localized Dialog chrome closes its old native element. Ignore that
                // disposed instance's deferred close/error callbacks; they do not dismiss the
                // new drawer or reset its root-owned preferences.
                generation.update_value(|value| *value += 1);
                let current = generation.get_value();
                let drawer_footer = footer;
                view! {
                    <Dialog id="studio-navigation-drawer" class="workspace-navigation-drawer"
                        title=text(Key::Workspace, language) close_label=text(Key::CloseNavigation, language)
                        open=Signal::derive(move || open.get())
                        on_close_request=Callback::new(move |_| {
                            if generation.get_value() == current { open.set(false); }
                        })
                        on_error=Callback::new(move |error: DialogError| {
                            if generation.get_value() != current { return; }
                            open.set(false);
                            // Crossing to the rail hides the old opener; the resize helper places
                            // focus on the equivalent rail link after the shared dialog closes.
                            let hidden_opener = menu.get_untracked().is_some_and(|node| node.offset_width() == 0);
                            if available.get_untracked() && !(error == DialogError::ReturnFocusRejected && hidden_opener) {
                                failure.set(true);
                            }
                        })>
                        <WorkspaceNavigation panel locale scope="drawer" footer=drawer_footer
                            on_select=Callback::new(move |_| open.set(false))/>
                    </Dialog>
                }
            }}
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::*;
    use leptos::wasm_bindgen::{closure::Closure, JsCast};
    use web_sys::ResizeObserver;

    /// Observe actual CSS visibility, so one container breakpoint owns both layout and dismissal.
    pub(super) fn close_at_rail(
        menu: NodeRef<leptos::html::Div>,
        open: RwSignal<bool>,
        panel: RwSignal<WorkspacePane>,
    ) {
        let binding = StoredValue::new_local(None::<(ResizeObserver, Closure<dyn FnMut()>)>);
        Effect::new(move |_| {
            let Some(menu) = menu.get() else {
                return;
            };
            let observed = menu.clone();
            let measure = move || {
                if menu.offset_width() == 0 && open.get_untracked() {
                    open.set(false);
                    leptos::leptos_dom::helpers::request_animation_frame(move || {
                        if let Some(pane) = panel.try_get_untracked() {
                            crate::navigation::focus_heading(&format!("{}-control", pane.id()));
                        }
                    });
                }
            };
            let callback = Closure::wrap(Box::new(measure) as Box<dyn FnMut()>);
            if let Ok(observer) = ResizeObserver::new(callback.as_ref().unchecked_ref()) {
                observer.observe(&observed);
                binding.set_value(Some((observer, callback)));
            }
        });
        on_cleanup(move || {
            binding.try_update_value(|value| {
                if let Some((observer, _)) = value.take() {
                    observer.disconnect();
                }
            });
        });
    }
}
