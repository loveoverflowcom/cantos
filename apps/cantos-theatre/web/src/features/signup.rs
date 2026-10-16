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
    Button, ButtonState, ButtonVariant, DisabledReason, FieldState, TextField, TextFieldKind,
    TextFieldVariant,
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
) -> impl IntoView {
    let email = EmailFieldBindings::new(state, locale);
    let submit_press = Callback::new(move |_: leptos::ev::MouseEvent| on_submit.run(()));
    let verify_press = Callback::new(move |_: leptos::ev::MouseEvent| on_verify.run(()));
    let new_request_press = Callback::new(move |_: leptos::ev::MouseEvent| on_new_request.run(()));
    let cancel_press = Callback::new(move |_: leptos::ev::MouseEvent| on_cancel.run(()));
    let login_press = Callback::new(move |_: leptos::ev::MouseEvent| on_login.run(()));
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
        <div class="theatre-signup">
            <p id="theatre-signup-status" role="status" aria-live="polite" aria-atomic="true">
                {move || auth_copy::text(phase_key(state.get().phase()), locale.get())}
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
                                on_press=verify_press>{label(locale, CopyKey::SignupConfirm)}</Button>
                        })
                    }}
                </div>
            </Show>
            <div class="theatre-auth-actions">
                <Show when=move || !form_visible() && !matches!(state.get().phase(), SignupPhase::AwaitingConfirmation | SignupPhase::Verifying)>
                    <Button id="theatre-signup-new-request" variant=ButtonVariant::AuthSecondary
                        on_press=new_request_press>{label(locale, CopyKey::SignupNewRequest)}</Button>
                </Show>
                <Show when=move || state.get().phase() == SignupPhase::SetupLinkSent>
                    <Button id="theatre-signup-login" variant=ButtonVariant::AuthPrimary
                        on_press=login_press>{label(locale, CopyKey::ReturnLogin)}</Button>
                </Show>
                <Button id="theatre-signup-cancel" variant=ButtonVariant::AuthSecondary
                    on_press=cancel_press>{label(locale, CopyKey::SignupCancel)}</Button>
            </div>
            <p class="theatre-auth-helper theatre-muted">{label(locale, CopyKey::SignupCancelHelp)}</p>
        </div>
    }
}

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
                <Button id="theatre-signup-fixture-accepted" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_submit(issued, Ok(())); }
                }))>{label(locale, CopyKey::SignupFixtureAccepted)}</Button>
                <Button id="theatre-signup-fixture-setup" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_verify(issued, Ok(VerificationStatus::SetupLinkSent)); }
                }))>{label(locale, CopyKey::SignupFixtureSetup)}</Button>
                <Button id="theatre-signup-fixture-unavailable" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_verify(issued, Ok(VerificationStatus::Unavailable)); }
                }))>{label(locale, CopyKey::SignupFixtureUnavailable)}</Button>
                <Button id="theatre-signup-fixture-unknown" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() {
                        if current.phase() == SignupPhase::Submitting { current.finish_submit(issued, Err(SignupFailure::UnknownOutcome)); }
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
        Owner::new().with(|| {
            let state = RwSignal::new(initial);
            let locale = RwSignal::new(language);
            let noop = Callback::new(|()| {});
            view! { <SignupPanel state locale on_submit=noop on_verify=noop on_new_request=noop on_cancel=noop on_login=noop/> }.to_html()
        })
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
