//! Browser shell for owner delegation management: one fenced request authority, the explicit
//! clipboard effect and focus management. Every decision stays with the server.
use super::{
    copy::{self, text, Key},
    problems, progress, recovery_focus, request_activity,
    view::{
        grant_dom_id, CopyState, GrantItem, IssueForm, TokenReveal, UnrecoverableNotice,
        ISSUE_HEADING, LIST_HEADING, RETRY, REVEAL_HEADING, STATUS, TOKEN_FIELD,
        UNRECOVERABLE_HEADING,
    },
    Grants, IssueDraft, Problem,
};
use crate::{
    live::{
        workspace::{uuid, Dispatch},
        ClientError, Command, Phase, Reply, RequestState,
    },
    ui::{ActionButton, DelegationRevokeDialog},
};
use cantos_contracts::{CatalogWork, DelegationGrantResponse};
use cantos_ui::ButtonVariant;
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::AbortController;

fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

/// Move focus after the DOM reflects the acknowledged change.
fn focus_later(id: String) {
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        if let Some(element) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(&id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    });
}

/// A pending request disables the control that started it, which drops keyboard focus to the
/// page. Only then is focus moved to where the outcome is explained.
fn focus_if_lost(id: &'static str) {
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let lost = document
            .active_element()
            .is_none_or(|element| element.tag_name().eq_ignore_ascii_case("body"));
        if let (true, Some(element)) = (
            lost,
            document
                .get_element_by_id(id)
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()),
        ) {
            let _ = element.focus();
        }
    });
}

/// Explicit user-initiated copy. `navigator.clipboard` is absent outside secure contexts.
async fn write_clipboard(secret: &str) -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(clipboard) = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("clipboard"))
    else {
        return false;
    };
    if clipboard.is_undefined() || clipboard.is_null() {
        return false;
    }
    let clipboard: web_sys::Clipboard = clipboard.unchecked_into();
    wasm_bindgen_futures::JsFuture::from(clipboard.write_text(secret))
        .await
        .is_ok()
}

/// Fallback: select the read-only field so the owner can copy it with the keyboard.
fn select_token() {
    if let Some(field) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(TOKEN_FIELD))
        .and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
    {
        let _ = field.focus();
        field.select();
    }
}

