//! Theatre auth presentation; the shell owns navigation and all identity effects.
use super::{
    account::account_failure_key,
    auth_copy::{self, CopyKey},
    signup::SignupPanel,
};
use crate::{
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthRoute, AuthStep, ReturnDestination},
        registration::{ConfigurationFailure, RegistrationPhase, RegistrationState},
        signup::{SignupPhase, SignupState},
    },
    messages::{self, Key, Locale},
};
use cantos_contracts::theatre_identity::RegistrationUnavailableReason;
use cantos_ui::{
    AccessibleName, BackButton, Button, ButtonState, ButtonVariant, DisabledReason, IconName, Logo,
    LogoTone, LogoVariant, Space, Surface, SurfaceTone, SurfaceVariant,
};
use leptos::prelude::*;

pub(crate) fn label(locale: RwSignal<Locale>, key: CopyKey) -> impl Fn() -> &'static str + Copy {
    move || auth_copy::text(key, locale.get())
}

fn back_key(route: AuthRoute) -> CopyKey {
    match route.step {
        AuthStep::Register => CopyKey::ReturnLogin,
        AuthStep::Login | AuthStep::RegistrationComplete => match route.origin {
            ReturnDestination::Home => CopyKey::ReturnHome,
            ReturnDestination::Account => CopyKey::ReturnAccount,
            ReturnDestination::Membership | ReturnDestination::MembershipFromAccount => {
                CopyKey::ReturnMembership
            }
        },
    }
}

fn blocked_login_key(state: &AccountViewState) -> Key {
    match state.phase() {
        SessionPhase::Idle | SessionPhase::Checking => Key::AccountChecking,
        SessionPhase::Guest => Key::LiveGuestStatus,
        SessionPhase::SignedIn => Key::SignedInListener,
        SessionPhase::Failed(failure) => account_failure_key(failure),
        SessionPhase::LogoutPending if state.cleanup_in_flight() => Key::AccountLogoutPending,
        SessionPhase::LogoutPending => Key::AccountLogoutRetryHelp,
    }
}

fn registration_key(phase: RegistrationPhase) -> CopyKey {
    match phase {
        RegistrationPhase::Idle | RegistrationPhase::Checking => CopyKey::SignupChecking,
        RegistrationPhase::Available => CopyKey::SignupEmailHelp,
        RegistrationPhase::Unavailable(RegistrationUnavailableReason::IdentityUnconfigured) => {
            CopyKey::SignupNotConfigured
        }
        RegistrationPhase::Unavailable(RegistrationUnavailableReason::ProviderUnsupported) => {
            CopyKey::SignupProviderUnsupported
        }
        RegistrationPhase::Unavailable(RegistrationUnavailableReason::RegistrationUnconfigured) => {
            CopyKey::SignupRegistrationUnconfigured
        }
        RegistrationPhase::Failed(ConfigurationFailure::Unavailable) => {
            CopyKey::SignupServiceUnavailable
        }
        RegistrationPhase::Failed(ConfigurationFailure::Protocol) => CopyKey::SignupInvalidResponse,
    }
}

/// The guest CTA pair is the same composition in Login and Account. Availability comes only
/// from the existing session owner; a failure to construct localized reason text hides actions.
#[component]
pub(crate) fn AuthActions(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    scope: &'static str,
    on_login: Callback<()>,
    on_register: Callback<()>,
    #[prop(optional)] preview_icons: bool,
) -> impl IntoView {
    view! {
        <div class="theatre-auth-actions">
            {move || {
                let current = state.get();
                let availability = if current.can_login() {
                    Some(ButtonState::Enabled)
                } else {
                    DisabledReason::new(messages::text(blocked_login_key(&current), locale.get()))
                        .ok()
                        .map(|reason| match current.phase() {
                            SessionPhase::Idle | SessionPhase::Checking => ButtonState::Loading(reason),
                            SessionPhase::LogoutPending if current.cleanup_in_flight() => ButtonState::Loading(reason),
                            SessionPhase::Guest | SessionPhase::SignedIn | SessionPhase::Failed(_) | SessionPhase::LogoutPending => ButtonState::Disabled(reason),
                        })
                };
                availability.map(|availability| view! {
                    <Button id=format!("{scope}-login") variant=ButtonVariant::AuthPrimary
                        icon=if preview_icons { IconName::Login } else { IconName::ArrowForward } state=availability.clone()
                        focus_on_press=true
                        on_press=Callback::new(move |_| on_login.run(()))>
                        {label(locale, CopyKey::CantosIdLogin)}
                    </Button>
                    <Button id=format!("{scope}-register") variant=ButtonVariant::AuthSecondary
                        nostrip:icon=preview_icons.then_some(IconName::PersonAdd)
                        state=availability focus_on_press=true
                        on_press=Callback::new(move |_| on_register.run(()))>
                        {label(locale, CopyKey::CreateAccount)}
                    </Button>
                })
            }}
        </div>
    }
}

