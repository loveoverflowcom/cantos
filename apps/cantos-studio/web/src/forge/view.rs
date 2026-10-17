//! Explicit debug composition over the separately tested internal Forge mock contracts.
use super::{
    mock::{MockPort, Scenario},
    model::{
        ConnectionState, ForgePort, Intent, LibraryState, Model, Rejection, RunState, Snapshot,
        StorageState, SyncPlan, SyncState,
    },
};
use crate::{
    messages::forge::{text, Key},
    ui::{ActionButton, BlockReason, ContextSelect, ForgeSyncDialog},
};
use cantos_ui::{ButtonVariant, IconName};
use leptos::{ev::MouseEvent, prelude::*};
use std::collections::BTreeMap;

pub const RECIPES: [(&str, Key); 10] = [
    ("ready", Key::RecipeReady),
    ("empty", Key::RecipeEmpty),
    ("loading", Key::RecipeLoading),
    ("error", Key::RecipeError),
    ("locked", Key::RecipeLocked),
    ("stale", Key::RecipeStale),
    ("run-unknown", Key::RecipeUnknown),
    ("storage-failed", Key::RecipeStorage),
    ("sync-partial", Key::RecipePartial),
    ("sync-unknown", Key::RecipeSyncUnknown),
];
#[cfg(target_arch = "wasm32")]
const OUTCOMES: [(&str, Key, Scenario); 5] = [
    ("success", Key::OutcomeSuccess, Scenario::Success),
    ("failure", Key::OutcomeFailure, Scenario::Failure),
    ("unknown", Key::OutcomeUnknown, Scenario::Unknown),
    ("storage", Key::OutcomeStorage, Scenario::StorageFailure),
    ("partial", Key::OutcomePartial, Scenario::Partial),
];

#[cfg(target_arch = "wasm32")]
pub fn requested() -> bool {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .is_some_and(|query| query == "?preview=studio-forge")
}

fn settle(model: &mut Model, intent: Intent, outcome: Scenario) -> Result<(), Rejection> {
    if let Some(request) = model.reduce(intent)? {
        model.complete(MockPort::new(outcome).complete(&request))?;
    }
    Ok(())
}

/// Replays real mock-controller intents; this does not manufacture UI-only successful states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeError {
    UnknownRecipe,
    Controller(Rejection),
}

pub fn reset_recipe(model: &mut Model, recipe: &str) -> Result<(), RecipeError> {
    if !RECIPES.iter().any(|(id, _)| *id == recipe) {
        return Err(RecipeError::UnknownRecipe);
    }
    run_recipe(model, recipe).map_err(RecipeError::Controller)
}

