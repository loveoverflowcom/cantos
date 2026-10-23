//! Leptos adaptation shell: every mutation is an explicit, immutable intent.
use super::{response_proposal, response_status, AdaptationIntent, AdaptationReview};
use crate::contract::{
    AdaptationContextResponse, AdaptationCoverageDisposition, AdaptationFindingCode,
    AdaptationProposal, AdaptationReviewResponse, AdaptationStatus, AdaptationSubmissionReceipt,
    ImportOutcome, ImportResponse,
};
use crate::{
    api::{StudioContext, StudioHttp},
    authoring_view::StructuredEditor,
    editor::ValidationPreview,
    import::ExtractionPreview,
    messages,
    view::{confirm_discard, operation_id, ValidationControls},
};
use leptos::{prelude::*, task::spawn_local};
use std::sync::Arc;

fn dispatch(state: RwSignal<AdaptationReview>, api: StudioHttp, intent: AdaptationIntent) {
    let api = api.snapshot();
    spawn_local(async move {
        let result = api.accept_adaptation(&intent.run_id, &intent.request).await;
        let _ = state.try_update(|review| {
            if review.pending.as_ref() == Some(&intent) {
                *review = match result {
                    Ok(revision) => review.accepted_result(&intent, revision),
                    Err(error) => review.failed(&error),
                };
            }
        });
    });
}

fn read_run(state: RwSignal<AdaptationReview>, api: StudioHttp, activity: RwSignal<bool>) {
    let Some(reading) = state.get_untracked().start_read() else {
        return;
    };
    let id = reading.run_id.clone();
    let actor = reading.actor.clone();
    let ticket = reading.ticket;
    state.set(reading);
    let api = api.snapshot();
    spawn_local(async move {
        let result = api.read_adaptation(&id).await;
        let _ = state.try_update(|review| {
            *review = match result {
                Ok(run) => review.loaded_with_buffer(ticket, &actor, run, activity.get_untracked()),
                Err(error) => review.read_failed(ticket, &actor, &error),
            };
        });
    });
}