#[component]
pub(crate) fn SessionRecovery(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    scope: &'static str,
    on_refresh: Callback<()>,
    on_logout: Callback<()>,
    #[prop(optional)] preview_icons: bool,
) -> impl IntoView {
    view! {
        <div class="theatre-account-recovery">
            <Show when=move || matches!(state.get().phase(), SessionPhase::Failed(_))>
                <Button id=format!("{scope}-refresh") variant=ButtonVariant::AuthSecondary
                    nostrip:icon=preview_icons.then_some(IconName::Refresh)
                    on_press=Callback::new(move |_| on_refresh.run(()))>
                    {move || messages::text(Key::AccountRefresh, locale.get())}
                </Button>
            </Show>
            <Show when=move || state.get().can_retry_logout()>
                <Button id=format!("{scope}-logout-retry") variant=ButtonVariant::AuthSecondary
                    nostrip:icon=preview_icons.then_some(IconName::Refresh)
                    on_press=Callback::new(move |_| on_logout.run(()))>
                    {move || messages::text(Key::AccountLogoutRetry, locale.get())}
                </Button>
            </Show>
        </div>
    }
}

#[component]
pub(crate) fn AuthPage(
    state: RwSignal<AccountViewState>,
    registration: RwSignal<RegistrationState>,
    signup: RwSignal<SignupState>,
    locale: RwSignal<Locale>,
    route: AuthRoute,
    on_back: Callback<()>,
    on_register: Callback<()>,
    on_login: Callback<()>,
    on_home: Callback<()>,
    on_membership: Callback<()>,
    on_refresh: Callback<()>,
    on_logout: Callback<()>,
    on_registration_refresh: Callback<()>,
    on_signup_submit: Callback<()>,
    on_signup_verify: Callback<()>,
    on_signup_new: Callback<()>,
    on_signup_cancel: Callback<()>,
    on_signup_login: Callback<()>,
    on_signup_setup: Callback<()>,
    /// Enabled only by the existing explicit auth-preview composition root. This is not
    /// shared icon admission or a live fallback when a dependency is unavailable.
    #[prop(optional)]
    preview_icons: bool,
) -> impl IntoView {
    // Success is unavailable until a provider registration receipt exists. In particular a
    // reconstructed completion URL is not a receipt and never opens Membership success actions.
    let _ = on_membership;
    let is_login = route.step == AuthStep::Login;
    let heading = if is_login {
        CopyKey::LoginHeading
    } else {
        CopyKey::RegisterHeading
    };
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
            <div class="theatre-auth-layout">
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
                <div class="theatre-auth-copy">
                    <p class="theatre-eyebrow">{label(locale, if is_login { CopyKey::LoginEyebrow } else { CopyKey::RegisterEyebrow })}</p>
                    <h1 id="theatre-auth-heading">{label(locale, heading)}</h1>
                    <p class="theatre-muted">{label(locale, if is_login { CopyKey::LoginHelp } else { CopyKey::RegisterHelp })}</p>
                    <p id="theatre-auth-status" role="status" aria-live="polite">
                        {move || messages::text(blocked_login_key(&state.get()), locale.get())}
                    </p>
                    {if is_login {
                        view! {
                            <AuthActions state locale scope="theatre-auth" on_login on_register preview_icons/>
                            <p class="theatre-auth-helper theatre-muted">{label(locale, CopyKey::ProviderHelp)}</p>
                            <Show when=move || state.get().phase() == SessionPhase::SignedIn>
                                <p>{label(locale, CopyKey::SignedInActionHelp)}</p>
                                <a class="theatre-text-link" href="#account">{move || messages::text(Key::Account, locale.get())}</a>
                            </Show>
                        }.into_any()
                    } else {
                        view! {
                            <Show when=move || registration.get().phase() == RegistrationPhase::Available || signup.get().phase() != SignupPhase::Editing
                                fallback=move || view! {
                            <div class="theatre-auth-actions">
                                <div id="theatre-registration-status" role="status" aria-live="polite" aria-atomic="true">
                                {move || {
                                    let phase = registration.get().phase();
                                    DisabledReason::new(auth_copy::text(registration_key(phase), locale.get())).ok().map(|reason| view! {
                                    <Button id="theatre-registration-unavailable" variant=ButtonVariant::AuthPrimary
                                        nostrip:icon=preview_icons.then_some(IconName::PersonAdd)
                                        state=if matches!(phase, RegistrationPhase::Idle | RegistrationPhase::Checking) { ButtonState::Loading(reason) } else { ButtonState::Disabled(reason) }>
                                        {label(locale, CopyKey::CreateAccount)}
                                    </Button>
                                })}}
                                </div>
                                <Show when=move || matches!(registration.get().phase(), RegistrationPhase::Failed(_))>
                                    <Button id="theatre-registration-refresh" variant=ButtonVariant::AuthSecondary
                                        nostrip:icon=preview_icons.then_some(IconName::Refresh)
                                        on_press=Callback::new(move |_| on_registration_refresh.run(()))>
                                        {label(locale, CopyKey::SignupRetry)}
                                    </Button>
                                </Show>
                            </div>
                                }>
                                <SignupPanel state=signup locale on_submit=on_signup_submit on_verify=on_signup_verify
                                    login_via_back=route.step == AuthStep::Register preview_icons
                                    on_new_request=on_signup_new on_cancel=on_signup_cancel on_login=on_signup_login on_setup=on_signup_setup/>
                            </Show>
                            {(route.step == AuthStep::RegistrationComplete).then(|| view! {
                                <p role="status">{label(locale, CopyKey::RegistrationUnconfirmed)}</p>
                            })}
                        }.into_any()
                    }}
                    <SessionRecovery state locale scope="theatre-auth" on_refresh on_logout preview_icons/>
                </div>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::account_view::{AccountFailure, SessionCompletion};

    fn guest() -> AccountViewState {
        let mut state = AccountViewState::default();
        let issued = state.begin_refresh().unwrap();
        let SessionCompletion::Cleanup(cleanup) = state
            .finish_session(issued, Err(AccountFailure::SignedOut))
            .unwrap()
        else {
            panic!("guest lookup requires cleanup")
        };
        assert!(state.finish_logout(cleanup, Ok(())));
        state
    }

    fn render(route: AuthRoute, initial: AccountViewState, language: Locale) -> String {
        let mut availability = RegistrationState::default();
        let issued = availability.begin().unwrap();
        assert!(availability.finish(
            issued,
            Ok(cantos_contracts::theatre_identity::Configuration {
                login: cantos_contracts::theatre_identity::LoginMethod::Oidc,
                registration: cantos_contracts::theatre_identity::Registration::Unavailable {
                    reason: RegistrationUnavailableReason::ProviderUnsupported,
                },
            })
        ));
        render_with_registration(route, initial, language, availability)
    }

    fn render_with_registration(
        route: AuthRoute,
        initial: AccountViewState,
        language: Locale,
        availability: RegistrationState,
    ) -> String {
        render_with_signup(
            route,
            initial,
            language,
            availability,
            SignupState::default(),
            false,
        )
    }

    fn render_with_signup(
        route: AuthRoute,
        initial: AccountViewState,
        language: Locale,
        availability: RegistrationState,
        enrollment: SignupState,
        preview_icons: bool,
    ) -> String {
        Owner::new().with(|| {
            let state = RwSignal::new(initial);
            let locale = RwSignal::new(language);
            let registration = RwSignal::new(availability);
            let signup = RwSignal::new(enrollment);
            let noop = Callback::new(|()| {});
            view! { <AuthPage state registration signup locale route preview_icons on_back=noop on_register=noop on_login=noop
            on_home=noop on_membership=noop on_refresh=noop on_logout=noop on_registration_refresh=noop
            on_signup_submit=noop on_signup_verify=noop on_signup_new=noop on_signup_cancel=noop on_signup_login=noop on_signup_setup=noop/> }
            .to_html()
        })
    }

    #[test]
    fn registration_has_one_contextual_back_and_a_named_home_escape_in_every_origin() {
        use cantos_contracts::theatre_identity::{Configuration, LoginMethod, Registration};
        for language in [Locale::Vietnamese, Locale::English] {
            for origin in [
                ReturnDestination::Home,
                ReturnDestination::Account,
                ReturnDestination::Membership,
                ReturnDestination::MembershipFromAccount,
            ] {
                let mut availability = RegistrationState::default();
                let issued = availability.begin().unwrap();
                assert!(availability.finish(
                    issued,
                    Ok(Configuration {
                        login: LoginMethod::Oidc,
                        registration: Registration::EmailVerification {}
                    })
                ));
                for preview_icons in [false, true] {
                    let html = render_with_signup(
                        AuthRoute::register(origin),
                        guest(),
                        language,
                        availability,
                        SignupState::default(),
                        preview_icons,
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
                    assert_eq!(leading_icons.len(), usize::from(preview_icons));
                    for opening in leading_icons {
                        assert!(opening.contains("aria-hidden=\"true\""));
                    }
                }
            }
        }
    }

    #[test]
    fn completed_setup_reuses_register_back_but_completion_origin_keeps_distinct_login() {
        use crate::application::signup::VerificationToken;
        use cantos_contracts::theatre_registration::VerificationStatus;
        let mut enrollment = SignupState::default();
        enrollment.capture(
            VerificationToken::from_fragment(&format!(
                "#register-verify?token={}",
                "af".repeat(32)
            ))
            .unwrap(),
        );
        let (issued, _) = enrollment.begin_verify().unwrap();
        assert!(enrollment.finish_verify(issued, Ok(VerificationStatus::SetupLinkSent)));
        for language in [Locale::Vietnamese, Locale::English] {
            for origin in [
                ReturnDestination::Home,
                ReturnDestination::Account,
                ReturnDestination::Membership,
                ReturnDestination::MembershipFromAccount,
            ] {
                for route in [
                    AuthRoute::register(origin),
                    AuthRoute::registration_complete(origin),
                ] {
                    let html = render_with_signup(
                        route,
                        guest(),
                        language,
                        RegistrationState::default(),
                        enrollment.clone(),
                        false,
                    );
                    assert_eq!(
                        html.contains("id=\"theatre-signup-login\""),
                        route.step == AuthStep::RegistrationComplete
                    );
                    assert_eq!(
                        native_button_name(&html, "theatre-auth-back"),
                        auth_copy::text(back_key(route), language)
                    );
                    assert!(!html.contains("id=\"theatre-signup-cancel\""));
                }
            }
        }
    }

    #[test]
    fn registration_loading_unconfigured_unsupported_and_failures_stay_closed_with_named_recovery()
    {
        use cantos_contracts::theatre_identity::{Configuration, LoginMethod, Registration};
        for language in [Locale::Vietnamese, Locale::English] {
            for (result, key, retry) in [
                (None, CopyKey::SignupChecking, false),
                (
                    Some(Ok(Configuration {
                        login: LoginMethod::Unconfigured,
                        registration: Registration::Unavailable {
                            reason: RegistrationUnavailableReason::IdentityUnconfigured,
                        },
                    })),
                    CopyKey::SignupNotConfigured,
                    false,
                ),
                (
                    Some(Ok(Configuration {
                        login: LoginMethod::Oidc,
                        registration: Registration::Unavailable {
                            reason: RegistrationUnavailableReason::ProviderUnsupported,
                        },
                    })),
                    CopyKey::SignupProviderUnsupported,
                    false,
                ),
                (
                    Some(Err(ConfigurationFailure::Unavailable)),
                    CopyKey::SignupServiceUnavailable,
                    true,
                ),
                (
                    Some(Err(ConfigurationFailure::Protocol)),
                    CopyKey::SignupInvalidResponse,
                    true,
                ),
            ] {
                let mut availability = RegistrationState::default();
                let issued = availability.begin().unwrap();
                if let Some(result) = result {
                    assert!(availability.finish(issued, result));
                }
                let html = render_with_registration(
                    AuthRoute::register(ReturnDestination::Account),
                    guest(),
                    language,
                    availability,
                );
                assert!(html.contains(auth_copy::text(key, language)));
                assert!(html.contains("id=\"theatre-registration-status\" role=\"status\" aria-live=\"polite\" aria-atomic=\"true\""));
                assert_eq!(html.contains("id=\"theatre-registration-refresh\""), retry);
                let opening = html
                    .split("id=\"theatre-registration-unavailable\"")
                    .nth(1)
                    .unwrap()
                    .split('>')
                    .next()
                    .unwrap();
                assert!(opening.contains("disabled"));
                assert!(opening.contains("theatre-registration-unavailable-reason"));
                for forbidden in [
                    "<input",
                    "<form",
                    "type=\"password\"",
                    "Registration complete",
                    "Cantos Theatre",
                ] {
                    assert!(!html.contains(forbidden), "{forbidden}");
                }
            }
        }
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

    #[test]
    fn login_reuses_capsule_actions_in_vertical_order_and_internal_context_names() {
        for (origin, back) in [
            (ReturnDestination::Home, "Về Trang chủ"),
            (ReturnDestination::Account, "Về Tài khoản"),
            (ReturnDestination::Membership, "Về Membership"),
            (ReturnDestination::MembershipFromAccount, "Về Membership"),
        ] {
            let html = render(AuthRoute::login(origin), guest(), Locale::Vietnamese);
            assert!(html.contains(back));
            assert!(html.contains("data-logo-variant=\"cantos\""));
            assert!(
                html.find("Đăng nhập với Cantos ID").unwrap()
                    < html.find("Tạo tài khoản Cantos").unwrap()
            );
            assert!(html.contains("data-variant=\"auth-primary\""));
            assert!(html.contains("data-variant=\"auth-secondary\""));
            for forbidden in [
                "<input",
                "<form",
                "type=\"password\"",
                "role=\"tab\"",
                "Cantos Theatre",
                "Điều khoản",
                "Quyền riêng tư",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn registration_is_unavailable_without_password_collection_or_fabricated_completion() {
        for step in [AuthStep::Register, AuthStep::RegistrationComplete] {
            let html = render(
                AuthRoute {
                    step,
                    origin: ReturnDestination::Account,
                },
                guest(),
                Locale::English,
            );
            assert!(html.contains("Account creation is unavailable"));
            assert!(html.contains("theatre-registration-unavailable-reason"));
            assert!(html.contains(match step {
                AuthStep::Register => "Back to sign-in",
                AuthStep::RegistrationComplete => "Back to Account",
                AuthStep::Login => unreachable!("registration cases only"),
            }));
            for forbidden in [
                "<input",
                "<form",
                "type=\"password\"",
                "Sign-up complete",
                "Registration complete",
                "membership-plans",
                "/register",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn completion_fallback_back_copy_names_its_actual_internal_destination() {
        use crate::application::auth_navigation::{MembershipReturn, NavigationTarget};

        for (origin, expected, copy) in [
            (
                ReturnDestination::Home,
                NavigationTarget::Home,
                "Back to Home",
            ),
            (
                ReturnDestination::Account,
                NavigationTarget::Account,
                "Back to Account",
            ),
            (
                ReturnDestination::Membership,
                NavigationTarget::Membership(MembershipReturn::Home),
                "Back to Membership",
            ),
            (
                ReturnDestination::MembershipFromAccount,
                NavigationTarget::Membership(MembershipReturn::Account),
                "Back to Membership",
            ),
        ] {
            let route = AuthRoute::registration_complete(origin);
            assert_eq!(route.back(), expected);
            let html = render(route, guest(), Locale::English);
            assert_eq!(
                native_button_name(&html, "theatre-auth-back"),
                copy,
                "{origin:?}"
            );
            assert!(!html.contains("id=\"theatre-register-back\""));
            assert!(html.contains("Account registration has not been confirmed"));
        }
    }

    #[test]
    fn guest_login_actions_keep_unique_focus_return_ids_in_both_locales() {
        for language in [Locale::Vietnamese, Locale::English] {
            let html = render(AuthRoute::login(ReturnDestination::Home), guest(), language);
            for id in [
                "theatre-auth-back",
                "theatre-auth-login",
                "theatre-auth-register",
            ] {
                assert_eq!(html.matches(&format!("id=\"{id}\"")).count(), 1, "{id}");
                let open = html
                    .split(&format!("id=\"{id}\""))
                    .nth(1)
                    .unwrap()
                    .split('>')
                    .next()
                    .unwrap();
                assert!(!open.contains("disabled"), "{id}: {open}");
            }
        }
    }

    #[test]
    fn unresolved_cleanup_never_enables_a_new_login_or_register_intent() {
        let mut state = guest();
        let cleanup = state.begin_logout().unwrap();
        state.finish_logout(cleanup, Err(AccountFailure::Unavailable));
        let html = render(
            AuthRoute::login(ReturnDestination::Home),
            state,
            Locale::Vietnamese,
        );
        for id in ["theatre-auth-login", "theatre-auth-register"] {
            let open = html
                .split(&format!("id=\"{id}\""))
                .nth(1)
                .unwrap()
                .split('>')
                .next()
                .unwrap();
            assert!(open.contains("disabled"), "{id}: {open}");
        }
        assert!(html.contains("Hoàn tất đăng xuất"));
    }
}
