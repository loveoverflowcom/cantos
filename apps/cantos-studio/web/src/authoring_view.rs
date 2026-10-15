//! Native, buffered fields over the lossless authoring core. Backend gates stay in the shell.
use crate::{
    authoring::{
        AuthoringError, ChangeKind, Document, Field, History, InputActivity, PronunciationField,
        Scene, Target,
    },
    messages::authoring::{
        authoring_copy, choice, item_action, pronunciation_remove, selected_choices,
        validation_rule, ItemAction,
    },
};
use cantos_contracts::FieldIssue;
use cantos_ui::{FieldError, FieldLabel, Icon, IconName};
use leptos::context::Provider;
use leptos::prelude::*;

#[derive(Clone, Copy)]
struct AuthoringContext {
    document: Memo<Option<Document>>,
    apply: Callback<Document, bool>,
    english: RwSignal<bool>,
    readonly: Signal<bool>,
    issues: Signal<Vec<FieldIssue>>,
    activity: RwSignal<bool>,
    pending_fields: RwSignal<InputActivity>,
    notice: RwSignal<Option<AuthoringError>>,
    movement: RwSignal<Option<(String, usize, String)>>,
    namespace: &'static str,
}

impl AuthoringContext {
    fn commit(self, result: Result<Document, AuthoringError>) -> bool {
        match result {
            Ok(document) => self.apply.run(document),
            Err(error) => {
                self.notice.set(Some(error));
                false
            }
        }
    }

    fn change(self, edit: impl FnOnce(&Document) -> Result<Document, AuthoringError>) -> bool {
        if self.readonly.get_untracked() {
            return false;
        }
        self.document
            .get_untracked()
            .is_some_and(|document| self.commit(edit(&document)))
    }

    fn structural_change(
        self,
        edit: impl FnOnce(&Document) -> Result<Document, AuthoringError>,
    ) -> bool {
        if self.activity.get_untracked() {
            return false;
        }
        self.change(edit)
    }
}

fn context() -> AuthoringContext {
    expect_context::<AuthoringContext>()
}

fn minted_id(prefix: &str) -> Result<String, AuthoringError> {
    crate::view::operation_id()
        .map(|id| format!("{prefix}-{id}"))
        .map_err(|_| AuthoringError::InvalidId(String::new()))
}

fn target_id(target: &Target) -> &str {
    match target {
        Target::Work => "work",
        Target::Adaptation => "adaptation",
        Target::Episode => "episode",
        Target::Character(id)
        | Target::Act(id)
        | Target::Scene(id)
        | Target::Dialogue(id)
        | Target::Cue(id) => id,
    }
}

fn scene(document: &Document, id: &str) -> Option<Scene> {
    document
        .acts()
        .into_iter()
        .flat_map(|act| act.scenes)
        .find(|scene| scene.id == id)
}

fn field_id(namespace: &str, target: &Target, field: Field) -> String {
    format!("{namespace}-{}-{field:?}", target_id(target))
}

fn matching_issue(document: &Document, target: &Target, field: Field, issue: &FieldIssue) -> bool {
    document
        .diagnostic_paths(target, field)
        .is_ok_and(|paths| paths.contains(&issue.path))
        || (matches!(target, Target::Character(_))
            && field == Field::Role
            && issue.path == "characters")
}

fn focus_field(id: &str) {
    if let Some(element) = leptos::leptos_dom::helpers::document().get_element_by_id(id) {
        let mut parent = element.parent_element();
        while let Some(element) = parent {
            if element.tag_name() == "DETAILS" {
                let _ = element.set_attribute("open", "");
            }
            parent = element.parent_element();
        }
        if let Some(element) = wasm_bindgen::JsCast::dyn_ref::<web_sys::HtmlElement>(&element) {
            let _ = element.focus();
        }
    }
}

fn finding_location(document: &Document, namespace: &str, issue: &FieldIssue) -> (Target, String) {
    let mut candidates = vec![
        (Target::Work, Field::Title, Target::Work),
        (Target::Episode, Field::Title, Target::Work),
        (Target::Adaptation, Field::Language, Target::Work),
    ];
    for character in document.characters() {
        let target = Target::Character(character.id);
        for field in [Field::Name, Field::Role, Field::Personality] {
            candidates.push((target.clone(), field, target.clone()));
        }
    }
    for act in document.acts() {
        let target = Target::Act(act.id);
        candidates.push((target.clone(), Field::Title, target));
        for scene in act.scenes {
            let pane = Target::Scene(scene.id.clone());
            candidates.push((pane.clone(), Field::Title, pane.clone()));
            for dialogue in scene.dialogues {
                let target = Target::Dialogue(dialogue.id.clone());
                for field in [
                    Field::Text,
                    Field::Speaker,
                    Field::Emotion,
                    Field::Intensity,
                ] {
                    candidates.push((target.clone(), field, pane.clone()));
                }
                for index in 0..dialogue.pronunciation_overrides.len() {
                    for (suffix, name) in [("surface", "surface"), ("replacement", "replacement")] {
                        let pointer = document.path(&target).unwrap_or_default();
                        let semantic = document
                            .diagnostic_paths(&target, Field::Text)
                            .ok()
                            .and_then(|paths| paths.into_iter().find(|path| !path.starts_with('/')))
                            .unwrap_or_default();
                        let semantic = semantic.strip_suffix("/text").unwrap_or(&semantic);
                        if issue.path
                            == format!("{pointer}/pronunciation_overrides/{index}/{suffix}")
                            || issue.path
                                == format!("{semantic}/pronunciation_overrides/{index}/{suffix}")
                        {
                            return (
                                pane.clone(),
                                format!("{namespace}-{}-pronunciation-{index}-{name}", dialogue.id),
                            );
                        }
                    }
                }
            }
            for cue in scene.sound_cues {
                let target = Target::Cue(cue.id);
                for field in [
                    Field::CueKind,
                    Field::CueDescription,
                    Field::CueDialogue,
                    Field::CueEdge,
                ] {
                    candidates.push((target.clone(), field, pane.clone()));
                }
            }
        }
    }
    candidates
        .into_iter()
        .find(|(target, field, _)| matching_issue(document, target, *field, issue))
        .map(|(target, field, pane)| (pane, field_id(namespace, &target, field)))
        .unwrap_or((Target::Work, format!("{namespace}-json")))
}

