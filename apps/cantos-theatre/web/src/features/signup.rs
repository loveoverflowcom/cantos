//! Web-owned email enrollment composition using the shared field and auth action variants.
use super::{
    auth::label,
    auth_copy::{self, CopyKey},
};
use crate::{
    application::signup::{SignupFailure, SignupPhase, SignupState},
    messages::Locale,
};
use cantos_ui::{
    Button, ButtonState, ButtonVariant, DisabledReason, FieldState, IconName, TextField,
    TextFieldKind, TextFieldVariant,
};
use leptos::prelude::*;

pub(crate) fn phase_key(phase: SignupPhase) -> CopyKey {
    match phase {
        SignupPhase::Editing => CopyKey::SignupEmailHelp,
        SignupPhase::Submitting => CopyKey::SignupSubmitting,
        SignupPhase::EmailAccepted => CopyKey::SignupEmailAccepted,
        SignupPhase::SubmitFailed(SignupFailure::InvalidEmail) => CopyKey::SignupInvalidEmail,
        SignupPhase::SubmitFailed(SignupFailure::Unavailable) => CopyKey::SignupRequestUnavailable,
        SignupPhase::SubmitFailed(SignupFailure::UnknownOutcome | SignupFailure::Protocol) => {
            CopyKey::SignupRequestUnknown
        }
        SignupPhase::AwaitingConfirmation => CopyKey::SignupConfirmHelp,
        SignupPhase::Verifying => CopyKey::SignupVerifying,
        SignupPhase::SetupLinkSent => CopyKey::SignupSetupLinkSent,
        SignupPhase::SetupReady => CopyKey::SignupSetupReady,
        SignupPhase::SetupOpened => CopyKey::SignupSetupOpened,
        SignupPhase::LinkUnavailable => CopyKey::SignupLinkUnavailable,
        SignupPhase::VerificationFailed(
            SignupFailure::Unavailable | SignupFailure::InvalidEmail,
        ) => CopyKey::SignupVerificationUnavailable,
        SignupPhase::VerificationFailed(
            SignupFailure::UnknownOutcome | SignupFailure::Protocol,
        ) => CopyKey::SignupVerificationUnknown,
        SignupPhase::CaptureFailed => CopyKey::SignupCaptureFailed,
    }
}

fn status_key(state: &SignupState) -> CopyKey {
    match (state.is_firebase(), state.phase()) {
        (
            true,
            SignupPhase::VerificationFailed(
                SignupFailure::UnknownOutcome | SignupFailure::Protocol,
            ),
        ) => CopyKey::SignupFirebaseUnknown,
        (
            true,
            SignupPhase::VerificationFailed(
                SignupFailure::Unavailable | SignupFailure::InvalidEmail,
            ),
        ) => CopyKey::SignupFirebaseUnavailable,
        _ => phase_key(state.phase()),
    }
}

/// Presentation only: cancellation authority and request fencing remain in the signup owner.
/// Email Back already cancels its proof. Only an unconsumed Firebase proof has a distinct
/// explicit revoke action; once confirmation starts, neither Back nor Cancel can undo it.
fn show_explicit_cancel(state: &SignupState) -> bool {
    state.is_firebase() && state.phase() == SignupPhase::AwaitingConfirmation
}

fn show_login(state: &SignupState) -> bool {
    matches!(
        state.phase(),
        SignupPhase::SetupLinkSent | SignupPhase::SetupOpened
    ) || (state.is_firebase() && matches!(state.phase(), SignupPhase::VerificationFailed(_)))
}

