//! Host-compilable Leptos publication boundary. Browser I/O is supplied by the runtime.
use super::{
    account::{AccountError, AccountGeneration, AccountScope, RequestFence},
    account_view::{
        AccountFailure, AccountViewState, CleanupIntent, ExplicitProgress, JournalStorageFault,
        LibraryCompletion, LibraryRequest, SessionCompletion, SessionPhase, SyncResult,
    },
    library::{LibraryPage, Section},
    library_sync::{
        AccountSyncJournal, AppliedReceipt, CommandError, PendingMutation, ResumeRefusal,
        ResumeTicket,
    },
    player_session::PlaybackObservation,
};
use cantos_contracts::listener_library::{
    AudioIdentity, AudioPresentation, HistoryState, ProgressRecord, Receipt, ResumeStatus,
    SavedRecord, WorkPresentation,
};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct AccountWriter {
    state: RwSignal<AccountViewState>,
}

// Delegation keeps signal mutation out of wasm-only code. The pure owner admits each
// capability; refused capabilities do not publish. Admitted no-op suppression is S3's scope.
macro_rules! capability {
    ($name:ident($($arg:ident: $ty:ty),* $(,)?) -> $out:ty) => {
        pub fn $name(self, $($arg: $ty),*) -> Option<SyncResult<$out>> {
            self.state.try_maybe_update(|state| {
                let result = state.$name($($arg),*);
                (result.is_ok(), result)
            })
        }
    };
}

impl AccountWriter {
    pub fn new(state: RwSignal<AccountViewState>) -> Self {
        Self { state }
    }

