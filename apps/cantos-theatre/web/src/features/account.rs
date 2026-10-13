//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::{context::TheatreContext, routes::Route},
    application::account_view::{AccountFailure, AccountViewState, LibraryPhase, SessionPhase},
    messages::{self, Key, Locale},
};
use leptos::prelude::*;

/// A fact the mock cannot yet provide. It is text, never a control: nothing here pretends to work.
#[component]
fn StatusRow(ctx: TheatreContext, label: Key, help: Key, value: Key) -> impl IntoView {
    view! {
        <div class="theatre-row">
            <dt class="theatre-row-label">{move || ctx.text(label)}</dt>
            <dd class="theatre-row-value">{move || ctx.text(value)}</dd>
            <dd class="theatre-row-help theatre-muted">{move || ctx.text(help)}</dd>
        </div>
    }
}

#[component]
fn LinkRow(
    ctx: TheatreContext,
    route: Route,
    asset: &'static str,
    label: Key,
    help: Key,
) -> impl IntoView {
    view! {
        <li>
            <a class="theatre-row theatre-row-link" href=route.hash()>
                {icon(asset)}
                <span class="theatre-row-copy">
                    <span class="theatre-row-label">{move || ctx.text(label)}</span>
                    <span class="theatre-row-help theatre-muted">{move || ctx.text(help)}</span>
                </span>
            </a>
        </li>
    }
}

/// Identity first, then what the account can and cannot do today. State is the mock's own:
/// there is no sign-in, sync or conflict model, so none is drawn.
#[component]
pub(crate) fn Account(ctx: TheatreContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Account/>
        <div class="theatre-account">
            <div class="theatre-panel theatre-account-identity">
                <span class="theatre-avatar" data-size="large" aria-hidden="true"><PersonGlyph/></span>
                <div class="theatre-account-name">
                    <h2>{move || ctx.text(Key::GuestListener)}</h2>
                    <p class="theatre-muted">{move || ctx.text(Key::GuestStatus)}</p>
                </div>
            </div>
            <div class="theatre-account-content">
                <div class="theatre-panel">
                    <section aria-labelledby="theatre-account-status">
                        <h2 id="theatre-account-status" class="theatre-group-heading">{move || ctx.text(Key::AccountStatusHeading)}</h2>
                        <dl class="theatre-rows">
                            <StatusRow ctx=ctx label=Key::SignIn help=Key::SignInHelp value=Key::NotIntegrated/>
                            <StatusRow ctx=ctx label=Key::Sync help=Key::SyncHelp value=Key::NotIntegrated/>
                            <StatusRow ctx=ctx label=Key::StorageLabel help=Key::StorageHelp value=Key::StorageValue/>
                        </dl>
                    </section>
                </div>
                <div class="theatre-panel">
                    <section aria-labelledby="theatre-account-links">
                        <h2 id="theatre-account-links" class="theatre-group-heading">{move || ctx.text(Key::AccountShortcutsHeading)}</h2>
                        <ul class="theatre-rows">
                            <LinkRow ctx=ctx route=Route::Settings asset="settings" label=Key::Settings help=Key::SettingsLinkHelp/>
                            <LinkRow ctx=ctx route=Route::Library asset="library_books" label=Key::Library help=Key::LibraryLinkHelp/>
                        </ul>
                    </section>
                </div>
            </div>
        </div>
    }
}

pub(crate) fn account_failure_key(failure: AccountFailure) -> Key {
    match failure {
        AccountFailure::SignedOut => Key::LiveGuestStatus,
        AccountFailure::SessionChanged => Key::AccountSessionChanged,
        AccountFailure::Unavailable => Key::AccountUnavailable,
        AccountFailure::Protocol => Key::AccountProtocol,
        AccountFailure::Navigation => Key::AccountNavigation,
        AccountFailure::LogoutPending => Key::AccountLogoutRetryHelp,
    }
}

fn live_status_key(state: &AccountViewState) -> Key {
    match state.phase() {
        SessionPhase::Idle | SessionPhase::Checking => Key::AccountChecking,
        SessionPhase::Guest => Key::LiveGuestStatus,
        SessionPhase::SignedIn => Key::SignedInListener,
        SessionPhase::Failed(failure) => account_failure_key(failure),
        SessionPhase::LogoutPending => {
            if state.cleanup_in_flight() {
                Key::AccountLogoutPending
            } else {
                Key::AccountLogoutRetryHelp
            }
        }
    }
}

