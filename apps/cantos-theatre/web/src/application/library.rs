//! Read-only Library paging. Exact server records are retained; only page admission is local.
use super::account_view::AccountFailure;
use crate::contract::library::{
    HistoryState, PresentedContinueItem, PresentedContinuePage, PresentedHistoryItem,
    PresentedHistoryPage, PresentedSavedItem, PresentedSavedPage,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Continue,
    Saved,
    History,
}
impl Section {
    pub const ALL: [Self; 3] = [Self::Continue, Self::Saved, Self::History];
    pub fn name(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Saved => "saved",
            Self::History => "history",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Continue => 0,
            Self::Saved => 1,
            Self::History => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PagePhase {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed(AccountFailure),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PageState {
    phase: PagePhase,
    cursor: Option<String>,
    generation: u64,
    consumed: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRequest {
    pub section: Section,
    generation: u64,
    cursor: Option<String>,
}
impl PageRequest {
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryPage {
    Continue(PresentedContinuePage),
    Saved(PresentedSavedPage),
    History(PresentedHistoryPage),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryProjection {
    pub continuing: Vec<PresentedContinueItem>,
    pub saved: Vec<PresentedSavedItem>,
    pub history: Vec<PresentedHistoryItem>,
    pub history_state: Option<HistoryState>,
    pages: [PageState; 3],
}
impl LibraryProjection {
    #[cfg(test)]
    pub(super) fn exhaust_page_generation_for_test(&mut self, section: Section) {
        self.pages[section.index()].generation = u64::MAX;
    }

    pub fn phase(&self, section: Section) -> PagePhase {
        self.pages[section.index()].phase
    }
    pub fn has_more(&self, section: Section) -> bool {
        self.pages[section.index()].cursor.is_some()
    }
    pub fn clear_history_cursor(&mut self) {
        let page = &mut self.pages[Section::History.index()];
        page.cursor = None;
        page.consumed.clear();
        if page.phase == PagePhase::Idle {
            page.phase = PagePhase::Ready;
        }
    }
    pub fn begin(&mut self, section: Section, restart: bool) -> Option<PageRequest> {
        let page = &mut self.pages[section.index()];
        if page.phase == PagePhase::Loading {
            return None;
        }
        if !restart
            && (page.phase == PagePhase::Ready && page.cursor.is_none()
                || page.phase == PagePhase::Failed(AccountFailure::CursorExpired))
        {
            return None;
        }
        let generation = page.generation.checked_add(1)?;
        if restart {
            match section {
                Section::Continue => self.continuing.clear(),
                Section::Saved => self.saved.clear(),
                Section::History => {
                    self.history.clear();
                    self.history_state = None;
                }
            }
            page.cursor = None;
            page.consumed.clear();
        }
        page.generation = generation;
        page.phase = PagePhase::Loading;
        Some(PageRequest {
            section,
            generation,
            cursor: page.cursor.clone(),
        })
    }
    pub fn finish(
        &mut self,
        request: &PageRequest,
        result: Result<LibraryPage, AccountFailure>,
    ) -> bool {
        let page = &self.pages[request.section.index()];
        if page.phase != PagePhase::Loading
            || page.generation != request.generation
            || page.cursor != request.cursor
        {
            return false;
        }
        let is_continuation = request.cursor.is_some();
        let result = result.and_then(|response| {
            let (cursor, repeated) = match &response {
                LibraryPage::Continue(p) if request.section == Section::Continue => {
                    let mut seen = std::collections::BTreeSet::new();
                    let internal_dup = p
                        .items
                        .iter()
                        .any(|row| !seen.insert(row.progress.audio.work_id.as_str()));
                    let cross_page_dup = is_continuation
                        && p.items.iter().any(|row| {
                            self.continuing
                                .iter()
                                .any(|old| old.progress.audio.work_id == row.progress.audio.work_id)
                        });
                    (&p.next_cursor, internal_dup || cross_page_dup)
                }
                LibraryPage::Saved(p) if request.section == Section::Saved => {
                    let mut seen = std::collections::BTreeSet::new();
                    let internal_dup = p
                        .items
                        .iter()
                        .any(|row| !seen.insert(row.saved.work_id.as_str()));
                    let cross_page_dup = is_continuation
                        && p.items.iter().any(|row| {
                            self.saved
                                .iter()
                                .any(|old| old.saved.work_id == row.saved.work_id)
                        });
                    (&p.next_cursor, internal_dup || cross_page_dup)
                }
                LibraryPage::History(p) if request.section == Section::History => {
                    if is_continuation
                        && self
                            .history_state
                            .as_ref()
                            .is_some_and(|state| state.epoch != p.state.epoch)
                    {
                        return Err(AccountFailure::CursorExpired);
                    }
                    if self
                        .history_state
                        .as_ref()
                        .is_some_and(|state| p.state.epoch < state.epoch)
                    {
                        return Err(AccountFailure::CursorExpired);
                    }
                    let mut seen = std::collections::BTreeSet::new();
                    let internal_dup = p
                        .items
                        .iter()
                        .any(|row| !seen.insert(row.audio.work_id.as_str()));
                    let cross_page_dup = is_continuation
                        && p.items.iter().any(|row| {
                            self.history
                                .iter()
                                .any(|old| old.audio.work_id == row.audio.work_id)
                        });
                    (&p.next_cursor, internal_dup || cross_page_dup)
                }
                _ => return Err(AccountFailure::Protocol),
            };
            if repeated
                || cursor.as_ref().is_some_and(|next| {
                    request.cursor.as_ref() == Some(next) || page.consumed.contains(next)
                })
            {
                return Err(AccountFailure::Protocol);
            }
            Ok(response)
        });
        let page = &mut self.pages[request.section.index()];
        match result {
            Ok(response) => {
                page.cursor = match response {
                    LibraryPage::Continue(p) => {
                        if !is_continuation {
                            self.continuing.clear();
                        }
                        self.continuing.extend(p.items);
                        p.next_cursor
                    }
                    LibraryPage::Saved(p) => {
                        if !is_continuation {
                            self.saved.clear();
                        }
                        self.saved.extend(p.items);
                        p.next_cursor
                    }
                    LibraryPage::History(p) => {
                        if !is_continuation {
                            self.history.clear();
                        }
                        self.history_state = Some(p.state);
                        self.history.extend(p.items);
                        p.next_cursor
                    }
                };
                if let Some(cursor) = &request.cursor {
                    page.consumed.push(cursor.clone());
                }
                page.phase = PagePhase::Ready;
            }
            Err(error) => page.phase = PagePhase::Failed(error),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        account::AccountScope,
        account_view::{AccountViewState, LibraryCompletion},
    };
    use crate::live::library::{decode_page, request_path};
    const SAVED: &[u8] = include_bytes!("../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_saved.json");
    const HISTORY: &[u8] = include_bytes!("../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_history.json");
    fn account(actor: &str, session: u128) -> AccountViewState {
        let mut s = AccountViewState::default();
        let issued = s.begin_refresh().unwrap();
        s.finish_session(
            issued,
            Ok(AccountScope::from_session(
                actor.into(),
                uuid::Uuid::from_u128(session).to_string(),
            )
            .unwrap()),
            || uuid::Uuid::from_u128(100).to_string(),
        )
        .unwrap();
        s
    }
    fn saved(id: u128, cursor: Option<String>) -> LibraryPage {
        let LibraryPage::Saved(mut page) = decode_page(Section::Saved, SAVED).unwrap() else {
            panic!()
        };
        page.items[0].saved.work_id = uuid::Uuid::from_u128(id).to_string();
        page.items[0].presentation.as_mut().unwrap().work_id = page.items[0].saved.work_id.clone();
        page.next_cursor = cursor;
        LibraryPage::Saved(page)
    }
    #[test]
    fn each_list_owns_loading_failure_cursor_retry_and_end_of_paging() {
        let mut s = account("a", 1);
        let first = s.begin_library(Section::Saved, false).unwrap();
        let continuing = s.begin_library(Section::Continue, false).unwrap();
        assert!(s.begin_library(Section::Saved, false).is_none());
        s.finish_page(&continuing, Err(AccountFailure::Unavailable))
            .unwrap();
        s.finish_page(&first, Ok(saved(1, Some("ab".repeat(32)))))
            .unwrap();
        let more = s.begin_library(Section::Saved, false).unwrap();
        assert_eq!(
            request_path(&more),
            format!(
                "/api/v1/theatre/library/saved?presentation=1&limit=20&cursor={}",
                "ab".repeat(32)
            )
        );
        s.finish_page(&more, Err(AccountFailure::Unavailable))
            .unwrap();
        assert_eq!(s.library().unwrap().saved.len(), 1);
        let retry = s.begin_library(Section::Saved, false).unwrap();
        assert_eq!(retry.page.cursor(), more.page.cursor());
        assert_eq!(
            s.finish_page(&more, Ok(saved(2, None))),
            Ok(LibraryCompletion::Ignored)
        );
        s.finish_page(&retry, Ok(saved(2, None))).unwrap();
        assert_eq!(s.library().unwrap().saved.len(), 2);
        assert!(s.begin_library(Section::Saved, false).is_none());
        assert_eq!(
            s.library().unwrap().phase(Section::Continue),
            PagePhase::Failed(AccountFailure::Unavailable)
        );
        assert!(s.begin_library(Section::Continue, false).is_some());
    }
    #[test]
    fn repeated_cursor_or_work_is_rejected_atomically_and_restart_is_section_local() {
        let mut s = account("a", 1);
        let first = s.begin_library(Section::Saved, false).unwrap();
        s.finish_page(&first, Ok(saved(1, Some("ab".repeat(32)))))
            .unwrap();
        for response in [saved(2, Some("ab".repeat(32))), saved(1, None)] {
            let more = s.begin_library(Section::Saved, false).unwrap();
            s.finish_page(&more, Ok(response)).unwrap();
            assert_eq!(s.library().unwrap().saved.len(), 1);
            assert_eq!(
                s.library().unwrap().phase(Section::Saved),
                PagePhase::Failed(AccountFailure::Protocol)
            );
        }
        let restart = s.begin_library(Section::Saved, true).unwrap();
        assert_eq!(restart.page.cursor(), None);
        assert!(s.library().unwrap().saved.is_empty());
    }
    #[test]
    fn history_epoch_changes_require_explicit_restart_without_losing_other_lists() {
        let mut s = account("a", 1);
        let mut history = serde_json::from_slice::<PresentedHistoryPage>(HISTORY).unwrap();
        history.next_cursor = Some("ab".repeat(32));
        let first = s.begin_library(Section::History, false).unwrap();
        s.finish_page(&first, Ok(LibraryPage::History(history.clone())))
            .unwrap();
        assert_eq!(
            s.library().unwrap().history_state,
            Some(history.state.clone())
        );
        let more = s.begin_library(Section::History, false).unwrap();
        history.items.clear();
        history.state.epoch += 1;
        s.finish_page(&more, Ok(LibraryPage::History(history)))
            .unwrap();
        assert_eq!(
            s.library().unwrap().phase(Section::History),
            PagePhase::Failed(AccountFailure::CursorExpired)
        );
        assert_eq!(s.library().unwrap().history.len(), 1);
        assert!(s.begin_library(Section::History, false).is_none());
        assert!(s.begin_library(Section::History, true).is_some());
    }
    #[test]
    fn late_page_success_and_401_cannot_reenter_after_logout_or_account_switch() {
        let mut s = account("a", 1);
        let old = s.begin_library(Section::Saved, false).unwrap();
        let cleanup = s.begin_logout().unwrap();
        assert!(s.library().is_none());
        assert!(s.finish_logout(cleanup, Ok(())));
        let issued = s.begin_refresh().unwrap();
        s.finish_session(
            issued,
            Ok(
                AccountScope::from_session("b".into(), uuid::Uuid::from_u128(2).to_string())
                    .unwrap(),
            ),
            || uuid::Uuid::from_u128(100).to_string(),
        )
        .unwrap();
        for result in [
            Ok(saved(1, None)),
            Err(AccountFailure::SignedOut),
            Err(AccountFailure::SessionChanged),
        ] {
            assert_eq!(s.finish_page(&old, result), Ok(LibraryCompletion::Ignored));
            assert!(s.library().unwrap().saved.is_empty());
            assert_eq!(s.actor(), Some("b"));
        }
        let current = s.begin_library(Section::Saved, false).unwrap();
        assert!(matches!(
            s.finish_page(&current, Err(AccountFailure::SignedOut)),
            Ok(LibraryCompletion::Cleanup(_))
        ));
        assert!(s.library().is_none());
    }
}
