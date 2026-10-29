//! Native, buffered fields over the lossless authoring core. Backend gates stay in the shell.
use crate::contract::FieldIssue;
use crate::{
    authoring::{
        AuthoringError, ChangeKind, Document, Field, History, InputActivity, PronunciationField,
        Scene, Target,
    },
    messages::{
        authoring::{
            authoring_copy, choice, item_action, pronunciation_remove, selected_choices,
            validation_rule, ItemAction,
        },
        workspace::{text as workspace_text, Key as WorkspaceKey},
    },
    ui::{BlockReason, LocalizedReason},
};
use cantos_ui::{ChoiceState, ErrorState, FieldError, FieldLabel, Select, SelectOption};
use leptos::context::Provider;
use leptos::prelude::*;
use leptos::{wasm_bindgen::JsCast, web_sys::HtmlElement};

#[derive(Clone, Copy)]
struct AuthoringContext {
    document: Memo<Option<Document>>,
    apply: Callback<Document, bool>,
    locale: Signal<crate::localization::Locale>,
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

#[cfg(target_arch = "wasm32")]
fn minted_id(prefix: &str) -> Result<String, AuthoringError> {
    crate::view::operation_id()
        .map(|id| format!("{prefix}-{id}"))
        .map_err(|_| AuthoringError::InvalidId(String::new()))
}

/// Native string-render tests have no browser random source: structural inserts refuse.
#[cfg(not(target_arch = "wasm32"))]
fn minted_id(_prefix: &str) -> Result<String, AuthoringError> {
    Err(AuthoringError::InvalidId(String::new()))
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
    let document = leptos::leptos_dom::helpers::document();
    // An enhanced shared Select hides its native control behind a combobox trigger.
    let trigger = document
        .get_element_by_id(&format!("{id}-trigger"))
        .filter(|trigger| !trigger.has_attribute("hidden"));
    if let Some(element) = trigger.or_else(|| document.get_element_by_id(id)) {
        let mut parent = element.parent_element();
        while let Some(element) = parent {
            if element.tag_name() == "DETAILS" {
                let _ = element.set_attribute("open", "");
            }
            parent = element.parent_element();
        }
        if let Some(element) = element.dyn_ref::<HtmlElement>() {
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
    locale: Signal<crate::localization::Locale>,
    readonly: Signal<bool>,
    namespace: &'static str,
    baseline: Signal<String>,
    issues: Signal<Vec<FieldIssue>>,
    activity: RwSignal<bool>,
    children: ChildrenFn,
) -> impl IntoView {
    let document = Memo::new(move |_| Document::parse(&draft.get()).ok());
    let content_language =
        Signal::derive(move || document.get().map(|document| document.metadata().language));
    let baseline_language = Signal::derive(move || {
        Document::parse(&baseline.get())
            .ok()
            .map(|document| document.metadata().language)
    });
    let history = RwSignal::new(None::<History>);
    let emitted = RwSignal::new(String::new());
    let notice = RwSignal::new(None::<AuthoringError>);
    let movement = RwSignal::new(None::<(String, usize, String)>);
    let pending_fields = RwSignal::new(InputActivity::default());
    let selected = RwSignal::new(None::<Target>);
    let nav_open = RwSignal::new(false);
    let nav_toggle = NodeRef::<leptos::html::Button>::new();
    // Narrow containers render the toggle and fold the navigation away once a target is chosen, so the
    // editing pane stays inside the first screen; wide layouts hide the toggle and keep focus on the tree.
    let reveal_canvas = move || {
        if nav_toggle
            .get_untracked()
            .is_some_and(|toggle| toggle.offset_parent().is_some())
        {
            nav_open.set(false);
            leptos::leptos_dom::helpers::request_animation_frame(move || {
                focus_field(&format!("{namespace}-canvas"))
            });
        }
    };
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
        locale,
        readonly,
        issues,
        activity,
        pending_fields,
        notice,
        movement,
        namespace,
    };
    let copy = move || authoring_copy(locale.get());
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
                    <button class="cui-button" data-variant="text" type="button" disabled=move || readonly.get() || !history.with(|history| history.as_ref().is_some_and(History::can_undo)) on:click=move |_| move_history(false)>{move || copy().undo}</button>
                    <button class="cui-button" data-variant="text" type="button" disabled=move || readonly.get() || !history.with(|history| history.as_ref().is_some_and(History::can_redo)) on:click=move |_| move_history(true)>{move || copy().redo}</button>
                </div>
            </div>
            <p class="help" id=format!("{namespace}-history-help")>{move || copy().history_help}</p>
            <p class="authoring-action-status" role="status" aria-live="polite">{move || movement.get().map(|(id, position, group)| crate::messages::authoring::item_moved(&id, position, &group, locale.get())).unwrap_or_default()}</p>
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
            <div class="authoring-workspace" data-nav=move || if nav_open.get() { "open" } else { "closed" }>
            <Show when=move || document.get().is_some() fallback=move || view! {
                <p class="authoring-empty">{move || if draft.get().is_empty() { copy().empty } else { copy().invalid }}</p>
            }>
                    <button type="button" class="cui-button authoring-nav-toggle" id=format!("{namespace}-nav-toggle") data-variant="tonal" node_ref=nav_toggle aria-controls=format!("{namespace}-nav") aria-expanded=move || nav_open.get().to_string() on:click=move |_| nav_open.update(|open| *open = !*open)>{move || copy().navigation}</button>
                    <nav class="authoring-nav" id=format!("{namespace}-nav") aria-label=move || copy().navigation>
                        <h3>{move || copy().scenes}</h3>
                        <For each=move || document.get().map(|document| document.acts()).unwrap_or_default() key=|act| act.id.clone() children=move |act| {
                            let id = StoredValue::new(act.id);
                            view! {
                                <div class="authoring-act">
                                        <button type="button" class="cui-button authoring-nav-item" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Act(id.get_value())) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Act(id.get_value()))); reveal_canvas(); } }>
                                        <span class="authoring-nav-kind">{move || copy().act}</span>
                                        <crate::ui::ContentText language=content_language text=Signal::derive(move || document.get().and_then(|document| document.acts().into_iter().find(|act| act.id == id.get_value())).map(|act| act.title).unwrap_or_default())/>
                                    </button>
                                    <For each=move || document.get().and_then(|document| document.acts().into_iter().find(|act| act.id == id.get_value())).map(|act| act.scenes).unwrap_or_default() key=|scene| scene.id.clone() children=move |scene| {
                                        let scene_id = StoredValue::new(scene.id);
                                        view! {
                                            <button type="button" class="cui-button authoring-nav-item authoring-nav-scene" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Scene(scene_id.get_value())) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Scene(scene_id.get_value()))); reveal_canvas(); } }>
                                                <span class="authoring-nav-kind">{move || copy().scene}</span>
                                                <crate::ui::ContentText language=content_language text=Signal::derive(move || document.get().and_then(|document| crate::authoring_view::scene(&document, &scene_id.get_value())).map(|scene| scene.title).unwrap_or_default())/>
                                            </button>
                                        }
                                    } />
                                </div>
                            }
                        } />
                        <button class="cui-button" data-variant="tonal" type="button" disabled=move || readonly.get() on:click=move |_| {
                            if let Ok(id) = minted_id("act") {
                                if ctx.structural_change(|document| document.insert_act(&id, crate::messages::authoring::new_content_label(&document.metadata().language, &id, crate::messages::authoring::NewContentItem::Act))) { selected.set(Some(Target::Act(id))); reveal_canvas(); }
                            }
                        }>{move || copy().add_act}</button>
                        <h3>{move || copy().characters}</h3>
                        <For each=move || document.get().map(|document| document.characters()).unwrap_or_default() key=|character| character.id.clone() children=move |character| {
                            let id = StoredValue::new(character.id);
                            view! {
                                <button type="button" class="cui-button authoring-nav-item" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Character(id.get_value())) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Character(id.get_value()))); reveal_canvas(); } }>
                                    <crate::ui::ContentText language=content_language text=Signal::derive(move || document.get().and_then(|document| document.characters().into_iter().find(|character| character.id == id.get_value())).map(|character| character.name).unwrap_or_default())/>
                                </button>
                            }
                        } />
                        <button class="cui-button" data-variant="tonal" type="button" disabled=move || readonly.get() on:click=move |_| {
                            if let Ok(id) = minted_id("character") {
                                if ctx.structural_change(|document| document.insert_character(&id, crate::messages::authoring::new_content_label(&document.metadata().language, &id, crate::messages::authoring::NewContentItem::Character), "character", "")) { selected.set(Some(Target::Character(id))); reveal_canvas(); }
                            }
                        }>{move || copy().add_character}</button>
                        <button type="button" class="cui-button authoring-nav-item" data-variant="text" aria-pressed=move || selected.get() == Some(Target::Work) on:click=move |_| { if !activity.get_untracked() { selected.set(Some(Target::Work)); reveal_canvas(); } }>{move || copy().metadata}</button>
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
                                    <li><button class="cui-button" data-variant="text" type="button" on:click=move |_| {
                                        if activity.get_untracked() { return; }
                                        if let Some(document) = document.get_untracked() {
                                            let (pane, field) = finding_location(&document, namespace, &finding);
                                            selected.set(Some(pane));
                                            leptos::leptos_dom::helpers::request_animation_frame(move || focus_field(&field));
                                        } else {
                                            leptos::leptos_dom::helpers::request_animation_frame(move || focus_field(&format!("{namespace}-json")));
                                        }
                                    }><span>{validation_rule(&issue.rule, locale.get())}</span><code>{issue.path}</code></button><code>{issue.rule}</code></li>
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
                                    <dl><dt>{move || copy().before}</dt><dd><crate::ui::ContentText language=baseline_language text=Signal::stored(change.before.unwrap_or_default())/></dd><dt>{move || copy().after}</dt><dd><crate::ui::ContentText language=content_language text=Signal::stored(change.after.unwrap_or_default())/></dd></dl>
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
/// The wrapper reuses the shared external-label field anatomy (`cui-field`, Fields v3 strokes);
/// only presentation is shared, the buffer and its single commit stay here.
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
    /// Localized findings for this field; empty means valid.
    #[prop(optional)]
    error: Option<Signal<String>>,
) -> impl IntoView {
    let initial = value.get_untracked();
    let buffer = RwSignal::new(initial.clone());
    let focused = RwSignal::new(false);
    let composing = RwSignal::new(false);
    let dirty = RwSignal::new(false);
    let field_id = StoredValue::new(id.clone());
    let error_id = StoredValue::new(format!("{id}-error"));
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
    let message = move || {
        error
            .and_then(|signal| signal.try_get())
            .unwrap_or_default()
    };
    let invalid = move || !message().is_empty();
    // ARIA reads an empty `aria-invalid` as false, so state is always the literal string.
    let aria_invalid = move || invalid().to_string();
    let described_by = move || invalid().then(|| error_id.try_get_value()).flatten();
    let readonly = move || readonly.try_get().unwrap_or(true);
    let state = move || if readonly() { "readonly" } else { "editable" };
    let messages = move || {
        view! {
            <Show when=invalid><FieldError id=error_id.get_value()>{message}</FieldError></Show>
        }
    };
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
            <div class="cui-field authoring-field" data-variant="regular" data-state=state>
                <FieldLabel for_id=id.clone()>{move || label.try_get().unwrap_or_default()}</FieldLabel>
                <textarea class="cui-field-control cui-textarea" id=id node_ref=node lang=language spellcheck="false" prop:value=initial readonly=readonly aria-invalid=aria_invalid aria-describedby=described_by
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
                {messages}
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
            <div class="cui-field authoring-field" data-variant="regular" data-state=state>
                <FieldLabel for_id=id.clone()>{move || label.try_get().unwrap_or_default()}</FieldLabel>
                <input class="cui-field-control" id=id node_ref=node type=if numeric { "number" } else { "text" } min=if numeric { Some("0") } else { None } max=if numeric { Some("1000") } else { None } step=if numeric { Some("1") } else { None }
                    lang=language autocomplete="off" prop:value=initial readonly=readonly aria-invalid=aria_invalid aria-describedby=described_by
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
                {messages}
            </div>
        }.into_any()
    }
}

