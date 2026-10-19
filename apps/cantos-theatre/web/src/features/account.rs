//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use super::{
    auth::{label, AuthActions, SessionRecovery},
    auth_copy::CopyKey,
};
#[cfg(feature = "preview")]
use crate::app::{context::TheatreContext, routes::Route};
use crate::{
    application::account_view::{AccountFailure, AccountViewState, SessionPhase},
    messages::{self, Key, Locale},
};
use cantos_ui::{Button, ButtonVariant, Space, Surface, SurfaceTone, SurfaceVariant, Theme};
use leptos::prelude::*;

#[cfg(feature = "preview")]
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

#[cfg(feature = "preview")]
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

#[cfg(feature = "preview")]
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
                            <li><a class="theatre-row theatre-row-link" href="?mode=membership-preview">{move || ctx.text(Key::MembershipPreview)}</a></li>
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
        AccountFailure::PopupBlocked => Key::AccountPopupBlocked,
        AccountFailure::LogoutPending => Key::AccountLogoutRetryHelp,
        AccountFailure::CursorExpired => Key::LiveLibraryCursorExpired,
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
    registration: RwSignal<crate::application::registration::RegistrationState>,
    on_registration_refresh: Callback<()>,
    theme: RwSignal<Theme>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    on_logout: Callback<()>,
    on_refresh: Callback<()>,
    on_membership: Callback<()>,
) -> impl IntoView {
    view! {
        <section id="account" aria-labelledby="theatre-live-account-heading">
            <header class="theatre-page-heading">
                <p class="theatre-eyebrow">{label(locale, CopyKey::AccountEyebrow)}</p>
                <h1 id="theatre-live-account-heading">{label(locale, CopyKey::AccountHeading)}</h1>
            </header>
            <div class="theatre-account">
                <Surface tone=SurfaceTone::PrimaryContainer variant=SurfaceVariant::Profile inset=Space::Lg class="theatre-account-profile">
                    <span class="theatre-avatar" data-size="large" aria-hidden="true"><PersonGlyph/></span>
                    <h2>{move || super::auth_copy::text(
                        if state.get().phase() == SessionPhase::SignedIn { CopyKey::SignedHeading } else { CopyKey::GuestHeading }, locale.get())}</h2>
                    <p id="theatre-live-account-status" role="status" aria-live="polite">
                        {move || messages::text(live_status_key(&state.get()), locale.get())}
                    </p>
                    <p>{move || super::auth_copy::text(
                        if state.get().phase() == SessionPhase::SignedIn { CopyKey::SignedHelp } else { CopyKey::GuestHelp }, locale.get())}</p>
                    <Show when=move || state.get().can_authenticate()>
                        <AuthActions state registration locale scope="theatre-account" on_login on_register on_configuration_retry=on_registration_refresh/>
                    </Show>
                    <Show when=move || state.get().phase() == SessionPhase::SignedIn>
                        <a class="theatre-action theatre-account-library-link" href="#library">{label(locale, CopyKey::OpenLibrary)}</a>
                    </Show>
                    <SessionRecovery state locale scope="theatre-account" on_refresh on_logout/>
                    <Show when=move || {
                            let current = state.get();
                            !matches!(current.phase(), SessionPhase::Idle | SessionPhase::Checking | SessionPhase::LogoutPending | SessionPhase::Failed(_))
                    }>
                        <Button id="theatre-account-refresh-current" variant=ButtonVariant::Text
                            on_press=Callback::new(move |_| on_refresh.run(()))>
                            {move || messages::text(Key::AccountRefresh, locale.get())}
                        </Button>
                    </Show>
                </Surface>
                <div class="theatre-account-content">
                    <Surface tone=SurfaceTone::Container variant=SurfaceVariant::Membership inset=Space::Lg class="theatre-account-membership">
                        <section aria-labelledby="theatre-account-membership-heading">
                            <p class="theatre-eyebrow">{label(locale, CopyKey::MembershipName)}</p>
                            <h2 id="theatre-account-membership-heading">{label(locale, CopyKey::MembershipHeading)}</h2>
                            <p>{label(locale, CopyKey::MembershipHelp)}</p>
                            <p class="theatre-muted">{label(locale, CopyKey::MembershipUnknown)}</p>
                            <Button id="theatre-account-membership" variant=ButtonVariant::AuthSecondary
                                focus_on_press=true
                                on_press=Callback::new(move |_| on_membership.run(()))>
                                {label(locale, CopyKey::MembershipLearn)}
                            </Button>
                        </section>
                    </Surface>
                    <Surface tone=SurfaceTone::Plain variant=SurfaceVariant::AccountGroup inset=Space::Lg class="theatre-account-preferences">
                        <section aria-labelledby="theatre-account-preferences-heading">
                            <h2 id="theatre-account-preferences-heading" class="theatre-group-heading">{label(locale, CopyKey::PreferencesHeading)}</h2>
                            <div class="theatre-account-theme">
                                <label for="theatre-account-theme">{move || messages::text(Key::Theme, locale.get())}</label>
                                <select id="theatre-account-theme" aria-describedby="theatre-account-theme-help"
                                    prop:value=move || theme.get().as_str()
                                    on:change=move |event| {
                                        if let Some(selected) = Theme::from_attribute(&event_target_value(&event)) { theme.set(selected); }
                                    }>
                                    <option value="system">{move || messages::text(Key::System, locale.get())}</option>
                                    <option value="light">{move || messages::text(Key::Light, locale.get())}</option>
                                    <option value="dark">{move || messages::text(Key::Dark, locale.get())}</option>
                                </select>
                                <p id="theatre-account-theme-help" class="theatre-muted">{label(locale, CopyKey::ThemeHelp)}</p>
                            </div>
                            <ul class="theatre-account-links">
                                <li><a href="#library">{label(locale, CopyKey::OpenLibrary)}</a></li>
                                <li><a href="#settings">{label(locale, CopyKey::SettingsHelp)}</a></li>
                            </ul>
                            <Show when=move || state.get().phase() == SessionPhase::SignedIn>
                                <Button id="theatre-account-logout" variant=ButtonVariant::Text
                                    on_press=Callback::new(move |_| on_logout.run(()))>
                                    {move || messages::text(Key::SignOut, locale.get())}
                                </Button>
                            </Show>
                        </section>
                    </Surface>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_live(state: RwSignal<AccountViewState>, locale: RwSignal<Locale>) -> String {
        let theme = RwSignal::new(Theme::System);
        let noop = Callback::new(|()| {});
        view! { <LiveAccount state locale theme registration=RwSignal::new(crate::application::registration::RegistrationState::default()) on_registration_refresh=noop on_login=noop on_register=noop
        on_logout=noop on_refresh=noop on_membership=noop/> }
        .to_html()
    }

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
            let pending = render_live(state, locale);
            assert!(pending.contains("Đang đăng xuất"));
            assert!(!pending.contains("id=\"theatre-account-login\""));
            state
                .write()
                .finish_logout(cleanup, Err(AccountFailure::Unavailable));
            let retry = render_live(state, locale);
            assert!(retry.contains("Hoàn tất đăng xuất"));
            assert!(!retry.contains("id=\"theatre-account-login\""));
            state.write().finish_logout(cleanup, Ok(()));
            let guest = render_live(state, locale);
            assert!(guest.contains("Đăng nhập"));
            for forbidden in ["Cantos Theatre", "access_token", "session_id", "/studio"] {
                assert!(!guest.contains(forbidden), "{forbidden}");
            }
        });
    }

    #[test]
    fn signed_in_account_uses_safe_fallback_without_actor_id_counts_or_membership_entitlement() {
        Owner::new().with(|| {
            let mut current = AccountViewState::default();
            let issued = current.begin_refresh().unwrap();
            let actor = "9f469d30-bce4-447b-bb7e-fc4e7d785718";
            current
                .finish_session(
                    issued,
                    Ok(crate::application::account::AccountScope::from_session(
                        actor.to_owned(),
                        uuid::Uuid::from_u128(1).to_string(),
                    )
                    .unwrap()),
                )
                .unwrap();
            let state = RwSignal::new(current);
            let locale = RwSignal::new(Locale::English);
            let html = render_live(state, locale);
            for required in [
                "Welcome back",
                "Signed-in listener",
                "Open my library",
                "Sign out",
                "Membership details are currently unavailable",
                "theatre-account-theme",
            ] {
                assert!(html.contains(required), "{required}");
            }
            for forbidden in [
                actor,
                "Account ID:",
                "Minh An",
                "03",
                "05",
                "active membership",
                "Not integrated",
                "data-variant=\"auth-primary\"",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        });
    }
}
