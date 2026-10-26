//! Header account entry: an avatar trigger and a floating popover.
//!
//! Presentation only. The session owner (`SessionState`) stays in `view.rs`; this component owns
//! the disclosure geometry, dismissal (Escape, outside press, focus leaving) and focus return,
//! and renders whatever account actions the shell passes as children.
use cantos_ui::{avatar_initials, avatar_seed, avatar_tone, AvatarInitials};
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
}

/// App-private identity badge. The shared `Avatar` is not admitted for new adoption until it has
/// both-renderer qualification evidence (the preview catalog gate), so this renders the same
/// shared initials and tone contracts with Studio-owned markup; swap to `Avatar` once qualified.
#[component]
fn ActorBadge(#[prop(into)] actor: Signal<Option<String>>, size: &'static str) -> impl IntoView {
    let initials =
        move || avatar_initials(&actor.get().unwrap_or_default(), AvatarInitials::Single);
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
    /// Authenticated actor shown in the popover; the avatar initials derive from it.
    #[prop(into)]
    actor: Signal<Option<String>>,
    #[prop(into)] available: Signal<bool>,
    open: RwSignal<bool>,
    /// Localized accessible name of the trigger and the popover group.
    #[prop(into)]
    label: Signal<String>,
    /// Localized caption above the actor in the popover.
    #[prop(into)]
    caption: Signal<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let wrapper = NodeRef::<html::Div>::new();
    let trigger = NodeRef::<html::Button>::new();
    let name = Signal::derive(move || actor.get().unwrap_or_default());
    let visible = Memo::new(move |_| menu_visible(available.get(), open.get()));

    Effect::new(move |_| {
        if !available.get() && open.get_untracked() {
            open.set(false);
        }
    });
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::leptos_dom::helpers::window_event_listener;
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
                <ActorBadge actor=actor size="medium"/>
            </button>
            <Show when=move || visible.get()>
                <div id=MENU_ID class="studio-account-menu" role="group" aria-label=move || label.get()>
                    <div class="studio-account-identity">
                        <ActorBadge actor=actor size="large"/>
                        <div class="studio-account-name">
                            <p class="studio-account-caption">{move || caption.get()}</p>
                            <p class="identity">{move || name.get()}</p>
                        </div>
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
                label="Tài khoản Studio" caption="Actor đã đăng nhập">
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
    fn collapsed_trigger_is_a_named_disclosure_with_the_actor_initial_and_no_popover() {
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
            assert!(html.contains(">M<"), "{html}");
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
            for required in ["Actor đã đăng nhập", "minh@example.test", "probe-action"] {
                assert!(menu.contains(required), "{required}: {html}");
            }
            assert!(html.matches("data-size=\"large\"").count() == 1, "{html}");
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
    fn vietnamese_actor_uses_the_given_initial_not_a_placeholder() {
        Owner::new().with(|| {
            let html = render(Some("Ánh Nguyễn"), true, false);
            assert!(html.contains(">Á<"), "{html}");
        });
    }
}
