//! Account identity and, for a healthy guest, one optional explanation below the actions.
//! The admitted shared Avatar supplies the neutral circle; no account name/photo is fabricated.
//! The app-owned help is non-modal: a click or tap anywhere outside the group
//! closes it after it reached its target, so a pinned panel never swallows the first press on a
//! sign-in or create-account action.
use super::{
    account_menu::focus_left,
    auth_copy::{text, CopyKey},
    common::{ChevronGlyph, PersonOutlineGlyph},
};
use crate::messages::Locale;
use cantos_ui::{Avatar, AvatarSize};
use leptos::{ev, html, prelude::*};

const INFO_ID: &str = "theatre-guest-info";
const INFO_PANEL_ID: &str = "theatre-guest-info-panel";

/// How the optional explanation is open. Hover and focus reveal it; a click or tap pins it until
/// it is dismissed; Escape, an outside press and leaving the group close it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct InfoOpen {
    hovered: bool,
    focused: bool,
    pinned: bool,
}

impl InfoOpen {
    pub(crate) const fn is_open(self) -> bool {
        self.hovered || self.focused || self.pinned
    }

    pub(crate) const fn hover(self, on: bool) -> Self {
        Self {
            hovered: on,
            ..self
        }
    }

    pub(crate) const fn focus(self, on: bool) -> Self {
        Self {
            focused: on,
            ..self
        }
    }

    /// A press toggles the pinned state; closing also clears hover and focus so the toggle is visible.
    pub(crate) const fn press(self) -> Self {
        if self.pinned {
            Self {
                hovered: false,
                focused: false,
                pinned: false,
            }
        } else {
            Self {
                pinned: true,
                ..self
            }
        }
    }

    pub(crate) const fn dismiss(self) -> Self {
        Self {
            hovered: false,
            focused: false,
            pinned: false,
        }
    }

    /// A click inside the group leaves the state to its own handlers; one outside closes it.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) const fn press_at(self, inside_group: bool) -> Self {
        if inside_group {
            self
        } else {
            self.dismiss()
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::InfoOpen;
    use leptos::{ev, html, prelude::*, wasm_bindgen::JsCast, web_sys::Node};

    /// Closes the help on a click outside `wrapper` without intercepting it. It listens to the
    /// click, not the pointer-down: the in-flow panel collapsing on pointer-down would move the
    /// target away from under the pointer and lose the click. The listener is removed with the
    /// owning component.
    pub(super) fn dismiss_on_outside_press(wrapper: NodeRef<html::Div>, state: RwSignal<InfoOpen>) {
        let handle = window_event_listener(ev::click, move |event| {
            let inside = wrapper.get_untracked().is_some_and(|wrapper| {
                event
                    .target()
                    .and_then(|target| target.dyn_into::<Node>().ok())
                    .is_some_and(|node| wrapper.contains(Some(&node)))
            });
            if state.get_untracked().is_open() {
                state.update(|s| *s = s.press_at(inside));
            }
        });
        on_cleanup(move || handle.remove());
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod browser {
    use super::InfoOpen;
    use leptos::{html, prelude::*};

    pub(super) fn dismiss_on_outside_press(_: NodeRef<html::Div>, _: RwSignal<InfoOpen>) {}
}

/// Shared decorative fallback until the account projection supplies authorized profile facts.
#[component]
pub(crate) fn ProfileAvatar(#[prop(optional)] navigation: bool) -> impl IntoView {
    let size = if navigation {
        AvatarSize::Small
    } else {
        AvatarSize::Medium
    };
    view! {
        <span class="theatre-avatar theatre-profile-avatar" data-profile="true" data-navigation-avatar=navigation.then_some("true") aria-hidden="true">
            <Avatar display_name="" size accent=true/>
            <span class="theatre-profile-glyph"><PersonOutlineGlyph/></span>
        </span>
    }
}

/// The optional fact for a guest. The trigger's visible text is its accessible name; the panel is
/// in flow, so it never covers the heading or the actions above it.
#[component]
pub(crate) fn GuestInfo(locale: RwSignal<Locale>) -> impl IntoView {
    let state = RwSignal::new(InfoOpen::default());
    let wrapper = NodeRef::<html::Div>::new();
    let open = move || state.get().is_open();
    browser::dismiss_on_outside_press(wrapper, state);
    view! {
        <div class="theatre-guest-info" node_ref=wrapper
            on:mouseenter=move |_| state.update(|s| *s = s.hover(true))
            on:mouseleave=move |_| state.update(|s| *s = s.hover(false))
            on:keydown=move |event: ev::KeyboardEvent| {
                if event.key() == "Escape" && state.get_untracked().is_open() {
                    event.prevent_default();
                    state.update(|s| *s = s.dismiss());
                }
            }
            on:focusout=move |event: ev::FocusEvent| {
                if focus_left(&event, wrapper) {
                    state.update(|s| *s = s.focus(false).dismiss());
                }
            }>
            <button id=INFO_ID type="button" class="theatre-guest-info-trigger"
                aria-controls=INFO_PANEL_ID
                aria-expanded=move || open().to_string()
                on:focus=move |_| state.update(|s| *s = s.focus(true))
                on:blur=move |_| state.update(|s| *s = s.focus(false))
                on:click=move |_| state.update(|s| *s = s.press())>
                <span class="theatre-guest-info-label">{move || text(CopyKey::GuestInfoLabel, locale.get())}</span>
                <ChevronGlyph/>
            </button>
            <p id=INFO_PANEL_ID class="theatre-guest-info-panel" hidden=move || !open()>
                {move || text(CopyKey::GuestInfo, locale.get())}
            </p>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_focus_and_press_open_and_every_dismissal_closes() {
        let closed = InfoOpen::default();
        assert!(!closed.is_open());
        assert!(closed.hover(true).is_open() && !closed.hover(true).hover(false).is_open());
        assert!(closed.focus(true).is_open() && !closed.focus(true).focus(false).is_open());
        // A click pins: leaving hover and focus no longer closes it.
        let pinned = closed.hover(true).focus(true).press();
        assert!(pinned.hover(false).focus(false).is_open());
        // A second press, Escape and an outside dismissal each close it from every combination.
        assert!(!pinned.press().is_open());
        assert!(!pinned.dismiss().is_open());
        assert!(!closed.hover(true).focus(true).dismiss().is_open());
    }

    #[test]
    fn an_outside_press_closes_every_open_state_and_an_inside_press_changes_nothing() {
        let closed = InfoOpen::default();
        let open_states = [
            closed.hover(true),
            closed.focus(true),
            closed.press(),
            closed.hover(true).focus(true).press(),
        ];
        for state in open_states {
            assert!(state.is_open());
            assert_eq!(state.press_at(true), state);
            assert!(!state.press_at(false).is_open());
        }
        assert_eq!(closed.press_at(false), closed);
    }

    #[test]
    fn the_help_has_no_page_covering_layer_that_could_swallow_a_press() {
        // The pinned help once drew a fixed full-viewport scrim above the sign-in and
        // create-account actions, so their first press only closed the help.
        let css = include_str!("../styles/theatre.css");
        let source = include_str!("guest_identity.rs");
        let view = source.split("#[cfg(test)]").next().expect("view source");
        assert!(!css.contains("guest-info-scrim") && !view.contains("guest-info-scrim"));
        let trigger = css
            .split(".theatre-guest-info-trigger {")
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("trigger rule");
        assert!(!trigger.contains("z-index") && !trigger.contains("position: fixed"));
    }
}
