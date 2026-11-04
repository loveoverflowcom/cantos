//! Header account entry: an avatar trigger and a floating popover.
//!
//! Presentation only. The session owner (`SessionState`) stays in `view.rs`; this component owns
//! the disclosure geometry, dismissal and focus return,
//! and renders whatever account actions the shell passes as children.
use cantos_ui::{
    avatar_initials, avatar_seed, avatar_tone, AvatarInitials, Button, ButtonVariant, IconName,
};
use leptos::{ev, html, prelude::*};

pub const ENTRY_ID: &str = "studio-account-entry";
pub const MENU_ID: &str = "studio-account-menu";

#[cfg(target_arch = "wasm32")]
mod browser {
    use leptos::{
        ev, html,
        prelude::*,
        wasm_bindgen::JsCast,
        web_sys::{HtmlElement, Node},
    };

    fn focusable(wrapper: &HtmlElement) -> Vec<HtmlElement> {
        let Ok(candidates) = wrapper
            .query_selector_all("button, a[href], input, select, textarea, summary, [tabindex]")
        else {
            return Vec::new();
        };
        (0..candidates.length())
            .filter_map(|index| candidates.item(index)?.dyn_into::<HtmlElement>().ok())
            .filter(|element| {
                element.tab_index() >= 0
                    && !element.matches(":disabled").unwrap_or(true)
                    && element
                        .closest("[hidden], [inert]")
                        .ok()
                        .flatten()
                        .is_none()
                    && element.get_bounding_client_rect().width() > 0.0
                    && element.get_bounding_client_rect().height() > 0.0
            })
            .collect()
    }

    pub(super) fn focus_first(menu: NodeRef<html::Div>) {
        if let Some(menu) = menu.get_untracked() {
            if let Some(first) = focusable(&menu).first() {
                let _ = first.focus();
            }
        }
    }

    pub(super) fn focus(element: NodeRef<html::Button>) {
        if let Some(element) = element.get_untracked() {
            let _ = element.unchecked_into::<HtmlElement>().focus();
        }
    }

    /// Only a focus move to a real element outside the popover closes it; a click on
    /// non-focusable padding reports no related target and must not.
    pub(super) fn focus_left(event: &ev::FocusEvent, wrapper: NodeRef<html::Div>) -> bool {
        let (Some(wrapper), Some(related)) = (wrapper.get_untracked(), event.related_target())
        else {
            return false;
        };
        related
            .dyn_ref::<Node>()
            .is_some_and(|node| !wrapper.contains(Some(node)))
    }

