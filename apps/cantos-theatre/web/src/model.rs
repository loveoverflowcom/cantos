//! Pure, session-only presentation state. Not the listener playback or sync contract.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkId { Mountain, River, Dream }
impl WorkId {
    pub const ALL: [Self; 3] = [Self::Mountain, Self::River, Self::Dream];
    pub const fn slug(self) -> &'static str {
        match self { Self::Mountain => "son-ha-ky", Self::River => "den-khuya", Self::Dream => "mong-ho-diep" }
    }
    pub fn from_slug(value: &str) -> Option<Self> { Self::ALL.into_iter().find(|work| work.slug() == value) }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre { All, Fantasy, Memory, Dream }
#[derive(Clone, Copy, Debug)]
pub struct Work {
    pub id: WorkId, pub title: &'static str, pub genre: Genre,
    pub synopsis: &'static str, pub episodes: &'static [Episode],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpisodeId { pub work: WorkId, pub index: usize }
#[derive(Clone, Copy, Debug)]
pub struct Episode { pub title: &'static str, pub duration: u32, pub available: bool }
const MOUNTAIN: [Episode; 6] = [
    Episode { title: "Một tiếng chuông trong sương", duration: 1480, available: true },
    Episode { title: "Bến cũ bên dòng Vọng", duration: 1604, available: true },
    Episode { title: "Người giữ tiếng chuông", duration: 1722, available: true },
    Episode { title: "Ngày mai, mình có diễn tiếp không?", duration: 1588, available: true },
    Episode { title: "Ánh đèn cuối sân khấu — Một lời hẹn chưa bao giờ nói ra", duration: 1833, available: true },
    Episode { title: "Cửa núi chưa mở", duration: 0, available: false },
];
const RIVER: [Episode; 4] = [
    Episode { title: "Ngọn đèn đầu ngõ", duration: 1232, available: true },
    Episode { title: "Người dẫn chuyện bên sông", duration: 1470, available: true },
    Episode { title: "Một lá thư gửi mùa thu", duration: 1640, available: true },
    Episode { title: "Trở về bến cũ", duration: 1320, available: true },
];
const DREAM: [Episode; 5] = [
    Episode { title: "Cánh bướm trong vườn", duration: 1328, available: true },
    Episode { title: "Giấc mơ của người kể chuyện", duration: 1560, available: true },
    Episode { title: "Dấu chân bên hiên vắng", duration: 1474, available: true },
    Episode { title: "Qua một miền tưởng tượng", duration: 1620, available: true },
    Episode { title: "Khi bình minh trở lại", duration: 1510, available: true },
];
pub fn work(id: WorkId) -> Work {
    match id {
        WorkId::Mountain => Work { id, title: "Sơn Hà Ký", genre: Genre::Fantasy,
            synopsis: "Một tiếng chuông lay động sơn hà. Một hành trình tìm lại điều đã lãng quên.", episodes: &MOUNTAIN },
        WorkId::River => Work { id, title: "Đèn khuya bên sông", genre: Genre::Memory,
            synopsis: "Ánh đèn trên bến nhỏ, những lá thư và một câu chuyện đợi người trở về.", episodes: &RIVER },
        WorkId::Dream => Work { id, title: "Mộng hồ điệp", genre: Genre::Dream,
            synopsis: "Theo một cánh bướm qua những miền ký ức, nơi giấc mơ gặp lời kể.", episodes: &DREAM },
    }
}
pub fn episode(id: EpisodeId) -> Option<&'static Episode> { work(id.work).episodes.get(id.index) }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route { Discover, Explore, Work(WorkId), Library, Player, Queue, Settings, Account, NotFound }
impl Route {
    pub fn from_hash(hash: &str) -> Self {
        match hash.trim_start_matches('#') {
            "" | "discover" => Self::Discover, "explore" => Self::Explore,
            "library" => Self::Library, "player" => Self::Player, "queue" => Self::Queue,
            "settings" => Self::Settings, "account" => Self::Account,
            value => value.strip_prefix("work/").and_then(WorkId::from_slug).map(Self::Work).unwrap_or(Self::NotFound),
        }
    }
    pub fn hash(self) -> String {
        match self {
            Self::Discover => "#discover".into(), Self::Explore => "#explore".into(),
            Self::Work(id) => format!("#work/{}", id.slug()), Self::Library => "#library".into(),
            Self::Player => "#player".into(), Self::Queue => "#queue".into(),
            Self::Settings => "#settings".into(), Self::Account => "#account".into(), Self::NotFound => "#not-found".into(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scenario { #[default] Ready, Loading, Empty, Error }
impl Scenario {
    pub const fn value(self) -> &'static str {
        match self { Self::Ready => "ready", Self::Loading => "loading", Self::Empty => "empty", Self::Error => "error" }
    }
    pub fn from_value(value: &str) -> Option<Self> {
        match value { "ready" => Some(Self::Ready), "loading" => Some(Self::Loading), "empty" => Some(Self::Empty), "error" => Some(Self::Error), _ => None }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LibraryTab { #[default] Continue, Saved, Bookmarks }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport { Paused, Simulating, Ended }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockPlayer { pub episode: EpisodeId, pub position: u32, pub transport: Transport }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bookmark { pub episode: EpisodeId, pub position: u32 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemoState {
    pub scenario: Scenario, pub query: String, pub genre: Genre, pub saved: BTreeSet<WorkId>,
    pub library_tab: LibraryTab, pub player: Option<MockPlayer>, pub queue: Vec<EpisodeId>,
    pub bookmarks: Vec<Bookmark>,
    /// Preference mock only. No engine, clock or timer side effect.
    pub speed: u16,
}
impl Default for DemoState {
    fn default() -> Self {
        Self {
            scenario: Scenario::Ready, query: String::new(), genre: Genre::All,
            saved: BTreeSet::from([WorkId::Mountain]), library_tab: LibraryTab::Continue,
            player: Some(MockPlayer { episode: EpisodeId { work: WorkId::Mountain, index: 2 }, position: 768, transport: Transport::Paused }),
            queue: vec![EpisodeId { work: WorkId::Mountain, index: 3 }], bookmarks: Vec::new(), speed: 100,
        }
    }
}
#[derive(Clone, Debug)]
pub enum Event {
    SetScenario(Scenario), Search(String), Filter(Genre), ToggleSaved(WorkId), LibraryTab(LibraryTab),
    Select(EpisodeId), ToggleTransport, Seek(u32), Skip(i32), Enqueue(EpisodeId), RemoveQueue(EpisodeId),
    Next, AddBookmark, RemoveBookmark(Bookmark), ResumeBookmark(Bookmark), Speed(u16), ClearLibrary, Reset,
}
fn selectable(id: EpisodeId) -> bool { episode(id).is_some_and(|episode| episode.available && episode.duration > 0) }
fn select(state: &mut DemoState, id: EpisodeId) {
    if !selectable(id) { return; }
    if state.player.as_ref().is_none_or(|player| player.episode != id) {
        state.player = Some(MockPlayer { episode: id, position: 0, transport: Transport::Paused });
    }
    state.queue.retain(|queued| *queued != id);
}
fn seek(state: &mut DemoState, position: u32) {
    let Some(player) = state.player.as_mut() else { return; };
    let Some(episode) = episode(player.episode) else { return; };
    player.position = position.min(episode.duration);
    player.transport = if player.position == episode.duration { Transport::Ended } else { Transport::Paused };
}
/// Changes only temporary presentation. No real playback, persistence or sync decisions.
#[must_use]
pub fn reduce(mut state: DemoState, event: Event) -> DemoState {
    match event {
        Event::SetScenario(scenario) => state.scenario = scenario,
        Event::Search(query) => state.query = query,
        Event::Filter(genre) => state.genre = genre,
        Event::ToggleSaved(id) => { if !state.saved.remove(&id) { state.saved.insert(id); } }
        Event::LibraryTab(tab) => state.library_tab = tab,
        Event::Select(id) => select(&mut state, id),
        Event::ToggleTransport => if let Some(player) = state.player.as_mut() {
            player.transport = match player.transport {
                Transport::Paused => Transport::Simulating, Transport::Simulating => Transport::Paused,
                Transport::Ended => { player.position = 0; Transport::Simulating }
            };
        },
        Event::Seek(position) => seek(&mut state, position),
        Event::Skip(delta) => if let Some(player) = &state.player {
            let position = (i64::from(player.position) + i64::from(delta)).clamp(0, i64::from(u32::MAX)) as u32;
            seek(&mut state, position);
        },
        Event::Enqueue(id) => if selectable(id) && !state.queue.contains(&id)
            && state.player.as_ref().is_none_or(|player| player.episode != id) { state.queue.push(id); },
        Event::RemoveQueue(id) => state.queue.retain(|queued| *queued != id),
        Event::Next => if !state.queue.is_empty() { let id = state.queue.remove(0); select(&mut state, id); },
        Event::AddBookmark => if let Some(player) = &state.player {
            let bookmark = Bookmark { episode: player.episode, position: player.position };
            if !state.bookmarks.contains(&bookmark) { state.bookmarks.push(bookmark); }
        },
        Event::RemoveBookmark(bookmark) => state.bookmarks.retain(|item| *item != bookmark),
        Event::ResumeBookmark(bookmark) => if state.bookmarks.contains(&bookmark) && selectable(bookmark.episode) {
            select(&mut state, bookmark.episode); seek(&mut state, bookmark.position);
        },
        Event::Speed(speed) => if [75, 100, 125, 150, 200].contains(&speed) { state.speed = speed; },
        Event::ClearLibrary => { state.saved.clear(); state.bookmarks.clear(); state.player = None; state.queue.clear(); }
        Event::Reset => return DemoState::default(),
    }
    state
}
pub fn matching_works(state: &DemoState) -> Vec<WorkId> {
    let query = state.query.trim().to_lowercase();
    WorkId::ALL.into_iter().filter(|id| {
        let item = work(*id);
        (state.genre == Genre::All || state.genre == item.genre)
            && (query.is_empty() || format!("{} {}", item.title, item.synopsis).to_lowercase().contains(&query))
    }).collect()
}
pub fn clock(seconds: u32) -> String { format!("{}:{:02}", seconds / 60, seconds % 60) }

#[cfg(test)]
mod tests {
    use super::*;
    const OTHER: EpisodeId = EpisodeId { work: WorkId::River, index: 1 };
    #[test]
    fn routes_round_trip_and_unknown_values_have_a_recovery_route() {
        for route in [Route::Discover, Route::Explore, Route::Library, Route::Player, Route::Queue, Route::Settings, Route::Account, Route::NotFound] {
            assert_eq!(Route::from_hash(&route.hash()), route);
        }
        for work in WorkId::ALL { assert_eq!(Route::from_hash(&Route::Work(work).hash()), Route::Work(work)); }
        assert_eq!(Route::from_hash("#work/secret-draft"), Route::NotFound);
    }
    #[test]
    fn selection_is_repeat_safe_and_unavailable_or_unknown_items_cannot_play() {
        let state = reduce(DemoState::default(), Event::Select(OTHER));
        let state = reduce(state, Event::Seek(48));
        assert_eq!(reduce(state.clone(), Event::Select(OTHER)), state);
        for id in [EpisodeId { work: WorkId::Mountain, index: 5 }, EpisodeId { work: WorkId::River, index: 99 }] {
            assert_eq!(reduce(state.clone(), Event::Select(id)), state);
            assert_eq!(reduce(state.clone(), Event::Enqueue(id)), state);
        }
    }
    #[test]
    fn seeks_are_bounded_and_ended_mock_restarts_only_after_a_gesture() {
        let state = reduce(DemoState::default(), Event::Skip(-10000));
        assert_eq!(state.player.as_ref().map(|player| player.position), Some(0));
        let state = reduce(state, Event::Seek(u32::MAX));
        assert_eq!(state.player.as_ref().map(|player| player.position), Some(1722));
        assert_eq!(state.player.as_ref().map(|player| player.transport), Some(Transport::Ended));
        let state = reduce(state, Event::ToggleTransport);
        assert_eq!(state.player.as_ref().map(|player| player.position), Some(0));
        assert_eq!(state.player.as_ref().map(|player| player.transport), Some(Transport::Simulating));
    }
    #[test]
    fn bookmarks_are_deduplicated_and_resumed_at_their_exact_demo_position() {
        let state = reduce(DemoState::default(), Event::AddBookmark);
        let state = reduce(state, Event::AddBookmark);
        assert_eq!(state.bookmarks.len(), 1);
        let mark = state.bookmarks[0];
        let state = reduce(state, Event::Select(OTHER));
        let state = reduce(state, Event::ResumeBookmark(mark));
        assert_eq!(state.player.as_ref().map(|player| player.position), Some(768));
        let state = reduce(state, Event::RemoveBookmark(mark));
        assert!(state.bookmarks.is_empty());
        assert_eq!(reduce(state.clone(), Event::ResumeBookmark(mark)), state);
    }
    #[test]
    fn queue_is_deduplicated_and_empty_next_is_safe() {
        let state = reduce(DemoState::default(), Event::Enqueue(OTHER));
        let state = reduce(state, Event::Enqueue(OTHER));
        assert_eq!(state.queue.len(), 2);
        let state = reduce(state, Event::Next);
        assert_eq!(state.player.as_ref().map(|player| player.episode.index), Some(3));
        let state = reduce(state, Event::RemoveQueue(OTHER));
        assert_eq!(reduce(state.clone(), Event::Next), state);
    }
    #[test]
    fn scenarios_do_not_erase_position_and_reset_restores_all_demo_data() {
        let initial = DemoState::default();
        for scenario in [Scenario::Ready, Scenario::Loading, Scenario::Empty, Scenario::Error] {
            let state = reduce(initial.clone(), Event::SetScenario(scenario));
            assert_eq!(state.player, initial.player);
            assert_eq!(reduce(state, Event::Reset), initial);
        }
        let empty = reduce(initial.clone(), Event::ClearLibrary);
        assert!(empty.player.is_none() && empty.saved.is_empty() && empty.bookmarks.is_empty() && empty.queue.is_empty());
        assert_eq!(reduce(empty.clone(), Event::ToggleTransport), empty);
        assert_eq!(reduce(empty, Event::Reset), initial);
    }
    #[test]
    fn mock_search_is_case_insensitive_and_deliberately_diacritic_sensitive() {
        let state = reduce(DemoState::default(), Event::Search(" SƠN HÀ ".into()));
        assert_eq!(matching_works(&state), vec![WorkId::Mountain]);
        assert!(matching_works(&reduce(state, Event::Filter(Genre::Memory))).is_empty());
        assert_eq!(clock(768), "12:48");
    }
}