#[component]
pub fn ScriptAdaptation(
    actor: Signal<String>,
    english: Signal<bool>,
    dirty: RwSignal<bool>,
    activity: RwSignal<bool>,
    session_pending: Signal<bool>,
) -> impl IntoView {
    let state = RwSignal::new(AdaptationReview::default());
    let preview = RwSignal::new(ValidationPreview::default());
    let issues = RwSignal::new(Vec::new());
    let api = expect_context::<StudioContext>().api;
    let copy = move || messages::adaptation_copy(english.get());
    let signed_actor = Memo::new(move |_| actor.get());
    Effect::new(move |_| {
        state.update(|review| *review = review.signed_in_as(signed_actor.get()));
    });
    // Immutable source/run facts do not remount their reading panes when the JSON changes.
    let source = Memo::new(move |_| state.with(|review| review.source.clone()));
    let stored = Memo::new(move |_| state.with(|review| review.stored.clone()));
    let input_revision = Memo::new(move |_| state.with(|review| review.input_revision.clone()));
    let accepted = Memo::new(move |_| state.with(|review| review.accepted.clone()));
    let blocked =
        move || session_pending.get() || state.with(AdaptationReview::blocked) || activity.get();
    let draft = Signal::derive(move || {
        state.with(|review| {
            if review.draft_actor == review.actor {
                review.draft.clone()
            } else {
                String::new()
            }
        })
    });
    let accept_blocked = move || {
        session_pending.get()
            || activity.get()
            || state.with(|review| !review.can_accept())
            || !preview.with(|p| p.is_current(&actor.get(), &draft.get()))
    };
    Effect::new(move |_| {
        dirty.set(
            activity.get()
                || state
                    .with(|review| review.draft_changed || review.pending.is_some() || review.busy),
        );
        if activity.get() {
            state.update(|review| *review = review.review_findings(false));
        }
    });
    Effect::new(move |_| {
        issues.set(state.with(|review| review.issues.clone()));
    });

    view! {
        <section class="script-adaptation" aria-labelledby="adaptation-title">
            <h2 id="adaptation-title">{move || copy().title}</h2>
            <p id="adaptation-help" class="help">{move || copy().help}</p>
            <p id="adaptation-reason" class="help">{move || state.with(|review| if review.pending.is_some() { copy().pending } else { "" })}</p>
            <Show when=move || state.with(|review| review.pending.is_some())>
                <p class="identity">{move || copy().operation}<output>{move || state.with(|review| review.pending.as_ref().map(|intent| intent.operation_id().to_owned()).unwrap_or_default())}</output></p>
                <button class="cui-button" data-variant="secondary" type="button" aria-describedby="adaptation-reason" disabled=move || session_pending.get() || activity.get() || state.with(|review| !review.can_retry()) on:click=move |_| {
                    if session_pending.get_untracked() || activity.get_untracked() { return; }
                    if let Some((sending, intent)) = state.get_untracked().retry() { state.set(sending); dispatch(state, api, intent); }
                }>{move || copy().retry}</button>
            </Show>
            <form class="adaptation-reopen" on:submit=move |event| { event.prevent_default(); if !session_pending.get_untracked() && !activity.get_untracked() { read_run(state, api, activity); } }>
                <label for="adaptation-run-id">{move || copy().run_id}</label>
                <div class="toolbar">
                    <input class="cui-field-control" id="adaptation-run-id" maxlength="36" disabled=move || blocked() || state.with(|review| review.draft_changed) autocomplete="off" spellcheck="false"
                        prop:value=move || state.with(|review| review.run_id.clone())
                        on:input=move |event| state.update(|review| *review = review.select_run(event_target_value(&event))) />
                    <button class="cui-button" data-variant="secondary" type="submit" disabled=move || session_pending.get() || activity.get() || state.with(|review| !review.can_read()) aria-describedby="adaptation-new-help">{move || copy().open}</button>
                </div>
            </form>
            <div class="toolbar">
                <button class="cui-button" data-variant="secondary" type="button" disabled=move || session_pending.get() || activity.get() || state.with(|review| !review.can_read()) on:click=move |_| { if !session_pending.get_untracked() && !activity.get_untracked() { read_run(state, api, activity); } }>{move || copy().refresh}</button>
                <button class="cui-button" data-variant="secondary" type="button" disabled=blocked aria-describedby="adaptation-new-help" on:click=move |_| {
                    if blocked() || (state.with_untracked(|review| review.draft_changed) && !confirm_discard(english.get_untracked())) { return; }
                    state.update(|review| *review = review.new_run()); preview.set(ValidationPreview::default()); issues.set(vec![]);
                }>{move || copy().new_run}</button>
            </div>
            <p id="adaptation-new-help" class="help">{move || copy().new_help}</p>
            <p id="adaptation-status" class="status" role="status" aria-live="polite">{move || state.with(|review| messages::adaptation_status(&review.status, english.get()))}</p>
            <Show when=move || state.with(|review| !review.issues.is_empty())>
                <ul class="import-warnings">{move || state.with(|review| review.issues.clone()).into_iter().map(|issue| view! { <li><code>{issue.path}</code>" · "<code>{issue.rule}</code></li> }).collect_view()}</ul>
            </Show>
            <div hidden=move || stored.get().is_none_or(|response| response_proposal(&response).is_none())>
                <StructuredEditor draft=draft english=english activity=activity namespace="proposal"
                    readonly=Signal::derive(move || stored.get().is_none_or(|response| !matches!(response_status(&response), AdaptationStatus::Succeeded | AdaptationStatus::Accepted)) || state.with(|review| review.busy || review.draft_actor != review.actor))
                    baseline=Signal::derive(move || stored.get().and_then(|response| response_proposal(&response).map(|proposal| proposal.script_json.clone())).unwrap_or_default())
                    issues=Signal::derive(move || issues.get())
                    on_change=Callback::new(move |text| { state.update(|review| *review = review.edit_draft(text)); issues.set(vec![]); })>
                    <details open><summary>{move || copy().original}</summary>
                        {move || source.get().map(|source| view! { <SourceComparison source=source english=english /> })}
                    </details>
                    <details><summary>{move || copy().provenance}</summary>
                        {move || stored.get().map(|response| view! { <ReviewDetails response=response english=english /> })}
                    </details>
                </StructuredEditor>
                <ValidationControls id="adaptation-validate" preview=preview issues=issues draft=draft actor=actor english=english disabled=Signal::derive(move || blocked() || activity.get()) />
                <label class="adaptation-confirm" for="adaptation-reviewed">
                    <input id="adaptation-reviewed" type="checkbox" disabled=move || blocked() || activity.get() || !preview.with(|p| p.is_current(&actor.get(), &draft.get())) prop:checked=move || state.with(|review| review.reviewed_findings)
                        on:change=move |event| state.update(|review| *review = review.review_findings(event_target_checked(&event))) />
                    <span>{move || copy().review}</span>
                </label>
                <button class="cui-button" data-variant="primary" type="button" disabled=accept_blocked aria-describedby="adaptation-accept-reason adaptation-accept-help" on:click=move |_| {
                    if accept_blocked() { return; }
                    match operation_id() {
                        Ok(operation) => if let Some((sending, intent)) = state.get_untracked().start_accept(operation) { state.set(sending); dispatch(state, api, intent); },
                        Err(error) => state.update(|review| *review = review.failed(&error)),
                    }
                }>{move || copy().accept}</button>
                <p id="adaptation-accept-reason" class="help">{move || if accept_blocked() { copy().accept_required } else { "" }}</p>
                <p id="adaptation-accept-help" class="help">{move || copy().accept_help}</p>
            </div>
            {move || input_revision.get().map(|revision| view! {
                <details><summary>{move || messages::adaptation_input_revision(english.get())}</summary>
                    <p class="identity">{format!("{} / {} · {}", revision.script_id, revision.revision, revision.export_digest)}</p>
                    <pre tabindex="0" lang="vi-VN">{revision.script_json.clone()}</pre>
                </details>
            })}
            {move || accepted.get().map(|revision| {
                let revision = StoredValue::new(revision);
                view! {
                    <section aria-labelledby="adaptation-accepted-title">
                        <h3 id="adaptation-accepted-title">{move || copy().accepted}</h3>
                        <p class="identity">{revision.with_value(|revision| format!("{} / {} · {} · {}", revision.script_id, revision.revision, revision.accepted_by, revision.accepted_at))}</p>
                        <p class="identity"><code>{revision.with_value(|revision| revision.export_digest.clone())}</code></p>
                        <details><summary>{move || crate::messages::workspace::text(crate::messages::workspace::Key::Stored, english.get())}</summary><pre tabindex="0" lang="vi-VN">{revision.with_value(|revision| revision.script_json.clone())}</pre></details>
                        <button class="cui-button" data-variant="secondary" type="button" disabled=blocked on:click=move |_| {
                            if activity.get_untracked() { return; }
                            let Some(reading) = state.get_untracked().start_accepted_read() else { return; };
                            let actor = reading.actor.clone(); let ticket = reading.ticket;
                            let (script_id, version) = revision.with_value(|revision| (revision.script_id.clone(), revision.revision));
                            state.set(reading);
                            let api = api.snapshot();
    spawn_local(async move {
                                let result = api.read_revision(&script_id, version).await;
                                let _ = state.try_update(|review| *review = match result {
                                    Ok(revision) => review.accepted_loaded_with_buffer(ticket, &actor, revision, activity.get_untracked()),
                                    Err(error) => review.read_failed(ticket, &actor, &error),
                                });
                            });
                        }>{move || copy().reopen}</button>
                    </section>
                }
            })}
        </section>
    }
}