    /// A press anywhere outside the entry (including empty page background) dismisses.
    pub(super) fn press_outside(event: &ev::PointerEvent, wrapper: NodeRef<html::Div>) -> bool {
        let (Some(wrapper), Some(target)) = (wrapper.get_untracked(), event.target()) else {
            return false;
        };
        target
            .dyn_ref::<Node>()
            .is_some_and(|node| !wrapper.contains(Some(node)))
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod browser {
    use leptos::{ev, html, prelude::*};

    pub(super) fn focus_left(_: &ev::FocusEvent, _: NodeRef<html::Div>) -> bool {
        false
    }
    pub(super) fn focus(_: NodeRef<html::Button>) {}
}

/// App-private identity badge. The shared `Avatar` is not admitted for new adoption until it has
/// both-renderer qualification evidence (the preview catalog gate), so this renders the same
/// shared initials and tone contracts with Studio-owned markup; swap to `Avatar` once qualified.
#[component]
fn ActorBadge(
    #[prop(into)] actor: Signal<Option<String>>,
    label: Signal<String>,
    size: &'static str,
) -> impl IntoView {
    // The session actor is an opaque authority key, never a display name.
    let initials = move || avatar_initials(&label.get(), AvatarInitials::Single);
    let tone = move || {
        let actor = actor.get();
        let name = actor.as_deref().unwrap_or_default();
        avatar_tone(&avatar_seed(name, actor.as_deref())).as_str()
    };
    view! { <span class="studio-avatar" data-size=size data-tone=tone aria-hidden="true">{initials}</span> }
}

/// The popover is mounted only while the entry is available and open; availability always wins.
pub const fn menu_visible(available: bool, open: bool) -> bool {
    available && open
}

/// `available` hides the entry outside the workspace and forces the popover closed. `open` is
/// owned by the shell so it can discard typed reauthentication input when the popover closes.
#[component]
pub fn AccountEntry(
    /// Opaque actor key, used only to choose a stable avatar tone.
    #[prop(into)]
    actor: Signal<Option<String>>,
    #[prop(into)] available: Signal<bool>,
    open: RwSignal<bool>,
    /// Localized accessible name of the trigger and the popover group.
    #[prop(into)]
    label: Signal<String>,
    /// Localized explicit dismissal; the trigger can be obscured by the narrow sheet.
    #[prop(into)]
    close_label: Signal<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let wrapper = NodeRef::<html::Div>::new();
    let trigger = NodeRef::<html::Button>::new();
    let menu = NodeRef::<html::Div>::new();
    let visible = Memo::new(move |_| menu_visible(available.get(), open.get()));

    Effect::new(move |_| {
        if !available.get() && open.get_untracked() {
            open.set(false);
        }
    });
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::leptos_dom::helpers::window_event_listener;
        Effect::new(move |_| {
            if visible.get() && menu.get().is_some() {
                browser::focus_first(menu);
            }
        });
        let press = window_event_listener(ev::pointerdown, move |event| {
            if open.get_untracked() && browser::press_outside(&event, wrapper) {
                open.set(false);
            }
        });
        let escape = window_event_listener(ev::keydown, move |event| {
            if event.key() == "Escape" && open.get_untracked() {
                event.prevent_default();
                open.set(false);
                browser::focus(trigger);
            }
        });
        on_cleanup(move || {
            press.remove();
            escape.remove();
        });
    }
    view! {
        <div class="studio-account" node_ref=wrapper hidden=move || !available.get()
            on:focusout=move |event: ev::FocusEvent| {
                if browser::focus_left(&event, wrapper) {
                    open.set(false);
                }
            }>
            <button id=ENTRY_ID node_ref=trigger type="button" class="studio-account-trigger"
                aria-label=move || label.get() aria-controls=MENU_ID
                aria-expanded=move || visible.get().to_string()
                on:click=move |_| open.update(|open| *open = !*open)>
                <ActorBadge actor=actor label=label size="medium"/>
            </button>
            <Show when=move || visible.get()>
                <div class="studio-account-scrim" on:click=move |_| open.set(false) aria-hidden="true"></div>
                <div id=MENU_ID node_ref=menu class="studio-account-menu" role="group" aria-label=move || label.get()>
                    <div class="studio-account-header">
                        <div class="studio-account-identity">
                            <ActorBadge actor=actor label=label size="large"/>
                            <div class="studio-account-name">
                                <p class="identity">{move || label.get()}</p>
                            </div>
                        </div>
                        <Button id="studio-account-close" class="studio-account-close" variant=ButtonVariant::Text icon=IconName::Close
                            on_press=Callback::new(move |_| {
                                open.set(false);
                                browser::focus(trigger);
                            })><span class="cui-sr-only">{move || close_label.get()}</span></Button>
                    </div>
                    <div class="studio-account-actions">{children()}</div>
                </div>
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(actor: Option<&str>, available: bool, open: bool) -> String {
        let actor = RwSignal::new(actor.map(str::to_owned));
        let open = RwSignal::new(open);
        view! {
            <AccountEntry actor=actor available=Signal::derive(move || available) open=open
                label="Tài khoản Studio" close_label="Đóng">
                <button id="probe-action" type="button">"Đăng xuất"</button>
            </AccountEntry>
        }
        .to_html()
    }

    #[test]
    fn availability_wins_over_open() {
        assert!(menu_visible(true, true));
        for (available, open) in [(true, false), (false, true), (false, false)] {
            assert!(!menu_visible(available, open));
        }
    }

    #[test]
    fn collapsed_trigger_is_a_named_disclosure_with_a_neutral_initial_and_no_popover() {
        Owner::new().with(|| {
            let html = render(Some("minh@example.test"), true, false);
            assert!(html.contains("id=\"studio-account-entry\""), "{html}");
            assert!(html.contains("aria-label=\"Tài khoản Studio\""), "{html}");
            assert!(html.contains("aria-expanded=\"false\""), "{html}");
            assert!(
                html.contains("aria-controls=\"studio-account-menu\""),
                "{html}"
            );
            assert!(html.contains("studio-avatar"), "{html}");
            assert!(html.contains(">T<"), "{html}");
            assert!(!html.contains("minh@example.test"), "{html}");
            assert!(!html.contains("id=\"studio-account-menu\""), "{html}");
            assert!(!html.contains("probe-action"), "{html}");
            assert!(
                !html.contains("<details") && !html.contains("<summary"),
                "{html}"
            );
        });
    }

    #[test]
    fn open_popover_shows_identity_and_the_shell_actions_inside_the_group() {
        Owner::new().with(|| {
            let html = render(Some("minh@example.test"), true, true);
            assert!(html.contains("aria-expanded=\"true\""), "{html}");
            let menu = html
                .split("id=\"studio-account-menu\"")
                .nth(1)
                .expect(&html);
            assert!(html.contains("role=\"group\""), "{html}");
            for required in ["Tài khoản Studio", "probe-action"] {
                assert!(menu.contains(required), "{required}: {html}");
            }
            assert!(!html.contains("minh@example.test"), "{html}");
            assert!(html.matches("data-size=\"large\"").count() == 1, "{html}");
            assert!(html.contains("class=\"studio-account-scrim\""), "{html}");
        });
    }

    #[test]
    fn unavailable_entry_is_hidden_and_never_mounts_the_popover() {
        Owner::new().with(|| {
            let html = render(Some("minh@example.test"), false, true);
            assert!(html.contains("hidden"), "{html}");
            assert!(html.contains("aria-expanded=\"false\""), "{html}");
            assert!(!html.contains("id=\"studio-account-menu\""), "{html}");
            assert!(!html.contains("probe-action"), "{html}");
        });
    }

    #[test]
    fn opaque_actor_ids_never_appear_as_names_or_initials() {
        Owner::new().with(|| {
            let html = render(Some("f1234567-1234-1234-1234-123456789abc"), true, true);
            assert!(
                !html.contains("f1234567-1234-1234-1234-123456789abc"),
                "{html}"
            );
            assert!(!html.contains(">F<"), "{html}");
            assert!(html.contains(">T<"), "{html}");
        });
    }
}