fn collection(document: &Document, target: &Target) -> Vec<String> {
    match target {
        Target::Character(_) => document
            .characters()
            .into_iter()
            .map(|row| row.id)
            .collect(),
        Target::Act(_) => document.acts().into_iter().map(|row| row.id).collect(),
        Target::Scene(id) => document
            .acts()
            .into_iter()
            .find(|act| act.scenes.iter().any(|row| row.id == *id))
            .map(|act| act.scenes.into_iter().map(|row| row.id).collect())
            .unwrap_or_default(),
        Target::Dialogue(id) => document
            .acts()
            .into_iter()
            .flat_map(|act| act.scenes)
            .find(|scene| scene.dialogues.iter().any(|row| row.id == *id))
            .map(|scene| scene.dialogues.into_iter().map(|row| row.id).collect())
            .unwrap_or_default(),
        Target::Cue(id) => document
            .acts()
            .into_iter()
            .flat_map(|act| act.scenes)
            .find(|scene| scene.sound_cues.iter().any(|row| row.id == *id))
            .map(|scene| scene.sound_cues.into_iter().map(|row| row.id).collect())
            .unwrap_or_default(),
        _ => vec![],
    }
}

/// All state that protects unsaved work is supplied by the route shell. Pane switching only
/// changes selection; committed content, local history and source comparison stay mounted.
#[component]
pub fn StructuredEditor(
    draft: Signal<String>,
    on_change: Callback<String>,
    english: RwSignal<bool>,
    readonly: Signal<bool>,
    namespace: &'static str,
    baseline: Signal<String>,
    issues: Signal<Vec<FieldIssue>>,
    activity: RwSignal<bool>,
    children: ChildrenFn,
) -> impl IntoView {
    let document = Memo::new(move |_| Document::parse(&draft.get()).ok());
    let history = RwSignal::new(None::<History>);
    let emitted = RwSignal::new(String::new());
    let notice = RwSignal::new(None::<AuthoringError>);
    let movement = RwSignal::new(None::<(String, usize, String)>);
    let pending_fields = RwSignal::new(InputActivity::default());
    let selected = RwSignal::new(None::<Target>);
    Effect::new(move |_| {
        let incoming = draft.get();
        let parsed = document.get();
        if incoming != emitted.get_untracked() {
            history.set(parsed.clone().map(History::new));
        }
        if let Some(document) = parsed {
            let selection_exists = selected
                .get_untracked()
                .as_ref()
                .is_some_and(|target| document.path(target).is_ok());
            if !selection_exists {
                selected.set(
                    document
                        .acts()
                        .into_iter()
                        .flat_map(|act| act.scenes)
                        .next()
                        .map(|scene| Target::Scene(scene.id))
                        .or(Some(Target::Work)),
                );
            }
        }
    });
    let apply = Callback::new(move |next: Document| {
        if readonly.get_untracked() {
            return false;
        }
        let text = next.to_json();
        history.update(|history| {
            *history = Some(match history.as_ref() {
                Some(current) => current.apply(next),
                None => History::new(next),
            });
        });
        emitted.set(text.clone());
        notice.set(None);
        on_change.run(text);
        true
    });
    let ctx = AuthoringContext {
        document,
        apply,
        english,
        readonly,
        issues,
        activity,
        pending_fields,
        notice,
        movement,
        namespace,
    };
    let copy = move || authoring_copy(english.get());
    let move_history = move |redo: bool| {
        if readonly.get_untracked() || activity.get_untracked() {
            return;
        }
        if let Some(current) = history.get_untracked() {
            let next = if redo { current.redo() } else { current.undo() };
            let text = next.current().to_json();
            history.set(Some(next));
            emitted.set(text.clone());
            notice.set(None);
            on_change.run(text);
        }
    };
    view! {
        // Components share their caller's owner. Provider retains a child owner so mounted
        // proposal and revision editors cannot shadow one another's authoring context.
        <Provider value=ctx>
        <section class="authoring" aria-labelledby=format!("{namespace}-authoring-title")>
            <div class="authoring-heading">
                <h2 id=format!("{namespace}-authoring-title")>{move || copy().title}</h2>
                <div class="toolbar authoring-history" aria-describedby=format!("{namespace}-history-help")>
                    <button class="cui-button" data-variant="secondary" type="button" disabled=move || readonly.get() || !history.with(|history| history.as_ref().is_some_and(History::can_undo)) on:click=move |_| move_history(false)>{move || copy().undo}</button>
                    <button class="cui-button" data-variant="secondary" type="button" disabled=move || readonly.get() || !history.with(|history| history.as_ref().is_some_and(History::can_redo)) on:click=move |_| move_history(true)>{move || copy().redo}</button>
                </div>
            </div>
            <p class="help" id=format!("{namespace}-history-help")>{move || copy().history_help}</p>
            <p class="authoring-action-status" role="status" aria-live="polite">{move || movement.get().map(|(id, position, group)| crate::messages::authoring::item_moved(&id, position, &group, english.get())).unwrap_or_default()}</p>
            <Show when=move || activity.get()><p class="help authoring-buffered">{move || copy().buffered}</p></Show>
            <Show when=move || readonly.get()><p class="mode">{move || copy().readonly}</p></Show>
            <Show when=move || notice.get().is_some()>
                <p class="authoring-error" role="alert">{move || copy().operation_failed}
                    {move || notice.with(|notice| match notice {
                        Some(AuthoringError::Referenced { references, .. }) => references.join(", "),
                        _ => String::new(),
                    })}
                </p>
            </Show>
            <div class="authoring-workspace">
            <Show when=move || document.get().is_some() fallback=move || view! {
                <p class="authoring-empty">{move || if draft.get().is_empty() { copy().empty } else { copy().invalid }}</p>
            }>
                    <nav class="authoring-nav" aria-label=move || copy().navigation>
                        <h3>{move || copy().scenes}</h3>
                        <For each=move || document.get().map(|document| document.acts()).unwrap_or_default() key=|act| act.id.clone() children=move |act| {
                            let id = StoredValue::new(act.id);
                            view! {
                                <div class="authoring-act">
                                        <button type="button" class="cui-button authoring-nav-item" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Act(id.get_value())) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Act(id.get_value()))); } }>
                                        <span class="authoring-nav-kind">{move || copy().act}</span>
                                        <span lang="vi-VN">{move || document.get().and_then(|document| document.acts().into_iter().find(|act| act.id == id.get_value())).map(|act| act.title).unwrap_or_default()}</span>
                                    </button>
                                    <For each=move || document.get().and_then(|document| document.acts().into_iter().find(|act| act.id == id.get_value())).map(|act| act.scenes).unwrap_or_default() key=|scene| scene.id.clone() children=move |scene| {
                                        let scene_id = StoredValue::new(scene.id);
                                        view! {
                                            <button type="button" class="cui-button authoring-nav-item authoring-nav-scene" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Scene(scene_id.get_value())) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Scene(scene_id.get_value()))); } }>
                                                <span class="authoring-nav-kind">{move || copy().scene}</span>
                                                <span lang="vi-VN">{move || document.get().and_then(|document| crate::authoring_view::scene(&document, &scene_id.get_value())).map(|scene| scene.title).unwrap_or_default()}</span>
                                            </button>
                                        }
                                    } />
                                </div>
                            }
                        } />
                        <button class="cui-button" data-variant="secondary" type="button" disabled=move || readonly.get() on:click=move |_| {
                            if let Ok(id) = minted_id("act") {
                                if ctx.structural_change(|document| document.insert_act(&id, authoring_copy(document.metadata().language.starts_with("en")).new_act)) { selected.set(Some(Target::Act(id))); }
                            }
                        }>{move || copy().add_act}</button>
                        <h3>{move || copy().characters}</h3>
                        <For each=move || document.get().map(|document| document.characters()).unwrap_or_default() key=|character| character.id.clone() children=move |character| {
                            let id = StoredValue::new(character.id);
                            view! {
                                <button type="button" class="cui-button authoring-nav-item" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Character(id.get_value())) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Character(id.get_value()))); } }>
                                    <span lang="vi-VN">{move || document.get().and_then(|document| document.characters().into_iter().find(|character| character.id == id.get_value())).map(|character| character.name).unwrap_or_default()}</span>
                                </button>
                            }
                        } />
                        <button class="cui-button" data-variant="secondary" type="button" disabled=move || readonly.get() on:click=move |_| {
                            if let Ok(id) = minted_id("character") {
                                if ctx.structural_change(|document| document.insert_character(&id, authoring_copy(document.metadata().language.starts_with("en")).new_character, "character", "")) { selected.set(Some(Target::Character(id))); }
                            }
                        }>{move || copy().add_character}</button>
                        <button type="button" class="cui-button authoring-nav-item" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Work) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Work)); } }>{move || copy().metadata}</button>
                    </nav>
                    <div class="authoring-canvas" id=format!("{namespace}-canvas") tabindex="-1">
                        {move || match selected.get() {
                            Some(Target::Scene(id)) => view! { <SceneEditor id=id /> }.into_any(),
                            Some(Target::Act(id)) => view! { <ActEditor id=id selected=selected /> }.into_any(),
                            Some(Target::Character(id)) => view! { <CharacterEditor id=id /> }.into_any(),
                            _ => view! { <MetadataEditor /> }.into_any(),
                        }}
                    </div>
            </Show>
                    <aside class="authoring-inspector" aria-label=move || copy().inspector>
                        <h3>{move || copy().validation}</h3>
                        <p class="help">{move || copy().validation_help}</p>
                        <Show when=move || !issues.get().is_empty() fallback=move || view! { <p>{move || copy().no_issues}</p> }>
                            <ul class="authoring-findings">{move || issues.get().into_iter().map(|issue| {
                                let finding = issue.clone();
                                view! {
                                    <li><button class="cui-button" data-variant="secondary" type="button" on:click=move |_| {
                                        if activity.get_untracked() { return; }
                                        if let Some(document) = document.get_untracked() {
                                            let (pane, field) = finding_location(&document, namespace, &finding);
                                            selected.set(Some(pane));
                                            leptos::leptos_dom::helpers::request_animation_frame(move || focus_field(&field));
                                        } else {
                                            leptos::leptos_dom::helpers::request_animation_frame(move || focus_field(&format!("{namespace}-json")));
                                        }
                                    }><span>{validation_rule(&issue.rule, english.get())}</span><code>{issue.path}</code></button><code>{issue.rule}</code></li>
                                }
                            }).collect_view()}</ul>
                        </Show>
                        <h3>{move || copy().comparison}</h3>
                        <p class="help">{move || copy().comparison_help}</p>
                        {move || {
                            let changes = document.get().zip(Document::parse(&baseline.get()).ok()).map(|(current, base)| current.changes_from(&base)).unwrap_or_default();
                            if changes.is_empty() { view! { <p>{move || copy().unchanged}</p> }.into_any() }
                            else { view! { <div class="authoring-comparison">{changes.into_iter().map(|change| view! {
                                <details class="authoring-change">
                                    <summary>{match change.kind { ChangeKind::Added => copy().added, ChangeKind::Removed => copy().removed, ChangeKind::Changed => copy().changed, ChangeKind::Reordered => copy().reordered }}<code>{change.entity_id.clone().unwrap_or(change.path.clone())}</code><code>{change.path}</code></summary>
                                    <dl><dt>{move || copy().before}</dt><dd lang="vi-VN">{change.before.unwrap_or_default()}</dd><dt>{move || copy().after}</dt><dd lang="vi-VN">{change.after.unwrap_or_default()}</dd></dl>
                                </details>
                            }).collect_view()}</div> }.into_any() }
                        }}
                        <div class="authoring-review-extension">{children()}</div>
                    </aside>
            </div>
            <details class="authoring-advanced" open=move || document.get().is_none()>
                <summary>{move || copy().advanced}</summary>
                <p class="help">{move || copy().advanced_help}</p>
                <BufferedText id=format!("{namespace}-json") label=Signal::derive(move || copy().json) value=draft readonly=readonly activity=activity multiline=true
                    on_commit=Callback::new(move |value: String| {
                        if pending_fields.with_untracked(|fields| fields.pending_count() > 1) { return false; }
                        emitted.set(value.clone());
                        if let Ok(document) = Document::parse(&value) {
                            history.update(|history| *history = Some(match history.as_ref() { Some(current) => current.apply(document), None => History::new(document) }));
                        } else { history.set(None); }
                        on_change.run(value); true
                    }) />
            </details>
        </section>
        </Provider>
    }
}

