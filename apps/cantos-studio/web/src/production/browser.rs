//! Native Leptos shell over immutable, actor-bound production commands.
use super::*;
use crate::ui::{ActionButton, BlockReason, ContextSelect};
use crate::{
    api::{StudioContext, StudioHttp},
    messages::{
        self,
        production::{text, Key},
    },
    view::{confirm_discard, operation_id},
};
use cantos_ui::{Announcement, ButtonVariant, FieldLabel, Status as UiStatus};
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;

#[derive(Clone, Copy)]
struct ProductionTyping(
    RwSignal<std::collections::BTreeSet<String>>,
    RwSignal<Production>,
);

fn dispatch(state: RwSignal<Production>, api: StudioHttp, intent: Intent) {
    let api = api.snapshot();
    spawn_local(async move {
        let script = &intent.context.script;
        let result = match &intent.command {
            Command::Settings(request) => api
                .save_production_settings(script, request)
                .await
                .map(Receipt::Settings),
            Command::Rights(request) => api
                .save_production_rights(script, request)
                .await
                .map(Receipt::Rights),
            Command::Freeze(request) => api
                .freeze_production(script, request)
                .await
                .map(|s| Receipt::Freeze(Box::new(s))),
            Command::Approve {
                snapshot_id,
                request,
            } => api
                .approve_production(script, snapshot_id, request)
                .await
                .map(Receipt::Approve),
        };
        let current = state
            .try_with_untracked(|s| s.pending() == Some(&intent) && s.context == intent.context)
            .unwrap_or(false);
        let _ = state.try_update(|state| {
            *state = match result {
                Ok(receipt) => state.recorded(&intent, receipt),
                Err(error) => state.failed(&intent, error),
            }
        });
        if let Command::Approve { snapshot_id, .. } = &intent.command {
            if current
                && state
                    .try_with_untracked(|s| {
                        s.context == intent.context && s.pending().is_none() && s.error.is_none()
                    })
                    .unwrap_or(false)
            {
                read_snapshot(state, api, snapshot_id.clone());
            }
        }
    });
}

fn submit(state: RwSignal<Production>, api: StudioHttp, command: Command) {
    if let Some((sending, intent)) = state.get_untracked().start(command) {
        state.set(sending);
        dispatch(state, api, intent);
    }
}

fn read_snapshot(state: RwSignal<Production>, api: StudioHttp, id: String) {
    let Some((reading, ticket)) = state
        .get_untracked()
        .start_read(ReadKind::Snapshot(id.clone()))
    else {
        return;
    };
    state.set(reading);
    let api = api.snapshot();
    spawn_local(async move {
        let result = api.production_snapshot(&ticket.context.script, &id).await;
        let _ = state.try_update(|state| {
            *state = match result {
                Ok(snapshot) => state.snapshot_loaded(&ticket, snapshot),
                Err(error) => state.read_failed(&ticket, error),
            }
        });
    });
}

#[derive(Clone, Debug)]
struct RightsForm {
    subject: String,
    record_id: String,
    status: ProductionRightsStatus,
    reference: String,
    from: String,
    until: String,
    holder: String,
    languages: String,
    territory: String,
    attribution: String,
    restrictions: String,
    permitted_scope: String,
}

impl Default for RightsForm {
    fn default() -> Self {
        Self {
            subject: String::new(),
            record_id: String::new(),
            status: ProductionRightsStatus::Pending,
            reference: String::new(),
            from: "0".into(),
            until: String::new(),
            holder: String::new(),
            languages: String::new(),
            territory: "private-planning".into(),
            attribution: String::new(),
            restrictions: String::new(),
            permitted_scope: String::new(),
        }
    }
}

impl RightsForm {
    fn declaration(&self, form: &SettingsForm) -> Option<ProductionRightsDeclaration> {
        let subject = if let Some(id) = self.subject.strip_prefix("evidence:") {
            ProductionRightsSubject::Evidence {
                record_id: id.into(),
            }
        } else {
            let id = self.subject.strip_prefix("voice:")?;
            let binding = form.bindings.iter().find(|b| b.character_id == id)?;
            ProductionRightsSubject::Voice {
                provider_id: binding.provider_id.clone(),
                model_id: binding.model_id.clone(),
                voice_id: binding.voice_id.clone(),
            }
        };
        Some(ProductionRightsDeclaration {
            record_id: self.record_id.clone(),
            subject,
            scope: ProductionRightsScope::ProductionSynthesis,
            status: self.status,
            reference: self.reference.clone(),
            valid_from_unix: self.from.parse().ok()?,
            valid_until_unix: if self.until.is_empty() {
                None
            } else {
                Some(self.until.parse().ok()?)
            },
            rights_holder: self.holder.clone(),
            languages: self
                .languages
                .split(',')
                .map(|l| l.trim().to_owned())
                .filter(|l| !l.is_empty())
                .collect(),
            territory: self.territory.clone(),
            attribution: self.attribution.clone(),
            restrictions: self.restrictions.clone(),
            permitted_scope: self.permitted_scope.clone(),
        })
    }
}

#[component]
fn Findings(
    findings: Vec<ProductionFinding>,
    locale: Signal<crate::localization::Locale>,
) -> impl IntoView {
    view! { <div class="production-findings"><h4>{move || text(Key::Findings, locale.get())}</h4>
        {if findings.is_empty() { view! { <p>{move || text(Key::NoFindings, locale.get())}</p> }.into_any() }
        else { view! { <ul>{findings.into_iter().map(|finding| view! { <li><strong>{move || messages::production::finding(finding.code, locale.get())}</strong><small><code>{finding.path}</code></small></li> }).collect_view()}</ul> }.into_any() }}
    </div> }
}

#[component]
fn Estimate(
    estimate: ProductionEstimate,
    locale: Signal<crate::localization::Locale>,
) -> impl IntoView {
    let value = match estimate {
        ProductionEstimate::Unavailable { .. } => None,
        ProductionEstimate::Known {
            currency,
            amount_minor,
            units,
            rate_reference,
            rate_version,
        } => Some((currency, amount_minor, units, rate_reference, rate_version)),
    };
    view! { <dl class="source-identity production-cost"><dt>{move || text(Key::Estimate, locale.get())}</dt><dd>{move || value.as_ref().map(|(currency,minor,units,reference,version)|format!("{} · {units} {} · {reference} / {version}",format_money(*currency,*minor,locale.get()),text(Key::UnicodeUnits,locale.get()))).unwrap_or_else(|| text(Key::UnknownEstimate, locale.get()).into())}</dd>
        <dt>{move || text(Key::Reservation, locale.get())}</dt><dd>{move || text(Key::NoReservation, locale.get())}</dd>
        <dt>{move || text(Key::Actual, locale.get())}</dt><dd>{move || text(Key::NoActual, locale.get())}</dd>
    </dl> }
}