#[component]
pub fn DelegationPanel(
    actor: Signal<String>,
    work: Signal<Option<CatalogWork>>,
    english: RwSignal<bool>,
    session_pending: Signal<bool>,
    /// True while a delegation write is in flight or unresolved; the workspace keeps the Work.
    unresolved: RwSignal<bool>,
    /// Only the current network attempt blocks session observation or reauthentication.
    busy: RwSignal<bool>,
    dirty: RwSignal<bool>,
) -> impl IntoView {
    let state = RwSignal::new(RequestState::default());
    let grants = RwSignal::new(Grants::default());
    let draft = RwSignal::new(IssueDraft::default());
    let issue_problems = RwSignal::new(Vec::<Problem>::new());
    let copy_state = RwSignal::new(CopyState::Idle);
    let revoke_target = RwSignal::new(None::<DelegationGrantResponse>);
    let revoke_open = RwSignal::new(false);
    let dialog_failed = RwSignal::new(false);
    let clock = RwSignal::new(now_ms());
    let work_id =
        Memo::new(move |_| work.with(|w| w.as_ref().map(|w| w.id.clone()).unwrap_or_default()));
    let work_title = Memo::new(move |_| {
        work.with(|w| {
            w.as_ref()
                .map(|w| w.details.title.clone())
                .unwrap_or_default()
        })
    });
    let owner = Memo::new(move |_| {
        let actor = actor.get();
        !actor.is_empty() && work.with(|w| w.as_ref().is_some_and(|w| w.owner_id == actor))
    });
    let archived = Signal::derive(move || {
        work.with(|w| w.as_ref().is_some_and(|w| w.lifecycle == "archived"))
    });
    let reveal = Memo::new(move |_| grants.with(|g| g.reveal.clone()));
    let unrecoverable = Memo::new(move |_| grants.with(|g| g.unrecoverable.clone()));
    let dispatch = Dispatch {
        state,
        session_pending,
        abort: StoredValue::new_local(None::<AbortController>),
        api: expect_context::<crate::api::StudioContext>().api,
        complete: Callback::new(
            move |(command, reply): (Command, Result<Reply, ClientError>)| match reply {
                Ok(reply) => {
                    let focus = match (&command, &reply) {
                        (Command::IssueDelegation { .. }, Reply::DelegationIssued(issued)) => {
                            let _ = draft.try_set(IssueDraft::default());
                            let _ = issue_problems.try_set(vec![]);
                            Some(if issued.token.is_some() {
                                let _ = copy_state.try_set(CopyState::Idle);
                                // A visible unsaved secret is protected by the unload prompt.
                                let _ = dirty.try_set(true);
                                REVEAL_HEADING.to_owned()
                            } else {
                                UNRECOVERABLE_HEADING.to_owned()
                            })
                        }
                        (Command::RevokeDelegation { grant, .. }, Reply::Revoked) => Some(
                            if grants
                                .with_untracked(|g| g.items.iter().any(|item| item.id == *grant))
                            {
                                grant_dom_id(grant)
                            } else {
                                LIST_HEADING.to_owned()
                            },
                        ),
                        _ => None,
                    };
                    let _ = clock.try_set(now_ms());
                    let _ = grants.try_update(|g| *g = g.apply(&command, reply));
                    match focus {
                        Some(id) => focus_later(id),
                        None => focus_if_lost(recovery_focus(&command, Phase::Ready, &[])),
                    }
                }
                Err(error) => {
                    if matches!(error, ClientError::Http { status: 403, .. }) {
                        let _ = grants.try_update(|g| *g = g.forbid());
                    }
                    let found = problems(&error);
                    let outcome = state
                        .try_with_untracked(RequestState::phase)
                        .unwrap_or(Phase::Failed);
                    focus_if_lost(if matches!(error, ClientError::Http { status: 403, .. }) {
                        "delegation-title"
                    } else {
                        recovery_focus(&command, outcome, &found)
                    });
                    if matches!(command, Command::IssueDelegation { .. }) {
                        let _ = issue_problems.try_set(found);
                    }
                }
            },
        ),
    };
    let blocked = Signal::derive(move || {
        session_pending.get()
            || !owner.get()
            || grants.with(|g| g.forbidden)
            || state.with(|s| s.pending().is_some())
    });
    // One effect orders the context reset before the first read of a newly selected Work.
    Effect::new(move |_| {
        let actor = actor.get();
        let target = work_id.get();
        let ready = owner.get() && !session_pending.get() && !target.is_empty();
        if state.with_untracked(|s| s.actor() != actor || s.target() != target) {
            dispatch.abort();
            state.update(|s| {
                if let Some(next) = s.context(&actor, &target) {
                    *s = next;
                }
            });
            grants.set(Grants::for_work(&target));
            draft.set(IssueDraft::default());
            issue_problems.set(vec![]);
            copy_state.set(CopyState::Idle);
            revoke_open.set(false);
            revoke_target.set(None);
        }
        if ready
            && grants.with_untracked(|g| !g.loaded && !g.forbidden && g.work == target)
            && state.with_untracked(|s| s.phase() == Phase::Idle)
        {
            dispatch.send(Command::Delegations {
                work: target,
                after: None,
            });
        }
    });
    Effect::new(move |_| {
        let activity = state.with(request_activity);
        busy.set(activity.busy);
        unresolved.set(activity.unresolved);
    });
    // Keep this panel mounted with the workspace: context changes reset its facts while its
    // reactive controls retain a stable owner. Cleanup performs only the local abort effect.
    on_cleanup(move || dispatch.abort());

    let refresh = move || {
        dispatch.send(Command::Delegations {
            work: work_id.get_untracked(),
            after: None,
        })
    };
    let issue = Callback::new(move |()| {
        if blocked.get_untracked() {
            return;
        }
        // Do not replace a show-once secret before the owner explicitly saves/dismisses it.
        if grants.with_untracked(|g| g.reveal.is_some()) {
            return;
        }
        let work = work_id.get_untracked();
        let draft = draft.get_untracked();
        issue_problems.set(vec![]);
        dispatch.build((|| {
            Ok(Command::IssueDelegation {
                work,
                request: draft.request(uuid()?, uuid()?, now_ms())?,
            })
        })());
    });
    let open_revoke = Callback::new(move |grant: DelegationGrantResponse| {
        if blocked.get_untracked() {
            return;
        }
        dialog_failed.set(false);
        revoke_target.set(Some(grant));
        revoke_open.set(true);
    });
    let confirm_revoke = Callback::new(move |_| {
        let Some(grant) = revoke_target.get_untracked() else {
            return;
        };
        revoke_open.set(false);
        dispatch.send(Command::RevokeDelegation {
            work: work_id.get_untracked(),
            grant: grant.id,
        });
    });
    let on_copy = Callback::new(move |()| {
        let Some(secret) = grants.with_untracked(|g| g.reveal.as_ref().map(|r| r.token.clone()))
        else {
            return;
        };
        spawn_local(async move {
            let copied = write_clipboard(secret.reveal()).await;
            // Report only for the same, still-visible reveal.
            let current = grants
                .try_with_untracked(|g| g.reveal.as_ref().is_some_and(|r| r.token == secret))
                .unwrap_or(false);
            if current {
                if !copied {
                    select_token();
                }
                let _ = copy_state.try_set(if copied {
                    CopyState::Copied
                } else {
                    CopyState::Manual
                });
            }
        });
    });
    let drop_secret = move || {
        grants.update(|g| *g = g.dismiss_reveal());
        copy_state.set(CopyState::Idle);
        // The acknowledged reply also carried the secret; drop that copy too.
        state.update(|s| {
            if matches!(s.result(), Some(Reply::DelegationIssued(_))) {
                if let Some(next) = s.forget_result() {
                    *s = next;
                }
            }
        });
    };
    let on_dismiss = Callback::new(move |()| {
        drop_secret();
        focus_later(ISSUE_HEADING.to_owned());
    });
    // The shell hides private workspaces while the browser session does not confirm their
    // actor (another account, expiry). A one-time secret never waits there, hidden.
    Effect::new(move |_| {
        if (session_pending.get() || !owner.get()) && grants.with_untracked(|g| g.reveal.is_some())
        {
            drop_secret();
        }
    });
    let abandon = Callback::new(move |_| {
        if session_pending.get_untracked() {
            return;
        }
        state.update(|s| {
            if let Some(next) = s.abandon() {
                *s = next;
            }
        });
        refresh();
    });

    view! {
        <section id="delegation-panel" class="live-section delegation-panel" aria-labelledby="delegation-title"
            hidden=move || work_id.get().is_empty() inert=move || work_id.get().is_empty()>
            <h3 id="delegation-title" tabindex="-1">{move || text(Key::Title, english.get())}</h3>
            <p class="delegation-member" hidden=move || owner.get() && !grants.with(|g| g.forbidden)>
                {move || text(if grants.with(|g| g.forbidden) { Key::ForbiddenHelp } else { Key::MemberOnly }, english.get())}
            </p>
            <div hidden=move || !owner.get() || grants.with(|g| g.forbidden)
                inert=move || !owner.get() || grants.with(|g| g.forbidden)>
                <p>{move || text(Key::Intro, english.get())}</p>
                <div class="live-status delegation-status">
                    <div id=STATUS tabindex="-1" role="status" aria-live="polite">
                        <p>{move || state.with(progress).map(|key| text(key, english.get())).unwrap_or_default()}</p>
                        <ul class="delegation-problems">
                            {move || state.with(|s| s.error().map(problems).unwrap_or_default()).into_iter()
                                .map(|problem| view! { <li>{copy::problem(&problem, english.get())}</li> }).collect_view()}
                        </ul>
                        <p hidden=move || !dialog_failed.get()>{move || text(Key::DialogFailed, english.get())}</p>
                    </div>
                    <Show when=move || state.with(|s| s.phase() == Phase::Pending)>
                        <div class="toolbar">
                            <ActionButton id="delegation-cancel" english=english blocked=Signal::derive(|| false)
                                variant=ButtonVariant::Text on_press=Callback::new(move |_| {
                                dispatch.cancel();
                                focus_later(if state.with_untracked(|s| s.phase() == Phase::Ambiguous) {
                                    RETRY
                                } else {
                                    STATUS
                                }.to_owned());
                            })>
                                {move || text(Key::Cancel, english.get())}
                            </ActionButton>
                        </div>
                    </Show>
                    <Show when=move || state.with(|s| s.phase() == Phase::Ambiguous)>
                        <div class="toolbar">
                            <ActionButton id=RETRY english=english variant=ButtonVariant::Tonal
                                blocked=Signal::derive(move || session_pending.get() || state.with(|s| !s.can_retry()))
                                on_press=Callback::new(move |_| dispatch.retry())>
                                {move || text(Key::Retry, english.get())}
                            </ActionButton>
                            <ActionButton id="delegation-abandon" english=english variant=ButtonVariant::Text
                                described_by="delegation-abandon-help" blocked=session_pending on_press=abandon>
                                {move || text(Key::Abandon, english.get())}
                            </ActionButton>
                        </div>
                        <p id="delegation-abandon-help" class="cui-field-help">{move || text(Key::AbandonHelp, english.get())}</p>
                    </Show>
                </div>
                {move || reveal.get().map(|reveal| view! {
                    <TokenReveal reveal=reveal english=english copy_state=copy_state.into() on_copy=on_copy on_dismiss=on_dismiss/>
                })}
                {move || unrecoverable.get().map(|grant| view! {
                    <UnrecoverableNotice grant=grant english=english blocked=blocked on_revoke=open_revoke/>
                })}
                <IssueForm draft=draft problems=issue_problems english=english blocked=blocked archived=archived
                    secret_visible=Signal::derive(move || reveal.with(Option::is_some)) on_submit=issue/>
                <section class="delegation-list" aria-labelledby=LIST_HEADING>
                    <h4 id=LIST_HEADING tabindex="-1">{move || text(Key::ListTitle, english.get())}</h4>
                    <div class="toolbar">
                        <ActionButton id="delegation-refresh" english=english variant=ButtonVariant::Secondary blocked=blocked
                            on_press=Callback::new(move |_| refresh())>{move || text(Key::Refresh, english.get())}</ActionButton>
                        <Show when=move || grants.with(|g| g.next_after.is_some())>
                            <ActionButton id="delegation-more" english=english variant=ButtonVariant::Secondary blocked=blocked
                                on_press=Callback::new(move |_| dispatch.send(Command::Delegations {
                                    work: work_id.get_untracked(),
                                    after: grants.with_untracked(|g| g.next_after.clone()),
                                }))>{move || text(Key::More, english.get())}</ActionButton>
                        </Show>
                    </div>
                    <Show when=move || grants.with(|g| g.loaded && g.items.is_empty())>
                        <p>{move || text(Key::Empty, english.get())}</p>
                        <p class="cui-field-help">{move || text(Key::EmptyHint, english.get())}</p>
                    </Show>
                    <p class="cui-field-help" hidden=move || grants.with(|g| g.loaded)>{move || text(Key::NotLoaded, english.get())}</p>
                    <ul class="delegation-grants">
                        <For each=move || grants.with(|g| g.items.iter().map(|grant| (grant.clone(), g.revoked_here(&grant.id), clock.get())).collect::<Vec<_>>())
                            key=|(grant, here, now)| (grant.id.clone(), grant.revoked_at, *here, *now >= grant.expires_at)
                            children=move |(grant, here, now)| view! {
                                <GrantItem grant=grant revoked_here=here now_ms=now english=english blocked=blocked on_revoke=open_revoke/>
                            }/>
                    </ul>
                </section>
                <DelegationRevokeDialog open=revoke_open english=english blocked=blocked on_confirm=confirm_revoke
                    on_error=Callback::new(move |_| { dialog_failed.set(true); revoke_open.set(false); })
                    details=Signal::derive(move || revoke_target.with(|t| t.as_ref().map(|g| format!(
                        "{}: {} · {}: {}", text(Key::DelegateLabel, english.get()), g.delegate_actor_id,
                        text(Key::WorkLabel, english.get()), work_title.get())).unwrap_or_default()))/>
            </div>
        </section>
    }
}