/// A mounted native input owns text while focused. Reactive model updates only synchronize
/// unfocused controls; composing input never triggers a controlled value write.
#[component]
fn BufferedText(
    id: String,
    label: Signal<&'static str>,
    value: Signal<String>,
    readonly: Signal<bool>,
    activity: RwSignal<bool>,
    on_commit: Callback<String, bool>,
    #[prop(default = false)] multiline: bool,
    #[prop(default = false)] numeric: bool,
    #[prop(optional)] invalid: Option<Signal<bool>>,
    #[prop(optional)] described_by: Option<Signal<String>>,
) -> impl IntoView {
    let initial = value.get_untracked();
    let buffer = RwSignal::new(initial.clone());
    let focused = RwSignal::new(false);
    let composing = RwSignal::new(false);
    let dirty = RwSignal::new(false);
    let field_id = StoredValue::new(id.clone());
    let ctx = context();
    let pending_fields = ctx.pending_fields;
    let language = move || {
        ctx.document
            .get()
            .map(|document| document.metadata().language)
            .unwrap_or_else(|| "vi-VN".to_string())
    };
    let mark_pending = move |pending: bool| {
        pending_fields.update(|fields| *fields = fields.mark(&field_id.get_value(), pending));
        activity.set(pending_fields.with_untracked(InputActivity::is_pending));
    };
    on_cleanup(move || mark_pending(false));
    let update = move |text: String| {
        buffer.set(text);
        dirty.set(true);
        mark_pending(true);
    };
    let commit = move || {
        if composing.get_untracked() || !dirty.get_untracked() {
            return;
        }
        if on_commit.run(buffer.get_untracked()) {
            dirty.set(false);
            mark_pending(false);
        }
    };
    // A document becoming invalid disposes structured-field derivations before their DOM
    // attributes finish teardown. Retired inputs must not panic or regain edit authority.
    let invalid = move || invalid.is_some_and(|signal| signal.try_get().unwrap_or(false));
    if multiline {
        let node = NodeRef::<leptos::html::Textarea>::new();
        Effect::new(move |_| {
            let Some(next) = value.try_get() else {
                return;
            };
            if let Some(node) = node.get() {
                if !focused.get()
                    && !composing.get_untracked()
                    && !dirty.get_untracked()
                    && node.value() != next
                {
                    node.set_value(&next);
                    buffer.set(next);
                }
            }
        });
        view! {
            <div class="authoring-field">
                <FieldLabel for_id=id.clone()>{move || label.try_get().unwrap_or_default()}</FieldLabel>
                <textarea class="cui-field-control" id=id node_ref=node lang=language spellcheck="false" prop:value=initial readonly=move || readonly.try_get().unwrap_or(true) aria-invalid=invalid aria-describedby=move || described_by.and_then(|signal| signal.try_get()).filter(|value| !value.is_empty())
                    on:focus=move |_| focused.set(true)
                    on:input=move |event| update(event_target_value(&event))
                    on:compositionstart=move |_| { composing.set(true); mark_pending(true); }
                    on:compositionend=move |event| { composing.set(false); update(event_target_value(&event)); commit(); }
                    on:keydown=move |event| {
                        if event.key() == "Escape" && !composing.get_untracked() {
                            event.prevent_default(); event.stop_propagation();
                            let committed = value.get_untracked(); buffer.set(committed.clone()); dirty.set(false); mark_pending(false);
                            if let Some(node) = node.get_untracked() { node.set_value(&committed); }
                        }
                    }
                    on:blur=move |_| { focused.set(false); commit(); } />
            </div>
        }.into_any()
    } else {
        let node = NodeRef::<leptos::html::Input>::new();
        Effect::new(move |_| {
            let Some(next) = value.try_get() else {
                return;
            };
            if let Some(node) = node.get() {
                if !focused.get()
                    && !composing.get_untracked()
                    && !dirty.get_untracked()
                    && node.value() != next
                {
                    node.set_value(&next);
                    buffer.set(next);
                }
            }
        });
        view! {
            <div class="authoring-field">
                <FieldLabel for_id=id.clone()>{move || label.try_get().unwrap_or_default()}</FieldLabel>
                <input class="cui-field-control" id=id node_ref=node type=if numeric { "number" } else { "text" } min=if numeric { Some("0") } else { None } max=if numeric { Some("1000") } else { None } step=if numeric { Some("1") } else { None }
                    lang=language autocomplete="off" prop:value=initial readonly=move || readonly.try_get().unwrap_or(true) aria-invalid=invalid aria-describedby=move || described_by.and_then(|signal| signal.try_get()).filter(|value| !value.is_empty())
                    on:focus=move |_| focused.set(true)
                    on:input=move |event| update(event_target_value(&event))
                    on:compositionstart=move |_| { composing.set(true); mark_pending(true); }
                    on:compositionend=move |event| { composing.set(false); update(event_target_value(&event)); commit(); }
                    on:keydown=move |event| {
                        if event.key() == "Escape" && !composing.get_untracked() {
                            event.prevent_default(); event.stop_propagation();
                            let committed = value.get_untracked(); buffer.set(committed.clone()); dirty.set(false); mark_pending(false);
                            if let Some(node) = node.get_untracked() { node.set_value(&committed); }
                        }
                    }
                    on:blur=move |_| { focused.set(false); commit(); } />
            </div>
        }.into_any()
    }
}

