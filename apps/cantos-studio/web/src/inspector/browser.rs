//! Leptos shell for private source/history reads and explicit owner-review submission.
use super::{
    should_read_import, source_ids, Inspector, PreservedSource, ReadKind, ReviewIntent, SourceRead,
};
use crate::{
    api::{StudioContext, StudioHttp},
    editor::{error_status, Editor},
    import::ExtractionPreview,
    messages::{
        self,
        workspace::{text, Key},
    },
    view::operation_id,
};
use cantos_contracts::{
    ImportOutcome, ImportResponse, ReviewResponse, RevisionResponse, SourceResponse,
};
use leptos::{prelude::*, task::spawn_local};

fn set_inspector(
    inspector: RwSignal<Inspector>,
    activity: Option<RwSignal<bool>>,
    next: Inspector,
) {
    if let Some(activity) = activity {
        let _ = activity.try_set(next.has_activity());
    }
    let _ = inspector.try_set(next);
}

fn update_inspector(
    inspector: RwSignal<Inspector>,
    editor: RwSignal<Editor>,
    activity: Option<RwSignal<bool>>,
    reduce: impl FnOnce(&Inspector, &Editor) -> Inspector,
) {
    let (Some(current), Some(before)) = (editor.try_get_untracked(), inspector.try_get_untracked())
    else {
        return;
    };
    let next = reduce(&before.observe(&current), &current);
    set_inspector(inspector, activity, next);
}

fn read(
    inspector: RwSignal<Inspector>,
    editor: RwSignal<Editor>,
    activity: Option<RwSignal<bool>>,
    api: StudioHttp,
    kind: ReadKind,
) {
    let current = editor.get_untracked();
    let Some((reading, ticket)) = inspector
        .get_untracked()
        .observe(&current)
        .start_read(&current, kind)
    else {
        return;
    };
    set_inspector(inspector, activity, reading);
    spawn_local(async move {
        match &ticket.kind {
            ReadKind::History { after } => {
                let result = api.history(&ticket.context.script, *after, 20).await;
                update_inspector(inspector, editor, activity, |state, _| match result {
                    Ok(history) => state.history_loaded(&ticket, history),
                    Err(error) => state.read_failed(&ticket, error.code),
                });
            }
            ReadKind::Revision { revision } => {
                let result = api.read_revision(&ticket.context.script, *revision).await;
                update_inspector(inspector, editor, activity, |state, _| match result {
                    Ok(revision) => state.revision_loaded(&ticket, revision),
                    Err(error) => state.read_failed(&ticket, error.code),
                });
            }
            ReadKind::Sources { ids } => {
                let mut sources = Vec::with_capacity(ids.len());
                for id in ids {
                    let (result, import_fallback) =
                        match api.read_source(&ticket.context.script, id).await {
                            Ok(source) => (Ok(PreservedSource::Text(source)), false),
                            Err(error) if should_read_import(id, &error.code) => (
                                api.read_import(id)
                                    .await
                                    .map(|source| PreservedSource::Imported(Box::new(source)))
                                    .map_err(|error| error.code),
                                true,
                            ),
                            Err(error) => (Err(error.code), false),
                        };
                    sources.push(SourceRead {
                        id: id.clone(),
                        result,
                        import_fallback,
                    });
                }
                update_inspector(inspector, editor, activity, |state, _| {
                    state.sources_loaded(&ticket, sources)
                });
            }
        }
    });
}

fn review(
    inspector: RwSignal<Inspector>,
    editor: RwSignal<Editor>,
    activity: Option<RwSignal<bool>>,
    api: StudioHttp,
    intent: ReviewIntent,
) {
    spawn_local(async move {
        let result = api.review(&intent.snapshot.script, &intent.request).await;
        update_inspector(inspector, editor, activity, |state, _| match result {
            Ok(response) => state.review_loaded(&intent, response),
            Err(error) => state.review_failed(&intent, error.code),
        });
    });
}

