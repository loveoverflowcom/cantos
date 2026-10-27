//! Web-owned mailbox-verified signup composition using the shared field and auth action variants.
//! Opening the mailed link never sends anything: the owner chooses a password and confirms.
use super::{
    auth::{label, login_key_submits, PasswordEye},
    auth_copy::{self, CopyKey},
};
use crate::{
    application::signup::{
        CompletionFailure, PasswordIssue, SignupFailure, SignupPhase, SignupState,
    },
    messages::Locale,
};
use cantos_ui::{
    Button, ButtonState, ButtonVariant, Checkbox, ChoiceState, DisabledReason, FieldState,
    IconName, StatusTone, TextField, TextFieldKind, TextFieldVariant,
};
use leptos::prelude::*;

pub(crate) fn phase_key(phase: SignupPhase) -> CopyKey {
    match phase {
        SignupPhase::Editing => CopyKey::SignupEmailHelp,
        SignupPhase::Submitting => CopyKey::SignupSubmitting,
        SignupPhase::EmailAccepted => CopyKey::SignupEmailAccepted,
        SignupPhase::SubmitFailed(SignupFailure::InvalidEmail) => CopyKey::SignupInvalidEmail,
        SignupPhase::SubmitFailed(SignupFailure::Unavailable) => CopyKey::SignupRequestUnavailable,
        SignupPhase::SubmitFailed(SignupFailure::RateLimited(_)) => {
            CopyKey::SignupRequestRateLimited
        }
        SignupPhase::SubmitFailed(SignupFailure::NotConfigured) => CopyKey::SignupNotConfigured,
        SignupPhase::SubmitFailed(SignupFailure::UnknownOutcome | SignupFailure::Protocol) => {
            CopyKey::SignupRequestUnknown
        }
        SignupPhase::AwaitingConfirmation => CopyKey::SignupConfirmHelp,
        SignupPhase::Completing => CopyKey::SignupCompleting,
        SignupPhase::CompleteRetry(CompletionFailure::RateLimited(_)) => {
            CopyKey::SignupRetryRateLimited
        }
        SignupPhase::CompleteRetry(_) => CopyKey::SignupRetryHelp,
        SignupPhase::CompleteRefused(CompletionFailure::Invalid) => CopyKey::SignupInvalidPassword,
        SignupPhase::CompleteRefused(CompletionFailure::Forbidden) => CopyKey::SignupForbidden,
        SignupPhase::CompleteRefused(CompletionFailure::NotConfigured) => {
            CopyKey::SignupNotConfigured
        }
        SignupPhase::CompleteRefused(_) => CopyKey::SignupLinkRefused,
        SignupPhase::Completed(_) => CopyKey::AccountCreatedNotice,
        SignupPhase::CaptureFailed => CopyKey::SignupCaptureFailed,
    }
}

/// Wall clock for the 429 hold shown by the panel. Host builds have no clock and never hold.
fn clock_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// The wait copy outranks the phase while a 429 holds new starts: editing the email moves the
/// phase back to `Editing`, but the hold (and its reason) stays.
fn status_key(state: &SignupState, now_ms: u64) -> CopyKey {
    if state.submit_wait_seconds(now_ms).is_some() {
        CopyKey::SignupRequestRateLimited
    } else {
        phase_key(state.phase())
    }
}

fn status_tone(state: &SignupState, now_ms: u64) -> StatusTone {
    if state.submit_wait_seconds(now_ms).is_some() {
        return StatusTone::Warning;
    }
    match state.phase() {
        SignupPhase::Editing | SignupPhase::AwaitingConfirmation => StatusTone::Neutral,
        SignupPhase::Submitting
        | SignupPhase::Completing
        | SignupPhase::EmailAccepted
        | SignupPhase::Completed(_) => StatusTone::Info,
        SignupPhase::SubmitFailed(
            SignupFailure::RateLimited(_) | SignupFailure::UnknownOutcome | SignupFailure::Protocol,
        )
        | SignupPhase::CompleteRetry(_) => StatusTone::Warning,
        SignupPhase::SubmitFailed(
            SignupFailure::InvalidEmail | SignupFailure::Unavailable | SignupFailure::NotConfigured,
        )
        | SignupPhase::CompleteRefused(_)
        | SignupPhase::CaptureFailed => StatusTone::Error,
    }
}

/// Completion is history, never a session: the next step is signing in (normally the shell
/// has already moved there; this is the fallback if that navigation did not happen).
fn show_login(state: &SignupState) -> bool {
    matches!(state.phase(), SignupPhase::Completed(_))
}

fn password_visible(phase: SignupPhase) -> bool {
    matches!(
        phase,
        SignupPhase::AwaitingConfirmation | SignupPhase::Completing
    )
}