    capability!(request_saved(work: &str, desired: bool, presentation: Option<WorkPresentation>, mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Option<PendingMutation>);
    capability!(retry_saved(work: &str, mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Option<PendingMutation>);
    capability!(request_progress(audio: AudioIdentity, intent: ExplicitProgress, presentation: Option<AudioPresentation>, now_ms: i64, mint: impl FnOnce() -> String) -> Option<PendingMutation>);
    capability!(resolve_progress_conflict(publication: &str, keep_local: bool, mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Option<(PendingMutation, u64)>);
    capability!(retry_progress(publication: &str, mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Option<PendingMutation>);
    capability!(prompt_clear_history() -> Option<HistoryState>);
    capability!(cancel_clear_history() -> ());
    capability!(confirm_clear_history(mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Option<PendingMutation>);
    capability!(retry_history(mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Option<PendingMutation>);
    capability!(begin_resume(publication: String, status: ResumeStatus, presentation: Option<AudioPresentation>) -> Option<ResumeTicket>);
    capability!(reset_resume() -> ());
    capability!(ensure_saved_read(work: &str, presentation: Option<WorkPresentation>) -> bool);
    capability!(ensure_progress_read(publication: &str, presentation: Option<AudioPresentation>) -> bool);
    capability!(reread_saved(fence: &RequestFence, work: &str) -> bool);
    capability!(reread_progress(fence: &RequestFence, publication: &str) -> bool);
    capability!(finish_saved_read(fence: &RequestFence, record: SavedRecord) -> bool);
    capability!(fail_saved_read(fence: &RequestFence, work: &str) -> ());
    capability!(finish_progress_read(fence: &RequestFence, publication: &str, record: Option<ProgressRecord>, mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Option<PendingMutation>);
    capability!(fail_progress_read(fence: &RequestFence, publication: &str) -> ());
    capability!(finish_resume(fence: &RequestFence, ticket: ResumeTicket, result: Result<u32, ResumeRefusal>) -> bool);
    capability!(apply_receipt(fence: &RequestFence, operation: &str, receipt: Receipt, via_lookup: bool, mint: impl FnOnce() -> String, reported_at_ms: Option<i64>) -> Result<AppliedReceipt, CommandError>);
    capability!(record_unknown_outcome(fence: &RequestFence, operation: &str) -> bool);
    capability!(record_command_error(fence: &RequestFence, operation: &str, error: CommandError, rotate_device: impl FnOnce() -> String) -> bool);
    capability!(observe_playback(observation: PlaybackObservation, presentation: Option<AudioPresentation>, now_ms: i64, mint_session: impl FnOnce() -> String, mint_operation: impl FnOnce() -> String) -> Option<PendingMutation>);

    pub fn begin_refresh(self) -> Option<Result<AccountGeneration, AccountError>> {
        self.state.try_update(AccountViewState::begin_refresh)
    }
    pub fn begin_logout(self) -> Option<(AccountGeneration, CleanupIntent)> {
        self.state
            .try_update(|state| {
                if state.cleanup_in_flight() {
                    return None;
                }
                let issued = state.begin_logout().ok()?;
                Some((issued, state.cleanup_intent().clone()))
            })
            .flatten()
    }
    pub fn credentials_lost(self) -> Option<AccountGeneration> {
        self.state
            .try_update(|state| {
                if state.phase() != SessionPhase::SignedIn {
                    return None;
                }
                state.begin_logout().ok()
            })
            .flatten()
    }
    pub fn finish_session(
        self,
        issued: AccountGeneration,
        result: Result<AccountScope, AccountFailure>,
        mint: impl FnOnce() -> String,
    ) -> Option<Result<SessionCompletion, AccountError>> {
        self.state
            .try_update(|state| state.finish_session(issued, result, mint))
    }
    pub fn finish_logout(
        self,
        issued: AccountGeneration,
        result: Result<(), AccountFailure>,
    ) -> Option<bool> {
        self.state
            .try_update(|state| state.finish_logout(issued, result))
    }
    pub fn fail_fenced_command(
        self,
        fence: &RequestFence,
        failure: AccountFailure,
    ) -> Option<Result<LibraryCompletion, AccountError>> {
        self.state
            .try_update(|state| state.fail_fenced_command(fence, failure))
    }
    pub fn begin_library(self, section: Section, restart: bool) -> Option<LibraryRequest> {
        self.state
            .try_update(|state| state.begin_library(section, restart))
            .flatten()
    }
    pub fn finish_page(
        self,
        request: &LibraryRequest,
        result: Result<LibraryPage, AccountFailure>,
        mint: impl FnOnce() -> String,
    ) -> Option<Result<(LibraryCompletion, Option<PendingMutation>), AccountError>> {
        self.state
            .try_update(|state| state.finish_page_with_followup(request, result, mint))
    }
    pub fn navigation_failed(self, failure: AccountFailure) {
        let _ = self
            .state
            .try_update(|state| state.navigation_failed(failure));
    }
    pub fn restore_sync_journal(
        self,
        fence: &RequestFence,
        journal: AccountSyncJournal,
    ) -> Option<bool> {
        self.state
            .try_update(|state| state.restore_sync_journal(fence, journal))
    }
    pub fn record_journal_restore(
        self,
        fence: &RequestFence,
        result: Result<(), JournalStorageFault>,
    ) {
        let _ = self
            .state
            .try_update(|state| state.record_journal_restore(fence, result));
    }

    /// Snapshot only the journal, drop the read guard, then call storage. No account clone.
    pub fn persist_journal(
        self,
        persist: impl FnOnce(&AccountSyncJournal) -> Result<(), JournalStorageFault>,
    ) -> bool {
        let Some(Some((fence, journal))) = self.state.try_with_untracked(|state| {
            if !state.can_persist_journal() {
                return None;
            }
            Some((state.fence()?, state.sync()?.to_journal()))
        }) else {
            return false;
        };
        let result = persist(&journal);
        let _ = self
            .state
            .try_update(|state| state.record_storage(&fence, result));
        result.is_ok()
    }

    fn accepts_dispatch(self, fence: &RequestFence, pending: &PendingMutation) -> bool {
        self.state
            .try_with_untracked(|state| {
                state.accepts(fence)
                    && state
                        .sync()
                        .and_then(|sync| sync.pending_mutation(&pending.operation_id))
                        .is_some_and(|stored| stored.payload == pending.payload)
            })
            .unwrap_or(false)
    }

    /// The callback creates the network future only after a durable snapshot. Its execution
    /// and every awaited completion remain fenced by the runtime and the pure owner.
    pub fn dispatch_after_persist<T>(
        self,
        fence: RequestFence,
        pending: PendingMutation,
        persist: impl FnOnce(&AccountSyncJournal) -> Result<(), JournalStorageFault>,
        send: impl FnOnce(RequestFence, PendingMutation) -> T,
    ) -> Option<T> {
        if !self.accepts_dispatch(&fence, &pending) {
            return None;
        }
        if !self.persist_journal(persist) {
            let _ = self.record_unknown_outcome(&fence, &pending.operation_id);
            return None;
        }
        // Storage is injected and executes outside a guard; it can reenter the account owner.
        if !self.accepts_dispatch(&fence, &pending) {
            return None;
        }
        Some(send(fence, pending))
    }
}

#[cfg(test)]
#[path = "account_writer/tests.rs"]
mod tests;
