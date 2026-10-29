//! Why Listen has nothing to play, in two tiers: the short blocker stays on the control as its
//! `ButtonState` reason, and this disclosure holds the optional detail. A native disabled button
//! takes no focus or press, so the explanation has its own named trigger; the essential reason is
//! never reachable only through it.
use crate::{
    application::public_work::PageStatus,
    messages::{text, Key, Locale},
};
use leptos::{ev, html, prelude::*};

pub(crate) const HELP_ID: &str = "theatre-work-listen-help";
const TRIGGER_ID: &str = "theatre-work-listen-help-trigger";

/// Why the first-unit lookup produced no Listen target. Loading and failure are real states with
/// their own copy and recovery; only the two "nothing to play" conclusions carry detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListenAbsence {
    Loading,
    Failed,
    /// The whole list was read and no public unit has audio.
    Unavailable,
    /// More of the list exists, so the absence is only of the part loaded so far.
    NoTarget,
}

impl ListenAbsence {
    pub(crate) fn of(status: Option<&PageStatus>, list_complete: bool) -> Self {
        match status {
            Some(PageStatus::Loading) => Self::Loading,
            Some(PageStatus::Failed(_)) => Self::Failed,
            _ if list_complete => Self::Unavailable,
            _ => Self::NoTarget,
        }
    }

    pub(crate) const fn reason(self) -> Key {
        match self {
            Self::Loading => Key::PublicWorkUnitsLoading,
            Self::Failed => Key::PublicWorkUnitsFailed,
            Self::Unavailable => Key::WorkListenUnavailable,
            Self::NoTarget => Key::WorkListenNoTarget,
        }
    }

    pub(crate) const fn is_busy(self) -> bool {
        matches!(self, Self::Loading)
    }

    pub(crate) const fn detail(self) -> Option<Key> {
        match self {
            Self::Unavailable => Some(Key::WorkListenUnavailableHelp),
            Self::NoTarget => Some(Key::WorkListenNoTargetHelp),
            Self::Loading | Self::Failed => None,
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn focus(trigger: NodeRef<html::Button>) {
    use leptos::wasm_bindgen::JsCast;
    if let Some(trigger) = trigger.get_untracked() {
        let _ = trigger
            .unchecked_into::<leptos::web_sys::HtmlElement>()
            .focus();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn focus(_trigger: NodeRef<html::Button>) {}

/// Only a focus move to a real element outside the disclosure closes it; a click on
/// non-focusable padding reports no related target and is closed by the scrim instead.
#[cfg(target_arch = "wasm32")]
fn focus_left(event: &ev::FocusEvent, wrapper: NodeRef<html::Div>) -> bool {
    use leptos::{wasm_bindgen::JsCast, web_sys::Node};
    let (Some(wrapper), Some(related)) = (wrapper.get_untracked(), event.related_target()) else {
        return false;
    };
    related
        .dyn_ref::<Node>()
        .is_some_and(|node| !wrapper.contains(Some(node)))
}

#[cfg(not(target_arch = "wasm32"))]
fn focus_left(_event: &ev::FocusEvent, _wrapper: NodeRef<html::Div>) -> bool {
    false
}

/// A named text trigger and an absolutely placed note, so opening never shifts the layout
/// beneath. Keyboard and touch use the same button; Escape, the scrim and leaving focus dismiss.
#[component]
pub(crate) fn ListenHelp(
    locale: RwSignal<Locale>,
    /// The detail to show, or `None` when the control has no optional explanation.
    detail: Signal<Option<Key>>,
    /// The ID of the control's visible reason, so the trigger reads as "Why? <reason>".
    reason_id: String,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let wrapper = NodeRef::<html::Div>::new();
    let trigger = NodeRef::<html::Button>::new();
    // The note belongs to one conclusion; when it changes or goes away the note must not linger.
    Effect::new(move |_| {
        if detail.get().is_none() {
            open.set(false);
        }
    });
    view! {
        <Show when=move || detail.get().is_some()>
            <div class="theatre-listen-help" node_ref=wrapper
                on:keydown=move |event: ev::KeyboardEvent| {
                    if event.key() == "Escape" && open.get_untracked() {
                        event.prevent_default();
                        open.set(false);
                        focus(trigger);
                    }
                }
                on:focusout=move |event: ev::FocusEvent| {
                    if focus_left(&event, wrapper) {
                        open.set(false);
                    }
                }>
                <button id=TRIGGER_ID node_ref=trigger type="button" class="theatre-listen-help-trigger"
                    aria-controls=HELP_ID aria-describedby=reason_id.clone()
                    aria-expanded=move || open.get().to_string()
                    on:click=move |_| open.update(|open| *open = !*open)>
                    {move || text(Key::WorkListenHelpLabel, locale.get())}
                </button>
                <Show when=move || open.get()>
                    <div class="theatre-listen-help-scrim" aria-hidden="true" on:click=move |_| open.set(false)></div>
                    <p id=HELP_ID class="theatre-listen-help-note" role="note">
                        {move || detail.get().map(|key| text(key, locale.get())).unwrap_or_default()}
                    </p>
                </Show>
            </div>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::public_work::ReadFailure;

    #[test]
    fn absence_separates_loading_failure_and_the_two_conclusions() {
        assert_eq!(
            ListenAbsence::of(Some(&PageStatus::Loading), true),
            ListenAbsence::Loading
        );
        assert_eq!(
            ListenAbsence::of(Some(&PageStatus::Failed(ReadFailure::Unavailable)), false),
            ListenAbsence::Failed
        );
        assert_eq!(
            ListenAbsence::of(Some(&PageStatus::Idle), true),
            ListenAbsence::Unavailable
        );
        assert_eq!(ListenAbsence::of(None, false), ListenAbsence::NoTarget);
    }

    #[test]
    fn only_a_settled_absence_has_detail_and_loading_alone_is_busy() {
        for absence in [ListenAbsence::Loading, ListenAbsence::Failed] {
            assert_eq!(absence.detail(), None);
        }
        for absence in [ListenAbsence::Unavailable, ListenAbsence::NoTarget] {
            assert!(absence.detail().is_some());
            assert!(!absence.is_busy());
        }
        assert!(ListenAbsence::Loading.is_busy());
    }

    #[test]
    fn the_short_reason_is_never_the_long_detail() {
        for locale in Locale::supported() {
            for absence in [ListenAbsence::Unavailable, ListenAbsence::NoTarget] {
                let reason = text(absence.reason(), locale);
                let detail = text(absence.detail().unwrap(), locale);
                assert!(reason.chars().count() <= 40, "{reason}");
                assert_ne!(reason, detail);
            }
        }
    }
}
