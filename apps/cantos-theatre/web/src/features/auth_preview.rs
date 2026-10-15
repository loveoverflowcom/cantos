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
    messages::Locale,
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

#[derive(Clone, Copy, Debug)]
enum Copy {
    Heading,
    Notice,
    Guest,
    SignedA,
    SignedB,
    LogoutPending,
    CleanupFail,
    CleanupAck,
    BeginDelayed,
    CompleteDelayed,
    Cancel,
    CleanupHelp,
    DelayHelp,
    Ready,
    Applied,
    NeedsCleanup,
    RequestPending,
    Rejected,
    Delayed,
    Ignored,
    CleanupFailed,
    Cancelled,
}

fn text(key: Copy, locale: Locale) -> &'static str {
    match (key, locale) {
        (Copy::Heading, Locale::Vietnamese) => "Auth preview · Chỉ dữ liệu mẫu",
        (Copy::Heading, Locale::English) => "Auth preview · Fixtures only",
        (Copy::Notice, Locale::Vietnamese) => "Công cụ kiểm tra riêng: không gọi mạng, Cantos ID, timer hoặc mật khẩu. A/B là tài khoản mẫu; không chứng nhận đăng nhập hoặc đăng ký thực.",
        (Copy::Notice, Locale::English) => "Isolated diagnostics: no network, Cantos ID, timers or passwords. A/B are fixture accounts; this does not verify real sign-in or registration.",
        (Copy::Guest, Locale::Vietnamese) => "Mẫu khách · cleanup đã xác nhận",
        (Copy::Guest, Locale::English) => "Guest fixture · cleanup acknowledged",
        (Copy::SignedA, Locale::Vietnamese) => "Mẫu đăng nhập A",
        (Copy::SignedA, Locale::English) => "Signed-in fixture A",
        (Copy::SignedB, Locale::Vietnamese) => "Mẫu đăng nhập B",
        (Copy::SignedB, Locale::English) => "Signed-in fixture B",
        (Copy::LogoutPending, Locale::Vietnamese) => "Bắt đầu mẫu đăng xuất",
        (Copy::LogoutPending, Locale::English) => "Begin logout fixture",
        (Copy::CleanupFail, Locale::Vietnamese) => "Mẫu cleanup thất bại",
        (Copy::CleanupFail, Locale::English) => "Fail cleanup fixture",
        (Copy::CleanupAck, Locale::Vietnamese) => "Xác nhận cleanup mẫu",
        (Copy::CleanupAck, Locale::English) => "Acknowledge cleanup fixture",
        (Copy::BeginDelayed, Locale::Vietnamese) => "Giữ phản hồi phiên A",
        (Copy::BeginDelayed, Locale::English) => "Hold session A response",
        (Copy::CompleteDelayed, Locale::Vietnamese) => "Trả phản hồi A đã giữ",
        (Copy::CompleteDelayed, Locale::English) => "Deliver held A response",
        (Copy::Cancel, Locale::Vietnamese) => "Hủy yêu cầu mẫu và rời auth",
        (Copy::Cancel, Locale::English) => "Cancel fixture request and leave auth",
        (Copy::CleanupHelp, Locale::Vietnamese) => "Bắt đầu mẫu đăng xuất trước khi thử phản hồi cleanup; xác nhận thành công mới mở lại Login.",
        (Copy::CleanupHelp, Locale::English) => "Begin the logout fixture before delivering cleanup responses; only acknowledgement re-enables Login.",
        (Copy::DelayHelp, Locale::Vietnamese) => "Giữ phản hồi A trước khi trả phản hồi. Sau khi hủy hoặc chuyển khách → B, phản hồi A cũ phải được bỏ qua.",
        (Copy::DelayHelp, Locale::English) => "Hold A before delivering its response. After cancellation or a Guest → B switch, the old A response must be ignored.",
        (Copy::Ready, Locale::Vietnamese) => "Sẵn sàng kiểm tra trạng thái mẫu.",
        (Copy::Ready, Locale::English) => "Ready to inspect fixture states.",
        (Copy::Applied, Locale::Vietnamese) => "Đã áp dụng chuyển trạng thái mẫu qua AccountViewState.",
        (Copy::Applied, Locale::English) => "Fixture transition applied through AccountViewState.",
        (Copy::NeedsCleanup, Locale::Vietnamese) => "Core chặn thao tác: phải xác nhận cleanup trước.",
        (Copy::NeedsCleanup, Locale::English) => "Core refused the action: acknowledge cleanup first.",
        (Copy::RequestPending, Locale::Vietnamese) => "Core chặn yêu cầu trùng trong khi phản hồi cũ còn chờ.",
        (Copy::RequestPending, Locale::English) => "Core refused a duplicate request while a response is pending.",
        (Copy::Rejected, Locale::Vietnamese) => "Core từ chối chuyển trạng thái mẫu; phiên hiện tại được giữ.",
        (Copy::Rejected, Locale::English) => "Core refused the fixture transition; the current session is retained.",
        (Copy::Delayed, Locale::Vietnamese) => "Đã giữ generation của yêu cầu A. Hủy hoặc chuyển khách → B rồi trả A để kiểm tra phản hồi trễ.",
        (Copy::Delayed, Locale::English) => "Captured request A's generation. Cancel or switch Guest → B, then deliver A to inspect a stale response.",
        (Copy::Ignored, Locale::Vietnamese) => "Core đã bỏ qua phản hồi A cũ; trạng thái hiện tại được giữ.",
        (Copy::Ignored, Locale::English) => "Core ignored the old A response; the current state is retained.",
        (Copy::CleanupFailed, Locale::Vietnamese) => "Cleanup mẫu chưa xác nhận; Login vẫn bị khóa, có thể thử hoàn tất đăng xuất.",
        (Copy::CleanupFailed, Locale::English) => "Fixture cleanup remains unacknowledged; Login stays blocked and sign-out can be retried.",
        (Copy::Cancelled, Locale::Vietnamese) => "Đã hủy generation mẫu và rời auth. Phản hồi A đã giữ có thể được trả để kiểm tra việc bỏ qua.",
        (Copy::Cancelled, Locale::English) => "Fixture generation cancelled and auth closed. Deliver the held A response to inspect rejection.",
    }
}