#[component]
pub fn ProductionInputs(
    editor: RwSignal<Editor>,
    locale: Signal<crate::localization::Locale>,
    script_activity: RwSignal<bool>,
    dirty: RwSignal<bool>,
    activity: RwSignal<bool>,
    session_pending: Signal<bool>,
) -> impl IntoView {
    let state = RwSignal::new(Production::default());
    let composing = RwSignal::new(std::collections::BTreeSet::<String>::new());
    provide_context(ProductionTyping(composing, state));
    let api = expect_context::<StudioContext>().api;
    let rights = RwSignal::new(RightsForm::default());
    let rights_dirty = RwSignal::new(false);
    let retained_rights = RwSignal::new(None::<(Context, RightsForm, bool)>);
    let snapshot_id = RwSignal::new(String::new());
    let context = Memo::new(move |_| Context::from_editor(&editor.get()));
    Effect::new(move |_| {
        context.get();
        let current = editor.get_untracked();
        let previous = state.get_untracked();
        let mut next = previous.observe(&current);
        if next.context != previous.context
            && (next.context.actor != previous.context.actor
                || next.context.script != previous.context.script)
        {
            if next.context.script != previous.context.script {
                rights.set(RightsForm::default());
                rights_dirty.set(false);
                retained_rights.set(None);
            } else if let Some((context, form, changed)) =
                retained_rights.get_untracked().filter(|(context, _, _)| {
                    context.actor == next.context.actor && context.script == next.context.script
                })
            {
                let _ = context;
                rights.set(form);
                rights_dirty.set(changed);
                retained_rights.set(None);
            } else {
                if rights_dirty.get_untracked() {
                    retained_rights.set(Some((
                        previous.context.clone(),
                        rights.get_untracked(),
                        true,
                    )));
                }
                rights.set(RightsForm::default());
                rights_dirty.set(false);
            }
        }
        if rights_dirty.get_untracked() {
            next = next.rights_edited();
        }
        state.set(next);
    });
    Effect::new(move |_| {
        dirty.set(
            state.with(Production::is_dirty)
                || rights_dirty.get()
                || retained_rights.with(|r| r.is_some())
                || composing.with(|fields| !fields.is_empty()),
        );
        activity.set(
            state.with(Production::has_activity) || composing.with(|fields| !fields.is_empty()),
        );
    });
    let script_dirty = move || editor.with(Editor::is_dirty) || script_activity.get();
    let form_blocked = move || {
        session_pending.get()
            || state.with(Production::has_activity)
            || editor.with(|e| e.busy() || e.pending().is_some())
            || script_dirty()
            || !state.with(|s| s.context.ready())
    };
    let blocked = move || form_blocked() || composing.with(|fields| !fields.is_empty());
    let readonly = Signal::derive(form_blocked);
    let saved_version = move || {
        state.with(|s| {
            s.stored
                .as_ref()
                .and_then(|s| s.settings.as_ref())
                .map_or(0, |s| s.version)
        })
    };
    let review_blocked =
        move || blocked() || state.with(Production::is_dirty) || saved_version() == 0;
    let freeze_blocked = move || review_blocked() || state.with(|s| s.preview.is_none());
    let approve_blocked = move || {
        blocked()
            || state.with(|s| {
                s.is_dirty()
                    || s.confirmation.is_none()
                    || s.snapshot
                        .as_ref()
                        .is_none_or(|s| !s.eligibility.inputs_eligible || s.approval.is_some())
            })
    };

    view! { <section class="production-inputs" aria-labelledby="production-title">
        <h2 id="production-title">{move || text(Key::Pane, locale.get())}</h2>
        <p class="help">{move || text(Key::Intro, locale.get())}</p>
        <p class="production-reference">{move || text(Key::ReferenceOnly, locale.get())}</p>
        <Show when=move || !state.with(|s| s.context.ready())><p class="help">{move || text(Key::OpenSaved, locale.get())}</p></Show>
        <Show when=script_dirty><p id="production-dirty-reason" class="status">{move || text(Key::Dirty, locale.get())}</p></Show>
        <Show when=move || state.with(|s| s.context.ready())>
            <dl class="source-identity"><dt>{move || text(Key::Revision, locale.get())}</dt><dd><code>{move || state.with(|s| format!("{} · {}", s.context.script, s.context.revision))}</code></dd>
                <dt>{move || text(Key::ExportDigest, locale.get())}</dt><dd><code>{move || state.with(|s| s.context.export_digest.clone())}</code></dd>
            </dl>
        </Show>
        <button class="cui-button" data-variant="secondary" type="button" disabled=move || session_pending.get() || state.with(Production::has_activity) || composing.with(|fields| !fields.is_empty()) || !state.with(|s| s.context.ready()) on:click=move |_| {
            if session_pending.get_untracked() {return;}
            let Some((reading, ticket)) = state.get_untracked().start_read(ReadKind::State) else { return; };
            let script = editor.with_untracked(|e| e.remote().map(|r| r.script_json.clone()).unwrap_or_default());
            state.set(reading);
            let api = api.snapshot();
    spawn_local(async move {
                let result = async { let catalog = api.production_catalog().await?; let stored = api.production_state(&ticket.context.script).await?; Ok::<_,ApiError>((catalog, stored)) }.await;
                let _ = state.try_update(|s| *s = match result { Ok((catalog, stored)) => s.loaded(&ticket, catalog, stored, &script), Err(error) => s.read_failed(&ticket, error) });
            });
        }>{move || text(Key::Load, locale.get())}</button>
        <button class="cui-button" data-variant="secondary" type="button" disabled=move ||state.with(Production::has_activity)||composing.with(|f|!f.is_empty()) on:click=move |_|{
            if state.with_untracked(Production::has_activity)||composing.with_untracked(|f|!f.is_empty())||!confirm_discard(locale.get_untracked()){return;}
            if let Some(discarded)=state.get_untracked().discard_drafts(){state.set(discarded);rights.set(RightsForm::default());rights_dirty.set(false);retained_rights.set(None);}
        }>{move ||text(Key::Discard,locale.get())}</button>
        <UiStatus announcement=Announcement::Polite>{move || state.with(|s| if let Some(error) = &s.error { messages::status(crate::editor::error_status(&error.code), locale.get()) } else if s.busy() { text(Key::Busy, locale.get()) } else if s.stored.is_some() { text(Key::Ready, locale.get()) } else { "" })}</UiStatus>
        <ul class="production-findings">{move || state.with(|s|s.error.as_ref().map(|e|e.issues.clone()).unwrap_or_default()).into_iter().map(|issue|view!{<li><code>{issue.path}" · "{issue.rule}</code></li>}).collect_view()}</ul>
        <Show when=move || state.with(|s| s.pending().is_some())><section class="production-pending" aria-describedby="production-pending-help">
            <p id="production-pending-help">{move || text(Key::PendingHelp, locale.get())}</p>
            <dl class="source-identity"><dt>{move || text(Key::Actor, locale.get())}</dt><dd>{move || state.with(|s| s.pending().map(|i| i.context.actor.clone()).unwrap_or_default())}</dd>
                <dt>{move || text(Key::Operation, locale.get())}</dt><dd><code>{move || state.with(|s| s.pending().map(|i| i.command.operation_id().to_owned()).unwrap_or_default())}</code></dd></dl>
            <button class="cui-button" data-variant="secondary" type="button" disabled=move || session_pending.get() || !state.with(Production::can_retry) on:click=move |_| { if session_pending.get_untracked() { return; }
                if let Some((sending,intent)) = state.get_untracked().retry() { state.set(sending); dispatch(state,api,intent); } }>{move || text(Key::Retry,locale.get())}</button>
        </section></Show>
        <Show when=move || state.with(|s| s.form.is_some())><div class="production-layout">
            <section class="production-settings" aria-labelledby="casting-title"><h3 id="casting-title">{move || text(Key::Settings, locale.get())}</h3>
                <p>{move || text(Key::Stored,locale.get())}" · "<output>{saved_version}</output>" · "{move || if state.with(Production::is_dirty) { text(Key::Unsaved,locale.get()) } else { "" }}</p>
                <Show when=move ||state.with(Production::is_dirty)><p class="help">{move ||text(Key::Revalidate,locale.get())}</p></Show>
                <CastingFields state=state locale=locale readonly=readonly script=Signal::derive(move || editor.with(|e| e.remote().map(|r| r.script_json.clone()).unwrap_or_default())) />
                <BudgetFields state=state locale=locale readonly=readonly />
                <ActionButton id="production-settings-save" locale=locale blocked=Signal::derive(blocked) reason=BlockReason::ProductionReview described_by="production-dirty-reason" on_press=Callback::new(move |_| {
                    if blocked() { return; }
                    let Some(settings) = state.with_untracked(|s| s.form.as_ref().and_then(SettingsForm::settings)) else { state.update(|s| s.error = Some(error(ErrorCode::InvalidRequest))); return; };
                    match operation_id() { Ok(operation_id) => submit(state,api,Command::Settings(SaveProductionSettingsRequest { operation_id, expected_revision: state.with_untracked(|s| s.context.revision), expected_settings_version:saved_version(),settings })), Err(error) => state.update(|s| s.error=Some(error)) }
                })>{move || text(Key::Save,locale.get())}</ActionButton>
            </section>
            <section class="production-inspector" aria-labelledby="production-review-title"><h3 id="production-review-title">{move || text(Key::Validate,locale.get())}</h3>
                <p class="help">{move || text(Key::ValidationHelp,locale.get())}</p>
                <ActionButton id="production-review" locale=locale blocked=Signal::derive(review_blocked) reason=BlockReason::ProductionReview variant=ButtonVariant::Tonal on_press=Callback::new(move |_| {
                    if review_blocked() { return; } let Some((reading,ticket)) = state.get_untracked().start_read(ReadKind::Preview) else { return; }; state.set(reading);
                    let api = api.snapshot();
    spawn_local(async move { let result=api.production_preview(&ticket.context.script).await; let _=state.try_update(|s| *s=match result { Ok(preview)=>s.preview_loaded(&ticket,preview),Err(error)=>s.read_failed(&ticket,error) }); });
                })>{move || text(Key::Validate,locale.get())}</ActionButton>
                {move || state.with(|s| s.preview.clone()).map(|preview| view! { <dl class="source-identity"><dt>{move || text(Key::Digest,locale.get())}</dt><dd><code>{preview.input_digest}</code></dd></dl><Estimate estimate=preview.estimate locale=locale /><Findings findings=preview.findings locale=locale /> })}
                <p class="help">{move || text(Key::FreezeHelp,locale.get())}</p>
                <ActionButton id="production-freeze" locale=locale blocked=Signal::derive(freeze_blocked) reason=BlockReason::ProductionReview on_press=Callback::new(move |_| {
                    if freeze_blocked() { return; } let Some(preview)=state.with_untracked(|s| s.preview.clone()) else { return; };
                    match operation_id() { Ok(operation_id)=>submit(state,api,Command::Freeze(FreezeProductionRequest { operation_id,expected_revision:preview.script_revision,settings_version:preview.settings_version,input_digest:preview.input_digest })),Err(error)=>state.update(|s| s.error=Some(error)) }
                })>{move || text(Key::Freeze,locale.get())}</ActionButton>
            </section>
        </div>
        <RightsFields state=state rights=rights rights_dirty=rights_dirty locale=locale readonly=readonly script=Signal::derive(move || editor.with(|e| e.remote().map(|r| r.script_json.clone()).unwrap_or_default())) />
        </Show>
        <section class="production-snapshots" aria-labelledby="production-snapshot-title"><h3 id="production-snapshot-title">{move || text(Key::Snapshot,locale.get())}</h3>
            <div class="toolbar"><label for="production-snapshot-id">{move || text(Key::SnapshotId,locale.get())}</label><input class="cui-field-control" id="production-snapshot-id" autocomplete="off" spellcheck="false" prop:value=move || snapshot_id.get() on:input=move |e| snapshot_id.set(event_target_value(&e)) />
                <button class="cui-button" data-variant="secondary" type="button" disabled=move || state.with(Production::has_activity) || composing.with(|fields| !fields.is_empty()) || !state.with(|s| s.context.ready()) on:click=move |_| read_snapshot(state,api,snapshot_id.get_untracked())>{move || text(Key::Inspect,locale.get())}</button></div>
            <div class="toolbar">{move || state.with(|s| s.stored.as_ref().map(|s| s.snapshots.clone()).unwrap_or_default()).into_iter().map(|snapshot| { let id=StoredValue::new(snapshot.id.clone()); view! { <button class="cui-button" data-variant="secondary" type="button" disabled=move || state.with(Production::has_activity) || composing.with(|fields| !fields.is_empty()) on:click=move |_| { snapshot_id.set(id.get_value());read_snapshot(state,api,id.get_value()); }><code>{snapshot.id}</code>" · "{snapshot.script_revision}</button> } }).collect_view()}</div>
            {move || state.with(|s| s.snapshot.clone()).map(|snapshot| view! { <SnapshotDetails snapshot=snapshot locale=locale /> })}
            <Show when=move || state.with(|s| s.snapshot.is_some())><crate::ui::ReviewCheckbox id="production-confirm" locale=locale reason=BlockReason::ProductionReview
                label=Signal::derive(move ||text(Key::ConfirmApproval,locale.get()).to_owned())
                checked=Signal::derive(move ||state.with(|s|s.confirmation.is_some()))
                blocked=Signal::derive(move ||blocked() || state.with(Production::is_dirty))
                described_by=Some("production-approval-help".to_owned())
                on_change=Callback::new(move |checked|state.update(|s|*s=s.confirm(checked)))/>
                <p id="production-approval-help" class="help">{move || text(Key::ApprovalHelp,locale.get())}</p>
                <ActionButton id="production-approve" locale=locale blocked=Signal::derive(approve_blocked) reason=BlockReason::ProductionReview described_by="production-approval-help" on_press=Callback::new(move |_| {
                    if approve_blocked() { return; } let Some(snapshot)=state.with_untracked(|s| s.snapshot.clone()) else { return; };
                    match operation_id() { Ok(operation_id)=>submit(state,api,Command::Approve { snapshot_id:snapshot.id,request:ApproveProductionRequest { operation_id,input_digest:snapshot.document.input_digest } }),Err(error)=>state.update(|s| s.error=Some(error)) }
                })>{move || text(Key::Approve,locale.get())}</ActionButton>
            </Show>
        </section>
    </section> }
}