fn findings_text(issues: Vec<FieldIssue>, locale: crate::localization::Locale) -> String {
    issues
        .into_iter()
        .map(|issue| validation_rule(&issue.rule, locale))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Everything the shared Select shows for one script choice, projected from one document.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ChoiceProjection {
    value: String,
    options: Vec<SelectOption>,
    error: Option<String>,
}

/// An unresolved model value stays selected as a disabled entry in interface copy: it is never
/// dropped, rewritten or offered as a valid choice. Authored labels carry the content language.
fn choice_projection(
    document: &Document,
    target: &Target,
    field: Field,
    choices: Vec<(String, String)>,
    issues: &[FieldIssue],
    locale: crate::localization::Locale,
    content: bool,
) -> ChoiceProjection {
    let value = document.field(target, field).unwrap_or_default();
    let known = choices.iter().any(|(choice, _)| *choice == value);
    let unresolved = if field == Field::Speaker {
        authoring_copy(locale).unresolved
    } else {
        choice("unknown", locale)
    };
    let language = content.then(|| document.metadata().language);
    let options = selected_choices(&value, choices, unresolved)
        .into_iter()
        .enumerate()
        .map(|(index, (value, label, _))| {
            let placeholder = !known && index == 0;
            SelectOption {
                value,
                label,
                disabled: placeholder,
                lang: if placeholder { None } else { language.clone() },
                icon: None,
            }
        })
        .collect();
    let error = if known {
        let findings = issues
            .iter()
            .filter(|issue| matching_issue(document, target, field, issue))
            .cloned()
            .collect();
        findings_text(findings, locale)
    } else {
        unresolved.to_owned()
    };
    ChoiceProjection {
        value,
        options,
        error: (!error.is_empty()).then_some(error),
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
    let error = Signal::derive(move || {
        findings_text(findings.try_get().unwrap_or_default(), ctx.locale.get())
    });
    view! {
        <BufferedText id=id label=label value=Signal::derive(move || ctx.document.get().and_then(|document| document.field(&target.get_value(), field).ok()).unwrap_or_default())
            readonly=ctx.readonly activity=ctx.activity multiline=multiline numeric=field == Field::Intensity error=error
            on_commit=Callback::new(move |text: String| ctx.change(|document| document.set(&target.get_value(), field, &text))) />
    }
}

/// The shared Select owns anatomy, popup and keyboard; the editor owns value, findings and the
/// immediate document change. Native select has no readonly state, so read-only is disabled.
#[component]
fn ChoiceField(
    target: Target,
    field: Field,
    label: Signal<&'static str>,
    options: Signal<Vec<(String, String)>>,
    /// Option labels are authored script text (names, lines), tagged with the content language.
    #[prop(default = false)]
    content: bool,
) -> impl IntoView {
    let ctx = context();
    let Ok(reason) = LocalizedReason::new(BlockReason::ScriptReadOnly) else {
        return view! { <ErrorState title=workspace_text(WorkspaceKey::ControlUnavailable, ctx.locale.get_untracked())><p>{workspace_text(WorkspaceKey::PermissionHelp, ctx.locale.get_untracked())}</p></ErrorState> }.into_any();
    };
    let reason = StoredValue::new(reason);
    let target = StoredValue::new(target);
    let id = field_id(ctx.namespace, &target.get_value(), field);
    // A draft that stops parsing unmounts this pane in the same flush that changes the document
    // and clears findings. Holding the last projection keeps the shared Select's render effects
    // from re-running on signals its owner has already disposed.
    let projection =
        Memo::new(
            move |previous: Option<&ChoiceProjection>| match ctx.document.get() {
                Some(document) => choice_projection(
                    &document,
                    &target.get_value(),
                    field,
                    options.get(),
                    &ctx.issues.get(),
                    ctx.locale.get(),
                    content,
                ),
                None => previous.cloned().unwrap_or_default(),
            },
        );
    let value = Memo::new(move |_| projection.with(|projection| projection.value.clone()));
    let choices = Memo::new(move |_| projection.with(|projection| projection.options.clone()));
    let error = Memo::new(move |_| projection.with(|projection| projection.error.clone()));
    let state = Memo::new(move |_| {
        if ctx.readonly.get() {
            ChoiceState::Disabled(reason.with_value(|reason| reason.get(ctx.locale.get())))
        } else {
            ChoiceState::Enabled
        }
    });
    view! {
        <Select id=id label=Signal::derive(move || label.get().to_owned()) value=value options=choices
            error=error state=state class="authoring-field"
            on_input=Callback::new(move |value: String| { ctx.change(|document| document.set(&target.get_value(), field, &value)); }) />
    }.into_any()
}

fn choices(
    values: &'static [&'static str],
    locale: Signal<crate::localization::Locale>,
) -> Signal<Vec<(String, String)>> {
    Signal::derive(move || {
        values
            .iter()
            .map(|value| (value.to_string(), choice(value, locale.get()).to_string()))
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
            <button class="cui-button" data-variant="text" type="button" aria-label=move || item_action(ItemAction::MoveUp, target_id(&target.get_value()), ctx.locale.get()) disabled=move || ctx.readonly.get() || at_edge(true) on:click=move |_| movement(true)>{move || authoring_copy(ctx.locale.get()).move_up}</button>
            <button class="cui-button" data-variant="text" type="button" aria-label=move || item_action(ItemAction::MoveDown, target_id(&target.get_value()), ctx.locale.get()) disabled=move || ctx.readonly.get() || at_edge(false) on:click=move |_| movement(false)>{move || authoring_copy(ctx.locale.get()).move_down}</button>
            <button class="cui-button" data-variant="text" type="button" aria-label=move || item_action(ItemAction::Remove, target_id(&target.get_value()), ctx.locale.get()) node_ref=trigger disabled=move || ctx.readonly.get() on:click=move |_| confirm.set(true)>{move || authoring_copy(ctx.locale.get()).remove}</button>
            <Show when=move || confirm.get()>
                <div class="authoring-remove-confirm" role="group" aria-label=move || authoring_copy(ctx.locale.get()).confirm_remove on:keydown=move |event| { if event.key() == "Escape" { event.stop_propagation(); cancel(); } }>
                    <p>{move || authoring_copy(ctx.locale.get()).remove_help}</p><code>{move || target_id(&target.get_value()).to_string()}</code>
                    <button class="cui-button" data-variant="danger" type="button" aria-label=move || item_action(ItemAction::ConfirmRemove, target_id(&target.get_value()), ctx.locale.get()) node_ref=confirmation disabled=move || ctx.readonly.get() on:click=move |_| {
                        if ctx.structural_change(|document| document.remove(&target.get_value())) {
                            confirm.set(false);
                            if let Some(element) = leptos::leptos_dom::helpers::document().get_element_by_id(&format!("{}-canvas", ctx.namespace)) {
                                if let Some(element) = element.dyn_ref::<HtmlElement>() { let _ = element.focus(); }
                            }
                        }
                    }>{move || authoring_copy(ctx.locale.get()).confirm_remove}</button>
                    <button class="cui-button" data-variant="text" type="button" on:click=move |_| cancel()>{move || authoring_copy(ctx.locale.get()).cancel}</button>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn MetadataEditor() -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.locale.get());
    view! {
        <h3>{move || copy().metadata}</h3>
        <TextField target=Target::Work field=Field::Title label=Signal::derive(move || copy().work_title) />
        <TextField target=Target::Episode field=Field::Title label=Signal::derive(move || copy().episode_title) />
        <ChoiceField target=Target::Adaptation field=Field::Language label=Signal::derive(move || copy().language) options=choices(&["vi", "vi-VN", "en", "en-US"], ctx.locale) />
        <p class="help">{move || copy().metadata_help}</p>
    }
}

#[component]
fn CharacterEditor(id: String) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.locale.get());
    let target = Target::Character(id.clone());
    view! {
        <h3>{move || copy().characters}</h3><code class="authoring-id">{id}</code>
        <TextField target=target.clone() field=Field::Name label=Signal::derive(move || copy().character_name) />
        <ChoiceField target=target.clone() field=Field::Role label=Signal::derive(move || copy().role) options=choices(&["narrator", "character"], ctx.locale) />
        <TextField target=target.clone() field=Field::Personality label=Signal::derive(move || copy().personality) multiline=true />
        <ItemActions target=target />
    }
}

