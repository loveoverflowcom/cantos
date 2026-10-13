//! Mounted listener account lifecycle; private projections are owned by one verified session.
use super::account::{AccountError, AccountGeneration, AccountScope, AccountState, RequestFence};
use cantos_contracts::listener_library::ResumeStatus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountFailure {
    SignedOut,
    SessionChanged,
    Unavailable,
    Protocol,
    Navigation,
    LogoutPending,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LibraryPhase {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed(AccountFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressSummary {
    pub work: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub resume: ResumeStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryProjection {
    pub saved: Vec<String>,
    pub continuing: Vec<ProgressSummary>,
    pub history: Vec<String>,
    pub has_more: bool,
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

#[derive(Clone, Default)]
pub struct AccountViewState {
    account: AccountState,
    phase: SessionPhase,
    library_phase: LibraryPhase,
    library: Option<LibraryProjection>,
    cleanup_in_flight: bool,
    cleanup_intent: CleanupIntent,
}

impl AccountViewState {
    pub fn phase(&self) -> SessionPhase {
        self.phase
    }
    pub fn library_phase(&self) -> LibraryPhase {
        self.library_phase
    }
    pub fn library(&self) -> Option<&LibraryProjection> {
        self.library.as_ref()
    }
    pub fn can_login(&self) -> bool {
        self.phase == SessionPhase::Guest && !self.account.logout_pending()
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
    pub fn navigation_failed(&mut self) {
        if self.can_login() {
            self.phase = SessionPhase::Failed(AccountFailure::Navigation);
        }
    }
    fn clear_private(&mut self) {
        self.library = None;
        self.library_phase = LibraryPhase::Idle;
    }
    pub fn begin_refresh(&mut self) -> Result<AccountGeneration, AccountError> {
        if self.account.logout_pending() {
            return Err(AccountError::LogoutPending);
        }
        if self.phase == SessionPhase::Checking || self.library_phase == LibraryPhase::Loading {
            return Err(AccountError::RequestPending);
        }
        self.account.leave()?;
        self.clear_private();
        self.phase = SessionPhase::Checking;
        Ok(self.account.generation())
    }
    pub fn finish_session(
        &mut self,
        issued: AccountGeneration,
        result: Result<AccountScope, AccountFailure>,
    ) -> Result<SessionCompletion, AccountError> {
        if issued != self.account.generation() || self.phase != SessionPhase::Checking {
            return Ok(SessionCompletion::Ignored);
        }
        match result {
            Ok(scope) => {
                self.account.finish_session(issued, scope)?;
                self.phase = SessionPhase::SignedIn;
                self.library_phase = LibraryPhase::Loading;
                Ok(SessionCompletion::Authenticated(self.account.fence()?))
            }
            Err(AccountFailure::SignedOut) => {
                Ok(SessionCompletion::Cleanup(self.begin_expiry_cleanup()?))
            }
            Err(error) => {
                self.clear_private();
                self.phase = SessionPhase::Failed(error);
                Ok(SessionCompletion::Ignored)
            }
        }
    }
    pub fn begin_logout(&mut self) -> Result<AccountGeneration, AccountError> {
        if self.phase != SessionPhase::LogoutPending {
            self.cleanup_intent = self.account.scope().map_or(CleanupIntent::StaleOnly, |scope| {
                CleanupIntent::ExpectedSession(scope.session().to_owned())
            });
        }
        let issued = self.account.begin_logout()?;
        self.clear_private();
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
    pub fn finish_library(
        &mut self,
        fence: &RequestFence,
        result: Result<LibraryProjection, AccountFailure>,
    ) -> Result<LibraryCompletion, AccountError> {
        if !self.account.accepts(fence) {
            return Ok(LibraryCompletion::Ignored);
        }
        match result {
            Ok(projection) => {
                self.library = Some(projection);
                self.library_phase = LibraryPhase::Ready;
                Ok(LibraryCompletion::Applied)
            }
            Err(AccountFailure::SignedOut) => {
                Ok(LibraryCompletion::Cleanup(self.begin_expiry_cleanup()?))
            }
            Err(error) => {
                self.library = None;
                self.library_phase = LibraryPhase::Failed(error);
                Ok(LibraryCompletion::Applied)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(actor: &str, session: u128) -> AccountScope {
        AccountScope::from_session(actor.into(), uuid::Uuid::from_u128(session).to_string())
            .unwrap()
    }

    fn projection(work: &str) -> LibraryProjection {
        LibraryProjection {
            saved: vec![work.into()],
            continuing: vec![ProgressSummary {
                work: work.into(),
                position_ms: 20_000,
                duration_ms: 60_000,
                resume: cantos_contracts::listener_library::ResumeStatus::ExactAvailable,
            }],
            history: vec![work.into()],
            has_more: false,
        }
    }

    fn authenticate(state: &mut AccountViewState, actor: &str, session: u128) -> RequestFence {
        let issued = state.begin_refresh().unwrap();
        let SessionCompletion::Authenticated(fence) = state
            .finish_session(issued, Ok(scope(actor, session)))
            .unwrap()
        else {
            panic!("current verified session must start its own library read")
        };
        fence
    }

    #[test]
    fn bootstrap_signed_out_requires_acknowledged_cookie_cleanup_before_login() {
        let mut state = AccountViewState::default();
        let issued = state.begin_refresh().unwrap();
        let SessionCompletion::Cleanup(cleanup) = state
            .finish_session(issued, Err(AccountFailure::SignedOut))
            .unwrap()
        else {
            panic!("401 must schedule cookie cleanup")
        };
        assert_eq!(state.phase(), SessionPhase::LogoutPending);
        assert!(!state.can_login());
        assert!(!state.finish_logout(cleanup, Err(AccountFailure::Unavailable)));
        assert_eq!(state.phase(), SessionPhase::LogoutPending);
        assert!(!state.can_login());
        assert!(state.finish_logout(cleanup, Ok(())));
        assert_eq!(state.phase(), SessionPhase::Guest);
        assert!(state.can_login());
    }

    #[test]
    fn logout_hides_every_private_projection_before_io_and_fences_late_library_results() {
        let mut state = AccountViewState::default();
        let old = authenticate(&mut state, "actor-a", 1);
        assert_eq!(
            state.finish_library(&old, Ok(projection("private-a"))).unwrap(),
            LibraryCompletion::Applied
        );
        let cleanup = state.begin_logout().unwrap();
        assert!(state.library().is_none());
        assert_eq!(state.library_phase(), LibraryPhase::Idle);
        assert_eq!(
            state.finish_library(&old, Ok(projection("late-a"))).unwrap(),
            LibraryCompletion::Ignored
        );
        assert_eq!(
            state.finish_library(&old, Err(AccountFailure::SignedOut)).unwrap(),
            LibraryCompletion::Ignored
        );
        assert!(state.finish_logout(cleanup, Ok(())));
        let current = authenticate(&mut state, "actor-b", 2);
        state.finish_library(&current, Ok(projection("private-b"))).unwrap();
        assert_eq!(
            state.finish_library(&old, Err(AccountFailure::SignedOut)).unwrap(),
            LibraryCompletion::Ignored
        );
        assert_eq!(state.library().unwrap().saved, ["private-b"]);
        assert_eq!(state.phase(), SessionPhase::SignedIn);
    }

    #[test]
    fn stale_session_success_and_failure_cannot_reenter_or_clear_a_newer_account() {
        let mut state = AccountViewState::default();
        let old_lookup = state.begin_refresh().unwrap();
        let cleanup = state.begin_logout().unwrap();
        assert!(state.finish_logout(cleanup, Ok(())));
        let current = authenticate(&mut state, "actor-b", 2);
        state.finish_library(&current, Ok(projection("private-b"))).unwrap();
        for result in [Ok(scope("actor-a", 1)), Err(AccountFailure::SignedOut)] {
            assert_eq!(
                state.finish_session(old_lookup, result),
                Ok(SessionCompletion::Ignored)
            );
            assert_eq!(state.library().unwrap().saved, ["private-b"]);
            assert_eq!(state.phase(), SessionPhase::SignedIn);
        }
    }

    #[test]
    fn current_expiry_clears_progress_saved_history_and_retains_logout_barrier() {
        let mut state = AccountViewState::default();
        let current = authenticate(&mut state, "actor-a", 1);
        state.finish_library(&current, Ok(projection("private-a"))).unwrap();
        let LibraryCompletion::Cleanup(cleanup) = state
            .finish_library(&current, Err(AccountFailure::SignedOut))
            .unwrap()
        else {
            panic!("current expiry must hide projections and schedule cookie cleanup")
        };
        assert!(state.library().is_none());
        assert!(!state.can_login());
        let retry = state.begin_logout().unwrap();
        assert!(!state.finish_logout(cleanup, Ok(())));
        assert!(!state.can_login());
        assert!(state.finish_logout(retry, Ok(())));
        assert!(state.can_login());
    }

    #[test]
    fn duplicate_refresh_does_not_dispatch_another_session_or_private_read_batch() {
        let mut state = AccountViewState::default();
        let issued = state.begin_refresh().unwrap();
        assert_eq!(state.begin_refresh(), Err(AccountError::RequestPending));
        let SessionCompletion::Authenticated(fence) = state.finish_session(issued, Ok(scope("actor-a", 1))).unwrap() else {
            panic!("current session must own one private read batch")
        };
        assert_eq!(state.begin_refresh(), Err(AccountError::RequestPending));
        state.finish_library(&fence, Ok(projection("private-a"))).unwrap();
        assert!(state.begin_refresh().is_ok());
        assert!(state.library().is_none());
    }

    #[test]
    fn cookie_rotation_hides_a_projection_and_rechecks_b_without_clearing_b_cookie() {
        let mut state = AccountViewState::default();
        let old = authenticate(&mut state, "actor-a", 1);
        state.finish_library(&old, Ok(projection("private-a"))).unwrap();
        assert_eq!(
            state.finish_library(&old, Err(AccountFailure::SessionChanged)).unwrap(),
            LibraryCompletion::Applied
        );
        assert!(state.library().is_none());
        assert_eq!(state.library_phase(), LibraryPhase::Failed(AccountFailure::SessionChanged));
        assert!(!state.can_retry_logout());
        let current = authenticate(&mut state, "actor-b", 2);
        state.finish_library(&current, Ok(projection("private-b"))).unwrap();
        assert_eq!(
            state.finish_library(&old, Err(AccountFailure::SignedOut)).unwrap(),
            LibraryCompletion::Ignored
        );
        assert_eq!(state.actor(), Some("actor-b"));
        assert_eq!(state.library().unwrap().saved, ["private-b"]);
    }

    #[test]
    fn delayed_401_cleanup_cannot_revoke_a_cookie_rotated_to_b_in_another_tab() {
        let mut state = AccountViewState::default();
        let old = authenticate(&mut state, "actor-a", 1);
        state.finish_library(&old, Ok(projection("private-a"))).unwrap();
        let LibraryCompletion::Cleanup(cleanup) = state.finish_library(&old, Err(AccountFailure::SignedOut)).unwrap() else {
            panic!("expiry requires guarded cleanup")
        };
        assert_eq!(state.cleanup_intent(), &CleanupIntent::StaleOnly);
        assert!(!state.finish_logout(cleanup, Err(AccountFailure::SessionChanged)));
        assert_eq!(state.phase(), SessionPhase::Failed(AccountFailure::SessionChanged));
        assert!(!state.can_login());
        let current = authenticate(&mut state, "actor-b", 2);
        state.finish_library(&current, Ok(projection("private-b"))).unwrap();
        let explicit = state.begin_logout().unwrap();
        assert_eq!(state.cleanup_intent(), &CleanupIntent::ExpectedSession(uuid::Uuid::from_u128(2).to_string()));
        assert!(!state.finish_logout(explicit, Err(AccountFailure::Unavailable)));
        state.begin_logout().unwrap();
        assert_eq!(state.cleanup_intent(), &CleanupIntent::ExpectedSession(uuid::Uuid::from_u128(2).to_string()));
    }
}
