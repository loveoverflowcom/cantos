//! Email/password sign-in presentation. The form owns no authority: it edits a `SignInForm`
//! and reports an explicit submit; the shell builds the intent and calls the API.
use crate::{
    messages::signin::{failure_text, text, Key},
    signin::SignInForm,
    ui::{BlockReason, FormSubmit},
};
use cantos_ui::{
    AccessibleName, Announcement, ButtonState, ButtonVariant, DisabledReason, FieldState, Icon,
    IconButton, IconName, Status, TextField, TextFieldKind, TextFieldVariant,
};
use leptos::{ev::SubmitEvent, prelude::*};

fn password_eye_resources(
    locale: crate::localization::Locale,
) -> Option<(AccessibleName, AccessibleName, DisabledReason)> {
    Some((
        AccessibleName::new(text(Key::ShowPassword, locale)).ok()?,
        AccessibleName::new(text(Key::HidePassword, locale)).ok()?,
        DisabledReason::new(BlockReason::ContextReady.text(locale)).ok()?,
    ))
}

/// `id` namespaces the controls so the entry form and the in-workspace reauthentication form
/// can coexist. `blocked` is the shell's own barrier (another auth/request in flight).
#[component]
pub fn SignInPanel(
    #[prop(into)] id: String,
    form: RwSignal<SignInForm>,
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    on_submit: Callback<()>,
    /// Reauthentication copy explains that only the retained owner is accepted.
    #[prop(optional)]
    reauthenticate: bool,
) -> impl IntoView {
    if reauthenticate {
        // The shell owns session authority; disposing this form only wipes its inputs and
        // fences its presentation result. Reopening starts with a fresh credential form.
        on_cleanup(move || {
            let _ = form.try_update(SignInForm::clear);
        });
    }
    let email_id = format!("{id}-email");
    let password_id = format!("{id}-password");
    let password_eye_id = format!("{id}-password-eye");
    let submit_id = format!("{id}-submit");
    let status_id = format!("{id}-status");
    let submitting = Signal::derive(move || form.with(SignInForm::submitting));
    // Presentation only: the shared field retypes the existing input and preserves selection.
    // Clearing the secret (submission, logout or closing reauthentication) also resets the eye.
    let revealed = RwSignal::new(false);
    Effect::new(move |_| {
        if form.with(|form| form.password().is_empty()) {
            revealed.set(false);
        }
    });
    // Allocate the eye's reactive handles under the form owner, before the trailing view runs.
    // A locale update or reauthentication unmount must not leave DOM effects with disposed handles.
    let password_eye = crate::localization::generated::LocaleMap::try_new(|locale| {
        password_eye_resources(locale).ok_or(())
    })
    .ok()
    .map(|resources| {
        let resources = StoredValue::new(resources);
        let name = Signal::derive(move || {
            resources.with_value(|resources| {
                let (show, hide, _) = resources.get(locale.get());
                if revealed.get() {
                    hide.clone()
                } else {
                    show.clone()
                }
            })
        });
        let icon = Signal::derive(move || {
            if revealed.get() {
                IconName::VisibilityOff
            } else {
                IconName::Visibility
            }
        });
        let state = Signal::derive(move || {
            if blocked.get() || submitting.get() {
                resources.with_value(|resources| {
                    ButtonState::Disabled(resources.get(locale.get()).2.clone())
                })
            } else {
                ButtonState::Enabled
            }
        });
        ViewFn::from(move || {
            view! {
                <IconButton id=password_eye_id.clone() name=name icon=icon state=state
                    on_press=Callback::new(move |_| revealed.update(|shown| *shown = !*shown))/>
            }
        })
    });
    let field_state = Signal::derive(move || {
        if submitting.get() {
            DisabledReason::new(text(Key::Submitting, locale.get()))
                .map(FieldState::Loading)
                .unwrap_or(FieldState::ReadOnly)
        } else {
            FieldState::Editable
        }
    });
    let failure = Signal::derive(move || {
        form.with(|form| {
            form.failure()
                .map(|failure| failure_text(failure, locale.get()))
        })
    });
    view! {
        <form id=format!("{id}-form") class="signin-form" novalidate=true
            on:submit=move |event: SubmitEvent| {
                event.prevent_default();
                if !blocked.get_untracked() && !submitting.get_untracked() {
                    on_submit.run(());
                }
            }>
            <Show when=move || reauthenticate>
                <p class="help">{move || text(Key::ReauthenticateHelp, locale.get())}</p>
            </Show>
            <TextField id=email_id label=Signal::derive(move || text(Key::Email, locale.get()).to_owned())
                value=Signal::derive(move || form.with(|form| form.email().to_owned()))
                on_input=Callback::new(move |value: String| form.update(|form| form.set_email(value)))
                kind=TextFieldKind::Email variant=TextFieldVariant::Floating state=field_state required=true
                autocomplete="username"/>
            <TextField id=password_id label=Signal::derive(move || text(Key::Password, locale.get()).to_owned())
                value=Signal::derive(move || form.with(|form| form.password().to_owned()))
                on_input=Callback::new(move |value: String| form.update(|form| form.set_password(value)))
                kind=Signal::derive(move || if revealed.get() { TextFieldKind::Text } else { TextFieldKind::Password })
                variant=TextFieldVariant::Floating state=field_state required=true
                autocomplete="current-password"
                trailing=move || password_eye.as_ref().map(ViewFn::run)/>
            <FormSubmit id=submit_id locale=locale variant=ButtonVariant::AuthPrimary
                icon=ViewFn::from(|| view! { <Icon name=IconName::Login/> })
                busy=submitting
                blocked=Signal::derive(move || blocked.get() || submitting.get())>
                {move || text(if submitting.get() { Key::Submitting } else { Key::Submit }, locale.get())}
            </FormSubmit>
            <div id=status_id hidden=move || failure.get().is_none()>
                <Status announcement=Announcement::Polite>{move || failure.get().unwrap_or_default()}</Status>
            </div>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signin::SignInFailure;
    use cantos_web_auth::Failure;

    fn render(
        form: SignInForm,
        locale: crate::localization::Locale,
        blocked: bool,
        reauth: bool,
    ) -> String {
        let owner = Owner::new();
        owner.with(|| {
            let form = RwSignal::new(form);
            view! { <SignInPanel id="session" form=form locale=Signal::stored(locale)
                blocked=Signal::stored(blocked) on_submit=Callback::new(|()| {}) reauthenticate=reauth/> }
            .to_html()
        })
    }

    #[test]
    fn the_form_is_a_native_email_password_form_with_credential_manager_hints() {
        for locale in crate::localization::Locale::supported() {
            let html = render(SignInForm::default(), locale, false, false);
            assert!(html.contains("<form"), "{html}");
            assert!(html.contains("type=\"email\""), "{html}");
            assert!(html.contains("type=\"password\""), "{html}");
            assert!(html.contains("autocomplete=\"username\""), "{html}");
            assert!(html.contains("autocomplete=\"current-password\""), "{html}");
            assert!(html.contains("type=\"submit\""), "{html}");
            assert_eq!(html.matches("class=\"cui-button-icon\"").count(), 1);
            assert!(html.contains("aria-hidden=\"true\""), "{html}");
            assert!(html.contains(text(Key::Password, locale)), "{html}");
        }
    }

    #[test]
    fn sign_in_and_reauthentication_submit_through_the_auth_primary_action() {
        for reauth in [false, true] {
            let html = render(
                SignInForm::default(),
                crate::localization::Locale::Vietnamese,
                false,
                reauth,
            );
            let at = html.find("id=\"session-submit\"").unwrap();
            let start = html[..at].rfind("<button").unwrap();
            let submit = &html[start..at + html[at..].find('>').unwrap()];
            assert!(submit.contains("data-variant=\"auth-primary\""), "{submit}");
            assert!(submit.contains("type=\"submit\""), "{submit}");
        }
    }

    #[test]
    fn email_and_password_use_the_shared_floating_fields_v3_shell() {
        let html = render(
            SignInForm::default(),
            crate::localization::Locale::Vietnamese,
            false,
            false,
        );
        assert_eq!(
            html.matches("data-variant=\"floating\"").count(),
            2,
            "{html}"
        );
        assert_eq!(html.matches("cui-field-shell").count(), 2, "{html}");
    }

    #[test]
    fn each_form_has_a_named_non_submitting_password_eye_inside_the_field() {
        for (locale, label) in [
            (crate::localization::Locale::Vietnamese, "Hiện mật khẩu"),
            (crate::localization::Locale::English, "Show password"),
        ] {
            for reauth in [false, true] {
                let html = render(SignInForm::default(), locale, false, reauth);
                let eye = html.split("id=\"session-password-eye\"").nth(1).unwrap();
                let eye = eye.split("</button>").next().unwrap();
                assert!(eye.contains("type=\"button\""), "{eye}");
                assert!(eye.contains(label), "{eye}");
                assert!(!eye.contains("disabled"), "{eye}");
                assert!(html.contains("cui-field-trailing"), "{html}");
            }
        }
    }

    #[test]
    fn password_visibility_is_unavailable_while_the_form_is_pending_or_blocked() {
        for pending in [false, true] {
            let mut form = SignInForm::default();
            if pending {
                form.set_email("owner@example.invalid".into());
                form.set_password("password".into());
                let _ = form.begin(None).unwrap();
            }
            let html = render(
                form,
                crate::localization::Locale::Vietnamese,
                !pending,
                false,
            );
            let eye = html.split("id=\"session-password-eye\"").nth(1).unwrap();
            let eye = eye.split("</button>").next().unwrap();
            assert!(eye.contains("disabled"), "{eye}");
            assert!(
                eye.contains("aria-describedby=\"session-password-eye-reason\""),
                "{eye}"
            );
        }
    }

    #[test]
    fn entry_and_reauthentication_eyes_are_namespaced_and_initially_masked() {
        let owner = Owner::new();
        owner.with(|| {
            let form = RwSignal::new(SignInForm::default());
            let html = view! {
                <SignInPanel id="session" form=form locale=Signal::stored(crate::localization::Locale::Vietnamese)
                    blocked=Signal::stored(false) on_submit=Callback::new(|()| {})/>
                <SignInPanel id="account" form=form locale=Signal::stored(crate::localization::Locale::Vietnamese)
                    blocked=Signal::stored(false) on_submit=Callback::new(|()| {}) reauthenticate=true/>
            }.to_html();
            for id in ["session-password-eye", "account-password-eye"] {
                assert_eq!(html.matches(&format!("id=\"{id}\"")).count(), 1, "{html}");
            }
            assert_eq!(html.matches("type=\"password\"").count(), 2, "{html}");
        });
    }

    #[test]
    fn closing_reauthentication_discards_input_and_fences_its_late_result() {
        Owner::new().with(|| {
            let form = RwSignal::new(SignInForm::default());
            for pending in [false, true] {
                form.update(|form| {
                    form.set_email("owner@example.invalid".into());
                    form.set_password("temporary secret".into());
                });
                let issued = pending.then(|| form.try_update(|form| form.begin(None)).flatten().unwrap().0);
                let panel_owner = Owner::new();
                panel_owner.with(|| {
                    let _ = view! {
                        <SignInPanel id="account" form=form locale=Signal::stored(crate::localization::Locale::Vietnamese)
                            blocked=Signal::stored(false) on_submit=Callback::new(|()| {}) reauthenticate=true/>
                    }.to_html();
                });
                panel_owner.cleanup();
                form.with(|form| {
                    assert_eq!(form.email(), "");
                    assert_eq!(form.password(), "");
                    assert_eq!(form.phase(), crate::signin::SignInPhase::Editing);
                });
                if let Some(issued) = issued {
                    assert_eq!(form.try_update(|form| form.finish(issued, Err(Failure::InvalidCredentials))), Some(false));
                }
            }
        });
    }

    #[test]
    fn the_status_row_stays_hidden_until_there_is_a_failure_to_announce() {
        let html = render(
            SignInForm::default(),
            crate::localization::Locale::Vietnamese,
            false,
            false,
        );
        let status = html
            .split("id=\"session-status\"")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap();
        assert!(status.contains("hidden"), "{html}");

        let mut form = SignInForm::default();
        form.set_email("a@b.example".into());
        form.set_password("password".into());
        let (issued, _) = form.begin(None).unwrap();
        form.finish(issued, Err(Failure::InvalidCredentials));
        let html = render(form, crate::localization::Locale::Vietnamese, false, false);
        let status = html
            .split("id=\"session-status\"")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap();
        assert!(!status.contains("hidden"), "{html}");
    }

    #[test]
    fn submitting_keeps_native_submit_and_label_but_replaces_login_icon_with_progress() {
        for locale in crate::localization::Locale::supported() {
            let mut form = SignInForm::default();
            form.set_email("owner@example.invalid".into());
            form.set_password("password".into());
            let _ = form.begin(None).unwrap();
            let html = render(form, locale, false, false);
            let submit = html
                .split("id=\"session-submit\"")
                .nth(1)
                .unwrap()
                .split("</button>")
                .next()
                .unwrap();
            assert!(submit.contains("type=\"submit\""), "{html}");
            assert!(submit.contains("aria-busy=\"true\""), "{html}");
            assert!(submit.contains("disabled"), "{html}");
            assert!(submit.contains("cui-loading-indicator"), "{html}");
            assert!(!submit.contains("<svg"), "{html}");
            assert!(submit.contains(text(Key::Submitting, locale)), "{html}");
        }
    }

    #[test]
    fn a_typed_password_is_a_controlled_value_and_an_empty_form_has_no_failure_text() {
        let mut form = SignInForm::default();
        form.set_email("owner@example.invalid".into());
        let html = render(form, crate::localization::Locale::Vietnamese, false, false);
        assert!(html.contains("owner@example.invalid"));
        for failure in [
            SignInFailure::InvalidCredentials,
            SignInFailure::NotPermitted,
        ] {
            assert!(!html.contains(&failure_text(
                failure,
                crate::localization::Locale::Vietnamese
            )));
        }
    }

    #[test]
    fn each_failure_is_announced_and_the_submit_blocks_while_another_barrier_holds() {
        for (failure, key) in [
            (Failure::Forbidden, Key::NotPermitted),
            (Failure::InvalidCredentials, Key::InvalidCredentials),
            (Failure::NotFound, Key::NotAvailable),
            (Failure::Unavailable, Key::Unavailable),
        ] {
            for locale in crate::localization::Locale::supported() {
                let mut form = SignInForm::default();
                form.set_email("a@b.example".into());
                form.set_password("password".into());
                let (issued, _) = form.begin(None).unwrap();
                form.finish(issued, Err(failure));
                let html = render(form, locale, false, false);
                assert!(
                    html.contains(text(key, locale)),
                    "{failure:?} {locale}: {html}"
                );
            }
        }
        let html = render(
            SignInForm::default(),
            crate::localization::Locale::Vietnamese,
            true,
            false,
        );
        assert!(html.contains("disabled"), "{html}");
        assert!(
            html.contains("aria-describedby=\"session-submit-reason\""),
            "{html}"
        );
    }

    #[test]
    fn rate_limits_show_their_seconds_and_reauthentication_explains_the_pinned_owner() {
        let mut form = SignInForm::default();
        form.set_email("a@b.example".into());
        form.set_password("password".into());
        let (issued, _) = form.begin(None).unwrap();
        form.finish(issued, Err(Failure::RateLimited(17)));
        assert!(
            render(form, crate::localization::Locale::English, false, false)
                .contains("Wait 17 seconds")
        );
        let html = render(
            SignInForm::default(),
            crate::localization::Locale::English,
            false,
            true,
        );
        assert!(
            html.contains(text(
                Key::ReauthenticateHelp,
                crate::localization::Locale::English
            )),
            "{html}"
        );
        let entry = render(
            SignInForm::default(),
            crate::localization::Locale::English,
            false,
            false,
        );
        assert!(!entry.contains(text(
            Key::ReauthenticateHelp,
            crate::localization::Locale::English
        )));
    }
}