#[component]
fn ActEditor(id: String, selected: RwSignal<Option<Target>>) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.locale.get());
    let saved_id = StoredValue::new(id.clone());
    view! {
        <h3>{move || copy().act}</h3><code class="authoring-id">{id.clone()}</code>
        <TextField target=Target::Act(id.clone()) field=Field::Title label=Signal::derive(move || copy().act_title) />
        <ItemActions target=Target::Act(id) />
        <button class="cui-button" data-variant="tonal" type="button" disabled=move || ctx.readonly.get() on:click=move |_| {
            if let Ok(id) = minted_id("scene") {
                if ctx.structural_change(|document| document.insert_scene(&saved_id.get_value(), &id, crate::messages::authoring::new_content_label(&document.metadata().language, &id, crate::messages::authoring::NewContentItem::Scene))) { selected.set(Some(Target::Scene(id))); }
            }
        }>{move || copy().add_scene}</button>
    }
}

#[component]
fn SceneEditor(id: String) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.locale.get());
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
            <button class="cui-button" data-variant="tonal" type="button" disabled=move || ctx.readonly.get() on:click=move |_| add_dialogue(false)>{move || copy().add_dialogue}</button>
            <button class="cui-button" data-variant="tonal" type="button" disabled=move || ctx.readonly.get() on:click=move |_| add_dialogue(true)>{move || copy().add_narration}</button>
        </div>
        <h4>{move || copy().cues}</h4>
        <For each=move || ctx.document.get().and_then(|document| scene(&document, &saved_id.get_value())).map(|scene| scene.sound_cues).unwrap_or_default() key=|cue| cue.id.clone() children=move |cue| view! { <CueEditor id=cue.id scene_id=saved_id.get_value() /> } />
        <button class="cui-button" data-variant="tonal" type="button" disabled=move || ctx.readonly.get() || ctx.document.get().and_then(|document| scene(&document, &saved_id.get_value())).is_none_or(|scene| scene.dialogues.is_empty()) on:click=move |_| {
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
    let copy = move || authoring_copy(ctx.locale.get());
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
            <ChoiceField target=target.clone() field=Field::Speaker label=Signal::derive(move || copy().speaker) content=true options=Signal::derive(move || ctx.document.get().map(|document| document.characters().into_iter().map(|character| (character.id, character.name)).collect()).unwrap_or_default()) />
            <TextField target=target.clone() field=Field::Text label=Signal::derive(move || copy().spoken_text) multiline=true />
            <details class="authoring-performance">
                <summary>{move || copy().performance}</summary>
                <div class="authoring-fields">
                    <ChoiceField target=target.clone() field=Field::Emotion label=Signal::derive(move || copy().emotion) options=choices(&["neutral", "calm", "hopeful", "warm", "sad", "angry"], ctx.locale) />
                    <TextField target=target.clone() field=Field::Intensity label=Signal::derive(move || copy().intensity) />
                </div>
                <h5>{move || copy().pronunciation}</h5><p class="help">{move || copy().pronunciation_help}</p>
                <For each=move || ctx.document.get().map(|document| document.acts().into_iter().flat_map(|act| act.scenes).flat_map(|scene| scene.dialogues).find(|dialogue| dialogue.id == saved_id.get_value()).map(|dialogue| (0..dialogue.pronunciation_overrides.len()).collect::<Vec<_>>()).unwrap_or_default()).unwrap_or_default() key=|index| *index children=move |index| view! { <PronunciationEditor dialogue_id=saved_id.get_value() index=index /> } />
                <button class="cui-button" data-variant="tonal" type="button" disabled=move || ctx.readonly.get() on:click=move |_| { ctx.structural_change(|document| document.add_pronunciation(&saved_id.get_value(), "", "")); }>{move || copy().add_pronunciation}</button>
            </details>
            <ItemActions target=target />
        </section>
    }
}

