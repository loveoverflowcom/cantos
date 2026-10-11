use crate::ui::{
    ActionButton, BlockReason, DiscardDraftDialog, FormSubmit, WorkspaceNavigation, WorkspacePane,
};
use crate::{
    adaptation::ScriptAdaptation,
    api::{StudioContext, StudioHttp},
    authoring_view::StructuredEditor,
    editor::{Editor, Intent, Status, ValidationPreview},
    import::ManuscriptImport,
    inspector::RevisionInspector,
    messages::{
        self,
        workspace::{text, Key},
    },
    production::ProductionInputs,
};
use cantos_contracts::{ApiError, ErrorCode, FieldIssue, ValidateScriptRequest};
use cantos_ui::{
    Announcement, Button, ButtonVariant, Container, ContainerWidth, Density, FieldLabel, Select,
    Status as UiStatus, TextField, TextFieldKind, Theme, UiRoot,
};
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;

pub(crate) fn operation_id() -> Result<String, ApiError> {
    let error = || ApiError {
        code: ErrorCode::Unavailable,
        current_revision: None,
        issues: vec![],
    };
    let window = web_sys::window().ok_or_else(error)?;
    let crypto = window.crypto().map_err(|_| error())?;
    let mut bytes = [0u8; 16];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| error())?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

pub(crate) fn confirm_discard(english: bool) -> bool {
    web_sys::window().is_some_and(|window| {
        window
            .confirm_with_message(text(Key::DiscardConfirm, english))
            .unwrap_or(false)
    })
}

pub(crate) fn check_script(
    preview: RwSignal<ValidationPreview>,
    issues: RwSignal<Vec<FieldIssue>>,
    api: StudioHttp,
    actor: String,
    snapshot: String,
    current: Signal<String>,
) {
    if preview.with_untracked(|preview| preview.busy) || actor.is_empty() {
        return;
    }
    let checking = preview
        .get_untracked()
        .start(actor.clone(), snapshot.clone());
    let ticket = checking.ticket;
    preview.set(checking);
    issues.set(vec![]);
    spawn_local(async move {
        let result = api
            .validate(&ValidateScriptRequest {
                script_json: snapshot.clone(),
            })
            .await
            .map(|response| response.issues);
        let _ = preview.try_update(|p| *p = p.finish(ticket, &actor, &snapshot, result));
        if preview
            .try_with_untracked(|p| {
                p.ticket == ticket
                    && p.actor == actor
                    && p.snapshot == snapshot
                    && current.get_untracked() == snapshot
            })
            .unwrap_or(false)
        {
            let _ = issues.try_set(preview.with_untracked(|p| p.issues.clone()));
        }
    });
}

#[component]
pub(crate) fn ValidationControls(
    #[prop(into)] id: String,
    preview: RwSignal<ValidationPreview>,
    issues: RwSignal<Vec<FieldIssue>>,
    draft: Signal<String>,
    actor: Signal<String>,
    english: RwSignal<bool>,
    disabled: Signal<bool>,
) -> impl IntoView {
    let api = expect_context::<StudioContext>().api;
    view! {
        <div class="validation-controls">
            <ActionButton id=id english=english variant=ButtonVariant::Tonal reason=BlockReason::ContextReady blocked=Signal::derive(move || disabled.get() || actor.get().is_empty()) busy=Signal::derive(move || preview.with(|p| p.busy)) on_press=Callback::new(move |_| {
                if !disabled.get_untracked() { check_script(preview, issues, api, actor.get_untracked(), draft.get_untracked(), draft); }
            })>{move || text(Key::Validate, english.get())}</ActionButton>
            <UiStatus announcement=Announcement::Polite>{move || preview.with(|p| {
                if p.busy { text(Key::Validating, english.get()) }
                else if p.is_current(&actor.get(), &draft.get()) { text(Key::Valid, english.get()) }
                else if !p.snapshot.is_empty() && (p.snapshot != draft.get() || p.actor != actor.get()) { text(Key::Invalidated, english.get()) }
                else if let Some(error) = &p.error { messages::status(crate::editor::error_status(error), english.get()) }
                else if !p.snapshot.is_empty() { text(Key::Invalidated, english.get()) }
                else { text(Key::ValidateFirst, english.get()) }
            })}</UiStatus>
            <p class="help">{move || text(Key::ValidateHelp, english.get())}</p>
        </div>
    }
}

