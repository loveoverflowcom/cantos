//! Mounted listener account lifecycle; private projections are owned by one verified session.
mod publication;
use super::account::{AccountError, AccountGeneration, AccountScope, AccountState, RequestFence};
pub use super::library::LibraryProjection;
use super::library::{LibraryPage, PagePhase, PageRequest, Section};
use super::library_sync::{
    audio_identity_from_source, record_started_in_projection, AppliedReceipt, CommandError,
    PendingMutation, ResumeAttemptState, ResumeRefusal, ResumeTicket,
};
pub use super::library_sync::{AccountSyncJournal, LibrarySyncState};
use super::player_session::PlaybackObservation;
use crate::contract::library::{
    AudioIdentity, AudioPresentation, HistoryState, ProgressKind, ProgressRecord, Receipt,
    ResumeStatus, SavedRecord, WorkPresentation,
};

/// The API refused an email/password sign-in outright. It never names which factor failed,
/// and the previous (guest) state is unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginRefusal {
    /// 401: wrong or unknown email/password.
    Credentials,
    /// 400: the request shape or password policy was refused.
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountFailure {
    Diagnostic(super::errors::ErrorKind),
    SignedOut,
    SessionChanged,
    Unavailable,
    /// 429 with a bounded Retry-After in seconds; never retried densely.
    RateLimited(u32),
    /// 404: Identity is not configured. There is no fallback provider or mock.
    NotFound,
    Protocol,
    LoginRefused(LoginRefusal),
    LogoutPending,
    CursorExpired,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SessionPhase {
    #[default]
    Idle,
    Checking,
    Guest,
    SignedIn,
    Failed(AccountFailure),
    LogoutPending,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionCompletion {
    Authenticated(RequestFence),
    Cleanup(AccountGeneration),
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryCompletion {
    Applied,
    Cleanup(AccountGeneration),
    Ignored,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CleanupIntent {
    ExpectedSession(String),
    #[default]
    StaleOnly,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountViewState {
    account: AccountState,
    phase: SessionPhase,
    library: Option<LibraryProjection>,
    sync: Option<LibrarySyncState>,
    cleanup_in_flight: bool,
    cleanup_intent: CleanupIntent,
    storage_fault: Option<JournalStorageFault>,
    journal_restore_blocked: bool,
    login_refusal: Option<LoginRefusal>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalStorageFault {
    Unavailable,
    Corrupt,
}

impl AccountViewState {
    #[cfg(test)]
    pub(super) fn set_generation_for_test(&mut self, generation: u64) {
        self.account.set_generation_for_test(generation);
    }
    pub fn storage_fault(&self) -> Option<JournalStorageFault> {
        self.storage_fault
    }
    pub fn record_storage(
        &mut self,
        fence: &RequestFence,
        result: Result<(), JournalStorageFault>,
    ) {
        if self.accepts(fence) {
            self.storage_fault = result.err();
        }
    }
    pub fn record_journal_restore(
        &mut self,
        fence: &RequestFence,
        result: Result<(), JournalStorageFault>,
    ) {
        if self.accepts(fence) {
            self.journal_restore_blocked = result.is_err();
            self.storage_fault = result.err();
        }
    }
    pub fn journal_restore_blocked(&self) -> bool {
        self.journal_restore_blocked
    }
    pub fn can_persist_journal(&self) -> bool {
        !self.journal_restore_blocked && self.storage_fault != Some(JournalStorageFault::Corrupt)
    }

    pub fn phase(&self) -> SessionPhase {
        self.phase
    }
    pub fn library(&self) -> Option<&LibraryProjection> {
        self.library.as_ref()
    }
    pub fn sync(&self) -> Option<&LibrarySyncState> {
        self.sync.as_ref()
    }
    pub fn fence(&self) -> Option<RequestFence> {
        if self.phase != SessionPhase::SignedIn {
            return None;
        }
        self.account.fence().ok()
    }
    pub fn accepts(&self, fence: &RequestFence) -> bool {
        self.phase == SessionPhase::SignedIn && self.account.accepts(fence)
    }
    pub fn can_authenticate(&self) -> bool {
        self.phase == SessionPhase::Guest && !self.account.logout_pending()
    }
    /// Email/password sign-in is always offered to a guest. There is no provider capability to
    /// read first; an unconfigured API answers the attempt with a clear error instead.
    pub fn can_login(&self) -> bool {
        self.can_authenticate()
    }
    /// The last definite refusal of a sign-in attempt, until the next lookup or logout.
    pub fn login_refusal(&self) -> Option<LoginRefusal> {
        self.login_refusal
    }
    pub fn actor(&self) -> Option<&str> {
        self.account.scope().map(AccountScope::actor)
    }
    pub fn cleanup_in_flight(&self) -> bool {
        self.cleanup_in_flight
    }
    pub fn generation(&self) -> AccountGeneration {
        self.account.generation()
    }
    pub fn cleanup_intent(&self) -> &CleanupIntent {
        &self.cleanup_intent
    }
    pub fn can_retry_logout(&self) -> bool {
        self.phase == SessionPhase::LogoutPending && !self.cleanup_in_flight
    }
    fn clear_private(&mut self) {
        self.storage_fault = None;
        self.journal_restore_blocked = false;
        self.library = None;
        self.sync = None;
    }
    /// Generation exhaustion is an intentional fail-closed transition, not an atomic refusal.
    fn block_exhaustion(&mut self) {
        self.clear_private();
        self.phase = SessionPhase::Failed(AccountFailure::Unavailable);
        self.cleanup_intent = CleanupIntent::StaleOnly;
        self.cleanup_in_flight = false;
    }
    pub fn begin_refresh(&mut self) -> Result<AccountGeneration, AccountError> {
        if self.phase == SessionPhase::Checking {
            return Err(AccountError::RequestPending);
        }
        self.begin_cookie_observation()
    }
    /// A peer cookie change supersedes even an in-flight lookup. A signal grants no account:
    /// clear private state and require a new server observation, preserving our own logout.
    pub fn begin_cookie_observation(&mut self) -> Result<AccountGeneration, AccountError> {
        if self.account.logout_pending() {
            return Err(AccountError::LogoutPending);
        }
        if let Err(error) = self.account.leave() {
            if error == AccountError::Exhausted {
                self.block_exhaustion();
            }
            return Err(error);
        }
        self.clear_private();
        self.login_refusal = None;
        self.phase = SessionPhase::Checking;
        Ok(self.account.generation())
    }
    pub fn finish_session(
        &mut self,
        issued: AccountGeneration,
        result: Result<AccountScope, AccountFailure>,
        new_device_id: impl FnOnce() -> String,
    ) -> Result<SessionCompletion, AccountError> {
        if issued != self.account.generation() || self.phase != SessionPhase::Checking {
            return Ok(SessionCompletion::Ignored);
        }
        match result {
            Ok(scope) => {
                let actor = scope.actor().to_owned();
                if let Err(error) = self.account.finish_session(issued, scope) {
                    if error == AccountError::Exhausted {
                        self.block_exhaustion();
                    }
                    return Err(error);
                }
                self.phase = SessionPhase::SignedIn;
                self.library = Some(LibraryProjection::default());
                self.sync = LibrarySyncState::new(actor, new_device_id());
                Ok(SessionCompletion::Authenticated(self.account.fence()?))
            }
            Err(AccountFailure::SignedOut) => {
                Ok(SessionCompletion::Cleanup(self.begin_expiry_cleanup()?))
            }
            Err(AccountFailure::LoginRefused(refusal)) => {
                // A definite refusal replaced no cookie: the listener is still a guest and can
                // try again, with nothing to clean up.
                self.clear_private();
                self.login_refusal = Some(refusal);
                self.phase = SessionPhase::Guest;
                Ok(SessionCompletion::Ignored)
            }
            Err(error) => {
                self.clear_private();
                self.phase = SessionPhase::Failed(error);
                Ok(SessionCompletion::Ignored)
            }
        }
    }
    /// Restore a validated account-scoped outbox journal for the active session fence.
    pub fn restore_sync_journal(
        &mut self,
        fence: &RequestFence,
        journal: AccountSyncJournal,
    ) -> bool {
        if !self.accepts(fence) {
            return false;
        }
        let Some(actor) = self.actor().map(str::to_owned) else {
            return false;
        };
        let Some(validated) = journal.restore_for_actor(&actor) else {
            return false;
        };
        self.sync = Some(LibrarySyncState::from_journal(validated));
        if let (Some(sync), Some(library)) = (self.sync.as_ref(), self.library.as_mut()) {
            sync.sync_projection(library);
        }
        true
    }
    /// Handle an authentication failure observed by a fenced library or progress command.
    pub fn fail_fenced_command(
        &mut self,
        fence: &RequestFence,
        failure: AccountFailure,
    ) -> Result<LibraryCompletion, AccountError> {
        if !self.accepts(fence) {
            return Ok(LibraryCompletion::Ignored);
        }
        match failure {
            AccountFailure::SignedOut => {
                Ok(LibraryCompletion::Cleanup(self.begin_expiry_cleanup()?))
            }
            AccountFailure::SessionChanged => {
                self.clear_private();
                self.phase = SessionPhase::Failed(AccountFailure::SessionChanged);
                Ok(LibraryCompletion::Applied)
            }
            _ => Ok(LibraryCompletion::Ignored),
        }
    }
    pub fn begin_logout(&mut self) -> Result<AccountGeneration, AccountError> {
        let cleanup_intent = if self.phase != SessionPhase::LogoutPending {
            self.account
                .scope()
                .map_or(CleanupIntent::StaleOnly, |scope| {
                    CleanupIntent::ExpectedSession(scope.session().to_owned())
                })
        } else {
            self.cleanup_intent.clone()
        };
        let issued = match self.account.begin_logout() {
            Ok(issued) => issued,
            Err(error) => {
                if error == AccountError::Exhausted {
                    self.block_exhaustion();
                }
                return Err(error);
            }
        };
        self.cleanup_intent = cleanup_intent;
        self.clear_private();
        self.login_refusal = None;
        self.phase = SessionPhase::LogoutPending;
        self.cleanup_in_flight = true;
        Ok(issued)
    }
    fn begin_expiry_cleanup(&mut self) -> Result<AccountGeneration, AccountError> {
        let issued = self.begin_logout()?;
        self.cleanup_intent = CleanupIntent::StaleOnly;
        Ok(issued)
    }
    pub fn finish_logout(
        &mut self,
        issued: AccountGeneration,
        result: Result<(), AccountFailure>,
    ) -> bool {
        if issued != self.account.generation() || self.phase != SessionPhase::LogoutPending {
            return false;
        }
        self.cleanup_in_flight = false;
        if result == Err(AccountFailure::SessionChanged) {
            if self.account.observe_session_change(issued) {
                self.phase = SessionPhase::Failed(AccountFailure::SessionChanged);
            }
            return false;
        }
        if result.is_err() || !self.account.finish_logout(issued) {
            return false;
        }
        self.phase = SessionPhase::Guest;
        true
    }
    pub fn begin_library(&mut self, section: Section, restart: bool) -> Option<LibraryRequest> {
        if self.phase != SessionPhase::SignedIn {
            return None;
        }
        let fence = self.account.fence().ok()?;
        let page = self.library.as_mut()?.begin(section, restart)?;
        Some(LibraryRequest { fence, page })
    }
    pub fn finish_page(
        &mut self,
        request: &LibraryRequest,
        result: Result<LibraryPage, AccountFailure>,
    ) -> Result<LibraryCompletion, AccountError> {
        if !self.account.accepts(&request.fence) {
            return Ok(LibraryCompletion::Ignored);
        }
        let Some(library) = self.library.as_mut() else {
            return Ok(LibraryCompletion::Ignored);
        };
        let failure = result.as_ref().err().copied();
        let page_snapshot = result.as_ref().ok().cloned();
        if !library.finish(&request.page, result) {
            return Ok(LibraryCompletion::Ignored);
        }
        if library.phase(request.page.section) == super::library::PagePhase::Ready {
            if let (Some(page), Some(sync)) = (page_snapshot, self.sync.as_mut()) {
                match page {
                    LibraryPage::Continue(body) => {
                        for item in body.items {
                            if let Some(presentation) = item.presentation {
                                sync.remember_audio_presentation(presentation);
                            }
                            sync.observe_progress_record(item.progress);
                        }
                    }
                    LibraryPage::Saved(body) => {
                        for item in body.items {
                            if let Some(presentation) = item.presentation {
                                sync.remember_work_presentation(presentation);
                            }
                            sync.observe_saved_record(item.saved);
                        }
                    }
                    LibraryPage::History(body) => {
                        sync.observe_history_state(body.state);
                        for item in body.items {
                            if let Some(presentation) = item.presentation {
                                sync.remember_audio_presentation(presentation);
                            }
                            if let Some(progress) = item.progress {
                                sync.observe_progress_record(progress);
                            }
                        }
                    }
                }
                sync.sync_projection(library);
            }
        }
        match failure {
            Some(AccountFailure::SignedOut) => {
                Ok(LibraryCompletion::Cleanup(self.begin_expiry_cleanup()?))
            }
            Some(AccountFailure::SessionChanged) => {
                self.clear_private();
                self.phase = SessionPhase::Failed(AccountFailure::SessionChanged);
                Ok(LibraryCompletion::Applied)
            }
            _ => Ok(LibraryCompletion::Applied),
        }
    }

    /// Admit the page before releasing a start that was waiting for its initial history epoch.
    pub fn finish_page_with_followup(
        &mut self,
        request: &LibraryRequest,
        result: Result<LibraryPage, AccountFailure>,
        next_operation_id: impl FnOnce() -> String,
    ) -> Result<(LibraryCompletion, Option<PendingMutation>), AccountError> {
        let completion = self.finish_page(request, result)?;
        let pending = if completion == LibraryCompletion::Applied
            && request.page.section == Section::History
            && self
                .library()
                .is_some_and(|library| library.phase(Section::History) == PagePhase::Ready)
        {
            self.with_completion(request.fence(), |sync, _| {
                sync.flush_queued_history_start(next_operation_id, None)
            })
            .ok()
            .and_then(|admitted| admitted.value)
        } else {
            None
        };
        Ok((completion, pending))
    }
}

/// The current account admitted a capability. Effects are data for the shell after its guard drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admitted<T> {
    pub fence: RequestFence,
    pub value: T,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentRefusal {
    SignedOut,
    RestoreBlocked,
    NoSync,
    StaleFence,
}

pub type SyncResult<T> = Result<Admitted<T>, IntentRefusal>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplicitProgress {
    MarkCompleted,
    Restart,
}

impl AccountViewState {
    // Only the owner can borrow sync/projection mutably. Admission happens before invoking
    // any generator or touching presentation metadata; reconciliation happens once afterwards.
    fn with_intent<T>(
        &mut self,
        apply: impl FnOnce(&mut LibrarySyncState, Option<&mut LibraryProjection>) -> T,
    ) -> SyncResult<T> {
        let fence = self.fence().ok_or(IntentRefusal::SignedOut)?;
        if self.journal_restore_blocked {
            return Err(IntentRefusal::RestoreBlocked);
        }
        self.with_completion(&fence, apply)
    }

    fn with_playback_intent<T>(
        &mut self,
        apply: impl FnOnce(&mut LibrarySyncState) -> (bool, T),
    ) -> (bool, SyncResult<T>) {
        let Some(fence) = self.fence() else {
            return (false, Err(IntentRefusal::SignedOut));
        };
        if self.journal_restore_blocked {
            return (false, Err(IntentRefusal::RestoreBlocked));
        }
        let Some(sync) = self.sync.as_mut() else {
            return (false, Err(IntentRefusal::NoSync));
        };
        let (mut published, value) = apply(sync);
        if let Some(library) = self.library.as_mut() {
            published |= sync.sync_projection(library);
        }
        (published, Ok(Admitted { fence, value }))
    }

    // Completions may reconcile already admitted operations while restore is blocked. They
    // never mint a new user intent under that condition; this preserves receipt recovery.
    fn with_completion<T>(
        &mut self,
        fence: &RequestFence,
        apply: impl FnOnce(&mut LibrarySyncState, Option<&mut LibraryProjection>) -> T,
    ) -> SyncResult<T> {
        if !self.accepts(fence) {
            return Err(IntentRefusal::StaleFence);
        }
        let sync = self.sync.as_mut().ok_or(IntentRefusal::NoSync)?;
        let value = apply(sync, self.library.as_mut());
        if let Some(library) = self.library.as_mut() {
            sync.sync_projection(library);
        }
        Ok(Admitted {
            fence: fence.clone(),
            value,
        })
    }

    #[cfg(test)]
    pub(crate) fn seed_sync<T>(
        &mut self,
        seed: impl FnOnce(&mut LibrarySyncState, Option<&mut LibraryProjection>) -> T,
    ) -> Option<(RequestFence, T)> {
        self.with_intent(seed)
            .ok()
            .map(|admitted| (admitted.fence, admitted.value))
    }
}

// User intents. Presentation metadata is admitted with the intent, never before its fence.
impl AccountViewState {
    pub fn request_saved(
        &mut self,
        work: &str,
        desired: bool,
        presentation: Option<WorkPresentation>,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Option<PendingMutation>> {
        self.with_intent(|sync, _| {
            if let Some(presentation) = presentation {
                sync.remember_work_presentation(presentation);
            }
            sync.request_saved(work, desired, mint(), reported_at_ms)
        })
    }
    pub fn retry_saved(
        &mut self,
        work: &str,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Option<PendingMutation>> {
        self.with_intent(|sync, _| sync.retry_saved(work, mint, reported_at_ms))
    }
    pub fn request_progress(
        &mut self,
        audio: AudioIdentity,
        intent: ExplicitProgress,
        presentation: Option<AudioPresentation>,
        now_ms: i64,
        mint: impl FnOnce() -> String,
    ) -> SyncResult<Option<PendingMutation>> {
        self.with_intent(|sync, _| {
            if let Some(presentation) = presentation {
                sync.remember_audio_presentation(presentation);
            }
            let (position, kind) = match intent {
                ExplicitProgress::MarkCompleted => (audio.duration_ms, ProgressKind::MarkCompleted),
                ExplicitProgress::Restart => (0, ProgressKind::Restart),
            };
            sync.record_progress_intent(audio, position, kind, true, now_ms, mint)
        })
    }
    pub fn resolve_progress_conflict(
        &mut self,
        publication: &str,
        keep_local: bool,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Option<(PendingMutation, u64)>> {
        self.with_intent(|sync, _| {
            sync.resolve_progress_conflict(publication, keep_local, mint, reported_at_ms)
        })
    }
    pub fn retry_progress(
        &mut self,
        publication: &str,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Option<PendingMutation>> {
        self.with_intent(|sync, _| sync.retry_progress(publication, mint, reported_at_ms))
    }
    pub fn prompt_clear_history(&mut self) -> SyncResult<Option<HistoryState>> {
        self.with_intent(|sync, _| sync.prompt_clear_history())
    }
    pub fn cancel_clear_history(&mut self) -> SyncResult<()> {
        self.with_intent(|sync, _| sync.cancel_clear_history())
    }
    pub fn confirm_clear_history(
        &mut self,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Option<PendingMutation>> {
        self.with_intent(|sync, _| sync.confirm_clear_history(mint, reported_at_ms))
    }
    pub fn retry_history(
        &mut self,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Option<PendingMutation>> {
        self.with_intent(|sync, _| sync.retry_history(mint, reported_at_ms))
    }
    pub fn begin_resume(
        &mut self,
        publication: String,
        status: ResumeStatus,
        presentation: Option<AudioPresentation>,
    ) -> SyncResult<Option<ResumeTicket>> {
        self.with_intent(|sync, _| {
            if let Some(presentation) = presentation {
                sync.remember_audio_presentation(presentation);
            }
            match status {
                ResumeStatus::ExactAvailable => sync.begin_resume(publication),
                ResumeStatus::ReleaseReplaced | ResumeStatus::AccessUnavailable => {
                    let reason = match status {
                        ResumeStatus::ReleaseReplaced => ResumeRefusal::ReleaseReplaced,
                        _ => ResumeRefusal::AccessUnavailable,
                    };
                    sync.set_resume_attempt(ResumeAttemptState::Refused {
                        publication_id: publication,
                        reason,
                    });
                    None
                }
            }
        })
    }
    pub fn reset_resume(&mut self) -> SyncResult<()> {
        self.with_intent(|sync, _| sync.set_resume_attempt(ResumeAttemptState::Idle))
    }
    pub fn ensure_saved_read(
        &mut self,
        work: &str,
        presentation: Option<WorkPresentation>,
    ) -> SyncResult<bool> {
        self.with_intent(|sync, _| {
            if let Some(presentation) = presentation {
                sync.remember_work_presentation(presentation);
            }
            if sync
                .saved_work(work)
                .is_some_and(|entry| entry.record.is_some() || entry.loading)
            {
                false
            } else {
                sync.begin_saved_read(work)
            }
        })
    }
    pub fn ensure_progress_read(
        &mut self,
        publication: &str,
        presentation: Option<AudioPresentation>,
    ) -> SyncResult<bool> {
        self.with_intent(|sync, _| {
            if let Some(presentation) = presentation {
                sync.remember_audio_presentation(presentation);
            }
            if sync
                .progress_for(publication)
                .is_some_and(|entry| entry.record.is_some() || entry.point_read_in_flight)
            {
                false
            } else {
                sync.begin_progress_read(publication)
            }
        })
    }
}

// Read and resume completions retain the account fence and request identity.
impl AccountViewState {
    pub fn reread_saved(&mut self, fence: &RequestFence, work: &str) -> SyncResult<bool> {
        self.with_completion(fence, |sync, _| sync.begin_saved_read(work))
    }
    pub fn reread_progress(&mut self, fence: &RequestFence, publication: &str) -> SyncResult<bool> {
        self.with_completion(fence, |sync, _| sync.begin_progress_read(publication))
    }
    pub fn finish_saved_read(
        &mut self,
        fence: &RequestFence,
        record: SavedRecord,
    ) -> SyncResult<bool> {
        self.with_completion(fence, |sync, _| sync.observe_saved_record(record))
    }
    pub fn fail_saved_read(&mut self, fence: &RequestFence, work: &str) -> SyncResult<()> {
        self.with_completion(fence, |sync, _| sync.fail_saved_read(work))
    }
    pub fn finish_progress_read(
        &mut self,
        fence: &RequestFence,
        publication: &str,
        record: Option<ProgressRecord>,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Option<PendingMutation>> {
        self.with_completion(fence, |sync, _| {
            sync.finish_progress_read(publication, record, mint, reported_at_ms)
        })
    }
    pub fn fail_progress_read(
        &mut self,
        fence: &RequestFence,
        publication: &str,
    ) -> SyncResult<()> {
        self.with_completion(fence, |sync, _| sync.fail_progress_read(publication))
    }
    pub fn finish_resume(
        &mut self,
        fence: &RequestFence,
        ticket: ResumeTicket,
        result: Result<u32, ResumeRefusal>,
    ) -> SyncResult<bool> {
        self.with_completion(fence, |sync, _| {
            if !sync.accepts_resume(ticket) {
                return false;
            }
            let ResumeAttemptState::Preparing { publication_id } = sync.resume_attempt() else {
                return false;
            };
            let publication_id = publication_id.clone();
            sync.set_resume_attempt(match result {
                Ok(position_ms) => ResumeAttemptState::Restored {
                    publication_id,
                    position_ms,
                },
                Err(reason) => ResumeAttemptState::Refused {
                    publication_id,
                    reason,
                },
            });
            true
        })
    }
}

// Command completions: the pending operation remains owned by LibrarySyncState.
impl AccountViewState {
    pub fn apply_receipt(
        &mut self,
        fence: &RequestFence,
        operation: &str,
        receipt: Receipt,
        via_lookup: bool,
        mint: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> SyncResult<Result<AppliedReceipt, CommandError>> {
        self.with_completion(fence, |sync, library| {
            let result = sync.apply_receipt(operation, receipt, via_lookup, mint, reported_at_ms);
            if let (Ok(applied), Some(library)) = (&result, library) {
                if let Some((audio, started_at_ms)) = &applied.accepted_history_start {
                    record_started_in_projection(library, sync, audio, *started_at_ms);
                }
            }
            result
        })
    }
    pub fn record_unknown_outcome(
        &mut self,
        fence: &RequestFence,
        operation: &str,
    ) -> SyncResult<bool> {
        self.with_completion(fence, |sync, _| sync.record_unknown_outcome(operation))
    }
    pub fn record_command_error(
        &mut self,
        fence: &RequestFence,
        operation: &str,
        error: CommandError,
        rotate_device: impl FnOnce() -> String,
    ) -> SyncResult<bool> {
        self.with_completion(fence, |sync, _| {
            sync.record_command_error(operation, error, rotate_device)
        })
    }
}

// Playback observations carry acknowledged engine facts. No aggregate snapshot is cloned.
impl AccountViewState {
    pub fn observe_playback(
        &mut self,
        observation: PlaybackObservation,
        presentation: Option<AudioPresentation>,
        now_ms: i64,
        mint_session: impl FnOnce() -> String,
        mint_operation: impl FnOnce() -> String,
    ) -> SyncResult<Option<PendingMutation>> {
        self.observe_playback_publication(
            observation,
            presentation,
            now_ms,
            mint_session,
            mint_operation,
        )
        .1
    }

    pub(super) fn observe_playback_publication(
        &mut self,
        observation: PlaybackObservation,
        presentation: Option<AudioPresentation>,
        now_ms: i64,
        mint_session: impl FnOnce() -> String,
        mint_operation: impl FnOnce() -> String,
    ) -> (bool, SyncResult<Option<PendingMutation>>) {
        self.with_playback_intent(|sync| {
            let before = super::library_sync::publication::PlaybackPublication::new(
                sync,
                observation,
                presentation.as_ref(),
            );
            if let Some(presentation) = presentation {
                sync.remember_audio_presentation(presentation);
            }
            let value = (|| match observation {
                PlaybackObservation::SessionReset => {
                    sync.reset_engine_session();
                    None
                }
                PlaybackObservation::EngineStarted { identity, source } => {
                    sync.record_engine_started(identity, source, mint_session, mint_operation, None)
                }
                PlaybackObservation::PositionTick {
                    source,
                    position_ms,
                }
                | PlaybackObservation::SeekConfirmed {
                    source,
                    position_ms,
                }
                | PlaybackObservation::FlushCheckpoint {
                    source,
                    position_ms,
                } => {
                    let audio = audio_identity_from_source(source)?;
                    let (kind, flush) = match observation {
                        PlaybackObservation::SeekConfirmed { .. } => {
                            (ProgressKind::ExplicitSeek, true)
                        }
                        PlaybackObservation::FlushCheckpoint { .. } => {
                            (ProgressKind::Checkpoint, true)
                        }
                        _ => (ProgressKind::Checkpoint, false),
                    };
                    sync.record_progress_intent(
                        audio,
                        u64::from(position_ms),
                        kind,
                        flush,
                        now_ms,
                        mint_operation,
                    )
                }
            })();
            let after = before.after(sync);
            (before != after, value)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryRequest {
    fence: RequestFence,
    pub page: PageRequest,
}
impl LibraryRequest {
    pub fn session(&self) -> &str {
        self.fence.session()
    }
    pub fn fence(&self) -> &RequestFence {
        &self.fence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(actor: &str, session: u128) -> AccountScope {
        AccountScope::from_session(actor.into(), uuid::Uuid::from_u128(session).to_string())
            .unwrap()
    }
    fn authenticate(s: &mut AccountViewState, actor: &str, session: u128) {
        let issued = s.begin_refresh().unwrap();
        assert!(matches!(
            s.finish_session(issued, Ok(scope(actor, session)), || uuid::Uuid::from_u128(
                100
            )
            .to_string()),
            Ok(SessionCompletion::Authenticated(_))
        ));
    }
    fn page() -> LibraryPage {
        crate::live::library::decode_page(Section::Saved, include_bytes!("../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_saved.json")).unwrap()
    }
    #[test]
    fn old_account_storage_failures_cannot_block_a_new_account_or_overwrite_its_journal() {
        let mut state = AccountViewState::default();
        authenticate(&mut state, "a", 1);
        let old = state.fence().unwrap();
        state.record_journal_restore(&old, Err(JournalStorageFault::Corrupt));
        assert!(!state.can_persist_journal());
        authenticate(&mut state, "b", 2);
        state.record_journal_restore(&old, Err(JournalStorageFault::Corrupt));
        state.record_storage(&old, Err(JournalStorageFault::Unavailable));
        assert_eq!(state.storage_fault(), None);
        assert!(state.can_persist_journal());
        assert_eq!(state.actor(), Some("b"));
    }

    #[test]
    fn bootstrap_signed_out_requires_acknowledged_cleanup_before_authorization() {
        let mut s = AccountViewState::default();
        let issued = s.begin_refresh().unwrap();
        let SessionCompletion::Cleanup(cleanup) = s
            .finish_session(issued, Err(AccountFailure::SignedOut), || {
                uuid::Uuid::from_u128(100).to_string()
            })
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(s.phase(), SessionPhase::LogoutPending);
        assert!(!s.can_authenticate());
        assert!(!s.finish_logout(cleanup, Err(AccountFailure::Unavailable)));
        assert!(s.can_retry_logout());
        assert!(!s.can_authenticate());
        assert!(s.finish_logout(cleanup, Ok(())));
        assert!(s.can_authenticate());
    }
    #[test]
    fn logout_hides_every_projection_and_late_pages_cannot_return_during_or_after_account_switch() {
        let mut s = AccountViewState::default();
        authenticate(&mut s, "a", 1);
        let old = s.begin_library(Section::Saved, false).unwrap();
        s.finish_page(&old, Ok(page())).unwrap();
        let cleanup = s.begin_logout().unwrap();
        assert!(s.library().is_none());
        assert_eq!(
            s.finish_page(&old, Ok(page())),
            Ok(LibraryCompletion::Ignored)
        );
        assert!(s.finish_logout(cleanup, Ok(())));
        authenticate(&mut s, "b", 2);
        for result in [Ok(page()), Err(AccountFailure::SignedOut)] {
            assert_eq!(s.finish_page(&old, result), Ok(LibraryCompletion::Ignored));
            assert_eq!(s.actor(), Some("b"));
            assert!(s.library().unwrap().saved.is_empty());
        }
    }
    #[test]
    fn stale_session_success_and_failure_cannot_reenter_or_clear_a_newer_account() {
        let mut s = AccountViewState::default();
        let old = s.begin_refresh().unwrap();
        let cleanup = s.begin_logout().unwrap();
        s.finish_logout(cleanup, Ok(()));
        authenticate(&mut s, "b", 2);
        for result in [Ok(scope("a", 1)), Err(AccountFailure::SignedOut)] {
            assert_eq!(
                s.finish_session(old, result, || uuid::Uuid::from_u128(100).to_string()),
                Ok(SessionCompletion::Ignored)
            );
            assert_eq!(s.actor(), Some("b"));
            assert_eq!(s.phase(), SessionPhase::SignedIn);
        }
    }
    #[test]
    fn current_expiry_clears_all_lists_and_retries_keep_the_cleanup_generation_barrier() {
        let mut s = AccountViewState::default();
        authenticate(&mut s, "a", 1);
        let request = s.begin_library(Section::Saved, false).unwrap();
        let LibraryCompletion::Cleanup(cleanup) = s
            .finish_page(&request, Err(AccountFailure::SignedOut))
            .unwrap()
        else {
            panic!()
        };
        assert!(s.library().is_none());
        assert!(!s.can_authenticate());
        let retry = s.begin_logout().unwrap();
        assert!(!s.finish_logout(cleanup, Ok(())));
        assert!(s.finish_logout(retry, Ok(())));
        assert!(s.can_authenticate());
    }
    #[test]
    fn a_peer_cookie_observation_supersedes_a_pending_lookup_without_adopting_its_account() {
        let mut s = AccountViewState::default();
        let old = s.begin_refresh().unwrap();
        let current = s.begin_cookie_observation().unwrap();
        assert_ne!(old, current);
        assert_eq!(s.phase(), SessionPhase::Checking);
        assert!(s.actor().is_none());
        assert!(s.library().is_none());
        assert_eq!(
            s.finish_session(old, Ok(scope("a", 1)), || uuid::Uuid::from_u128(100)
                .to_string()),
            Ok(SessionCompletion::Ignored)
        );
        assert_eq!(s.phase(), SessionPhase::Checking);
        assert!(matches!(
            s.finish_session(current, Ok(scope("b", 2)), || uuid::Uuid::from_u128(101)
                .to_string()),
            Ok(SessionCompletion::Authenticated(_))
        ));
        assert_eq!(s.actor(), Some("b"));
        // A peer signal cannot cancel or reopen our own outstanding logout.
        let cleanup = s.begin_logout().unwrap();
        assert_eq!(
            s.begin_cookie_observation(),
            Err(AccountError::LogoutPending)
        );
        assert!(s.finish_logout(cleanup, Ok(())));
    }

    #[test]
    fn duplicate_session_refresh_is_blocked_but_new_refresh_fences_pending_library_pages() {
        let mut s = AccountViewState::default();
        let issued = s.begin_refresh().unwrap();
        assert_eq!(s.begin_refresh(), Err(AccountError::RequestPending));
        s.finish_session(issued, Ok(scope("a", 1)), || {
            uuid::Uuid::from_u128(100).to_string()
        })
        .unwrap();
        let old = s.begin_library(Section::Saved, false).unwrap();
        s.begin_refresh().unwrap();
        assert!(s.library().is_none());
        assert_eq!(
            s.finish_page(&old, Ok(page())),
            Ok(LibraryCompletion::Ignored)
        );
    }
    #[test]
    fn session_rotation_hides_all_lists_and_late_a_401_cannot_clear_b() {
        let mut s = AccountViewState::default();
        authenticate(&mut s, "a", 1);
        let old = s.begin_library(Section::Saved, false).unwrap();
        s.finish_page(&old, Err(AccountFailure::SessionChanged))
            .unwrap();
        assert_eq!(
            s.phase(),
            SessionPhase::Failed(AccountFailure::SessionChanged)
        );
        assert!(s.library().is_none());
        assert!(!s.can_retry_logout());
        authenticate(&mut s, "b", 2);
        assert_eq!(
            s.finish_page(&old, Err(AccountFailure::SignedOut)),
            Ok(LibraryCompletion::Ignored)
        );
        assert_eq!(s.actor(), Some("b"));
    }
    #[test]
    fn delayed_expiry_cleanup_cannot_revoke_b_and_explicit_cleanup_stays_pinned_across_retries() {
        let mut s = AccountViewState::default();
        authenticate(&mut s, "a", 1);
        let request = s.begin_library(Section::Saved, false).unwrap();
        let LibraryCompletion::Cleanup(cleanup) = s
            .finish_page(&request, Err(AccountFailure::SignedOut))
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(s.cleanup_intent(), &CleanupIntent::StaleOnly);
        assert!(!s.finish_logout(cleanup, Err(AccountFailure::SessionChanged)));
        assert!(!s.can_authenticate());
        authenticate(&mut s, "b", 2);
        let explicit = s.begin_logout().unwrap();
        let expected = CleanupIntent::ExpectedSession(uuid::Uuid::from_u128(2).to_string());
        assert_eq!(s.cleanup_intent(), &expected);
        assert!(!s.finish_logout(explicit, Err(AccountFailure::Unavailable)));
        s.begin_logout().unwrap();
        assert_eq!(s.cleanup_intent(), &expected);
    }
    #[test]
    fn initial_history_page_releases_one_waiting_engine_start_and_ignores_replays() {
        use super::super::library_sync::MutationPayload;
        use crate::contract::library::{HistoryState, PresentedHistoryPage};
        use crate::core::{
            playback::{CallbackIdentity, Source},
            public_ids::PublicWorkId,
            public_track::{PinnedRelease, ReleaseRequest},
        };

        let manifest: crate::contract::discovery::PublicationManifest =
            serde_json::from_str(include_str!(
                "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/manifest.json"
            ))
            .unwrap();
        let release = PinnedRelease::admit(
            &manifest,
            ReleaseRequest::parse(
                PublicWorkId::parse(&manifest.work_id).unwrap(),
                &manifest.unit_id,
                &manifest.publication_id,
            )
            .unwrap(),
        )
        .unwrap();
        let source = Source::public(release.track(), release.duration_ms()).unwrap();
        let identity = CallbackIdentity {
            generation: 1,
            track: source.track(),
        };
        let mut state = AccountViewState::default();
        authenticate(&mut state, "listener-a", 1);
        let request = state.begin_library(Section::History, false).unwrap();
        let (_, pending) = state
            .seed_sync(|sync, _| {
                sync.record_engine_started(
                    identity,
                    source,
                    || uuid::Uuid::from_u128(50).to_string(),
                    || uuid::Uuid::from_u128(51).to_string(),
                    None,
                )
            })
            .unwrap();
        assert!(pending.is_none());
        let failed = state
            .finish_page_with_followup(&request, Err(AccountFailure::Unavailable), || {
                uuid::Uuid::from_u128(52).to_string()
            })
            .unwrap();
        assert_eq!(failed, (LibraryCompletion::Applied, None));
        let request = state.begin_library(Section::History, false).unwrap();
        let page = LibraryPage::History(PresentedHistoryPage {
            state: HistoryState {
                revision: 2,
                epoch: 1,
            },
            items: Vec::new(),
            next_cursor: None,
        });
        let (completion, pending) = state
            .finish_page_with_followup(&request, Ok(page.clone()), || {
                uuid::Uuid::from_u128(52).to_string()
            })
            .unwrap();
        assert_eq!(completion, LibraryCompletion::Applied);
        let pending = pending.expect("observed history epoch must release the acknowledged start");
        let MutationPayload::HistoryStart { write } = pending.payload else {
            panic!("expected history start");
        };
        assert_eq!(
            write.engine_session_id,
            uuid::Uuid::from_u128(50).to_string()
        );
        assert_eq!(write.history_epoch, 1);
        assert_eq!(write.audio.publication_id, manifest.publication_id);
        let repeated = state
            .finish_page_with_followup(&request, Ok(page), || uuid::Uuid::from_u128(53).to_string())
            .unwrap();
        assert_eq!(repeated, (LibraryCompletion::Ignored, None));
    }

    #[test]
    fn account_switch_isolates_sync_registers_and_fences_late_command_responses() {
        let mut s = AccountViewState::default();
        authenticate(&mut s, "a", 1);
        let work_id = uuid::Uuid::from_u128(99).to_string();
        let (fence_a, pending_a) = s
            .seed_sync(|sync, _| {
                sync.request_saved(
                    &work_id,
                    true,
                    uuid::Uuid::from_u128(500).to_string(),
                    Some(1_000),
                )
            })
            .unwrap();
        assert!(pending_a.is_some());
        let journal_a = s.sync().unwrap().to_journal();

        let cleanup = s.begin_logout().unwrap();
        assert!(s.sync().is_none());
        assert_eq!(
            s.record_unknown_outcome(&fence_a, "missing"),
            Err(IntentRefusal::StaleFence)
        );
        assert!(s.finish_logout(cleanup, Ok(())));

        authenticate(&mut s, "b", 2);
        let fence_b = s.fence().unwrap();
        // A's journal is rejected for actor B.
        assert!(!s.restore_sync_journal(&fence_b, journal_a));
        // Late A command failure or receipt cannot touch B.
        assert_eq!(
            s.fail_fenced_command(&fence_a, AccountFailure::SignedOut),
            Ok(LibraryCompletion::Ignored)
        );
        assert_eq!(
            s.finish_saved_read(
                &fence_a,
                SavedRecord {
                    work_id: work_id.clone(),
                    saved: true,
                    revision: 1,
                    listener_order: 1,
                    accepted_at_ms: 2_000,
                }
            ),
            Err(IntentRefusal::StaleFence)
        );
        assert!(s.sync().unwrap().saved_work(&work_id).is_none());
    }
    #[test]
    fn ordinary_lifecycle_outcomes_and_refusals_are_characterized() {
        for failure in [
            AccountFailure::Unavailable,
            AccountFailure::Protocol,
            AccountFailure::SessionChanged,
        ] {
            let mut state = AccountViewState::default();
            let issued = state.begin_refresh().unwrap();
            let before = state.clone();
            assert_eq!(state.begin_refresh(), Err(AccountError::RequestPending));
            assert_eq!(state, before);
            assert_eq!(
                state.finish_session(issued, Err(failure), || uuid::Uuid::from_u128(100)
                    .to_string()),
                Ok(SessionCompletion::Ignored)
            );
            assert_eq!(state.phase(), SessionPhase::Failed(failure));
            assert!(state.library().is_none());
            assert!(state.sync().is_none());
        }
        for failure in [
            None,
            Some(AccountFailure::Unavailable),
            Some(AccountFailure::SessionChanged),
        ] {
            let mut state = AccountViewState::default();
            authenticate(&mut state, "a", 1);
            let fence = state.fence().unwrap();
            let before = state.clone();
            assert_eq!(
                state.fail_fenced_command(&fence, AccountFailure::Unavailable),
                Ok(LibraryCompletion::Ignored)
            );
            assert_eq!(state, before);
            let issued = state.begin_logout().unwrap();
            assert_eq!(state.phase(), SessionPhase::LogoutPending);
            assert!(state.cleanup_in_flight());
            let before = state.clone();
            assert_eq!(state.begin_refresh(), Err(AccountError::LogoutPending));
            assert_eq!(state, before);
            let success = state.finish_logout(issued, failure.map_or(Ok(()), Err));
            assert_eq!(success, failure.is_none());
            assert_eq!(
                state.phase(),
                match failure {
                    None => SessionPhase::Guest,
                    Some(AccountFailure::SessionChanged) =>
                        SessionPhase::Failed(AccountFailure::SessionChanged),
                    Some(_) => SessionPhase::LogoutPending,
                }
            );
            assert!(!state.cleanup_in_flight());
        }
    }

    #[test]
    fn exhaustion_blocks_every_invalidation_path_and_preserves_durable_journal() {
        for action in 0..4 {
            let mut state = AccountViewState::default();
            authenticate(&mut state, "a", 1);
            state.account.set_generation_for_test(u64::MAX);
            let fence = state.fence().unwrap();
            let request = state.begin_library(Section::Saved, false).unwrap();
            let work = uuid::Uuid::from_u128(99).to_string();
            state.seed_sync(|sync, _| {
                sync.request_saved(&work, true, uuid::Uuid::from_u128(500).to_string(), None)
            });
            let journal = state.sync().unwrap().to_journal();
            match action {
                0 => assert_eq!(state.begin_refresh(), Err(AccountError::Exhausted)),
                1 => assert_eq!(state.begin_logout(), Err(AccountError::Exhausted)),
                2 => assert_eq!(
                    state.fail_fenced_command(&fence, AccountFailure::SignedOut),
                    Err(AccountError::Exhausted)
                ),
                3 => assert_eq!(
                    state.finish_page(&request, Err(AccountFailure::SignedOut)),
                    Err(AccountError::Exhausted)
                ),
                _ => unreachable!(),
            }
            assert_eq!(
                state.phase(),
                SessionPhase::Failed(AccountFailure::Unavailable)
            );
            assert_eq!(state.generation(), fence_generation_max());
            assert_eq!(state.actor(), None);
            assert_eq!(state.fence(), None);
            assert!(!state.accepts(&fence));
            assert_eq!(state.library(), None);
            assert_eq!(state.sync(), None);
            assert!(!state.cleanup_in_flight());
            assert_eq!(state.cleanup_intent(), &CleanupIntent::StaleOnly);
            let blocked = state.clone();
            assert_eq!(
                state.finish_page(&request, Ok(page())),
                Ok(LibraryCompletion::Ignored)
            );
            assert_eq!(
                state.fail_fenced_command(&fence, AccountFailure::SignedOut),
                Ok(LibraryCompletion::Ignored)
            );
            assert!(!state.finish_logout(state.generation(), Ok(())));
            assert_eq!(state, blocked);
            assert!(journal.clone().restore_for_actor("b").is_none());
            let mut reloaded = AccountViewState::default();
            authenticate(&mut reloaded, "a", 2);
            assert!(reloaded.restore_sync_journal(&reloaded.fence().unwrap(), journal.clone()));
            let mut expected = journal;
            for pending in &mut expected.pending {
                pending.recovery = super::super::library_sync::RecoveryMode::ReadReceiptFirst;
            }
            assert_eq!(reloaded.sync().unwrap().to_journal(), expected);
        }
    }

    fn fence_generation_max() -> AccountGeneration {
        let mut state = AccountState::default();
        state.set_generation_for_test(u64::MAX);
        state.generation()
    }
    #[test]
    fn session_device_ids_are_injected_only_for_admitted_success() {
        for device in [100, 101] {
            let mut state = AccountViewState::default();
            let issued = state.begin_refresh().unwrap();
            let id = uuid::Uuid::from_u128(device).to_string();
            let completion = state
                .finish_session(issued, Ok(scope("a", 1)), || id.clone())
                .unwrap();
            assert_eq!(
                completion,
                SessionCompletion::Authenticated(state.fence().unwrap())
            );
            assert_eq!(state.sync().unwrap().device_id(), id);
            let before = state.clone();
            assert_eq!(
                state.finish_session(issued, Ok(scope("b", 2)), || panic!(
                    "stale completion must not mint"
                )),
                Ok(SessionCompletion::Ignored)
            );
            assert_eq!(state, before);
        }
    }

    #[test]
    fn last_generation_is_usable_once_but_exhausted_retry_and_session_lookup_stay_closed() {
        let mut state = AccountViewState::default();
        authenticate(&mut state, "a", 1);
        state.account.set_generation_for_test(u64::MAX - 1);
        let issued = state.begin_logout().unwrap();
        assert_eq!(issued, fence_generation_max());
        assert_eq!(state.begin_logout(), Err(AccountError::Exhausted));
        let before = state.clone();
        assert!(!state.finish_logout(issued, Ok(())));
        assert_eq!(state, before);
        assert_eq!(
            state.phase(),
            SessionPhase::Failed(AccountFailure::Unavailable)
        );

        for result in [Ok(scope("a", 1)), Err(AccountFailure::SignedOut)] {
            let mut state = AccountViewState::default();
            state.account.set_generation_for_test(u64::MAX - 1);
            let issued = state.begin_refresh().unwrap();
            assert_eq!(
                state.finish_session(issued, result, || panic!("exhausted lookup must not mint")),
                Err(AccountError::Exhausted)
            );
            assert_eq!(
                state.phase(),
                SessionPhase::Failed(AccountFailure::Unavailable)
            );
            assert!(state.library().is_none());
            assert!(state.sync().is_none());
            let before = state.clone();
            assert_eq!(
                state.finish_session(issued, Ok(scope("b", 2)), || panic!("late completion")),
                Ok(SessionCompletion::Ignored)
            );
            assert_eq!(state, before);
        }
    }
    #[test]
    fn non_invalidating_page_exhaustion_refuses_without_clearing_private_state() {
        for section in Section::ALL {
            let mut state = AccountViewState::default();
            authenticate(&mut state, "a", 1);
            let request = state.begin_library(Section::Saved, false).unwrap();
            state.finish_page(&request, Ok(page())).unwrap();
            state
                .library
                .as_mut()
                .unwrap()
                .exhaust_page_generation_for_test(section);
            let before = state.clone();
            assert_eq!(state.begin_library(section, true), None);
            assert_eq!(state, before);
            assert_eq!(state.phase(), SessionPhase::SignedIn);
        }
    }
}

#[cfg(test)]
#[path = "account_view/login_tests.rs"]
mod login_tests;
#[cfg(test)]
#[path = "account_view/sync_tests.rs"]
mod sync_tests;
