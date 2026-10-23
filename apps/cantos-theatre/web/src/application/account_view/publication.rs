//! Owned comparison facts for cold writes; playback uses its bounded entry comparison.
use super::super::library_sync::publication::SyncPublication;
use super::*;
use crate::contract::library::{PresentedContinueItem, PresentedHistoryItem, PresentedSavedItem};

#[derive(PartialEq, Eq)]
pub(in crate::application) struct AccountControls {
    phase: SessionPhase,
    scope: Option<AccountScope>,
    generation: AccountGeneration,
    logout_pending: bool,
    cleanup: bool,
    intent: CleanupIntent,
    storage: Option<JournalStorageFault>,
    restore_blocked: bool,
    login_refusal: Option<LoginRefusal>,
    library: bool,
    sync: bool,
}

#[derive(PartialEq, Eq)]
struct LibraryPublication {
    continuing: Vec<PresentedContinueItem>,
    saved: Vec<PresentedSavedItem>,
    history: Vec<PresentedHistoryItem>,
    state: Option<HistoryState>,
    pages: [(PagePhase, bool); 3],
}

#[derive(PartialEq, Eq)]
pub(in crate::application) struct AccountPublication {
    controls: AccountControls,
    library: Option<LibraryPublication>,
    sync: Option<SyncPublication>,
}

impl AccountViewState {
    pub(in crate::application) fn control_publication(&self) -> AccountControls {
        let Self {
            account,
            phase,
            library,
            sync,
            cleanup_in_flight,
            cleanup_intent,
            storage_fault,
            journal_restore_blocked,
            login_refusal,
        } = self;
        AccountControls {
            phase: *phase,
            scope: account.scope().cloned(),
            generation: account.generation(),
            logout_pending: account.logout_pending(),
            cleanup: *cleanup_in_flight,
            intent: cleanup_intent.clone(),
            storage: *storage_fault,
            restore_blocked: *journal_restore_blocked,
            login_refusal: *login_refusal,
            library: library.is_some(),
            sync: sync.is_some(),
        }
    }

    pub(in crate::application) fn publication_snapshot(&self) -> AccountPublication {
        AccountPublication {
            controls: self.control_publication(),
            library: self.library().map(|v| LibraryPublication {
                continuing: v.continuing.clone(),
                saved: v.saved.clone(),
                history: v.history.clone(),
                state: v.history_state.clone(),
                pages: Section::ALL.map(|s| (v.phase(s), v.has_more(s))),
            }),
            sync: self.sync().map(LibrarySyncState::publication_snapshot),
        }
    }
}