#[component]
pub fn RevisionInspector(
    state: RwSignal<Editor>,
    english: RwSignal<bool>,
    #[prop(optional)] activity: Option<RwSignal<bool>>,
    #[prop(optional)] draft_activity: Option<RwSignal<bool>>,
) -> impl IntoView {
    let inspector = RwSignal::new(Inspector::default());
    let api = expect_context::<StudioContext>().api;
    let copy = move || messages::copy(english.get());
    let workspace = move |key| text(key, english.get());
    let import_copy = move || messages::import_copy(english.get());
    let draft_active = move || draft_activity.is_some_and(|activity| activity.get());
    Effect::new(move |_| {
        let current = state.get();
        let active = draft_active();
        let next =
            inspector.with_untracked(|inspector| inspector.observe_workspace(&current, active));
        set_inspector(inspector, activity, next);
    });
    let history = Memo::new(move |_| inspector.with(|inspector| inspector.history.clone()));
    let viewed = Memo::new(move |_| inspector.with(|inspector| inspector.viewed.clone()));
    let sources = Memo::new(move |_| inspector.with(|inspector| inspector.sources.clone()));
    let reviewed = Memo::new(move |_| inspector.with(|inspector| inspector.reviewed.clone()));
    let remote = Memo::new(move |_| state.with(|editor| editor.remote.clone()));
    let read_blocked =
        move || inspector.with(|inspector| state.with(|editor| !inspector.can_read(editor)));
    let review_blocked = move || {
        inspector.with(|inspector| {
            state.with(|editor| {
                !inspector
                    .observe_workspace(editor, draft_active())
                    .can_review(editor)
            })
        })
    };
    let rebase_blocked = move || {
        draft_active()
            || inspector.with(Inspector::has_activity)
            || state.with(|editor| {
                editor.busy
                    || editor.pending.is_some()
                    || !editor.can_edit()
                    || editor
                        .remote
                        .as_ref()
                        .is_none_or(|remote| remote.revision == editor.base)
            })
    };

    view! {
        <div class="revision-inspector">
            <p id="inspector-permission-help" class="help">{move || workspace(Key::PermissionHelp)}</p>
            <p class="status" role="status" aria-live="polite">
                {move || inspector.with(|inspector| {
                    if let Some(error) = &inspector.error {
                        messages::status(error_status(error), english.get())
                    } else if inspector.sending_review || inspector.reading.is_some() {
                        copy().busy
                    } else { "" }
                })}
            </p>
            <Show when=move || inspector.with(|inspector| inspector.pending.is_some())>
                <section class="inspector-pending" aria-describedby="inspector-pending-help">
                    <p id="inspector-pending-help" class="help">{move || workspace(Key::PendingReview)}</p>
                    <dl class="source-identity">
                        <dt>{move || import_copy().operation}</dt><dd><code>{move || inspector.with(|inspector| inspector.pending.as_ref().map(|intent| intent.request.operation_id.clone()).unwrap_or_default())}</code></dd>
                        <dt>{move || copy().actor}</dt><dd>{move || inspector.with(|inspector| inspector.pending.as_ref().map(|intent| intent.snapshot.actor.clone()).unwrap_or_default())}</dd>
                        <dt>{move || copy().script}</dt><dd>{move || inspector.with(|inspector| inspector.pending.as_ref().map(|intent| intent.snapshot.script.clone()).unwrap_or_default())}</dd>
                        <dt>{move || copy().base}</dt><dd>{move || inspector.with(|inspector| inspector.pending.as_ref().map(|intent| intent.request.revision.to_string()).unwrap_or_default())}</dd>
                    </dl>
                    <button type="button" aria-disabled=move || inspector.with(|inspector| !inspector.can_retry()) aria-describedby="inspector-pending-help inspector-permission-help" on:click=move |_| {
                        let current = state.get_untracked();
                        let Some((sending, intent)) = inspector.get_untracked().observe(&current).retry() else { return; };
                        set_inspector(inspector, activity, sending);
                        review(inspector, state, activity, api, intent);
                    }>{move || copy().retry}</button>
                </section>
            </Show>
            <section aria-labelledby="inspector-source-title">
                <h3 id="inspector-source-title">{move || workspace(Key::Source)}</h3>
                <p id="inspector-source-help" class="help">{move || workspace(Key::SourceHelp)}</p>
                <button type="button" aria-disabled=move || read_blocked() || remote.get().is_none_or(|remote| source_ids(&remote.script_json).is_empty()) aria-describedby="inspector-source-help inspector-permission-help inspector-pending-help" on:click=move |_| {
                    let Some(remote) = remote.get_untracked() else { return; };
                    read(inspector, state, activity, api, ReadKind::Sources { ids: source_ids(&remote.script_json) });
                }>{move || workspace(Key::OpenSource)}</button>
                <For each=move || sources.with(|sources| sources.iter().map(|source| source.id.clone()).collect::<Vec<_>>()) key=|id| id.clone() let:id>
                    {
                        let id = StoredValue::new(id);
                        view! {
                            {move || sources.with(|sources| sources.iter().find(|source| source.id == id.get_value()).cloned()).map(|source| match source.result {
                                Ok(PreservedSource::Text(response)) => view! { <SourceDetails source=response english=english /> }.into_any(),
                                Ok(PreservedSource::Imported(response)) => view! { <ImportedSourceDetails source=*response english=english /> }.into_any(),
                                Err(error) => view! {
                                    <p><code>{source.id}</code>" · "{move || messages::status(error_status(&error), english.get())}</p>
                                    <Show when=move || source.import_fallback><p class="help">{move || workspace(Key::BinarySourceHelp)}</p></Show>
                                }.into_any(),
                            })}
                        }
                    }
                </For>
            </section>
            <section aria-labelledby="inspector-history-title">
                <h3 id="inspector-history-title">{move || workspace(Key::History)}</h3>
                <p id="inspector-history-help" class="help">{move || workspace(Key::HistoryHelp)}</p>
                <div class="toolbar">
                    <button type="button" aria-disabled=read_blocked aria-describedby="inspector-history-help inspector-permission-help inspector-pending-help" on:click=move |_| read(inspector, state, activity, api, ReadKind::History { after: 0 })>{move || workspace(Key::LoadHistory)}</button>
                    <Show when=move || history.get().is_some_and(|history| history.next_after.is_some())>
                        <button type="button" aria-disabled=read_blocked aria-describedby="inspector-history-help inspector-permission-help" on:click=move |_| {
                            if let Some(after) = history.get_untracked().and_then(|history| history.next_after) {
                                read(inspector, state, activity, api, ReadKind::History { after });
                            }
                        }>{move || workspace(Key::MoreHistory)}</button>
                    </Show>
                </div>
                <Show when=move || history.get().is_none()><p class="help">{move || workspace(Key::NoHistory)}</p></Show>
                <ul class="revision-history">
                    <For each=move || history.get().map(|history| history.revisions).unwrap_or_default() key=|revision| (revision.script_id.clone(), revision.revision) let:revision>
                        {
                            let version = revision.revision;
                            let revision = StoredValue::new(revision);
                            view! {
                                <li>
                                    <p class="identity">{revision.with_value(|revision| format!("{} / {} · {} · {}", revision.script_id, revision.revision, revision.accepted_by, revision.accepted_at))}</p>
                                    <code>{revision.with_value(|revision| revision.export_digest.clone())}</code>
                                    <button type="button" aria-disabled=read_blocked aria-describedby="inspector-history-help inspector-permission-help" on:click=move |_| read(inspector, state, activity, api, ReadKind::Revision { revision: version })>{move || workspace(Key::OpenVersion)}" "{version}</button>
                                    {move || history.get().map(|history| history.reviews.into_iter().filter(|review| review.revision == version).map(|review| view! { <ReviewDetails review=review english=english /> }).collect_view())}
                                </li>
                            }
                        }
                    </For>
                </ul>
                {move || viewed.get().map(|revision| view! { <RevisionDetails revision=revision english=english /> })}
            </section>
            <section aria-labelledby="inspector-saved-title">
                <h3 id="inspector-saved-title">{move || copy().preview}</h3>
                {move || remote.get().map(|revision| view! { <RevisionDetails revision=revision english=english /> })}
                <button type="button" aria-disabled=rebase_blocked aria-describedby="inspector-history-help inspector-permission-help inspector-pending-help" on:click=move |_| {
                    if !rebase_blocked() { state.update(|editor| *editor = editor.rebase()); }
                }>{move || copy().rebase}</button>
                <p id="inspector-review-help" class="help">{move || workspace(Key::ReviewHelp)}</p>
                <label class="adaptation-confirm" for="inspector-review-confirm">
                    <input id="inspector-review-confirm" type="checkbox" disabled=move || draft_active() || inspector.with(Inspector::has_activity) || state.with(|editor| editor.is_dirty() || editor.remote.as_ref().is_none_or(|remote| remote.revision != editor.base) || editor.base == 0 || editor.actor.is_empty() || editor.busy || editor.pending.is_some() || !editor.can_edit())
                        aria-describedby="inspector-review-help inspector-permission-help" prop:checked=move || inspector.with(|inspector| inspector.confirmation.is_some())
                        on:change=move |event| {
                            let checked = event_target_checked(&event);
                            update_inspector(inspector, state, activity, |inspector, editor| inspector.observe_workspace(editor, draft_active()).confirm(editor, checked && !draft_active()));
                        } />
                    <span>{move || workspace(Key::ReviewSavedConfirm)}</span>
                </label>
                <button type="button" aria-disabled=review_blocked aria-describedby="inspector-review-help inspector-permission-help inspector-pending-help" on:click=move |_| {
                    if review_blocked() { return; }
                    match operation_id() {
                        Ok(operation) => {
                            let current = state.get_untracked();
                            if let Some((sending, intent)) = inspector.get_untracked().observe_workspace(&current, draft_active()).start_review(&current, operation) {
                                set_inspector(inspector, activity, sending);
                                review(inspector, state, activity, api, intent);
                            }
                        }
                        Err(error) => update_inspector(inspector, state, activity, |inspector, _| inspector.local_error(&error)),
                    }
                }>{move || workspace(Key::ReviewVersion)}</button>
                {move || reviewed.get().map(|review| view! { <ReviewDetails review=review english=english /> })}
            </section>
        </div>
    }
}

