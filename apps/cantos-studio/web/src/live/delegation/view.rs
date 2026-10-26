//! Presentation-only delegation components. They render acknowledged facts and dispatch
//! caller callbacks; requests, clipboard and focus effects belong to the browser panel.
use super::{
    copy::{self, text, Key},
    display_time, field_problem, grant_state, Field, GrantState, IssueDraft, Lifetime, Problem,
    Reveal, ACTIONS,
};
use crate::contract::{DelegationAction, DelegationGrantResponse};
use crate::ui::{ActionButton, FormSubmit};
use cantos_ui::{ButtonVariant, IconName};
use leptos::prelude::*;

pub const REVEAL_HEADING: &str = "delegation-reveal-title";
pub const UNRECOVERABLE_HEADING: &str = "delegation-unrecoverable-title";
pub const TOKEN_FIELD: &str = "delegation-token";
pub const ISSUE_HEADING: &str = "delegation-issue-title";
pub const LIST_HEADING: &str = "delegation-list-title";
pub const STATUS: &str = "delegation-status";
pub const RETRY: &str = "delegation-retry";
pub const DELEGATE_FIELD: &str = "delegation-delegate";
pub const LIFETIME_FIELD: &str = "delegation-lifetime";
pub const FIRST_ACTION: &str = "delegation-action-work-read";

pub fn grant_dom_id(grant: &str) -> String {
    format!("delegation-grant-{grant}")
}

/// Outcome of the explicit copy action; the token itself never enters this value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CopyState {
    #[default]
    Idle,
    Copied,
    /// Clipboard unavailable or refused: the field text was selected for a manual copy.
    Manual,
}

fn action_slug(action: DelegationAction) -> &'static str {
    match action {
        DelegationAction::WorkRead => "work-read",
        DelegationAction::ContentRead => "content-read",
        DelegationAction::ContentWrite => "content-write",
    }
}

fn field_error(
    problems: RwSignal<Vec<Problem>>,
    field: Field,
    locale: Signal<crate::localization::Locale>,
) -> Signal<Option<String>> {
    Signal::derive(move || {
        problems
            .with(|p| field_problem(p, field))
            .map(|p| copy::problem(&p, locale.get()))
    })
}

fn clear_field(problems: RwSignal<Vec<Problem>>, field: Field) {
    if problems.with_untracked(|p| p.iter().any(|problem| problem.field() == Some(field))) {
        problems.update(|p| p.retain(|problem| problem.field() != Some(field)));
    }
}

/// The single reveal of a new secret. The value lives only in this mounted field.
#[component]
pub fn TokenReveal(
    reveal: Reveal,
    locale: Signal<crate::localization::Locale>,
    copy_state: Signal<CopyState>,
    on_copy: Callback<()>,
    on_dismiss: Callback<()>,
) -> impl IntoView {
    let delegate = reveal.grant.delegate_actor_id.clone();
    let secret = reveal.token.reveal().to_owned();
    let initial = secret.clone();
    view! {
        <section class="delegation-notice delegation-reveal" data-tone="warning" aria-labelledby=REVEAL_HEADING>
            <h4 id=REVEAL_HEADING tabindex="-1">{move || text(Key::RevealTitle, locale.get())}</h4>
            <p>{move || text(Key::RevealBody, locale.get())}</p>
            <div class="cui-field delegation-token-field">
                <label class="cui-field-label" for=TOKEN_FIELD>
                    {move || text(Key::RevealField, locale.get())}" "<strong>{delegate.clone()}</strong>
                </label>
                <textarea id=TOKEN_FIELD class="cui-field-control delegation-token" readonly=true rows="3"
                    spellcheck="false" autocomplete="off" autocapitalize="off" aria-describedby="delegation-token-help"
                    prop:value=secret>{initial}</textarea>
                <p id="delegation-token-help" class="cui-field-help">{move || text(Key::RevealUse, locale.get())}</p>
            </div>
            <div class="toolbar">
                <ActionButton id="delegation-copy" icon=IconName::ContentCopy locale=locale on_press=Callback::new(move |_| on_copy.run(()))>
                    {move || text(Key::Copy, locale.get())}
                </ActionButton>
                <ActionButton id="delegation-dismiss" icon=IconName::Close locale=locale variant=ButtonVariant::Secondary on_press=Callback::new(move |_| on_dismiss.run(()))>
                    {move || text(Key::Dismiss, locale.get())}
                </ActionButton>
            </div>
            <p class="delegation-copy-status" role="status">
                {move || match copy_state.get() {
                    CopyState::Idle => "",
                    CopyState::Copied => text(Key::Copied, locale.get()),
                    CopyState::Manual => text(Key::CopyManual, locale.get()),
                }}
            </p>
        </section>
    }
}