#[component]
fn FormField(
    id: String,
    label: Key,
    value: Signal<String>,
    change: Callback<String>,
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
    #[prop(default = "text")] kind: &'static str,
    #[prop(optional, into)] min: MaybeProp<String>,
    #[prop(optional, into)] max: MaybeProp<String>,
    #[prop(optional, into)] unsupported: MaybeProp<bool>,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Input>::new();
    let initial = value.get_untracked();
    let composing = RwSignal::new(false);
    let typing_context = expect_context::<ProductionTyping>();
    let typing = typing_context.0;
    let identity = StoredValue::new(id.clone());
    Effect::new(move |_| {
        let current = value.get();
        if !composing.get() {
            if let Some(node) = node.get() {
                if node.value() != current {
                    node.set_value(&current);
                }
            }
        }
    });
    on_cleanup(move || {
        let _ = typing.try_update(|fields| {
            fields.remove(&identity.get_value());
        });
        if let Some(active) = typing.try_with_untracked(|fields| !fields.is_empty()) {
            let _ = typing_context.1.try_update(|s| s.input_composing = active);
        }
    });
    let flagged = move || unsupported.get().unwrap_or(false);
    let note = format!("{id}-unsupported");
    let label_id = format!("{id}-label");
    let labelled_by = label_id.clone();
    view! { <div class="production-field"><FieldLabel for_id=id.clone()><span id=label_id.clone()>{move || text(label,locale.get())}</span></FieldLabel>
        <input class="cui-field-control" id=id.clone() node_ref=node type=kind min=move || min.get() max=move || max.get() step="1" autocomplete="off" disabled=move || readonly.get() prop:value=initial
            aria-labelledby=labelled_by
            aria-invalid=move || flagged().then_some("true") aria-describedby={let note=note.clone(); move || flagged().then(|| note.clone())}
            on:input=move |e| { if !readonly.get_untracked() && !composing.get_untracked() { change.run(event_target_value(&e)); } }
            on:compositionstart=move |_|{if readonly.get_untracked(){return;}composing.set(true);typing.update(|fields|{fields.insert(identity.get_value());});typing_context.1.update(|s|s.input_composing=true);}
            on:compositionend=move |e|{if !readonly.get_untracked(){change.run(event_target_value(&e));}composing.set(false);typing.update(|fields|{fields.remove(&identity.get_value());});typing_context.1.update(|s|s.input_composing=typing.with_untracked(|fields|!fields.is_empty()));} />
        <Show when=flagged><small id=note.clone() class="production-unsupported">{move || text(Key::UnsupportedControl,locale.get())}</small></Show>
    </div> }
}

