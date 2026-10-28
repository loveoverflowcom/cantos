//! App-owned overview of the existing workspace projection and request authority.
use super::{
    copy::{self, work_kind},
    dashboard_copy::{text, Key},
    ClientError, Facts, Reply,
};
use crate::contract::AudioSubmissionState;
use crate::ui::{ActionButton, BlockReason};
use cantos_ui::{Announcement, ButtonVariant, LoadingState, Surface};
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

fn lifecycle(value: &str, locale: crate::localization::Locale) -> &str {
    match value {
        "draft" => text(Key::Draft, locale),
        "archived" => text(Key::Archived, locale),
        _ => text(Key::UnknownLifecycle, locale),
    }
}

/// Which audio lines the facts actually read warrant; an unread state never fills a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AudioNotes {
    state: bool,
    not_read: bool,
    candidate: bool,
    help: bool,
}

fn audio_notes(unit_selected: bool, submission_read: bool, candidate_read: bool) -> AudioNotes {
    AudioNotes {
        state: submission_read,
        not_read: unit_selected && !submission_read,
        candidate: candidate_read,
        help: submission_read || candidate_read,
    }
}

fn audio_state(state: AudioSubmissionState, locale: crate::localization::Locale) -> &'static str {
    text(
        match state {
            AudioSubmissionState::AwaitingBytes => Key::AudioAwaiting,
            AudioSubmissionState::Admitted => Key::AudioAdmitted,
            AudioSubmissionState::Revoked => Key::AudioRevoked,
            AudioSubmissionState::Expired => Key::AudioExpired,
            AudioSubmissionState::Stale => Key::AudioStale,
        },
        locale,
    )
}

