//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use super::{
    account_ui::{account_ui, session_tone, AccountUi},
    auth::{label, AuthActions, SessionRecovery},
    auth_copy::CopyKey,
    guest_identity::{GuestInfo, ProfileAvatar},
};
#[cfg(feature = "preview")]
use crate::app::context::TheatreContext;
use crate::app::routes::Route;
use crate::{
    application::account_view::{AccountFailure, AccountViewState, SessionPhase},
    messages::{self, Key, Locale},
};
use cantos_ui::{
    Button, ButtonVariant, IconName, Select, SelectDensity, SelectLabelPlacement, Space, Surface,
    SurfaceTone, SurfaceVariant, Theme, ThemeCycleButton,
};
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
        AccountFailure::Diagnostic(kind) => crate::messages::error_key(kind),
        AccountFailure::SignedOut => Key::LiveGuestStatus,
        AccountFailure::SessionChanged => Key::AccountSessionChanged,
        AccountFailure::Unavailable => Key::AccountUnavailable,
        AccountFailure::Protocol => Key::AccountProtocol,
        AccountFailure::RateLimited(_) => Key::AccountRateLimited,
        AccountFailure::NotFound => Key::AccountNotFound,
        // A refusal returns to the guest form, which owns the message beside its fields.
        AccountFailure::LoginRefused(_) => Key::LiveGuestStatus,
        AccountFailure::LogoutPending => Key::AccountLogoutRetryHelp,
        AccountFailure::CursorExpired => Key::LiveLibraryCursorExpired,
    }
}

/// The visible status of a session phase, shared by Account and the Discovery intro.
pub(crate) fn live_status_key(state: &AccountUi) -> Key {
    match state.phase {
        SessionPhase::Idle | SessionPhase::Checking => Key::AccountChecking,
        SessionPhase::Guest => Key::LiveGuestStatus,
        SessionPhase::SignedIn => Key::SignedInListener,
        SessionPhase::Failed(failure) => account_failure_key(failure),
        SessionPhase::LogoutPending => {
            if state.cleanup_in_flight {
                Key::AccountLogoutPending
            } else {
                Key::AccountLogoutRetryHelp
            }
        }
    }
}

fn profile_heading(
    ui: Memo<AccountUi>,
    locale: RwSignal<Locale>,
) -> impl Fn() -> &'static str + Copy {
    move || {
        super::auth_copy::text(
            if ui.get().phase == SessionPhase::SignedIn {
                CopyKey::SignedHeading
            } else {
                CopyKey::GuestHeading
            },
            locale.get(),
        )
    }
}

fn profile_status(
    ui: Memo<AccountUi>,
    locale: RwSignal<Locale>,
) -> impl Fn() -> &'static str + Copy {
    move || messages::text(live_status_key(&ui.get()), locale.get())
}

/// The session settled as a guest with nothing to recover: the only phase whose status line and
/// second help sentence are redundant. Checking, Failed and logout phases are not.
const fn is_healthy_guest(phase: SessionPhase) -> bool {
    matches!(phase, SessionPhase::Guest)
}

/// One supportive line: what signing in unlocks for a guest, the welcome for a signed-in listener.
/// Other phases carry their own visible status instead.
const fn profile_help(phase: SessionPhase) -> Option<CopyKey> {
    match phase {
        SessionPhase::Guest => Some(CopyKey::GuestHelp),
        SessionPhase::SignedIn => Some(CopyKey::SignedHelp),
        _ => None,
    }
}

/// The short, always-visible name of the active theme mode beside the icon-only cycle control.
const fn theme_value_key(theme: Theme) -> Key {
    match theme {
        Theme::Dark => Key::Dark,
        Theme::Light => Key::Light,
        Theme::System => Key::System,
    }
}

/// A navigation row: leading icon, one label, trailing chevron. The whole row is the target and
/// keeps the route's own hash, so history and Back behave as for any other link.
#[component]
fn ActionRow(
    id: &'static str,
    route: Route,
    asset: &'static str,
    locale: RwSignal<Locale>,
    label: Key,
) -> impl IntoView {
    view! {
        <li>
            <a id=id class="theatre-action-row" href=route.hash()>
                {icon(asset)}
                <span class="theatre-action-row-label">{move || messages::text(label, locale.get())}</span>
                <ChevronGlyph/>
            </a>
        </li>
    }
}