fn focus_target(state: &SignupState, login_via_back: bool) -> &'static str {
    match state.phase() {
        SignupPhase::Editing | SignupPhase::SubmitFailed(_) => "theatre-signup-email",
        SignupPhase::Submitting | SignupPhase::Verifying => "theatre-signup-status",
        SignupPhase::AwaitingConfirmation => "theatre-signup-confirm",
        SignupPhase::SetupReady => "theatre-signup-open-setup",
        _ if show_login(state) => {
            if login_via_back {
                "theatre-signup-status"
            } else {
                "theatre-signup-login"
            }
        }
        SignupPhase::EmailAccepted
        | SignupPhase::SetupLinkSent
        | SignupPhase::SetupOpened
        | SignupPhase::LinkUnavailable
        | SignupPhase::VerificationFailed(_)
        | SignupPhase::CaptureFailed => "theatre-signup-new-request",
    }
}

/// Allocate controlled props in the panel owner, before the form's conditional child owner.
/// Show may dispose/recreate the form; none of its retained props may live in that child.
#[derive(Clone, Copy)]
struct EmailFieldBindings {
    label: Signal<String>,
    value: Signal<String>,
    state: Signal<FieldState>,
    error: Signal<Option<String>>,
    on_input: Callback<String>,
}

impl EmailFieldBindings {
    fn new(signup: RwSignal<SignupState>, locale: RwSignal<Locale>) -> Self {
        Self {
            label: Signal::derive(move || {
                auth_copy::text(CopyKey::SignupEmail, locale.get()).to_owned()
            }),
            value: Signal::derive(move || signup.get().email().to_owned()),
            state: Signal::derive(move || {
                if signup.get().phase() == SignupPhase::Submitting {
                    FieldState::ReadOnly
                } else {
                    FieldState::Editable
                }
            }),
            error: Signal::derive(move || {
                (signup.get().phase() == SignupPhase::SubmitFailed(SignupFailure::InvalidEmail))
                    .then(|| auth_copy::text(CopyKey::SignupInvalidEmail, locale.get()).to_owned())
            }),
            on_input: Callback::new(move |value| signup.update(|current| current.set_email(value))),
        }
    }
}

/// This boundary stays visible independently of the navigation parser while a proof URL could
/// not be scrubbed. Only explicit successful scrub or real navigation to a safe URL releases it.
#[component]
pub(crate) fn SignupLinkSafety(
    blocked: RwSignal<bool>,
    locale: RwSignal<Locale>,
    on_retry: Callback<()>,
    children: ChildrenFn,
) -> impl IntoView {
    view! {
        <Show when=move || !blocked.get() fallback=move || view! {
            <section class="theatre-auth-copy" aria-labelledby="theatre-signup-history-heading">
                <h1 id="theatre-signup-history-heading">{label(locale, CopyKey::RegisterHeading)}</h1>
                <p id="theatre-signup-history-status" role="alert">{label(locale, CopyKey::SignupHistoryBlocked)}</p>
                <Button id="theatre-signup-clear-link" variant=ButtonVariant::AuthPrimary
                    on_press=Callback::new(move |_| on_retry.run(()))>{label(locale, CopyKey::SignupClearLink)}</Button>
            </section>
        }>{children()}</Show>
    }
}