/// A replayed issuance: the grant exists, its secret is gone for good.
#[component]
pub fn UnrecoverableNotice(
    grant: DelegationGrantResponse,
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    on_revoke: Callback<DelegationGrantResponse>,
) -> impl IntoView {
    let delegate = grant.delegate_actor_id.clone();
    let id = grant.id.clone();
    view! {
        <section class="delegation-notice delegation-unrecoverable" data-tone="warning" aria-labelledby=UNRECOVERABLE_HEADING>
            <h4 id=UNRECOVERABLE_HEADING tabindex="-1">{move || text(Key::UnrecoverableTitle, locale.get())}</h4>
            <p>{move || text(Key::UnrecoverableBody, locale.get())}</p>
            <p>{move || text(Key::DelegateLabel, locale.get())}": "<strong>{delegate.clone()}</strong>" · "
                {move || text(Key::GrantId, locale.get())}": "<code>{id.clone()}</code></p>
            <ActionButton id="delegation-revoke-unrecoverable" icon=IconName::Close locale=locale variant=ButtonVariant::Secondary
                focus_on_press=true blocked=blocked
                on_press=Callback::new(move |_| on_revoke.run(grant.clone()))>
                {move || text(Key::RevokeThis, locale.get())}
            </ActionButton>
        </section>
    }
}

/// One grant. Status is text plus tone; revocation names the delegate for assistive tech.
#[component]
pub fn GrantItem(
    grant: DelegationGrantResponse,
    revoked_here: bool,
    now_ms: i64,
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    on_revoke: Callback<DelegationGrantResponse>,
) -> impl IntoView {
    let state = grant_state(&grant, revoked_here, now_ms);
    let tone = match state {
        GrantState::Active => "success",
        GrantState::Expired => "neutral",
        GrantState::Revoked => "warning",
    };
    let heading = format!("{}-delegate", grant_dom_id(&grant.id));
    let labelled_by = heading.clone();
    let delegate = grant.delegate_actor_id.clone();
    let actions = grant.actions.clone();
    let (issued_at, expires_at, revoked_at) = (grant.issued_at, grant.expires_at, grant.revoked_at);
    let id = grant.id.clone();
    let revoke = (state != GrantState::Revoked).then(|| {
        let delegate = delegate.clone();
        let target = grant.clone();
        view! {
            <ActionButton id=format!("delegation-revoke-{id}") icon=IconName::Close locale=locale variant=ButtonVariant::Secondary
                focus_on_press=true blocked=blocked
                on_press=Callback::new(move |_| on_revoke.run(target.clone()))>
                {move || text(Key::Revoke, locale.get())}
                <span class="cui-sr-only">" "{move || text(Key::RevokeFor, locale.get())}" "{delegate.clone()}</span>
            </ActionButton>
        }
    });
    view! {
        <li id=grant_dom_id(&grant.id) class="delegation-grant" tabindex="-1" aria-labelledby=labelled_by data-state=format!("{state:?}").to_lowercase()>
            <div class="delegation-grant-heading">
                <h5 id=heading><span class="cui-sr-only">{move || text(Key::DelegateLabel, locale.get())}": "</span>{delegate}</h5>
                <span class="delegation-state" data-tone=tone>{move || copy::state(state, locale.get())}</span>
            </div>
            <ul class="delegation-grant-actions">
                {actions.into_iter().map(|action| view! {
                    <li>{move || copy::action_short(action, locale.get())}" "<code>{copy::action_label(action)}</code></li>
                }).collect_view()}
            </ul>
            <dl class="delegation-grant-times">
                <div><dt>{move || text(Key::Issued, locale.get())}</dt><dd>{move || display_time(issued_at, locale.get())}</dd></div>
                <div><dt>{move || text(Key::Expires, locale.get())}</dt><dd>{move || display_time(expires_at, locale.get())}</dd></div>
                {revoked_at.map(|at| view! {
                    <div><dt>{move || text(Key::RevokedAt, locale.get())}</dt><dd>{move || display_time(at, locale.get())}</dd></div>
                })}
                <div><dt>{move || text(Key::GrantId, locale.get())}</dt><dd><code>{grant.id.clone()}</code></dd></div>
            </dl>
            {(revoked_here && revoked_at.is_none()).then(|| view! {
                <p class="cui-field-help">{move || text(Key::RevokedNow, locale.get())}</p>
            })}
            {revoke}
        </li>
    }
}

