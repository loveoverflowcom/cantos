//! App-owned overview of the existing workspace projection and request authority.
use super::{
    copy::work_kind,
    dashboard_copy::{text, Key},
    ClientError, Facts, Reply,
};
use crate::contract::AudioSubmissionState;
use crate::ui::{ActionButton, BlockReason};
use cantos_ui::{ButtonVariant, Surface};
use leptos::prelude::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CatalogRead {
    #[default]
    NotRead,
    Loading,
    Ready,
    Failed(ClientError),
}

/// One initial bounded read per confirmed owner observation. A failed read is retried only by
/// explicit intent; a later request result never silently turns a failed list into empty data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DashboardQuery {
    context: Option<(String, u64)>,
    pub read: CatalogRead,
}

impl DashboardQuery {
    pub fn attempted(&self, actor: &str, epoch: u64) -> bool {
        self.context
            .as_ref()
            .is_some_and(|(owner, observed)| owner == actor && *observed == epoch)
    }

    pub fn start(&self, actor: &str, epoch: u64) -> Self {
        Self {
            context: Some((actor.into(), epoch)),
            read: CatalogRead::Loading,
        }
    }

    pub fn finish(&self, reply: &Result<Reply, ClientError>) -> Self {
        Self {
            read: match reply {
                Ok(Reply::Works(_)) => CatalogRead::Ready,
                Err(error) => CatalogRead::Failed(error.clone()),
                Ok(_) => CatalogRead::Failed(ClientError::ReplyMismatch),
            },
            ..self.clone()
        }
    }
}

fn lifecycle(value: &str, en: bool) -> &str {
    match value {
        "draft" => text(Key::Draft, en),
        "archived" => text(Key::Archived, en),
        _ => text(Key::UnknownLifecycle, en),
    }
}

fn audio_state(state: AudioSubmissionState, en: bool) -> &'static str {
    text(
        match state {
            AudioSubmissionState::AwaitingBytes => Key::AudioAwaiting,
            AudioSubmissionState::Admitted => Key::AudioAdmitted,
            AudioSubmissionState::Revoked => Key::AudioRevoked,
            AudioSubmissionState::Expired => Key::AudioExpired,
            AudioSubmissionState::Stale => Key::AudioStale,
        },
        en,
    )
}

