//! The Theatre-presentation preview of the exact saved Work revision (ADR 0054 D03, #650).
//! Everything shown comes from `Facts::preview`, which the request owner fills only with a reply
//! that named the revision asked for. Account switch, Work change and route change drop it with
//! the other facts; a late reply for another context never reaches it.
use super::{
    copy::{kind_language, revision_label, text, unit_kind, work_kind, Key},
    preview::{preview_more, preview_saved, AfterSave, Problem},
    workspace::Dispatch,
    Command, Facts, Phase,
};
use crate::ui::{ActionButton, BlockReason};
use cantos_ui::{ButtonVariant, IconName};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

const TITLE_ID: &str = "live-preview-title";
const OPEN_ID: &str = "live-preview-open";

fn focus(id: &str) {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(id))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}

#[component]
pub(in crate::live) fn PreviewPanel(
    facts: RwSignal<Facts>,
    dispatch: Dispatch,
    locale: Signal<crate::localization::Locale>,
    blocked: Signal<bool>,
    unsaved: Signal<bool>,
    active: Signal<bool>,
    after_save: RwSignal<Option<AfterSave>>,
) -> impl IntoView {
    // Leaving the workspace route ends the preview: draft text must not wait in a hidden pane,
    // and a pending preview read is already cancelled by the workspace when it goes inactive.
    Effect::new(move |_| {
        if !active.get() {
            after_save.set(None);
            if facts.with_untracked(|facts| facts.preview.is_some()) {
                facts.update(|facts| facts.preview = None);
            }
        }
    });
    // "Save, then preview" previews only its own acknowledged save, never a later one.
    Effect::new(move |_| {
        let follow_up = dispatch.state.with(|state| {
            let armed = after_save.get_untracked()?;
            match (state.command(), state.result()) {
                (Some(command), Some(reply)) => armed.follow_up(command, reply),
                _ => None,
            }
        });
        if let Some(command) = follow_up {
            after_save.set(None);
            dispatch.send(command);
        } else if dispatch
            .state
            .with(|state| matches!(state.phase(), Phase::Failed | Phase::Ambiguous))
        {
            after_save.set(None);
        }
    });
    let shown = Signal::derive(move || facts.with(|facts| facts.preview.clone()));
    let loading = Signal::derive(move || {
        dispatch
            .state
            .with(|state| matches!(state.pending(), Some(Command::Preview { .. })))
    });
    let problem = Signal::derive(move || {
        dispatch
            .state
            .with(|state| match (state.command(), state.error()) {
                (Some(Command::Preview { .. }), Some(error)) => Some(Problem::of(error)),
                _ => None,
            })
    });
    // The first page of a new preview takes focus; closing hands it back to the trigger.
    Effect::new(move |previous: Option<bool>| {
        let open = shown.with(Option::is_some);
        if open && previous == Some(false) {
            focus(TITLE_ID);
        }
        open
    });
    let has_work = Signal::derive(move || facts.with(|facts| facts.work.is_some()));
    view! {
        <section class="cw-preview-zone" aria-labelledby=TITLE_ID>
        <h5 id=TITLE_ID tabindex="-1">{move || text(Key::PreviewTitle, locale.get())}</h5>
        <div class="cw-actions">
            <ActionButton id=OPEN_ID locale=locale variant=ButtonVariant::Tonal icon=IconName::Visibility
                reason=BlockReason::ContextReady
                blocked=Signal::derive(move || blocked.get() || unsaved.get() || !has_work.get())
                on_press=Callback::new(move |_| {
                    if blocked.get_untracked() || unsaved.get_untracked() { return; }
                    if let Some(command) = facts.with_untracked(|facts| preview_saved(facts.work.as_ref())) {
                        dispatch.send(command);
                    }
                })>{move || text(Key::PreviewOpen, locale.get())}</ActionButton>
        </div>
        <p class="help cw-reason" id="live-preview-unsaved" role="status" hidden=move || !unsaved.get()>
            {move || text(Key::PreviewUnsaved, locale.get())}
        </p>
        <p class="help" id="live-preview-loading" role="status" hidden=move || !loading.get()>
            {move || text(Key::PreviewLoading, locale.get())}
        </p>
        <div id="live-preview-problem" class="cw-preview-problem" role="alert" hidden=move || problem.get().is_none()>
            <p>{move || text(match problem.get() {
                Some(Problem::Stale) => Key::PreviewStale,
                Some(Problem::Archived) => Key::PreviewArchived,
                Some(Problem::Unavailable) => Key::PreviewUnavailable,
                _ => Key::PreviewFailed,
            }, locale.get())}</p>
            <ActionButton id="live-preview-reload" locale=locale variant=ButtonVariant::Text
                reason=BlockReason::ContextReady
                blocked=Signal::derive(move || blocked.get() || problem.get() != Some(Problem::Stale))
                on_press=Callback::new(move |_| {
                    if blocked.get_untracked() { return; }
                    if let Some(work) = facts.with_untracked(|facts| facts.work.clone()) {
                        dispatch.send(Command::Work { id: work.id });
                    }
                })>{move || text(Key::PreviewReload, locale.get())}</ActionButton>
        </div>
        <Show when=move || shown.with(Option::is_some)>
            <div id="live-preview" class="cw-preview"
                data-revision=move || shown.with(|shown| shown.as_ref().map(|shown| shown.work().revision.to_string()).unwrap_or_default())>
                <p id="live-preview-banner" class="cw-preview-banner" role="status">
                    {move || text(Key::PreviewBanner, locale.get())}
                    " "
                    <span class="cw-preview-revision">{move || shown.with(|shown| shown.as_ref().map(|shown| revision_label(shown.work().revision)).unwrap_or_default())}</span>
                </p>
                <article class="cw-preview-surface">
                    <h6 class="cw-preview-work" lang=move || shown.with(|shown| shown.as_ref().map(|shown| shown.work().language.clone()))>
                        {move || shown.with(|shown| shown.as_ref().map(|shown| shown.work().title.clone()).unwrap_or_default())}
                    </h6>
                    <p class="cw-preview-meta">
                        {move || shown.with(|shown| shown.as_ref().map(|shown| kind_language(work_kind(&shown.work().kind, locale.get()), &shown.work().language)).unwrap_or_default())}
                    </p>
                    <p class="cw-preview-synopsis" lang=move || shown.with(|shown| shown.as_ref().map(|shown| shown.work().language.clone()))>
                        {move || shown.with(|shown| shown.as_ref().map(|shown| shown.work().synopsis.clone()).unwrap_or_default())}
                    </p>
                    <p class="help cw-preview-unavailable">{move || text(Key::PreviewCoverUnavailable, locale.get())}</p>
                    <h6 id="live-preview-units-title">{move || text(Key::PreviewUnits, locale.get())}</h6>
                    <p class="help cw-preview-unavailable">{move || text(Key::PreviewAudioUnavailable, locale.get())}</p>
                    <p class="help" hidden=move || shown.with(|shown| shown.as_ref().is_none_or(|shown| !shown.units().is_empty()))>
                        {move || text(Key::PreviewNoUnits, locale.get())}
                    </p>
                    <ol class="cw-preview-units" aria-labelledby="live-preview-units-title">
                        <For each=move || shown.with(|shown| shown.as_ref().map(|shown| shown.units().to_vec()).unwrap_or_default())
                            key=|unit| unit.id.clone() children=move |unit| {
                            let language = shown.with_untracked(|shown| shown.as_ref().map(|shown| shown.work().language.clone()));
                            view! {
                                <li value=unit.ordinal lang=language>
                                    <strong class="cw-preview-unit-title">{unit.title}</strong>
                                    <span class="cw-preview-unit-kind">{move || unit_kind(&unit.kind, locale.get())}</span>
                                    <span class="cw-preview-unit-description">{unit.description}</span>
                                </li>
                            }
                        }/>
                    </ol>
                </article>
                <div class="cw-actions">
                    <ActionButton id="live-preview-more" locale=locale variant=ButtonVariant::Tonal reason=BlockReason::PageNavigation
                        blocked=Signal::derive(move || blocked.get() || shown.with(|shown| shown.as_ref().is_none_or(|shown| shown.next().is_none())))
                        on_press=Callback::new(move |_| {
                            if blocked.get_untracked() { return; }
                            if let Some(command) = shown.with_untracked(|shown| shown.as_ref().and_then(preview_more)) {
                                dispatch.send(command);
                            }
                        })>{move || text(Key::PreviewMore, locale.get())}</ActionButton>
                    <ActionButton id="live-preview-close" locale=locale variant=ButtonVariant::Text icon=IconName::Close
                        reason=BlockReason::ContextReady
                        on_press=Callback::new(move |_| {
                            facts.update(|facts| facts.preview = None);
                            focus(OPEN_ID);
                        })>{move || text(Key::PreviewClose, locale.get())}</ActionButton>
                </div>
            </div>
        </Show>
        </section>
    }
}