#[component]
pub(crate) fn SignupPanel(
    state: RwSignal<SignupState>,
    locale: RwSignal<Locale>,
    on_submit: Callback<()>,
    on_verify: Callback<()>,
    on_new_request: Callback<()>,
    on_cancel: Callback<()>,
    on_login: Callback<()>,
    on_setup: Callback<()>,
    /// Register's contextual Back already returns to sign-in, including local cleanup.
    #[prop(optional)]
    login_via_back: bool,
    /// New icon mappings stay in the explicit no-network diagnostic until shared admission.
    #[prop(optional)]
    preview_icons: bool,
) -> impl IntoView {
    let email = EmailFieldBindings::new(state, locale);
    let panel = NodeRef::<leptos::html::Div>::new();
    let previous_phase = StoredValue::new(state.get_untracked().phase());
    Effect::new(move |_| {
        let current = state.get();
        if previous_phase.get_value() != current.phase() {
            previous_phase.set_value(current.phase());
            crate::platform::focus_in_signup(panel, focus_target(&current, login_via_back));
        }
    });
    let submit_press = Callback::new(move |_: leptos::ev::MouseEvent| on_submit.run(()));
    let verify_press = Callback::new(move |_: leptos::ev::MouseEvent| on_verify.run(()));
    let new_request_press = Callback::new(move |_: leptos::ev::MouseEvent| on_new_request.run(()));
    let cancel_press = Callback::new(move |_: leptos::ev::MouseEvent| on_cancel.run(()));
    let login_press = Callback::new(move |_: leptos::ev::MouseEvent| on_login.run(()));
    let setup_press = Callback::new(move |_: leptos::ev::MouseEvent| on_setup.run(()));
    let form_visible = move || {
        matches!(
            state.get().phase(),
            SignupPhase::Editing | SignupPhase::Submitting | SignupPhase::SubmitFailed(_)
        )
    };
    let request_pending = move || {
        matches!(
            state.get().phase(),
            SignupPhase::Submitting | SignupPhase::Verifying
        )
    };
    view! {
        <div class="theatre-signup" node_ref=panel>
            <p id="theatre-signup-status" tabindex="-1" role="status" aria-live="polite" aria-atomic="true">
                {move || auth_copy::text(status_key(&state.get()), locale.get())}
            </p>
            <Show when=form_visible>
                <form class="theatre-auth-actions" on:submit=move |event| { event.prevent_default(); on_submit.run(()); }>
                    <TextField id="theatre-signup-email" kind=TextFieldKind::Email variant=TextFieldVariant::AuthTonal
                        label=email.label
                        value=email.value
                        autocomplete="email" name="email" required=true
                        state=email.state
                        error=email.error
                        on_input=email.on_input/>
                    {move || {
                        let availability = if request_pending() {
                            DisabledReason::new(auth_copy::text(CopyKey::SignupSubmitting, locale.get())).ok().map(ButtonState::Loading)
                        } else { Some(ButtonState::Enabled) };
                        availability.map(|availability| view! {
                            <Button id="theatre-signup-submit" variant=ButtonVariant::AuthPrimary state=availability
                                nostrip:icon=preview_icons.then_some(IconName::Mail)
                                on_press=submit_press>{label(locale, CopyKey::SignupSendEmail)}</Button>
                        })
                    }}
                </form>
            </Show>
            <Show when=move || matches!(state.get().phase(), SignupPhase::AwaitingConfirmation | SignupPhase::Verifying)>
                <div class="theatre-auth-actions">
                    {move || {
                        let availability = if request_pending() {
                            DisabledReason::new(auth_copy::text(CopyKey::SignupVerifying, locale.get())).ok().map(ButtonState::Loading)
                        } else { Some(ButtonState::Enabled) };
                        availability.map(|availability| view! {
                            <Button id="theatre-signup-confirm" variant=ButtonVariant::AuthPrimary state=availability
                                nostrip:icon=preview_icons.then_some(IconName::Mail)
                                on_press=verify_press>{label(locale, CopyKey::SignupConfirm)}</Button>
                        })
                    }}
                </div>
            </Show>
            <div class="theatre-auth-actions">
                <Show when=move || state.get().phase() == SignupPhase::SetupReady>
                    <p><span>{label(locale, CopyKey::SignupSetupLogin)}</span>": "<strong>{move || state.get().setup_login().unwrap_or_default().to_owned()}</strong></p>
                    <Button id="theatre-signup-open-setup" variant=ButtonVariant::AuthPrimary
                        nostrip:icon=preview_icons.then_some(IconName::ArrowForward)
                        on_press=setup_press>{label(locale, CopyKey::SignupOpenSetup)}</Button>
                </Show>
                <Show when=move || !form_visible() && !matches!(state.get().phase(), SignupPhase::AwaitingConfirmation | SignupPhase::Verifying | SignupPhase::SetupReady | SignupPhase::SetupOpened)
                    && !show_login(&state.get())>
                    <Button id="theatre-signup-new-request" variant=ButtonVariant::AuthPrimary
                        nostrip:icon=preview_icons.then_some(IconName::Refresh)
                        on_press=new_request_press>{label(locale, CopyKey::SignupNewRequest)}</Button>
                </Show>
                <Show when=move || !login_via_back && show_login(&state.get())>
                    <Button id="theatre-signup-login" variant=ButtonVariant::AuthPrimary
                        nostrip:icon=preview_icons.then_some(IconName::Login)
                        on_press=login_press>{label(locale, CopyKey::ReturnLogin)}</Button>
                </Show>
            </div>
            <Show when=move || show_explicit_cancel(&state.get())>
                <Button id="theatre-signup-cancel" variant=ButtonVariant::Text
                    nostrip:icon=preview_icons.then_some(IconName::Close)
                    on_press=cancel_press>{label(locale, CopyKey::SignupCancel)}</Button>
                <p class="theatre-auth-helper theatre-muted">{label(locale, CopyKey::SignupCancelHelp)}</p>
            </Show>
        </div>
    }
}