#[component]
fn ReviewDetails(response: Arc<AdaptationReviewResponse>, english: Signal<bool>) -> impl IntoView {
    let AdaptationReviewResponse::Caller { context } = response.as_ref();
    view! { <CallerContextDetails context=Arc::new(context.as_ref().clone()) english=english /> }
}

#[component]
fn CallerContextDetails(
    context: Arc<AdaptationContextResponse>,
    english: Signal<bool>,
) -> impl IntoView {
    let context = StoredValue::new(context);
    let copy = move || messages::adaptation_copy(english.get());
    let host_copy = move || messages::host_copy(english.get());
    view! {
        <section aria-labelledby="adaptation-provenance-title">
            <h3 id="adaptation-provenance-title">{move || copy().provenance}</h3>
            <p class="status" role="status" aria-live="polite">{move || context.with_value(|context| messages::adaptation_run_status(context.status, english.get()))}</p>
            <dl class="source-identity">
                <dt>{move || copy().run_id}</dt><dd>{context.with_value(|context| context.id.clone())}</dd>
                <dt>{move || copy().operation}</dt><dd>{context.with_value(|context| context.request.operation_id.clone())}</dd>
                <dt>{move || host_copy().context_digest}</dt><dd><code>{context.with_value(|context| context.context_digest.clone())}</code></dd>
                <dt>{move || host_copy().context_version}</dt><dd>{context.with_value(|context| context.context_version.clone())}</dd>
                <dt>{move || copy().source_id}</dt><dd>{context.with_value(|context| context.request.source_id.clone())}</dd>
                <dt>{move || copy().checksum}</dt><dd><code>{context.with_value(|context| context.request.source_sha256.clone())}</code></dd>
                <dt>{move || copy().extraction}</dt><dd>{context.with_value(|context| context.request.extractor_version.clone())}</dd>
                <dt>{move || copy().script_id}</dt><dd>{context.with_value(|context| context.request.script_id.clone())}</dd>
                <dt>{move || copy().base}</dt><dd>{context.with_value(|context| context.request.expected_revision)}</dd>
                <dt>{move || copy().prompt}</dt><dd>{context.with_value(|context| format!("{} / {}", context.prompt_version, context.contract_version))}</dd>
                <dt>{move || copy().records}</dt><dd>{context.with_value(|context| format!("{} / {}", context.generation_record_id, context.rights_record_id))}</dd>
                <dt>{move || host_copy().rights_claim}</dt><dd>{move || context.with_value(|context| if context.request.rights_authorization { host_copy().authorized } else { host_copy().absent })}</dd>
            </dl>
            {context.with_value(|context| context.latest_submission.clone()).map(|receipt| view! { <CallerReceiptDetails receipt=receipt english=english /> })}
            {context.with_value(|context| context.proposal.clone()).map(|proposal| view! { <ProposalFindings proposal=proposal english=english /> })}
        </section>
    }
}

