//! Shell-owned logout confirmation.
//!
//! App-private composition over the shared Dialog/Button. The mounted account (`AccountViewState`)
//! stays the only auth owner: this file projects `AccountUi`, opens/closes overlays and calls the
//! existing logout callback exactly once per confirmation.
use super::account_ui::{account_ui, AccountUi};
use crate::{
    application::account_view::{AccountViewState, SessionPhase},
    messages::{self, Key, Locale},
};
use cantos_ui::{
    Button, ButtonState, ButtonVariant, Dialog, DialogCloseReason, DisabledReason, IconName,
};
use leptos::prelude::*;

/// What the confirmation shows. It is derived from the account owner, never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogoutStep {
    /// Nothing has been dispatched; confirming sends the one logout intent.
    Ask,
    /// Dispatched and cleanup is running; a repeat is refused.
    InProgress,
    /// Logout is not confirmed; the owner allows a retry. Neither success nor "all saved".
    Unconfirmed,
    /// Guest, checking or failed: no sign-out question is left to ask.
    Settled,
}

impl LogoutStep {
    pub(crate) fn of(ui: &AccountUi) -> Self {
        match ui.phase {
            SessionPhase::SignedIn => Self::Ask,
            SessionPhase::LogoutPending if ui.cleanup_in_flight => Self::InProgress,
            SessionPhase::LogoutPending if ui.can_retry_logout => Self::Unconfirmed,
            _ => Self::Settled,
        }
    }

    pub(crate) const fn can_confirm(self) -> bool {
        matches!(self, Self::Ask | Self::Unconfirmed)
    }
}

#[cfg(any(target_arch = "wasm32", test))]
/// A click on `::backdrop` is reported on the dialog element with coordinates outside its box.
pub(crate) fn outside_box(x: f64, y: f64, left: f64, top: f64, right: f64, bottom: f64) -> bool {
    x < left || x > right || y < top || y > bottom
}

