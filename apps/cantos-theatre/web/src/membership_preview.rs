//! Opt-in, session-only diagnostic. No API, identity, storage, payment or player adapter.
mod copy;
use self::copy::{text, CopyKey};
use crate::{core::membership::*, messages::Locale};
use leptos::prelude::*;

fn label(locale: RwSignal<Locale>, key: CopyKey) -> impl Fn() -> &'static str + Copy {
    move || text(key, locale.get())
}

fn change(state: RwSignal<MembershipState>, action: impl FnOnce(&mut MembershipState)) {
    state.update(action);
    sync_hash(state.get_untracked().screen());
}

#[cfg(any(target_arch = "wasm32", test))]
fn screen_hash(screen: MembershipScreen) -> &'static str {
    match screen {
        MembershipScreen::Intro => "#membership",
        MembershipScreen::Form => "#membership-request",
        MembershipScreen::Received => "#membership-received",
        MembershipScreen::AccountRegistered => "#registered",
        MembershipScreen::Account => "#account",
        MembershipScreen::Plans(_) => "#membership-plans",
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn apply_hash(state: &mut MembershipState, hash: &str) {
    if screen_hash(state.screen()) == hash {
        return;
    }
    match hash {
        "#membership-request" => {
            state.open_intro();
            state.open_form();
        }
        "#membership-received" => {
            if !state.show_received() {
                state.open_intro();
            }
        }
        "#registered" => {
            if !state.show_account_registered_fixture() {
                state.open_intro();
            }
        }
        "#account" => state.open_account(),
        "#membership-plans" => {
            if !state.open_plans() {
                state.open_plans_deep_link();
            }
        }
        _ => state.open_intro(),
    }
}

#[cfg(target_arch = "wasm32")]
fn sync_hash(screen: MembershipScreen) {
    if let Some(window) = leptos::web_sys::window() {
        if window.location().hash().ok().as_deref() != Some(screen_hash(screen)) {
            let _ = window.location().set_hash(screen_hash(screen));
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn sync_hash(_: MembershipScreen) {}

#[cfg(target_arch = "wasm32")]
fn bind_history(state: RwSignal<MembershipState>) {
    use leptos::leptos_dom::helpers::window_event_listener_untyped;
    let read = move || {
        let hash = leptos::web_sys::window()
            .and_then(|w| w.location().hash().ok())
            .unwrap_or_default();
        state.update(|s| apply_hash(s, &hash));
        sync_hash(state.get_untracked().screen());
    };
    let initial_hash = leptos::web_sys::window()
        .and_then(|w| w.location().hash().ok())
        .unwrap_or_default();
    if initial_hash == "#membership-plans" {
        state.update(|s| s.open_plans_deep_link());
    } else {
        read();
    }
    let listener = window_event_listener_untyped("hashchange", move |_| read());
    on_cleanup(move || listener.remove());
}
#[cfg(not(target_arch = "wasm32"))]
fn bind_history(_: RwSignal<MembershipState>) {}

fn focus_id(id: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsCast;
        if let Some(element) = leptos::web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<leptos::web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = id;
}

#[component]
pub fn MembershipPreview() -> impl IntoView {
    let state = RwSignal::new(MembershipState::default());
    let locale = RwSignal::new(Locale::Vietnamese);
    let dark = RwSignal::new(false);
    let attempt = RwSignal::new(None::<SubmissionAttempt>);
    bind_history(state);
    let screen = Memo::new(move |_| state.get().screen());
    Effect::new(move |_| {
        screen.get();
        focus_id("membership-main");
        crate::platform::set_title("Membership · Cantos");
    });
    view! {
        <div class="cantos-ui membership-preview" data-density="comfortable" data-theme=move || if dark.get() { "dark" } else { "light" } lang=move || locale.get().lang()>
            <header class="membership-toolbar">
                <a href="?mode=mock#account">{label(locale, CopyKey::Exit)}</a>
                <strong>"Cantos"</strong>
                <button type="button" class="theatre-action theatre-action-tonal" aria-pressed=move || dark.get().to_string()
                    on:click=move |_| dark.update(|v| *v = !*v)>{label(locale, CopyKey::Dark)}</button>
                <select id="membership-locale" aria-label=label(locale, CopyKey::Language) on:change=move |ev| {
                    locale.set(if event_target_value(&ev) == "en" { Locale::English } else { Locale::Vietnamese });
                }><option value="vi-VN">"Tiếng Việt"</option><option value="en">"English"</option></select>
            </header>
            <p class="theatre-demo-notice">{label(locale, CopyKey::PreviewNotice)}</p>
            <div class="membership-layout">
                <aside class="membership-story theatre-panel">
                    <img src="assets/artwork/son-ha-ky.webp" alt=""/>
                    <p class="theatre-eyebrow">"Cantos Membership"</p>
                    <h2>{label(locale, CopyKey::StoryTitle)}</h2>
                    <p>{label(locale, CopyKey::StoryBody)}</p>
                </aside>
                <main id="membership-main" tabindex="-1">
                    <button id="membership-back" type="button" class="theatre-back" on:click=move |_| change(state, |s| s.back())>
                        <span aria-hidden="true">"← "</span>{move || text(back_key(state.get().screen()), locale.get())}
                    </button>
                    <p class="theatre-eyebrow">"Cantos Membership"</p>
                    {move || match screen.get() {
                        MembershipScreen::Intro => view! {
                            <h1>{label(locale, CopyKey::IntroTitle)}</h1><p>{label(locale, CopyKey::IntroBody)}</p>
                            <PreviewButton id="membership-open-form" locale key=CopyKey::Register on_press=Callback::new(move |_| change(state, |s| { s.open_form(); }))/>
                            <PreviewButton id="membership-view-plans" locale key=CopyKey::ViewPlans tonal=true on_press=Callback::new(move |_| change(state, |s| { s.open_plans(); }))/>
                        }.into_any(),
                        MembershipScreen::Form => view! { <InterestForm state locale attempt/> }.into_any(),
                        MembershipScreen::Received => view! {
                            <h1>{label(locale, CopyKey::ReceivedTitle)}</h1><p role="status">{label(locale, CopyKey::ReceivedBody)}</p>
                            <div class="theatre-panel"><strong>{label(locale, CopyKey::InterestOnly)}</strong>
                                <p>{move || state.get().receipt().map(|r| r.payload().email().to_owned())}</p></div>
                            <NextActions state locale/>
                        }.into_any(),
                        MembershipScreen::AccountRegistered => view! {
                            <h1>{label(locale, CopyKey::AccountCreated)}</h1><p>{label(locale, CopyKey::AccountFixture)}</p>
                            <NextActions state locale/>
                        }.into_any(),
                        MembershipScreen::Account => view! {
                            <h1>{label(locale, CopyKey::Account)}</h1><p>{label(locale, CopyKey::Unavailable)}</p>
                            <PreviewButton id="membership-account-plans" locale key=CopyKey::ViewPlans on_press=Callback::new(move |_| change(state, |s| { s.open_plans(); }))/>
                            <PreviewButton id="membership-intro" locale key=CopyKey::About tonal=true on_press=Callback::new(move |_| change(state, |s| s.open_intro()))/>
                        }.into_any(),
                        MembershipScreen::Plans(_) => view! { <Plans state locale/> }.into_any(),
                    }}
                </main>
            </div>
            <details class="membership-tools theatre-panel"><summary>{label(locale, CopyKey::Tools)}</summary>
                <p>{label(locale, CopyKey::ToolsHelp)}</p>
                <div class="theatre-actions">
                    <PreviewButton id="membership-fixture-enable" locale key=CopyKey::EnableFixture tonal=true on_press=Callback::new(move |_| {
                        attempt.set(None); state.set(MembershipState::fixture()); sync_hash(MembershipScreen::Intro);
                    })/>
                    <Show when=move || state.get().mode() == MembershipMode::Fixture>
                        <PreviewButton id="membership-account-fixture" locale key=CopyKey::AccountFixtureEntry tonal=true on_press=Callback::new(move |_| change(state, |s| { s.show_account_registered_fixture(); }))/>
                    </Show>
                    <PreviewButton id="membership-reset" locale key=CopyKey::ClearSession tonal=true on_press=Callback::new(move |_| {
                        attempt.set(None); change(state, |s| s.clear_session());
                    })/>
                </div>
                <p role="status">{move || text(if state.get().mode() == MembershipMode::Fixture { CopyKey::FixtureMode } else { CopyKey::Unavailable }, locale.get())}</p>
            </details>
        </div>
    }
}

#[component]
fn PreviewButton(
    id: &'static str,
    locale: RwSignal<Locale>,
    key: CopyKey,
    on_press: Callback<leptos::ev::MouseEvent>,
    #[prop(optional)] tonal: bool,
) -> impl IntoView {
    view! { <button type="button" id=id class=if tonal { "theatre-action theatre-action-tonal" } else { "theatre-action" }
    on:click=move |ev| on_press.run(ev)><span class="cui-control-label">{label(locale,key)}</span></button> }
}

#[component]
fn NextActions(state: RwSignal<MembershipState>, locale: RwSignal<Locale>) -> impl IntoView {
    view! { <div class="membership-actions">
        <PreviewButton id="membership-view-plans" locale key=CopyKey::ViewPlans on_press=Callback::new(move |_| change(state, |s| { s.open_plans(); }))/>
        <a class="theatre-action theatre-action-tonal" href="?mode=mock#discover">{label(locale,CopyKey::Later)}</a>
    </div> }
}

#[component]
fn InterestForm(
    state: RwSignal<MembershipState>,
    locale: RwSignal<Locale>,
    attempt: RwSignal<Option<SubmissionAttempt>>,
) -> impl IntoView {
    let locked = move || {
        matches!(
            state.get().submission(),
            SubmissionStatus::Pending | SubmissionStatus::Unknown | SubmissionStatus::Received
        )
    };
    let error = move |name: bool| {
        state
            .get()
            .field_errors()
            .iter()
            .copied()
            .find(|e| is_name_error(*e) == name)
            .map(field_key)
    };
    view! {
        <h1>{label(locale,CopyKey::FormTitle)}</h1><p>{label(locale,CopyKey::FormBody)}</p>
        <form novalidate on:submit=move |ev| {
            ev.prevent_default();
            match state.write().begin_submission() {
                Ok(issued) => attempt.set(Some(issued)),
                Err(SubmitRefusal::InvalidFields(errors)) => {
                    focus_id(if errors.first().is_some_and(|e| is_name_error(*e)) { "membership-name" } else { "membership-email" });
                }
                Err(SubmitRefusal::Unavailable | SubmitRefusal::AlreadyPending | SubmitRefusal::AlreadyReceived | SubmitRefusal::NotOnForm | SubmitRefusal::SequenceExhausted) => (),
            }
        }>
            <label for="membership-name">{label(locale,CopyKey::Name)}</label>
            <input id="membership-name" type="text" autocomplete="name" value=move || state.get().draft().name.clone() prop:value=move || state.get().draft().name.clone()
                readonly=locked aria-invalid=move || error(true).is_some().to_string() aria-describedby="membership-name-error"
                on:input=move |ev| { let value=event_target_value(&ev); state.update(|s| { let _ = s.set_name(value); }); }/>
            <p id="membership-name-error" class="membership-error">{move || error(true).map(|key| text(key,locale.get()))}</p>
            <label for="membership-email">{label(locale,CopyKey::Email)}</label>
            <input id="membership-email" type="email" autocomplete="email" value=move || state.get().draft().email.clone() prop:value=move || state.get().draft().email.clone()
                readonly=locked aria-invalid=move || error(false).is_some().to_string() aria-describedby="membership-email-error"
                on:input=move |ev| { let value=event_target_value(&ev); state.update(|s| { let _ = s.set_email(value); }); }/>
            <p id="membership-email-error" class="membership-error">{move || error(false).map(|key| text(key,locale.get()))}</p>
            <button id="membership-submit" class="theatre-action" type="submit" aria-describedby="membership-submit-status"
                disabled=move || state.get().mode() == MembershipMode::Unavailable || state.get().submission() == SubmissionStatus::Pending>
                <span class="cui-control-label">{move || text(if state.get().submission() == SubmissionStatus::Unknown { CopyKey::Reconcile } else { CopyKey::Register }, locale.get())}</span>
            </button>
        </form>
        <p id="membership-submit-status" role="status" aria-live="polite">{move || text(status_key(&state.get()),locale.get())}</p>
        <Show when=move || state.get().mode() == MembershipMode::Fixture && state.get().submission() == SubmissionStatus::Pending>
            <fieldset class="theatre-panel"><legend>{label(locale,CopyKey::FixtureOutcome)}</legend>
                {[("membership-confirm",CopyKey::Confirm,FinishOutcome::Received),("membership-fail",CopyKey::Fail,FinishOutcome::Failed),("membership-timeout",CopyKey::Timeout,FinishOutcome::Unknown)].into_iter().map(move |(id,key,outcome)| view! {
                    <PreviewButton id locale key tonal=true on_press=Callback::new(move |_| {
                        if let Some(issued) = attempt.get_untracked() { change(state, |s| { s.finish(&issued,outcome); }); }
                    })/>
                }).collect_view()}
            </fieldset>
        </Show>
    }
}

fn status_key(state: &MembershipState) -> CopyKey {
    if state.mode() == MembershipMode::Unavailable {
        return CopyKey::Unavailable;
    }
    match state.submission() {
        SubmissionStatus::Idle => CopyKey::FixtureMode,
        SubmissionStatus::Pending => CopyKey::Pending,
        SubmissionStatus::Failed => CopyKey::Failed,
        SubmissionStatus::Unknown => CopyKey::Unknown,
        SubmissionStatus::Received => CopyKey::InterestOnly,
    }
}
fn is_name_error(error: FieldError) -> bool {
    matches!(
        error,
        FieldError::NameRequired | FieldError::NameTooLong | FieldError::NameInvalid
    )
}
fn field_key(error: FieldError) -> CopyKey {
    match error {
        FieldError::NameRequired => CopyKey::NameRequired,
        FieldError::NameTooLong => CopyKey::NameTooLong,
        FieldError::NameInvalid => CopyKey::NameInvalid,
        FieldError::EmailRequired => CopyKey::EmailRequired,
        FieldError::EmailInvalid => CopyKey::EmailInvalid,
        FieldError::EmailTooLong => CopyKey::EmailTooLong,
    }
}
fn back_key(screen: MembershipScreen) -> CopyKey {
    match screen {
        MembershipScreen::Form | MembershipScreen::Plans(PlansOrigin::Intro) => CopyKey::BackIntro,
        MembershipScreen::Plans(PlansOrigin::Received) => CopyKey::BackReceived,
        MembershipScreen::Plans(PlansOrigin::AccountRegistered) => CopyKey::BackRegistered,
        MembershipScreen::Intro
        | MembershipScreen::Received
        | MembershipScreen::AccountRegistered
        | MembershipScreen::Account
        | MembershipScreen::Plans(PlansOrigin::Account) => CopyKey::BackAccount,
    }
}

#[component]
fn Plans(state: RwSignal<MembershipState>, locale: RwSignal<Locale>) -> impl IntoView {
    view! {
        <h1>{label(locale,CopyKey::PlansTitle)}</h1><p>{label(locale,CopyKey::PlansBody)}</p>
        <p role="status">{move || text(match state.get().plans_status() {
            PlansStatus::Unpublished => CopyKey::Unpublished, PlansStatus::Loading => CopyKey::PlansLoading,
            PlansStatus::Empty => CopyKey::PlansEmpty, PlansStatus::Unavailable => CopyKey::PlansUnavailable, PlansStatus::Failed => CopyKey::PlansFailed,
        },locale.get())}</p>
        <Show when=move || state.get().plans_status() == PlansStatus::Unpublished>
            <div class="membership-plans">{[CopyKey::Month,CopyKey::Year].into_iter().map(move |key| view! {
                <article class="theatre-panel"><h2>{label(locale,key)}</h2><strong>{label(locale,CopyKey::Unpublished)}</strong>
                    <p>{label(locale,CopyKey::NoApprovedPlans)}</p></article>
            }).collect_view()}</div>
        </Show>
        <a class="theatre-text-link" href="?mode=mock#discover">{label(locale,CopyKey::Later)}</a>
        <details><summary>{label(locale,CopyKey::PlansFixture)}</summary>
            <select id="membership-plans-state" aria-label=label(locale,CopyKey::PlansFixture) on:change=move |ev| {
                let status=match event_target_value(&ev).as_str() { "loading"=>PlansStatus::Loading,"empty"=>PlansStatus::Empty,"unavailable"=>PlansStatus::Unavailable,"failed"=>PlansStatus::Failed,_=>PlansStatus::Unpublished };
                state.update(|s| s.set_plans_status(status));
            }>
                {[("unpublished",CopyKey::Unpublished),("loading",CopyKey::PlansLoading),("empty",CopyKey::PlansEmpty),("unavailable",CopyKey::PlansUnavailable),("failed",CopyKey::PlansFailed)].into_iter().map(move |(value,key)| view! {<option value=value>{label(locale,key)}</option>}).collect_view()}
            </select>
        </details>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> MembershipState {
        let mut state = MembershipState::fixture();
        state.open_form();
        state.set_name("Nguyễn Thị Ánh Nguyệt".into()).unwrap();
        state.set_email("anh-nguyet@example.test".into()).unwrap();
        state
    }

    #[test]
    fn form_is_two_labeled_fields_one_submit_and_no_plan_or_payment_controls() {
        Owner::new().with(|| {
            let state = RwSignal::new(fixture());
            let locale = RwSignal::new(Locale::Vietnamese);
            let attempt = RwSignal::new(None);
            let html = view! { <InterestForm state locale attempt/> }.to_html();
            assert_eq!(html.matches("<input").count(), 2);
            assert_eq!(html.matches("type=\"submit\"").count(), 1);
            for name in [
                "Tên của bạn",
                "Email",
                "Đăng ký Membership",
                "Nguyễn Thị Ánh Nguyệt",
            ] {
                assert!(html.contains(name), "{name}");
            }
            for forbidden in [
                "type=\"checkbox\"",
                "<select",
                "password",
                "checkout",
                "Chưa công bố",
                "Điều khoản",
                "Quyền riêng tư",
                "stepper",
                "/api/",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        });
    }

    #[test]
    fn unavailable_does_not_offer_fixture_completion_or_account_success() {
        Owner::new().with(|| {
            let state = RwSignal::new(MembershipState::default());
            state.update(|s| {
                s.open_form();
            });
            let locale = RwSignal::new(Locale::English);
            let attempt = RwSignal::new(None);
            let html = view! { <InterestForm state locale attempt/> }.to_html();
            assert!(html.contains("disabled"));
            assert!(html.contains("registration is unavailable"));
            assert!(!html.contains("Fixture: confirm"));
            assert!(!html.contains("Registration complete"));
        });
    }

    #[test]
    fn invalid_fields_keep_contact_and_render_inline_errors_without_remounting_screen() {
        Owner::new().with(|| {
            let state = RwSignal::new(fixture());
            state.update(|s| {
                s.set_email("missing-at".into()).unwrap();
            });
            assert_eq!(
                state.write().begin_submission(),
                Err(SubmitRefusal::InvalidFields(vec![FieldError::EmailInvalid]))
            );
            let locale = RwSignal::new(Locale::Vietnamese);
            let attempt = RwSignal::new(None);
            let html = view! { <InterestForm state locale attempt/> }.to_html();
            assert!(html.contains("missing-at"));
            assert!(html.contains("Nguyễn Thị Ánh Nguyệt"));
            assert!(html.contains("aria-invalid=\"true\""));
            assert!(html.contains("Kiểm tra email"));
            assert_eq!(state.get_untracked().screen(), MembershipScreen::Form);
        });
    }

    #[test]
    fn route_input_cannot_forge_a_receipt_or_follow_an_external_return() {
        let mut state = MembershipState::default();
        apply_hash(&mut state, "#membership-received");
        assert_eq!(state.screen(), MembershipScreen::Intro);
        apply_hash(&mut state, "#registered");
        assert_eq!(state.screen(), MembershipScreen::Intro);
        apply_hash(&mut state, "https://other.invalid/");
        assert_eq!(state.screen(), MembershipScreen::Intro);
        assert_eq!(state.receipt(), None);
        state.open_plans_deep_link();
        state.back();
        assert_eq!(state.screen(), MembershipScreen::Account);
    }

    #[test]
    fn browser_history_plans_return_to_receipt_and_stale_form_history_cannot_resubmit() {
        let mut state = fixture();
        let issued = state.begin_submission().unwrap();
        state.finish(&issued, FinishOutcome::Received);
        state.open_plans();
        apply_hash(&mut state, "#membership-received");
        apply_hash(&mut state, "#membership-plans");
        assert_eq!(
            state.screen(),
            MembershipScreen::Plans(PlansOrigin::Received)
        );
        state.back();
        apply_hash(&mut state, "#membership-request");
        assert_eq!(state.screen(), MembershipScreen::Received);
        assert_eq!(state.receipt(), Some(&issued));
    }

    #[test]
    fn every_failure_has_a_distinct_localized_projection_and_plan_failure_keeps_receipt() {
        assert!(is_name_error(FieldError::NameRequired));
        assert!(!is_name_error(FieldError::EmailInvalid));
        for locale in [Locale::Vietnamese, Locale::English] {
            for error in [
                FieldError::NameRequired,
                FieldError::NameTooLong,
                FieldError::NameInvalid,
                FieldError::EmailRequired,
                FieldError::EmailTooLong,
                FieldError::EmailInvalid,
            ] {
                assert!(!text(field_key(error), locale).is_empty());
            }
        }
        let mut state = fixture();
        let issued = state.begin_submission().unwrap();
        state.finish(&issued, FinishOutcome::Received);
        state.open_plans();
        state.set_plans_status(PlansStatus::Failed);
        assert_eq!(state.receipt(), Some(&issued));
        Owner::new().with(|| {
            let state = RwSignal::new(state);
            let locale = RwSignal::new(Locale::English);
            let html = view! { <Plans state locale/> }.to_html();
            assert!(html.contains("Plan loading failed"));
            assert!(!html.contains("Monthly"));
            assert!(!html.contains("checkout"));
        });
    }
}