fn status_key(status: Outcome) -> Copy {
    match status {
        Outcome::Ready => Copy::Ready,
        Outcome::Applied => Copy::Applied,
        Outcome::Blocked(AccountError::LogoutPending) => Copy::NeedsCleanup,
        Outcome::Blocked(AccountError::RequestPending) => Copy::RequestPending,
        Outcome::Blocked(
            AccountError::InvalidScope
            | AccountError::Exhausted
            | AccountError::SignedOut
            | AccountError::WrongAccount
            | AccountError::DuplicateIntent,
        ) => Copy::Rejected,
        Outcome::Delayed => Copy::Delayed,
        Outcome::Ignored => Copy::Ignored,
        Outcome::CleanupFailed => Copy::CleanupFailed,
        Outcome::Cancelled => Copy::Cancelled,
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
            <h2 id="theatre-auth-preview-heading">{move || text(Copy::Heading, locale.get())}</h2>
            <p>{move || text(Copy::Notice, locale.get())}</p>
            <details>
                <summary>{move || text(Copy::Ready, locale.get())}</summary>
                <div class="theatre-actions">
                    <button id="theatre-preview-guest" type="button" class="theatre-action" on:click=move |_| status.set(outcome(guest(state)))>{move || text(Copy::Guest, locale.get())}</button>
                    <button id="theatre-preview-signed-a" type="button" class="theatre-action" on:click=move |_| status.set(outcome(signed_in(state, PreviewActor::A)))>{move || text(Copy::SignedA, locale.get())}</button>
                    <button id="theatre-preview-signed-b" type="button" class="theatre-action" on:click=move |_| status.set(outcome(signed_in(state, PreviewActor::B)))>{move || text(Copy::SignedB, locale.get())}</button>
                    <button id="theatre-preview-logout-pending" type="button" class="theatre-action" on:click=move |_| status.set(outcome(logout_pending(state)))>{move || text(Copy::LogoutPending, locale.get())}</button>
                    <button id="theatre-preview-cleanup-fail" type="button" class="theatre-action"
                        aria-describedby="theatre-preview-cleanup-help"
                        disabled=move || state.get().phase() != SessionPhase::LogoutPending on:click=move |_| {
                        if state.get_untracked().phase() == SessionPhase::LogoutPending {
                            let _ = logout_ack(state, false);
                            status.set(Outcome::CleanupFailed);
                        }
                    }>{move || text(Copy::CleanupFail, locale.get())}</button>
                    <button id="theatre-preview-cleanup-ack" type="button" class="theatre-action"
                        aria-describedby="theatre-preview-cleanup-help"
                        disabled=move || state.get().phase() != SessionPhase::LogoutPending on:click=move |_| {
                        if state.get_untracked().phase() == SessionPhase::LogoutPending {
                            status.set(if logout_ack(state, true) { Outcome::Applied } else { Outcome::Ignored });
                        }
                    }>{move || text(Copy::CleanupAck, locale.get())}</button>
                    <button id="theatre-preview-delay-begin" type="button" class="theatre-action" on:click=move |_| {
                        let result = state.write().begin_refresh();
                        match result {
                            Ok(issued) => { delayed.set(Some(issued)); status.set(Outcome::Delayed); }
                            Err(error) => status.set(Outcome::Blocked(error)),
                        }
                    }>{move || text(Copy::BeginDelayed, locale.get())}</button>
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
                        }>{move || text(Copy::CompleteDelayed, locale.get())}</button>
                    <button id="theatre-preview-cancel" type="button" class="theatre-action" on:click=move |_| {
                        match guest(state) {
                            Ok(()) => { on_cancel.run(()); status.set(Outcome::Cancelled); }
                            Err(error) => status.set(Outcome::Blocked(error)),
                        }
                    }>{move || text(Copy::Cancel, locale.get())}</button>
                </div>
                <p id="theatre-preview-cleanup-help">{move || text(Copy::CleanupHelp, locale.get())}</p>
                <p id="theatre-preview-delay-help">{move || text(Copy::DelayHelp, locale.get())}</p>
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
}