#[component]
fn FormSelect(
    id: String,
    label: Key,
    value: Signal<String>,
    options: Signal<Vec<(String, String)>>,
    change: Callback<String>,
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
    #[prop(optional, into)] unsupported: MaybeProp<bool>,
    #[prop(default = Key::UnsupportedControl)] note: Key,
) -> impl IntoView {
    view! { <ContextSelect id=id label=Signal::derive(move || text(label, locale.get()).to_owned())
    value=value options=options unknown_label=Signal::derive(move || text(Key::UnsupportedChoice, locale.get()).to_owned())
    error=Signal::derive(move || unsupported.get().unwrap_or(false).then(|| {
        let current = value.get();
        if current.is_empty() { text(note, locale.get()).to_owned() }
        else { format!("{current} · {}", text(note, locale.get())) }
    }))
    on_change=change locale=locale readonly=readonly /> }
}

#[derive(Clone, Copy)]
enum BindingField {
    Provider,
    Model,
    Voice,
    Language,
    Rights,
    Rate,
    Pitch,
    Emotion,
    Intensity,
}

fn binding_value(binding: &BindingForm, field: BindingField) -> String {
    match field {
        BindingField::Provider => &binding.provider_id,
        BindingField::Model => &binding.model_id,
        BindingField::Voice => &binding.voice_id,
        BindingField::Language => &binding.language,
        BindingField::Rights => &binding.voice_rights_record_id,
        BindingField::Rate => &binding.rate,
        BindingField::Pitch => &binding.pitch,
        BindingField::Emotion => &binding.emotion,
        BindingField::Intensity => &binding.intensity,
    }
    .clone()
}

fn set_binding(binding: &mut BindingForm, field: BindingField, value: String) {
    let target = match field {
        BindingField::Provider => &mut binding.provider_id,
        BindingField::Model => &mut binding.model_id,
        BindingField::Voice => &mut binding.voice_id,
        BindingField::Language => &mut binding.language,
        BindingField::Rights => &mut binding.voice_rights_record_id,
        BindingField::Rate => &mut binding.rate,
        BindingField::Pitch => &mut binding.pitch,
        BindingField::Emotion => &mut binding.emotion,
        BindingField::Intensity => &mut binding.intensity,
    };
    *target = value;
}