#[component]
fn RevisionDetails(revision: RevisionResponse, english: RwSignal<bool>) -> impl IntoView {
    let revision = StoredValue::new(revision);
    view! {
        <details class="revision-export">
            <summary>{move || text(Key::Stored, english.get())}" · "{revision.with_value(|revision| revision.revision)}</summary>
            <p class="identity">{revision.with_value(|revision| format!("{} / {} · {} · {}", revision.script_id, revision.revision, revision.accepted_by, revision.accepted_at))}</p>
            <p class="identity"><code>{revision.with_value(|revision| revision.content_digest.clone())}</code></p>
            <p class="identity"><code>{revision.with_value(|revision| revision.export_digest.clone())}</code></p>
            <pre tabindex="0" lang="vi-VN">{revision.with_value(|revision| revision.script_json.clone())}</pre>
        </details>
    }
}

#[component]
fn ReviewDetails(review: ReviewResponse, english: RwSignal<bool>) -> impl IntoView {
    let review = StoredValue::new(review);
    view! {
        <p class="review-record">{move || text(Key::Reviewed, english.get())}" "{review.with_value(|review| review.revision)}" · "{review.with_value(|review| review.reviewed_by.clone())}" · "{review.with_value(|review| review.reviewed_at.clone())}</p>
        <details>
            <summary>{move || messages::import_copy(english.get()).operation}</summary>
            <p class="identity"><code>{review.with_value(|review| review.id.clone())}</code>" · "<code>{review.with_value(|review| review.operation_id.clone())}</code></p>
            <p class="identity"><code>{review.with_value(|review| review.content_digest.clone())}</code></p>
            <p class="identity"><code>{review.with_value(|review| review.export_digest.clone())}</code></p>
        </details>
    }
}