fn focus_target(state: &SignupState, login_via_back: bool) -> &'static str {
    match state.phase() {
        SignupPhase::Editing | SignupPhase::SubmitFailed(_) => "theatre-signup-email",
        SignupPhase::Submitting | SignupPhase::Completing => "theatre-signup-status",
        SignupPhase::AwaitingConfirmation => "theatre-signup-password",
        SignupPhase::CompleteRetry(_) => "theatre-signup-retry",
        SignupPhase::CompleteRefused(CompletionFailure::Invalid) => {
            "theatre-signup-choose-password"
        }
        _ if show_login(state) && !login_via_back => "theatre-signup-login",
        _ if show_login(state) => "theatre-signup-status",
        SignupPhase::EmailAccepted
        | SignupPhase::CompleteRefused(_)
        | SignupPhase::Completed(_)
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

/// The chosen password and its confirmation. Values mirror RAM-only state; they are never
/// persisted, logged or placed in an error notice, and are cleared before the request leaves.
#[derive(Clone, Copy)]
struct PasswordBindings {
    password_label: Signal<String>,
    repeat_label: Signal<String>,
    password: Signal<String>,
    repeat: Signal<String>,
    field_state: Signal<FieldState>,
    password_help: Signal<Option<String>>,
    password_error: Signal<Option<String>>,
    repeat_error: Signal<Option<String>>,
    confirm_label: Signal<String>,
    confirmed: Signal<bool>,
    confirm_state: Signal<ChoiceState>,
}

impl PasswordBindings {
    fn new(signup: RwSignal<SignupState>, locale: RwSignal<Locale>) -> Self {
        let text = move |key| auth_copy::text(key, locale.get()).to_owned();
        let issue = move || {
            let current = signup.get();
            let typed = !current.password().is_empty();
            let repeated = !current.repeat().is_empty();
            (typed || repeated)
                .then(|| current.password_issue())
                .flatten()
        };
        Self {
            password_label: Signal::derive(move || text(CopyKey::SignupPassword)),
            repeat_label: Signal::derive(move || text(CopyKey::SignupPasswordRepeat)),
            password: Signal::derive(move || signup.get().password().to_owned()),
            repeat: Signal::derive(move || signup.get().repeat().to_owned()),
            field_state: Signal::derive(move || {
                if signup.get().phase() == SignupPhase::Completing {
                    FieldState::ReadOnly
                } else {
                    FieldState::Editable
                }
            }),
            password_help: Signal::derive(move || Some(text(CopyKey::SignupPasswordHelp))),
            password_error: Signal::derive(move || match issue() {
                Some(PasswordIssue::TooShort) if !signup.get().password().is_empty() => {
                    Some(text(CopyKey::SignupPasswordShort))
                }
                Some(PasswordIssue::TooLong) => Some(text(CopyKey::SignupPasswordLong)),
                _ => None,
            }),
            repeat_error: Signal::derive(move || {
                (issue() == Some(PasswordIssue::Mismatch) && !signup.get().repeat().is_empty())
                    .then(|| text(CopyKey::SignupPasswordMismatch))
            }),
            confirm_label: Signal::derive(move || text(CopyKey::SignupConfirmCheckbox)),
            confirmed: Signal::derive(move || signup.get().confirmed()),
            confirm_state: Signal::derive(move || {
                if signup.get().phase() == SignupPhase::Completing {
                    DisabledReason::new(auth_copy::text(CopyKey::SignupCompleting, locale.get()))
                        .map_or(ChoiceState::Enabled, ChoiceState::Loading)
                } else {
                    ChoiceState::Enabled
                }
            }),
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
                <p id="theatre-signup-history-status" class="theatre-auth-feedback" data-tone="error" role="alert">{label(locale, CopyKey::SignupHistoryBlocked)}</p>
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
    on_confirm: Callback<()>,
    on_retry: Callback<()>,
    on_new_request: Callback<()>,
    on_login: Callback<()>,
    /// Register's contextual Back already returns to sign-in, including local cleanup.
    #[prop(optional)]
    login_via_back: bool,
) -> impl IntoView {
    let email = EmailFieldBindings::new(state, locale);
    let secret = PasswordBindings::new(state, locale);
    let password_revealed = RwSignal::new(false);
    let repeat_revealed = RwSignal::new(false);
    let panel = NodeRef::<leptos::html::Div>::new();
    let previous_phase = StoredValue::new(state.get_untracked().phase());
    Effect::new(move |_| {
        let current = state.get();
        if previous_phase.get_value() != current.phase() {
            previous_phase.set_value(current.phase());
            if !password_visible(current.phase()) {
                password_revealed.set(false);
                repeat_revealed.set(false);
            }
            crate::platform::focus_in_signup(panel, focus_target(&current, login_via_back));
        }
    });
    let submit_press = Callback::new(move |_: leptos::ev::MouseEvent| on_submit.run(()));
    let confirm_press = Callback::new(move |_: leptos::ev::MouseEvent| on_confirm.run(()));
    let retry_press = Callback::new(move |_: leptos::ev::MouseEvent| on_retry.run(()));
    let new_request_press = Callback::new(move |_: leptos::ev::MouseEvent| on_new_request.run(()));
    let login_press = Callback::new(move |_: leptos::ev::MouseEvent| on_login.run(()));
    let choose_press = Callback::new(move |_: leptos::ev::MouseEvent| {
        state.update(|current| {
            let _ = current.edit_after_refusal();
        })
    });
    let email_form_visible = move || {
        matches!(
            state.get().phase(),
            SignupPhase::Editing | SignupPhase::Submitting | SignupPhase::SubmitFailed(_)
        )
    };
    let request_pending = move || {
        matches!(
            state.get().phase(),
            SignupPhase::Submitting | SignupPhase::Completing
        )
    };
    let confirm_state = move || {
        let current = state.get();
        if current.phase() == SignupPhase::Completing {
            DisabledReason::new(auth_copy::text(CopyKey::SignupCompleting, locale.get()))
                .ok()
                .map(ButtonState::Loading)
        } else if current.can_confirm() {
            Some(ButtonState::Enabled)
        } else {
            DisabledReason::new(auth_copy::text(CopyKey::SignupConfirmHelp, locale.get()))
                .ok()
                .map(ButtonState::Disabled)
        }
    };
    view! {
        <div class="theatre-signup" node_ref=panel>
            <Show when=move || {
                let current = state.get();
                current.submit_wait_seconds(clock_ms()).is_some()
                    || !matches!(current.phase(), SignupPhase::Editing | SignupPhase::AwaitingConfirmation | SignupPhase::SubmitFailed(SignupFailure::InvalidEmail))
            }>
                <p id="theatre-signup-status" class="theatre-auth-feedback"
                    data-tone=move || status_tone(&state.get(), clock_ms()).as_str()
                    tabindex="-1" role="status" aria-live="polite" aria-atomic="true">
                    {move || auth_copy::text(status_key(&state.get(), clock_ms()), locale.get())}
                </p>
            </Show>
            <Show when=email_form_visible>
                <form class="theatre-auth-actions" on:submit=move |event| { event.prevent_default(); on_submit.run(()); }>
                    <TextField id="theatre-signup-email" kind=TextFieldKind::Email variant=TextFieldVariant::Floating
                        label=email.label
                        value=email.value
                        autocomplete="email" name="email" required=true
                        state=email.state
                        error=email.error
                        on_input=email.on_input/>
                    {move || {
                        let availability = if request_pending() {
                            DisabledReason::new(auth_copy::text(CopyKey::SignupSubmitting, locale.get())).ok().map(ButtonState::Loading)
                        } else if state.get().submit_wait_seconds(clock_ms()).is_some() {
                            // Not `disabled`: the control stays focusable and says why.
                            DisabledReason::new(auth_copy::text(CopyKey::SignupRequestRateLimited, locale.get())).ok().map(ButtonState::Disabled)
                        } else { Some(ButtonState::Enabled) };
                        availability.map(|availability| view! {
                            <Button id="theatre-signup-submit" variant=ButtonVariant::AuthPrimary state=availability
                                icon=IconName::Mail
                                on_press=submit_press>{label(locale, CopyKey::SignupSendEmail)}</Button>
                        })
                    }}
                </form>
            </Show>
            <Show when=move || password_visible(state.get().phase())>
                <form class="theatre-auth-actions" on:submit=move |event| { event.prevent_default(); on_confirm.run(()); }
                    on:keydown=move |event| {
                        use leptos::wasm_bindgen::JsCast;
                        let input_target = event.target().is_some_and(|target| {
                            target.dyn_ref::<leptos::web_sys::HtmlInputElement>()
                                .is_some_and(|input| matches!(input.id().as_str(), "theatre-signup-password" | "theatre-signup-repeat"))
                        });
                        // Eye buttons retain native keyboard activation; Enter from an input
                        // confirms only outside IME composition.
                        if login_key_submits(&event.key(), event.is_composing(), input_target) {
                            event.prevent_default();
                            on_confirm.run(());
                        }
                    }>
                    <TextField id="theatre-signup-password" kind=Signal::derive(move || if password_revealed.get() { TextFieldKind::Text } else { TextFieldKind::Password }) variant=TextFieldVariant::Floating
                        label=secret.password_label
                        value=secret.password
                        autocomplete="new-password" required=true
                        state=secret.field_state
                        help=secret.password_help
                        error=secret.password_error
                        on_input=Callback::new(move |value| state.update(|current| current.set_password(value)))
                        trailing=move || view! {
                            <PasswordEye id="theatre-signup-password-eye" locale revealed=password_revealed/>
                        }/>
                    <TextField id="theatre-signup-repeat" kind=Signal::derive(move || if repeat_revealed.get() { TextFieldKind::Text } else { TextFieldKind::Password }) variant=TextFieldVariant::Floating
                        label=secret.repeat_label
                        value=secret.repeat
                        autocomplete="new-password" required=true
                        state=secret.field_state
                        error=secret.repeat_error
                        on_input=Callback::new(move |value| state.update(|current| current.set_repeat(value)))
                        trailing=move || view! {
                            <PasswordEye id="theatre-signup-repeat-eye" locale revealed=repeat_revealed/>
                        }/>
                    <Checkbox id="theatre-signup-confirm-box"
                        label=secret.confirm_label
                        checked=secret.confirmed
                        state=secret.confirm_state
                        on_change=Callback::new(move |value| state.update(|current| current.set_confirmed(value)))/>
                    {move || confirm_state().map(|availability| view! {
                        <Button id="theatre-signup-confirm" variant=ButtonVariant::AuthPrimary state=availability
                            icon=IconName::PersonAdd
                            on_press=confirm_press>{label(locale, CopyKey::SignupCreate)}</Button>
                    })}
                </form>
            </Show>
            <div class="theatre-auth-actions">
                <Show when=move || matches!(state.get().phase(), SignupPhase::CompleteRetry(_))>
                    <Button id="theatre-signup-retry" variant=ButtonVariant::AuthPrimary
                        icon=IconName::Refresh
                        on_press=retry_press>{label(locale, CopyKey::SignupRetrySame)}</Button>
                </Show>
                <Show when=move || state.get().phase() == SignupPhase::CompleteRefused(CompletionFailure::Invalid)>
                    <Button id="theatre-signup-choose-password" variant=ButtonVariant::AuthPrimary
                        icon=IconName::Refresh
                        on_press=choose_press>{label(locale, CopyKey::SignupChoosePassword)}</Button>
                </Show>
                <Show when=move || {
                    let phase = state.get().phase();
                    !email_form_visible() && !password_visible(phase)
                        && !matches!(phase, SignupPhase::CompleteRetry(_) | SignupPhase::CompleteRefused(CompletionFailure::Invalid))
                        && !show_login(&state.get())
                }>
                    <Button id="theatre-signup-new-request" variant=ButtonVariant::AuthPrimary
                        icon=IconName::Refresh
                        on_press=new_request_press>{label(locale, CopyKey::SignupNewRequest)}</Button>
                </Show>
                <Show when=move || !login_via_back && show_login(&state.get())>
                    <Button id="theatre-signup-login" variant=ButtonVariant::AuthPrimary
                        icon=IconName::Login
                        on_press=login_press>{label(locale, CopyKey::ReturnLogin)}</Button>
                </Show>
            </div>
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
    use crate::application::signup::ProofLink;
    use crate::contract::identity_auth::SignupStatus;
    let fixture_link = || {
        ProofLink::from_fragment(&format!(
            "#operation={}&proof={}&secret={}",
            "af".repeat(16),
            "bc".repeat(16),
            "de".repeat(32)
        ))
    };
    view! {
        <section class="theatre-auth-preview" aria-label=label(locale, CopyKey::SignupFixtureNotice)>
            <p>{label(locale, CopyKey::SignupFixtureNotice)}</p>
            <div class="theatre-actions">
                <Button id="theatre-signup-fixture-link" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(link) = fixture_link() { current.capture(link); }
                }))>{label(locale, CopyKey::SignupFixtureLink)}</Button>
                <Button id="theatre-signup-fixture-accepted" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_submit(issued, Ok(()), 0); }
                }))>{label(locale, CopyKey::SignupFixtureAccepted)}</Button>
                <Button id="theatre-signup-fixture-completed" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_complete(issued, Ok(SignupStatus::Completed), 0); }
                }))>{label(locale, CopyKey::SignupFixtureCompleted)}</Button>
                <Button id="theatre-signup-fixture-refused" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_complete(issued, Err(CompletionFailure::Refused), 0); }
                }))>{label(locale, CopyKey::SignupFixtureRefused)}</Button>
                <Button id="theatre-signup-fixture-unknown" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() {
                        if current.phase() == SignupPhase::Submitting { current.finish_submit(issued, Err(SignupFailure::UnknownOutcome), 0); }
                        else { current.finish_complete(issued, Err(CompletionFailure::UnknownOutcome), 0); }
                    }
                }))>{label(locale, CopyKey::SignupFixtureUnknown)}</Button>
                <Button id="theatre-signup-fixture-invalid" variant=ButtonVariant::Text on_press=Callback::new(move |_| state.update(|current| {
                    if let Some(issued) = current.pending_request() { current.finish_submit(issued, Err(SignupFailure::InvalidEmail), 0); }
                }))>{label(locale, CopyKey::SignupFixtureInvalid)}</Button>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::signup::ProofLink;
    use crate::contract::identity_auth::SignupStatus;

    const PASSWORD: &str = "correct horse battery";

    #[test]
    fn signup_feedback_tones_preserve_uncertainty_and_inline_validation() {
        let mut submitting = SignupState::default();
        assert_eq!(status_tone(&submitting, 0), StatusTone::Neutral);
        submitting.set_email("listener@example.invalid".to_owned());
        let (issued, _) = submitting.begin_submit(0).unwrap();
        assert_eq!(status_tone(&submitting, 0), StatusTone::Info);
        let mut accepted = submitting.clone();
        accepted.finish_submit(issued, Ok(()), 0);
        assert_eq!(status_tone(&accepted, 0), StatusTone::Info);
        for (failure, expected) in [
            (SignupFailure::InvalidEmail, StatusTone::Error),
            (SignupFailure::Unavailable, StatusTone::Error),
            (SignupFailure::NotConfigured, StatusTone::Error),
            (SignupFailure::RateLimited(5), StatusTone::Warning),
            (SignupFailure::UnknownOutcome, StatusTone::Warning),
            (SignupFailure::Protocol, StatusTone::Warning),
        ] {
            let mut state = submitting.clone();
            assert!(state.finish_submit(issued, Err(failure), 0));
            assert_eq!(status_tone(&state, 0), expected, "{failure:?}");
            for locale in [Locale::Vietnamese, Locale::English] {
                let html = render(state.clone(), locale);
                if failure == SignupFailure::InvalidEmail {
                    assert!(!html.contains("id=\"theatre-signup-status\""));
                    assert!(html.contains("aria-invalid=\"true\""));
                    assert_eq!(
                        html.matches(auth_copy::text(CopyKey::SignupInvalidEmail, locale))
                            .count(),
                        1
                    );
                } else {
                    assert!(html.contains(&format!("data-tone=\"{}\"", expected.as_str())));
                    assert_eq!(html.matches("role=\"status\"").count(), 1);
                }
                if matches!(
                    failure,
                    SignupFailure::UnknownOutcome | SignupFailure::Protocol
                ) {
                    assert!(html.contains(auth_copy::text(CopyKey::SignupRequestUnknown, locale)));
                }
            }
            if matches!(failure, SignupFailure::RateLimited(_)) {
                state.set_email("another@example.invalid".into());
                assert_eq!(state.phase(), SignupPhase::Editing);
                assert_eq!(status_tone(&state, 0), StatusTone::Warning);
            }
        }
        for (failure, expected) in [
            (CompletionFailure::Invalid, StatusTone::Error),
            (CompletionFailure::Refused, StatusTone::Error),
            (CompletionFailure::Forbidden, StatusTone::Error),
            (CompletionFailure::NotConfigured, StatusTone::Error),
            (CompletionFailure::Unavailable, StatusTone::Warning),
            (CompletionFailure::RateLimited(5), StatusTone::Warning),
            (CompletionFailure::UnknownOutcome, StatusTone::Warning),
        ] {
            let (mut state, issued) = completing();
            state.finish_complete(issued, Err(failure), 0);
            assert_eq!(status_tone(&state, 0), expected, "{failure:?}");
            for locale in [Locale::Vietnamese, Locale::English] {
                let html = render(state.clone(), locale);
                assert_eq!(html.matches("role=\"status\"").count(), 1);
                assert!(html.contains(&format!("data-tone=\"{}\"", expected.as_str())));
                if failure == CompletionFailure::UnknownOutcome {
                    assert!(html.contains(auth_copy::text(CopyKey::SignupRetryHelp, locale)));
                    assert_eq!(html.matches("id=\"theatre-signup-retry\"").count(), 1);
                }
            }
        }
    }
    fn secret() -> String {
        "de".repeat(32)
    }

    fn render(initial: SignupState, language: Locale) -> String {
        render_options(initial, language, false)
    }

    fn render_options(initial: SignupState, language: Locale, login_via_back: bool) -> String {
        Owner::new().with(|| {
            let state = RwSignal::new(initial);
            let locale = RwSignal::new(language);
            let noop = Callback::new(|()| {});
            view! { <SignupPanel state locale login_via_back on_submit=noop on_confirm=noop on_retry=noop on_new_request=noop on_login=noop/> }.to_html()
        })
    }

    fn link() -> ProofLink {
        ProofLink::from_fragment(&format!(
            "#operation={}&proof={}&secret={}",
            "af".repeat(16),
            "bc".repeat(16),
            secret()
        ))
        .unwrap()
    }
    fn awaiting() -> SignupState {
        let mut state = SignupState::default();
        state.capture(link());
        state
    }
    fn ready() -> SignupState {
        let mut state = awaiting();
        state.set_password(PASSWORD.into());
        state.set_repeat(PASSWORD.into());
        state.set_confirmed(true);
        state
    }
    fn completing() -> (SignupState, u64) {
        let mut state = ready();
        let (issued, _) = state.begin_complete().unwrap();
        (state, issued)
    }
    fn control(html: &str, id: &str) -> String {
        let after = html.split(&format!("id=\"{id}\"")).nth(1).unwrap();
        after.split('>').next().unwrap().to_owned()
    }

    #[test]
    fn every_signup_phase_has_one_next_action_and_focus_target() {
        let mut submitting = SignupState::default();
        submitting.set_email("listener@example.invalid".to_owned());
        let (issued, _) = submitting.begin_submit(0).unwrap();
        let mut accepted = submitting.clone();
        assert!(accepted.finish_submit(issued, Ok(()), 0));
        let (done, done_issued) = completing();
        let mut retry = done.clone();
        assert!(retry.finish_complete(done_issued, Err(CompletionFailure::UnknownOutcome), 0));
        let mut cases = vec![
            (
                SignupState::default(),
                Some("submit"),
                "theatre-signup-email",
            ),
            (submitting.clone(), Some("submit"), "theatre-signup-status"),
            (accepted, Some("new-request"), "theatre-signup-new-request"),
            (awaiting(), Some("confirm"), "theatre-signup-password"),
            (ready(), Some("confirm"), "theatre-signup-password"),
            (done.clone(), Some("confirm"), "theatre-signup-status"),
            (retry, Some("retry"), "theatre-signup-retry"),
        ];
        for failure in [
            SignupFailure::InvalidEmail,
            SignupFailure::Unavailable,
            SignupFailure::RateLimited(9),
            SignupFailure::NotConfigured,
            SignupFailure::UnknownOutcome,
            SignupFailure::Protocol,
        ] {
            let mut failed = submitting.clone();
            assert!(failed.finish_submit(issued, Err(failure), 0));
            cases.push((failed, Some("submit"), "theatre-signup-email"));
        }
        for failure in [
            CompletionFailure::Unavailable,
            CompletionFailure::RateLimited(5),
            CompletionFailure::UnknownOutcome,
        ] {
            let (mut state, issued) = completing();
            assert!(state.finish_complete(issued, Err(failure), 0));
            cases.push((state, Some("retry"), "theatre-signup-retry"));
        }
        for failure in [
            CompletionFailure::Refused,
            CompletionFailure::Forbidden,
            CompletionFailure::NotConfigured,
        ] {
            let (mut state, issued) = completing();
            assert!(state.finish_complete(issued, Err(failure), 0));
            cases.push((state, Some("new-request"), "theatre-signup-new-request"));
        }
        let (mut invalid, issued) = completing();
        assert!(invalid.finish_complete(issued, Err(CompletionFailure::Invalid), 0));
        cases.push((
            invalid,
            Some("choose-password"),
            "theatre-signup-choose-password",
        ));
        for status in [SignupStatus::Completed, SignupStatus::Replayed] {
            let (mut state, issued) = completing();
            assert!(state.finish_complete(issued, Ok(status), 0));
            cases.push((state, Some("login"), "theatre-signup-login"));
        }
        let mut capture_failed = SignupState::default();
        capture_failed.capture_failed();
        cases.push((
            capture_failed,
            Some("new-request"),
            "theatre-signup-new-request",
        ));
        for (state, primary, focus) in cases {
            for locale in [Locale::Vietnamese, Locale::English] {
                for login_via_back in [false, true] {
                    let html = render_options(state.clone(), locale, login_via_back);
                    let expected = if primary == Some("login") && login_via_back {
                        None
                    } else {
                        primary
                    };
                    assert_eq!(
                        html.matches("data-variant=\"auth-primary\"").count(),
                        usize::from(expected.is_some()),
                        "{:?}",
                        state.phase()
                    );
                    for id in [
                        "submit",
                        "confirm",
                        "retry",
                        "new-request",
                        "choose-password",
                        "login",
                    ] {
                        assert_eq!(
                            html.contains(&format!("id=\"theatre-signup-{id}\"")),
                            expected == Some(id),
                            "{:?}: {id}",
                            state.phase()
                        );
                    }
                    let expected_focus = if primary == Some("login") && login_via_back {
                        "theatre-signup-status"
                    } else {
                        focus
                    };
                    assert_eq!(focus_target(&state, login_via_back), expected_focus);
                    assert!(html.contains(&format!("id=\"{expected_focus}\"")));
                    // No proof ever reaches the markup. A typed password is mirrored only into its
                    // own controlled field, which is DOM state, never URL, storage or text.
                    for hidden in [secret().as_str(), "operation=", "proof="] {
                        assert!(!html.contains(hidden), "{hidden}");
                    }
                }
            }
        }
    }

    #[test]
    fn signup_fields_use_the_same_floating_shell_as_login_with_the_label_inside() {
        for language in [Locale::Vietnamese, Locale::English] {
            let email = render(SignupState::default(), language);
            assert_eq!(
                email.matches("data-variant=\"floating\"").count(),
                1,
                "{email}"
            );
            assert!(!email.contains("data-variant=\"auth-tonal\""), "{email}");
            let field = control(&email, "theatre-signup-email");
            assert!(field.contains("type=\"email\""), "{field}");
            assert!(field.contains("autocomplete=\"email\""), "{field}");
            assert!(field.contains("required"), "{field}");

            let password = render(awaiting(), language);
            assert_eq!(
                password.matches("data-variant=\"floating\"").count(),
                2,
                "{password}"
            );
            assert!(
                !password.contains("data-variant=\"auth-tonal\""),
                "{password}"
            );
        }
    }

    #[test]
    fn the_password_form_uses_new_password_semantics_and_never_prefills_a_secret() {
        for language in [Locale::Vietnamese, Locale::English] {
            let html = render(awaiting(), language);
            for id in ["theatre-signup-password", "theatre-signup-repeat"] {
                let field = control(&html, id);
                assert!(field.contains("type=\"password\""), "{field}");
                assert!(field.contains("autocomplete=\"new-password\""), "{field}");
                assert!(field.contains("value=\"\""), "{field}");
                let eye = control(&html, &format!("{id}-eye"));
                assert!(eye.contains("type=\"button\""), "{eye}");
                assert!(
                    eye.contains(auth_copy::text(CopyKey::LoginShowPassword, language)),
                    "{eye}"
                );
            }
            assert!(html.contains("type=\"checkbox\""));
            assert!(html.contains(auth_copy::text(CopyKey::SignupPasswordHelp, language)));
            assert!(html.contains(auth_copy::text(CopyKey::SignupConfirmCheckbox, language)));
            // Nothing is sent by opening the link: the confirm control is disabled until the
            // owner typed a matching password and affirmed.
            assert!(control(&html, "theatre-signup-confirm").contains("disabled"));
            assert!(!html.contains("theatre-signup-email"));
            let ready = render(ready(), language);
            assert!(!control(&ready, "theatre-signup-confirm").contains("disabled"));
            // Only the controlled field mirrors the typed value; no status text repeats it.
            assert!(!ready.contains("aria-label=\"correct"));
        }
    }

    #[test]
    fn password_enter_only_submits_from_input_outside_composition() {
        assert!(login_key_submits("Enter", false, true));
        assert!(!login_key_submits("Enter", true, true));
        assert!(!login_key_submits("Enter", false, false));
        assert!(!login_key_submits("Space", false, true));
    }

    #[test]
    fn hints_name_short_long_and_mismatched_passwords_without_blocking_the_server_policy() {
        let mut short = awaiting();
        short.set_password("short".into());
        short.set_repeat("short".into());
        let mut mismatch = awaiting();
        mismatch.set_password(PASSWORD.into());
        mismatch.set_repeat("correct horse batter".into());
        let mut long = awaiting();
        long.set_password("a".repeat(129));
        long.set_repeat("a".repeat(129));
        for language in [Locale::Vietnamese, Locale::English] {
            for (state, key) in [
                (short.clone(), CopyKey::SignupPasswordShort),
                (mismatch.clone(), CopyKey::SignupPasswordMismatch),
                (long.clone(), CopyKey::SignupPasswordLong),
            ] {
                let html = render(state, language);
                assert!(html.contains(auth_copy::text(key, language)), "{key:?}");
                assert!(control(&html, "theatre-signup-confirm").contains("disabled"));
            }
            // An untouched form shows no error.
            let html = render(awaiting(), language);
            for key in [
                CopyKey::SignupPasswordShort,
                CopyKey::SignupPasswordMismatch,
                CopyKey::SignupPasswordLong,
            ] {
                assert!(!html.contains(auth_copy::text(key, language)));
            }
        }
    }

    #[test]
    fn completion_in_flight_locks_the_fields_and_a_lost_response_offers_only_the_same_request() {
        let (flight, issued) = completing();
        let html = render(flight.clone(), Locale::English);
        assert!(control(&html, "theatre-signup-password").contains("readonly"));
        assert!(control(&html, "theatre-signup-repeat").contains("readonly"));
        assert!(html.contains("Creating your account"));
        let mut retry = flight;
        assert!(retry.finish_complete(issued, Err(CompletionFailure::Unavailable), 0));
        for language in [Locale::Vietnamese, Locale::English] {
            let html = render(retry.clone(), language);
            assert!(html.contains(auth_copy::text(CopyKey::SignupRetryHelp, language)));
            assert!(html.contains(auth_copy::text(CopyKey::SignupRetrySame, language)));
            // No password field remains to edit: only the retained identical request can go.
            assert!(!html.contains("type=\"password\""));
            assert!(!html.contains("theatre-signup-email"));
        }
    }

    #[test]
    fn a_definite_invalid_password_allows_a_new_explicit_choice_but_a_spent_link_does_not() {
        let (mut state, issued) = completing();
        assert!(state.finish_complete(issued, Err(CompletionFailure::Invalid), 0));
        let html = render(state, Locale::Vietnamese);
        assert!(html.contains(auth_copy::text(
            CopyKey::SignupInvalidPassword,
            Locale::Vietnamese
        )));
        assert!(html.contains("theatre-signup-choose-password"));
        let (mut state, issued) = completing();
        assert!(state.finish_complete(issued, Err(CompletionFailure::Refused), 0));
        let html = render(state, Locale::Vietnamese);
        assert!(html.contains(auth_copy::text(
            CopyKey::SignupLinkRefused,
            Locale::Vietnamese
        )));
        assert!(!html.contains("theatre-signup-choose-password"));
        assert!(!html.contains("type=\"password\""));
    }

    #[test]
    fn email_start_is_generic_and_never_claims_account_state_or_session() {
        let mut state = SignupState::default();
        state.set_email("listener@example.invalid".to_owned());
        let (issued, _) = state.begin_submit(0).unwrap();
        assert!(state.finish_submit(issued, Ok(()), 0));
        for language in [Locale::Vietnamese, Locale::English] {
            let html = render(state.clone(), language);
            assert!(html.contains(auth_copy::text(CopyKey::SignupEmailAccepted, language)));
            assert!(!html.contains("listener@example.invalid"));
            assert!(!html.contains("type=\"password\""));
        }
        for language in [Locale::Vietnamese, Locale::English] {
            for key in [
                CopyKey::SignupEmailAccepted,
                CopyKey::SignupInvalidEmail,
                CopyKey::SignupLinkRefused,
                CopyKey::SignupRetryHelp,
            ] {
                let copy = auth_copy::text(key, language).to_lowercase();
                for claim in [
                    "already registered",
                    "đã đăng ký rồi",
                    "đã có tài khoản với",
                ] {
                    assert!(!copy.contains(claim), "{key:?}: {claim}");
                }
            }
        }
    }

    #[test]
    fn every_phase_resolves_to_nonblank_copy_in_both_languages() {
        let (done, issued) = completing();
        let mut states = vec![
            SignupState::default(),
            awaiting(),
            ready(),
            done.clone(),
            SignupState::default(),
        ];
        for outcome in [
            Err(CompletionFailure::Invalid),
            Err(CompletionFailure::Refused),
            Err(CompletionFailure::Forbidden),
            Err(CompletionFailure::NotConfigured),
            Err(CompletionFailure::RateLimited(3)),
            Err(CompletionFailure::Unavailable),
            Err(CompletionFailure::UnknownOutcome),
            Ok(SignupStatus::Completed),
        ] {
            let mut state = done.clone();
            assert!(state.finish_complete(issued, outcome, 0));
            states.push(state);
        }
        for state in states {
            for language in [Locale::Vietnamese, Locale::English] {
                assert!(!auth_copy::text(phase_key(state.phase()), language)
                    .trim()
                    .is_empty());
            }
        }
        for failure in [
            SignupFailure::InvalidEmail,
            SignupFailure::Unavailable,
            SignupFailure::RateLimited(4),
            SignupFailure::NotConfigured,
            SignupFailure::UnknownOutcome,
            SignupFailure::Protocol,
        ] {
            for language in [Locale::Vietnamese, Locale::English] {
                assert!(
                    !auth_copy::text(phase_key(SignupPhase::SubmitFailed(failure)), language)
                        .trim()
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn link_safety_boundary_replaces_the_children_while_blocked() {
        Owner::new().with(|| {
            for (blocked, shown) in [(true, false), (false, true)] {
                let blocked = RwSignal::new(blocked);
                let locale = RwSignal::new(Locale::English);
                let html = view! {
                    <SignupLinkSafety blocked locale on_retry=Callback::new(|()| {})>
                        <p id="child">"child"</p>
                    </SignupLinkSafety>
                }
                .to_html();
                assert_eq!(html.contains("id=\"child\""), shown);
                assert_eq!(html.contains("theatre-signup-clear-link"), !shown);
            }
        });
    }
}