fn run_recipe(model: &mut Model, recipe: &str) -> Result<(), Rejection> {
    let library = match recipe {
        "empty" => LibraryState::Empty,
        "loading" => LibraryState::Loading,
        "error" => LibraryState::Error,
        "locked" => LibraryState::Locked,
        _ => LibraryState::Ready,
    };
    model.reset(library);
    match recipe {
        "run-unknown" => settle(model, Intent::Generate, Scenario::Unknown),
        "storage-failed" => settle(model, Intent::Generate, Scenario::StorageFailure),
        "stale" | "sync-partial" | "sync-unknown" => {
            settle(model, Intent::Generate, Scenario::Success)?;
            let take = model.snapshot().takes.last().ok_or(Rejection::NoTake)?.id;
            model.reduce(Intent::SelectTake(take))?;
            if recipe == "stale" {
                let edited = format!(
                    "{}\nNgày mai, mình có diễn tiếp không?",
                    model.snapshot().buffer
                );
                model.reduce(Intent::Edit(edited))?;
                model.reduce(Intent::Save)?;
            } else {
                model.reduce(Intent::PreviewSync)?;
                settle(
                    model,
                    Intent::ConfirmSync,
                    if recipe == "sync-partial" {
                        Scenario::Partial
                    } else {
                        Scenario::Unknown
                    },
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn rejection_key(rejection: Rejection) -> Key {
    match rejection {
        Rejection::Disposed => Key::SessionRetired,
        Rejection::LibraryUnavailable | Rejection::NoUncertainOperation => Key::UnavailableAction,
        Rejection::Busy => Key::AdvanceHelp,
        Rejection::NoDraft | Rejection::UnknownDraft => Key::NoDraft,
        Rejection::EmptyText => Key::EmptyText,
        Rejection::TextTooLong => Key::TooLong,
        Rejection::Unchanged => Key::NoChanges,
        Rejection::ConnectionUnavailable => Key::Unavailable,
        Rejection::Unsaved => Key::GenerateHelp,
        Rejection::Unresolved => Key::Unknown,
        Rejection::UnknownTake | Rejection::NotRetained | Rejection::Stale | Rejection::NoTake => {
            Key::SyncHelp
        }
        Rejection::MissingPlan => Key::PlanHelp,
        Rejection::StaleCompletion | Rejection::WrongCompletion => Key::StaleResponse,
        Rejection::LimitReached => Key::Capacity,
    }
}

#[cfg(target_arch = "wasm32")]
fn dispatch(model: RwSignal<Model>, problem: RwSignal<Option<Key>>, intent: Intent) {
    if let Some(result) = model.try_update(|model| model.reduce(intent)) {
        problem.set(result.err().map(rejection_key));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogRecovery {
    PreviewCancelled,
    PreserveOperation,
}

/// Focus failures must never rewrite an already-dispatched operation as an opening failure.
pub fn recover_dialog_error(model: &mut Model, error: cantos_ui::DialogError) -> DialogRecovery {
    if error != cantos_ui::DialogError::ReturnFocusRejected
        && matches!(model.snapshot().sync, SyncState::Preview(_))
        && model.reduce(Intent::CancelSync).is_ok()
    {
        DialogRecovery::PreviewCancelled
    } else {
        DialogRecovery::PreserveOperation
    }
}

#[component]
fn Action(
    #[prop(into)] id: String,
    label: Key,
    icon: IconName,
    english: Signal<bool>,
    on_press: Callback<MouseEvent>,
    #[prop(into, default = false.into())] blocked: Signal<bool>,
    #[prop(default = Key::UnavailableAction)] reason: Key,
    #[prop(default = ButtonVariant::Text)] variant: ButtonVariant,
    #[prop(optional)] focus_on_press: bool,
    #[prop(into, default = String::new().into())] context: Signal<String>,
) -> impl IntoView {
    view! { <ActionButton id=id english=english icon=icon variant=variant blocked=blocked
        reason=BlockReason::Forge(reason) focus_on_press=focus_on_press on_press=on_press>
        {move || text(label, english.get())}
        <span class="cui-sr-only">{move ||context.get()}</span>
    </ActionButton> }
}

#[component]
fn Choice(
    id: &'static str,
    label: Key,
    value: Signal<String>,
    options: Signal<Vec<(String, String)>>,
    english: Signal<bool>,
    on_change: Callback<String>,
    #[prop(into, default = false.into())] readonly: Signal<bool>,
) -> impl IntoView {
    view! { <ContextSelect id=id label=Signal::derive(move || text(label,english.get()).to_owned())
    value=value options=options unknown_label=Signal::derive(move || text(Key::Unavailable,english.get()).to_owned())
    english=english readonly=readonly on_change=on_change/> }
}

/// The explicit debug composition root owns the mock adapter, lifetime and injected preferences.
#[component]
#[cfg(target_arch = "wasm32")]
pub fn Preview(theme: RwSignal<cantos_ui::Theme>) -> impl IntoView {
    use crate::messages::Locale;
    use cantos_ui::Theme;
    let model = RwSignal::new(Model::new());
    let problem = RwSignal::new(None::<Key>);
    let locale = RwSignal::new(Locale::Vietnamese);
    let english = Signal::derive(move || locale.get().is_english());
    let recipe = RwSignal::new("ready".to_owned());
    let outcome = RwSignal::new("success".to_owned());
    let reset_key = RwSignal::new(0_u64);
    let port = StoredValue::new(MockPort::new(Scenario::Success));
    let projection = Signal::derive(move || model.with(Model::snapshot));
    let on_intent = Callback::new(move |intent| dispatch(model, problem, intent));
    on_cleanup(move || {
        let _ = model.try_update(Model::dispose);
    });
    Effect::new(move |_| {
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Some(root) = document.document_element() {
                let _ = root.set_attribute("lang", locale.get().lang());
            }
        }
    });
    let advance = Callback::new(move |_| {
        if let Some(request) = model.with_untracked(|m| m.snapshot().pending) {
            let _ = port.try_update_value(|port| {
                if let Some((_, _, scenario)) = OUTCOMES
                    .iter()
                    .find(|(id, _, _)| *id == outcome.get_untracked())
                {
                    port.set_scenario(*scenario);
                    let result = model.try_update(|m| m.complete(port.complete(&request)));
                    if let Some(result) = result {
                        problem.set(result.err().map(rejection_key));
                    }
                }
            });
        }
    });
    view! {
        <main class="forge-mock" lang=move ||locale.get().lang()>
            <header class="forge-heading"><div><p class="forge-brand">"Cantos Studio"</p><h1>{move ||text(Key::Title,english.get())}</h1><p>{move ||text(Key::Intro,english.get())}</p></div></header>
            <p class="forge-boundary">{move ||text(Key::Boundary,english.get())}</p>
            <details class="forge-diagnostics"><summary>{move ||text(Key::Recipe,english.get())}</summary>
                <div class="forge-preferences">
                    <Choice id="forge-recipe" label=Key::Recipe value=recipe.into()
                        options=Signal::derive(move ||RECIPES.iter().map(|(id,key)|(id.to_string(),text(*key,english.get()).to_owned())).collect()) on_change=Callback::new(move |v|recipe.set(v)) english=english/>
                    <Choice id="forge-theme" label=Key::Theme value=Signal::derive(move ||theme.get().as_str().to_owned())
                        options=Signal::derive(move ||[("light",Key::Light),("dark",Key::Dark),("system",Key::System)].into_iter().map(|(v,k)|(v.to_owned(),text(k,english.get()).to_owned())).collect())
                        on_change=Callback::new(move |v: String|if let Some(value)=Theme::from_attribute(&v){theme.set(value);}) english=english/>
                    <Choice id="forge-locale" label=Key::Locale value=Signal::derive(move ||locale.get().lang().to_owned())
                        options=Signal::derive(||Locale::ALL.into_iter().map(|l|(l.lang().to_owned(),l.autonym().to_owned())).collect())
                        on_change=Callback::new(move |v: String|if let Some(value)=Locale::from_lang(&v){locale.set(value);}) english=english/>
                </div>
                <Action id="forge-reset" label=Key::Reset icon=IconName::Refresh english=english
                    on_press=Callback::new(move |_|{let result=model.try_update(|m|reset_recipe(m,&recipe.get_untracked()));if let Some(result)=result {if result.is_ok(){reset_key.update(|key|*key=key.saturating_add(1));}problem.set(result.err().map(|error|match error{RecipeError::UnknownRecipe=>Key::UnavailableAction,RecipeError::Controller(rejection)=>rejection_key(rejection)}));}})/>
            </details>
            <p id="forge-problem" role="alert" hidden=move ||problem.get().is_none()>{move ||problem.get().map(|key|text(key,english.get()))}</p>
            <Workspace model=projection reset_key=reset_key english=english on_intent=on_intent on_dialog_error=Callback::new(move |error|{
                if let Some(recovery)=model.try_update(|m|recover_dialog_error(m,error)) {
                    match recovery {
                    DialogRecovery::PreserveOperation => {
                        if super::platform::focus_step().is_err(){problem.set(Some(Key::FocusError));}
                    },
                    DialogRecovery::PreviewCancelled => {
                        problem.set(Some(Key::DialogError));
                    },
                    }
                }
            })/>
            <section class="forge-step" aria-labelledby="forge-step-title">
                <h2 id="forge-step-title" tabindex="-1">{move ||text(Key::Advance,english.get())}</h2><p>{move ||text(Key::AdvanceHelp,english.get())}</p>
                <Choice id="forge-outcome" label=Key::Outcome value=outcome.into()
                    options=Signal::derive(move ||OUTCOMES.iter().map(|(id,key,_)|(id.to_string(),text(*key,english.get()).to_owned())).collect())
                    on_change=Callback::new(move |v|outcome.set(v)) english=english/>
                <Action id="forge-advance" label=Key::Advance icon=IconName::ArrowForward english=english on_press=advance
                    blocked=Signal::derive(move ||projection.with(|m|m.pending.is_none())) reason=Key::NotPending variant=ButtonVariant::Tonal/>
            </section>
        </main>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Library,
    Editor,
    Settings,
}

/// Receives immutable mock facts and intents only; never constructs adapters or effects.
#[component]
pub fn Workspace(
    model: Signal<Snapshot>,
    english: Signal<bool>,
    on_intent: Callback<Intent>,
    on_dialog_error: Callback<cantos_ui::DialogError>,
    #[prop(into, default = 0_u64.into())] reset_key: Signal<u64>,
) -> impl IntoView {
    let pane = RwSignal::new(Pane::Library);
    let search = RwSignal::new(String::new());
    let work = RwSignal::new(String::new());
    let composing = RwSignal::new(false);
    // Preserve raw input even when the bounded model rejects a paste. Only an explicit recipe
    // reset clears these buffers; preferences and draft/pane navigation never discard them.
    let buffers = RwSignal::new(BTreeMap::<u64, String>::new());
    Effect::new(move |_| {
        reset_key.get();
        buffers.set(BTreeMap::new());
    });
    let buffer = Signal::derive(move || model.with(|m| editor_buffer(m, &buffers.get())));
    let matches_model = Signal::derive(move || buffer.get() == model.with(|m| m.buffer.clone()));
    let unsaved = Signal::derive(move || model.with(|m| m.dirty) || !matches_model.get());
    let edit = Callback::new(move |value: String| {
        if buffer.get_untracked() == value {
            return;
        }
        if let Some(id) = model.with_untracked(|m| m.current.as_ref().map(|d| d.id)) {
            buffers.update(|buffers| {
                buffers.insert(id, value.clone());
            });
            on_intent.run(Intent::Edit(value));
        }
    });
    let available = Signal::derive(move || model.with(|m| m.library == LibraryState::Ready));
    let editing = Signal::derive(move || {
        available.get() && model.with(|m| m.current.is_some() && m.pending.is_none())
    });
    let drafts = Memo::new(move |_| {
        let query = search.get().to_lowercase();
        let work = work.get();
        model.with(|m| {
            m.drafts
                .iter()
                .filter(|d| {
                    (work.is_empty() || d.work == work)
                        && format!("{} {} {}", d.work, d.chapter, d.title)
                            .to_lowercase()
                            .contains(&query)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    view! {
        <nav class="forge-nav" aria-label=move ||text(Key::Title,english.get())>
            {[("library",Pane::Library,Key::Library,IconName::MenuBook),("editor",Pane::Editor,Key::Editor,IconName::Mic),("settings",Pane::Settings,Key::Settings,IconName::Settings)].into_iter().map(|(id,target,key,icon)|view!{
                <div aria-current=move ||(pane.get()==target).then_some("page")><Action id=format!("forge-nav-{id}") label=key icon=icon english=english on_press=Callback::new(move |_|pane.set(target))/></div>
            }).collect_view()}
        </nav>
        <section class="forge-library" hidden=move ||pane.get()!=Pane::Library aria-labelledby="forge-library-title">
            <div class="forge-section-heading"><h2 id="forge-library-title">{move ||text(Key::Library,english.get())}</h2>
                <Action id="forge-new" label=Key::New icon=IconName::Add english=english variant=ButtonVariant::Tonal
                    blocked=Signal::derive(move ||model.with(|m|!matches!(m.library,LibraryState::Ready|LibraryState::Empty)||m.pending.is_some()))
                    on_press=Callback::new(move |_|{on_intent.run(Intent::NewDraft);pane.set(Pane::Editor);})/>
            </div>
            <div class="cui-field"><label class="cui-field-label" for="forge-search">{move ||text(Key::Search,english.get())}</label>
                <input class="cui-field-control" id="forge-search" type="search" prop:value=move ||search.get() on:input=move |e|search.set(event_target_value(&e))/></div>
            <Choice id="forge-work-filter" label=Key::Work value=work.into() english=english
                options=Signal::derive(move ||{
                    let mut options=vec![(String::new(),text(Key::All,english.get()).to_owned())];
                    for draft in model.with(|m|m.drafts.clone()) {if !options.iter().any(|(id,_)|*id==draft.work){options.push((draft.work.clone(),draft.work));}}
                    options
                }) on_change=Callback::new(move |value|work.set(value))/>
            <p role="status">{move ||match model.with(|m|m.library){LibraryState::Ready=>if drafts.with(|d|d.is_empty()){text(Key::NoMatch,english.get())}else{""},LibraryState::Empty=>text(Key::Empty,english.get()),LibraryState::Loading=>text(Key::Loading,english.get()),LibraryState::Error=>text(Key::LibraryError,english.get()),LibraryState::Locked=>text(Key::Locked,english.get())}}</p>
            <ul class="forge-drafts" hidden=move ||!available.get()><For each=move ||drafts.get() key=|d|d.id children=move |draft|{
                let id=draft.id;
                let context=draft.title.clone();
                view! {<li><div><p class="forge-eyebrow" lang="vi-VN">{draft.work}<span>" · "</span>{draft.chapter}</p><h3 lang="vi-VN">{draft.title}</h3></div>
                    <Action id=format!("forge-open-{id}") label=Key::Open icon=IconName::ArrowForward english=english context=Signal::derive(move ||context.clone()) on_press=Callback::new(move |_|{on_intent.run(Intent::Open(id));pane.set(Pane::Editor);})/>
                </li>}
            }/></ul>
        </section>
        <section hidden=move ||pane.get()!=Pane::Editor aria-labelledby="forge-editor-title">
            <h2 id="forge-editor-title">{move ||text(Key::Editor,english.get())}</h2>
            <p hidden=move ||model.with(|m|m.current.is_some())>{move ||text(Key::NoDraft,english.get())}</p>
            <div class="forge-editor-grid" hidden=move ||!available.get()||model.with(|m|m.current.is_none())>
                <section class="forge-script"><div class="forge-section-heading"><h3>{move ||model.with(|m|m.current.as_ref().map(|d|d.title.clone()))}</h3>
                    <Action id="forge-save" label=Key::Save icon=IconName::Save english=english
                        blocked=Signal::derive(move ||!editing.get()||!matches_model.get()||model.with(|m|!m.dirty||m.buffer.trim().is_empty()))
                        on_press=Callback::new(move |_|on_intent.run(Intent::Save))/></div>
                    <p class="forge-eyebrow" lang="vi-VN">{move ||model.with(|m|m.current.as_ref().map(|d|format!("{} · {}",d.work,d.chapter)))}</p>
                    <p class="forge-revision" role="status">{move ||format!("{} · r{} · {}",text(Key::Revision,english.get()),model.with(|m|m.current.as_ref().and_then(|d|d.revisions.last().map(|r|r.number)).unwrap_or(0)),text(if unsaved.get(){Key::Dirty}else{Key::Saved},english.get()))}</p>
                    <div class="cui-field"><label class="cui-field-label" for="forge-script">{move ||text(Key::Script,english.get())}</label>
                        <textarea id="forge-script" class="cui-field-control cui-textarea" lang="vi-VN" readonly=move ||!editing.get() aria-describedby="forge-script-help forge-script-error forge-rights" aria-invalid=move ||!matches_model.get() prop:value=move ||buffer.get()
                            on:compositionstart=move |_|composing.set(true)
                            on:compositionend=move |e|{composing.set(false);edit.run(event_target_value(&e));}
                            on:input=move |e|if !composing.get_untracked(){edit.run(event_target_value(&e));}/>
                        <p class="cui-field-help" id="forge-script-help">{move ||text(Key::ScriptHelp,english.get())}</p>
                        <p class="cui-field-error" id="forge-script-error" hidden=move ||matches_model.get()>{move ||text(Key::TooLong,english.get())}</p>
                    </div>
                    <p id="forge-rights" class="forge-muted">{move ||text(Key::Rights,english.get())}</p>
                </section>
                <aside class="forge-takes" aria-labelledby="forge-takes-title"><h3 id="forge-takes-title">{move ||text(Key::Takes,english.get())}</h3>
                    <p role="status">{move ||text(run_key(&model.with(|m|m.run.clone())),english.get())}</p>
                    <p class="forge-muted">{move ||run_identity(&model.with(|m|m.run.clone()),english.get())}</p>
                    <Action id="forge-generate" label=Key::Generate icon=IconName::Mic english=english variant=ButtonVariant::Primary
                        reason=Key::GenerateHelp blocked=Signal::derive(move ||!matches_model.get()||model.with(|m|!m.can_generate)) on_press=Callback::new(move |_|on_intent.run(Intent::Generate))/>
                    <p hidden=move ||model.with(|m|!m.takes.is_empty())>{move ||text(Key::NoTakes,english.get())}</p>
                    <ul class="forge-take-list"><For each=move ||model.with(|m|m.takes.iter().map(|t|t.id).collect::<Vec<_>>()) key=|id|*id children=move |id|view!{<TakeRow id=id model=model unsaved=unsaved english=english on_intent=on_intent/>}/></ul>
                    <p class="forge-muted">{move ||text(Key::AudioUnavailable,english.get())}</p>
                    <Action id="forge-play" label=Key::Play icon=IconName::PlayArrow english=english blocked=true reason=Key::AudioUnavailable on_press=Callback::new(|_|{})/>
                    <Action id="forge-reconcile" label=Key::Reconcile icon=IconName::Refresh english=english reason=Key::UnavailableAction
                        blocked=Signal::derive(move ||model.with(|m|m.pending.is_some()||m.connection!=ConnectionState::Connected||!matches!(m.run,RunState::Uncertain{..})&&!matches!(m.sync,SyncState::Unknown{..}|SyncState::Partial{..})))
                        on_press=Callback::new(move |_|on_intent.run(Intent::Reconcile))/>
                    <hr/><p role="status">{move ||text(sync_key(&model.with(|m|m.sync.clone())),english.get())}</p>
                    <p class="forge-plan-identity">{move ||sync_identity(&model.with(|m|m.sync.clone()),english.get())}</p>
                    <Action id="forge-sync" label=Key::Sync icon=IconName::CloudSync english=english variant=ButtonVariant::Tonal focus_on_press=true
                        blocked=Signal::derive(move ||!matches_model.get()||model.with(|m|!m.can_sync)) reason=Key::SyncHelp on_press=Callback::new(move |_|on_intent.run(Intent::PreviewSync))/>
                </aside>
            </div>
        </section>
        <section class="forge-settings" hidden=move ||pane.get()!=Pane::Settings aria-labelledby="forge-settings-title"><h2 id="forge-settings-title">{move ||text(Key::Settings,english.get())}</h2>
            <h3>{move ||text(Key::Provider,english.get())}</h3><p>{move ||text(Key::ProviderValue,english.get())}</p><p>{move ||text(Key::ProviderHelp,english.get())}</p>
            <Choice id="forge-connection" label=Key::Connection english=english value=Signal::derive(move ||if model.with(|m|m.connection==ConnectionState::Connected){"connected"}else{"unavailable"}.to_owned())
                options=Signal::derive(move ||[("connected",Key::Connected),("unavailable",Key::Unavailable)].into_iter().map(|(v,k)|(v.to_owned(),text(k,english.get()).to_owned())).collect())
                readonly=Signal::derive(move ||!available.get()||model.with(|m|m.pending.is_some()))
                on_change=Callback::new(move |v: String|match v.as_str(){"connected"=>on_intent.run(Intent::SetConnection(ConnectionState::Connected)),"unavailable"=>on_intent.run(Intent::SetConnection(ConnectionState::Unavailable)),_=>{}})/>
            <h3>{move ||text(Key::DraftStore,english.get())}</h3><p>{move ||text(Key::DraftStoreHelp,english.get())}</p>
        </section>
        <ForgeSyncDialog open=Signal::derive(move ||model.with(|m|matches!(m.sync,SyncState::Preview(_)))) english=english
            details=Signal::derive(move ||model.with(|m|match &m.sync{SyncState::Preview(plan)=>plan_text(plan,english.get()),_=>String::new()}))
            on_cancel=Callback::new(move |_|on_intent.run(Intent::CancelSync))
            on_confirm=Callback::new(move |_|on_intent.run(Intent::ConfirmSync)) on_error=on_dialog_error/>
    }
}

#[component]
fn TakeRow(
    id: u64,
    model: Signal<Snapshot>,
    english: Signal<bool>,
    on_intent: Callback<Intent>,
    unsaved: Signal<bool>,
) -> impl IntoView {
    let take = Memo::new(move |_| model.with(|m| m.takes.iter().find(|t| t.id == id).cloned()));
    view! {<li aria-current=move ||model.with(|m|(m.selected_take==Some(id)).then_some("true"))>
        <h4>{move ||format!("{} {id} · r{}",text(Key::Take,english.get()),take.with(|t|t.as_ref().map(|t|t.revision).unwrap_or(0)))}</h4>
        <p>{move ||text(if unsaved.get()||take.with(|t|t.as_ref().is_none_or(|t|t.stale)){Key::Stale}else{Key::Current},english.get())}</p>
        <p>{move ||format!("{}: {}",text(Key::Storage,english.get()),text(if take.with(|t|t.as_ref().is_some_and(|t|t.storage==StorageState::Retained)){Key::RetainedStorage}else{Key::FailedStorage},english.get()))}</p>
        <p hidden=move ||model.with(|m|m.selected_take!=Some(id))>{move ||text(Key::Selected,english.get())}</p>
        <Action id=format!("forge-take-{id}") label=Key::SelectTake icon=IconName::Check english=english
            context=Signal::derive(move ||format!("{} {id}",text(Key::Take,english.get())))
            blocked=Signal::derive(move ||unsaved.get()||model.with(|m|m.pending.is_some())||take.with(|t|t.as_ref().is_none_or(|t|t.stale||t.storage!=StorageState::Retained)))
            reason=Key::SyncHelp on_press=Callback::new(move |_|on_intent.run(Intent::SelectTake(id)))/>
    </li>}
}

fn run_key(run: &RunState) -> Key {
    match run {
        RunState::Idle => Key::Idle,
        RunState::Running { .. } => Key::Running,
        RunState::Retained { .. } => Key::Retained,
        RunState::Failed { .. } => Key::Failed,
        RunState::Uncertain { .. } => Key::Unknown,
        RunState::StorageFailed { .. } => Key::StorageFailed,
    }
}

fn editor_buffer(model: &Snapshot, buffers: &BTreeMap<u64, String>) -> String {
    model
        .current
        .as_ref()
        .and_then(|draft| buffers.get(&draft.id))
        .cloned()
        .unwrap_or_else(|| model.buffer.clone())
}
fn sync_key(sync: &SyncState) -> Key {
    match sync {
        SyncState::Idle => Key::SyncIdle,
        SyncState::Preview(_) => Key::Plan,
        SyncState::Sending { .. } => Key::Sending,
        SyncState::Receipt { .. } => Key::Receipt,
        SyncState::Partial { .. } => Key::Partial,
        SyncState::Unknown { .. } => Key::SyncUnknown,
    }
}
fn plan_text(plan: &SyncPlan, english: bool) -> String {
    format!(
        "{} {} · {} {} · {} {}",
        text(Key::DraftIdentity, english),
        plan.draft,
        text(Key::Revision, english),
        plan.revision,
        text(Key::Take, english),
        plan.take
    )
}

fn sync_identity(sync: &SyncState, english: bool) -> String {
    match sync {
        SyncState::Idle => String::new(),
        SyncState::Preview(plan) => plan_text(plan, english),
        SyncState::Sending { operation, plan }
        | SyncState::Receipt { operation, plan }
        | SyncState::Partial { operation, plan }
        | SyncState::Unknown { operation, plan } => format!(
            "{} {operation} · {}",
            text(Key::Operation, english),
            plan_text(plan, english)
        ),
    }
}

fn run_identity(run: &RunState, english: bool) -> String {
    match run {
        RunState::Idle => String::new(),
        RunState::Running { attempt, revision }
        | RunState::Failed { attempt, revision }
        | RunState::Uncertain { attempt, revision } => {
            format!("{} {attempt} · r{revision}", text(Key::Attempt, english))
        }
        RunState::Retained { attempt, take } | RunState::StorageFailed { attempt, take } => {
            format!(
                "{} {attempt} · {} {take}",
                text(Key::Attempt, english),
                text(Key::Take, english)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes_replay_controller_states_and_unknown_recipe_preserves_the_session() {
        for (id, _) in RECIPES {
            let mut model = Model::new();
            reset_recipe(&mut model, id).unwrap();
            let state = model.snapshot();
            match id {
                "empty" => assert_eq!(state.library, LibraryState::Empty),
                "loading" => assert_eq!(state.library, LibraryState::Loading),
                "error" => assert_eq!(state.library, LibraryState::Error),
                "locked" => {
                    assert_eq!(state.library, LibraryState::Locked);
                    assert!(state.drafts.is_empty());
                }
                "stale" => {
                    assert!(state.takes[0].stale);
                    assert!(!state.can_sync);
                }
                "run-unknown" => assert!(matches!(state.run, RunState::Uncertain { .. })),
                "storage-failed" => assert!(matches!(state.run, RunState::StorageFailed { .. })),
                "sync-partial" => assert!(matches!(state.sync, SyncState::Partial { .. })),
                "sync-unknown" => assert!(matches!(state.sync, SyncState::Unknown { .. })),
                "ready" => assert!(state.can_generate),
                _ => panic!("unasserted recipe"),
            }
            assert!(state.pending.is_none());
            let before = model.clone();
            assert_eq!(
                reset_recipe(&mut model, "unknown-recipe"),
                Err(RecipeError::UnknownRecipe)
            );
            assert_eq!(model, before);
        }
    }

    #[test]
    fn native_render_keeps_mock_labels_and_names_without_dispatch_or_audio_claim() {
        let owner = Owner::new();
        owner.with(||{
            for english in [false,true] {
                let calls=RwSignal::new(0);
                let model=RwSignal::new(Model::new());
                let html=view!{<Workspace model=Signal::derive(move ||model.with(Model::snapshot)) english=Signal::derive(move ||english)
                    on_intent=Callback::new(move |_|calls.update(|n|*n+=1)) on_dialog_error=Callback::new(|_|{})/>}.to_html();
                assert_eq!(calls.get_untracked(),0);
                assert_eq!(html.matches("id=\"forge-script\"").count(),1);
                assert!(html.contains(text(Key::Generate,english)),"{html}");
                assert!(html.contains(text(Key::AudioUnavailable,english)));
                assert!(html.contains("type=\"button\""));
                assert!(html.contains("aria-hidden=\"true\""));
                assert!(!html.contains("<audio"));
                assert!(!html.contains("type=\"password\""));
            }
        });
    }

    #[test]
    fn editing_marks_retained_take_stale_before_save_without_restarting_generation() {
        let mut model = Model::new();
        settle(&mut model, Intent::Generate, Scenario::Success).unwrap();
        let before = model.snapshot().run;
        model
            .reduce(Intent::Edit("Lời thoại đang sửa, chưa lưu.".into()))
            .unwrap();
        let state = model.snapshot();
        assert!(state.takes[0].stale);
        assert!(!state.can_generate);
        assert!(!state.can_sync);
        assert_eq!(state.run, before);
        assert!(state.pending.is_none());
    }

    #[test]
    fn oversized_raw_editor_text_survives_rejection_and_draft_navigation_for_correction() {
        let mut model = Model::new();
        let raw = "Tiếng Việt, giữ nguyên bản dán. ".repeat(1024);
        let mut buffers = BTreeMap::new();
        buffers.insert(1, raw.clone());
        assert_eq!(
            model.reduce(Intent::Edit(raw.clone())),
            Err(Rejection::TextTooLong)
        );
        assert_eq!(editor_buffer(&model.snapshot(), &buffers), raw);
        assert_ne!(model.snapshot().buffer, raw);
        model.reduce(Intent::Open(2)).unwrap();
        assert_eq!(
            editor_buffer(&model.snapshot(), &buffers),
            model.snapshot().buffer
        );
        model.reduce(Intent::Open(1)).unwrap();
        assert_eq!(editor_buffer(&model.snapshot(), &buffers), raw);
        // Explicit reset clears the shell buffers; theme/locale changes have no such effect.
        buffers.clear();
        assert_eq!(
            editor_buffer(&model.snapshot(), &buffers),
            model.snapshot().buffer
        );
    }

    #[test]
    fn dialog_focus_failure_preserves_dispatched_sync_and_its_exact_identity() {
        let mut model = Model::new();
        reset_recipe(&mut model, "sync-partial").unwrap();
        let identity = sync_identity(&model.snapshot().sync, true);
        assert!(identity.contains("Draft"));
        model.reduce(Intent::Reconcile).unwrap();
        let before = model.clone();
        for error in [
            cantos_ui::DialogError::ReturnFocusRejected,
            cantos_ui::DialogError::OpenRejected,
        ] {
            assert_eq!(
                recover_dialog_error(&mut model, error),
                DialogRecovery::PreserveOperation
            );
            assert_eq!(model, before);
        }
        let mut fresh = Model::new();
        settle(&mut fresh, Intent::Generate, Scenario::Success).unwrap();
        let take = fresh.snapshot().takes[0].id;
        fresh.reduce(Intent::SelectTake(take)).unwrap();
        fresh.reduce(Intent::PreviewSync).unwrap();
        assert_eq!(
            recover_dialog_error(&mut fresh, cantos_ui::DialogError::OpenRejected),
            DialogRecovery::PreviewCancelled
        );
        assert_eq!(fresh.snapshot().sync, SyncState::Idle);
    }
}