#[component]
pub(crate) fn LiveAccount(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    theme: RwSignal<Theme>,
    on_locale: Callback<String>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    on_logout: Callback<()>,
    on_refresh: Callback<()>,
    on_membership: Callback<()>,
) -> impl IntoView {
    let ui = account_ui(state);
    view! {
        <section id="account" aria-labelledby="theatre-live-account-heading">
            <header class="theatre-page-heading">
                <p class="theatre-eyebrow">{label(locale, CopyKey::AccountEyebrow)}</p>
                <h1 id="theatre-live-account-heading">{label(locale, CopyKey::AccountHeading)}</h1>
            </header>
            <div class="theatre-account">
                <Surface tone=SurfaceTone::PrimaryContainer variant=SurfaceVariant::Profile inset=Space::Lg class="theatre-account-profile">
                    <ProfileAvatar/>
                    <h2>{profile_heading(ui, locale)}</h2>
                    // A healthy guest has nothing to report: the heading and one line say it.
                    // Every other phase keeps its live status, failure and recovery visible.
                    <Show when=move || !is_healthy_guest(ui.get().phase)>
                        <p id="theatre-live-account-status" class="theatre-auth-feedback"
                            data-tone=move || session_tone(ui.get().phase).as_str()
                            role="status" aria-live="polite" aria-atomic="true">
                            {profile_status(ui, locale)}
                        </p>
                    </Show>
                    {move || profile_help(ui.get().phase).map(|key| view! {
                        <p class="theatre-account-help">{label(locale, key)}</p>
                    })}
                    <Show when=move || ui.get().can_authenticate>
                        <AuthActions state locale scope="theatre-account" on_login on_register/>
                    </Show>
                    // Optional, so it follows the actions it explains instead of leading the card.
                    <Show when=move || is_healthy_guest(ui.get().phase)>
                        <GuestInfo locale/>
                    </Show>
                    <Show when=move || ui.get().phase == SessionPhase::SignedIn>
                        <a class="theatre-action theatre-account-library-link" href="#library">{label(locale, CopyKey::OpenLibrary)}</a>
                    </Show>
                    <SessionRecovery state locale scope="theatre-account" on_refresh on_logout/>
                </Surface>
                <div class="theatre-account-content">
                    <Surface tone=SurfaceTone::Plain variant=SurfaceVariant::AccountGroup inset=Space::Lg class="theatre-account-preferences">
                        <section aria-labelledby="theatre-account-preferences-heading">
                            <h2 id="theatre-account-preferences-heading" class="theatre-group-heading">{label(locale, CopyKey::PreferencesHeading)}</h2>
                            <div class="theatre-pref-row">
                                <div class="theatre-pref-tile theatre-pref-theme">
                                    <span class="theatre-pref-copy">
                                        <span id="theatre-account-theme-label" class="theatre-pref-label">{move || messages::text(Key::Theme, locale.get())}</span>
                                        <span class="theatre-pref-value" aria-hidden="true">{move || messages::text(theme_value_key(theme.get()), locale.get())}</span>
                                    </span>
                                    <ThemeCycleButton id="theatre-account-theme" theme=theme
                                        labels=Signal::derive(move || crate::localization::theme_labels(locale.get()))/>
                                </div>
                                <div class="theatre-pref-tile theatre-pref-language">
                                    <Select id="theatre-account-locale" class="theatre-pref-locale"
                                        density=SelectDensity::Compact label_placement=SelectLabelPlacement::Above
                                        label=Signal::derive(move || messages::text(Key::Locale, locale.get()).to_owned())
                                        value=Signal::derive(move || locale.get().lang().to_owned())
                                        options=crate::localization::locale_options()
                                        on_input=on_locale/>
                                </div>
                            </div>
                            <ul class="theatre-action-rows">
                                <ActionRow id="theatre-account-open-library" route=Route::Library asset="library_books" locale label=Key::Library/>
                                <ActionRow id="theatre-account-open-settings" route=Route::Settings asset="settings" locale label=Key::Settings/>
                            </ul>
                            <Show when=move || ui.get().phase == SessionPhase::SignedIn>
                                <Button id="theatre-account-logout" icon=IconName::Logout variant=ButtonVariant::Text
                                    on_press=Callback::new(move |_| on_logout.run(()))>
                                    {move || messages::text(Key::SignOut, locale.get())}
                                </Button>
                            </Show>
                        </section>
                    </Surface>
                    <Surface tone=SurfaceTone::Container variant=SurfaceVariant::Membership inset=Space::Lg class="theatre-account-membership">
                        <section aria-labelledby="theatre-account-membership-heading">
                            <p class="theatre-eyebrow">{label(locale, CopyKey::MembershipName)}</p>
                            <h2 id="theatre-account-membership-heading">{label(locale, CopyKey::MembershipHeading)}</h2>
                            // Unknown is neutral: no launch, closed-signup or active claim is made here.
                            <p id="theatre-account-membership-status" class="theatre-muted">{label(locale, CopyKey::MembershipStatusUnknown)}</p>
                            <Button id="theatre-account-membership" variant=ButtonVariant::AuthSecondary
                                icon=IconName::Star focus_on_press=true
                                on_press=Callback::new(move |_| on_membership.run(()))>
                                {label(locale, CopyKey::MembershipLearn)}
                            </Button>
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

    #[test]
    fn healthy_accounts_have_no_manual_refresh_or_empty_recovery() {
        use crate::features::auth_preview::{guest, signed_in, PreviewActor};
        Owner::new().with(|| {
            for language in [Locale::Vietnamese, Locale::English] {
                let state = RwSignal::new(AccountViewState::default());
                let locale = RwSignal::new(language);
                guest(state).unwrap();
                for signed in [false, true] {
                    if signed {
                        signed_in(state, PreviewActor::A).unwrap();
                    }
                    let html = render_live(state.read_only(), locale);
                    assert!(!html.contains("theatre-account-refresh-current"));
                    assert!(!html.contains("theatre-account-recovery"));
                }
            }
        });
    }

    #[test]
    fn preferences_precede_membership_and_form_two_tiles_that_name_their_value() {
        use crate::features::auth_preview::guest;
        Owner::new().with(|| {
            for (language, theme_value, autonym) in [
                (Locale::Vietnamese, "Hệ thống", "Tiếng Việt"),
                (Locale::English, "System", "English"),
            ] {
                let theme_label = messages::text(Key::Theme, language);
                let locale_label = messages::text(Key::Locale, language);
                let state = RwSignal::new(AccountViewState::default());
                let locale = RwSignal::new(language);
                guest(state).unwrap();
                let html = render_live(state.read_only(), locale);
                // Reading order is the narrow-viewport order: guest, preferences, Membership.
                let position =
                    |needle: &str| html.find(needle).unwrap_or_else(|| panic!("{needle}"));
                assert!(
                    position("theatre-account-profile") < position("theatre-account-preferences")
                        && position("theatre-account-preferences")
                            < position("theatre-account-membership-heading")
                );
                assert_eq!(html.matches("theatre-pref-tile").count(), 2, "{html}");
                let row = html
                    .split("theatre-pref-row")
                    .nth(1)
                    .expect("preference row");
                let row = row.split("theatre-action-rows").next().unwrap();
                let visible = visible_text(row);
                for shown in [theme_label, theme_value, locale_label, autonym] {
                    assert!(visible.contains(shown), "{shown}");
                }
            }
        });
    }

    #[test]
    fn preferences_use_one_theme_cycle_and_locale_in_one_row_with_icon_action_rows() {
        use crate::features::auth_preview::guest;
        Owner::new().with(|| {
            for language in [Locale::Vietnamese, Locale::English] {
                let state = RwSignal::new(AccountViewState::default());
                let locale = RwSignal::new(language);
                guest(state).unwrap();
                let html = render_live(state.read_only(), locale);
                // One cycle control, no theme Select, no repeated retention sentence.
                assert_eq!(html.matches("cui-theme-cycle-button").count(), 1, "{html}");
                assert!(!html.contains("id=\"theatre-account-theme-help\""));
                assert!(
                    !html.contains(messages::text(Key::Theme, language))
                        || html.contains("theatre-pref-label")
                );
                // Theme and locale share one row; the locale options come from the l10n owner.
                let row = html
                    .split("theatre-pref-row")
                    .nth(1)
                    .expect("preference row");
                let row = row.split("theatre-action-rows").next().unwrap();
                assert!(
                    row.contains("theatre-account-theme") && row.contains("theatre-account-locale")
                );
                for option in Locale::supported() {
                    assert!(row.contains(option.autonym()), "{}", option.autonym());
                }
                // Library and Settings are real route links with icon, label and chevron.
                for (id, hash) in [
                    ("theatre-account-open-library", Route::Library.hash()),
                    ("theatre-account-open-settings", Route::Settings.hash()),
                ] {
                    let anchor = html.split(&format!("id=\"{id}\"")).nth(1).expect(id);
                    let anchor = anchor.split("</a>").next().unwrap();
                    assert!(anchor.contains(&format!("href=\"{hash}\"")), "{anchor}");
                    assert!(anchor.contains("cui-icon") || anchor.contains("<svg"));
                    assert!(anchor.contains("theatre-chevron"));
                }
            }
        });
    }

    #[test]
    fn membership_unknown_is_neutral_and_keeps_its_real_action() {
        use crate::features::auth_preview::guest;
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            let locale = RwSignal::new(Locale::Vietnamese);
            guest(state).unwrap();
            let html = render_live(state.read_only(), locale);
            assert!(html.contains("Chưa có thông tin"));
            assert!(html.contains("id=\"theatre-account-membership\""));
            for claim in ["Sắp ra mắt", "Chưa mở đăng ký", "Coming soon"] {
                assert!(!html.contains(claim), "{claim}");
            }
            locale.set(Locale::English);
            assert!(render_live(state.read_only(), locale).contains("No information yet"));
        });
    }

    #[test]
    fn guest_cta_pair_is_deep_filled_and_transparent_outlined_of_one_size_from_tokens() {
        let css = include_str!("../styles/theatre.css");
        let rule = |selector: &str| {
            css.split(&format!("{selector} {{"))
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("missing rule {selector}"))
        };
        let scope = ".theatre-account-profile";
        let filled = rule(&format!(
            "{scope} .cui-button[data-variant=\"auth-primary\"]:not(:disabled)"
        ));
        assert!(filled.contains("background: var(--cantos-color-on-primary-container)"));
        assert!(filled.contains("color: var(--cantos-color-primary-container)"));
        let outlined = rule(&format!(
            "{scope} .cui-button[data-variant=\"auth-outlined\"]:not(:disabled)"
        ));
        assert!(outlined.contains("background: transparent"));
        assert!(outlined.contains("color: var(--cantos-color-on-primary-container)"));
        let both = rule(&format!(
            "{scope} .theatre-auth-actions .cui-button:is([data-variant=\"auth-primary\"], [data-variant=\"auth-outlined\"])"
        ));
        assert!(both.contains("min-block-size: var(--cantos-size-auth-primary)"));
        assert!(both.contains("border: var(--cantos-size-auth-outline) solid"));
        for block in [filled, outlined, both] {
            assert!(!block.contains('#') && !block.contains("rgb") && !block.contains("accent"));
        }
        // The pair is scoped to the guest card: the shared auth roles are not redefined globally.
        assert!(!css.contains("\n.cui-button[data-variant=\"auth-primary\"]"));
    }

    #[test]
    fn account_cards_trim_chrome_and_wrap_under_large_text_instead_of_overflowing() {
        let css = include_str!("../styles/theatre.css");
        let rule = |selector: &str| {
            css.split(&format!("{selector} {{"))
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("missing rule {selector}"))
        };
        // The query is relative to the account column and rem-based, so it follows text size.
        assert!(css.contains(".theatre-account { container-type: inline-size; }"));
        let narrow = css
            .split("@container (max-width: 12rem) {")
            .nth(1)
            .and_then(|rest| rest.split("\n}").next())
            .expect("narrow account container query");
        for needle in [
            ".theatre-account :is(.theatre-account-preferences, .theatre-account-profile).cui-surface",
            ".theatre-action-rows > li { min-inline-size: 0; }",
            ".theatre-guest-info-trigger { margin-inline-start: 0; }",
        ] {
            assert!(narrow.contains(needle), "{needle}");
        }
        // Wrapping rows keep their controls whole: the preference tiles stack below a readable
        // width and wrap their copy beside the control, the disclosure label wraps, and the
        // equal-size CTA pair keeps one height when a label wraps.
        assert!(
            rule(".theatre-pref-row").contains("repeat(auto-fit, minmax(min(100%, 12rem), 1fr))")
        );
        assert!(rule(".theatre-pref-tile").contains("flex-wrap: wrap"));
        assert!(rule(".theatre-guest-info-label").contains("overflow-wrap: anywhere"));
        assert!(
            rule(".theatre-account-profile .theatre-auth-actions").contains("grid-auto-rows: 1fr")
        );
    }

    #[test]
    fn theme_value_names_follow_the_cycle_modes() {
        assert_eq!(theme_value_key(Theme::Dark), Key::Dark);
        assert_eq!(theme_value_key(Theme::Light), Key::Light);
        assert_eq!(theme_value_key(Theme::System), Key::System);
    }

    fn render_live(state: ReadSignal<AccountViewState>, locale: RwSignal<Locale>) -> String {
        let theme = RwSignal::new(Theme::System);
        let noop = Callback::new(|()| {});
        let on_locale = Callback::new(|_: String| {});
        view! { <LiveAccount state locale theme on_locale on_login=noop on_register=noop
        on_logout=noop on_refresh=noop on_membership=noop/> }
        .to_html()
    }

    #[test]
    fn profile_closures_ignore_library_and_sync_notifications_but_follow_phase_and_locale() {
        use crate::application::library::Section;
        use crate::features::auth_preview::{signed_in, PreviewActor};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            signed_in(state, PreviewActor::A).unwrap();
            let locale = RwSignal::new(Locale::English);
            let ui = account_ui(state.read_only());
            let runs = Arc::new(AtomicUsize::new(0));
            let count = runs.clone();
            let heading = profile_heading(ui, locale);
            let status = profile_status(ui, locale);
            // These are the same production closures installed by LiveAccount. The wrapper
            // counts their execution, not the upstream projection's recomputations.
            let rendered = Memo::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                (heading(), status())
            });
            assert_eq!(rendered.get().1, "Signed-in listener");
            let fence = state.with_untracked(AccountViewState::fence).unwrap();
            for _ in 0..3 {
                state.update(|s| {
                    s.seed_sync(|sync, _| {
                        sync.begin_saved_read(&uuid::Uuid::from_u128(99).to_string())
                    });
                });
                assert_eq!(rendered.get().1, "Signed-in listener");
                state.update(|s| s.record_storage(&fence, Ok(())));
                assert_eq!(rendered.get().1, "Signed-in listener");
            }
            state.update(|s| {
                s.begin_library(Section::Saved, true);
            });
            assert_eq!(rendered.get().1, "Signed-in listener");
            assert_eq!(
                runs.load(Ordering::SeqCst),
                1,
                "unrelated writes add zero profile runs"
            );
            state.update(|s| {
                s.begin_logout().unwrap();
            });
            assert_eq!(
                rendered.get().1,
                messages::text(Key::AccountLogoutPending, Locale::English)
            );
            assert_eq!(
                runs.load(Ordering::SeqCst),
                2,
                "a phase change adds exactly one run"
            );
            state.update(|s| {
                s.finish_logout(s.generation(), Err(AccountFailure::Unavailable));
            });
            assert_eq!(
                rendered.get().1,
                messages::text(Key::AccountLogoutRetryHelp, Locale::English)
            );
            assert_eq!(
                runs.load(Ordering::SeqCst),
                3,
                "cleanup changes within the same phase remain reactive"
            );
            locale.set(Locale::Vietnamese);
            assert_eq!(
                rendered.get().1,
                messages::text(Key::AccountLogoutRetryHelp, Locale::Vietnamese)
            );
            assert_eq!(runs.load(Ordering::SeqCst), 4);
        });
    }

    #[test]
    fn profile_projection_tracks_guest_checking_signed_in_and_diagnostic_failure() {
        use crate::features::auth_preview::{guest, signed_in, PreviewActor};
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            let ui = account_ui(state.read_only());
            let locale = RwSignal::new(Locale::English);
            let status = profile_status(ui, locale);
            guest(state).unwrap();
            assert_eq!(ui.get().phase, SessionPhase::Guest);
            assert!(ui.get().can_authenticate);
            let issued = state.write().begin_refresh().unwrap();
            assert_eq!(ui.get().phase, SessionPhase::Checking);
            assert_eq!(
                status(),
                messages::text(Key::AccountChecking, Locale::English)
            );
            state
                .write()
                .finish_session(
                    issued,
                    Err(AccountFailure::Diagnostic(
                        crate::application::errors::ErrorKind::Network,
                    )),
                    || uuid::Uuid::from_u128(100).to_string(),
                )
                .unwrap();
            assert_eq!(
                ui.get().phase,
                SessionPhase::Failed(AccountFailure::Diagnostic(
                    crate::application::errors::ErrorKind::Network
                ))
            );
            assert_eq!(
                status(),
                messages::text(
                    crate::messages::error_key(crate::application::errors::ErrorKind::Network),
                    Locale::English
                )
            );
            signed_in(state, PreviewActor::A).unwrap();
            assert_eq!(ui.get().phase, SessionPhase::SignedIn);
            assert_eq!(status(), "Signed-in listener");
            let cleanup = state.write().begin_logout().unwrap();
            assert!(ui.get().cleanup_in_flight);
            assert!(!ui.get().can_retry_logout);
            state
                .write()
                .finish_logout(cleanup, Err(AccountFailure::Unavailable));
            assert!(!ui.get().cleanup_in_flight);
            assert!(ui.get().can_retry_logout);
            let retry = state.write().begin_logout().unwrap();
            assert!(!ui.get().can_retry_logout);
            state.write().finish_logout(retry, Ok(()));
            assert_eq!(ui.get().phase, SessionPhase::Guest);
        });
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
                .finish_session(issued, Err(AccountFailure::SignedOut), || {
                    uuid::Uuid::from_u128(100).to_string()
                })
                .unwrap()
            else {
                panic!("401 requires cleanup")
            };
            let pending = render_live(state.read_only(), locale);
            assert!(pending.contains("Đang đăng xuất"));
            assert!(!pending.contains("id=\"theatre-account-login\""));
            state
                .write()
                .finish_logout(cleanup, Err(AccountFailure::Unavailable));
            let retry = render_live(state.read_only(), locale);
            assert!(retry.contains("Hoàn tất đăng xuất"));
            assert!(!retry.contains("id=\"theatre-account-login\""));
            state.write().finish_logout(cleanup, Ok(()));
            let guest = render_live(state.read_only(), locale);
            assert!(guest.contains("Đăng nhập"));
            for forbidden in ["Cantos Theatre", "access_token", "session_id", "/studio"] {
                assert!(!guest.contains(forbidden), "{forbidden}");
            }
        });
    }

    fn visible_text(html: &str) -> String {
        html.split('>')
            .filter_map(|segment| segment.split('<').next())
            .collect()
    }

    #[test]
    fn healthy_guest_card_greets_the_listener_with_two_actions_and_a_named_closed_disclosure() {
        use crate::features::auth_preview::guest;
        Owner::new().with(|| {
            for (language, eyebrow, page, greeting, line, info, name, create) in [
                (
                    Locale::Vietnamese,
                    "MỘT GÓC NGHE CỦA RIÊNG BẠN",
                    "Góc của bạn.",
                    "Chào người nghe.",
                    "Đăng nhập để mở thư viện của bạn.",
                    "Bạn vẫn có thể khám phá và nghe nội dung công khai mà không cần đăng nhập.",
                    "Về chế độ nghe không đăng nhập",
                    "Tạo tài khoản",
                ),
                (
                    Locale::English,
                    "A LISTENING SPACE OF YOUR OWN",
                    "Your space.",
                    "Welcome, listener.",
                    "Sign in to open your library.",
                    "You can still explore and listen to public content without signing in.",
                    "About listening without signing in",
                    "Create account",
                ),
            ] {
                let state = RwSignal::new(AccountViewState::default());
                guest(state).unwrap();
                let html = render_live(state.read_only(), RwSignal::new(language));
                assert_eq!(html.matches("<h1").count(), 1);
                // The page keeps its eyebrow and heading; the status line and the second help
                // sentence stay redundant for a healthy guest.
                for removed in [
                    "theatre-live-account-status",
                    "Chưa đăng nhập vào Cantos",
                    "You can still explore public stories",
                    "Chào bạn.",
                    "Hello.",
                ] {
                    assert!(!html.contains(removed), "{removed}");
                }
                let text = visible_text(&html);
                for shown in [eyebrow, page, greeting, line, create, name] {
                    assert!(text.contains(shown), "{shown}");
                }
                assert!(html.contains("theatre-eyebrow"));
                // The disclosure is named by its own visible text, starts closed and says what it opens.
                assert!(html.contains("aria-expanded=\"false\""));
                assert!(html.contains("aria-controls=\"theatre-guest-info-panel\""));
                let trigger = html
                    .split("id=\"theatre-guest-info\"")
                    .nth(1)
                    .and_then(|rest| rest.split('>').next())
                    .expect("disclosure trigger");
                assert!(!trigger.contains("aria-label"), "{trigger}");
                let panel = html
                    .split("id=\"theatre-guest-info-panel\"")
                    .nth(1)
                    .unwrap();
                assert!(panel.split('>').next().unwrap().contains("hidden"));
                assert!(panel.contains(info));
                // It follows the actions it explains instead of leading the card.
                assert!(
                    html.find("id=\"theatre-account-register\"").unwrap()
                        < html.find("id=\"theatre-guest-info\"").unwrap()
                );
                // The avatar is a compact outline, never the solid large silhouette or an info circle.
                let avatar = html
                    .split("<span")
                    .find(|part| part.contains("theatre-avatar"))
                    .and_then(|part| part.split("</span>").next())
                    .expect("profile avatar");
                assert!(avatar.contains("data-profile=\"true\"") && !avatar.contains("data-size"));
                assert!(
                    avatar.contains("fill=\"none\"") && !avatar.contains("fill=\"currentColor\"")
                );
                assert!(!html.contains("data-guest"));
                // The two existing actions keep their ids and callbacks.
                assert!(html.contains("id=\"theatre-account-login\""));
                assert!(html.contains("id=\"theatre-account-register\""));
                assert!(
                    !html.contains("Cantos</"),
                    "no brand word in the action label"
                );
            }
        });
    }

    #[test]
    fn unhealthy_guest_phases_keep_their_live_status_and_no_info_control() {
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            let locale = RwSignal::new(Locale::Vietnamese);
            // Checking: the live status stays, announced politely.
            let checking = render_live(state.read_only(), locale);
            assert!(checking.contains("id=\"theatre-live-account-status\""));
            assert!(checking.contains("role=\"status\""));
            assert!(checking.contains(messages::text(Key::AccountChecking, Locale::Vietnamese)));
            assert!(!checking.contains("theatre-guest-info"));
            assert!(checking.contains("theatre-eyebrow"));
            // Failed: the failure text is visible with its retry control.
            let issued = state.write().begin_refresh().unwrap();
            state
                .write()
                .finish_session(issued, Err(AccountFailure::Unavailable), || {
                    uuid::Uuid::from_u128(100).to_string()
                })
                .unwrap();
            let failed = render_live(state.read_only(), locale);
            assert!(failed.contains(messages::text(Key::AccountUnavailable, Locale::Vietnamese)));
            assert!(failed.contains("id=\"theatre-account-refresh\""));
            assert!(!failed.contains("theatre-guest-info"));
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
                    || uuid::Uuid::from_u128(100).to_string(),
                )
                .unwrap();
            let state = RwSignal::new(current);
            let locale = RwSignal::new(Locale::English);
            let html = render_live(state.read_only(), locale);
            for required in [
                "Welcome back",
                "Signed-in listener",
                "Open my library",
                "Sign out",
                "No information yet",
                "theatre-account-theme",
            ] {
                assert!(html.contains(required), "{required}");
            }
            for forbidden in [
                actor,
                "Account ID:",
                "Minh An",
                "active membership",
                "Not integrated",
                "data-variant=\"auth-primary\"",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
            // Counts are visible copy. Official SVG path coordinates may contain the same digits.
            let visible_text = html
                .split('>')
                .filter_map(|segment| segment.split('<').next())
                .collect::<String>();
            for invented_count in ["03", "05"] {
                assert!(!visible_text.contains(invented_count), "{invented_count}");
            }
        });
    }
}
