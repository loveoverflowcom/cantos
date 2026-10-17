//! Explicit auth diagnostic fixtures. This module performs no network, provider or timer work.
//! The host mounts it only for `?mode=auth-preview`, with all live effects disabled.
use crate::{
    application::{
        account::{AccountError, AccountGeneration, AccountScope},
        account_view::{
            AccountFailure, AccountViewState, LibraryCompletion, LibraryProjection,
            SessionCompletion, SessionPhase,
        },
    },
    messages::{text, Key, Locale},
};
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewActor {
    A,
    B,
}

impl PreviewActor {
    fn scope(self) -> Result<AccountScope, AccountError> {
        let (actor, session) = match self {
            Self::A => ("auth-preview-fixture-a", 0xa1),
            Self::B => ("auth-preview-fixture-b", 0xb2),
        };
        AccountScope::from_session(actor.to_owned(), uuid::Uuid::from_u128(session).to_string())
    }
}

fn acknowledge_guest(current: &mut AccountViewState) -> Result<(), AccountError> {
    let issued = current.begin_logout()?;
    if current.finish_logout(issued, Ok(())) {
        Ok(())
    } else {
        Err(AccountError::LogoutPending)
    }
}

fn finish_fixture(
    current: &mut AccountViewState,
    issued: AccountGeneration,
    actor: PreviewActor,
) -> Result<bool, AccountError> {
    match current.finish_session(issued, Ok(actor.scope()?))? {
        SessionCompletion::Authenticated(fence) => {
            let empty = LibraryProjection {
                saved: Vec::new(),
                continuing: Vec::new(),
                history: Vec::new(),
                has_more: false,
            };
            match current.finish_library(&fence, Ok(empty))? {
                LibraryCompletion::Applied => Ok(true),
                LibraryCompletion::Ignored => Ok(false),
                LibraryCompletion::Cleanup(_) => Err(AccountError::LogoutPending),
            }
        }
        SessionCompletion::Ignored => Ok(false),
        SessionCompletion::Cleanup(_) => Err(AccountError::LogoutPending),
    }
}

pub(crate) fn guest(state: RwSignal<AccountViewState>) -> Result<(), AccountError> {
    acknowledge_guest(&mut state.write())
}

pub(crate) fn signed_in(
    state: RwSignal<AccountViewState>,
    actor: PreviewActor,
) -> Result<(), AccountError> {
    let mut current = state.write();
    let issued = current.begin_refresh()?;
    match finish_fixture(&mut current, issued, actor)? {
        true => Ok(()),
        false => Err(AccountError::WrongAccount),
    }
}

pub(crate) fn logout_pending(state: RwSignal<AccountViewState>) -> Result<(), AccountError> {
    state.write().begin_logout().map(|_| ())
}

