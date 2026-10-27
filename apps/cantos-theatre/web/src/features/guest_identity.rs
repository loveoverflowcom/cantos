//! Guest identity row of the live account card: the decorative person glyph in a tonal chip and
//! one reachable info control. App-private composition; the shared `Avatar` and `Tooltip` are not
//! used because neither offers a click/tap-pinned explanation (see the preview catalog gate).
use super::{
    account_menu::focus_left,
    auth_copy::{text, CopyKey},
    common::{InfoGlyph, PersonGlyph},
};
use crate::messages::Locale;
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

    pub(crate) const fn is_pinned(self) -> bool {
        self.pinned
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
}

#[component]
pub(crate) fn GuestIdentity(locale: RwSignal<Locale>) -> impl IntoView {
    let state = RwSignal::new(InfoOpen::default());
    let wrapper = NodeRef::<html::Div>::new();
    let open = move || state.get().is_open();
    view! {
        <div class="theatre-guest-identity" node_ref=wrapper
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
            <span class="theatre-avatar" data-size="large" data-guest="true" aria-hidden="true"><PersonGlyph/></span>
            <button id=INFO_ID type="button" class="theatre-guest-info-trigger"
                aria-label=move || text(CopyKey::GuestInfoLabel, locale.get())
                aria-controls=INFO_PANEL_ID
                aria-expanded=move || open().to_string()
                on:focus=move |_| state.update(|s| *s = s.focus(true))
                on:blur=move |_| state.update(|s| *s = s.focus(false))
                on:click=move |_| state.update(|s| *s = s.press())>
                <InfoGlyph/>
            </button>
            <Show when=move || state.get().is_pinned()>
                <div class="theatre-guest-info-scrim" aria-hidden="true"
                    on:click=move |_| state.update(|s| *s = s.dismiss())></div>
            </Show>
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
}