fn dispatch_save(
    state: RwSignal<Editor>,
    issues: RwSignal<Vec<FieldIssue>>,
    api: StudioHttp,
    intent: Intent,
) {
    spawn_local(async move {
        match api.save(&intent.script, &intent.request).await {
            Ok(response) => {
                let _ = state.try_update(|e| *e = e.saved(&intent, response));
            }
            Err(error) => {
                if state
                    .try_with_untracked(|e| {
                        e.pending.as_ref() == Some(&intent) && e.actor == intent.actor
                    })
                    .unwrap_or(false)
                {
                    let _ = issues.try_set(error.issues.clone());
                }
                let _ = state.try_update(|e| *e = e.save_failed(&intent, &error));
            }
        }
    });
}

#[component]
pub fn Studio() -> impl IntoView {
    provide_context(StudioContext { api: StudioHttp });
    let api = expect_context::<StudioContext>().api;
    let state = RwSignal::new(Editor::default());
    let issues = RwSignal::new(Vec::<FieldIssue>::new());
    let preview = RwSignal::new(ValidationPreview::default());
    let token = RwSignal::new(String::new());
    let selection = RwSignal::new(String::new());
    let english = RwSignal::new(false);
    let theme = RwSignal::new(Theme::System);
    let panel = RwSignal::new(WorkspacePane::Proposal);
    let discard_open = RwSignal::new(false);
    let dialog_failure = RwSignal::new(false);
    let activity = RwSignal::new(false);
    let proposal_dirty = RwSignal::new(false);
    let proposal_activity = RwSignal::new(false);
    let inspector_pending = RwSignal::new(false);
    let production_dirty = RwSignal::new(false);
    let production_activity = RwSignal::new(false);
    let reviewed = RwSignal::new(false);
    let actor = Signal::derive(move || state.with(|e| e.actor.clone()));
    let draft = Signal::derive(move || {
        state.with(|e| {
            if e.can_edit() {
                e.draft.clone()
            } else {
                String::new()
            }
        })
    });
    let copy = move || messages::copy(english.get());
    let blocked = move || state.with(|e| e.busy || e.pending.is_some()) || inspector_pending.get();
    let save_blocked = move || {
        blocked()
            || activity.get()
            || !reviewed.get()
            || !state.with(Editor::can_save)
            || !preview.with(|p| p.is_current(&actor.get(), &draft.get()))
    };
    Effect::new(move |_| {
        if activity.get() {
            reviewed.set(false);
        }
    });
    // Reconciliation adopts a different base only after a fresh review of the displayed head.
    let review_context = Memo::new(move |_| {
        state.with(|editor| {
            (
                editor.actor.clone(),
                editor.script.clone(),
                editor.base,
                editor
                    .remote
                    .as_ref()
                    .map(|revision| (revision.revision, revision.export_digest.clone())),
            )
        })
    });
    Effect::new(move |_| {
        review_context.get();
        reviewed.set(false);
    });
    // A browser-owned confirmation covers back, reload and close. Drafts remain session memory only.
    let unload =
        leptos::leptos_dom::helpers::window_event_listener_untyped("beforeunload", move |event| {
            if state.with_untracked(Editor::has_unsaved)
                || activity.get_untracked()
                || proposal_dirty.get_untracked()
                || inspector_pending.get_untracked()
                || production_dirty.get_untracked()
                || production_activity.get_untracked()
            {
                event.prevent_default();
                if let Some(event) = event.dyn_ref::<web_sys::BeforeUnloadEvent>() {
                    event.set_return_value("");
                }
            }
        });
    on_cleanup(move || unload.remove());
    let restored =
        leptos::leptos_dom::helpers::window_event_listener_untyped("pageshow", move |_| {
            let _ = state.try_update(|_| ());
            let _ = selection.try_update(|_| ());
            let _ = token.try_update(|_| ());
        });
    on_cleanup(move || restored.remove());

    view! {
        <UiRoot class="studio-shell" theme=theme density=Density::Comfortable>
        <Container width=ContainerWidth::Fluid>
        <main class="studio-main" lang=move || if english.get() { "en" } else { "vi-VN" }>
            <header class="studio-header">
                <h1>{move || copy().title}<small>"Vọng Đài"</small></h1>
                <div class="toolbar">
                    <Button id="studio-language" variant=ButtonVariant::Text on_press=Callback::new(move |_| english.update(|en| *en = !*en))>{move || if english.get() { "Tiếng Việt" } else { "English" }}</Button>
                    <Select id="theme-select" label=Signal::derive(move || copy().theme.to_owned())
                        value=Signal::derive(move || theme.get().as_str().to_owned())
                        on_input=Callback::new(move |value: String| { if let Some(selected) = Theme::from_attribute(&value) { theme.set(selected); } })>
                        <option value="system" selected=move || theme.get() == Theme::System>{move || copy().system}</option>
                        <option value="light" selected=move || theme.get() == Theme::Light>{move || copy().light}</option>
                        <option value="dark" selected=move || theme.get() == Theme::Dark>{move || copy().dark}</option>
                    </Select>
                </div>
            </header>
            <details class="session-panel" open=move || state.with(|e| e.actor.is_empty())>
                <summary>{move || text(Key::Session, english.get())}" · "{move || state.with(|e| if e.actor.is_empty() { copy().unknown.to_owned() } else { e.actor.clone() })}</summary>
                <p class="mode">{move || copy().mode}</p>
                <form on:submit=move |event| {
                    event.prevent_default(); if state.with_untracked(|e| e.busy) || activity.get_untracked() || proposal_activity.get_untracked() { return; }
                    let submitted = token.get_untracked();
                    state.update(|e| { e.busy = true; e.status = Status::Loading; });
                    spawn_local(async move {
                        match api.login(submitted).await {
                            Ok(response) => { let _ = token.try_set(String::new()); let _ = state.try_update(|e| *e = e.signed_in(response.actor_id)); },
                            Err(error) => { let _ = state.try_update(|e| *e = e.session_failed(&error)); }
                        }
                    });
                }>
                    <div class="toolbar session-controls">
                        <TextField id="session-token" kind=TextFieldKind::Password autocomplete="off" label=Signal::derive(move || copy().token.to_owned())
                            value=token on_input=Callback::new(move |value| token.set(value)) />
                        <FormSubmit id="session-submit" english=english blocked=Signal::derive(move || state.with(|e| e.busy) || activity.get() || proposal_activity.get())>{move || copy().login}</FormSubmit>
                    </div>
                </form>
            </details>
            <WorkspaceNavigation panel=panel english=english/>
            <div id="workspace-import" hidden=move || panel.get() != WorkspacePane::Import><ManuscriptImport actor=actor english=english /></div>
            <div id="workspace-proposal" hidden=move || panel.get() != WorkspacePane::Proposal><ScriptAdaptation actor=actor english=english dirty=proposal_dirty activity=proposal_activity /></div>
            <div id="workspace-production" hidden=move || panel.get() != WorkspacePane::Production><ProductionInputs editor=state english=english script_activity=activity dirty=production_dirty activity=production_activity /></div>
            <section id="workspace-revisions" hidden=move || panel.get() != WorkspacePane::Revisions aria-labelledby="revision-title">
                <h2 id="revision-title">{move || text(Key::Revisions, english.get())}</h2>
                <form on:submit=move |event| {
                    event.prevent_default(); if blocked() || activity.get_untracked() || production_activity.get_untracked() { return; }
                    let target = selection.get_untracked();
                    if state.with_untracked(|e| e.script != target) {
                        if (state.with_untracked(Editor::is_dirty) || production_dirty.get_untracked()) && !confirm_discard(english.get_untracked()) { return; }
                        state.update(|e| *e = e.discard_and_select(target)); issues.set(vec![]); reviewed.set(false); preview.set(ValidationPreview::default());
                    }
                    let Some(reading) = state.get_untracked().start_read() else { return; };
                    let script = reading.script.clone(); let ticket = reading.ticket;
                    state.set(reading);
                    spawn_local(async move {
                        let result = api.read(&script).await;
                        let _ = state.try_update(|e| *e = match result { Ok(response) => e.loaded_with_buffer(ticket, response, activity.get_untracked()), Err(error) => e.read_failed(ticket, &error) });
                    });
                }>
                    <FieldLabel for_id="script-id">{move || copy().script}</FieldLabel>
                    <div class="toolbar"><input class="cui-field-control" id="script-id" autocomplete="off" spellcheck="false" disabled=blocked prop:value=move || selection.get() on:input=move |e| selection.set(event_target_value(&e)) />
                        <FormSubmit id="revision-read" english=english blocked=Signal::derive(move || blocked() || activity.get() || production_activity.get())>{move || copy().read}</FormSubmit></div>
                </form>
                <div class="revision-strip">
                    <p class="identity"><code>{move || state.with(|e| e.script.clone())}</code>" · "{move || copy().base}<output id="base-revision">{move || state.with(|e| e.base)}</output></p>
                    <p>{move || if state.with(Editor::is_dirty) || activity.get() { text(Key::Unsaved, english.get()) } else { text(Key::Saved, english.get()) }}</p>
                </div>
                <StructuredEditor draft=draft english=english activity=activity namespace="revision"
                    readonly=Signal::derive(move || state.with(|e| e.busy || !e.can_edit()))
                    baseline=Signal::derive(move || state.with(|e| e.remote.as_ref().map(|r| r.script_json.clone()).unwrap_or_default()))
                    issues=Signal::derive(move || issues.get())
                    on_change=Callback::new(move |text| { state.update(|e| *e = e.edit(text)); issues.set(vec![]); reviewed.set(false); })>
                    <RevisionInspector state=state english=english activity=inspector_pending draft_activity=activity />
                </StructuredEditor>
                <ValidationControls id="revision-validate" preview=preview issues=issues draft=draft actor=actor english=english disabled=Signal::derive(move || blocked() || activity.get()) />
                <label class="adaptation-confirm" for="revision-reviewed"><input id="revision-reviewed" type="checkbox" disabled=move || blocked() || activity.get() || !preview.with(|p| p.is_current(&actor.get(), &draft.get())) prop:checked=move || reviewed.get()
                    on:change=move |e| reviewed.set(event_target_checked(&e)) /><span>{move || text(Key::ReviewBeforeSave, english.get())}</span></label>
                <div class="toolbar">
                    <ActionButton id="revision-save" english=english blocked=Signal::derive(save_blocked) busy=Signal::derive(move || state.with(|e| e.busy)) reason=BlockReason::SaveReview described_by="request-reason" on_press=Callback::new(move |_| {
                        if save_blocked() { return; }
                        match operation_id() {
                            Ok(op) => if let Some((saving,intent)) = state.get_untracked().start_save(op) { state.set(saving); issues.set(vec![]); dispatch_save(state,issues,api,intent); },
                            Err(error) => state.update(|e| *e = e.failed(&error)),
                        }
                    })>{move || copy().save}</ActionButton>
                    <Show when=move || state.with(|e| e.pending.is_some() && !e.busy)><ActionButton id="revision-retry" english=english variant=ButtonVariant::Tonal reason=BlockReason::Pending blocked=Signal::derive(move || activity.get() || state.with(|e| e.pending.as_ref().is_some_and(|i| i.actor != e.actor))) on_press=Callback::new(move |_| {
                        if activity.get_untracked() { return; }
                        if let Some((saving,intent)) = state.get_untracked().retry() { state.set(saving); dispatch_save(state,issues,api,intent); }
                    })>{move || copy().retry}</ActionButton></Show>
                    <ActionButton id="revision-discard" english=english variant=ButtonVariant::Secondary
                        focus_on_press=true
                        blocked=Signal::derive(move || blocked() || activity.get() || production_activity.get())
                        on_press=Callback::new(move |_| {
                            if blocked() || activity.get_untracked() || production_activity.get_untracked() { return; }
                            dialog_failure.set(false); discard_open.set(true);
                        })>{move || text(Key::Discard, english.get())}</ActionButton>
                </div>
                <p id="request-reason" class="help">{move || state.with(|e| if e.busy { copy().busy } else if e.pending.is_some() { copy().pending } else if e.actor.is_empty() { copy().sign_in_first } else if save_blocked() { text(Key::ValidateFirst, english.get()) } else { "" })}</p>
                <p class="help">{move || text(Key::PermissionHelp, english.get())}</p>
            </section>
            <UiStatus announcement=Announcement::Polite><span id="status">{move || messages::status(state.with(|e| e.status),english.get())}</span></UiStatus>
            <Show when=move || dialog_failure.get()><UiStatus announcement=Announcement::Polite><p>{move || text(Key::DialogFailure, english.get())}</p></UiStatus></Show>
            <DiscardDraftDialog open=discard_open english=english
                blocked=Signal::derive(move || blocked() || activity.get() || production_activity.get())
                on_error=Callback::new(move |_| { dialog_failure.set(true); discard_open.set(false); })
                on_confirm=Callback::new(move |()| {
                    if blocked() || activity.get_untracked() || production_activity.get_untracked() { return false; }
                    state.update(|e| *e = e.discard_and_select(String::new())); selection.set(String::new());
                    reviewed.set(false); issues.set(vec![]); preview.set(ValidationPreview::default()); true
                })/>
        </main>
        </Container>
        </UiRoot>
    }
}