#[component]
fn SourceDetails(source: SourceResponse, english: RwSignal<bool>) -> impl IntoView {
    let source = StoredValue::new(source);
    let copy = move || messages::import_copy(english.get());
    view! {
        <details class="source-record">
            <summary>{source.with_value(|source| source.reference.clone())}</summary>
            <dl class="source-identity">
                <dt>{move || copy().source_id}</dt><dd><code>{source.with_value(|source| source.id.clone())}</code></dd>
                <dt>{move || copy().reference}</dt><dd lang="vi-VN">{source.with_value(|source| source.reference.clone())}</dd>
                <dt>{move || copy().checksum}</dt><dd><code>{source.with_value(|source| source.sha256.clone())}</code></dd>
                <dt>{move || copy().recorded_at}</dt><dd>{source.with_value(|source| source.recorded_at.clone())}</dd>
            </dl>
            <pre tabindex="0" lang="vi-VN">{source.with_value(|source| source.original_text.clone())}</pre>
        </details>
    }
}

#[component]
fn ImportedSourceDetails(source: ImportResponse, english: RwSignal<bool>) -> impl IntoView {
    let api = expect_context::<StudioContext>().api;
    let download = api.original_import_path(&source.id).ok();
    let namespace = format!("inspector-{}", source.id);
    let source = StoredValue::new(source);
    let copy = move || messages::import_copy(english.get());
    view! {
        <details class="source-record">
            <summary>{source.with_value(|source| source.metadata.reference.clone())}</summary>
            <p class="help">{move || text(Key::BinarySourceHelp, english.get())}</p>
            <dl class="source-identity">
                <dt>{move || copy().source_id}</dt><dd><code>{source.with_value(|source| source.id.clone())}</code></dd>
                <dt>{move || copy().reference}</dt><dd lang="vi-VN">{source.with_value(|source| source.metadata.reference.clone())}</dd>
                <dt>{move || copy().rights_holder}</dt><dd lang="vi-VN">{move || source.with_value(|source| source.metadata.rights_holder.clone().unwrap_or_else(|| copy().absent.into()))}</dd>
                <dt>{move || copy().permission_evidence}</dt><dd lang="vi-VN">{move || source.with_value(|source| source.metadata.permission_evidence.clone().unwrap_or_else(|| copy().absent.into()))}</dd>
                <dt>{move || copy().usage_scope}</dt><dd lang="vi-VN">{move || source.with_value(|source| source.metadata.usage_scope.clone().unwrap_or_else(|| copy().absent.into()))}</dd>
                <dt>{move || copy().checksum}</dt><dd><code>{source.with_value(|source| source.sha256.clone())}</code></dd>
                <dt>{move || copy().recorded_at}</dt><dd>{source.with_value(|source| source.recorded_at.clone())}</dd>
                <dt>{move || copy().imported_by}</dt><dd>{source.with_value(|source| source.imported_by.clone())}</dd>
            </dl>
            {source.with_value(|source| source.original_text.clone()).map(|original| view! { <pre tabindex="0" lang="vi-VN">{original}</pre> })}
            <Show when=move || source.with_value(|source| source.original_text.is_none())><p>{move || copy().binary}</p></Show>
            {download.map(|path| view! { <a class="download-original" href=path download=source.with_value(|source| source.metadata.file_name.clone())>{move || copy().download}</a> })}
            {match source.with_value(|source| source.outcome.clone()) {
                ImportOutcome::Parsed { extraction } => view! { <ExtractionPreview extraction=extraction english=english namespace=namespace /> }.into_any(),
                ImportOutcome::Failed { error } => view! { <p>{move || messages::import_diagnostic(&error.code, english.get())}</p> }.into_any(),
            }}
        </details>
    }
}