#[cfg(any(target_arch = "wasm32", test))]
fn needs_focus_fallback(active_tag: Option<&str>) -> bool {
    active_tag.is_none_or(|tag| tag == "BODY")
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{needs_focus_fallback, outside_box};
    use leptos::{ev, wasm_bindgen::JsCast, web_sys::HtmlElement};

    pub(super) fn focus_by_id(id: &str) -> Option<HtmlElement> {
        let element = leptos::web_sys::window()?
            .document()?
            .get_element_by_id(id)?
            .dyn_into::<HtmlElement>()
            .ok()?;
        element.focus().ok()?;
        Some(element)
    }

    pub(super) fn focus_main_if_lost() {
        let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let active_tag = document.active_element().map(|element| element.tag_name());
        if needs_focus_fallback(active_tag.as_deref()) {
            let _ = focus_by_id("theatre-main");
        }
    }

    pub(super) fn backdrop_hit(event: &ev::MouseEvent) -> bool {
        let (Some(target), Some(current)) = (event.target(), event.current_target()) else {
            return false;
        };
        if target != current {
            return false;
        }
        let Ok(dialog) = current.dyn_into::<HtmlElement>() else {
            return false;
        };
        let rect = dialog.get_bounding_client_rect();
        outside_box(
            f64::from(event.client_x()),
            f64::from(event.client_y()),
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom(),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod browser {
    use leptos::ev;

    pub(super) fn focus_by_id(_: &str) -> Option<leptos::web_sys::HtmlElement> {
        None
    }
    pub(super) fn focus_main_if_lost() {}
    pub(super) fn backdrop_hit(_: &ev::MouseEvent) -> bool {
        false
    }
}

const CANCEL_ID: &str = "theatre-logout-cancel";

fn confirm_state(step: LogoutStep, locale: Locale) -> ButtonState {
    if step != LogoutStep::InProgress {
        return ButtonState::Enabled;
    }
    DisabledReason::new(messages::text(Key::LogoutInProgress, locale))
        .map(ButtonState::Loading)
        .unwrap_or(ButtonState::Enabled)
}

#[component]
fn LogoutDialog(
    locale: RwSignal<Locale>,
    open: RwSignal<bool>,
    step: Memo<LogoutStep>,
    on_confirm: Callback<()>,
) -> impl IntoView {
    let close = Callback::new(move |_| open.set(false));
    // The locale is read inside the closure so a language change remounts the closed dialog.
    view! {
        {move || {
            let language = locale.get();
            view! {
                <Dialog id="theatre-logout" class="theatre-logout-dialog"
                    title=messages::text(Key::LogoutTitle, language)
                    close_label=messages::text(Key::LogoutDismiss, language)
                    description=messages::text(Key::LogoutReassurance, language)
                    open=Signal::from(open)
                    on_close_request=Callback::new(move |_: DialogCloseReason| open.set(false))
                    on_error=Callback::new(move |_| open.set(false))
                    initial_focus=Callback::new(|()| browser::focus_by_id(CANCEL_ID))
                    actions=ViewFn::from(move || view! {
                        // Back leads with the arrow; while sign-out runs the same action only closes.
                        {move || if step.get() == LogoutStep::InProgress {
                            view! {
                                <Button id=CANCEL_ID variant=ButtonVariant::AuthSecondary
                                    on_press=Callback::new(move |_| close.run(()))>
                                    {messages::text(Key::LogoutDismiss, language)}
                                </Button>
                            }.into_any()
                        } else {
                            view! {
                                <Button id=CANCEL_ID variant=ButtonVariant::AuthSecondary icon=IconName::ArrowBack
                                    on_press=Callback::new(move |_| close.run(()))>
                                    {messages::text(Key::LogoutCancel, language)}
                                </Button>
                            }.into_any()
                        }}
                        <Button id="theatre-logout-confirm" variant=ButtonVariant::AuthPrimary icon=IconName::Logout
                            state=Signal::derive(move || confirm_state(step.get(), language))
                            on_press=Callback::new(move |_| on_confirm.run(()))>
                            {messages::text(Key::SignOut, language)}
                        </Button>
                    })
                    on:click=move |event| {
                        if browser::backdrop_hit(&event) {
                            open.set(false);
                        }
                    }>
                    <p id="theatre-logout-status" class="theatre-logout-status theatre-auth-feedback" data-tone="warning" role="status" aria-live="polite" aria-atomic="true">
                        {move || (step.get() == LogoutStep::Unconfirmed)
                            .then(|| messages::text(Key::LogoutUnconfirmed, language))}
                    </p>
                </Dialog>
            }
        }}
    }
}

fn confirmation_callback(
    open: RwSignal<bool>,
    step: Memo<LogoutStep>,
    submitted: RwSignal<bool>,
    on_logout: Callback<()>,
) -> Callback<()> {
    Callback::new(move |()| {
        // Reserve this confirmation before calling the owner: two activations in the same
        // browser turn must not send two intents while the owner still projects SignedIn.
        if open.get_untracked() && step.get_untracked().can_confirm() && !submitted.get_untracked()
        {
            submitted.set(true);
            on_logout.run(());
        }
    })
}

/// Keep mounted once at the shell lifetime. Me requests confirmation through `open`; only the
/// existing account projection admits confirmation or retry, and dismissal never sends logout.
#[component]
pub(crate) fn LogoutConfirmation(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    open: RwSignal<bool>,
    on_logout: Callback<()>,
) -> impl IntoView {
    let ui = account_ui(state);
    let step = Memo::new(move |_| LogoutStep::of(&ui.get()));
    let submitted = RwSignal::new(false);
    Effect::new(move |previous: Option<LogoutStep>| {
        let current = step.get();
        if previous != Some(current) {
            submitted.set(false);
        }
        if current == LogoutStep::Settled && open.get_untracked() {
            open.set(false);
        }
        current
    });
    // Dialog restores a connected opener first. Successful logout removes the Me logout
    // button, so recover only if focus is still lost after that restoration has had a turn.
    Effect::new(move |was_open: Option<bool>| {
        let is_open = open.get();
        if was_open == Some(true) && !is_open {
            set_timeout(
                move || {
                    if open.try_get_untracked() == Some(false) {
                        browser::focus_main_if_lost();
                    }
                },
                std::time::Duration::ZERO,
            );
        }
        is_open
    });
    let confirm = confirmation_callback(open, step, submitted, on_logout);
    view! { <LogoutDialog locale open step on_confirm=confirm/> }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::auth_preview::{guest, signed_in, PreviewActor};

    fn ui_of(state: RwSignal<AccountViewState>) -> AccountUi {
        account_ui(state.read_only()).get_untracked()
    }

    #[test]
    fn logout_step_follows_the_owner_and_only_ask_or_retry_may_confirm() {
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            guest(state).unwrap();
            assert_eq!(LogoutStep::of(&ui_of(state)), LogoutStep::Settled);
            signed_in(state, PreviewActor::A).unwrap();
            assert_eq!(LogoutStep::of(&ui_of(state)), LogoutStep::Ask);
            let cleanup = state.write().begin_logout().unwrap();
            let pending = LogoutStep::of(&ui_of(state));
            assert_eq!(pending, LogoutStep::InProgress);
            assert!(!pending.can_confirm(), "a pending logout is not replayed");
            state.write().finish_logout(
                cleanup,
                Err(crate::application::account_view::AccountFailure::Unavailable),
            );
            let unconfirmed = LogoutStep::of(&ui_of(state));
            assert_eq!(unconfirmed, LogoutStep::Unconfirmed);
            assert!(unconfirmed.can_confirm());
            let retry = state.write().begin_logout().unwrap();
            state.write().finish_logout(retry, Ok(()));
            assert_eq!(LogoutStep::of(&ui_of(state)), LogoutStep::Settled);
            assert!(!LogoutStep::Settled.can_confirm());
        });
    }

    #[test]
    fn backdrop_hits_are_clicks_outside_the_dialog_box_only() {
        let (l, t, r, b) = (100.0, 50.0, 580.0, 382.0);
        assert!(outside_box(10.0, 60.0, l, t, r, b));
        assert!(outside_box(200.0, 400.0, l, t, r, b));
        assert!(!outside_box(100.0, 50.0, l, t, r, b), "the edge is inside");
        assert!(!outside_box(300.0, 200.0, l, t, r, b), "padding is inside");
    }

    #[test]
    fn logout_focus_fallback_preserves_a_restored_opener_or_other_real_target() {
        assert!(needs_focus_fallback(None));
        assert!(needs_focus_fallback(Some("BODY")));
        for target in ["BUTTON", "INPUT", "A", "MAIN", "H1", "DIALOG"] {
            assert!(!needs_focus_fallback(Some(target)), "{target}");
        }
    }

    #[test]
    fn shell_confirmation_remains_mounted_for_each_account_phase() {
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            let locale = RwSignal::new(Locale::Vietnamese);
            let open = RwSignal::new(false);
            let render = || {
                view! {
                    <LogoutConfirmation state=state.read_only() locale open
                        on_logout=Callback::new(|()| {})/>
                }
                .to_html()
            };
            let assert_dialog = || {
                let html = render();
                assert_eq!(html.matches("<dialog").count(), 1);
                assert_eq!(html.matches("id=\"theatre-logout-confirm\"").count(), 1);
                assert!(!html.contains("theatre-account-entry"));
            };
            assert_dialog();
            let lookup = state.write().begin_refresh().unwrap();
            assert_dialog();
            state
                .write()
                .finish_session(
                    lookup,
                    Err(crate::application::account_view::AccountFailure::Unavailable),
                    || "unused-device".to_owned(),
                )
                .unwrap();
            assert_dialog();
            guest(state).unwrap();
            assert_dialog();
            signed_in(state, PreviewActor::A).unwrap();
            assert_dialog();
            let cleanup = state.write().begin_logout().unwrap();
            assert_dialog();
            state.write().finish_logout(
                cleanup,
                Err(crate::application::account_view::AccountFailure::Unavailable),
            );
            assert_dialog();
            let retry = state.write().begin_logout().unwrap();
            state.write().finish_logout(retry, Ok(()));
            assert_dialog();
        });
    }

    #[test]
    fn confirmation_reserves_one_intent_before_the_owner_changes_phase() {
        Owner::new().with(|| {
            let step = RwSignal::new(LogoutStep::Ask);
            let step = Memo::new(move |_| step.get());
            let open = RwSignal::new(true);
            let submitted = RwSignal::new(false);
            let intents = RwSignal::new(0);
            let confirm = confirmation_callback(
                open,
                step,
                submitted,
                Callback::new(move |()| {
                    assert!(submitted.get_untracked(), "reserve before dispatch");
                    intents.update(|count| *count += 1);
                }),
            );
            confirm.run(());
            confirm.run(());
            assert_eq!(intents.get_untracked(), 1);
            open.set(false);
            confirm.run(());
            open.set(true);
            confirm.run(());
            assert_eq!(intents.get_untracked(), 1, "dismiss/reopen never replays");
        });
    }

    #[test]
    fn closed_pending_and_settled_confirmations_emit_no_intent() {
        Owner::new().with(|| {
            for (is_open, owner_step) in [
                (false, LogoutStep::Ask),
                (false, LogoutStep::Unconfirmed),
                (true, LogoutStep::InProgress),
                (true, LogoutStep::Settled),
            ] {
                let open = RwSignal::new(is_open);
                let step = Memo::new(move |_| owner_step);
                let submitted = RwSignal::new(false);
                let intents = RwSignal::new(0);
                confirmation_callback(
                    open,
                    step,
                    submitted,
                    Callback::new(move |()| intents.update(|count| *count += 1)),
                )
                .run(());
                assert_eq!(intents.get_untracked(), 0, "{owner_step:?}/{is_open}");
                assert!(!submitted.get_untracked());
            }
        });
    }

    const ARROW_BACK_PATH: &str = "M20 11H7.83l5.59-5.59";

    fn cancel_label(language: Locale) -> &'static str {
        messages::text(Key::LogoutCancel, language)
    }

    #[test]
    fn the_back_button_is_named_back() {
        assert_eq!(cancel_label(Locale::Vietnamese), "Quay lại");
        assert_eq!(cancel_label(Locale::English), "Back");
    }

    #[test]
    fn confirmation_has_localized_actions_and_a_decorative_leading_back_arrow() {
        Owner::new().with(|| {
            for language in [Locale::Vietnamese, Locale::English] {
                let locale = RwSignal::new(language);
                let open = RwSignal::new(false);
                let step = Memo::new(|_| LogoutStep::Ask);
                let html = view! {
                    <LogoutDialog locale open step on_confirm=Callback::new(|()| {})/>
                }
                .to_html();
                assert_eq!(html.matches("<dialog").count(), 1);
                assert!(!html.contains("<dialog open"));
                for key in [Key::LogoutTitle, Key::LogoutReassurance, Key::SignOut] {
                    assert!(html.contains(messages::text(key, language)), "{key:?}");
                }
                let cancel = html.split("id=\"theatre-logout-cancel\"").nth(1).unwrap();
                let cancel = &cancel[..cancel.find("</button>").unwrap()];
                assert!(
                    cancel.find("cui-button-icon").unwrap()
                        < cancel.find(cancel_label(language)).unwrap(),
                    "the arrow leads the label"
                );
                assert!(cancel.contains(ARROW_BACK_PATH));
                assert!(
                    !cancel.contains("aria-label"),
                    "the visible text names Back"
                );
            }
        });
    }

    #[test]
    fn pending_confirm_is_loading_with_a_visible_reason_and_other_steps_are_enabled() {
        for locale in [Locale::Vietnamese, Locale::English] {
            let state = confirm_state(LogoutStep::InProgress, locale);
            assert!(!state.can_press() && state.is_busy());
            assert_eq!(
                state.reason().unwrap().as_str(),
                messages::text(Key::LogoutInProgress, locale)
            );
            for step in [
                LogoutStep::Ask,
                LogoutStep::Unconfirmed,
                LogoutStep::Settled,
            ] {
                assert!(confirm_state(step, locale).can_press());
            }
        }
    }
}