#[component]
fn TextField(
    target: Target,
    field: Field,
    label: Signal<&'static str>,
    #[prop(default = false)] multiline: bool,
) -> impl IntoView {
    let ctx = context();
    let target = StoredValue::new(target);
    let id = field_id(ctx.namespace, &target.get_value(), field);
    let error_id = StoredValue::new(format!("{id}-error"));
    let findings = Signal::derive(move || {
        ctx.document
            .get()
            .map(|document| {
                ctx.issues
                    .get()
                    .into_iter()
                    .filter(|issue| matching_issue(&document, &target.get_value(), field, issue))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let invalid =
        Signal::derive(move || findings.try_get().is_some_and(|issues| !issues.is_empty()));
    view! {
        <BufferedText id=id label=label value=Signal::derive(move || ctx.document.get().and_then(|document| document.field(&target.get_value(), field).ok()).unwrap_or_default())
            readonly=ctx.readonly activity=ctx.activity multiline=multiline numeric=field == Field::Intensity invalid=invalid described_by=Signal::derive(move || if invalid.try_get().unwrap_or(false) { error_id.try_get_value().unwrap_or_default() } else { String::new() })
            on_commit=Callback::new(move |text: String| ctx.change(|document| document.set(&target.get_value(), field, &text))) />
        <Show when=move || invalid.try_get().unwrap_or(false)><FieldError id=error_id.get_value()>{move || findings.try_get().unwrap_or_default().into_iter().map(|issue| validation_rule(&issue.rule, ctx.english.get())).collect::<Vec<_>>().join(" ")}</FieldError></Show>
    }
}

#[component]
fn ChoiceField(
    target: Target,
    field: Field,
    label: Signal<&'static str>,
    options: Signal<Vec<(String, String)>>,
) -> impl IntoView {
    let ctx = context();
    let target = StoredValue::new(target);
    let id = field_id(ctx.namespace, &target.get_value(), field);
    let error_id = StoredValue::new(format!("{id}-error"));
    let current = Signal::derive(move || {
        ctx.document
            .get()
            .and_then(|document| document.field(&target.get_value(), field).ok())
            .unwrap_or_default()
    });
    let unresolved = move || {
        !options
            .get()
            .iter()
            .any(|(value, _)| *value == current.get())
    };
    let unresolved_copy = move || {
        if field == Field::Speaker {
            authoring_copy(ctx.english.get()).unresolved
        } else {
            choice("unknown", ctx.english.get())
        }
    };
    let findings = Signal::derive(move || {
        ctx.document
            .get()
            .map(|document| {
                ctx.issues
                    .get()
                    .into_iter()
                    .filter(|issue| matching_issue(&document, &target.get_value(), field, issue))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let invalid = move || unresolved() || !findings.get().is_empty();
    view! {
        <div class="authoring-field">
            <FieldLabel for_id=id.clone()>{move || label.get()}</FieldLabel>
            <div class="cui-select">
            <select class="cui-field-control" id=id disabled=move || ctx.readonly.get() aria-invalid=invalid aria-describedby=move || invalid().then(|| error_id.get_value()) on:change=move |event| { ctx.change(|document| document.set(&target.get_value(), field, &event_target_value(&event))); }>
                // A select value written before its dynamic children exist is ignored by the
                // browser. Set option.selected from the current model on every option rebuild.
                {move || selected_choices(&current.get(), options.get(), unresolved_copy()).into_iter().map(|(value, label, selected)| view! { <option value=value prop:selected=selected>{label}</option> }).collect_view()}
            </select>
                <Icon name=IconName::ExpandMore/>
            </div>
            <Show when=invalid><FieldError id=error_id.get_value()>{move || if unresolved() { unresolved_copy().to_string() } else { findings.get().into_iter().map(|issue| validation_rule(&issue.rule, ctx.english.get())).collect::<Vec<_>>().join(" ") }}</FieldError></Show>
        </div>
    }
}

fn choices(
    values: &'static [&'static str],
    english: RwSignal<bool>,
) -> Signal<Vec<(String, String)>> {
    Signal::derive(move || {
        values
            .iter()
            .map(|value| (value.to_string(), choice(value, english.get()).to_string()))
            .collect()
    })
}

#[component]
fn ItemActions(target: Target) -> impl IntoView {
    let ctx = context();
    let target = StoredValue::new(target);
    let confirm = RwSignal::new(false);
    let trigger = NodeRef::<leptos::html::Button>::new();
    let confirmation = NodeRef::<leptos::html::Button>::new();
    Effect::new(move |_| {
        if confirm.get() {
            if let Some(button) = confirmation.get() {
                let _ = button.focus();
            }
        }
    });
    let cancel = move || {
        confirm.set(false);
        if let Some(button) = trigger.get_untracked() {
            let _ = button.focus();
        }
    };
    let movement = move |up: bool| {
        let changed = ctx.structural_change(|document| {
            let rows = collection(document, &target.get_value());
            let index = rows
                .iter()
                .position(|id| id == target_id(&target.get_value()))
                .unwrap_or_default();
            let before = if up {
                index.checked_sub(1).and_then(|index| rows.get(index))
            } else {
                rows.get(index + 2)
            };
            document.move_before(&target.get_value(), before.map(String::as_str))
        });
        if changed {
            if let Some(document) = ctx.document.get_untracked() {
                let moved = target.get_value();
                let rows = collection(&document, &moved);
                let position = rows
                    .iter()
                    .position(|id| id == target_id(&moved))
                    .unwrap_or_default()
                    + 1;
                let group = match &moved {
                    Target::Dialogue(id) | Target::Cue(id) => document
                        .acts()
                        .into_iter()
                        .flat_map(|act| act.scenes)
                        .find(|scene| {
                            scene.dialogues.iter().any(|dialogue| dialogue.id == *id)
                                || scene.sound_cues.iter().any(|cue| cue.id == *id)
                        })
                        .map(|scene| scene.title)
                        .unwrap_or_default(),
                    Target::Scene(id) => document
                        .acts()
                        .into_iter()
                        .find(|act| act.scenes.iter().any(|scene| scene.id == *id))
                        .map(|act| act.title)
                        .unwrap_or_default(),
                    Target::Act(_) => document.metadata().episode_title,
                    _ => document.metadata().work_title,
                };
                ctx.movement
                    .set(Some((target_id(&moved).to_string(), position, group)));
            }
        }
    };
    let at_edge = move |up: bool| {
        ctx.document.get().is_none_or(|document| {
            let rows = collection(&document, &target.get_value());
            let id = target.get_value();
            if up {
                rows.first().is_none_or(|row| row == target_id(&id))
            } else {
                rows.last().is_none_or(|row| row == target_id(&id))
            }
        })
    };
    view! {
        <div class="authoring-actions">
            <button class="cui-button" data-variant="secondary" type="button" aria-label=move || item_action(ItemAction::MoveUp, target_id(&target.get_value()), ctx.english.get()) disabled=move || ctx.readonly.get() || at_edge(true) on:click=move |_| movement(true)>{move || authoring_copy(ctx.english.get()).move_up}</button>
            <button class="cui-button" data-variant="secondary" type="button" aria-label=move || item_action(ItemAction::MoveDown, target_id(&target.get_value()), ctx.english.get()) disabled=move || ctx.readonly.get() || at_edge(false) on:click=move |_| movement(false)>{move || authoring_copy(ctx.english.get()).move_down}</button>
            <button class="cui-button" data-variant="secondary" type="button" aria-label=move || item_action(ItemAction::Remove, target_id(&target.get_value()), ctx.english.get()) node_ref=trigger disabled=move || ctx.readonly.get() on:click=move |_| confirm.set(true)>{move || authoring_copy(ctx.english.get()).remove}</button>
            <Show when=move || confirm.get()>
                <div class="authoring-remove-confirm" role="group" aria-label=move || authoring_copy(ctx.english.get()).confirm_remove on:keydown=move |event| { if event.key() == "Escape" { event.stop_propagation(); cancel(); } }>
                    <p>{move || authoring_copy(ctx.english.get()).remove_help}</p><code>{move || target_id(&target.get_value()).to_string()}</code>
                    <button class="cui-button" data-variant="secondary" type="button" aria-label=move || item_action(ItemAction::ConfirmRemove, target_id(&target.get_value()), ctx.english.get()) node_ref=confirmation disabled=move || ctx.readonly.get() on:click=move |_| {
                        if ctx.structural_change(|document| document.remove(&target.get_value())) {
                            confirm.set(false);
                            if let Some(element) = leptos::leptos_dom::helpers::document().get_element_by_id(&format!("{}-canvas", ctx.namespace)) {
                                if let Some(element) = wasm_bindgen::JsCast::dyn_ref::<web_sys::HtmlElement>(&element) { let _ = element.focus(); }
                            }
                        }
                    }>{move || authoring_copy(ctx.english.get()).confirm_remove}</button>
                    <button class="cui-button" data-variant="secondary" type="button" on:click=move |_| cancel()>{move || authoring_copy(ctx.english.get()).cancel}</button>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn MetadataEditor() -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.english.get());
    view! {
        <h3>{move || copy().metadata}</h3>
        <TextField target=Target::Work field=Field::Title label=Signal::derive(move || copy().work_title) />
        <TextField target=Target::Episode field=Field::Title label=Signal::derive(move || copy().episode_title) />
        <ChoiceField target=Target::Adaptation field=Field::Language label=Signal::derive(move || copy().language) options=choices(&["vi", "vi-VN", "en", "en-US"], ctx.english) />
        <p class="help">{move || copy().metadata_help}</p>
    }
}

#[component]
fn CharacterEditor(id: String) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.english.get());
    let target = Target::Character(id.clone());
    view! {
        <h3>{move || copy().characters}</h3><code class="authoring-id">{id}</code>
        <TextField target=target.clone() field=Field::Name label=Signal::derive(move || copy().character_name) />
        <ChoiceField target=target.clone() field=Field::Role label=Signal::derive(move || copy().role) options=choices(&["narrator", "character"], ctx.english) />
        <TextField target=target.clone() field=Field::Personality label=Signal::derive(move || copy().personality) multiline=true />
        <ItemActions target=target />
    }
}

#[component]
fn ActEditor(id: String, selected: RwSignal<Option<Target>>) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.english.get());
    let saved_id = StoredValue::new(id.clone());
    view! {
        <h3>{move || copy().act}</h3><code class="authoring-id">{id.clone()}</code>
        <TextField target=Target::Act(id.clone()) field=Field::Title label=Signal::derive(move || copy().act_title) />
        <ItemActions target=Target::Act(id) />
        <button class="cui-button" data-variant="secondary" type="button" disabled=move || ctx.readonly.get() on:click=move |_| {
            if let Ok(id) = minted_id("scene") {
                if ctx.structural_change(|document| document.insert_scene(&saved_id.get_value(), &id, authoring_copy(document.metadata().language.starts_with("en")).new_scene)) { selected.set(Some(Target::Scene(id))); }
            }
        }>{move || copy().add_scene}</button>
    }
}

#[component]
fn SceneEditor(id: String) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.english.get());
    let saved_id = StoredValue::new(id.clone());
    let add_dialogue = move |narration: bool| {
        if let Ok(id) = minted_id("dialogue") {
            ctx.structural_change(|document| {
                let characters = document.characters();
                let speaker = characters
                    .iter()
                    .find(|character| (character.role == "narrator") == narration)
                    .or_else(|| characters.first())
                    .map(|character| character.id.clone())
                    .unwrap_or_default();
                document.insert_dialogue(&saved_id.get_value(), &id, &speaker, "")
            });
        }
    };
    view! {
        <h3>{move || copy().scene}</h3><code class="authoring-id">{id.clone()}</code>
        <TextField target=Target::Scene(id.clone()) field=Field::Title label=Signal::derive(move || copy().scene_title) />
        <ItemActions target=Target::Scene(id) />
        <div class="authoring-dialogues">
            <For each=move || ctx.document.get().and_then(|document| scene(&document, &saved_id.get_value())).map(|scene| scene.dialogues).unwrap_or_default() key=|dialogue| dialogue.id.clone() children=move |dialogue| view! { <DialogueEditor id=dialogue.id /> } />
        </div>
        <div class="toolbar">
            <button class="cui-button" data-variant="secondary" type="button" disabled=move || ctx.readonly.get() on:click=move |_| add_dialogue(false)>{move || copy().add_dialogue}</button>
            <button class="cui-button" data-variant="secondary" type="button" disabled=move || ctx.readonly.get() on:click=move |_| add_dialogue(true)>{move || copy().add_narration}</button>
        </div>
        <h4>{move || copy().cues}</h4>
        <For each=move || ctx.document.get().and_then(|document| scene(&document, &saved_id.get_value())).map(|scene| scene.sound_cues).unwrap_or_default() key=|cue| cue.id.clone() children=move |cue| view! { <CueEditor id=cue.id scene_id=saved_id.get_value() /> } />
        <button class="cui-button" data-variant="secondary" type="button" disabled=move || ctx.readonly.get() || ctx.document.get().and_then(|document| scene(&document, &saved_id.get_value())).is_none_or(|scene| scene.dialogues.is_empty()) on:click=move |_| {
            if let Ok(id) = minted_id("cue") {
                ctx.structural_change(|document| {
                    let anchor = scene(document, &saved_id.get_value()).and_then(|scene| scene.dialogues.first().map(|dialogue| dialogue.id.clone())).unwrap_or_default();
                    document.insert_cue(&saved_id.get_value(), &id, "ambience", "", &anchor, "start")
                });
            }
        }>{move || copy().add_cue}</button>
    }
}

#[component]
fn DialogueEditor(id: String) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.english.get());
    let saved_id = StoredValue::new(id.clone());
    let target = Target::Dialogue(id.clone());
    let is_narrator = move || {
        ctx.document.get().is_some_and(|document| {
            document
                .field(&Target::Dialogue(saved_id.get_value()), Field::Speaker)
                .ok()
                .is_some_and(|id| {
                    document
                        .characters()
                        .iter()
                        .any(|character| character.id == id && character.role == "narrator")
                })
        })
    };
    view! {
        <section class="authoring-row" data-dialogue-id=id.clone() aria-labelledby=format!("{}-{}-label", ctx.namespace, id)>
            <div class="authoring-row-heading"><h4 id=format!("{}-{}-label", ctx.namespace, id)>{move || if is_narrator() { copy().narration } else { copy().dialogue }}</h4><code class="authoring-id">{id.clone()}</code></div>
            <ChoiceField target=target.clone() field=Field::Speaker label=Signal::derive(move || copy().speaker) options=Signal::derive(move || ctx.document.get().map(|document| document.characters().into_iter().map(|character| (character.id, character.name)).collect()).unwrap_or_default()) />
            <TextField target=target.clone() field=Field::Text label=Signal::derive(move || copy().spoken_text) multiline=true />
            <details class="authoring-performance">
                <summary>{move || copy().performance}</summary>
                <div class="authoring-fields">
                    <ChoiceField target=target.clone() field=Field::Emotion label=Signal::derive(move || copy().emotion) options=choices(&["neutral", "calm", "hopeful", "warm", "sad", "angry"], ctx.english) />
                    <TextField target=target.clone() field=Field::Intensity label=Signal::derive(move || copy().intensity) />
                </div>
                <h5>{move || copy().pronunciation}</h5><p class="help">{move || copy().pronunciation_help}</p>
                <For each=move || ctx.document.get().map(|document| document.acts().into_iter().flat_map(|act| act.scenes).flat_map(|scene| scene.dialogues).find(|dialogue| dialogue.id == saved_id.get_value()).map(|dialogue| (0..dialogue.pronunciation_overrides.len()).collect::<Vec<_>>()).unwrap_or_default()).unwrap_or_default() key=|index| *index children=move |index| view! { <PronunciationEditor dialogue_id=saved_id.get_value() index=index /> } />
                <button class="cui-button" data-variant="secondary" type="button" disabled=move || ctx.readonly.get() on:click=move |_| { ctx.structural_change(|document| document.add_pronunciation(&saved_id.get_value(), "", "")); }>{move || copy().add_pronunciation}</button>
            </details>
            <ItemActions target=target />
        </section>
    }
}