#[component]
fn CallerReceiptDetails(
    receipt: AdaptationSubmissionReceipt,
    english: Signal<bool>,
) -> impl IntoView {
    let receipt = StoredValue::new(receipt);
    let copy = move || messages::adaptation_copy(english.get());
    let host_copy = move || messages::host_copy(english.get());
    view! {
        <h4>{move || host_copy().submission}</h4>
        <p class="help">{move || host_copy().caller_warning}</p>
        <p>{move || receipt.with_value(|receipt| messages::adaptation_submission_status(receipt.status, english.get()))}</p>
        <dl class="source-identity">
            <dt>{move || host_copy().host_tool}</dt><dd lang="vi-VN">{receipt.with_value(|receipt| receipt.generation.host_tool.clone())}</dd>
            <dt>{move || copy().provider}</dt><dd>{move || receipt.with_value(|receipt| receipt.generation.provider.clone().unwrap_or_else(|| copy().unknown.into()))}</dd>
            <dt>{move || copy().model}</dt><dd>{move || receipt.with_value(|receipt| receipt.generation.model.clone().unwrap_or_else(|| copy().unknown.into()))}</dd>
            <dt>{move || copy().config}</dt><dd>{move || receipt.with_value(|receipt| receipt.generation.configuration_json.clone().unwrap_or_else(|| copy().unknown.into()))}</dd>
            <dt>{move || host_copy().recorded_prompt}</dt><dd>{receipt.with_value(|receipt| receipt.generation.prompt_version.clone())}</dd>
            <dt>{move || copy().usage}</dt><dd>{move || receipt.with_value(|receipt| messages::adaptation_usage(receipt.generation.usage.as_ref(), english.get()))}</dd>
            <dt>{move || copy().cost}</dt><dd>{move || receipt.with_value(|receipt| messages::adaptation_cost(receipt.generation.cost.as_ref(), english.get()))}</dd>
            <dt>{move || host_copy().submitted_by}</dt><dd>{receipt.with_value(|receipt| receipt.submitted_by.clone())}</dd>
            <dt>{move || host_copy().submitted_at}</dt><dd>{receipt.with_value(|receipt| receipt.submitted_at.clone())}</dd>
            <dt>{move || host_copy().output_checksum}</dt><dd><code>{receipt.with_value(|receipt| receipt.output_sha256.clone())}</code></dd>
            <dt>{move || copy().operation}</dt><dd>{receipt.with_value(|receipt| receipt.operation_id.clone())}</dd>
        </dl>
        {receipt.with_value(|receipt| receipt.problem.clone()).map(|problem| view! {
            <h4>{move || copy().problem}</h4><p>{move || messages::adaptation_problem(&problem.code, english.get())}</p>
            <ul>{problem.issues.into_iter().map(|issue| view! { <li><code>{issue.path}</code>" · "<code>{issue.rule}</code></li> }).collect_view()}</ul>
        })}
    }
}

