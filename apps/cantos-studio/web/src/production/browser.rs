//! Native Leptos shell over immutable, actor-bound production commands.
use super::*;
use crate::{
    api::{StudioContext, StudioHttp},
    messages::{
        self,
        production::{text, Key},
    },
    view::{confirm_discard, operation_id},
};
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;

#[derive(Clone, Copy)]
struct ProductionTyping(
    RwSignal<std::collections::BTreeSet<String>>,
    RwSignal<Production>,
);

fn dispatch(state: RwSignal<Production>, api: StudioHttp, intent: Intent) {
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
            .try_with_untracked(|s| {
                s.pending.as_ref() == Some(&intent) && s.context == intent.context
            })
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
                        s.context == intent.context && s.pending.is_none() && s.error.is_none()
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
fn Findings(findings: Vec<ProductionFinding>, english: RwSignal<bool>) -> impl IntoView {
    view! { <div class="production-findings"><h4>{move || text(Key::Findings, english.get())}</h4>
        {if findings.is_empty() { view! { <p>{move || text(Key::NoFindings, english.get())}</p> }.into_any() }
        else { view! { <ul>{findings.into_iter().map(|finding| view! { <li><strong>{move || messages::production::finding(finding.code, english.get())}</strong><small><code>{finding.path}</code></small></li> }).collect_view()}</ul> }.into_any() }}
    </div> }
}

#[component]
fn Estimate(estimate: ProductionEstimate, english: RwSignal<bool>) -> impl IntoView {
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
    view! { <dl class="source-identity production-cost"><dt>{move || text(Key::Estimate, english.get())}</dt><dd>{move || value.as_ref().map(|(currency,minor,units,reference,version)|format!("{} · {units} {} · {reference} / {version}",format_money(*currency,*minor,english.get()),text(Key::UnicodeUnits,english.get()))).unwrap_or_else(|| text(Key::UnknownEstimate, english.get()).into())}</dd>
        <dt>{move || text(Key::Reservation, english.get())}</dt><dd>{move || text(Key::NoReservation, english.get())}</dd>
        <dt>{move || text(Key::Actual, english.get())}</dt><dd>{move || text(Key::NoActual, english.get())}</dd>
    </dl> }
}

#[component]
pub fn ProductionInputs(
    editor: RwSignal<Editor>,
    english: RwSignal<bool>,
    script_activity: RwSignal<bool>,
    dirty: RwSignal<bool>,
    activity: RwSignal<bool>,
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
        state.with(Production::has_activity)
            || editor.with(|e| e.busy || e.pending.is_some())
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
        <h2 id="production-title">{move || text(Key::Pane, english.get())}</h2>
        <p class="help">{move || text(Key::Intro, english.get())}</p>
        <p class="production-reference">{move || text(Key::ReferenceOnly, english.get())}</p>
        <Show when=move || !state.with(|s| s.context.ready())><p class="help">{move || text(Key::OpenSaved, english.get())}</p></Show>
        <Show when=script_dirty><p id="production-dirty-reason" class="status">{move || text(Key::Dirty, english.get())}</p></Show>
        <dl class="source-identity"><dt>{move || text(Key::Revision, english.get())}</dt><dd><code>{move || state.with(|s| format!("{} · {}", s.context.script, s.context.revision))}</code></dd>
            <dt>{move || text(Key::ExportDigest, english.get())}</dt><dd><code>{move || state.with(|s| s.context.export_digest.clone())}</code></dd>
        </dl>
        <button type="button" aria-disabled=move || state.with(Production::has_activity) || composing.with(|fields| !fields.is_empty()) || !state.with(|s| s.context.ready()) on:click=move |_| {
            let Some((reading, ticket)) = state.get_untracked().start_read(ReadKind::State) else { return; };
            let script = editor.with_untracked(|e| e.remote.as_ref().map(|r| r.script_json.clone()).unwrap_or_default());
            state.set(reading);
            spawn_local(async move {
                let result = async { let catalog = api.production_catalog().await?; let stored = api.production_state(&ticket.context.script).await?; Ok::<_,ApiError>((catalog, stored)) }.await;
                let _ = state.try_update(|s| *s = match result { Ok((catalog, stored)) => s.loaded(&ticket, catalog, stored, &script), Err(error) => s.read_failed(&ticket, error) });
            });
        }>{move || text(Key::Load, english.get())}</button>
        <button type="button" aria-disabled=move ||state.with(Production::has_activity)||composing.with(|f|!f.is_empty()) on:click=move |_|{
            if state.with_untracked(Production::has_activity)||composing.with_untracked(|f|!f.is_empty())||!confirm_discard(english.get_untracked()){return;}
            if let Some(discarded)=state.get_untracked().discard_drafts(){state.set(discarded);rights.set(RightsForm::default());rights_dirty.set(false);retained_rights.set(None);}
        }>{move ||text(Key::Discard,english.get())}</button>
        <p class="status" role="status" aria-live="polite">{move || state.with(|s| if let Some(error) = &s.error { messages::status(crate::editor::error_status(&error.code), english.get()) } else if s.busy() { text(Key::Busy, english.get()) } else if s.stored.is_some() { text(Key::Ready, english.get()) } else { "" })}</p>
        <ul class="production-findings">{move || state.with(|s|s.error.as_ref().map(|e|e.issues.clone()).unwrap_or_default()).into_iter().map(|issue|view!{<li><code>{issue.path}" · "{issue.rule}</code></li>}).collect_view()}</ul>
        <Show when=move || state.with(|s| s.pending.is_some())><section class="production-pending" aria-describedby="production-pending-help">
            <p id="production-pending-help">{move || text(Key::PendingHelp, english.get())}</p>
            <dl class="source-identity"><dt>{move || text(Key::Actor, english.get())}</dt><dd>{move || state.with(|s| s.pending.as_ref().map(|i| i.context.actor.clone()).unwrap_or_default())}</dd>
                <dt>{move || text(Key::Operation, english.get())}</dt><dd><code>{move || state.with(|s| s.pending.as_ref().map(|i| i.command.operation_id().to_owned()).unwrap_or_default())}</code></dd></dl>
            <button type="button" aria-disabled=move || !state.with(Production::can_retry) on:click=move |_| { if let Some((sending,intent)) = state.get_untracked().retry() { state.set(sending); dispatch(state,api,intent); } }>{move || text(Key::Retry,english.get())}</button>
        </section></Show>
        <Show when=move || state.with(|s| s.form.is_some())><div class="production-layout">
            <section class="production-settings" aria-labelledby="casting-title"><h3 id="casting-title">{move || text(Key::Settings, english.get())}</h3>
                <p>{move || text(Key::Stored,english.get())}" · "<output>{saved_version}</output>" · "{move || if state.with(Production::is_dirty) { text(Key::Unsaved,english.get()) } else { "" }}</p>
                <Show when=move ||state.with(Production::is_dirty)><p class="help">{move ||text(Key::Revalidate,english.get())}</p></Show>
                <CastingFields state=state english=english readonly=readonly script=Signal::derive(move || editor.with(|e| e.remote.as_ref().map(|r| r.script_json.clone()).unwrap_or_default())) />
                <BudgetFields state=state english=english readonly=readonly />
                <button type="button" class="primary" aria-disabled=blocked aria-describedby="production-dirty-reason" on:click=move |_| {
                    if blocked() { return; }
                    let Some(settings) = state.with_untracked(|s| s.form.as_ref().and_then(SettingsForm::settings)) else { state.update(|s| s.error = Some(error(ErrorCode::InvalidRequest))); return; };
                    match operation_id() { Ok(operation_id) => submit(state,api,Command::Settings(SaveProductionSettingsRequest { operation_id, expected_revision: state.with_untracked(|s| s.context.revision), expected_settings_version:saved_version(),settings })), Err(error) => state.update(|s| s.error=Some(error)) }
                }>{move || text(Key::Save,english.get())}</button>
            </section>
            <section class="production-inspector" aria-labelledby="production-review-title"><h3 id="production-review-title">{move || text(Key::Validate,english.get())}</h3>
                <p class="help">{move || text(Key::ValidationHelp,english.get())}</p>
                <button type="button" aria-disabled=review_blocked on:click=move |_| {
                    if review_blocked() { return; } let Some((reading,ticket)) = state.get_untracked().start_read(ReadKind::Preview) else { return; }; state.set(reading);
                    spawn_local(async move { let result=api.production_preview(&ticket.context.script).await; let _=state.try_update(|s| *s=match result { Ok(preview)=>s.preview_loaded(&ticket,preview),Err(error)=>s.read_failed(&ticket,error) }); });
                }>{move || text(Key::Validate,english.get())}</button>
                {move || state.with(|s| s.preview.clone()).map(|preview| view! { <dl class="source-identity"><dt>{move || text(Key::Digest,english.get())}</dt><dd><code>{preview.input_digest}</code></dd></dl><Estimate estimate=preview.estimate english=english /><Findings findings=preview.findings english=english /> })}
                <p class="help">{move || text(Key::FreezeHelp,english.get())}</p>
                <button type="button" class="primary" aria-disabled=freeze_blocked on:click=move |_| {
                    if freeze_blocked() { return; } let Some(preview)=state.with_untracked(|s| s.preview.clone()) else { return; };
                    match operation_id() { Ok(operation_id)=>submit(state,api,Command::Freeze(FreezeProductionRequest { operation_id,expected_revision:preview.script_revision,settings_version:preview.settings_version,input_digest:preview.input_digest })),Err(error)=>state.update(|s| s.error=Some(error)) }
                }>{move || text(Key::Freeze,english.get())}</button>
            </section>
        </div>
        <RightsFields state=state rights=rights rights_dirty=rights_dirty english=english readonly=readonly script=Signal::derive(move || editor.with(|e| e.remote.as_ref().map(|r| r.script_json.clone()).unwrap_or_default())) />
        </Show>
        <section class="production-snapshots" aria-labelledby="production-snapshot-title"><h3 id="production-snapshot-title">{move || text(Key::Snapshot,english.get())}</h3>
            <div class="toolbar"><label for="production-snapshot-id">{move || text(Key::SnapshotId,english.get())}</label><input id="production-snapshot-id" autocomplete="off" spellcheck="false" prop:value=move || snapshot_id.get() on:input=move |e| snapshot_id.set(event_target_value(&e)) />
                <button type="button" aria-disabled=move || state.with(Production::has_activity) || composing.with(|fields| !fields.is_empty()) || !state.with(|s| s.context.ready()) on:click=move |_| read_snapshot(state,api,snapshot_id.get_untracked())>{move || text(Key::Inspect,english.get())}</button></div>
            <div class="toolbar">{move || state.with(|s| s.stored.as_ref().map(|s| s.snapshots.clone()).unwrap_or_default()).into_iter().map(|snapshot| { let id=StoredValue::new(snapshot.id.clone()); view! { <button type="button" aria-disabled=move || state.with(Production::has_activity) || composing.with(|fields| !fields.is_empty()) on:click=move |_| { snapshot_id.set(id.get_value());read_snapshot(state,api,id.get_value()); }><code>{snapshot.id}</code>" · "{snapshot.script_revision}</button> } }).collect_view()}</div>
            {move || state.with(|s| s.snapshot.clone()).map(|snapshot| view! { <SnapshotDetails snapshot=snapshot english=english /> })}
            <Show when=move || state.with(|s| s.snapshot.is_some())><label class="adaptation-confirm" for="production-confirm"><input id="production-confirm" type="checkbox" disabled=move || blocked() || state.with(Production::is_dirty) prop:checked=move || state.with(|s| s.confirmation.is_some()) on:change=move |e| state.update(|s| *s=s.confirm(event_target_checked(&e))) /><span>{move || text(Key::ConfirmApproval,english.get())}</span></label>
                <p id="production-approval-help" class="help">{move || text(Key::ApprovalHelp,english.get())}</p>
                <button type="button" class="primary" aria-disabled=approve_blocked aria-describedby="production-approval-help" on:click=move |_| {
                    if approve_blocked() { return; } let Some(snapshot)=state.with_untracked(|s| s.snapshot.clone()) else { return; };
                    match operation_id() { Ok(operation_id)=>submit(state,api,Command::Approve { snapshot_id:snapshot.id,request:ApproveProductionRequest { operation_id,input_digest:snapshot.document.input_digest } }),Err(error)=>state.update(|s| s.error=Some(error)) }
                }>{move || text(Key::Approve,english.get())}</button>
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
    english: RwSignal<bool>,
    readonly: Signal<bool>,
    #[prop(default = "text")] kind: &'static str,
    #[prop(optional)] min: Option<String>,
    #[prop(optional)] max: Option<String>,
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
    view! { <label class="production-field" for=id.clone()><span>{move || text(label,english.get())}</span>
        <input id=id.clone() node_ref=node type=kind min=min max=max step="1" autocomplete="off" disabled=move || readonly.get() prop:value=initial
            on:input=move |e| { if !readonly.get_untracked() && !composing.get_untracked() { change.run(event_target_value(&e)); } }
            on:compositionstart=move |_|{if readonly.get_untracked(){return;}composing.set(true);typing.update(|fields|{fields.insert(identity.get_value());});typing_context.1.update(|s|s.input_composing=true);}
            on:compositionend=move |e|{if !readonly.get_untracked(){change.run(event_target_value(&e));}composing.set(false);typing.update(|fields|{fields.remove(&identity.get_value());});typing_context.1.update(|s|s.input_composing=typing.with_untracked(|fields|!fields.is_empty()));} />
    </label> }
}

#[component]
fn FormSelect(
    id: String,
    label: Key,
    value: Signal<String>,
    options: Signal<Vec<(String, String)>>,
    change: Callback<String>,
    english: RwSignal<bool>,
    readonly: Signal<bool>,
) -> impl IntoView {
    view! { <label class="production-field" for=id.clone()><span>{move || text(label,english.get())}</span>
        <select id=id.clone() disabled=move || readonly.get() prop:value=move || value.get() on:change=move |e| { if !readonly.get_untracked() { change.run(event_target_value(&e)); } }>
            {move || {
                let mut choices = options.get();
                let current = value.get();
                if !current.is_empty() && !choices.iter().any(|(choice,_)|choice==&current) {
                    choices.push((current.clone(),format!("{} · {current}",text(Key::UnsupportedChoice,english.get()))));
                }
                choices.into_iter().map(|(choice,label)| {
                    let choice=StoredValue::new(choice);
                    view! { <option value=choice.get_value() prop:selected=move || value.get()==choice.get_value()>{label}</option> }
                }).collect_view()
            }}
        </select>
    </label> }
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
    english: RwSignal<bool>,
    readonly: Signal<bool>,
    script: Signal<String>,
) -> impl IntoView {
    view! { <For each=move || state.with(|s|s.form.as_ref().map(|f|f.bindings.iter().map(|b|b.character_id.clone()).collect::<Vec<_>>()).unwrap_or_default()) key=|id|id.clone() let:id>
        <CastingCharacter state=state english=english readonly=readonly script=script character_id=id />
    </For> }
}

#[component]
fn CastingCharacter(
    state: RwSignal<Production>,
    english: RwSignal<bool>,
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
    let capability = Signal::derive(move || {
        state.with(|s| {
            s.catalog
                .as_ref()
                .and_then(|c| {
                    c.capabilities.iter().find(|c| {
                        binding.get().is_some_and(|b| {
                            b.provider_id == c.provider_id && b.model_id == c.model_id
                        })
                    })
                })
                .cloned()
        })
    });
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
        state.with(|s| {
            s.catalog
                .as_ref()
                .map(|c| {
                    c.capabilities
                        .iter()
                        .filter(|c| {
                            binding
                                .get()
                                .is_some_and(|b| b.provider_id == c.provider_id)
                        })
                        .map(|c| (c.model_id.clone(), c.model_id.clone()))
                        .collect()
                })
                .unwrap_or_default()
        })
    });
    let voices = Signal::derive(move || {
        let mut voices = vec![(String::new(), text(Key::ChooseVoice, english.get()).into())];
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
        let mut emotions = vec![(String::new(), text(Key::Inherit, english.get()).into())];
        if let Some(c) = capability.get() {
            emotions.extend(c.emotions.into_iter().map(|e| {
                (
                    e.clone(),
                    messages::authoring::choice(&e, english.get()).into(),
                )
            }));
        }
        emotions
    });
    view! { <fieldset class="production-character" disabled=move ||readonly.get()><legend>{name}<code>{id.get_value()}</code></legend>
        <div class="production-fields">
            <FormSelect id=format!("{prefix}-provider") label=Key::Provider value=value(BindingField::Provider) options=providers change=change(BindingField::Provider) english=english readonly=readonly />
            <FormSelect id=format!("{prefix}-model") label=Key::Model value=value(BindingField::Model) options=models change=change(BindingField::Model) english=english readonly=readonly />
            <FormSelect id=format!("{prefix}-voice") label=Key::Voice value=value(BindingField::Voice) options=voices change=change(BindingField::Voice) english=english readonly=readonly />
            <FormSelect id=format!("{prefix}-language") label=Key::Language value=value(BindingField::Language) options=languages change=change(BindingField::Language) english=english readonly=readonly />
            <FormField id=format!("{prefix}-rights") label=Key::VoiceRights value=value(BindingField::Rights) change=change(BindingField::Rights) english=english readonly=readonly />
            <FormField id=format!("{prefix}-rate") label=Key::Rate value=value(BindingField::Rate) change=change(BindingField::Rate) english=english readonly=readonly kind="number" min=capability.get_untracked().map(|c|c.rate_min_permille.to_string()).unwrap_or_default() max=capability.get_untracked().map(|c|c.rate_max_permille.to_string()).unwrap_or_default() />
            <FormField id=format!("{prefix}-pitch") label=Key::Pitch value=value(BindingField::Pitch) change=change(BindingField::Pitch) english=english readonly=readonly kind="number" min=capability.get_untracked().map(|c|c.pitch_min_semitones.to_string()).unwrap_or_default() max=capability.get_untracked().map(|c|c.pitch_max_semitones.to_string()).unwrap_or_default() />
            <FormSelect id=format!("{prefix}-emotion") label=Key::Emotion value=value(BindingField::Emotion) options=emotions change=change(BindingField::Emotion) english=english readonly=readonly />
            <FormField id=format!("{prefix}-intensity") label=Key::Intensity value=value(BindingField::Intensity) change=change(BindingField::Intensity) english=english readonly=Signal::derive(move ||readonly.get()||capability.get().is_none_or(|c|!c.intensity_supported)) kind="number" min="0".into() max="1000".into() />
        </div>
        <h4>{move ||text(Key::Pronunciation,english.get())}</h4>
        <For each=move || binding.get().map(|b|(0..b.pronunciation.len()).collect::<Vec<_>>()).unwrap_or_default() key=|i|*i let:index>
            <div class="production-pronunciation">
                <FormField id=format!("production-{}-pronunciation-{index}-surface",id.get_value()) label=Key::Surface value=Signal::derive(move ||binding.get().and_then(|b|b.pronunciation.get(index).map(|p|p.surface.clone())).unwrap_or_default()) change=Callback::new(move |value|state.update(|s|*s=s.edit(|f|if let Some(p)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()).and_then(|b|b.pronunciation.get_mut(index)){p.surface=value;}))) english=english readonly=readonly />
                <FormField id=format!("production-{}-pronunciation-{index}-replacement",id.get_value()) label=Key::Replacement value=Signal::derive(move ||binding.get().and_then(|b|b.pronunciation.get(index).map(|p|p.replacement.clone())).unwrap_or_default()) change=Callback::new(move |value|state.update(|s|*s=s.edit(|f|if let Some(p)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()).and_then(|b|b.pronunciation.get_mut(index)){p.replacement=value;}))) english=english readonly=readonly />
                <button type="button" aria-disabled=move ||readonly.get() on:click=move |_|{if !readonly.get_untracked(){state.update(|s|*s=s.edit(|f|if let Some(b)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()){if index<b.pronunciation.len(){b.pronunciation.remove(index);}}));if let Some(element)=leptos::leptos_dom::helpers::document().get_element_by_id(&format!("production-{}-pronunciation-add",id.get_value())).and_then(|e|e.dyn_into::<web_sys::HtmlElement>().ok()){let _=element.focus();}}}>{move ||text(Key::Remove,english.get())}</button>
            </div>
        </For>
        <button id=format!("production-{}-pronunciation-add",id.get_value()) type="button" aria-disabled=move ||readonly.get()||capability.get().is_none_or(|c|!c.pronunciation_supported) on:click=move |_|{if !readonly.get_untracked()&&capability.get_untracked().is_some_and(|c|c.pronunciation_supported){state.update(|s|*s=s.edit(|f|if let Some(b)=f.bindings.iter_mut().find(|b|b.character_id==id.get_value()){b.pronunciation.push(ProductionPronunciation{surface:String::new(),replacement:String::new()});}));}}>{move ||text(Key::AddPronunciation,english.get())}</button>
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
    english: RwSignal<bool>,
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
    view! {<fieldset class="production-budget" disabled=move ||readonly.get()><legend>{move ||text(Key::Budget,english.get())}</legend><div class="production-fields">
        <FormSelect id="production-currency".into() label=Key::Currency value=Signal::derive(move ||state.with(|s|s.form.as_ref().map(|f|format!("{:?}",f.currency)).unwrap_or_default())) options=Signal::derive(||vec![("USD".into(),"USD".into()),("VND".into(),"VND".into())]) change=Callback::new(move |value:String|state.update(|s|*s=s.edit(|f|f.currency=if value=="VND"{ProductionCurrency::VND}else{ProductionCurrency::USD}))) english=english readonly=readonly />
        <FormField id="production-limit".into() label=Key::Limit value=value(BudgetField::Limit) change=change(BudgetField::Limit) english=english readonly=readonly kind="number" min="0".into() />
        <FormField id="production-scope".into() label=Key::Scope value=value(BudgetField::Scope) change=change(BudgetField::Scope) english=english readonly=readonly />
        <FormField id="production-territory".into() label=Key::Territory value=value(BudgetField::Territory) change=change(BudgetField::Territory) english=english readonly=readonly />
    </div><label class="adaptation-confirm" for="production-known-rate"><input id="production-known-rate" type="checkbox" disabled=move ||readonly.get() prop:checked=move ||state.with(|s|s.form.as_ref().is_some_and(|f|f.known_rate)) on:change=move |e|{if !readonly.get_untracked(){state.update(|s|*s=s.edit(|f|f.known_rate=event_target_checked(&e)));}} /><span>{move ||text(Key::KnownRate,english.get())}</span></label>
    <Show when=move ||state.with(|s|s.form.as_ref().is_some_and(|f|f.known_rate))><div class="production-fields">
        <FormField id="production-rate-reference".into() label=Key::RateReference value=value(BudgetField::Reference) change=change(BudgetField::Reference) english=english readonly=readonly />
        <FormField id="production-rate-version".into() label=Key::RateVersion value=value(BudgetField::Version) change=change(BudgetField::Version) english=english readonly=readonly />
        <FormField id="production-rate-units".into() label=Key::RateUnits value=value(BudgetField::Units) change=change(BudgetField::Units) english=english readonly=readonly kind="number" min="1".into() />
        <FormField id="production-rate-amount".into() label=Key::RateAmount value=value(BudgetField::Amount) change=change(BudgetField::Amount) english=english readonly=readonly kind="number" min="0".into() />
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
    english: RwSignal<bool>,
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
        let mut options = vec![(String::new(), text(Key::Subject, english.get()).into())];
        options.extend(evidence_ids(&script.get()).into_iter().map(|id| {
            (
                format!("evidence:{id}"),
                format!("{} · {id}", text(Key::Evidence, english.get())),
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
                                    text(Key::VoiceSubject, english.get()),
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
    view! {<section class="production-rights" aria-labelledby="production-rights-title"><h3 id="production-rights-title">{move ||text(Key::Rights,english.get())}</h3><p class="help">{move ||text(Key::RightsHelp,english.get())}</p>
        <ul class="production-rights-list">{move ||state.with(|s|s.stored.as_ref().map(|s|s.rights.clone()).unwrap_or_default()).into_iter().map(|r|view!{<li><strong>{r.claim.declaration.record_id}</strong>" · "{move ||messages::production::rights_status(r.claim.declaration.status,english.get())}" · "{r.claim.version}<small>{r.recorded_by}" · "{r.recorded_at}" · "{r.claim.declaration.reference}</small></li>}).collect_view()}</ul>
        <fieldset disabled=move ||readonly.get()><legend>{move ||text(Key::SaveRights,english.get())}</legend><div class="production-fields">
            <FormSelect id="production-rights-subject".into() label=Key::Subject value=Signal::derive(move ||rights.with(|r|r.subject.clone())) options=options change=Callback::new(move |value:String|{let record=if let Some(id)=value.strip_prefix("evidence:"){id.to_owned()}else{state.with_untracked(|s|s.form.as_ref().and_then(|f|f.bindings.iter().find(|b|Some(b.character_id.as_str())==value.strip_prefix("voice:")).map(|b|b.voice_rights_record_id.clone())).unwrap_or_default())};rights.update(|r|{r.subject=value;r.record_id=record;});state.update(|s| *s = s.rights_edited()); rights_dirty.set(true);}) english=english readonly=readonly />
            <FormField id="production-rights-record".into() label=Key::RecordId value=value(RightsField::Record) change=change(RightsField::Record) english=english readonly=readonly />
            <FormSelect id="production-rights-status".into() label=Key::ClaimStatus value=Signal::derive(move ||rights.with(|r|match r.status{ProductionRightsStatus::Pending=>"pending",ProductionRightsStatus::Granted=>"granted",ProductionRightsStatus::Revoked=>"revoked"}.into())) options=Signal::derive(move ||vec![("pending".into(),text(Key::Pending,english.get()).into()),("granted".into(),text(Key::Granted,english.get()).into()),("revoked".into(),text(Key::Revoked,english.get()).into())]) change=Callback::new(move |value:String|{rights.update(|r|r.status=match value.as_str(){"granted"=>ProductionRightsStatus::Granted,"revoked"=>ProductionRightsStatus::Revoked,_=>ProductionRightsStatus::Pending});state.update(|s| *s = s.rights_edited()); rights_dirty.set(true);}) english=english readonly=readonly />
            <FormField id="production-rights-reference".into() label=Key::Reference value=value(RightsField::Reference) change=change(RightsField::Reference) english=english readonly=readonly />
            <FormField id="production-rights-holder".into() label=Key::Holder value=value(RightsField::Holder) change=change(RightsField::Holder) english=english readonly=readonly />
            <FormField id="production-rights-languages".into() label=Key::RightsLanguages value=value(RightsField::Languages) change=change(RightsField::Languages) english=english readonly=readonly />
            <FormField id="production-rights-territory".into() label=Key::Territory value=value(RightsField::Territory) change=change(RightsField::Territory) english=english readonly=readonly />
            <FormField id="production-rights-scope".into() label=Key::PermittedScope value=value(RightsField::Scope) change=change(RightsField::Scope) english=english readonly=readonly />
            <FormField id="production-rights-attribution".into() label=Key::Attribution value=value(RightsField::Attribution) change=change(RightsField::Attribution) english=english readonly=readonly />
            <FormField id="production-rights-restrictions".into() label=Key::Restrictions value=value(RightsField::Restrictions) change=change(RightsField::Restrictions) english=english readonly=readonly />
            <FormField id="production-rights-from".into() label=Key::ValidFrom value=value(RightsField::From) change=change(RightsField::From) english=english readonly=readonly kind="number" />
            <FormField id="production-rights-until".into() label=Key::ValidUntil value=value(RightsField::Until) change=change(RightsField::Until) english=english readonly=readonly kind="number" />
        </div><p class="help">{move ||text(Key::TimeHelp,english.get())}</p><p class="help">{move ||text(Key::RestrictionsHelp,english.get())}</p><button type="button" aria-disabled=mutation_blocked on:click=move |_|{
            if mutation_blocked(){return;}let current=state.get_untracked();let Some(claim)=current.form.as_ref().and_then(|form|rights.get_untracked().declaration(form))else{state.update(|s|s.error=Some(error(ErrorCode::InvalidRequest)));return;};
            let expected_version=current.stored.as_ref().and_then(|s|s.rights.iter().find(|r|r.claim.declaration.record_id==claim.record_id).map(|r|r.claim.version)).unwrap_or(0);
            match operation_id(){Ok(operation_id)=>{submit(state,api,Command::Rights(SaveProductionRightsRequest{operation_id,expected_version,claim}));},Err(error)=>state.update(|s|s.error=Some(error))}
        }>{move ||text(Key::SaveRights,english.get())}</button></fieldset>
    </section>}
}

#[component]
fn SnapshotDetails(snapshot: ProductionSnapshotResponse, english: RwSignal<bool>) -> impl IntoView {
    let document = snapshot.document;
    view! { <article class="production-snapshot"><dl class="source-identity">
        <dt>{move || text(Key::SnapshotId,english.get())}</dt><dd><code>{snapshot.id}</code></dd>
        <dt>{move || text(Key::Revision,english.get())}</dt><dd>{document.revision.revision}</dd>
        <dt>{move || text(Key::Stored,english.get())}</dt><dd>{document.settings.version}</dd>
        <dt>{move || text(Key::Digest,english.get())}</dt><dd><code>{document.input_digest}</code></dd>
        <dt>{move || text(Key::Actor,english.get())}</dt><dd>{snapshot.created_by}</dd>
        <dt>{move || text(Key::RecordedAt,english.get())}</dt><dd>{snapshot.recorded_at}</dd>
        <dt>{move || text(Key::Eligibility,english.get())}</dt><dd>{move || if snapshot.eligibility.inputs_eligible { text(Key::Eligible,english.get()) } else { text(Key::Blocked,english.get()) }}</dd>
        <dt>{move || text(Key::CurrentApproval,english.get())}</dt><dd>{move || if snapshot.eligibility.approval_current { text(Key::ApprovalCurrent,english.get()) } else { text(Key::ApprovalNotCurrent,english.get()) }}</dd>
        <dt>{move || text(Key::Limit,english.get())}</dt><dd>{move ||format_money(document.settings.settings.budget.currency,document.settings.settings.budget.limit_minor,english.get())}</dd>
        <dt>{move || text(Key::Scope,english.get())}</dt><dd>{document.settings.settings.budget.scope}</dd>
        <dt>{move || text(Key::Territory,english.get())}</dt><dd>{document.settings.settings.budget.territory}</dd>
        <dt>{move || text(Key::Approval,english.get())}</dt><dd>{snapshot.approval.map(|a| format!("{} · {} · {}", a.approved_by,a.approved_at,a.input_digest)).unwrap_or_else(||text(Key::NoApproval,english.get_untracked()).into())}</dd>
    </dl><Estimate estimate=document.estimate english=english /><Findings findings=snapshot.eligibility.findings english=english />
    <details><summary>{move || text(Key::Settings,english.get())}</summary><pre>{serde_json::to_string_pretty(&document.resolved).unwrap_or_default()}</pre></details>
    <details><summary>{move || text(Key::Rights,english.get())}</summary><pre>{serde_json::to_string_pretty(&document.rights).unwrap_or_default()}</pre></details>
    </article> }
}
