//! Capability-scoped root handles. Feature-only controls live inside their screen owners.
use super::routes::Route;
use crate::{
    application::{
        catalog::{CatalogSession, Scenario},
        player_session::PlayerSession,
    },
    core::{
        catalog::{Episode, Work},
        ids::{EpisodeId, WorkId},
        listening::{
            history::HistoryState,
            library::{self, Bookmark, LibraryEvent, LibraryState},
            progress::ProgressState,
            queue::QueueState,
        },
        playback::{Event, Intent, ResumeSnapshot, Status, ValidatedResumeSnapshot},
    },
    infrastructure::{fake_media::FakeMedia, mock_catalog::MockCatalog},
    messages::{self, Key, Locale},
    platform,
};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct TheatreContext {
    pub catalog: CatalogSession,
    pub player: PlayerSession,
    pub library: ReadSignal<LibraryState>,
    library_writer: RwSignal<LibraryState>,
    pub progress: ReadSignal<ProgressState>,
    progress_writer: RwSignal<ProgressState>,
    pub history: ReadSignal<HistoryState>,
    history_writer: RwSignal<HistoryState>,
    pub queue: ReadSignal<QueueState>,
    queue_writer: RwSignal<QueueState>,
    pub route: RwSignal<Route>,
    pub locale: RwSignal<Locale>,
    pub theme: RwSignal<cantos_ui::Theme>,
    pub main: NodeRef<leptos::html::Main>,
}
impl TheatreContext {
    /// Called exactly once at the app root; no feature creates a player or engine.
    pub fn new(main: NodeRef<leptos::html::Main>) -> Self {
        let library = RwSignal::new(LibraryState::demo());
        let progress = RwSignal::new(ProgressState::default());
        let history = RwSignal::new(HistoryState::default());
        let queue = RwSignal::new(QueueState::default());
        let ctx = Self {
            catalog: CatalogSession::new(MockCatalog),
            player: PlayerSession::new(FakeMedia),
            library: library.read_only(),
            library_writer: library,
            progress: progress.read_only(),
            progress_writer: progress,
            history: history.read_only(),
            history_writer: history,
            queue: queue.read_only(),
            queue_writer: queue,
            route: RwSignal::new(Route::Discover),
            locale: RwSignal::new(Locale::Vietnamese),
            theme: RwSignal::new(cantos_ui::Theme::System),
            main,
        };
        ctx.reset_demo();
        ctx
    }
    pub fn text(self, key: Key) -> &'static str {
        messages::text(key, self.locale.get())
    }
    pub fn format(self, key: Key, values: &[(&str, String)]) -> String {
        messages::format(key, self.locale.get(), values)
    }
    pub fn work(self, id: WorkId) -> Work {
        self.catalog.work(id)
    }
    pub fn episode(self, id: EpisodeId) -> Option<Episode> {
        self.catalog.episode(id)
    }
    pub fn toggle_saved(self, id: WorkId) {
        self.set_saved(id, !self.library.get_untracked().is_saved(id));
    }
    pub fn set_saved(self, id: WorkId, saved: bool) {
        self.library_writer.set(library::reduce(
            self.library.get_untracked(),
            LibraryEvent::SetSaved { work: id, saved },
        ));
    }
    pub fn remove_bookmark(self, mark: Bookmark) {
        self.library_writer.set(library::reduce(
            self.library.get_untracked(),
            LibraryEvent::RemoveBookmark(mark),
        ));
    }
    pub fn add_bookmark(self) {
        let Some(snapshot) = self.player.snapshot() else {
            return;
        };
        let Some(episode) = snapshot.source().episode() else {
            return;
        };
        let mark = Bookmark {
            episode,
            position_ms: snapshot.position_ms(),
        };
        self.library_writer.set(library::reduce(
            self.library.get_untracked(),
            LibraryEvent::AddBookmark(mark),
        ));
    }
    fn remember_position(self) {
        if let Some(snapshot) = self.player.snapshot() {
            self.progress_writer
                .set(self.progress.get_untracked().remember(snapshot));
        }
    }
    /// Observe committed transitions, including synchronous callbacks from the fake engine.
    /// No reactive effect copies player state into listening records.
    pub fn playback(self, event: Event) {
        let record = matches!(
            &event,
            Event::PauseRequested
                | Event::SeekRequested { .. }
                | Event::SkipRequested { .. }
                | Event::EnginePosition { .. }
                | Event::EngineEnded { .. }
        );
        let before = self.player.state.get_untracked();
        self.player.dispatch(event);
        let after = self.player.state.get_untracked();
        self.history_writer.set(
            self.history
                .get_untracked()
                .observe_playback(&before, &after),
        );
        if before != after && (record || after.status() == Status::Playing) {
            self.remember_position();
        }
    }
    pub fn toggle_transport(self) {
        let playing = self
            .player
            .state
            .get_untracked()
            .session()
            .is_some_and(|session| session.intent() == Intent::Play);
        self.playback(if playing {
            Event::PauseRequested
        } else {
            Event::PlayRequested
        });
    }
    pub fn open_episode(self, id: EpisodeId) {
        let Some(source) = self.catalog.source(id) else {
            return;
        };
        self.remember_position();
        self.player.dispatch(Event::SourceSelected {
            source,
            position_ms: 0,
        });
        self.remember_position();
        self.queue_writer.set(self.queue.get_untracked().remove(id));
        platform::navigate(self.route, Route::Player);
    }
    pub fn resume_bookmark(self, mark: Bookmark) {
        if !self.library.get_untracked().bookmarks.contains(&mark) {
            return;
        }
        let Some(source) = self.catalog.source(mark.episode) else {
            return;
        };
        if mark.position_ms > source.duration_ms() {
            return;
        }
        self.remember_position();
        self.player.dispatch(Event::SourceSelected {
            source,
            position_ms: mark.position_ms,
        });
        self.playback(Event::SeekRequested {
            position_ms: mark.position_ms,
        });
        self.queue_writer
            .set(self.queue.get_untracked().remove(mark.episode));
        platform::navigate(self.route, Route::Player);
    }
    pub fn continue_progress(self) {
        let Some(progress) = self.progress.get_untracked().latest() else {
            return;
        };
        if let Some(episode) = progress.snapshot.source().episode() {
            self.resume_progress(episode);
        }
    }
    pub fn resume_progress(self, id: EpisodeId) {
        let Some(progress) = self.progress.get_untracked().find(id) else {
            return;
        };
        if self.catalog.source(id) != Some(progress.snapshot.source()) {
            return;
        }
        let previous = self.player.snapshot();
        let generation = self.player.state.get_untracked().generation();
        self.player.restore(progress.snapshot);
        if self.player.state.get_untracked().generation() == generation {
            return;
        }
        if let Some(snapshot) = previous {
            self.progress_writer
                .set(self.progress.get_untracked().remember(snapshot));
        }
        self.remember_position();
        platform::navigate(self.route, Route::Player);
    }
    pub fn resume_episode(self, id: EpisodeId) {
        if self.history.get_untracked().contains(id) {
            self.resume_progress(id);
        }
    }
    pub fn enqueue(self, id: EpisodeId) {
        let Some(source) = self.catalog.source(id) else {
            return;
        };
        let current = self
            .player
            .snapshot()
            .and_then(|snapshot| snapshot.source().episode());
        self.queue_writer
            .set(self.queue.get_untracked().enqueue(source, current));
    }
    pub fn remove_queue(self, id: EpisodeId) {
        self.queue_writer.set(self.queue.get_untracked().remove(id));
    }
    pub fn next(self) {
        let (next, id) = self.queue.get_untracked().pop();
        self.queue_writer.set(next);
        if let Some(id) = id {
            self.open_episode(id);
        }
    }
    pub fn mock_buffer(self) {
        if let Some(identity) = self.player.state.get_untracked().identity() {
            self.playback(Event::EngineBuffering { identity });
        }
    }
    pub fn mock_started(self) {
        if let Some(identity) = self.player.state.get_untracked().identity() {
            self.playback(Event::EngineStarted { identity });
        }
    }
    pub fn mock_ended(self) {
        if let Some(identity) = self.player.state.get_untracked().identity() {
            self.playback(Event::EngineEnded { identity });
        }
    }
    pub fn restore_position(self) {
        if let Some(snapshot) = self.player.snapshot() {
            self.player.restore(snapshot);
        }
    }
    pub fn set_speed(self, percent: u16) {
        if let Some(speed_permille) = percent.checked_mul(10) {
            self.playback(Event::SpeedRequested { speed_permille });
        }
    }
    pub fn clear_library(self) {
        self.library_writer.set(LibraryState::default());
        self.progress_writer.set(ProgressState::default());
        self.clear_history();
        // Saved/bookmark/progress clear never unloads the root player or queue.
    }
    pub fn clear_history(self) {
        self.history_writer.set(HistoryState::default());
    }
    pub fn reset_demo(self) {
        self.catalog.load(Scenario::Ready);
        self.library_writer.set(LibraryState::demo());
        self.clear_history();
        self.queue_writer.set(QueueState {
            items: vec![EpisodeId {
                work: WorkId::Mountain,
                index: 3,
            }],
        });
        if let Some(source) = self.catalog.source(EpisodeId {
            work: WorkId::Mountain,
            index: 2,
        }) {
            let Ok(snapshot) = ValidatedResumeSnapshot::try_from(ResumeSnapshot {
                source,
                position_ms: 768000,
                speed_permille: 1000,
            }) else {
                return;
            };
            self.progress_writer
                .set(ProgressState::default().remember(snapshot));
            self.player.restore(snapshot);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_starts_only_after_simulated_play_and_keeps_ended_episodes() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let river_first = EpisodeId {
                work: WorkId::River,
                index: 0,
            };
            let river_next = EpisodeId {
                work: WorkId::River,
                index: 1,
            };
            assert!(ctx.history.get_untracked().records().is_empty());
            ctx.route.set(Route::Work(WorkId::River));
            ctx.open_episode(river_first);
            ctx.playback(Event::SeekRequested { position_ms: 5000 });
            assert!(ctx.history.get_untracked().records().is_empty());
            ctx.playback(Event::PlayRequested);
            assert_eq!(ctx.player.status.get_untracked(), Status::Playing);
            assert_eq!(
                ctx.history.get_untracked().records()[0].episode,
                river_first
            );
            ctx.mock_ended();
            assert_eq!(ctx.player.status.get_untracked(), Status::Ended);
            assert_eq!(
                ctx.history.get_untracked().records()[0].episode,
                river_first
            );
            assert_eq!(
                ctx.progress
                    .get_untracked()
                    .find(river_first)
                    .unwrap()
                    .snapshot
                    .position_ms(),
                ctx.catalog.source(river_first).unwrap().duration_ms()
            );
            ctx.playback(Event::PlayRequested);
            assert_eq!(
                ctx.progress
                    .get_untracked()
                    .find(river_first)
                    .unwrap()
                    .snapshot
                    .position_ms(),
                0
            );
            ctx.open_episode(river_next);
            assert_eq!(
                ctx.history.get_untracked().records()[0].episode,
                river_first
            );
            ctx.playback(Event::PlayRequested);
            assert_eq!(ctx.history.get_untracked().records().len(), 1);
            assert_eq!(ctx.history.get_untracked().records()[0].episode, river_next);
        });
    }

    #[test]
    fn clear_history_preserves_every_other_owner_and_rejects_captured_history_actions() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let episode = ctx.player.snapshot().unwrap().source().episode().unwrap();
            ctx.playback(Event::PlayRequested);
            ctx.playback(Event::SeekRequested {
                position_ms: 15_000,
            });
            ctx.add_bookmark();
            ctx.set_saved(WorkId::River, true);
            let player = ctx.player.state.get_untracked();
            let progress = ctx.progress.get_untracked();
            let library = ctx.library.get_untracked();
            let queue = ctx.queue.get_untracked();
            ctx.route.set(Route::Library);
            ctx.clear_history();
            ctx.resume_episode(episode);
            ctx.playback(Event::EngineStarted {
                identity: player.identity().unwrap(),
            });
            assert!(ctx.history.get_untracked().records().is_empty());
            assert_eq!(ctx.route.get_untracked(), Route::Library);
            assert_eq!(ctx.player.state.get_untracked(), player);
            assert_eq!(ctx.progress.get_untracked(), progress);
            assert_eq!(ctx.library.get_untracked(), library);
            assert_eq!(ctx.queue.get_untracked(), queue);
        });
    }

    #[test]
    fn history_resume_reads_current_progress_and_stays_paused_without_reordering_history() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let episode = EpisodeId {
                work: WorkId::River,
                index: 0,
            };
            ctx.open_episode(episode);
            ctx.playback(Event::PlayRequested);
            ctx.playback(Event::SeekRequested {
                position_ms: 15_000,
            });
            let stale_identity = ctx.player.state.get_untracked().identity().unwrap();
            ctx.playback(Event::PauseRequested);
            ctx.playback(Event::SeekRequested { position_ms: 5000 });
            let history = ctx.history.get_untracked();
            ctx.route.set(Route::Library);
            ctx.resume_episode(episode);
            assert_eq!(ctx.route.get_untracked(), Route::Player);
            assert_eq!(ctx.player.snapshot().unwrap().position_ms(), 5000);
            assert_eq!(
                ctx.player.status.get_untracked(),
                Status::Paused(crate::core::playback::PauseCause::ColdRestore)
            );
            ctx.playback(Event::EngineStarted {
                identity: stale_identity,
            });
            assert_eq!(ctx.history.get_untracked(), history);
            assert_eq!(ctx.player.snapshot().unwrap().position_ms(), 5000);
        });
    }

    #[test]
    fn unknown_or_unavailable_progress_does_not_replace_player_or_navigate() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let unavailable = EpisodeId {
                work: WorkId::Mountain,
                index: 5,
            };
            let snapshot = ResumeSnapshot {
                source: crate::core::playback::Source::new(unavailable, 60_000).unwrap(),
                position_ms: 10_000,
                speed_permille: 1000,
            };
            ctx.progress_writer.set(
                ProgressState::default()
                    .remember(ValidatedResumeSnapshot::try_from(snapshot).unwrap()),
            );
            ctx.route.set(Route::Library);
            let player = ctx.player.state.get_untracked();
            ctx.resume_progress(unavailable);
            ctx.resume_progress(EpisodeId {
                work: WorkId::Dream,
                index: 99,
            });
            assert_eq!(ctx.route.get_untracked(), Route::Library);
            assert_eq!(ctx.player.state.get_untracked(), player);
        });
    }

    #[test]
    fn rejected_resume_admission_keeps_positions_and_route_unchanged() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let source = ctx.player.snapshot().unwrap().source();
            for (snapshot, expected) in [
                (
                    ResumeSnapshot {
                        source,
                        position_ms: source.duration_ms() + 1,
                        speed_permille: 1000,
                    },
                    crate::core::playback::Rejection::OutOfBounds,
                ),
                (
                    ResumeSnapshot {
                        source,
                        position_ms: 0,
                        speed_permille: 1001,
                    },
                    crate::core::playback::Rejection::UnsupportedSpeed,
                ),
            ] {
                ctx.route.set(Route::Library);
                let player = ctx.player.state.get_untracked();
                let progress = ctx.progress.get_untracked();
                // Invalid raw facts cannot enter the retained progress owner.
                assert_eq!(ValidatedResumeSnapshot::try_from(snapshot), Err(expected));
                assert_eq!(ctx.player.state.get_untracked(), player);
                assert_eq!(ctx.progress.get_untracked(), progress);
                assert_eq!(ctx.route.get_untracked(), Route::Library);
            }
        });
    }
    #[test]
    fn admitted_progress_still_requires_the_current_catalog_source() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let current = ctx.player.snapshot().unwrap().source();
            let rebound = crate::core::playback::Source::new(
                current.episode().unwrap(),
                current.duration_ms() + 1,
            )
            .unwrap();
            let snapshot = ValidatedResumeSnapshot::try_from(ResumeSnapshot {
                source: rebound,
                position_ms: 1000,
                speed_permille: 1000,
            })
            .unwrap();
            let progress = ProgressState::default().remember(snapshot);
            ctx.progress_writer.set(progress.clone());
            ctx.route.set(Route::Library);
            let player = ctx.player.state.get_untracked();
            ctx.resume_progress(current.episode().unwrap());
            assert_eq!(ctx.player.state.get_untracked(), player);
            assert_eq!(ctx.progress.get_untracked(), progress);
            assert_eq!(ctx.route.get_untracked(), Route::Library);
        });
    }
    #[test]
    fn routes_share_one_root_player_and_library_clear_does_not_unload_it() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let identity = ctx.player.state.get_untracked().identity();
            for route in [
                Route::Discover,
                Route::Explore,
                Route::Library,
                Route::Player,
                Route::Queue,
                Route::Settings,
            ] {
                ctx.route.set(route);
                assert_eq!(ctx.player.state.get_untracked().identity(), identity);
            }
            ctx.add_bookmark();
            ctx.add_bookmark();
            assert_eq!(ctx.library.get_untracked().bookmarks.len(), 1);
            let before = ctx.player.state.get_untracked();
            ctx.clear_library();
            assert_eq!(ctx.player.state.get_untracked(), before);
            assert!(ctx.progress.get_untracked().records().is_empty());
            assert!(!ctx.queue.get_untracked().items.is_empty());
            ctx.reset_demo();
            assert!(ctx.player.state.get_untracked().generation() > before.generation());
        });
    }
    #[test]
    fn unavailable_sources_and_missing_bookmarks_never_change_session() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let before = ctx.player.state.get_untracked();
            for id in [
                EpisodeId {
                    work: WorkId::Mountain,
                    index: 5,
                },
                EpisodeId {
                    work: WorkId::River,
                    index: 99,
                },
            ] {
                ctx.open_episode(id);
                ctx.enqueue(id);
            }
            ctx.resume_bookmark(Bookmark {
                episode: EpisodeId {
                    work: WorkId::River,
                    index: 0,
                },
                position_ms: 0,
            });
            assert_eq!(ctx.player.state.get_untracked(), before);
        });
    }
    #[test]
    fn repeated_continue_keeps_current_progress_instead_of_swapping_sessions() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let other = EpisodeId {
                work: WorkId::River,
                index: 0,
            };
            ctx.open_episode(other);
            ctx.playback(Event::SeekRequested { position_ms: 15000 });
            for _ in 0..3 {
                ctx.continue_progress();
                let current = ctx.player.snapshot().unwrap();
                assert_eq!(current.source().episode(), Some(other));
                assert_eq!(current.position_ms(), 15000);
                assert_eq!(
                    ctx.progress.get_untracked().latest().unwrap().snapshot,
                    current
                );
            }
        });
    }

    #[test]
    fn reset_rejects_captured_bookmarks_and_old_callbacks_without_reviving_progress() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.add_bookmark();
            let old_mark = ctx.library.get_untracked().bookmarks[0];
            let other = EpisodeId {
                work: WorkId::River,
                index: 0,
            };
            ctx.open_episode(other);
            ctx.playback(Event::SeekRequested { position_ms: 15000 });
            let old_identity = ctx.player.state.get_untracked().identity().unwrap();
            let old_progress = ctx.progress.get_untracked().latest().unwrap();
            ctx.route.set(Route::Settings);
            ctx.reset_demo();
            let reset_player = ctx.player.state.get_untracked();
            let reset_progress = ctx.progress.get_untracked();
            let reset_queue = ctx.queue.get_untracked();
            ctx.resume_bookmark(old_mark);
            assert_eq!(ctx.route.get_untracked(), Route::Settings);
            assert_eq!(ctx.player.state.get_untracked(), reset_player);
            assert_eq!(ctx.progress.get_untracked(), reset_progress);
            assert_eq!(ctx.queue.get_untracked(), reset_queue);
            for event in [
                Event::EngineStarted {
                    identity: old_identity,
                },
                Event::EnginePosition {
                    identity: old_identity,
                    position_ms: 30000,
                },
                Event::EngineEnded {
                    identity: old_identity,
                },
            ] {
                ctx.playback(event);
                assert_eq!(ctx.player.state.get_untracked(), reset_player);
                assert_eq!(ctx.progress.get_untracked(), reset_progress);
            }
            // The click captures only ctx: Continue reads the new seed at dispatch, not old_progress.
            ctx.continue_progress();
            let resumed = ctx.player.snapshot().unwrap();
            assert_ne!(
                resumed.source().episode(),
                old_progress.snapshot.source().episode()
            );
            assert_eq!(
                resumed.source().episode(),
                Some(EpisodeId {
                    work: WorkId::Mountain,
                    index: 2
                })
            );
            assert_eq!(resumed.position_ms(), 768000);
            assert_eq!(ctx.progress.get_untracked().records().len(), 1);
            assert!(ctx.library.get_untracked().bookmarks.is_empty());
        });
    }
    #[test]
    fn rejected_empty_resume_actions_do_not_navigate() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.route.set(Route::Settings);
            ctx.queue_writer.set(QueueState::default());
            ctx.clear_library();
            let player = ctx.player.state.get_untracked();
            ctx.next();
            ctx.continue_progress();
            assert_eq!(ctx.route.get_untracked(), Route::Settings);
            assert_eq!(ctx.player.state.get_untracked(), player);
            assert!(ctx.progress.get_untracked().records().is_empty());
        });
    }
}
