//! Theatre auth presentation; the shell owns navigation and all identity effects.
use super::{
    account::account_failure_key,
    account_ui::{account_ui, session_tone, AccountUi},
    auth_copy::{self, CopyKey},
    signup::SignupPanel,
};
use crate::{
    application::{
        account_view::{AccountViewState, LoginRefusal, SessionPhase},
        auth_navigation::{AuthRoute, AuthStep, ReturnDestination},
        signup::{SignupPhase, SignupState},
    },
    messages::{self, Key, Locale},
};
use cantos_ui::{
    AccessibleName, BackButton, Button, ButtonState, ButtonVariant, DisabledReason, FieldState,
    IconButton, IconName, Logo, LogoTone, LogoVariant, Space, StatusTone, Surface, SurfaceTone,
    SurfaceVariant, TextField, TextFieldKind, TextFieldVariant,
};
use leptos::prelude::*;

pub(crate) fn label(locale: RwSignal<Locale>, key: CopyKey) -> impl Fn() -> &'static str + Copy {
    move || auth_copy::text(key, locale.get())
}

fn back_key(route: AuthRoute) -> CopyKey {
    match route.step {
        AuthStep::Register => CopyKey::ReturnLogin,
        AuthStep::Login => match route.origin {
            ReturnDestination::Home => CopyKey::ReturnHome,
            ReturnDestination::Account => CopyKey::ReturnAccount,
            ReturnDestination::Membership | ReturnDestination::MembershipFromAccount => {
                CopyKey::ReturnMembership
            }
        },
    }
}

fn blocked_login_key(state: &AccountUi) -> Key {
    match state.phase {
        SessionPhase::Idle | SessionPhase::Checking => Key::AccountChecking,
        SessionPhase::Guest => Key::LiveGuestStatus,
        SessionPhase::SignedIn => Key::SignedInListener,
        SessionPhase::Failed(failure) => account_failure_key(failure),
        SessionPhase::LogoutPending if state.cleanup_in_flight => Key::AccountLogoutPending,
        SessionPhase::LogoutPending => Key::AccountLogoutRetryHelp,
    }
}

/// The polite live region announces a refusal; the generic copy names no factor.
fn status_text(state: &AccountUi, locale: Locale) -> &'static str {
    match (state.phase, state.login_refusal) {
        (SessionPhase::Guest, Some(refusal)) => auth_copy::text(refusal_key(refusal), locale),
        _ => messages::text(blocked_login_key(state), locale),
    }
}

fn status_tone(state: &AccountUi) -> StatusTone {
    if state.phase == SessionPhase::Guest && state.login_refusal.is_some() {
        StatusTone::Error
    } else {
        session_tone(state.phase)
    }
}

fn refusal_key(refusal: LoginRefusal) -> CopyKey {
    match refusal {
        LoginRefusal::Credentials => CopyKey::LoginRefusedCredentials,
        LoginRefusal::Invalid => CopyKey::LoginRefusedInvalid,
    }
}

fn action_state(enabled: bool, checking: bool, reason: &str) -> Option<ButtonState> {
    if enabled {
        return Some(ButtonState::Enabled);
    }
    DisabledReason::new(reason).ok().map(|reason| {
        if checking {
            ButtonState::Loading(reason)
        } else {
            ButtonState::Disabled(reason)
        }
    })
}
// Equal ButtonState outputs stop work even when a different upstream fact notified. Sign-in and
// account creation share one admission: a guest with no unresolved cleanup.
fn auth_action_state(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
) -> Memo<Option<ButtonState>> {
    let ui = account_ui(state);
    Memo::new(move |_| {
        let current = ui.get();
        let reason = messages::text(blocked_login_key(&current), locale.get());
        action_state(
            current.can_authenticate,
            matches!(current.phase, SessionPhase::Idle | SessionPhase::Checking),
            reason,
        )
    })
}

/// Busy marks the one action in flight. A secondary action (account creation) never starts the
/// pending session lookup, so it stays blocked by the same admission without a spinner.
fn secondary_state(state: ButtonState) -> ButtonState {
    match state {
        ButtonState::Loading(reason) => ButtonState::Disabled(reason),
        other => other,
    }
}

// Memoized so a Loading -> Disabled change of the primary does not rebuild an unchanged secondary.
fn secondary_action_state(availability: Memo<Option<ButtonState>>) -> Memo<Option<ButtonState>> {
    Memo::new(move |_| availability.get().map(secondary_state))
}

pub(crate) fn login_key_submits(key: &str, is_composing: bool, input_target: bool) -> bool {
    input_target && key == "Enter" && !is_composing
}

fn password_eye_names(locale: Locale) -> Option<(AccessibleName, AccessibleName)> {
    Some((
        AccessibleName::new(auth_copy::text(CopyKey::LoginShowPassword, locale)).ok()?,
        AccessibleName::new(auth_copy::text(CopyKey::LoginHidePassword, locale)).ok()?,
    ))
}

#[component]
pub(crate) fn PasswordEye(
    id: &'static str,
    locale: RwSignal<Locale>,
    revealed: RwSignal<bool>,
) -> impl IntoView {
    view! {
        {move || password_eye_names(locale.get()).map(|(show, hide)| view! {
            <IconButton id=id
                name=Signal::derive(move || if revealed.get() { hide.clone() } else { show.clone() })
                icon=Signal::derive(move || if revealed.get() { IconName::VisibilityOff } else { IconName::Visibility })
                on_press=Callback::new(move |_| revealed.update(|shown| *shown = !*shown))/>
        })}
    }
}