#[component]
fn CastingFields(
    state: RwSignal<Production>,
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
    script: Signal<String>,
) -> impl IntoView {
    view! { <For each=move || state.with(|s|s.form.as_ref().map(|f|f.bindings.iter().map(|b|b.character_id.clone()).collect::<Vec<_>>()).unwrap_or_default()) key=|id|id.clone() let:id>
        <CastingCharacter state=state locale=locale readonly=readonly script=script character_id=id />
    </For> }
}

#[component]
fn CastingCharacter(
    state: RwSignal<Production>,
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
    script: Signal<String>,
    character_id: String,
) -> impl IntoView {
    let id = StoredValue::new(character_id);
    let binding = Signal::derive(move || {
        state.with(|s| {
            s.form
                .as_ref()
                .and_then(|f| f.bindings.iter().find(|b| b.character_id == id.get_value()))
                .cloned()
        })
    });
    let capability = Memo::new(move |_| {
        let binding = binding.get()?;
        state.with(|s| {
            s.catalog
                .as_ref()
                .and_then(|c| binding.capability(c).cloned())
        })
    });
    let unsupported = Memo::new(move |_| {
        binding
            .get()
            .map(|b| b.unsupported(capability.get().as_ref()))
            .unwrap_or_default()
    });
    let flag = move |control: CastingControl| {
        Signal::derive(move || unsupported.with(|u| u.contains(&control)))
    };
    // A profile without an intensity control admits only "inherit" or an explicit 0.
    let fixed_delivery =
        Memo::new(move |_| capability.get().is_some_and(|c| !c.intensity_supported));
    let name = move || {
        Document::parse(&script.get())
            .ok()
            .and_then(|d| {
                d.characters()
                    .into_iter()
                    .find(|c| c.id == id.get_value())
                    .map(|c| c.name)
            })
            .unwrap_or_else(|| id.get_value())
    };
    let value = move |field| {
        Signal::derive(move || {
            binding
                .get()
                .map(|b| binding_value(&b, field))
                .unwrap_or_default()
        })
    };
    let change = move |field| {
        Callback::new(move |value: String| {
            state.update(|s| {
                *s = s.edit(|f| {
                    if let Some(binding) = f
                        .bindings
                        .iter_mut()
                        .find(|b| b.character_id == id.get_value())
                    {
                        set_binding(binding, field, value);
                    }
                })
            })
        })
    };
    let prefix = format!("production-{}", id.get_value());
    let providers = Signal::derive(move || {
        state.with(|s| {
            s.catalog
                .as_ref()
                .map(|c| {
                    c.capabilities
                        .iter()
                        .map(|c| (c.provider_id.clone(), c.provider_id.clone()))
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect()
                })
                .unwrap_or_default()
        })
    });
    let models = Signal::derive(move || {
        let mut models = vec![(String::new(), text(Key::ChooseModel, locale.get()).into())];
        state.with(|s| {
            if let Some(c) = &s.catalog {
                models.extend(
                    c.capabilities
                        .iter()
                        .filter(|c| {
                            binding
                                .get()
                                .is_some_and(|b| b.provider_id == c.provider_id)
                        })
                        .map(|c| (c.model_id.clone(), c.model_id.clone())),
                );
            }
        });
        models
    });
    let select_provider = Callback::new(move |provider: String| {
        state.update(|s| {
            let Some(catalog) = s.catalog.clone() else {
                return;
            };
            *s = s.edit(|f| {
                if let Some(binding) = f
                    .bindings
                    .iter_mut()
                    .find(|b| b.character_id == id.get_value())
                {
                    binding.select_provider(&catalog, provider);
                }
            })
        })
    });
    let intensities = Signal::derive(move || {
        vec![
            (String::new(), text(Key::Inherit, locale.get()).into()),
            ("0".into(), text(Key::DefaultIntensity, locale.get()).into()),
        ]
    });
    let range = move |bound: fn(&ProductionCapability) -> String| {
        Signal::derive(move || capability.get().as_ref().map(bound))
    };
    let voices = Signal::derive(move || {
        let mut voices = vec![(String::new(), text(Key::ChooseVoice, locale.get()).into())];
        if let Some(c) = capability.get() {
            voices.extend(c.voice_ids.into_iter().map(|id| (id.clone(), id)));
        }
        voices
    });
    let languages = Signal::derive(move || {
        capability
            .get()
            .map(|c| c.languages.into_iter().map(|l| (l.clone(), l)).collect())
            .unwrap_or_default()
    });
    let emotions = Signal::derive(move || {
        let mut emotions = vec![(String::new(), text(Key::Inherit, locale.get()).into())];
        if let Some(c) = capability.get() {
            emotions.extend(c.emotions.into_iter().map(|e| {
                (
                    e.clone(),
                    messages::authoring::choice(&e, locale.get()).into(),
                )
            }));
        }
        emotions
    });
    view! { <fieldset class="production-character" disabled=move ||readonly.get()><legend>{name}<code>{id.get_value()}</code></legend>
        <div class="production-fields">
            <FormSelect id=format!("{prefix}-provider") label=Key::Provider value=value(BindingField::Provider) options=providers change=select_provider locale=locale readonly=readonly />
            <FormSelect id=format!("{prefix}-model") label=Key::Model value=value(BindingField::Model) options=models change=change(BindingField::Model) locale=locale readonly=readonly unsupported=flag(CastingControl::Model) note=Key::ModelRequired />
            <FormSelect id=format!("{prefix}-voice") label=Key::Voice value=value(BindingField::Voice) options=voices change=change(BindingField::Voice) locale=locale readonly=readonly unsupported=flag(CastingControl::Voice) />
            <FormSelect id=format!("{prefix}-language") label=Key::Language value=value(BindingField::Language) options=languages change=change(BindingField::Language) locale=locale readonly=readonly unsupported=flag(CastingControl::Language) />
            <FormField id=format!("{prefix}-rights") label=Key::VoiceRights value=value(BindingField::Rights) change=change(BindingField::Rights) locale=locale readonly=readonly />
            <FormField id=format!("{prefix}-rate") label=Key::Rate value=value(BindingField::Rate) change=change(BindingField::Rate) locale=locale readonly=readonly kind="number" min=range(|c|c.rate_min_permille.to_string()) max=range(|c|c.rate_max_permille.to_string()) unsupported=flag(CastingControl::Rate) />
            <FormField id=format!("{prefix}-pitch") label=Key::Pitch value=value(BindingField::Pitch) change=change(BindingField::Pitch) locale=locale readonly=readonly kind="number" min=range(|c|c.pitch_min_semitones.to_string()) max=range(|c|c.pitch_max_semitones.to_string()) unsupported=flag(CastingControl::Pitch) />
            <FormSelect id=format!("{prefix}-emotion") label=Key::Emotion value=value(BindingField::Emotion) options=emotions change=change(BindingField::Emotion) locale=locale readonly=readonly unsupported=flag(CastingControl::Emotion) />
            {move || if fixed_delivery.get() {
                view! { <FormSelect id=format!("{prefix}-intensity") label=Key::Intensity value=value(BindingField::Intensity) options=intensities change=change(BindingField::Intensity) locale=locale readonly=readonly unsupported=flag(CastingControl::Intensity) /> }.into_any()
            } else {
                view! { <FormField id=format!("{prefix}-intensity") label=Key::Intensity value=value(BindingField::Intensity) change=change(BindingField::Intensity) locale=locale readonly=Signal::derive(move ||readonly.get()||capability.get().is_none()) kind="number" min="0" max="1000" unsupported=flag(CastingControl::Intensity) /> }.into_any()
            }}
        </div>
        <Show when=move || fixed_delivery.get()><p class="help">{move ||text(Key::FixedDelivery,locale.get())}</p></Show>
        <h4>{move ||text(Key::Pronunciation,locale.get())}</h4>
        <Show when=move || flag(CastingControl::Pronunciation).get()><p class="production-unsupported">{move ||text(Key::UnsupportedControl,locale.get())}</p></Show>
        <For each=move || binding.get().map(|b|(0..b.pronunciation.len()).collect::<Vec<_>>()).unwrap_or_default() key=|i|*i let:index>
            <div class="production-pronunciation">
                <FormField id=format!("production-{}-pronunciation-{index}-surface",id.get_value()) label=Key::Surface value=Signal::derive(move ||binding.get().and_then(|b|b.pronunciation.get(index).map(|p|p.surface.clone())).unwrap_or_default()) change=Callback::new(move |value|state.update(|s|*s=s.edit(|f|if let Some(p)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()).and_then(|b|b.pronunciation.get_mut(index)){p.surface=value;}))) locale=locale readonly=readonly />
                <FormField id=format!("production-{}-pronunciation-{index}-replacement",id.get_value()) label=Key::Replacement value=Signal::derive(move ||binding.get().and_then(|b|b.pronunciation.get(index).map(|p|p.replacement.clone())).unwrap_or_default()) change=Callback::new(move |value|state.update(|s|*s=s.edit(|f|if let Some(p)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()).and_then(|b|b.pronunciation.get_mut(index)){p.replacement=value;}))) locale=locale readonly=readonly />
                <button class="cui-button" data-variant="secondary" type="button" disabled=move ||readonly.get() on:click=move |_|{if !readonly.get_untracked(){state.update(|s|*s=s.edit(|f|if let Some(b)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()){if index<b.pronunciation.len(){b.pronunciation.remove(index);}}));if let Some(element)=leptos::leptos_dom::helpers::document().get_element_by_id(&format!("production-{}-pronunciation-add",id.get_value())).and_then(|e|e.dyn_into::<web_sys::HtmlElement>().ok()){let _=element.focus();}}}>{move ||text(Key::Remove,locale.get())}</button>
            </div>
        </For>
        <button class="cui-button" data-variant="secondary" id=format!("production-{}-pronunciation-add",id.get_value()) type="button" disabled=move ||readonly.get()||capability.get().is_none_or(|c|!c.pronunciation_supported) on:click=move |_|{if !readonly.get_untracked()&&capability.get_untracked().is_some_and(|c|c.pronunciation_supported){state.update(|s|*s=s.edit(|f|if let Some(b)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()){b.pronunciation.push(ProductionPronunciation{surface:String::new(),replacement:String::new()});}));}}>{move ||text(Key::AddPronunciation,locale.get())}</button>
    </fieldset> }
}