#[component]
pub(crate) fn LiveAccount(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    view! {
        <section id="account" class="theatre-account" aria-labelledby="theatre-live-account-heading">
            <div class="theatre-panel theatre-account-identity">
                <span class="theatre-avatar" data-size="large" aria-hidden="true"><PersonGlyph/></span>
                <div class="theatre-account-name">
                    <h2 id="theatre-live-account-heading">{move || messages::text(Key::Account, locale.get())}</h2>
                    <p id="theatre-live-account-status" role="status" aria-live="polite">
                        {move || messages::text(live_status_key(&state.get()), locale.get())}
                    </p>
                    {move || state.get().actor().map(|actor| view! {
                        <p class="theatre-muted">{messages::format(Key::AccountReference, locale.get(), &[("actor", actor.to_owned())])}</p>
                    })}
                </div>
            </div>
            <div class="theatre-account-content theatre-panel">
                <div class="theatre-actions">
                    <Show when=move || state.get().can_login()>
                        <button type="button" class="theatre-action" on:click=move |_| {
                            #[cfg(target_arch = "wasm32")]
                            crate::live::runtime::login(state);
                        }><span class="cui-control-label">{move || messages::text(Key::SignIn, locale.get())}</span></button>
                    </Show>
                    <Show when=move || state.get().phase() == SessionPhase::SignedIn>
                        <button type="button" class="theatre-action" on:click=move |_| {
                            #[cfg(target_arch = "wasm32")]
                            crate::live::runtime::logout(state);
                        }><span class="cui-control-label">{move || messages::text(Key::SignOut, locale.get())}</span></button>
                    </Show>
                    <Show when=move || state.get().can_retry_logout()>
                        <button type="button" class="theatre-action" on:click=move |_| {
                            #[cfg(target_arch = "wasm32")]
                            crate::live::runtime::logout(state);
                        }><span class="cui-control-label">{move || messages::text(Key::AccountLogoutRetry, locale.get())}</span></button>
                    </Show>
                    <button type="button" class="theatre-action theatre-action-tonal"
                        aria-describedby="theatre-live-account-status"
                        disabled=move || {
                            let current = state.get();
                            matches!(current.phase(), SessionPhase::Checking | SessionPhase::LogoutPending)
                                || current.library_phase() == LibraryPhase::Loading
                        }
                        on:click=move |_| {
                            #[cfg(target_arch = "wasm32")]
                            crate::live::runtime::refresh(state);
                        }><span class="cui-control-label">{move || messages::text(Key::AccountRefresh, locale.get())}</span></button>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_account_is_identity_then_truthful_status_then_existing_links() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let html = view! { <Account ctx=ctx/> }.to_html();
            assert_eq!(html.matches("<h1").count(), 1);
            assert!(html.contains("theatre-avatar") && html.contains("Người nghe khách"));
            assert!(html.contains("Chưa đăng nhập"));
            for status in [
                "Đăng nhập",
                "Đồng bộ",
                "Chưa tích hợp",
                "Chỉ trong phiên này",
            ] {
                assert!(html.contains(status), "{status}");
            }
            for href in ["href=\"#settings\"", "href=\"#library\""] {
                assert!(html.contains(href), "{href}");
            }
            assert!(html.contains("Cài đặt nghe") && html.contains("Của tôi"));
            // No control pretends to work, and no progress-conflict choice exists in this domain.
            assert!(!html.contains("<button") && !html.contains("disabled"));
            for conflict in ["xung đột", "Giữ bản", "conflict"] {
                assert!(!html.contains(conflict), "{conflict}");
            }
        });
    }

    #[test]
    fn guest_account_localizes_without_reusing_vietnamese_in_english() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.locale.set(crate::messages::Locale::English);
            let html = view! { <Account ctx=ctx/> }.to_html();
            for text in [
                "Guest listener",
                "Not integrated",
                "This session only",
                "Listening settings",
            ] {
                assert!(html.contains(text), "{text}");
            }
            assert!(!html.contains("Người nghe khách"));
        });
    }

    #[test]
    fn live_guest_offers_login_only_after_cookie_cleanup_and_pending_logout_explains_retry() {
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            let locale = RwSignal::new(Locale::Vietnamese);
            let issued = state.write().begin_refresh().unwrap();
            let crate::application::account_view::SessionCompletion::Cleanup(cleanup) = state
                .write()
                .finish_session(issued, Err(AccountFailure::SignedOut))
                .unwrap()
            else {
                panic!("401 requires cleanup")
            };
            let pending = view! { <LiveAccount state locale/> }.to_html();
            assert!(pending.contains("Đang đăng xuất"));
            assert!(!pending.contains("Đăng nhập"));
            state
                .write()
                .finish_logout(cleanup, Err(AccountFailure::Unavailable));
            let retry = view! { <LiveAccount state locale/> }.to_html();
            assert!(retry.contains("Hoàn tất đăng xuất"));
            assert!(!retry.contains("Đăng nhập"));
            state.write().finish_logout(cleanup, Ok(()));
            let guest = view! { <LiveAccount state locale/> }.to_html();
            assert!(guest.contains("Đăng nhập"));
            for forbidden in ["Cantos Theatre", "access_token", "session_id", "/studio"] {
                assert!(!guest.contains(forbidden), "{forbidden}");
            }
        });
    }
}