#[component]
fn PronunciationEditor(dialogue_id: String, index: usize) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.locale.get());
    let id = StoredValue::new(dialogue_id.clone());
    view! {
        <div class="authoring-pronunciation authoring-fields">
            <PronunciationText dialogue_id=dialogue_id.clone() index=index field=PronunciationField::Surface />
            <PronunciationText dialogue_id=dialogue_id index=index field=PronunciationField::Replacement />
            <button class="cui-button" data-variant="text" type="button" aria-label=move || pronunciation_remove(&id.get_value(), index, ctx.locale.get()) disabled=move || ctx.readonly.get() on:click=move |_| { ctx.structural_change(|document| document.remove_pronunciation(&id.get_value(), index)); }>{move || copy().remove}</button>
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
    let error = Signal::derive(move || {
        findings_text(findings.try_get().unwrap_or_default(), ctx.locale.get())
    });
    view! {
        <BufferedText id=input_id label=Signal::derive(move || match field { PronunciationField::Surface => authoring_copy(ctx.locale.get()).surface, PronunciationField::Replacement => authoring_copy(ctx.locale.get()).replacement }) value=value readonly=ctx.readonly activity=ctx.activity error=error
            on_commit=Callback::new(move |text: String| ctx.change(|document| document.set_pronunciation(&id.get_value(), index, field, &text))) />
    }
}