#[derive(Clone, Copy)]
enum BudgetField {
    Limit,
    Scope,
    Territory,
    Reference,
    Version,
    Units,
    Amount,
}

#[component]
fn BudgetFields(
    state: RwSignal<Production>,
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
) -> impl IntoView {
    let value = move |field| {
        Signal::derive(move || {
            state.with(|s| {
                s.form
                    .as_ref()
                    .map(|f| {
                        match field {
                            BudgetField::Limit => &f.limit,
                            BudgetField::Scope => &f.scope,
                            BudgetField::Territory => &f.territory,
                            BudgetField::Reference => &f.rate_reference,
                            BudgetField::Version => &f.rate_version,
                            BudgetField::Units => &f.rate_units,
                            BudgetField::Amount => &f.rate_amount,
                        }
                        .clone()
                    })
                    .unwrap_or_default()
            })
        })
    };
    let change = move |field| {
        Callback::new(move |value| {
            state.update(|s| {
                *s = s.edit(|f| {
                    let target = match field {
                        BudgetField::Limit => &mut f.limit,
                        BudgetField::Scope => &mut f.scope,
                        BudgetField::Territory => &mut f.territory,
                        BudgetField::Reference => &mut f.rate_reference,
                        BudgetField::Version => &mut f.rate_version,
                        BudgetField::Units => &mut f.rate_units,
                        BudgetField::Amount => &mut f.rate_amount,
                    };
                    *target = value;
                })
            })
        })
    };
    view! {<fieldset class="production-budget" disabled=move ||readonly.get()><legend>{move ||text(Key::Budget,locale.get())}</legend><div class="production-fields">
        <FormSelect id="production-currency".into() label=Key::Currency value=Signal::derive(move ||state.with(|s|s.form.as_ref().map(|f|format!("{:?}",f.currency)).unwrap_or_default())) options=Signal::derive(||vec![("USD".into(),"USD".into()),("VND".into(),"VND".into())]) change=Callback::new(move |value:String|state.update(|s|*s=s.edit(|f|f.currency=if value=="VND"{ProductionCurrency::VND}else{ProductionCurrency::USD}))) locale=locale readonly=readonly />
        <FormField id="production-limit".into() label=Key::Limit value=value(BudgetField::Limit) change=change(BudgetField::Limit) locale=locale readonly=readonly kind="number" min="0" />
        <FormField id="production-scope".into() label=Key::Scope value=value(BudgetField::Scope) change=change(BudgetField::Scope) locale=locale readonly=readonly />
        <FormField id="production-territory".into() label=Key::Territory value=value(BudgetField::Territory) change=change(BudgetField::Territory) locale=locale readonly=readonly />
    </div><crate::ui::ReviewCheckbox id="production-known-rate" locale=locale reason=BlockReason::ReadOnly
        label=Signal::derive(move ||text(Key::KnownRate,locale.get()).to_owned())
        checked=Signal::derive(move ||state.with(|s|s.form.as_ref().is_some_and(|f|f.known_rate))) blocked=readonly
        on_change=Callback::new(move |checked|{if !readonly.get_untracked(){state.update(|s|*s=s.edit(|f|f.known_rate=checked));}})/>
    <Show when=move ||state.with(|s|s.form.as_ref().is_some_and(|f|f.known_rate))><div class="production-fields">
        <FormField id="production-rate-reference".into() label=Key::RateReference value=value(BudgetField::Reference) change=change(BudgetField::Reference) locale=locale readonly=readonly />
        <FormField id="production-rate-version".into() label=Key::RateVersion value=value(BudgetField::Version) change=change(BudgetField::Version) locale=locale readonly=readonly />
        <FormField id="production-rate-units".into() label=Key::RateUnits value=value(BudgetField::Units) change=change(BudgetField::Units) locale=locale readonly=readonly kind="number" min="1" />
        <FormField id="production-rate-amount".into() label=Key::RateAmount value=value(BudgetField::Amount) change=change(BudgetField::Amount) locale=locale readonly=readonly kind="number" min="0" />
    </div></Show></fieldset>}
}