#[cfg(feature = "preview")]
/// Mounted only by the explicit auth-preview root, never a live transport fallback.
#[component]
pub(crate) fn SignupPreviewControls(
    state: RwSignal<SignupState>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    use crate::application::signup::VerificationToken;
    use cantos_contracts::theatre_registration::VerificationStatus;
    view! {
        <section class="theatre-auth-preview" aria-label=label(locale, CopyKey::SignupFixtureNotice)>
            <p>{label(locale, CopyKey::SignupFixtureNotice)}</p>
            <div class="theatre-actions">
                <Button id="theatre-signup-fixture-link" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(token) = VerificationToken::from_fragment(&format!("#register-verify?token={}", "af".repeat(32))) { current.capture(token); }
                }))>{label(locale, CopyKey::SignupFixtureLink)}</Button>
                <Button id="theatre-signup-fixture-firebase" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    let query = "?mode=verifyEmail&oobCode=fixture-action&continueUrl=https%3A%2F%2Fcantos.example.invalid%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001";
                    if let Some(proof) = VerificationToken::from_firebase_query(query, "https://cantos.example.invalid") { current.capture(proof); }
                }))>{label(locale, CopyKey::SignupFixtureFirebase)}</Button>
                <Button id="theatre-signup-fixture-browser-setup" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() {
                        if let Some(setup) = crate::application::signup::SetupLink::new("listener_fixture".to_owned(), "https://idm.example.invalid/ui/reset?token=fixture-only".to_owned()) {
                            current.finish_firebase(issued, Ok(crate::application::signup::FirebaseVerification::SetupReady(setup)));
                        }
                    }
                }))>{label(locale, CopyKey::SignupFixtureBrowserSetup)}</Button>
                <Button id="theatre-signup-fixture-accepted" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_submit(issued, Ok(())); }
                }))>{label(locale, CopyKey::SignupFixtureAccepted)}</Button>
                <Button id="theatre-signup-fixture-setup" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_verify(issued, Ok(VerificationStatus::SetupLinkSent)); }
                }))>{label(locale, CopyKey::SignupFixtureSetup)}</Button>
                <Button id="theatre-signup-fixture-unavailable" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() {
                        if current.is_firebase() { current.finish_firebase(issued, Ok(crate::application::signup::FirebaseVerification::Unavailable)); }
                        else { current.finish_verify(issued, Ok(VerificationStatus::Unavailable)); }
                    }
                }))>{label(locale, CopyKey::SignupFixtureUnavailable)}</Button>
                <Button id="theatre-signup-fixture-unknown" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() {
                        if current.phase() == SignupPhase::Submitting { current.finish_submit(issued, Err(SignupFailure::UnknownOutcome)); }
                        else if current.is_firebase() { current.finish_firebase(issued, Err(SignupFailure::UnknownOutcome)); }
                        else { current.finish_verify(issued, Err(SignupFailure::UnknownOutcome)); }
                    }
                }))>{label(locale, CopyKey::SignupFixtureUnknown)}</Button>
                <Button id="theatre-signup-fixture-invalid" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_submit(issued, Err(SignupFailure::InvalidEmail)); }
                }))>{label(locale, CopyKey::SignupFixtureInvalid)}</Button>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::signup::VerificationToken;

    fn render(initial: SignupState, language: Locale) -> String {
        render_options(initial, language, false, false)
    }

    fn render_options(
        initial: SignupState,
        language: Locale,
        login_via_back: bool,
        preview_icons: bool,
    ) -> String {
        Owner::new().with(|| {
            let state = RwSignal::new(initial);
            let locale = RwSignal::new(language);
            let noop = Callback::new(|()| {});
            view! { <SignupPanel state locale login_via_back preview_icons on_submit=noop on_verify=noop on_new_request=noop on_cancel=noop on_login=noop on_setup=noop/> }.to_html()
        })
    }

    fn proof(firebase: bool) -> SignupState {
        let mut state = SignupState::default();
        let token = if firebase {
            VerificationToken::from_firebase_query(
                "?mode=verifyEmail&oobCode=fixture-action&continueUrl=https%3A%2F%2Fcantos.example.invalid%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001",
                "https://cantos.example.invalid",
            ).unwrap()
        } else {
            VerificationToken::from_fragment(&format!("#register-verify?token={}", "af".repeat(32)))
                .unwrap()
        };
        state.capture(token);
        state
    }

    #[test]
    fn every_signup_phase_has_one_next_action_without_duplicate_login_or_inert_cancel() {
        use crate::application::signup::{FirebaseVerification, SetupLink};
        use cantos_contracts::theatre_registration::VerificationStatus;
        let mut cases = vec![(
            SignupState::default(),
            "submit",
            false,
            "theatre-signup-email",
        )];
        let mut submitting = SignupState::default();
        submitting.set_email("listener@example.invalid".to_owned());
        let (issued, _) = submitting.begin_submit().unwrap();
        cases.push((submitting.clone(), "submit", false, "theatre-signup-status"));
        let mut accepted = submitting.clone();
        assert!(accepted.finish_submit(issued, Ok(())));
        cases.push((accepted, "new-request", false, "theatre-signup-new-request"));
        for failure in [
            SignupFailure::InvalidEmail,
            SignupFailure::Unavailable,
            SignupFailure::UnknownOutcome,
            SignupFailure::Protocol,
        ] {
            let mut failed = submitting.clone();
            assert!(failed.finish_submit(issued, Err(failure)));
            cases.push((failed, "submit", false, "theatre-signup-email"));
            for firebase in [false, true] {
                let mut failed = proof(firebase);
                let (issued, _) = failed.begin_verify().unwrap();
                if firebase {
                    assert!(failed.finish_firebase(issued, Err(failure)));
                } else {
                    assert!(failed.finish_verify(issued, Err(failure)));
                }
                cases.push((
                    failed,
                    if firebase { "login" } else { "new-request" },
                    false,
                    if firebase {
                        "theatre-signup-login"
                    } else {
                        "theatre-signup-new-request"
                    },
                ));
            }
        }
        for firebase in [false, true] {
            let mut verifying = proof(firebase);
            cases.push((
                verifying.clone(),
                "confirm",
                firebase,
                "theatre-signup-confirm",
            ));
            let (issued, _) = verifying.begin_verify().unwrap();
            cases.push((verifying.clone(), "confirm", false, "theatre-signup-status"));
            let mut unavailable = verifying.clone();
            if firebase {
                assert!(unavailable.finish_firebase(issued, Ok(FirebaseVerification::Unavailable)));
            } else {
                assert!(unavailable.finish_verify(issued, Ok(VerificationStatus::Unavailable)));
            }
            cases.push((
                unavailable,
                "new-request",
                false,
                "theatre-signup-new-request",
            ));
            if firebase {
                assert!(verifying.finish_firebase(
                    issued,
                    Ok(FirebaseVerification::SetupReady(
                        SetupLink::new(
                            "listener_fixture".to_owned(),
                            "https://idm.example.invalid/ui/reset?token=fixture".to_owned()
                        )
                        .unwrap()
                    ))
                ));
                cases.push((
                    verifying.clone(),
                    "open-setup",
                    false,
                    "theatre-signup-open-setup",
                ));
                let _ = verifying.take_setup_url().unwrap();
                cases.push((verifying.clone(), "login", false, "theatre-signup-login"));
                verifying.setup_navigation_failed();
                cases.push((verifying, "login", false, "theatre-signup-login"));
            } else {
                assert!(verifying.finish_verify(issued, Ok(VerificationStatus::SetupLinkSent)));
                cases.push((verifying, "login", false, "theatre-signup-login"));
            }
        }
        let mut capture_failed = SignupState::default();
        capture_failed.capture_failed();
        cases.push((
            capture_failed,
            "new-request",
            false,
            "theatre-signup-new-request",
        ));
        for (state, primary, cancel, focus) in cases {
            for locale in [Locale::Vietnamese, Locale::English] {
                for login_via_back in [false, true] {
                    let html = render_options(state.clone(), locale, login_via_back, true);
                    let expected_primary = if primary == "login" && login_via_back {
                        None
                    } else {
                        Some(primary)
                    };
                    assert_eq!(
                        html.matches("data-variant=\"auth-primary\"").count(),
                        usize::from(expected_primary.is_some()),
                        "{:?}",
                        state.phase()
                    );
                    for id in ["submit", "confirm", "new-request", "open-setup", "login"] {
                        assert_eq!(
                            html.contains(&format!("id=\"theatre-signup-{id}\"")),
                            expected_primary == Some(id),
                            "{:?}: {id}",
                            state.phase()
                        );
                    }
                    assert_eq!(html.contains("id=\"theatre-signup-cancel\""), cancel);
                    let expected_focus = if primary == "login" && login_via_back {
                        "theatre-signup-status"
                    } else {
                        focus
                    };
                    assert_eq!(focus_target(&state, login_via_back), expected_focus);
                    assert!(html.contains(&format!("id=\"{expected_focus}\"")));
                    assert!(!html.contains("fixture-action"));
                }
            }
        }
    }

    #[test]
    fn cancellation_is_only_offered_while_a_firebase_proof_can_be_revoked() {
        let mut email = SignupState::default();
        email.capture(
            VerificationToken::from_fragment(&format!(
                "#register-verify?token={}",
                "af".repeat(32)
            ))
            .unwrap(),
        );
        let mut firebase = SignupState::default();
        firebase.capture(VerificationToken::from_firebase_query(
            "?mode=verifyEmail&oobCode=fixture-action&continueUrl=https%3A%2F%2Fcantos.example.invalid%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001",
            "https://cantos.example.invalid",
        ).unwrap());
        for language in [Locale::Vietnamese, Locale::English] {
            for initial in [SignupState::default(), email.clone()] {
                let html = render(initial, language);
                assert!(!html.contains("id=\"theatre-signup-cancel\""));
                assert!(!html.contains(auth_copy::text(CopyKey::SignupCancelHelp, language)));
            }
            let html = render(firebase.clone(), language);
            let cancel = html
                .split("id=\"theatre-signup-cancel\"")
                .nth(1)
                .unwrap()
                .split("</button>")
                .next()
                .unwrap();
            assert!(cancel.contains("data-variant=\"text\""));
            assert!(html.contains(auth_copy::text(CopyKey::SignupCancelHelp, language)));
        }
        let _ = firebase.begin_verify().unwrap();
        assert!(!render(firebase, Locale::English).contains("id=\"theatre-signup-cancel\""));
    }

    #[test]
    fn email_bindings_outlive_repeated_conditional_form_owners_but_not_the_panel() {
        Owner::new().with(|| {
            let signup = RwSignal::new(SignupState::default());
            let locale = RwSignal::new(Locale::Vietnamese);
            let panel = Owner::new();
            let email = panel.with(|| EmailFieldBindings::new(signup, locale));
            for language in [Locale::Vietnamese, Locale::English, Locale::Vietnamese] {
                locale.set(language);
                signup.update(|current| { current.clear(); });
                email.on_input.run("listener@example.invalid".to_owned());
                let form = panel.with(Owner::new);
                let html = form.with(|| view! {
                    <TextField id="signup-owner-test" kind=TextFieldKind::Email label=email.label
                        value=email.value state=email.state error=email.error on_input=email.on_input/>
                }.to_html());
                assert!(html.contains("listener@example.invalid"));
                form.cleanup();
                assert_eq!(email.value.try_get(), Some("listener@example.invalid".to_owned()));
                let (issued, _) = signup.write().begin_submit().unwrap();
                assert_eq!(email.state.try_get(), Some(FieldState::ReadOnly));
                signup.update(|current| { current.finish_submit(issued, Err(SignupFailure::InvalidEmail)); });
                assert_eq!(email.state.try_get(), Some(FieldState::Editable));
                assert_eq!(email.error.try_get(), Some(Some(auth_copy::text(CopyKey::SignupInvalidEmail, language).to_owned())));
                assert_eq!(email.label.try_get(), Some(auth_copy::text(CopyKey::SignupEmail, language).to_owned()));
            }
            panel.cleanup();
            assert_eq!(email.label.try_get(), None);
            assert_eq!(email.value.try_get(), None);
            assert_eq!(email.state.try_get(), None);
            assert_eq!(email.error.try_get(), None);
        });
    }

    #[test]
    fn failed_history_scrub_stays_visible_across_route_cleanup_until_explicit_release() {
        use crate::{
            app::routes::Route,
            application::auth_navigation::{AuthRoute, ReturnDestination},
        };
        Owner::new().with(|| {
            let blocked = RwSignal::new(true);
            let locale = RwSignal::new(Locale::English);
            let route = RwSignal::new(Route::NotFound);
            let noop = Callback::new(|()| {});
            let mut signup = SignupState::default();
            signup.capture_failed();
            for destination in [Route::NotFound, Route::Discover, Route::Auth(AuthRoute::register(ReturnDestination::Home))] {
                route.set(destination);
                signup.clear(); // Navigation cleanup cannot dismiss the independent safety gate.
                let html = view! {
                    <SignupLinkSafety blocked locale on_retry=noop>
                        <p id="ordinary-route">{move || route.get().hash()}</p>
                    </SignupLinkSafety>
                }.to_html();
                assert!(html.contains("id=\"theatre-signup-history-status\" role=\"alert\""));
                assert!(html.contains("id=\"theatre-signup-clear-link\""));
                assert!(!html.contains("ordinary-route"));
                assert!(!html.contains("<input"));
                assert!(!html.contains("theatre-signup-confirm"));
                assert!(blocked.get_untracked());
            }
            blocked.set(false);
            let html = view! { <SignupLinkSafety blocked locale on_retry=noop><p id="safe-route"/></SignupLinkSafety> }.to_html();
            assert!(html.contains("safe-route"));
            assert!(!html.contains("theatre-signup-history-status"));
        });
    }

    #[test]
    fn email_entry_reuses_shared_semantics_and_never_collects_password_or_identity_choices() {
        for locale in [Locale::Vietnamese, Locale::English] {
            let html = render(SignupState::default(), locale);
            assert!(html.contains("type=\"email\""));
            assert!(html.contains("autocomplete=\"email\""));
            assert!(html.contains("for=\"theatre-signup-email\""));
            assert!(html.contains("data-variant=\"auth-tonal\""));
            assert!(html.contains(auth_copy::text(CopyKey::SignupEmail, locale)));
            for absent in [
                "type=\"password\"",
                "token=",
                "person_id",
                "Studio",
                "<select",
            ] {
                assert!(!html.contains(absent), "{absent}");
            }
        }
    }

    #[test]
    fn captured_link_requires_explicit_confirmation_and_never_renders_the_proof() {
        let mut state = SignupState::default();
        let secret = "af".repeat(32);
        state.capture(
            VerificationToken::from_fragment(&format!("#register-verify?token={secret}")).unwrap(),
        );
        for locale in [Locale::Vietnamese, Locale::English] {
            let html = render(state.clone(), locale);
            assert!(html.contains("id=\"theatre-signup-confirm\""));
            assert!(html.contains(auth_copy::text(CopyKey::SignupConfirmHelp, locale)));
            assert!(!html.contains(&secret));
            assert!(!html.contains("<input"));
        }
    }

    #[test]
    fn firebase_setup_is_a_one_shot_button_without_a_capability_href_or_session_claim() {
        use crate::application::signup::{FirebaseVerification, SetupLink};
        let mut state = SignupState::default();
        state.capture(VerificationToken::from_firebase_query(
            "?mode=verifyEmail&oobCode=fixture-secret&continueUrl=https%3A%2F%2Fcantos.example.invalid%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001",
            "https://cantos.example.invalid",
        ).unwrap());
        let (issued, _) = state.begin_verify().unwrap();
        let setup = SetupLink::new(
            "listener_fixture".to_owned(),
            "https://idm.example.invalid/ui/reset?token=private-fixture".to_owned(),
        )
        .unwrap();
        state.finish_firebase(issued, Ok(FirebaseVerification::SetupReady(setup)));
        for language in [Locale::Vietnamese, Locale::English] {
            let html = render(state.clone(), language);
            assert!(html.contains("id=\"theatre-signup-open-setup\""));
            assert!(html.contains("listener_fixture"));
            assert!(html.contains(auth_copy::text(CopyKey::SignupSetupReady, language)));
            for absent in [
                "private-fixture",
                "fixture-secret",
                "href=",
                "<input",
                "theatre-signup-confirm",
                "theatre-signup-new-request",
            ] {
                assert!(!html.contains(absent), "{absent}");
            }
        }
        let _ = state.take_setup_url();
        let html = render(state.clone(), Locale::English);
        assert!(html.contains("id=\"theatre-signup-login\""));
        assert!(!html.contains("theatre-signup-open-setup"));
        assert!(!html.contains("listener_fixture"));
        state.setup_navigation_failed();
        let html = render(state, Locale::English);
        assert!(html.contains(auth_copy::text(
            CopyKey::SignupFirebaseUnknown,
            Locale::English
        )));
        assert!(!html.contains("theatre-signup-new-request"));
        assert!(!html.contains("theatre-signup-confirm"));
    }

    #[test]
    fn smtp_receipt_and_unknown_verification_never_render_account_success_or_automatic_retry() {
        for result in [
            Ok(cantos_contracts::theatre_registration::VerificationStatus::SetupLinkSent),
            Err(SignupFailure::UnknownOutcome),
        ] {
            let mut state = SignupState::default();
            state.capture(
                VerificationToken::from_fragment(&format!(
                    "#register-verify?token={}",
                    "af".repeat(32)
                ))
                .unwrap(),
            );
            let (issued, _) = state.begin_verify().unwrap();
            state.finish_verify(issued, result);
            let html = render(state, Locale::English);
            assert!(!html.contains("id=\"theatre-signup-confirm\""));
            assert!(!html.contains("Registration complete"));
            assert!(!html.contains("<input"));
            assert!(html.contains(if result.is_ok() {
                "Inbox delivery and completed account setup have not been confirmed"
            } else {
                "not retried automatically"
            }));
        }
    }
}