#[component]
fn ProposalFindings(proposal: AdaptationProposal, english: Signal<bool>) -> impl IntoView {
    let copy = move || messages::adaptation_copy(english.get());

    let no_findings = proposal.findings.is_empty();
    view! {
        <h4>{move || copy().findings}</h4>
        <Show when=move || no_findings><p>{move || copy().no_findings}</p></Show>
        <ul class="import-warnings">{proposal.findings.into_iter().map(|finding| {
            let finding = StoredValue::new(finding);
            view! { <li><strong>{move || finding.with_value(|finding| messages::adaptation_finding(finding.code, english.get()))}</strong>
                {finding.with_value(|finding| finding.block).map(|block| view! { <p>{move || copy().block}" "{block}</p> })}
                <p>{move || finding.with_value(|finding| messages::adaptation_finding_explanation(finding.code, english.get()))}</p>
                <Show when=move || finding.with_value(|finding| finding.code == AdaptationFindingCode::SourceWarning)>
                    <p>{move || finding.with_value(|finding| messages::import_diagnostic(&finding.detail, english.get()))}</p>
                    <small><code>{finding.with_value(|finding| finding.detail.clone())}</code></small>
                </Show>
                <Show when=move || finding.with_value(|finding| messages::adaptation_has_provider_detail(finding.code))>
                    <p class="help">{move || messages::adaptation_provider_detail(english.get())}</p>
                    <blockquote lang="vi-VN">{finding.with_value(|finding| finding.detail.clone())}</blockquote>
                </Show>
            </li> }
        }).collect_view()}</ul>
        <details><summary>{move || copy().coverage}</summary>
            <p class="help">{move || copy().coverage_help}</p>
            <ul class="adaptation-coverage">{proposal.coverage.into_iter().map(|coverage| {
                let coverage = StoredValue::new(coverage);
                view! { <li><strong>{move || copy().block}" "{coverage.with_value(|coverage| coverage.block)}</strong>" · "{move || coverage.with_value(|coverage| match coverage.disposition { AdaptationCoverageDisposition::Represented => copy().represented, AdaptationCoverageDisposition::Omitted => copy().omitted })}
                    <p class="identity"><code>{coverage.with_value(|coverage| coverage.dialogue_ids.join(", "))}</code></p>
                    {coverage.with_value(|coverage| coverage.reason.clone()).map(|reason| view! { <p lang="vi-VN">{reason}</p> })}
                </li> }
            }).collect_view()}</ul>
        </details>
    }
}

#[component]
fn SourceComparison(source: Arc<ImportResponse>, english: Signal<bool>) -> impl IntoView {
    let source = StoredValue::new(source);
    let copy = move || messages::adaptation_copy(english.get());
    let import_copy = move || messages::import_copy(english.get());
    view! {
        <dl class="source-identity">
            <dt>{move || import_copy().reference}</dt><dd>{source.with_value(|source| source.metadata.reference.clone())}</dd>
            <dt>{move || import_copy().rights_holder}</dt><dd>{move || source.with_value(|source| source.metadata.rights_holder.clone().unwrap_or_else(|| import_copy().absent.into()))}</dd>
            <dt>{move || import_copy().permission_evidence}</dt><dd>{move || source.with_value(|source| source.metadata.permission_evidence.clone().unwrap_or_else(|| import_copy().absent.into()))}</dd>
            <dt>{move || import_copy().usage_scope}</dt><dd>{move || source.with_value(|source| source.metadata.usage_scope.clone().unwrap_or_else(|| import_copy().absent.into()))}</dd>
            <dt>{move || copy().checksum}</dt><dd><code>{source.with_value(|source| source.sha256.clone())}</code></dd>
        </dl>
        {source.with_value(|source| source.original_text.clone()).map(|text| view! { <pre tabindex="0" lang="vi-VN">{text}</pre> })}
        <Show when=move || source.with_value(|source| source.original_text.is_none())><p>{move || copy().original_binary}</p></Show>
        <crate::download::DownloadOriginal namespace="adaptation" id=source.with_value(|r|r.id.clone()) file_name=source.with_value(|r|r.metadata.file_name.clone()) sha256=source.with_value(|r|r.sha256.clone()) english/>

        {match source.with_value(|source| source.outcome.clone()) {
            ImportOutcome::Parsed { extraction } => view! { <ExtractionPreview extraction=extraction english=english namespace="adaptation" /> }.into_any(),
            ImportOutcome::Failed { error } => view! { <p>{move || messages::import_diagnostic(&error.code, english.get())}</p> }.into_any(),
        }}
    }
}