#[derive(Clone, Copy)]
enum RightsField {
    Record,
    Reference,
    From,
    Until,
    Holder,
    Languages,
    Territory,
    Attribution,
    Restrictions,
    Scope,
}

#[component]
fn RightsFields(
    state: RwSignal<Production>,
    rights: RwSignal<RightsForm>,
    rights_dirty: RwSignal<bool>,
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
    script: Signal<String>,
) -> impl IntoView {
    let api = expect_context::<StudioContext>().api;
    let typing = expect_context::<ProductionTyping>().0;
    let mutation_blocked = move || readonly.get() || typing.with(|fields| !fields.is_empty());
    Effect::new(move |_| {
        let declaration = state.with(|s| {
            s.form
                .as_ref()
                .and_then(|form| rights.get().declaration(form))
        });
        if declaration.is_some_and(|declaration| {
            state.with(|s| {
                s.stored.as_ref().is_some_and(|stored| {
                    stored
                        .rights
                        .iter()
                        .any(|r| r.claim.declaration == declaration)
                })
            })
        }) {
            rights_dirty.set(false);
            if state.with_untracked(|s| s.rights_draft_dirty) {
                state.update(|s| *s = s.rights_reconciled());
            }
        }
    });
    let value = move |field| {
        Signal::derive(move || {
            rights.with(|f| {
                match field {
                    RightsField::Record => &f.record_id,
                    RightsField::Reference => &f.reference,
                    RightsField::From => &f.from,
                    RightsField::Until => &f.until,
                    RightsField::Holder => &f.holder,
                    RightsField::Languages => &f.languages,
                    RightsField::Territory => &f.territory,
                    RightsField::Attribution => &f.attribution,
                    RightsField::Restrictions => &f.restrictions,
                    RightsField::Scope => &f.permitted_scope,
                }
                .clone()
            })
        })
    };
    let change = move |field| {
        Callback::new(move |value| {
            rights.update(|f| {
                let target = match field {
                    RightsField::Record => &mut f.record_id,
                    RightsField::Reference => &mut f.reference,
                    RightsField::From => &mut f.from,
                    RightsField::Until => &mut f.until,
                    RightsField::Holder => &mut f.holder,
                    RightsField::Languages => &mut f.languages,
                    RightsField::Territory => &mut f.territory,
                    RightsField::Attribution => &mut f.attribution,
                    RightsField::Restrictions => &mut f.restrictions,
                    RightsField::Scope => &mut f.permitted_scope,
                };
                *target = value;
            });
            state.update(|s| *s = s.rights_edited());
            rights_dirty.set(true);
        })
    };
    let options = Signal::derive(move || {
        let mut options = vec![(String::new(), text(Key::Subject, locale.get()).into())];
        options.extend(evidence_ids(&script.get()).into_iter().map(|id| {
            (
                format!("evidence:{id}"),
                format!("{} · {id}", text(Key::Evidence, locale.get())),
            )
        }));
        options.extend(state.with(|s| {
            s.form
                .as_ref()
                .map(|f| {
                    f.bindings
                        .iter()
                        .filter(|b| !b.voice_id.is_empty())
                        .map(|b| {
                            (
                                format!("voice:{}", b.character_id),
                                format!(
                                    "{} · {} · {}",
                                    text(Key::VoiceSubject, locale.get()),
                                    b.character_id,
                                    b.voice_id
                                ),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }));
        options
    });
    view! {<section class="production-rights" aria-labelledby="production-rights-title"><h3 id="production-rights-title">{move ||text(Key::Rights,locale.get())}</h3><p class="help">{move ||text(Key::RightsHelp,locale.get())}</p>
        <ul class="production-rights-list">{move ||state.with(|s|s.stored.as_ref().map(|s|s.rights.clone()).unwrap_or_default()).into_iter().map(|r|view!{<li><strong>{r.claim.declaration.record_id}</strong>" · "{move ||messages::production::rights_status(r.claim.declaration.status,locale.get())}" · "{r.claim.version}<small>{r.recorded_by}" · "{r.recorded_at}" · "{r.claim.declaration.reference}</small></li>}).collect_view()}</ul>
        <fieldset disabled=move ||readonly.get()><legend>{move ||text(Key::SaveRights,locale.get())}</legend><div class="production-fields">
            <FormSelect id="production-rights-subject".into() label=Key::Subject value=Signal::derive(move ||rights.with(|r|r.subject.clone())) options=options change=Callback::new(move |value:String|{let record=if let Some(id)=value.strip_prefix("evidence:"){id.to_owned()}else{state.with_untracked(|s|s.form.as_ref().and_then(|f|f.bindings.iter().find(|b|Some(b.character_id.as_str())==value.strip_prefix("voice:")).map(|b|b.voice_rights_record_id.clone())).unwrap_or_default())};rights.update(|r|{r.subject=value;r.record_id=record;});state.update(|s| *s = s.rights_edited()); rights_dirty.set(true);}) locale=locale readonly=readonly />
            <FormField id="production-rights-record".into() label=Key::RecordId value=value(RightsField::Record) change=change(RightsField::Record) locale=locale readonly=readonly />
            <FormSelect id="production-rights-status".into() label=Key::ClaimStatus value=Signal::derive(move ||rights.with(|r|match r.status{ProductionRightsStatus::Pending=>"pending",ProductionRightsStatus::Granted=>"granted",ProductionRightsStatus::Revoked=>"revoked"}.into())) options=Signal::derive(move ||vec![("pending".into(),text(Key::Pending,locale.get()).into()),("granted".into(),text(Key::Granted,locale.get()).into()),("revoked".into(),text(Key::Revoked,locale.get()).into())]) change=Callback::new(move |value:String|{rights.update(|r|r.status=match value.as_str(){"granted"=>ProductionRightsStatus::Granted,"revoked"=>ProductionRightsStatus::Revoked,_=>ProductionRightsStatus::Pending});state.update(|s| *s = s.rights_edited()); rights_dirty.set(true);}) locale=locale readonly=readonly />
            <FormField id="production-rights-reference".into() label=Key::Reference value=value(RightsField::Reference) change=change(RightsField::Reference) locale=locale readonly=readonly />
            <FormField id="production-rights-holder".into() label=Key::Holder value=value(RightsField::Holder) change=change(RightsField::Holder) locale=locale readonly=readonly />
            <FormField id="production-rights-languages".into() label=Key::RightsLanguages value=value(RightsField::Languages) change=change(RightsField::Languages) locale=locale readonly=readonly />
            <FormField id="production-rights-territory".into() label=Key::Territory value=value(RightsField::Territory) change=change(RightsField::Territory) locale=locale readonly=readonly />
            <FormField id="production-rights-scope".into() label=Key::PermittedScope value=value(RightsField::Scope) change=change(RightsField::Scope) locale=locale readonly=readonly />
            <FormField id="production-rights-attribution".into() label=Key::Attribution value=value(RightsField::Attribution) change=change(RightsField::Attribution) locale=locale readonly=readonly />
            <FormField id="production-rights-restrictions".into() label=Key::Restrictions value=value(RightsField::Restrictions) change=change(RightsField::Restrictions) locale=locale readonly=readonly />
            <FormField id="production-rights-from".into() label=Key::ValidFrom value=value(RightsField::From) change=change(RightsField::From) locale=locale readonly=readonly kind="number" />
            <FormField id="production-rights-until".into() label=Key::ValidUntil value=value(RightsField::Until) change=change(RightsField::Until) locale=locale readonly=readonly kind="number" />
        </div><p class="help">{move ||text(Key::TimeHelp,locale.get())}</p><p class="help">{move ||text(Key::RestrictionsHelp,locale.get())}</p><button class="cui-button" data-variant="secondary" type="button" disabled=mutation_blocked on:click=move |_|{
            if mutation_blocked(){return;}let current=state.get_untracked();let Some(claim)=current.form.as_ref().and_then(|form|rights.get_untracked().declaration(form))else{state.update(|s|s.error=Some(error(ErrorCode::InvalidRequest)));return;};
            let expected_version=current.stored.as_ref().and_then(|s|s.rights.iter().find(|r|r.claim.declaration.record_id==claim.record_id).map(|r|r.claim.version)).unwrap_or(0);
            match operation_id(){Ok(operation_id)=>{submit(state,api,Command::Rights(SaveProductionRightsRequest{operation_id,expected_version,claim}));},Err(error)=>state.update(|s|s.error=Some(error))}
        }>{move ||text(Key::SaveRights,locale.get())}</button></fieldset>
    </section>}
}

#[component]
fn SnapshotDetails(
    snapshot: ProductionSnapshotResponse,
    locale: Signal<crate::localization::Locale>,
) -> impl IntoView {
    let document = snapshot.document;
    view! { <article class="production-snapshot"><dl class="source-identity">
        <dt>{move || text(Key::SnapshotId,locale.get())}</dt><dd><code>{snapshot.id}</code></dd>
        <dt>{move || text(Key::Revision,locale.get())}</dt><dd>{document.revision.revision}</dd>
        <dt>{move || text(Key::Stored,locale.get())}</dt><dd>{document.settings.version}</dd>
        <dt>{move || text(Key::Digest,locale.get())}</dt><dd><code>{document.input_digest}</code></dd>
        <dt>{move || text(Key::Actor,locale.get())}</dt><dd>{snapshot.created_by}</dd>
        <dt>{move || text(Key::RecordedAt,locale.get())}</dt><dd>{snapshot.recorded_at}</dd>
        <dt>{move || text(Key::Eligibility,locale.get())}</dt><dd>{move || if snapshot.eligibility.inputs_eligible { text(Key::Eligible,locale.get()) } else { text(Key::Blocked,locale.get()) }}</dd>
        <dt>{move || text(Key::CurrentApproval,locale.get())}</dt><dd>{move || if snapshot.eligibility.approval_current { text(Key::ApprovalCurrent,locale.get()) } else { text(Key::ApprovalNotCurrent,locale.get()) }}</dd>
        <dt>{move || text(Key::Limit,locale.get())}</dt><dd>{move ||format_money(document.settings.settings.budget.currency,document.settings.settings.budget.limit_minor,locale.get())}</dd>
        <dt>{move || text(Key::Scope,locale.get())}</dt><dd>{document.settings.settings.budget.scope}</dd>
        <dt>{move || text(Key::Territory,locale.get())}</dt><dd>{document.settings.settings.budget.territory}</dd>
        <dt>{move || text(Key::Approval,locale.get())}</dt><dd>{snapshot.approval.map(|a| format!("{} · {} · {}", a.approved_by,a.approved_at,a.input_digest)).unwrap_or_else(||text(Key::NoApproval,locale.get_untracked()).into())}</dd>
    </dl><Estimate estimate=document.estimate locale=locale /><Findings findings=snapshot.eligibility.findings locale=locale />
    <details><summary>{move || text(Key::Settings,locale.get())}</summary><pre>{serde_json::to_string_pretty(&document.resolved).unwrap_or_default()}</pre></details>
    <details><summary>{move || text(Key::Rights,locale.get())}</summary><pre>{serde_json::to_string_pretty(&document.rights).unwrap_or_default()}</pre></details>
    </article> }
}