#[component]
fn CueEditor(id: String, scene_id: String) -> impl IntoView {
    let ctx = context();
    let copy = move || authoring_copy(ctx.locale.get());
    let scene_id = StoredValue::new(scene_id);
    let saved_id = StoredValue::new(id.clone());
    let target = Target::Cue(id.clone());
    view! {
        <section class="authoring-cue" data-cue-id=id.clone()>
            <code class="authoring-id">{id.clone()}</code>
            <ChoiceField target=target.clone() field=Field::CueKind label=Signal::derive(move || copy().cue_kind) options=choices(&["ambience", "music", "sfx"], ctx.locale) />
            <TextField target=target.clone() field=Field::CueDescription label=Signal::derive(move || copy().description) multiline=true />
            <ChoiceField target=target.clone() field=Field::CueDialogue label=Signal::derive(move || copy().anchor) content=true options=Signal::derive(move || ctx.document.get().and_then(|document| scene(&document, &scene_id.get_value())).map(|scene| scene.dialogues.into_iter().map(|dialogue| (dialogue.id, dialogue.text)).collect()).unwrap_or_default()) />
            <ChoiceField target=target.clone() field=Field::CueEdge label=Signal::derive(move || copy().edge) options=choices(&["start", "end"], ctx.locale) />
            <Show when=move || ctx.document.get().and_then(|document| scene(&document, &scene_id.get_value())).and_then(|scene| scene.sound_cues.into_iter().find(|cue| cue.id == saved_id.get_value())).is_some_and(|cue| cue.asset.is_some())><p class="help">{move || copy().asset_help}</p></Show>
            <ItemActions target=target />
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(
        "../../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json"
    );

    fn fixture() -> Document {
        Document::parse(FIXTURE).unwrap()
    }

    fn issue(document: &Document, target: &Target, field: Field, rule: &str) -> FieldIssue {
        let path = document
            .diagnostic_paths(target, field)
            .unwrap()
            .into_iter()
            .find(|path| path.starts_with('/'))
            .unwrap();
        FieldIssue {
            path,
            rule: rule.into(),
        }
    }

    /// The start tag of the element carrying `id`.
    fn tag<'a>(html: &'a str, id: &str) -> &'a str {
        let at = html
            .find(&format!("id=\"{id}\""))
            .unwrap_or_else(|| panic!("{id}: {html}"));
        let start = html[..at].rfind('<').unwrap();
        let end = at + html[at..].find('>').unwrap();
        &html[start..=end]
    }

    fn is_field(start_tag: &str) -> bool {
        start_tag.contains("class=\"cui-field ")
    }

    /// Start tags of every `div` (attribute order is renderer-defined, so callers use `contains`).
    fn divs(html: &str) -> Vec<&str> {
        html.match_indices("<div")
            .map(|(at, _)| &html[at..=at + html[at..].find('>').unwrap()])
            .collect()
    }

    /// The markup from the shared field wrapper that contains `id` up to the next field wrapper.
    fn field<'a>(html: &'a str, id: &str) -> &'a str {
        let at = html.find(&format!("id=\"{id}\"")).unwrap();
        let start = html[..at]
            .match_indices("<div")
            .map(|(start, _)| start)
            .filter(|start| is_field(&html[*start..=*start + html[*start..].find('>').unwrap()]))
            .last()
            .unwrap_or_else(|| panic!("{id}: {html}"));
        let end = html[at..]
            .match_indices("<div")
            .map(|(end, _)| at + end)
            .find(|end| is_field(&html[*end..=*end + html[*end..].find('>').unwrap()]))
            .unwrap_or(html.len());
        &html[start..end]
    }

    fn button<'a>(html: &'a str, label: &str) -> &'a str {
        let at = html
            .find(&format!(">{label}</button>"))
            .unwrap_or_else(|| panic!("{label}: {html}"));
        let start = html[..at].rfind("<button").unwrap();
        &html[start..=at]
    }

    fn editor(document: &Document, issues: Vec<FieldIssue>, readonly: bool) -> (String, usize) {
        let owner = Owner::new();
        owner.with(|| {
            let commits = RwSignal::new(0_usize);
            let html = view! {
                <StructuredEditor draft=Signal::stored(document.to_json()) locale=Signal::stored(crate::localization::Locale::Vietnamese)
                    readonly=Signal::stored(readonly) namespace="test" baseline=Signal::stored(FIXTURE.to_owned())
                    issues=Signal::stored(issues) activity=RwSignal::new(false)
                    on_change=Callback::new(move |_: String| commits.update(|count| *count += 1))>
                    <span></span>
                </StructuredEditor>
            }
            .to_html();
            (html, commits.get_untracked())
        })
    }

    /// Mounts one pane under the same context the editor provides; no effect runs in SSR.
    fn pane<V: IntoView + 'static>(
        document: &Document,
        issues: Vec<FieldIssue>,
        readonly: bool,
        render: impl FnOnce() -> V + Send + 'static,
    ) -> String {
        let owner = Owner::new();
        owner.with(|| {
            let draft = RwSignal::new(document.to_json());
            let ctx = AuthoringContext {
                document: Memo::new(move |_| Document::parse(&draft.get()).ok()),
                apply: Callback::new(|_| false),
                locale: Signal::stored(crate::localization::Locale::Vietnamese),
                readonly: Signal::stored(readonly),
                issues: Signal::stored(issues),
                activity: RwSignal::new(false),
                pending_fields: RwSignal::new(InputActivity::default()),
                notice: RwSignal::new(None),
                movement: RwSignal::new(None),
                namespace: "test",
            };
            view! { <Provider value=ctx>{render()}</Provider> }.to_html()
        })
    }

    #[test]
    fn buffered_fields_reuse_the_shared_external_label_anatomy_without_render_commits() {
        let (html, commits) = editor(&fixture(), vec![], false);
        assert_eq!(commits, 0, "rendering never commits");
        // Work title, episode title and the advanced JSON editor keep their buffered inputs.
        let buffered = divs(&html)
            .into_iter()
            .filter(|tag| tag.contains("class=\"cui-field authoring-field\""))
            .collect::<Vec<_>>();
        assert_eq!(buffered.len(), 3, "{html}");
        for wrapper in buffered {
            assert!(wrapper.contains("data-variant=\"regular\""), "{wrapper}");
            assert!(wrapper.contains("data-state=\"editable\""), "{wrapper}");
        }
        let title = tag(&html, "test-work-Title");
        assert!(title.contains("class=\"cui-field-control\""), "{title}");
        assert!(title.contains("aria-invalid=\"false\""), "{title}");
        assert!(!title.contains("aria-describedby"), "{title}");
        assert!(!title.contains("readonly"), "{title}");
        assert!(
            html.contains("<label for=\"test-work-Title\" class=\"cui-field-label"),
            "{html}"
        );
        assert!(tag(&html, "test-json").contains("class=\"cui-field-control cui-textarea\""));
        assert!(
            !html.contains("aria-invalid=\"\""),
            "ARIA reads an empty value as false: {html}"
        );
    }

    #[test]
    fn a_finding_marks_the_buffered_control_invalid_and_keeps_its_message_inside_the_field() {
        let document = fixture();
        let issues = vec![issue(&document, &Target::Work, Field::Title, "empty_text")];
        let (html, commits) = editor(&document, issues, false);
        assert_eq!(commits, 0);
        let title = tag(&html, "test-work-Title");
        assert!(title.contains("aria-invalid=\"true\""), "{title}");
        assert!(
            title.contains("aria-describedby=\"test-work-Title-error\""),
            "{title}"
        );
        let wrapper = field(&html, "test-work-Title");
        assert!(
            wrapper.contains("<p id=\"test-work-Title-error\" class=\"cui-field-error"),
            "{wrapper}"
        );
        assert!(
            wrapper.contains(validation_rule(
                "empty_text",
                crate::localization::Locale::Vietnamese
            )),
            "{wrapper}"
        );
        assert!(tag(&html, "test-episode-Title").contains("aria-invalid=\"false\""));
    }

    #[test]
    fn script_choices_render_the_shared_select_and_keep_an_unresolved_speaker_visible() {
        let document = fixture()
            .set(
                &Target::Dialogue("dialogue-03".into()),
                Field::Speaker,
                "minh-cu",
            )
            .unwrap();
        let html = pane(
            &document,
            vec![],
            false,
            || view! { <DialogueEditor id="dialogue-03".to_owned()/> },
        );
        let speaker = field(&html, "test-dialogue-03-Speaker");
        assert!(
            speaker.contains("class=\"cui-field cui-select-field authoring-field\""),
            "{speaker}"
        );
        assert!(speaker.contains("data-state=\"enabled\""), "{speaker}");
        // SSR keeps the native fallback; the menu trigger enhances it in a supporting browser.
        assert!(tag(&html, "test-dialogue-03-Speaker").contains("aria-invalid=\"true\""));
        assert!(tag(&html, "test-dialogue-03-Speaker-trigger").contains(" hidden"));
        let unresolved = authoring_copy(crate::localization::Locale::Vietnamese).unresolved;
        assert!(
            speaker.contains(&format!(
                "<option value=\"minh-cu\" disabled selected>{unresolved}</option>"
            )),
            "{speaker}"
        );
        for name in ["Người dẫn chuyện", "An", "Minh"] {
            assert!(
                speaker.contains(&format!("lang=\"vi-VN\">{name}</option>")),
                "{speaker}"
            );
        }
        assert!(
            speaker.contains(&format!(
                "id=\"test-dialogue-03-Speaker-error\" class=\"cui-field-error\">{unresolved}"
            )),
            "{speaker}"
        );
        // Interface-copy choices stay in the interface language.
        assert!(html.contains(">Ấm áp</option>"), "{html}");
        assert!(!html.contains("lang=\"vi-VN\">Ấm áp"), "{html}");
    }

    #[test]
    fn read_only_text_stays_native_readonly_and_choices_say_why_they_are_disabled() {
        let html = pane(
            &fixture(),
            vec![],
            true,
            || view! { <DialogueEditor id="dialogue-01".to_owned()/> },
        );
        let text = field(&html, "test-dialogue-01-Text");
        assert!(text.contains("data-state=\"readonly\""), "{text}");
        assert!(
            tag(&html, "test-dialogue-01-Text").contains(" readonly"),
            "{html}"
        );
        let speaker = field(&html, "test-dialogue-01-Speaker");
        assert!(speaker.contains("data-state=\"disabled\""), "{speaker}");
        assert!(tag(&html, "test-dialogue-01-Speaker").contains(" disabled"));
        assert!(
            speaker.contains(&format!(
                ">{}</p>",
                authoring_copy(crate::localization::Locale::Vietnamese).readonly_choice
            )),
            "{speaker}"
        );
    }

    #[test]
    fn editor_tools_drop_outlined_buttons_for_quiet_text_and_tonal_add_actions() {
        let html = pane(
            &fixture(),
            vec![],
            false,
            || view! { <SceneEditor id="scene-01".to_owned()/> },
        );
        assert!(!html.contains("data-variant=\"secondary\""), "{html}");
        let copy = authoring_copy(crate::localization::Locale::Vietnamese);
        for label in [
            copy.add_dialogue,
            copy.add_narration,
            copy.add_cue,
            copy.add_pronunciation,
        ] {
            assert!(
                button(&html, label).contains("data-variant=\"tonal\""),
                "{label}: {html}"
            );
        }
        for label in [copy.move_up, copy.move_down, copy.remove] {
            assert!(
                button(&html, label).contains("data-variant=\"text\""),
                "{label}: {html}"
            );
        }
        assert!(html.matches("data-variant=\"text\"").count() >= 3, "{html}");
    }

    #[test]
    fn choice_projection_keeps_the_model_value_findings_and_content_language() {
        let document = fixture();
        let speaker = |id: &str| Target::Dialogue(id.into());
        let characters: Vec<(String, String)> = document
            .characters()
            .into_iter()
            .map(|character| (character.id, character.name))
            .collect();
        let known = choice_projection(
            &document,
            &speaker("dialogue-02"),
            Field::Speaker,
            characters.clone(),
            &[],
            crate::localization::Locale::Vietnamese,
            true,
        );
        assert_eq!(known.value, "an");
        assert_eq!(known.error, None);
        assert!(known
            .options
            .iter()
            .all(|option| !option.disabled && option.lang.as_deref() == Some("vi-VN")));

        let finding = issue(
            &document,
            &speaker("dialogue-02"),
            Field::Speaker,
            "unknown_speaker",
        );
        let flagged = choice_projection(
            &document,
            &speaker("dialogue-02"),
            Field::Speaker,
            characters.clone(),
            &[finding],
            crate::localization::Locale::English,
            true,
        );
        assert_eq!(
            flagged.error.as_deref(),
            Some(validation_rule(
                "unknown_speaker",
                crate::localization::Locale::English
            ))
        );

        let unknown = document
            .set(&speaker("dialogue-02"), Field::Speaker, "minh-cu")
            .unwrap();
        let unresolved = choice_projection(
            &unknown,
            &speaker("dialogue-02"),
            Field::Speaker,
            characters,
            &[],
            crate::localization::Locale::Vietnamese,
            true,
        );
        assert_eq!(unresolved.value, "minh-cu");
        assert_eq!(unresolved.options[0].value, "minh-cu");
        assert!(unresolved.options[0].disabled);
        assert_eq!(
            unresolved.options[0].lang, None,
            "the placeholder is interface copy"
        );
        assert_eq!(
            unresolved.error.as_deref(),
            Some(authoring_copy(crate::localization::Locale::Vietnamese).unresolved)
        );
        assert_eq!(
            unresolved
                .options
                .iter()
                .filter(|option| option.disabled)
                .count(),
            1
        );
    }

    #[test]
    fn an_unresolved_cue_anchor_is_kept_as_the_disabled_current_choice() {
        let target = Target::Cue("cue-01".into());
        let document = fixture()
            .set(&target, Field::CueDialogue, "dialogue-missing")
            .unwrap();
        let original = document.to_json();
        let projected = choice_projection(
            &document,
            &target,
            Field::CueDialogue,
            vec![("dialogue-01".into(), "Người dẫn chuyện mở màn.".into())],
            &[],
            crate::localization::Locale::Vietnamese,
            true,
        );
        assert_eq!(projected.value, "dialogue-missing");
        assert_eq!(
            projected.options,
            vec![
                SelectOption {
                    value: "dialogue-missing".into(),
                    label: choice("unknown", crate::localization::Locale::Vietnamese).into(),
                    disabled: true,
                    lang: None,
                    icon: None,
                },
                SelectOption {
                    lang: Some("vi-VN".into()),
                    ..SelectOption::new("dialogue-01", "Người dẫn chuyện mở màn.")
                },
            ]
        );
        assert_eq!(
            projected.error.as_deref(),
            Some(choice("unknown", crate::localization::Locale::Vietnamese))
        );
        assert_eq!(document.to_json(), original);
    }

    #[test]
    fn the_navigation_toggle_starts_closed_and_names_the_tree_it_controls() {
        let (html, commits) = editor(&fixture(), vec![], false);
        assert_eq!(commits, 0);
        let toggle = tag(&html, "test-nav-toggle");
        assert!(toggle.contains("aria-controls=\"test-nav\""), "{toggle}");
        assert!(toggle.contains("aria-expanded=\"false\""), "{toggle}");
        assert!(html.contains("data-nav=\"closed\""), "{html}");
        assert!(tag(&html, "test-nav").starts_with("<nav "), "{html}");
        assert!(html.contains(">Điều hướng kịch bản</button>"), "{html}");
    }

    #[test]
    fn studio_css_folds_the_navigation_only_in_the_narrow_container() {
        let css = include_str!("../studio.css");
        let rules = |needle: &str| {
            css.lines()
                .filter(|line| line.contains(needle))
                .collect::<Vec<_>>()
        };
        let toggle = rules(".authoring-nav-toggle");
        assert_eq!(toggle.len(), 2, "{toggle:?}");
        assert!(
            toggle[0].starts_with(".studio-shell .authoring-nav-toggle { display: none; }"),
            "{toggle:?}"
        );
        assert!(
            toggle[1].starts_with("@container studio (max-width: 45rem)")
                && toggle[1].contains("display: block")
                && toggle[1].contains("[data-nav=\"closed\"] > .authoring-nav { display: none; }"),
            "{toggle:?}"
        );
    }

    #[test]
    fn studio_css_leaves_field_focus_and_label_spacing_to_the_shared_anatomy() {
        let css = include_str!("../studio.css");
        let focus = css
            .lines()
            .find(|line| {
                line.contains("summary, button, input, textarea, select")
                    && line.contains(":focus-visible")
            })
            .unwrap();
        assert!(
            focus.contains(":not(.cui-field-control):focus-visible"),
            "{focus}"
        );
        let label = css
            .lines()
            .find(|line| line.contains(".studio-shell :where(label)"))
            .unwrap();
        for shared in [
            ".cui-field > .cui-field-label",
            ".cui-field-shell > .cui-field-label",
        ] {
            assert!(label.contains(shared), "{label}");
        }
    }
}
