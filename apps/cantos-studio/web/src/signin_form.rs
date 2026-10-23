//! Email/password sign-in presentation. The form owns no authority: it edits a `SignInForm`
//! and reports an explicit submit; the shell builds the intent and calls the API.
use crate::{
    messages::signin::{failure_text, text, Key},
    signin::SignInForm,
    ui::FormSubmit,
};
use cantos_ui::{
    Announcement, DisabledReason, FieldState, Icon, IconName, Status, TextField, TextFieldKind,
};
use leptos::{ev::SubmitEvent, prelude::*};

/// `id` namespaces the controls so the entry form and the in-workspace reauthentication form
/// can coexist. `blocked` is the shell's own barrier (another auth/request in flight).
#[component]
pub fn SignInPanel(
    #[prop(into)] id: String,
    form: RwSignal<SignInForm>,
    english: Signal<bool>,
    blocked: Signal<bool>,
    on_submit: Callback<()>,
    /// Reauthentication copy explains that only the retained owner is accepted.
    #[prop(optional)]
    reauthenticate: bool,
) -> impl IntoView {
    let email_id = format!("{id}-email");
    let password_id = format!("{id}-password");
    let submit_id = format!("{id}-submit");
    let status_id = format!("{id}-status");
    let submitting = Signal::derive(move || form.with(SignInForm::submitting));
    let field_state = Signal::derive(move || {
        if submitting.get() {
            DisabledReason::new(text(Key::Submitting, english.get()))
                .map(FieldState::Loading)
                .unwrap_or(FieldState::ReadOnly)
        } else {
            FieldState::Editable
        }
    });
    let failure = Signal::derive(move || {
        form.with(|form| {
            form.failure()
                .map(|failure| failure_text(failure, english.get()))
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
            <p class="help">{move || text(if reauthenticate { Key::ReauthenticateHelp } else { Key::Help }, english.get())}</p>
            <TextField id=email_id label=Signal::derive(move || text(Key::Email, english.get()).to_owned())
                value=Signal::derive(move || form.with(|form| form.email().to_owned()))
                on_input=Callback::new(move |value: String| form.update(|form| form.set_email(value)))
                kind=TextFieldKind::Email state=field_state required=true
                autocomplete="username"/>
            <TextField id=password_id label=Signal::derive(move || text(Key::Password, english.get()).to_owned())
                value=Signal::derive(move || form.with(|form| form.password().to_owned()))
                on_input=Callback::new(move |value: String| form.update(|form| form.set_password(value)))
                kind=TextFieldKind::Password state=field_state required=true
                autocomplete="current-password"/>
            <FormSubmit id=submit_id english=english
                icon=ViewFn::from(|| view! { <Icon name=IconName::Login/> })
                busy=submitting
                blocked=Signal::derive(move || blocked.get() || submitting.get())>
                {move || text(if submitting.get() { Key::Submitting } else { Key::Submit }, english.get())}
            </FormSubmit>
            <div id=status_id>
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

    fn render(form: SignInForm, english: bool, blocked: bool, reauth: bool) -> String {
        let owner = Owner::new();
        owner.with(|| {
            let form = RwSignal::new(form);
            view! { <SignInPanel id="session" form=form english=Signal::stored(english)
                blocked=Signal::stored(blocked) on_submit=Callback::new(|()| {}) reauthenticate=reauth/> }
            .to_html()
        })
    }

    #[test]
    fn the_form_is_a_native_email_password_form_with_credential_manager_hints() {
        for english in [false, true] {
            let html = render(SignInForm::default(), english, false, false);
            assert!(html.contains("<form"), "{html}");
            assert!(html.contains("type=\"email\""), "{html}");
            assert!(html.contains("type=\"password\""), "{html}");
            assert!(html.contains("autocomplete=\"username\""), "{html}");
            assert!(html.contains("autocomplete=\"current-password\""), "{html}");
            assert!(html.contains("type=\"submit\""), "{html}");
            assert_eq!(html.matches("class=\"cui-button-icon\"").count(), 1);
            assert!(html.contains("aria-hidden=\"true\""), "{html}");
            assert!(html.contains(text(Key::Password, english)), "{html}");
        }
    }

    #[test]
    fn submitting_keeps_native_submit_and_label_but_replaces_login_icon_with_progress() {
        for english in [false, true] {
            let mut form = SignInForm::default();
            form.set_email("owner@example.invalid".into());
            form.set_password("password".into());
            let _ = form.begin(None).unwrap();
            let html = render(form, english, false, false);
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
            assert!(submit.contains("cui-action-progress"), "{html}");
            assert!(!submit.contains("<svg"), "{html}");
            assert!(submit.contains(text(Key::Submitting, english)), "{html}");
        }
    }

    #[test]
    fn a_typed_password_is_a_controlled_value_and_an_empty_form_has_no_failure_text() {
        let mut form = SignInForm::default();
        form.set_email("owner@example.invalid".into());
        let html = render(form, false, false, false);
        assert!(html.contains("owner@example.invalid"));
        for failure in [
            SignInFailure::InvalidCredentials,
            SignInFailure::NotPermitted,
        ] {
            assert!(!html.contains(&failure_text(failure, false)));
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
            for english in [false, true] {
                let mut form = SignInForm::default();
                form.set_email("a@b.example".into());
                form.set_password("password".into());
                let (issued, _) = form.begin(None).unwrap();
                form.finish(issued, Err(failure));
                let html = render(form, english, false, false);
                assert!(
                    html.contains(text(key, english)),
                    "{failure:?} {english}: {html}"
                );
            }
        }
        let html = render(SignInForm::default(), false, true, false);
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
        assert!(render(form, true, false, false).contains("Wait 17 seconds"));
        let html = render(SignInForm::default(), true, false, true);
        assert!(html.contains(text(Key::ReauthenticateHelp, true)), "{html}");
        assert!(!html.contains(text(Key::Help, true)));
    }
}