#[component]
fn PronunciationEditor(dialogue_id: String, index: usize) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.english.get());
    let id = StoredValue::new(dialogue_id.clone());
    view! {
        <div class="authoring-pronunciation authoring-fields">
            <PronunciationText dialogue_id=dialogue_id.clone() index=index field=PronunciationField::Surface />
            <PronunciationText dialogue_id=dialogue_id index=index field=PronunciationField::Replacement />
            <button class="cui-button" data-variant="secondary" type="button" aria-label=move || pronunciation_remove(&id.get_value(), index, ctx.english.get()) disabled=move || ctx.readonly.get() on:click=move |_| { ctx.structural_change(|document| document.remove_pronunciation(&id.get_value(), index)); }>{move || copy().remove}</button>
        </div>
    }
}

#[component]
fn PronunciationText(
    dialogue_id: String,
    index: usize,
    field: PronunciationField,
) -> impl IntoView {
    let ctx = context();
    let id = StoredValue::new(dialogue_id.clone());
    let suffix = match field {
        PronunciationField::Surface => "surface",
        PronunciationField::Replacement => "replacement",
    };
    let input_id = format!(
        "{}-{dialogue_id}-pronunciation-{index}-{suffix}",
        ctx.namespace
    );
    let error_id = StoredValue::new(format!("{input_id}-error"));
    let value = Signal::derive(move || {
        ctx.document
            .get()
            .and_then(|document| {
                document
                    .acts()
                    .into_iter()
                    .flat_map(|act| act.scenes)
                    .flat_map(|scene| scene.dialogues)
                    .find(|dialogue| dialogue.id == id.get_value())
            })
            .and_then(|dialogue| dialogue.pronunciation_overrides.get(index).cloned())
            .map(|pronunciation| match field {
                PronunciationField::Surface => pronunciation.surface,
                PronunciationField::Replacement => pronunciation.replacement,
            })
            .unwrap_or_default()
    });
    let findings = Signal::derive(move || {
        ctx.document
            .get()
            .map(|document| {
                let paths = document
                    .diagnostic_paths(&Target::Dialogue(id.get_value()), Field::Text)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|path| {
                        format!(
                            "{}/pronunciation_overrides/{index}/{suffix}",
                            path.strip_suffix("/text").unwrap_or(&path)
                        )
                    })
                    .collect::<Vec<_>>();
                ctx.issues
                    .get()
                    .into_iter()
                    .filter(|issue| paths.contains(&issue.path))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let invalid =
        Signal::derive(move || findings.try_get().is_some_and(|issues| !issues.is_empty()));
    view! {
        <div class="authoring-pronunciation-field">
            <BufferedText id=input_id label=Signal::derive(move || match field { PronunciationField::Surface => authoring_copy(ctx.english.get()).surface, PronunciationField::Replacement => authoring_copy(ctx.english.get()).replacement }) value=value readonly=ctx.readonly activity=ctx.activity invalid=invalid described_by=Signal::derive(move || if invalid.try_get().unwrap_or(false) { error_id.try_get_value().unwrap_or_default() } else { String::new() })
                on_commit=Callback::new(move |text: String| ctx.change(|document| document.set_pronunciation(&id.get_value(), index, field, &text))) />
            <Show when=move || invalid.try_get().unwrap_or(false)><FieldError id=error_id.get_value()>{move || findings.try_get().unwrap_or_default().into_iter().map(|issue| validation_rule(&issue.rule, ctx.english.get())).collect::<Vec<_>>().join(" ")}</FieldError></Show>
        </div>
    }
}

#[component]
fn CueEditor(id: String, scene_id: String) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.english.get());
    let scene_id = StoredValue::new(scene_id);
    let saved_id = StoredValue::new(id.clone());
    let target = Target::Cue(id.clone());
    view! {
        <section class="authoring-cue" data-cue-id=id.clone()>
            <code class="authoring-id">{id.clone()}</code>
            <ChoiceField target=target.clone() field=Field::CueKind label=Signal::derive(move || copy().cue_kind) options=choices(&["ambience", "music", "sfx"], ctx.english) />
            <TextField target=target.clone() field=Field::CueDescription label=Signal::derive(move || copy().description) multiline=true />
            <ChoiceField target=target.clone() field=Field::CueDialogue label=Signal::derive(move || copy().anchor) options=Signal::derive(move || ctx.document.get().and_then(|document| scene(&document, &scene_id.get_value())).map(|scene| scene.dialogues.into_iter().map(|dialogue| (dialogue.id, dialogue.text)).collect()).unwrap_or_default()) />
            <ChoiceField target=target.clone() field=Field::CueEdge label=Signal::derive(move || copy().edge) options=choices(&["start", "end"], ctx.english) />
            <Show when=move || ctx.document.get().and_then(|document| scene(&document, &scene_id.get_value())).and_then(|scene| scene.sound_cues.into_iter().find(|cue| cue.id == saved_id.get_value())).is_some_and(|cue| cue.asset.is_some())><p class="help">{move || copy().asset_help}</p></Show>
            <ItemActions target=target />
        </section>
    }
}