pub(crate) fn logout_ack(state: RwSignal<AccountViewState>, success: bool) -> bool {
    let mut current = state.write();
    let issued = current.generation();
    current.finish_logout(
        issued,
        if success {
            Ok(())
        } else {
            Err(AccountFailure::Unavailable)
        },
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Outcome {
    #[default]
    Ready,
    Applied,
    Blocked(AccountError),
    Delayed,
    Ignored,
    CleanupFailed,
    Cancelled,
}

fn outcome(result: Result<(), AccountError>) -> Outcome {
    match result {
        Ok(()) => Outcome::Applied,
        Err(error) => Outcome::Blocked(error),
    }
}

fn status_key(status: Outcome) -> Key {
    match status {
        Outcome::Ready => Key::AuthPreviewReady,
        Outcome::Applied => Key::AuthPreviewApplied,
        Outcome::Blocked(AccountError::LogoutPending) => Key::AuthPreviewNeedsCleanup,
        Outcome::Blocked(AccountError::RequestPending) => Key::AuthPreviewRequestPending,
        Outcome::Blocked(
            AccountError::InvalidScope
            | AccountError::Exhausted
            | AccountError::SignedOut
            | AccountError::WrongAccount
            | AccountError::DuplicateIntent,
        ) => Key::AuthPreviewRejected,
        Outcome::Delayed => Key::AuthPreviewDelayed,
        Outcome::Ignored => Key::AuthPreviewIgnored,
        Outcome::CleanupFailed => Key::AuthPreviewCleanupFailed,
        Outcome::Cancelled => Key::AuthPreviewCancelled,
    }
}

#[component]
pub(crate) fn AuthPreviewControls(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let status = RwSignal::new(Outcome::Ready);
    let delayed = RwSignal::new(None::<AccountGeneration>);
    view! {
        <aside class="theatre-auth-preview theatre-panel" aria-labelledby="theatre-auth-preview-heading">
            <h2 id="theatre-auth-preview-heading">{move || text(Key::AuthPreviewHeading, locale.get())}</h2>
            <p>{move || text(Key::AuthPreviewNotice, locale.get())}</p>
            <details>
                <summary>{move || text(Key::AuthPreviewReady, locale.get())}</summary>
                <div class="theatre-actions">
                    <button id="theatre-preview-guest" type="button" class="theatre-action" on:click=move |_| status.set(outcome(guest(state)))>{move || text(Key::AuthPreviewGuest, locale.get())}</button>
                    <button id="theatre-preview-signed-a" type="button" class="theatre-action" on:click=move |_| status.set(outcome(signed_in(state, PreviewActor::A)))>{move || text(Key::AuthPreviewSignedA, locale.get())}</button>
                    <button id="theatre-preview-signed-b" type="button" class="theatre-action" on:click=move |_| status.set(outcome(signed_in(state, PreviewActor::B)))>{move || text(Key::AuthPreviewSignedB, locale.get())}</button>
                    <button id="theatre-preview-logout-pending" type="button" class="theatre-action" on:click=move |_| status.set(outcome(logout_pending(state)))>{move || text(Key::AuthPreviewLogoutPending, locale.get())}</button>
                    <button id="theatre-preview-cleanup-fail" type="button" class="theatre-action"
                        aria-describedby="theatre-preview-cleanup-help"
                        disabled=move || state.get().phase() != SessionPhase::LogoutPending on:click=move |_| {
                        if state.get_untracked().phase() == SessionPhase::LogoutPending {
                            let _ = logout_ack(state, false);
                            status.set(Outcome::CleanupFailed);
                        }
                    }>{move || text(Key::AuthPreviewCleanupFail, locale.get())}</button>
                    <button id="theatre-preview-cleanup-ack" type="button" class="theatre-action"
                        aria-describedby="theatre-preview-cleanup-help"
                        disabled=move || state.get().phase() != SessionPhase::LogoutPending on:click=move |_| {
                        if state.get_untracked().phase() == SessionPhase::LogoutPending {
                            status.set(if logout_ack(state, true) { Outcome::Applied } else { Outcome::Ignored });
                        }
                    }>{move || text(Key::AuthPreviewCleanupAck, locale.get())}</button>
                    <button id="theatre-preview-delay-begin" type="button" class="theatre-action" on:click=move |_| {
                        let result = state.write().begin_refresh();
                        match result {
                            Ok(issued) => { delayed.set(Some(issued)); status.set(Outcome::Delayed); }
                            Err(error) => status.set(Outcome::Blocked(error)),
                        }
                    }>{move || text(Key::AuthPreviewBeginDelayed, locale.get())}</button>
                    <button id="theatre-preview-delay-complete" type="button" class="theatre-action"
                        aria-describedby="theatre-preview-delay-help"
                        disabled=move || delayed.get().is_none() on:click=move |_| {
                            if let Some(issued) = delayed.get_untracked() {
                                delayed.set(None);
                                status.set(match finish_fixture(&mut state.write(), issued, PreviewActor::A) {
                                    Ok(true) => Outcome::Applied,
                                    Ok(false) => Outcome::Ignored,
                                    Err(error) => Outcome::Blocked(error),
                                });
                            }
                        }>{move || text(Key::AuthPreviewCompleteDelayed, locale.get())}</button>
                    <button id="theatre-preview-cancel" type="button" class="theatre-action" on:click=move |_| {
                        match guest(state) {
                            Ok(()) => { on_cancel.run(()); status.set(Outcome::Cancelled); }
                            Err(error) => status.set(Outcome::Blocked(error)),
                        }
                    }>{move || text(Key::AuthPreviewCancel, locale.get())}</button>
                </div>
                <p id="theatre-preview-cleanup-help">{move || text(Key::AuthPreviewCleanupHelp, locale.get())}</p>
                <p id="theatre-preview-delay-help">{move || text(Key::AuthPreviewDelayHelp, locale.get())}</p>
            </details>
            <p id="theatre-auth-preview-status" role="status" aria-live="polite">{move || text(status_key(status.get()), locale.get())}</p>
        </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::account_view::LibraryPhase;

    #[test]
    fn fixtures_keep_cleanup_barrier_and_real_owner_fences_for_stale_account_completion() {
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            guest(state).unwrap();
            let old_a = state.write().begin_refresh().unwrap();
            logout_pending(state).unwrap();
            assert!(!logout_ack(state, false));
            assert!(!state.get_untracked().can_login());
            assert_eq!(
                signed_in(state, PreviewActor::B),
                Err(AccountError::LogoutPending)
            );
            assert!(logout_ack(state, true));
            signed_in(state, PreviewActor::B).unwrap();
            assert_eq!(
                finish_fixture(&mut state.write(), old_a, PreviewActor::A),
                Ok(false)
            );
            assert_eq!(
                state.get_untracked().actor(),
                Some("auth-preview-fixture-b")
            );
            assert_eq!(state.get_untracked().phase(), SessionPhase::SignedIn);
            assert_eq!(state.get_untracked().library_phase(), LibraryPhase::Ready);
            assert!(state.get_untracked().library().unwrap().saved.is_empty());
        });
    }

    #[test]
    fn preview_controls_are_labeled_diagnostics_and_never_claim_registration_or_request_credentials(
    ) {
        Owner::new().with(|| {
            let html = view! {
                <AuthPreviewControls state=RwSignal::new(AccountViewState::default())
                    locale=RwSignal::new(Locale::English) on_cancel=Callback::new(|()| {})/>
            }
            .to_html();
            assert!(html.contains("Auth preview · Fixtures only"));
            assert!(html.contains("no network, Cantos ID, timers or passwords"));
            assert!(html.contains("theatre-preview-delay-complete"));
            for absent in [
                "<input",
                "<form",
                "Registration complete",
                "/api/",
                "access_token",
            ] {
                assert!(!html.contains(absent), "{absent}");
            }
        });
    }

    #[test]
    fn preview_labels_and_all_typed_outcomes_resolve_in_both_locales() {
        for (locale, heading, button) in [
            (
                Locale::Vietnamese,
                "Auth preview · Chỉ dữ liệu mẫu",
                "Trả phản hồi A đã giữ",
            ),
            (
                Locale::English,
                "Auth preview · Fixtures only",
                "Deliver held A response",
            ),
        ] {
            Owner::new().with(|| {
                let html = view! { <AuthPreviewControls state=RwSignal::new(AccountViewState::default()) locale=RwSignal::new(locale) on_cancel=Callback::new(|()| ())/ > }.to_html();
                assert!(html.contains(heading) && html.contains(button), "{html}");
            });
            for status in [
                Outcome::Ready,
                Outcome::Applied,
                Outcome::Delayed,
                Outcome::Ignored,
                Outcome::CleanupFailed,
                Outcome::Cancelled,
            ] {
                assert!(
                    !text(status_key(status), locale).trim().is_empty(),
                    "{status:?}"
                );
            }
            for error in [
                AccountError::InvalidScope,
                AccountError::Exhausted,
                AccountError::SignedOut,
                AccountError::WrongAccount,
                AccountError::DuplicateIntent,
                AccountError::LogoutPending,
                AccountError::RequestPending,
            ] {
                assert!(
                    !text(status_key(Outcome::Blocked(error)), locale)
                        .trim()
                        .is_empty(),
                    "{error:?}"
                );
            }
        }
    }
}