/// Raw issuance inputs. Every field-level refusal comes from the server's 422 issues.
#[component]
pub fn IssueForm(
    draft: RwSignal<IssueDraft>,
    problems: RwSignal<Vec<Problem>>,
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    archived: Signal<bool>,
    #[prop(default = false.into())] secret_visible: Signal<bool>,
    on_submit: Callback<()>,
) -> impl IntoView {
    let delegate_error = field_error(problems, Field::Delegate, locale);
    let expiry_error = field_error(problems, Field::Expiry, locale);
    let actions_error = field_error(problems, Field::Actions, locale);
    let refused = Signal::derive(move || blocked.get() || archived.get() || secret_visible.get());
    let inputs = view! {
            // Freeze the submitted fields until the exact request is settled, so a success
            // cannot reset newer typing and a refusal always describes the visible draft.
            <fieldset class="delegation-inputs" disabled=move || blocked.get() aria-describedby="delegation-issue-help">
            <div class="live-grid">
                <div class="cui-field live-field">
                    <label class="cui-field-label" for=DELEGATE_FIELD>{move || text(Key::Delegate, locale.get())}</label>
                    <input id=DELEGATE_FIELD class="cui-field-control" type="text" autocomplete="off"
                        value=move || draft.with(|d| d.delegate.clone())
                        prop:value=move || draft.with(|d| d.delegate.clone())
                        aria-invalid=move || delegate_error.with(Option::is_some).to_string()
                        aria-describedby=move || if delegate_error.with(Option::is_some) {
                            "delegation-delegate-help delegation-delegate-error"
                        } else { "delegation-delegate-help" }
                        aria-errormessage=move || delegate_error.with(Option::is_some).then_some("delegation-delegate-error")
                        on:input=move |event| {
                            if !blocked.get_untracked() {
                                draft.update(|d| d.delegate = event_target_value(&event));
                                clear_field(problems, Field::Delegate);
                            }
                        }/>
                    <p id="delegation-delegate-help" class="cui-field-help">{move || text(Key::DelegateHelp, locale.get())}</p>
                    <p id="delegation-delegate-error" class="cui-field-error" hidden=move || delegate_error.with(Option::is_none)>
                        {move || delegate_error.get().unwrap_or_default()}
                    </p>
                </div>
                <div class="cui-field live-field">
                    <label class="cui-field-label" for=LIFETIME_FIELD>{move || text(Key::Lifetime, locale.get())}</label>
                    <select id=LIFETIME_FIELD class="cui-field-control"
                        prop:value=move || draft.with(|d| d.lifetime.value())
                        aria-invalid=move || expiry_error.with(Option::is_some).to_string()
                        aria-describedby=move || if expiry_error.with(Option::is_some) {
                            "delegation-lifetime-help delegation-lifetime-error"
                        } else { "delegation-lifetime-help" }
                        aria-errormessage=move || expiry_error.with(Option::is_some).then_some("delegation-lifetime-error")
                        on:change=move |event| {
                            if !blocked.get_untracked() {
                                if let Some(lifetime) = Lifetime::parse(&event_target_value(&event)) {
                                    draft.update(|d| d.lifetime = lifetime);
                                    clear_field(problems, Field::Expiry);
                                }
                            }
                        }>
                        {Lifetime::ALL.into_iter().map(|lifetime| view! {
                            <option value=lifetime.value() selected=move || draft.with(|d| d.lifetime == lifetime)>
                                {move || copy::lifetime(lifetime, locale.get())}
                            </option>
                        }).collect_view()}
                    </select>
                    <p id="delegation-lifetime-help" class="cui-field-help">{move || text(Key::LifetimeHelp, locale.get())}</p>
                    <p id="delegation-lifetime-error" class="cui-field-error" hidden=move || expiry_error.with(Option::is_none)>
                        {move || expiry_error.get().unwrap_or_default()}
                    </p>
                </div>
            </div>
            <fieldset class="delegation-actions" aria-describedby=move || {
                if actions_error.with(Option::is_some) { "delegation-actions-help delegation-actions-error" } else { "delegation-actions-help" }
            }>
                <legend class="cui-field-label">{move || text(Key::Actions, locale.get())}</legend>
                <p id="delegation-actions-help" class="cui-field-help">{move || text(Key::ActionsHelp, locale.get())}</p>
                {ACTIONS.into_iter().map(|action| {
                    let id = format!("delegation-action-{}", action_slug(action));
                    view! {
                        <div class="cui-field cui-choice">
                            <label for=id.clone() class="cui-choice-label">
                                <input id=id.clone() class="cui-choice-control" type="checkbox"
                                    checked=move || draft.with(|d| d.actions.contains(&action))
                                    prop:checked=move || draft.with(|d| d.actions.contains(&action))
                                    aria-invalid=move || actions_error.with(Option::is_some).to_string()
                                    aria-describedby=move || if actions_error.with(Option::is_some) {
                                        "delegation-actions-help delegation-actions-error"
                                    } else { "delegation-actions-help" }
                                    aria-errormessage=move || actions_error.with(Option::is_some).then_some("delegation-actions-error")
                                    on:change=move |event| {
                                        if !blocked.get_untracked() {
                                            draft.update(|d| *d = d.with_action(action, event_target_checked(&event)));
                                            clear_field(problems, Field::Actions);
                                        }
                                    }/>
                                <span class="cui-field-label">{move || format!("{} ({})", copy::action(action, locale.get()), copy::action_label(action))}</span>
                            </label>
                        </div>
                    }
                }).collect_view()}
                <p id="delegation-actions-error" class="cui-field-error" hidden=move || actions_error.with(Option::is_none)>
                    {move || actions_error.get().unwrap_or_default()}
                </p>
            </fieldset>
            </fieldset>
    }.into_any();
    view! {
        <form class="delegation-issue" aria-labelledby=ISSUE_HEADING novalidate=true
            on:submit=move |event| {
                event.prevent_default();
                if !refused.get_untracked() {
                    on_submit.run(());
                }
            }>
            <h4 id=ISSUE_HEADING tabindex="-1">{move || text(Key::IssueTitle, locale.get())}</h4>
            <p id="delegation-issue-help" class="cui-field-help" hidden=move || !secret_visible.get()>
                {move || text(Key::SaveTokenFirst, locale.get())}
            </p>
            {inputs}
            <p class="cui-field-help" hidden=move || !archived.get()>{move || text(Key::Archived, locale.get())}</p>
            <FormSubmit id="delegation-issue-submit" locale=locale blocked=refused>
                {move || text(Key::Issue, locale.get())}
            </FormSubmit>
        </form>
    }
}