/// Entry actions of the account page: they open the explicit auth routes, behind the same
/// cleanup barrier as the forms themselves.
#[component]
pub(crate) fn AuthActions(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    scope: &'static str,
    on_login: Callback<()>,
    on_register: Callback<()>,
) -> impl IntoView {
    let availability = auth_action_state(state, locale);
    let register_availability = secondary_action_state(availability);
    view! {
        <div class="theatre-auth-actions">
            {move || availability.get().map(|availability| view! {
                <Button id=format!("{scope}-login") variant=ButtonVariant::AuthPrimary
                    icon=IconName::Login
                    state=availability focus_on_press=true
                    on_press=Callback::new(move |_| on_login.run(()))>
                    {label(locale, CopyKey::LoginSubmit)}
                </Button>
            })}
            {move || register_availability.get().map(|availability| view! {
                <Button id=format!("{scope}-register") variant=ButtonVariant::AuthSecondary
                    icon=IconName::PersonAdd
                    state=availability focus_on_press=true
                    on_press=Callback::new(move |_| on_register.run(()))>
                    {label(locale, CopyKey::RegisterAction)}
                </Button>
            })}
        </div>
    }
}

#[component]
pub(crate) fn SessionRecovery(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    scope: &'static str,
    on_refresh: Callback<()>,
    on_logout: Callback<()>,
) -> impl IntoView {
    let ui = account_ui(state);
    view! {
        <>
            <Show when=move || matches!(ui.get().phase, SessionPhase::Failed(_))>
                <Button id=format!("{scope}-refresh") variant=ButtonVariant::AuthSecondary
                    icon=IconName::Refresh
                    on_press=Callback::new(move |_| on_refresh.run(()))>
                    {move || messages::text(Key::PublicRetry, locale.get())}
                </Button>
            </Show>
            <Show when=move || ui.get().can_retry_logout>
                <Button id=format!("{scope}-logout-retry") variant=ButtonVariant::AuthSecondary
                    icon=IconName::Refresh
                    on_press=Callback::new(move |_| on_logout.run(()))>
                    {move || messages::text(Key::AccountLogoutRetry, locale.get())}
                </Button>
            </Show>
        </>
    }
}

/// Email and password sign-in. The password lives only in this field's signal until the
/// explicit submit, which clears it before the typed intent leaves; it is never stored,
/// logged, persisted or placed in an error notice.
#[component]
fn LoginForm(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    availability: Memo<Option<ButtonState>>,
    on_submit: Callback<(String, String)>,
) -> impl IntoView {
    let ui = account_ui(state);
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    // The eye only swaps the native `type` of the same input; it owns no secret.
    let revealed = RwSignal::new(false);
    let field_state = Signal::derive(move || {
        if ui.get().can_authenticate {
            FieldState::Editable
        } else {
            FieldState::ReadOnly
        }
    });
    let submit = move || {
        if !ui.get_untracked().can_authenticate {
            return;
        }
        let address = email.get_untracked().trim().to_owned();
        let secret = password.get_untracked();
        if address.is_empty() || secret.is_empty() {
            return;
        }
        // The editable copy is cleared before any I/O; the moved String is the only one left.
        password.set(String::new());
        revealed.set(false);
        on_submit.run((address, secret));
    };
    view! {
        <form class="theatre-auth-actions" id="theatre-login-form"
            on:submit=move |event| { event.prevent_default(); submit(); }
            on:keydown=move |event| {
                use leptos::wasm_bindgen::JsCast;
                let input_target = event.target().is_some_and(|target| {
                    target.dyn_ref::<leptos::web_sys::HtmlInputElement>().is_some()
                });
                // Input Enter submits outside composition. Buttons keep their native activation,
                // so the password eye never submits credentials.
                if login_key_submits(&event.key(), event.is_composing(), input_target) {
                    event.prevent_default();
                    submit();
                }
            }>
            <TextField id="theatre-login-email" kind=TextFieldKind::Email variant=TextFieldVariant::Floating
                label=Signal::derive(move || auth_copy::text(CopyKey::LoginEmail, locale.get()).to_owned())
                value=email
                autocomplete="username" required=true
                state=field_state
                on_input=Callback::new(move |value| email.set(value))/>
            <TextField id="theatre-login-password" variant=TextFieldVariant::Floating
                kind=Signal::derive(move || if revealed.get() { TextFieldKind::Text } else { TextFieldKind::Password })
                label=Signal::derive(move || auth_copy::text(CopyKey::LoginPassword, locale.get()).to_owned())
                value=password
                autocomplete="current-password" required=true
                state=field_state
                on_input=Callback::new(move |value| password.set(value))
                trailing=move || view! {
                    <PasswordEye id="theatre-login-password-eye" locale revealed/>
                }/>
            {move || availability.get().map(|availability| view! {
                <Button id="theatre-auth-login" variant=ButtonVariant::AuthPrimary icon=IconName::Login state=availability
                    on_press=Callback::new(move |_| submit())>
                    {move || auth_copy::text(
                        if ui.get().phase == SessionPhase::Checking { CopyKey::LoginSubmitting } else { CopyKey::LoginSubmit },
                        locale.get())}
                </Button>
            })}
        </form>
    }
}

