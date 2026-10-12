//! Capability-scoped root handles. Feature-only controls live inside their screen owners.
use leptos::prelude::*;
use crate::{
    application::{catalog::{CatalogSession, Scenario}, player_session::PlayerSession},
    core::{catalog::{Episode, Work}, ids::{EpisodeId, WorkId}, listening::{library::{self, Bookmark, LibraryEvent, LibraryState}, progress::ProgressState, queue::QueueState}, playback::{Event, Intent, ResumeSnapshot}},
    infrastructure::{fake_media::FakeMedia, mock_catalog::MockCatalog},
    messages::{self, Key, Locale}, platform,
};
use super::routes::Route;

#[derive(Clone, Copy)]
pub struct TheatreContext {
    pub catalog: CatalogSession,
    pub player: PlayerSession,
    pub library: ReadSignal<LibraryState>,
    library_writer: RwSignal<LibraryState>,
    pub progress: ReadSignal<ProgressState>,
    progress_writer: RwSignal<ProgressState>,
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
        let queue = RwSignal::new(QueueState::default());
        let ctx = Self {
            catalog: CatalogSession::new(MockCatalog), player: PlayerSession::new(FakeMedia),
            library: library.read_only(), library_writer: library, progress: progress.read_only(), progress_writer: progress,
            queue: queue.read_only(), queue_writer: queue, route: RwSignal::new(Route::Discover),
            locale: RwSignal::new(Locale::Vietnamese), theme: RwSignal::new(cantos_ui::Theme::System), main,
        };
        ctx.reset_demo();
        ctx
    }
    pub fn text(self, key: Key) -> &'static str { messages::text(key, self.locale.get()) }
    pub fn format(self, key: Key, values: &[(&str, String)]) -> String { messages::format(key, self.locale.get(), values) }
    pub fn work(self, id: WorkId) -> Work { self.catalog.work(id) }
    pub fn episode(self, id: EpisodeId) -> Option<Episode> { self.catalog.episode(id) }
    pub fn toggle_saved(self, id: WorkId) {
        self.library_writer.set(library::reduce(self.library.get_untracked(), LibraryEvent::ToggleSaved(id)));
    }
    pub fn remove_bookmark(self, mark: Bookmark) {
        self.library_writer.set(library::reduce(self.library.get_untracked(), LibraryEvent::RemoveBookmark(mark)));
    }
    pub fn add_bookmark(self) {
        let Some(snapshot) = self.player.snapshot() else { return; };
        let mark = Bookmark { episode: snapshot.source.episode(), position_ms: snapshot.position_ms };
        self.library_writer.set(library::reduce(self.library.get_untracked(), LibraryEvent::AddBookmark(mark)));
    }
    fn remember_position(self) {
        if let Some(snapshot) = self.player.snapshot() {
            self.progress_writer.set(self.progress.get_untracked().remember(snapshot));
        }
    }
    /// Explicit user commands record local progress. No reactive effect copies player state.
    pub fn playback(self, event: Event) {
        let record = matches!(&event, Event::PauseRequested | Event::SeekRequested { .. } | Event::SkipRequested { .. });
        self.player.dispatch(event);
        if record { self.remember_position(); }
    }
    pub fn toggle_transport(self) {
        let playing = self.player.state.get_untracked().session().is_some_and(|session| session.intent() == Intent::Play);
        self.playback(if playing { Event::PauseRequested } else { Event::PlayRequested });
    }
    pub fn open_episode(self, id: EpisodeId) {
        let Some(source) = self.catalog.source(id) else { return; };
        self.remember_position();
        self.player.dispatch(Event::SourceSelected { source, position_ms: 0 });
        self.remember_position();
        self.queue_writer.set(self.queue.get_untracked().remove(id));
        platform::navigate(self.route, Route::Player);
    }
    pub fn resume_bookmark(self, mark: Bookmark) {
        if !self.library.get_untracked().bookmarks.contains(&mark) { return; }
        let Some(source) = self.catalog.source(mark.episode) else { return; };
        if mark.position_ms > source.duration_ms() { return; }
        self.remember_position();
        self.player.dispatch(Event::SourceSelected { source, position_ms: mark.position_ms });
        self.playback(Event::SeekRequested { position_ms: mark.position_ms });
        self.queue_writer.set(self.queue.get_untracked().remove(mark.episode));
        platform::navigate(self.route, Route::Player);
    }
    pub fn continue_progress(self) {
        let Some(progress) = self.progress.get_untracked().latest() else { return; };
        self.remember_position();
        self.player.restore(progress.snapshot);
        self.remember_position();
        platform::navigate(self.route, Route::Player);
    }
    pub fn enqueue(self, id: EpisodeId) {
        let Some(source) = self.catalog.source(id) else { return; };
        let current = self.player.snapshot().map(|snapshot| snapshot.source.episode());
        self.queue_writer.set(self.queue.get_untracked().enqueue(source, current));
    }
    pub fn remove_queue(self, id: EpisodeId) { self.queue_writer.set(self.queue.get_untracked().remove(id)); }
    pub fn next(self) {
        let (next, id) = self.queue.get_untracked().pop();
        self.queue_writer.set(next);
        if let Some(id) = id { self.open_episode(id); }
    }
    pub fn mock_buffer(self) {
        if let Some(identity) = self.player.state.get_untracked().identity() { self.player.dispatch(Event::EngineBuffering { identity }); }
    }
    pub fn mock_started(self) {
        if let Some(identity) = self.player.state.get_untracked().identity() { self.player.dispatch(Event::EngineStarted { identity }); }
    }
    pub fn mock_ended(self) {
        if let Some(identity) = self.player.state.get_untracked().identity() { self.player.dispatch(Event::EngineEnded { identity }); }
    }
    pub fn restore_position(self) {
        if let Some(snapshot) = self.player.snapshot() { self.player.restore(snapshot); }
    }
    pub fn set_speed(self, percent: u16) {
        if let Some(speed_permille) = percent.checked_mul(10) { self.playback(Event::SpeedRequested { speed_permille }); }
    }
    pub fn clear_library(self) {
        self.library_writer.set(LibraryState::default());
        self.progress_writer.set(ProgressState::default());
        // Saved/bookmark/progress clear never unloads the root player or queue.
    }
    pub fn reset_demo(self) {
        self.catalog.load(Scenario::Ready);
        self.library_writer.set(LibraryState::demo());
        self.queue_writer.set(QueueState { items: vec![EpisodeId { work: WorkId::Mountain, index: 3 }] });
        if let Some(source) = self.catalog.source(EpisodeId { work: WorkId::Mountain, index: 2 }) {
            let snapshot = ResumeSnapshot { source, position_ms: 768000, speed_permille: 1000 };
            self.progress_writer.set(ProgressState::default().remember(snapshot));
            self.player.restore(snapshot);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_share_one_root_player_and_library_clear_does_not_unload_it() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let identity = ctx.player.state.get_untracked().identity();
            for route in [Route::Discover, Route::Explore, Route::Library, Route::Player, Route::Queue, Route::Settings] {
                ctx.route.set(route);
                assert_eq!(ctx.player.state.get_untracked().identity(), identity);
            }
            ctx.add_bookmark(); ctx.add_bookmark();
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
            for id in [EpisodeId { work: WorkId::Mountain, index: 5 }, EpisodeId { work: WorkId::River, index: 99 }] { ctx.open_episode(id); ctx.enqueue(id); }
            ctx.resume_bookmark(Bookmark { episode: EpisodeId { work: WorkId::River, index: 0 }, position_ms: 0 });
            assert_eq!(ctx.player.state.get_untracked(), before);
        });
    }
    #[test]
    fn repeated_continue_keeps_current_progress_instead_of_swapping_sessions() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let other = EpisodeId { work: WorkId::River, index: 0 };
            ctx.open_episode(other);
            ctx.playback(Event::SeekRequested { position_ms: 15000 });
            for _ in 0..3 {
                ctx.continue_progress();
                let current = ctx.player.snapshot().unwrap();
                assert_eq!(current.source.episode(), other);
                assert_eq!(current.position_ms, 15000);
                assert_eq!(ctx.progress.get_untracked().latest().unwrap().snapshot, current);
            }
        });
    }

}