#[component]
pub fn Dashboard(
    facts: RwSignal<Facts>,
    query: RwSignal<DashboardQuery>,
    english: Signal<bool>,
    blocked: Signal<bool>,
    session_pending: Signal<bool>,
    on_new_work: Callback<()>,
    on_import: Callback<()>,
    on_editor: Callback<()>,
    on_continue: Callback<()>,
    on_audio: Callback<()>,
    on_refresh: Callback<()>,
    on_previous: Callback<()>,
    on_next: Callback<()>,
    on_pick: Callback<String>,
) -> impl IntoView {
    view! {
        <section id="studio-dashboard" class="studio-dashboard" aria-labelledby="dashboard-title">
            <header class="dashboard-heading">
                <div><h2 id="dashboard-title" tabindex="-1">{move || text(Key::Title, english.get())}</h2>
                    <p class="dashboard-lead">{move || text(Key::Lead, english.get())}</p></div>
                <div class="dashboard-actions">
                    <ActionButton id="dashboard-create-work" english=english variant=ButtonVariant::Tonal blocked=blocked on_press=Callback::new(move |_| on_new_work.run(()))>{move || text(Key::NewWork, english.get())}</ActionButton>
                    <ActionButton id="dashboard-import" english=english variant=ButtonVariant::Primary blocked=session_pending on_press=Callback::new(move |_| on_import.run(()))>{move || text(Key::Import, english.get())}</ActionButton>
                </div>
            </header>
            <div class="dashboard-grid">
                <Surface class="dashboard-resume">
                    <section aria-labelledby="dashboard-resume-title">
                        <p class="dashboard-eyebrow">{move || text(Key::Resume, english.get())}</p>
                        <h3 id="dashboard-resume-title" lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move || facts.with(|f| f.work.as_ref().map(|w| w.details.title.clone()).unwrap_or_else(|| text(Key::NoSelection, english.get()).into()))}</h3>
                        <Show when=move || facts.with(|f| f.unit.is_some())>
                            <p>{move || text(Key::SelectedUnit, english.get())}" · "<span lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move || facts.with(|f| f.unit.as_ref().map(|u| u.details.title.clone()).unwrap_or_default())}</span></p>
                        </Show>
                        <Show when=move ||facts.with(|f|f.work.is_some())><ActionButton id="dashboard-continue" english=english blocked=Signal::derive(move || session_pending.get() || facts.with(|f| f.work.is_none())) reason=BlockReason::ContextReady on_press=Callback::new(move |_| on_continue.run(()))>{move || text(Key::Continue, english.get())}</ActionButton></Show>
                    </section>
                </Surface>
                <Surface class="dashboard-works">
                    <section aria-labelledby="dashboard-works-title">
                        <div class="dashboard-heading"><h3 id="dashboard-works-title" tabindex="-1">{move || text(Key::Works, english.get())}</h3>
                            <ActionButton id="dashboard-refresh" english=english variant=ButtonVariant::Text blocked=blocked on_press=Callback::new(move |_| on_refresh.run(()))>{move || text(Key::Refresh, english.get())}</ActionButton></div>
                        <p class="help">{move || text(Key::PageHelp, english.get())}</p>
                        <div id="dashboard-status" class="dashboard-status" tabindex="-1">
                            <Show when=move || query.with(|q| q.read == CatalogRead::NotRead)><p role="status" aria-live="polite">{move || text(Key::NotRead, english.get())}</p></Show>
                            <Show when=move || query.with(|q| q.read == CatalogRead::Loading)><p role="status" aria-live="polite">{move || text(Key::Loading, english.get())}</p></Show>
                            <Show when=move || query.with(|q| matches!(q.read, CatalogRead::Failed(_)))>
                                <div role="status" aria-live="polite"><h4>{move ||text(Key::ReadFailed,english.get())}</h4><p>{move ||text(Key::RetryHelp,english.get())}</p></div>
                            </Show>
                            <Show when=move || query.with(|q| q.read == CatalogRead::Ready) && facts.with(|f| f.works.is_empty())>
                                <div><h4>{move ||text(Key::EmptyTitle,english.get())}</h4><p>{move ||text(Key::EmptyHelp,english.get())}</p></div>
                            </Show>
                        </div>
                        <ul class="dashboard-work-list">
                            {move || facts.with(|f| f.works.clone()).into_iter().map(|work| {
                                let id = work.id.clone();
                                let selected_id = work.id.clone();
                                view! { <li class="dashboard-work-row">
                                    <ActionButton id=format!("dashboard-work-{}", work.id) english=english variant=ButtonVariant::Text
                                        blocked=Signal::derive(move || session_pending.get() || (blocked.get() && facts.with(|f| f.work.as_ref().is_none_or(|w| w.id != selected_id))))
                                        on_press=Callback::new(move |_| on_pick.run(id.clone()))><span lang=work.details.language>{work.details.title}</span></ActionButton>
                                    <p class="dashboard-work-meta">{move || format!("{} · {} · r{}", work_kind(&work.kind, english.get()), lifecycle(&work.lifecycle, english.get()), work.revision)}</p>
                                </li> }
                            }).collect_view()}
                        </ul>
                        <Show when=move || query.with(|q| q.read == CatalogRead::Ready)>
                            <p class="help">{move || text(if facts.with(|f| f.works_next.is_some()) { Key::More } else { Key::End }, english.get())}</p>
                        </Show>
                        <div class="toolbar"><ActionButton id="dashboard-previous" english=english reason=BlockReason::PageNavigation variant=ButtonVariant::Tonal blocked=Signal::derive(move || blocked.get() || facts.with(|f| f.pages.previous_works().is_none())) on_press=Callback::new(move |_| on_previous.run(()))>{move || text(Key::Previous, english.get())}</ActionButton>
                        <Show when=move ||facts.with(|f|f.works_next.is_some())><ActionButton id="dashboard-next" english=english reason=BlockReason::PageNavigation variant=ButtonVariant::Tonal blocked=Signal::derive(move || blocked.get() || facts.with(|f| f.works_next.is_none())) on_press=Callback::new(move |_| on_next.run(()))>{move || text(Key::Next, english.get())}</ActionButton></Show></div>
                    </section>
                </Surface>
                <Surface class="dashboard-audio">
                    <section aria-labelledby="dashboard-audio-title">
                        <h3 id="dashboard-audio-title">{move || text(Key::Audio, english.get())}</h3>
                        <p>{move || facts.with(|f| f.submission.as_ref().map(|s| audio_state(s.state, english.get())).unwrap_or_else(|| text(Key::AudioNotRead, english.get())))}</p>
                        <Show when=move || facts.with(|f| f.candidate.is_some())><p>{move || text(Key::Candidate, english.get())}</p></Show>
                        <p class="help">{move || text(Key::AudioHelp, english.get())}</p>
                        <ActionButton id="dashboard-audio" english=english variant=ButtonVariant::Tonal blocked=session_pending on_press=Callback::new(move |_| on_audio.run(()))>{move || text(Key::OpenAudio, english.get())}</ActionButton>
                    </section>
                </Surface>
                <Surface class="dashboard-tools"><section aria-labelledby="dashboard-editor-title">
                    <h3 id="dashboard-editor-title">{move || text(Key::Editor, english.get())}</h3>
                    <p>{move || text(Key::EditorHelp, english.get())}</p>
                    <ActionButton id="dashboard-editor" english=english variant=ButtonVariant::Text blocked=session_pending on_press=Callback::new(move |_| on_editor.run(()))>{move || text(Key::OpenEditor, english.get())}</ActionButton>
                </section></Surface>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{CatalogPage, CatalogWork, CreateWorkRequest};

    #[test]
    fn failed_initial_read_stays_attempted_until_explicit_refresh_or_new_context() {
        let first = DashboardQuery::default().start("alice", 4);
        assert!(first.attempted("alice", 4));
        let failed = first.finish(&Err(ClientError::Transport));
        assert!(failed.attempted("alice", 4));
        assert!(!failed.attempted("alice", 5));
        assert!(!failed.attempted("bob", 4));
        assert_eq!(failed.read, CatalogRead::Failed(ClientError::Transport));
        let retried = failed.start("alice", 4);
        assert_eq!(retried.read, CatalogRead::Loading);
        let ready = retried.finish(&Ok(Reply::Works(CatalogPage {
            items: vec![],
            next_cursor: None,
        })));
        assert_eq!(ready.read, CatalogRead::Ready);
    }

    #[test]
    fn failed_or_unread_catalog_never_renders_a_successful_empty_page() {
        let owner = Owner::new();
        owner.with(|| {
            let facts = RwSignal::new(Facts::default());
            let english = Signal::stored(false);
            let noop = Callback::new(|()| {});
            for read in [CatalogRead::NotRead, CatalogRead::Loading, CatalogRead::Failed(ClientError::Transport), CatalogRead::Ready] {
                let empty = read == CatalogRead::Ready;
                let query = RwSignal::new(DashboardQuery { context: Some(("alice".into(), 1)), read });
                let html = view! { <Dashboard facts=facts query=query english=english blocked=Signal::derive(|| false) session_pending=Signal::derive(|| false)
                    on_new_work=noop on_import=noop on_editor=noop on_continue=noop on_audio=noop on_refresh=noop on_previous=noop on_next=noop on_pick=Callback::new(|_: String| {})/> }.to_html();
                assert_eq!(html.contains(text(Key::EmptyTitle, false)), empty, "{html}");
                assert!(!html.contains("Tổng số"));
                assert!(html.contains(text(Key::PageHelp, false)));
                assert!(!html.contains("id=\"dashboard-continue\""));
            }
        });
    }

    #[test]
    fn unresolved_write_keeps_recovery_navigation_available_and_blocks_new_context() {
        let owner = Owner::new();
        owner.with(|| {
            let request: CreateWorkRequest = serde_json::from_str(include_str!(
                "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
            )).unwrap();
            let work = CatalogWork {
                id: request.id,
                kind: request.kind,
                owner_id: "alice".into(),
                details: request.details,
                lifecycle: "draft".into(),
                revision: 1,
                created_at: 0,
                updated_at: 0,
            };
            let other = CatalogWork { id: "10000000-0000-4000-8000-000000000003".into(), ..work.clone() };
            let selected_id = work.id.clone();
            let other_id = other.id.clone();
            assert_ne!(selected_id, other_id);
            let facts = RwSignal::new(Facts {
                work: Some(work.clone()),
                works: vec![work, other],
                ..Facts::default()
            });
            let query = RwSignal::new(DashboardQuery { context: Some(("alice".into(), 1)), read: CatalogRead::Ready });
            let dispatches = RwSignal::new(0);
            let intent = Callback::new(move |()| dispatches.update(|count| *count += 1));
            let html = view! { <Dashboard facts=facts query=query english=Signal::stored(false)
                blocked=Signal::derive(|| true) session_pending=Signal::derive(|| false)
                on_new_work=intent on_import=intent on_editor=intent on_continue=intent on_audio=intent on_refresh=intent on_previous=intent on_next=intent
                on_pick=Callback::new(move |_: String| dispatches.update(|count| *count += 1))/> }.to_html();
            let disabled = |id: &str| {
                html.split(&format!("id=\"{id}\""))
                    .nth(1).unwrap().split('>').next().unwrap().contains(" disabled")
            };
            for id in ["dashboard-import", "dashboard-editor", "dashboard-continue", "dashboard-audio"] {
                assert!(!disabled(id), "{id}: {html}");
            }
            assert!(!disabled(&format!("dashboard-work-{selected_id}")));
            assert!(disabled(&format!("dashboard-work-{other_id}")));
            assert!(disabled("dashboard-create-work"));
            assert!(disabled("dashboard-refresh"));
            assert_eq!(dispatches.get_untracked(), 0);
        });
    }

    #[test]
    fn locale_projection_preserves_work_title_language_and_unknown_status_is_localized() {
        let owner = Owner::new();
        owner.with(|| {
            let request: CreateWorkRequest = serde_json::from_str(include_str!(
                "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
            )).unwrap();
            let mut work = CatalogWork {
                id: request.id, kind: request.kind, owner_id: "alice".into(), details: request.details,
                lifecycle: "future_status".into(), revision: 1, created_at: 0, updated_at: 0,
            };
            work.details.title = "Lời hẹn {time}".into();
            work.details.language = "vi-VN".into();
            let facts = RwSignal::new(Facts { work: Some(work.clone()), works: vec![work.clone()], ..Facts::default() });
            let english = RwSignal::new(false);
            let noop = Callback::new(|()| {});
            for en in [false, true] {
                english.set(en);
                let html = view! { <Dashboard facts=facts query=RwSignal::new(DashboardQuery::default())
                    english=Signal::derive(move ||english.get()) blocked=Signal::derive(||false) session_pending=Signal::derive(||false)
                    on_new_work=noop on_import=noop on_editor=noop on_continue=noop on_audio=noop on_refresh=noop on_previous=noop on_next=noop on_pick=Callback::new(|_:String|{})/> }.to_html();
                assert!(html.contains("<span lang=\"vi-VN\">Lời hẹn {time}</span>"), "{html}");
                assert!(html.contains(text(Key::UnknownLifecycle, en)), "{html}");
                assert!(!html.contains("future_status"), "{html}");
                assert_eq!(facts.get_untracked().work, Some(work.clone()));
            }
        });
    }
}
