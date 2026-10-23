//! Leptos shell for private manuscript import; transport and durable decisions have other owners.
use super::{
    optional_metadata, page_bounds, warning_on_page, ImportEditor, ImportIntent, ImportStatus,
    MAX_FILE_BYTES,
};
use crate::contract::{Extraction, ImportFormat, ImportOutcome, ImportResponse};
use crate::{
    api::{StudioContext, StudioHttp},
    messages,
    view::operation_id,
};
use cantos_ui::{Icon, IconName};
use leptos::{prelude::*, task::spawn_local};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

fn dispatch_import(state: RwSignal<ImportEditor>, api: StudioHttp, intent: ImportIntent) {
    let api = api.snapshot();
    spawn_local(async move {
        match api.import(&intent.request).await {
            Ok(response) => {
                let _ = state.try_update(|editor| *editor = editor.imported(&intent, response));
            }
            Err(error) => {
                let _ = state.try_update(|editor| {
                    if editor.pending.as_ref() == Some(&intent) {
                        *editor = editor.failed(&error);
                    }
                });
            }
        }
    });
}

fn format_value(format: ImportFormat) -> &'static str {
    match format {
        ImportFormat::Txt => "txt",
        ImportFormat::Markdown => "markdown",
        ImportFormat::Docx => "docx",
        ImportFormat::ScriptIr => "script_ir",
    }
}

fn selected_format(value: &str) -> ImportFormat {
    match value {
        "markdown" => ImportFormat::Markdown,
        "docx" => ImportFormat::Docx,
        "script_ir" => ImportFormat::ScriptIr,
        _ => ImportFormat::Txt,
    }
}