#[component]
pub(crate) fn AuthPage(
    state: ReadSignal<AccountViewState>,
    signup: RwSignal<SignupState>,
    locale: RwSignal<Locale>,
    route: AuthRoute,
    /// RAM-only: a finished signup sends the owner here to sign in. It is never a session.
    account_created: RwSignal<bool>,
    on_back: Callback<()>,
    on_register: Callback<()>,
    on_login: Callback<(String, String)>,
    on_home: Callback<()>,
    on_refresh: Callback<()>,
    on_logout: Callback<()>,
    on_signup_submit: Callback<()>,
    on_signup_confirm: Callback<()>,
    on_signup_retry: Callback<()>,
    on_signup_new: Callback<()>,
    on_signup_login: Callback<()>,
) -> impl IntoView {
    let ui = account_ui(state);
    let availability = auth_action_state(state, locale);
    let register_availability = secondary_action_state(availability);
    let is_login = route.step == AuthStep::Login;
    view! {
        <section class="theatre-auth" aria-labelledby="theatre-auth-heading">
            <div class="theatre-auth-top">
                {move || AccessibleName::new(auth_copy::text(back_key(route), locale.get())).ok().map(|name| view! {
                    <BackButton id="theatre-auth-back" name=name
                        on_back=Callback::new(move |_| on_back.run(()))/>
                })}
                <a id="theatre-auth-home" class="theatre-auth-brand" href="#discover"
                    aria-label=label(locale, CopyKey::ReturnHome)
                    on:click=move |event| { event.prevent_default(); on_home.run(()); }>
                    <Logo tone=LogoTone::Light class="theatre-logo-light"/>
                    <Logo tone=LogoTone::Dark class="theatre-logo-dark"/>
                </a>
            </div>
            <div class=if is_login { "theatre-auth-layout" } else { "theatre-auth-layout theatre-auth-register-layout" }>
                <Show when=move || is_login>
                <Surface tone=SurfaceTone::Container variant=SurfaceVariant::Auth inset=Space::None class="theatre-auth-story">
                    <div class="theatre-auth-art" aria-hidden="true">
                        <Logo variant=LogoVariant::Symbol tone=LogoTone::Light class="theatre-logo-light"/>
                        <Logo variant=LogoVariant::Symbol tone=LogoTone::Dark class="theatre-logo-dark"/>
                    </div>
                    <div class="theatre-auth-story-copy">
                        <p class="theatre-eyebrow">{move || messages::text(Key::BrandCaption, locale.get())}</p>
                        <h2>{label(locale, CopyKey::BrandHeading)}</h2>
                        <p>{label(locale, CopyKey::BrandHelp)}</p>
                        <p class="theatre-muted">{label(locale, CopyKey::BrandGenres)}</p>
                    </div>
                </Surface>
                </Show>
                <div class="theatre-auth-copy">
                    <Show when=move || is_login><p class="theatre-eyebrow">{label(locale, CopyKey::LoginEyebrow)}</p></Show>
                    <h1 id="theatre-auth-heading">{move || auth_copy::text(if is_login {
                        CopyKey::LoginHeading
                    } else if matches!(signup.get().phase(), SignupPhase::AwaitingConfirmation | SignupPhase::Completing | SignupPhase::CompleteRetry(_) | SignupPhase::CompleteRefused(_)) {
                        CopyKey::SignupPasswordHeading
                    } else {
                        CopyKey::RegisterHeading
                    }, locale.get())}</h1>
                    <Show when=move || is_login><p class="theatre-muted">{label(locale, CopyKey::LoginHelp)}</p></Show>
                    <Show when=move || is_login || ui.get().phase == SessionPhase::LogoutPending>
                        <p id="theatre-auth-status" class="theatre-auth-feedback"
                            data-tone=move || status_tone(&ui.get()).as_str()
                            role="status" aria-live="polite" aria-atomic="true">
                            {move || status_text(&ui.get(), locale.get())}
                        </p>
                    </Show>
                    {if is_login {
                        view! {
                            <Show when=move || account_created.get()>
                                <p id="theatre-auth-created" class="theatre-auth-feedback" data-tone="info" role="status">{label(locale, CopyKey::AccountCreatedNotice)}</p>
                            </Show>
                            <Show when=move || ui.get().phase != SessionPhase::SignedIn>
                                <LoginForm state locale availability on_submit=on_login/>
                                {move || register_availability.get().map(|availability| view! {
                                    <Button id="theatre-auth-register" variant=ButtonVariant::AuthSecondary
                                        icon=IconName::PersonAdd
                                        state=availability focus_on_press=true
                                        on_press=Callback::new(move |_| on_register.run(()))>
                                        {label(locale, CopyKey::RegisterAction)}
                                    </Button>
                                })}
                            </Show>
                            <Show when=move || ui.get().phase == SessionPhase::SignedIn>
                                <p>{label(locale, CopyKey::SignedInActionHelp)}</p>
                                <a class="theatre-text-link" href="#account">{move || messages::text(Key::Account, locale.get())}</a>
                            </Show>
                        }.into_any()
                    } else {
                        view! {
                            <SignupPanel state=signup locale on_submit=on_signup_submit on_confirm=on_signup_confirm
                                on_retry=on_signup_retry on_new_request=on_signup_new on_login=on_signup_login
                                login_via_back=true/>
                        }.into_any()
                    }}
                    <Show when=move || is_login || ui.get().phase == SessionPhase::LogoutPending>
                        <SessionRecovery state locale scope="theatre-auth" on_refresh on_logout/>
                    </Show>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::account_view::{AccountFailure, SessionCompletion};
    use crate::application::signup::ProofLink;

    #[test]
    fn failed_session_has_one_read_retry_and_registration_owns_its_feedback() {
        for language in [Locale::Vietnamese, Locale::English] {
            let mut failed = guest();
            let issued = failed.begin_refresh().unwrap();
            failed
                .finish_session(issued, Err(AccountFailure::Unavailable), || {
                    uuid::Uuid::from_u128(100).to_string()
                })
                .unwrap();
            let html = render(
                AuthRoute::login(ReturnDestination::Account),
                failed.clone(),
                language,
            );
            assert_eq!(html.matches("id=\"theatre-auth-refresh\"").count(), 1);
            assert_eq!(
                native_button_name(&html, "theatre-auth-refresh"),
                messages::text(Key::PublicRetry, language)
            );
            assert!(html.contains("data-tone=\"error\""));
            assert!(!html.contains("theatre-auth-logout-retry"));
            for state in [guest(), failed] {
                let register = render(
                    AuthRoute::register(ReturnDestination::Account),
                    state,
                    language,
                );
                assert!(!register.contains("id=\"theatre-auth-status\""));
                assert!(!register.contains("id=\"theatre-auth-refresh\""));
                assert!(!register.contains("id=\"theatre-signup-status\""));
                assert!(register.contains(auth_copy::text(CopyKey::RegisterHeading, language)));
                assert!(!register.contains(auth_copy::text(CopyKey::RegisterHelp, language)));
                assert!(!register.contains("theatre-auth-story"));
            }
            let mut pending = guest();
            let issued = pending.begin_logout().unwrap();
            pending.finish_logout(issued, Err(AccountFailure::Unavailable));
            for step in [AuthStep::Login, AuthStep::Register] {
                let html = render(
                    AuthRoute {
                        step,
                        origin: ReturnDestination::Account,
                    },
                    pending.clone(),
                    language,
                );
                assert_eq!(html.matches("id=\"theatre-auth-logout-retry\"").count(), 1);
                assert_eq!(
                    native_button_name(&html, "theatre-auth-logout-retry"),
                    messages::text(Key::AccountLogoutRetry, language)
                );
                assert!(!html.contains("id=\"theatre-auth-refresh\""));
                assert!(html.contains("data-tone=\"warning\""));
            }
        }
    }

    #[test]
    fn login_refusal_is_error_but_an_ordinary_guest_is_neutral() {
        for (state, expected) in [
            (guest(), "neutral"),
            (refused(LoginRefusal::Credentials), "error"),
            (refused(LoginRefusal::Invalid), "error"),
        ] {
            let html = render(
                AuthRoute::login(ReturnDestination::Home),
                state,
                Locale::English,
            );
            assert!(html.contains(&format!("data-tone=\"{expected}\"")));
            assert_eq!(html.matches("role=\"status\"").count(), 1);
        }
    }

    fn guest() -> AccountViewState {
        let mut state = AccountViewState::default();
        let issued = state.begin_refresh().unwrap();
        let SessionCompletion::Cleanup(cleanup) = state
            .finish_session(issued, Err(AccountFailure::SignedOut), || {
                uuid::Uuid::from_u128(100).to_string()
            })
            .unwrap()
        else {
            panic!("guest lookup requires cleanup")
        };
        assert!(state.finish_logout(cleanup, Ok(())));
        state
    }

    fn refused(refusal: LoginRefusal) -> AccountViewState {
        let mut state = guest();
        let issued = state.begin_refresh().unwrap();
        state
            .finish_session(issued, Err(AccountFailure::LoginRefused(refusal)), || {
                uuid::Uuid::from_u128(100).to_string()
            })
            .unwrap();
        state
    }

    fn render(route: AuthRoute, initial: AccountViewState, language: Locale) -> String {
        render_with_signup(route, initial, language, SignupState::default(), false)
    }

    fn render_with_signup(
        route: AuthRoute,
        initial: AccountViewState,
        language: Locale,
        enrollment: SignupState,
        created: bool,
    ) -> String {
        Owner::new().with(|| {
            let state = RwSignal::new(initial);
            let locale = RwSignal::new(language);
            let signup = RwSignal::new(enrollment);
            let account_created = RwSignal::new(created);
            let noop = Callback::new(|()| {});
            let login = Callback::new(|_: (String, String)| {});
            view! { <AuthPage state=state.read_only() signup locale route account_created on_back=noop on_register=noop on_login=login
            on_home=noop on_refresh=noop on_logout=noop
            on_signup_submit=noop on_signup_confirm=noop on_signup_retry=noop on_signup_new=noop on_signup_login=noop/> }
            .to_html()
        })
    }

    fn native_button_name(html: &str, id: &str) -> String {
        let identity = format!("id=\"{id}\"");
        let controls: Vec<_> = html
            .split("<button")
            .skip(1)
            .filter_map(|markup| {
                let (opening, rest) = markup.split_once('>')?;
                opening.contains(&identity).then_some((opening, rest))
            })
            .collect();
        assert_eq!(controls.len(), 1, "native button {id}");
        let (opening, rest) = controls[0];
        assert!(opening.contains("type=\"button\""));
        assert!(!opening.contains("aria-label=") && !opening.contains("aria-labelledby="));
        let body = rest.split_once("</button>").unwrap().0;
        let mut text = String::new();
        let mut in_tag = false;
        for character in body.chars() {
            match character {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => text.push(character),
                _ => (),
            }
        }
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn control(html: &str, id: &str) -> String {
        let after = html.split(&format!("id=\"{id}\"")).nth(1).unwrap();
        after.split('>').next().unwrap().to_owned()
    }

    const ORIGINS: [ReturnDestination; 4] = [
        ReturnDestination::Home,
        ReturnDestination::Account,
        ReturnDestination::Membership,
        ReturnDestination::MembershipFromAccount,
    ];

    #[test]
    fn login_is_an_email_password_form_with_browser_credential_semantics() {
        for (origin, back) in [
            (ReturnDestination::Home, "Về Trang chủ"),
            (ReturnDestination::Account, "Về Tài khoản"),
            (ReturnDestination::Membership, "Về Membership"),
            (ReturnDestination::MembershipFromAccount, "Về Membership"),
        ] {
            let html = render(AuthRoute::login(origin), guest(), Locale::Vietnamese);
            assert!(html.contains(back));
            assert!(html.contains("data-logo-variant=\"cantos\""));
            let email = control(&html, "theatre-login-email");
            assert!(
                email.contains("type=\"email\"") && email.contains("autocomplete=\"username\"")
            );
            let password = control(&html, "theatre-login-password");
            assert!(password.contains("type=\"password\""));
            assert!(password.contains("autocomplete=\"current-password\""));
            assert!(password.contains("value=\"\""), "no prefilled secret");
            // Sign-in comes before account creation, in the capsule actions.
            assert!(html.find("Đăng nhập</").unwrap() < html.find("Đăng ký").unwrap());
            assert!(html.contains("data-variant=\"auth-primary\""));
            assert!(html.contains("data-variant=\"auth-secondary\""));
            for forbidden in [
                "Cantos ID",
                "Cantos Theatre",
                "cửa sổ",
                "popup",
                "role=\"tab\"",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn login_copy_resolves_in_both_languages_without_provider_wording() {
        let en = render(
            AuthRoute::login(ReturnDestination::Home),
            guest(),
            Locale::English,
        );
        for expected in ["Sign in with your email and password", "Password"] {
            assert!(en.contains(expected), "{expected}");
        }
        for html in [
            en,
            render(
                AuthRoute::login(ReturnDestination::Home),
                guest(),
                Locale::Vietnamese,
            ),
        ] {
            for forbidden in ["Cantos ID", "provider", "popup", "window"] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn login_has_no_privacy_footnote_and_uses_floating_fields_with_an_independent_eye() {
        for language in [Locale::Vietnamese, Locale::English] {
            let html = render(AuthRoute::login(ReturnDestination::Home), guest(), language);
            for gone in [
                "never stored or logged",
                "Mật khẩu không được lưu hay ghi lại",
                "theatre-auth-helper",
                "data-variant=\"auth-tonal\"",
            ] {
                assert!(!html.contains(gone), "{gone}");
            }
            assert_eq!(html.matches("data-variant=\"floating\"").count(), 2);
            let password = control(&html, "theatre-login-password");
            assert!(password.contains("type=\"password\""), "masked by default");
            let eye = native_button_name(&html, "theatre-login-password-eye");
            assert_eq!(eye, auth_copy::text(CopyKey::LoginShowPassword, language));
            // The eye is inside the password field's shell, not a form submit control.
            assert!(
                html.find("theatre-login-password\"").unwrap()
                    < html.find("theatre-login-password-eye").unwrap()
            );
            assert!(
                html.find("theatre-login-password-eye").unwrap()
                    < html.find("theatre-auth-login").unwrap()
            );
        }
    }

    #[test]
    fn login_enter_only_submits_from_inputs_outside_ime_composition() {
        assert!(login_key_submits("Enter", false, true));
        assert!(!login_key_submits("Enter", true, true));
        assert!(!login_key_submits(" ", false, true));
        assert!(!login_key_submits("Escape", false, true));
        assert!(
            !login_key_submits("Enter", false, false),
            "Enter on the password eye or a button must keep its native activation"
        );
        assert!(!login_key_submits("Enter", true, false));
    }

    #[test]
    fn password_eye_names_are_admitted_in_both_locales_before_mounting() {
        for (locale, show, hide) in [
            (Locale::Vietnamese, "Hiện mật khẩu", "Ẩn mật khẩu"),
            (Locale::English, "Show password", "Hide password"),
        ] {
            let names = password_eye_names(locale).expect("both localized names must be nonblank");
            assert_eq!(names.0.as_str(), show);
            assert_eq!(names.1.as_str(), hide);
        }
    }

    #[test]
    fn a_refusal_returns_to_the_form_with_a_generic_message_that_names_no_factor() {
        for (refusal, key) in [
            (LoginRefusal::Credentials, CopyKey::LoginRefusedCredentials),
            (LoginRefusal::Invalid, CopyKey::LoginRefusedInvalid),
        ] {
            for language in [Locale::Vietnamese, Locale::English] {
                let html = render(
                    AuthRoute::login(ReturnDestination::Home),
                    refused(refusal),
                    language,
                );
                assert!(html.contains(auth_copy::text(key, language)), "{refusal:?}");
                // The form is usable again: fields editable, the submit control enabled.
                assert!(!control(&html, "theatre-login-email").contains("readonly"));
                assert!(!control(&html, "theatre-auth-login").contains("disabled"));
            }
        }
        for language in [Locale::Vietnamese, Locale::English] {
            let copy = auth_copy::text(CopyKey::LoginRefusedCredentials, language).to_lowercase();
            for leak in [
                "tài khoản không tồn tại",
                "no account",
                "unknown email",
                "wrong password",
                "sai mật khẩu",
            ] {
                assert!(!copy.contains(leak), "{leak}");
            }
        }
        let clean = render(
            AuthRoute::login(ReturnDestination::Home),
            guest(),
            Locale::English,
        );
        assert!(!clean.contains(auth_copy::text(
            CopyKey::LoginRefusedCredentials,
            Locale::English
        )));
    }

    #[test]
    fn checking_locks_the_form_and_unresolved_cleanup_never_enables_a_new_intent() {
        let mut checking = guest();
        checking.begin_refresh().unwrap();
        let html = render(
            AuthRoute::login(ReturnDestination::Home),
            checking,
            Locale::English,
        );
        assert!(control(&html, "theatre-login-email").contains("readonly"));
        assert!(control(&html, "theatre-login-password").contains("readonly"));
        assert!(html.contains("Signing in"));
        let mut state = guest();
        let cleanup = state.begin_logout().unwrap();
        state.finish_logout(cleanup, Err(AccountFailure::Unavailable));
        let html = render(
            AuthRoute::login(ReturnDestination::Home),
            state,
            Locale::Vietnamese,
        );
        for id in ["theatre-auth-login", "theatre-auth-register"] {
            assert!(control(&html, id).contains("disabled"), "{id}");
        }
        assert!(html.contains("Hoàn tất đăng xuất"));
    }

    /// Opening tag and body of one native button, so busy semantics and its spinner are both visible.
    fn button_markup(html: &str, id: &str) -> (String, String) {
        let identity = format!("id=\"{id}\"");
        let markup = html
            .split("<button")
            .skip(1)
            .find(|markup| {
                markup
                    .split_once('>')
                    .is_some_and(|(open, _)| open.contains(&identity))
            })
            .unwrap_or_else(|| panic!("native button {id}"));
        let (opening, rest) = markup.split_once('>').unwrap();
        (
            opening.to_owned(),
            rest.split_once("</button>").unwrap().0.to_owned(),
        )
    }

    fn assert_busy(html: &str, id: &str, busy: bool) {
        let (opening, body) = button_markup(html, id);
        assert_eq!(
            opening.contains("aria-busy=\"true\""),
            busy,
            "{id} aria-busy"
        );
        assert_eq!(
            opening.contains("data-state=\"loading\""),
            busy,
            "{id} state"
        );
        assert_eq!(body.contains("cui-loading-indicator"), busy, "{id} spinner");
    }

    #[test]
    fn only_the_sign_in_action_is_busy_while_the_session_is_checking() {
        let mut checking = guest();
        checking.begin_refresh().unwrap();
        for (state, name) in [
            (checking, "checking"),
            (AccountViewState::default(), "idle"),
        ] {
            for language in [Locale::Vietnamese, Locale::English] {
                let html = render(
                    AuthRoute::login(ReturnDestination::Home),
                    state.clone(),
                    language,
                );
                assert_busy(&html, "theatre-auth-login", true);
                assert_busy(&html, "theatre-auth-register", false);
                let (opening, _) = button_markup(&html, "theatre-auth-register");
                assert!(
                    opening.contains("disabled"),
                    "{name}: registration is blocked"
                );
                assert_eq!(
                    native_button_name(&html, "theatre-auth-register"),
                    auth_copy::text(CopyKey::RegisterAction, language),
                    "{name}: the label stays readable"
                );
            }
        }
    }

    #[test]
    fn account_entry_actions_keep_registration_free_of_a_spinner_while_checking() {
        Owner::new().with(|| {
            let mut checking = guest();
            checking.begin_refresh().unwrap();
            let state = RwSignal::new(checking);
            let locale = RwSignal::new(Locale::English);
            let noop = Callback::new(|()| {});
            let html = view! { <AuthActions state=state.read_only() locale scope="theatre-account" on_login=noop on_register=noop/> }.to_html();
            assert_busy(&html, "theatre-account-register", false);
            assert!(button_markup(&html, "theatre-account-register").0.contains("disabled"));
        });
    }

    #[test]
    fn controls_recover_after_every_login_outcome_and_neither_button_fakes_the_other() {
        let language = Locale::English;
        let route = AuthRoute::login(ReturnDestination::Home);
        let mut state = guest();
        state.begin_refresh().unwrap();
        let issued = state.generation();
        state
            .finish_session(
                issued,
                Err(AccountFailure::LoginRefused(LoginRefusal::Credentials)),
                || uuid::Uuid::from_u128(100).to_string(),
            )
            .unwrap();
        // Refusal: both actions are admitted again and nothing is busy.
        let html = render(route, state, language);
        for id in ["theatre-auth-login", "theatre-auth-register"] {
            assert_busy(&html, id, false);
            assert!(!button_markup(&html, id).0.contains("disabled"), "{id}");
        }
        // Network failure keeps both controls idle: recovery is the refresh action.
        let mut failed = guest();
        failed.begin_refresh().unwrap();
        let issued = failed.generation();
        failed
            .finish_session(issued, Err(AccountFailure::Unavailable), || {
                uuid::Uuid::from_u128(100).to_string()
            })
            .unwrap();
        let html = render(route, failed, language);
        for id in ["theatre-auth-login", "theatre-auth-register"] {
            assert_busy(&html, id, false);
        }
        // Unresolved cleanup blocks both without any spinner.
        let mut pending = guest();
        let cleanup = pending.begin_logout().unwrap();
        pending.finish_logout(cleanup, Err(AccountFailure::Unavailable));
        let html = render(route, pending, language);
        for id in ["theatre-auth-login", "theatre-auth-register"] {
            assert_busy(&html, id, false);
            assert!(button_markup(&html, id).0.contains("disabled"), "{id}");
        }
    }

    #[test]
    fn secondary_action_never_borrows_the_busy_state_of_a_primary_one() {
        let reason = DisabledReason::new("Checking").unwrap();
        assert_eq!(
            secondary_state(ButtonState::Loading(reason.clone())),
            ButtonState::Disabled(reason.clone())
        );
        assert_eq!(
            secondary_state(ButtonState::Disabled(reason.clone())),
            ButtonState::Disabled(reason)
        );
        assert_eq!(secondary_state(ButtonState::Enabled), ButtonState::Enabled);
    }

    #[test]
    fn a_signed_in_listener_sees_no_credential_form() {
        use crate::features::auth_preview::{signed_in, PreviewActor};
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            signed_in(state, PreviewActor::A).unwrap();
            let html = render(
                AuthRoute::login(ReturnDestination::Home),
                state.get_untracked(),
                Locale::English,
            );
            assert!(!html.contains("type=\"password\"") && !html.contains("theatre-login-form"));
            assert!(html.contains("href=\"#account\""));
        });
    }

    #[test]
    fn the_account_created_notice_is_ram_only_and_never_a_session() {
        for language in [Locale::Vietnamese, Locale::English] {
            let with = render_with_signup(
                AuthRoute::login(ReturnDestination::Home),
                guest(),
                language,
                SignupState::default(),
                true,
            );
            assert!(with.contains("id=\"theatre-auth-created\""));
            assert!(with.contains(auth_copy::text(CopyKey::AccountCreatedNotice, language)));
            // Still a guest asking to sign in: the form is present and the account is not open.
            assert!(with.contains("theatre-login-password"));
            assert!(!with.contains("href=\"#account\""));
            let without = render(AuthRoute::login(ReturnDestination::Home), guest(), language);
            assert!(!without.contains("theatre-auth-created"));
        }
    }

    #[test]
    fn registration_has_one_contextual_back_and_a_named_home_escape_in_every_origin() {
        for language in [Locale::Vietnamese, Locale::English] {
            for origin in ORIGINS {
                let html = render_with_signup(
                    AuthRoute::register(origin),
                    guest(),
                    language,
                    SignupState::default(),
                    false,
                );
                assert_eq!(
                    native_button_name(&html, "theatre-auth-back"),
                    auth_copy::text(CopyKey::ReturnLogin, language)
                );
                assert!(html.contains("id=\"theatre-auth-home\""));
                assert!(html.contains("href=\"#discover\""));
                assert!(html.contains(&format!(
                    "aria-label=\"{}\"",
                    auth_copy::text(CopyKey::ReturnHome, language)
                )));
                for absent in [
                    "theatre-register-back\"",
                    "theatre-register-home\"",
                    "theatre-signup-cancel\"",
                    "theatre-signup-login\"",
                    "theatre-login-form",
                    "theatre-login-password",
                ] {
                    assert!(!html.contains(absent), "{origin:?}: {absent}");
                }
                assert_eq!(html.matches("data-variant=\"auth-primary\"").count(), 1);
                let submit = html
                    .split("id=\"theatre-signup-submit\"")
                    .nth(1)
                    .unwrap()
                    .split("</button>")
                    .next()
                    .unwrap();
                let leading_icons: Vec<_> = submit
                    .split("<span")
                    .skip(1)
                    .filter_map(|markup| markup.split_once('>').map(|(opening, _)| opening))
                    .filter(|opening| opening.contains("cui-button-icon"))
                    .collect();
                assert_eq!(leading_icons.len(), 1);
                for opening in leading_icons {
                    assert!(opening.contains("aria-hidden=\"true\""));
                }
            }
        }
    }

    #[test]
    fn a_captured_link_shows_the_password_page_not_the_email_form_and_sends_nothing() {
        let mut enrollment = SignupState::default();
        enrollment.capture(
            ProofLink::from_fragment(&format!(
                "#operation={}&proof={}&secret={}",
                "af".repeat(16),
                "bc".repeat(16),
                "de".repeat(32)
            ))
            .unwrap(),
        );
        for language in [Locale::Vietnamese, Locale::English] {
            let html = render_with_signup(
                AuthRoute::register(ReturnDestination::Home),
                guest(),
                language,
                enrollment.clone(),
                false,
            );
            assert!(
                html.contains("theatre-signup-password") && html.contains("theatre-signup-repeat")
            );
            assert!(!html.contains("theatre-signup-email"));
            assert!(html.contains(auth_copy::text(CopyKey::SignupPasswordHeading, language)));
            assert!(control(&html, "theatre-signup-confirm").contains("disabled"));
            assert!(!html.contains(&"de".repeat(32)));
            assert_eq!(
                native_button_name(&html, "theatre-auth-back"),
                auth_copy::text(CopyKey::ReturnLogin, language)
            );
        }
    }

    #[test]
    fn guest_account_entry_actions_keep_unique_focus_return_ids_in_both_locales() {
        Owner::new().with(|| {
            for language in [Locale::Vietnamese, Locale::English] {
                let state = RwSignal::new(guest());
                let locale = RwSignal::new(language);
                let noop = Callback::new(|()| {});
                for scope in ["theatre-account", "theatre-auth-entry"] {
                    let html = view! { <AuthActions state=state.read_only() locale scope on_login=noop on_register=noop/> }.to_html();
                    for id in [format!("{scope}-login"), format!("{scope}-register")] {
                        assert_eq!(html.matches(&format!("id=\"{id}\"")).count(), 1, "{id}");
                    }
                    assert_eq!(
                        native_button_name(&html, &format!("{scope}-login")),
                        auth_copy::text(CopyKey::LoginSubmit, language)
                    );
                    assert_eq!(
                        native_button_name(&html, &format!("{scope}-register")),
                        auth_copy::text(CopyKey::RegisterAction, language)
                    );
                }
            }
        });
    }

    #[test]
    fn action_availability_equality_stops_button_factories_and_preserves_locale_and_errors() {
        use crate::features::auth_preview::{signed_in, PreviewActor};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            signed_in(state, PreviewActor::A).unwrap();
            let locale = RwSignal::new(Locale::English);
            let action = auth_action_state(state.read_only(), locale);
            let runs = Arc::new(AtomicUsize::new(0));
            let count = runs.clone();
            // AuthActions' Button factories read precisely this Memo's output.
            let factories = Memo::new(move |_| {
                count.fetch_add(1, Ordering::SeqCst);
                action.get()
            });
            // A signed-in listener cannot start another sign-in: equal output across locales.
            assert!(matches!(factories.get(), Some(ButtonState::Disabled(_))));
            let fence = state.with_untracked(AccountViewState::fence).unwrap();
            for _ in 0..3 {
                state.update(|s| {
                    s.seed_sync(|sync, _| {
                        sync.begin_saved_read(&uuid::Uuid::from_u128(88).to_string())
                    });
                });
                state.update(|s| s.record_storage(&fence, Ok(())));
                factories.get();
            }
            assert_eq!(runs.load(Ordering::SeqCst), 1);
            let cleanup = state.write().begin_logout().unwrap();
            state.write().finish_logout(cleanup, Ok(()));
            assert_eq!(factories.get(), Some(ButtonState::Enabled));
            assert_eq!(runs.load(Ordering::SeqCst), 2);
            locale.set(Locale::Vietnamese);
            assert_eq!(factories.get(), Some(ButtonState::Enabled));
            assert_eq!(
                runs.load(Ordering::SeqCst),
                2,
                "enabled ButtonState is equal across locales"
            );
            assert_eq!(
                label(locale, CopyKey::RegisterAction)(),
                auth_copy::text(CopyKey::RegisterAction, Locale::Vietnamese)
            );
            let issued = state.write().begin_refresh().unwrap();
            state
                .write()
                .finish_session(issued, Err(AccountFailure::Protocol), || {
                    uuid::Uuid::from_u128(100).to_string()
                })
                .unwrap();
            let expected = ButtonState::Disabled(
                DisabledReason::new(messages::text(Key::AccountProtocol, Locale::Vietnamese))
                    .unwrap(),
            );
            assert_eq!(factories.get(), Some(expected));
            assert_eq!(runs.load(Ordering::SeqCst), 3);
            locale.set(Locale::English);
            let expected = ButtonState::Disabled(
                DisabledReason::new(messages::text(Key::AccountProtocol, Locale::English)).unwrap(),
            );
            assert_eq!(factories.get(), Some(expected));
            assert_eq!(runs.load(Ordering::SeqCst), 4);
        });
    }

    #[test]
    fn route_back_targets_name_the_actual_internal_destination() {
        for language in [Locale::Vietnamese, Locale::English] {
            for origin in ORIGINS {
                let route = AuthRoute::login(origin);
                let html = render(route, guest(), language);
                assert_eq!(
                    native_button_name(&html, "theatre-auth-back"),
                    auth_copy::text(back_key(route), language)
                );
            }
        }
    }
}
