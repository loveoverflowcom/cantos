//! Saved works and point bookmarks are separate session-only concepts.
use crate::core::ids::{EpisodeId, WorkId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedWork {
    pub work: WorkId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub episode: EpisodeId,
    pub position_ms: u32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryState {
    pub saved: Vec<SavedWork>,
    pub bookmarks: Vec<Bookmark>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryEvent {
    ToggleSaved(WorkId),
    SetSaved { work: WorkId, saved: bool },
    AddBookmark(Bookmark),
    RemoveBookmark(Bookmark),
    Clear,
}

impl LibraryState {
    pub fn demo() -> Self {
        Self {
            saved: vec![SavedWork {
                work: WorkId::Mountain,
            }],
            bookmarks: Vec::new(),
        }
    }
    pub fn is_saved(&self, work: WorkId) -> bool {
        self.saved.iter().any(|saved| saved.work == work)
    }
}
#[must_use]
pub fn reduce(mut state: LibraryState, event: LibraryEvent) -> LibraryState {
    match event {
        LibraryEvent::ToggleSaved(work) => {
            if state.is_saved(work) {
                state.saved.retain(|saved| saved.work != work);
            } else {
                state.saved.push(SavedWork { work });
            }
        }
        LibraryEvent::SetSaved { work, saved } => {
            if saved && !state.is_saved(work) {
                state.saved.push(SavedWork { work });
            } else if !saved {
                state.saved.retain(|item| item.work != work);
            }
        }
        LibraryEvent::AddBookmark(mark) => {
            if !state.bookmarks.contains(&mark) {
                state.bookmarks.push(mark);
            }
        }
        LibraryEvent::RemoveBookmark(mark) => state.bookmarks.retain(|item| *item != mark),
        LibraryEvent::Clear => return LibraryState::default(),
    }
    state
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desired_save_intent_is_repeat_safe_and_preserves_bookmarks() {
        let mark = Bookmark {
            episode: EpisodeId {
                work: WorkId::River,
                index: 0,
            },
            position_ms: 6000,
        };
        let initial = reduce(LibraryState::demo(), LibraryEvent::AddBookmark(mark));
        let save = LibraryEvent::SetSaved {
            work: WorkId::River,
            saved: true,
        };
        let saved = reduce(initial.clone(), save);
        assert_eq!(reduce(saved.clone(), save), saved);
        assert_eq!(saved.bookmarks, [mark]);
        let remove = LibraryEvent::SetSaved {
            work: WorkId::River,
            saved: false,
        };
        let removed = reduce(saved, remove);
        assert_eq!(removed, initial);
        assert_eq!(reduce(removed.clone(), remove), removed);
    }
    #[test]
    fn saved_work_is_not_a_bookmark_and_bookmarks_are_repeat_safe() {
        let mark = Bookmark {
            episode: EpisodeId {
                work: WorkId::River,
                index: 0,
            },
            position_ms: 6000,
        };
        let state = reduce(LibraryState::demo(), LibraryEvent::AddBookmark(mark));
        assert_eq!(
            reduce(state.clone(), LibraryEvent::AddBookmark(mark)),
            state
        );
        assert!(!state.is_saved(WorkId::River));
        assert_eq!(reduce(state, LibraryEvent::Clear), LibraryState::default());
    }
}