#[component]
pub fn ManuscriptImport(
    actor: Signal<String>,
    english: Signal<bool>,
    session_pending: Signal<bool>,
    dirty: RwSignal<bool>,
    activity: RwSignal<bool>,
) -> impl IntoView {
    let state = RwSignal::new(ImportEditor::default());
    let api = expect_context::<StudioContext>().api;
    let file_input = NodeRef::<leptos::html::Input>::new();
    let copy = move || messages::import_copy(english.get());
    Effect::new(move |_| {
        let actor = actor.get();
        let previous = state.get_untracked();
        let next = previous.signed_in_as(actor);
        if previous.actor != next.actor {
            if let Some(input) = file_input.get_untracked() {
                input.set_value("");
            }
        }
        state.set(next);
    });
    Effect::new(move |_| {
        dirty.set(state.with(ImportEditor::has_unsaved));
        activity.set(state.with(|editor| editor.busy));
    });
    let blocked = move || session_pending.get() || state.with(ImportEditor::blocked);
    let save_blocked = move || {
        state.with(|editor| {
            session_pending.get()
                || editor.blocked()
                || editor.actor.is_empty()
                || editor.bytes.is_none()
                || editor.metadata.reference.trim().is_empty()
        })
    };
    let read_blocked = move || blocked() || state.with(|editor| editor.actor.is_empty());

    view! {
        <section class="manuscript-import" aria-labelledby="import-title">
            <h2 id="import-title">{move || copy().title}</h2>
            <p id="import-help" class="help">{move || copy().help}</p>
            <form on:submit=move |event| {
                event.prevent_default();
                if save_blocked() { return; }
                match operation_id() {
                    Ok(operation) => {
                        if let Some((saving, intent)) = state.get_untracked().start_import(operation) {
                            state.set(saving);
                            dispatch_import(state, api, intent);
                        }
                    }
                    Err(error) => state.update(|editor| *editor = editor.failed(&error)),
                }
            }>
                <label for="import-file">{move || copy().file}</label>
                <input class="cui-field-control" id="import-file" node_ref=file_input type="file" accept=".txt,.md,.markdown,.docx,.json" disabled=blocked
                    aria-describedby="import-help import-selected-file import-reason import-field-findings" on:change=move |event| {
                        if blocked() {return;}
                        let Some(input) = event.target().and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok()) else { return; };
                        let Some(file) = input.files().and_then(|files| files.get(0)) else { return; };
                        // Reject before materializing an ArrayBuffer; the server repeats all bounds.
                        let size = file.size();
                        let bounded_size = if size.is_finite() && size >= 0.0 && size <= MAX_FILE_BYTES as f64 {
                            size as u64
                        } else {
                            (MAX_FILE_BYTES as u64) + 1
                        };
                        let Some(reading) = state.get_untracked().start_file(file.name(), bounded_size) else { return; };
                        let ticket = reading.ticket;
                        let should_read = reading.status == ImportStatus::ReadingFile;
                        state.set(reading);
                        if should_read {
                            spawn_local(async move {
                                let bytes = JsFuture::from(file.array_buffer()).await.ok().map(|buffer| {
                                    js_sys::Uint8Array::new(&buffer).to_vec()
                                });
                                let _ = state.try_update(|editor| *editor = editor.file_loaded(ticket, bytes));
                            });
                        }
                    } />
                <p class="identity">{move || copy().file_name}<output id="import-selected-file">{move || state.with(|editor| editor.visible_metadata().map(|metadata|metadata.file_name.clone()).unwrap_or_default())}</output></p>
                <label for="import-format">{move || copy().format}</label>
                <div class="cui-select">
                <select class="cui-field-control" id="import-format" disabled=blocked prop:value=move || state.with(|editor| editor.visible_metadata().map(|metadata|format_value(metadata.format)).unwrap_or("txt"))
                    on:change=move |event| state.update(|editor| *editor = editor.edit_metadata(|metadata| metadata.format = selected_format(&event_target_value(&event))))>
                    <option value="txt">{move || copy().txt}</option>
                    <option value="markdown">{move || copy().markdown}</option>
                    <option value="docx">{move || copy().docx}</option>
                    <option value="script_ir">{move || copy().script_ir}</option>
                </select>
                    <Icon name=IconName::ExpandMore/>
                </div>
                <label for="import-reference">{move || copy().reference}</label>
                <input class="cui-field-control" id="import-reference" disabled=blocked maxlength="2048" autocomplete="off" required aria-describedby="import-field-findings"
                    prop:value=move || state.with(|editor| editor.visible_metadata().map(|metadata|metadata.reference.clone()).unwrap_or_default())
                    on:input=move |event| state.update(|editor| *editor = editor.edit_metadata(|metadata| metadata.reference = event_target_value(&event))) />
                <div class="import-metadata">
                    <div>
                        <label for="import-rights-holder">{move || copy().rights_holder}</label>
                        <input class="cui-field-control" id="import-rights-holder" disabled=blocked maxlength="2048" autocomplete="off" aria-describedby="import-rights-help import-field-findings"
                            prop:value=move || state.with(|editor| editor.visible_metadata().and_then(|metadata|metadata.rights_holder.clone()).unwrap_or_default())
                            on:input=move |event| state.update(|editor| *editor = editor.edit_metadata(|metadata| metadata.rights_holder = optional_metadata(event_target_value(&event)))) />
                    </div>
                    <div>
                        <label for="import-permission">{move || copy().permission_evidence}</label>
                        <input class="cui-field-control" id="import-permission" disabled=blocked maxlength="2048" autocomplete="off" aria-describedby="import-rights-help import-field-findings"
                            prop:value=move || state.with(|editor| editor.visible_metadata().and_then(|metadata|metadata.permission_evidence.clone()).unwrap_or_default())
                            on:input=move |event| state.update(|editor| *editor = editor.edit_metadata(|metadata| metadata.permission_evidence = optional_metadata(event_target_value(&event)))) />
                    </div>
                    <div>
                        <label for="import-usage">{move || copy().usage_scope}</label>
                        <input class="cui-field-control" id="import-usage" disabled=blocked maxlength="2048" autocomplete="off" aria-describedby="import-rights-help import-field-findings"
                            prop:value=move || state.with(|editor| editor.visible_metadata().and_then(|metadata|metadata.usage_scope.clone()).unwrap_or_default())
                            on:input=move |event| state.update(|editor| *editor = editor.edit_metadata(|metadata| metadata.usage_scope = optional_metadata(event_target_value(&event)))) />
                    </div>
                </div>
                <p id="import-rights-help" class="help">{move || copy().rights_help}</p>
                <div class="toolbar">
                    <button class="cui-button" data-variant="primary" type="submit" disabled=save_blocked aria-describedby="import-reason">{move || copy().save}</button>
                    <Show when=move || state.with(|editor| editor.pending.is_some() && !editor.busy)>
                        <button class="cui-button" data-variant="secondary" type="button" aria-describedby="import-reason" disabled=move || state.with(|editor| editor.pending.as_ref().is_some_and(|intent| intent.actor != editor.actor))
                            on:click=move |_| {
                                if session_pending.get_untracked() {return;}
                                if let Some((saving, intent)) = state.get_untracked().retry() {
                                    state.set(saving);
                                    dispatch_import(state, api, intent);
                                }
                            }>{move || copy().retry}</button>
                    </Show>
                </div>
                <p id="import-reason" class="help">{move || state.with(|editor| {
                    if editor.pending.is_some() { copy().pending } else if save_blocked() { copy().required } else { "" }
                })}</p>
                <Show when=move || state.with(|editor| editor.pending.is_some())>
                    <p class="identity">{move || copy().operation}<output>{move || state.with(|editor| editor.pending.as_ref().map(|intent| intent.request.metadata.operation_id.clone()).unwrap_or_default())}</output></p>
                </Show>
            </form>
            <form class="import-reopen" on:submit=move |event| {
                event.prevent_default();
                if read_blocked() {return;}
                if let Some(reading) = state.get_untracked().start_read() {
                    let id = reading.source_id.clone();
                    let actor = reading.actor.clone();
                    let ticket = reading.ticket;
                    state.set(reading);
                    let api = api.snapshot();
                    spawn_local(async move {
                        match api.read_import(&id).await {
                            Ok(response) => { let _ = state.try_update(|editor| *editor = editor.loaded(ticket, &actor, response)); }
                            Err(error) => { let _ = state.try_update(|editor| *editor = editor.read_failed(ticket, &actor, &error)); }
                        }
                    });
                }
            }>
                <label for="import-source-id">{move || copy().source_id}</label>
                <div class="toolbar">
                    <input class="cui-field-control" id="import-source-id" maxlength="36" disabled=blocked autocomplete="off" spellcheck="false"
                        prop:value=move || state.with(|editor| if editor.can_edit() {editor.source_id.clone()} else {String::new()})
                        on:input=move |event| state.update(|editor| *editor = editor.select_source(event_target_value(&event))) />
                    <button class="cui-button" data-variant="secondary" type="submit" disabled=read_blocked aria-describedby="import-reason">{move || copy().open}</button>
                </div>
            </form>
            <p id="import-status" class="status" role="status" aria-live="polite">{move || state.with(|editor| messages::import_status(&editor.status, english.get()))}</p>
            <div id="import-field-findings">
                <Show when=move || state.with(|editor| !editor.issues.is_empty())>
                    <ul class="import-warnings">
                        {move || state.with(|editor| editor.issues.clone()).into_iter().map(|issue| {
                            let issue = StoredValue::new(issue);
                            view! {
                                <li>
                                    <strong>{move || issue.with_value(|issue| messages::import_field(&issue.path, english.get()))}</strong>
                                    <p>{move || issue.with_value(|issue| messages::import_field_rule(&issue.rule, english.get()))}</p>
                                    <small><code>{issue.with_value(|issue| issue.path.clone())}</code>" · "<code>{issue.with_value(|issue| issue.rule.clone())}</code></small>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                </Show>
            </div>
            {move || state.with(|editor| editor.stored.clone()).map(|record| view! { <SourcePreview record=record english=english session_pending=session_pending /> })}
        </section>
    }
}

#[component]
fn SourcePreview(
    record: ImportResponse,
    english: Signal<bool>,
    session_pending: Signal<bool>,
) -> impl IntoView {
    let record = StoredValue::new(record);
    let copy = move || messages::import_copy(english.get());
    view! {
        <section aria-labelledby="import-stored-title" class="import-stored">
            <h3 id="import-stored-title">{move || copy().stored}</h3>
            <dl class="source-identity">
                <dt>{move || copy().source_id}</dt><dd>{record.with_value(|record| record.id.clone())}</dd>
                <dt>{move || copy().file_name}</dt><dd>{record.with_value(|record| record.metadata.file_name.clone())}</dd>
                <dt>{move || copy().format}</dt><dd>{move || record.with_value(|record| messages::import_format(record.metadata.format, english.get()))}</dd>
                <dt>{move || copy().reference}</dt><dd>{record.with_value(|record| record.metadata.reference.clone())}</dd>
                <dt>{move || copy().rights_holder}</dt><dd>{move || record.with_value(|record| record.metadata.rights_holder.clone().unwrap_or_else(|| copy().absent.into()))}</dd>
                <dt>{move || copy().permission_evidence}</dt><dd>{move || record.with_value(|record| record.metadata.permission_evidence.clone().unwrap_or_else(|| copy().absent.into()))}</dd>
                <dt>{move || copy().usage_scope}</dt><dd>{move || record.with_value(|record| record.metadata.usage_scope.clone().unwrap_or_else(|| copy().absent.into()))}</dd>
                <dt>{move || copy().checksum}</dt><dd><code>{record.with_value(|record| record.sha256.clone())}</code></dd>
                <dt>{move || copy().byte_len}</dt><dd>{record.with_value(|record| record.byte_len)}</dd>
                <dt>{move || copy().imported_by}</dt><dd>{record.with_value(|record| record.imported_by.clone())}</dd>
                <dt>{move || copy().recorded_at}</dt><dd>{record.with_value(|record| record.recorded_at.clone())}</dd>
            </dl>
            <p class="help">{move || copy().rights_help}</p>
            <div class="import-comparison">
                <section aria-labelledby="import-original-title">
                    <h3 id="import-original-title">{move || copy().original}</h3>
                    {record.with_value(|record| record.original_text.clone()).map(|text| view! { <pre tabindex="0" lang="vi-VN">{text}</pre> })}
                    <Show when=move || record.with_value(|record| record.original_text.is_none())><p>{move || copy().binary}</p></Show>
                    <crate::download::DownloadOriginal namespace="import" id=record.with_value(|r|r.id.clone()) file_name=record.with_value(|r|r.metadata.file_name.clone()) sha256=record.with_value(|r|r.sha256.clone()) english blocked=session_pending/>

                </section>
                <section aria-labelledby="import-parsed-title">
                    <h3 id="import-parsed-title">{move || copy().parsed}</h3>
                    <p class="help">{move || copy().parsed_help}</p>
                    {match record.with_value(|record| record.outcome.clone()) {
                        ImportOutcome::Parsed { extraction } => view! { <ExtractionPreview extraction=extraction english=english /> }.into_any(),
                        ImportOutcome::Failed { error } => {
                            let code = StoredValue::new(error.code);
                            view! {
                                <p>{move || copy().failed}</p>
                                <p>{move || code.with_value(|code| messages::import_diagnostic(code, english.get()))}</p>
                                <p>{move || copy().support_code}" · "<code>{code.get_value()}</code></p>
                                {error.offset.map(|offset| view! { <p>{move || copy().byte_offset}<output>{offset}</output></p> })}
                            }.into_any()
                        }
                    }}
                </section>
            </div>
        </section>
    }
}

#[component]
pub(crate) fn ExtractionPreview(
    extraction: Extraction,
    english: Signal<bool>,
    #[prop(into, default = "import".to_owned())] namespace: String,
) -> impl IntoView {
    let total = extraction.blocks.len();
    let page = RwSignal::new(0usize);
    let extraction = StoredValue::new(extraction);
    let copy = move || messages::import_copy(english.get());
    let range_id = StoredValue::new(format!("{namespace}-block-range"));
    view! {
        <p class="identity">{move || copy().extractor}<output>{extraction.with_value(|extraction| extraction.extractor_version.clone())}</output></p>
        <div class="toolbar">
            <button class="cui-button" data-variant="secondary" type="button" aria-describedby=move || range_id.get_value() disabled=move || page.get() == 0 on:click=move |_| page.update(|page| *page = page.saturating_sub(1))>{move || copy().previous_blocks}</button>
            <button class="cui-button" data-variant="secondary" type="button" aria-describedby=move || range_id.get_value() disabled=move || page_bounds(page.get(), total).end == total on:click=move |_| {
                if page_bounds(page.get_untracked(), total).end < total { page.update(|page| *page += 1); }
            }>{move || copy().next_blocks}</button>
        </div>
        <p id=move || range_id.get_value() role="status" aria-live="polite">{move || messages::import_block_range(page.get(), total, english.get())}</p>
        <h4>{move || copy().warnings}</h4>
        <Show when=move || extraction.with_value(|extraction| !extraction.warnings.iter().any(|warning| warning_on_page(warning.block, page.get(), total)))><p>{move || copy().no_warnings}</p></Show>
        <ul class="import-warnings">
            {move || extraction.with_value(|extraction| extraction.warnings.iter().filter(|warning| warning_on_page(warning.block, page.get(), total)).cloned().collect::<Vec<_>>()).into_iter().map(|warning| {
                let code = StoredValue::new(warning.code);
                view! {
                    <li>
                        <p>{move || code.with_value(|code| messages::import_diagnostic(code, english.get()))}</p>
                        <small>{move || copy().support_code}" · "<code>{code.get_value()}</code></small>
                        {warning.block.map(|block| view! { <small>" · "{move || copy().block}" "{block}</small> })}
                    </li>
                }
            }).collect_view()}
        </ul>
        <ol class="import-blocks" start=move || page_bounds(page.get(), total).start + 1>
            {move || extraction.with_value(|extraction| extraction.blocks[page_bounds(page.get(), total)].to_vec()).into_iter().map(|block| {
                let block = StoredValue::new(block);
                view! {
                    <li>
                        <p class="block-kind">{move || block.with_value(|block| messages::import_kind(block.kind, english.get()))}<small>" · "{move || copy().block}" "{block.with_value(|block| block.index)}</small></p>
                        <p class="block-text" lang="vi-VN">{block.with_value(|block| block.text.clone())}</p>
                        <dl class="block-metadata">
                            <dt>{move || copy().speaker}</dt><dd lang=move || block.with_value(|block| if block.speaker.is_some() || !english.get() { "vi-VN" } else { "en" })>{move || block.with_value(|block| block.speaker.clone().unwrap_or_else(|| copy().unknown.into()))}</dd>
                            <dt>{move || copy().scene}</dt><dd lang=move || block.with_value(|block| if block.scene.is_some() || !english.get() { "vi-VN" } else { "en" })>{move || block.with_value(|block| block.scene.clone().unwrap_or_else(|| copy().unknown.into()))}</dd>
                            <dt>{move || copy().cue}</dt><dd>{move || block.with_value(|block| block.cue_kind.map(|kind| messages::import_cue(kind, english.get())).unwrap_or(copy().unknown))}</dd>
                        </dl>
                    </li>
                }
            }).collect_view()}
        </ol>
        {extraction.with_value(|extraction| extraction.script_json.clone()).map(|json| view! {
            <details><summary>{move || copy().script_json}</summary><pre tabindex="0" lang="vi-VN">{json}</pre></details>
        })}
    }
}