#[component]
pub fn Dashboard(
    #[prop(into)] facts: Signal<Facts>,
    #[prop(into)] query: Signal<DashboardQuery>,
    locale: Signal<crate::localization::Locale>,
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
    let has_selection = Signal::derive(move || facts.with(|f| f.work.is_some()));
    // Row membership and order only; title, kind, lifecycle and revision are read per ID.
    let work_ids = Memo::new(move |_| {
        facts.with(|f| super::projection::row_ids(f.works.iter().map(|work| work.id.as_str())))
    });
    let has_pages = Signal::derive(move || {
        facts.with(|f| f.works_next.is_some() || f.pages.previous_works().is_some())
    });
    let notes = Signal::derive(move || {
        facts.with(|f| {
            audio_notes(
                f.unit.is_some(),
                f.submission.is_some(),
                f.candidate.is_some(),
            )
        })
    });
    view! {
        <section id="studio-dashboard" class="studio-dashboard" aria-labelledby="dashboard-title">
            <header class="dashboard-heading">
                <div><h2 id="dashboard-title" tabindex="-1">{move || text(Key::Title, locale.get())}</h2></div>
                <div class="dashboard-actions">
                    <ActionButton id="dashboard-create-work" locale=locale variant=ButtonVariant::Tonal blocked=blocked on_press=Callback::new(move |_| on_new_work.run(()))>{move || text(Key::NewWork, locale.get())}</ActionButton>
                    <ActionButton id="dashboard-import" locale=locale variant=ButtonVariant::Primary blocked=session_pending on_press=Callback::new(move |_| on_import.run(()))>{move || text(Key::Import, locale.get())}</ActionButton>
                </div>
            </header>
            <div class="dashboard-grid">
                <Surface class="dashboard-works">
                    <section aria-labelledby="dashboard-works-title">
                        <div class="dashboard-section-heading"><h3 id="dashboard-works-title" tabindex="-1">{move || text(Key::Works, locale.get())}</h3>
                            <ActionButton id="dashboard-refresh" locale=locale variant=ButtonVariant::Text blocked=blocked on_press=Callback::new(move |_| on_refresh.run(()))>{move || text(Key::Refresh, locale.get())}</ActionButton></div>
                        <Show when=move || query.with(|q| q.read == CatalogRead::Ready) && !has_selection.get() && facts.with(|f| !f.works.is_empty())>
                            <p class="help dashboard-hint">{move || text(Key::NoSelection, locale.get())}</p>
                        </Show>
                        <div id="dashboard-status" class="dashboard-status" tabindex="-1">
                            <Show when=move || query.with(|q| q.read == CatalogRead::NotRead)><p role="status" aria-live="polite">{move || text(Key::NotRead, locale.get())}</p></Show>
                            <Show when=move || query.with(|q| q.read == CatalogRead::Loading)><LoadingState announcement=Announcement::Polite>{move || text(Key::Loading, locale.get())}</LoadingState></Show>
                            <Show when=move || query.with(|q| matches!(q.read, CatalogRead::Failed(_)))>
                                <div role="status" aria-live="polite"><h4>{move ||text(Key::ReadFailed,locale.get())}</h4><p>{move ||text(Key::RetryHelp,locale.get())}</p></div>
                            </Show>
                            <Show when=move || query.with(|q| q.read == CatalogRead::Ready) && facts.with(|f| f.works.is_empty())>
                                <div><h4>{move ||text(Key::EmptyTitle,locale.get())}</h4><p>{move ||text(Key::EmptyHelp,locale.get())}</p></div>
                            </Show>
                        </div>
                        <ul class="dashboard-work-list">
                            <For each=move || work_ids.get() key=|id| id.clone() children=move |id| {
                                let row = Memo::new({ let id = id.clone(); move |_| facts.with(|f| f.works.iter().find(|work| work.id == id).cloned()) });
                                let selected_id = id.clone();
                                let current_id = id.clone();
                                let pick_id = id.clone();
                                view! { <li class="dashboard-work-row" aria-current=move || facts.with(|f| f.work.as_ref().is_some_and(|w| w.id == current_id)).then_some("true")>
                                    <ActionButton id=format!("dashboard-work-{id}") locale=locale variant=ButtonVariant::Text
                                        blocked=Signal::derive(move || session_pending.get() || (blocked.get() && facts.with(|f| f.work.as_ref().is_none_or(|w| w.id != selected_id))))
                                        on_press=Callback::new(move |_| on_pick.run(pick_id.clone()))><span lang=move || row.get().map(|work| work.details.language)>{move || row.get().map(|work| work.details.title)}</span></ActionButton>
                                    <p class="dashboard-work-meta">{move || row.get().map(|work| copy::row_status(work_kind(&work.kind, locale.get()), lifecycle(&work.lifecycle, locale.get()), work.revision))}</p>
                                </li> }
                            }/>
                        </ul>
                        <Show when=move || has_pages.get()>
                            <div class="toolbar dashboard-pages"><Show when=move || facts.with(|f| f.pages.previous_works().is_some())><ActionButton id="dashboard-previous" locale=locale reason=BlockReason::PageNavigation variant=ButtonVariant::Tonal blocked=Signal::derive(move || blocked.get() || facts.with(|f| f.pages.previous_works().is_none())) on_press=Callback::new(move |_| on_previous.run(()))>{move || text(Key::Previous, locale.get())}</ActionButton></Show>
                            <Show when=move ||facts.with(|f|f.works_next.is_some())><ActionButton id="dashboard-next" locale=locale reason=BlockReason::PageNavigation variant=ButtonVariant::Tonal blocked=Signal::derive(move || blocked.get() || facts.with(|f| f.works_next.is_none())) on_press=Callback::new(move |_| on_next.run(()))>{move || text(Key::Next, locale.get())}</ActionButton></Show></div>
                        </Show>
                    </section>
                </Surface>
                <div class="dashboard-secondary">
                    <Show when=move || has_selection.get()>
                        <Surface class="dashboard-resume">
                            <section aria-labelledby="dashboard-resume-title">
                                <p class="dashboard-eyebrow">{move || text(Key::Resume, locale.get())}</p>
                                <h3 id="dashboard-resume-title" lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move || facts.with(|f| f.work.as_ref().map(|w| w.details.title.clone()).unwrap_or_default())}</h3>
                                <Show when=move || facts.with(|f| f.unit.is_some())>
                                    <p>{move || text(Key::SelectedUnit, locale.get())}" · "<span lang=move ||facts.with(|f|f.work.as_ref().map(|w|w.details.language.clone()))>{move || facts.with(|f| f.unit.as_ref().map(|u| u.details.title.clone()).unwrap_or_default())}</span></p>
                                </Show>
                                <ActionButton id="dashboard-continue" locale=locale blocked=Signal::derive(move || session_pending.get() || facts.with(|f| f.work.is_none())) reason=BlockReason::ContextReady on_press=Callback::new(move |_| on_continue.run(()))>{move || text(Key::Continue, locale.get())}</ActionButton>
                            </section>
                        </Surface>
                    </Show>
                    <Surface class="dashboard-tools">
                        <section aria-labelledby="dashboard-audio-title">
                            <h3 id="dashboard-audio-title">{move || text(Key::Audio, locale.get())}</h3>
                            <Show when=move || notes.get().state><p>{move || facts.with(|f| f.submission.as_ref().map(|s| audio_state(s.state, locale.get())).unwrap_or_default())}</p></Show>
                            <Show when=move || notes.get().not_read><p class="help">{move || text(Key::AudioNotRead, locale.get())}</p></Show>
                            <Show when=move || notes.get().candidate><p>{move || text(Key::Candidate, locale.get())}</p></Show>
                            <Show when=move || notes.get().help><p class="help">{move || text(Key::AudioHelp, locale.get())}</p></Show>
                            <ActionButton id="dashboard-audio" locale=locale variant=ButtonVariant::Tonal blocked=session_pending on_press=Callback::new(move |_| on_audio.run(()))>{move || text(Key::OpenAudio, locale.get())}</ActionButton>
                        </section>
                        <section aria-labelledby="dashboard-editor-title">
                            <h3 id="dashboard-editor-title">{move || text(Key::Editor, locale.get())}</h3>
                            <ActionButton id="dashboard-editor" locale=locale variant=ButtonVariant::Tonal blocked=session_pending on_press=Callback::new(move |_| on_editor.run(()))>{move || text(Key::OpenEditor, locale.get())}</ActionButton>
                        </section>
                    </Surface>
                </div>
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
            let locale = Signal::stored(crate::localization::Locale::Vietnamese);
            let noop = Callback::new(|()| {});
            for read in [CatalogRead::NotRead, CatalogRead::Loading, CatalogRead::Failed(ClientError::Transport), CatalogRead::Ready] {
                let empty = read == CatalogRead::Ready;
                let query = RwSignal::new(DashboardQuery { context: Some(("alice".into(), 1)), read });
                let html = view! { <Dashboard facts=facts query=query locale=locale blocked=Signal::derive(|| false) session_pending=Signal::derive(|| false)
                    on_new_work=noop on_import=noop on_editor=noop on_continue=noop on_audio=noop on_refresh=noop on_previous=noop on_next=noop on_pick=Callback::new(|_: String| {})/> }.to_html();
                assert_eq!(html.contains(text(Key::EmptyTitle, crate::localization::Locale::Vietnamese)), empty, "{html}");
                assert!(!html.contains("Tổng số"));
                assert!(html.contains(text(Key::Refresh, crate::localization::Locale::Vietnamese)));
                assert!(!html.contains("id=\"dashboard-previous\""), "a single page has no pagination: {html}");
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
            let html = view! { <Dashboard facts=facts query=query locale=Signal::stored(crate::localization::Locale::Vietnamese)
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
            let locale = RwSignal::new(crate::localization::Locale::Vietnamese);
            let noop = Callback::new(|()| {});
            for choice in crate::localization::Locale::supported() {
                locale.set(choice);
                let html = view! { <Dashboard facts=facts query=RwSignal::new(DashboardQuery::default())
                    locale=Signal::derive(move ||locale.get()) blocked=Signal::derive(||false) session_pending=Signal::derive(||false)
                    on_new_work=noop on_import=noop on_editor=noop on_continue=noop on_audio=noop on_refresh=noop on_previous=noop on_next=noop on_pick=Callback::new(|_:String|{})/> }.to_html();
                assert!(html.contains("<span lang=\"vi-VN\">Lời hẹn {time}</span>"), "{html}");
                assert!(html.contains(text(Key::UnknownLifecycle, choice)), "{html}");
                assert!(!html.contains("future_status"), "{html}");
                assert_eq!(facts.get_untracked().work, Some(work.clone()));
            }
        });
    }

    fn work(id: &str, title: &str) -> CatalogWork {
        let request: CreateWorkRequest = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
        )).unwrap();
        let mut work = CatalogWork {
            id: id.into(),
            kind: request.kind,
            owner_id: "alice".into(),
            details: request.details,
            lifecycle: "draft".into(),
            revision: 3,
            created_at: 0,
            updated_at: 0,
        };
        work.details.title = title.into();
        work
    }

    fn render(facts: Facts, read: CatalogRead) -> String {
        let noop = Callback::new(|()| {});
        view! { <Dashboard facts=RwSignal::new(facts) query=RwSignal::new(DashboardQuery { context: Some(("alice".into(), 1)), read })
            locale=Signal::stored(crate::localization::Locale::Vietnamese) blocked=Signal::derive(|| false) session_pending=Signal::derive(|| false)
            on_new_work=noop on_import=noop on_editor=noop on_continue=noop on_audio=noop on_refresh=noop on_previous=noop on_next=noop
            on_pick=Callback::new(|_: String| {})/> }.to_html()
    }

    #[test]
    fn work_list_follows_the_heading_and_precedes_resume_and_tools() {
        let owner = Owner::new();
        owner.with(|| {
            let selected = work("10000000-0000-4000-8000-000000000001", "Lời hẹn");
            let facts = Facts {
                work: Some(selected.clone()),
                works: vec![selected],
                ..Facts::default()
            };
            let html = render(facts, CatalogRead::Ready);
            let at = |needle: &str| {
                html.find(needle)
                    .unwrap_or_else(|| panic!("{needle}: {html}"))
            };
            assert!(at("id=\"dashboard-import\"") < at("dashboard-work-list"));
            assert!(at("dashboard-work-list") < at("id=\"dashboard-continue\""));
            assert!(at("id=\"dashboard-continue\"") < at("id=\"dashboard-audio\""));
            assert!(at("id=\"dashboard-audio\"") < at("id=\"dashboard-editor\""));
            assert_eq!(html.matches("aria-current=\"true\"").count(), 1, "{html}");
        });
    }

    #[test]
    fn rows_are_keyed_by_entity_id_and_read_title_revision_and_lifecycle_per_id() {
        let owner = Owner::new();
        owner.with(|| {
            let first = work("10000000-0000-4000-8000-000000000001", "Lời hẹn");
            let second = work("10000000-0000-4000-8000-000000000002", "Đêm trắng");
            let facts = RwSignal::new(Facts {
                works: vec![first.clone(), second.clone(), first.clone()],
                ..Facts::default()
            });
            let noop = Callback::new(|()| {});
            let html = |facts: RwSignal<Facts>| {
                view! { <Dashboard facts=facts query=RwSignal::new(DashboardQuery { context: Some(("alice".into(), 1)), read: CatalogRead::Ready })
                    locale=Signal::stored(crate::localization::Locale::Vietnamese) blocked=Signal::derive(|| false) session_pending=Signal::derive(|| false)
                    on_new_work=noop on_import=noop on_editor=noop on_continue=noop on_audio=noop on_refresh=noop on_previous=noop on_next=noop
                    on_pick=Callback::new(|_: String| {})/> }.to_html()
            };
            // A repeated identity never mounts a second row.
            let initial = html(facts);
            assert_eq!(initial.matches("dashboard-work-row").count(), 2, "{initial}");
            // The same ID with a new title, revision and lifecycle is the same row, updated.
            facts.update(|f| {
                let patched = &mut f.works[0];
                patched.details.title = "Lời hẹn (sửa)".into();
                patched.revision = 4;
                patched.lifecycle = "archived".into();
            });
            let patched = html(facts);
            assert_eq!(patched.matches("dashboard-work-row").count(), 2, "{patched}");
            assert!(patched.contains("Lời hẹn (sửa)") && patched.contains("r4"), "{patched}");
            assert!(patched.contains(text(Key::Archived, crate::localization::Locale::Vietnamese)), "{patched}");
            // A row removed between notifications is simply gone; its neighbour is untouched.
            facts.update(|f| f.works.retain(|work| work.id == second.id));
            let removed = html(facts);
            assert_eq!(removed.matches("dashboard-work-row").count(), 1, "{removed}");
            assert!(!removed.contains("Lời hẹn") && removed.contains("Đêm trắng"), "{removed}");
        });
    }

    #[test]
    fn without_a_selection_there_is_a_short_hint_and_no_resume_block() {
        let owner = Owner::new();
        owner.with(|| {
            let facts = Facts {
                works: vec![work("10000000-0000-4000-8000-000000000001", "Lời hẹn")],
                ..Facts::default()
            };
            let html = render(facts, CatalogRead::Ready);
            assert!(
                html.contains(text(
                    Key::NoSelection,
                    crate::localization::Locale::Vietnamese
                )),
                "{html}"
            );
            assert!(
                !html.contains(text(Key::Resume, crate::localization::Locale::Vietnamese)),
                "{html}"
            );
            assert!(!html.contains("id=\"dashboard-continue\""), "{html}");
            assert!(!html.contains("aria-current"), "{html}");
            for id in [
                "dashboard-import",
                "dashboard-create-work",
                "dashboard-audio",
                "dashboard-editor",
            ] {
                assert!(
                    html.contains(&format!("id=\"{id}\"")),
                    "{id} stays reachable: {html}"
                );
            }
        });
    }

    #[test]
    fn pagination_offers_only_the_directions_that_exist() {
        let owner = Owner::new();
        owner.with(|| {
            let one = work("10000000-0000-4000-8000-000000000001", "Lời hẹn");
            let single = render(
                Facts {
                    works: vec![one.clone()],
                    ..Facts::default()
                },
                CatalogRead::Ready,
            );
            assert!(
                !single.contains("dashboard-previous") && !single.contains("dashboard-next"),
                "{single}"
            );
            let paged = render(
                Facts {
                    works: vec![one],
                    works_next: Some("cursor".into()),
                    ..Facts::default()
                },
                CatalogRead::Ready,
            );
            assert!(
                paged.contains("id=\"dashboard-next\"") && !paged.contains("dashboard-previous"),
                "first page has no previous control: {paged}"
            );
        });
    }

    #[test]
    fn audio_lines_follow_the_facts_actually_read() {
        let notes = |unit, submission, candidate| audio_notes(unit, submission, candidate);
        assert_eq!(
            notes(false, false, false),
            AudioNotes {
                state: false,
                not_read: false,
                candidate: false,
                help: false
            }
        );
        assert_eq!(
            notes(true, false, false),
            AudioNotes {
                state: false,
                not_read: true,
                candidate: false,
                help: false
            }
        );
        assert_eq!(
            notes(true, true, false),
            AudioNotes {
                state: true,
                not_read: false,
                candidate: false,
                help: true
            }
        );
        assert_eq!(
            notes(false, true, true),
            AudioNotes {
                state: true,
                not_read: false,
                candidate: true,
                help: true
            }
        );
        assert_eq!(
            notes(true, false, true),
            AudioNotes {
                state: false,
                not_read: true,
                candidate: true,
                help: true
            }
        );
    }
}
