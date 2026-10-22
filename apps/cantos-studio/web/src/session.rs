//! A sign-in is not authenticated until the API admits the browser's owner cookie session.
use cantos_contracts::{ApiError, ErrorCode, FieldIssue};
use cantos_web_auth::Failure;

/// A noncredential server correlation UUID, captured immutably for one HTTP attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionFence([u8; 36]);

impl SessionFence {
    pub fn new(value: &str) -> Option<Self> {
        if !crate::adaptation::is_run_id(value) {
            return None;
        }
        Some(Self(value.as_bytes().try_into().ok()?))
    }
    pub fn header(self) -> String {
        self.0.into_iter().map(char::from).collect()
    }
    pub fn accepts(self, value: Option<&str>) -> bool {
        value.and_then(Self::new) == Some(self)
    }
}

/// An authentication denial may invalidate only the presentation that launched it. The local
/// observation epoch also distinguishes repeated observations of the same server session.
pub fn current_attempt(
    expected: Option<SessionFence>,
    expected_epoch: u64,
    current: Option<SessionFence>,
    current_epoch: u64,
) -> bool {
    expected.is_some() && expected == current && expected_epoch == current_epoch
}

pub fn unauthenticated() -> ApiError {
    ApiError {
        code: ErrorCode::Unauthenticated,
        current_revision: None,
        issues: vec![],
    }
}

/// The browser's session is no longer the one the request was made for (another tab, an
/// account switch, a refused account): an observed denial that re-reads the session.
pub fn session_changed() -> ApiError {
    let mut error = unauthenticated();
    error.issues.push(FieldIssue {
        path: "session".into(),
        rule: "session_changed".into(),
    });
    error
}

pub fn is_session_changed(error: &ApiError) -> bool {
    error.code == ErrorCode::Unauthenticated
        && error
            .issues
            .iter()
            .any(|issue| issue.path == "session" && issue.rule == "session_changed")
}

fn coded(code: ErrorCode) -> ApiError {
    ApiError {
        code,
        current_revision: None,
        issues: vec![],
    }
}

/// The Studio projection of an identity failure. Unknown effects stay `Unavailable` (never
/// "signed out"); a 403 keeps its own code so the entry can say this account may not access
/// Studio (it is never read as a verified password: an Origin refusal is also a 403); a missing
/// sign-in capability (404) is explicit and has no fallback. A 429 hold stays `Unavailable` here
/// because `ApiError` is a shared contract: its wait is read from the owner (`api::rate_wait`).
pub fn failure_error(failure: Failure) -> ApiError {
    match failure {
        Failure::SignedOut | Failure::InvalidCredentials => unauthenticated(),
        Failure::AccountMismatch => session_changed(),
        Failure::Forbidden => coded(ErrorCode::Forbidden),
        Failure::NotFound => coded(ErrorCode::NotFound),
        Failure::Invalid => coded(ErrorCode::InvalidRequest),
        // A fence or intent that changed under a request: an unknown outcome for a write.
        Failure::Changed => unconfirmed_response(),
        Failure::RateLimited(_)
        | Failure::Unavailable
        | Failure::Protocol
        | Failure::CleanupPending => coded(ErrorCode::Unavailable),
    }
}

/// The API answers 409 with `unauthenticated` when the request's session fence names a session
/// the cookie no longer holds (another tab changed account). A business conflict (stale
/// revision, reused operation) has its own code and never matches.
pub fn is_fence_mismatch(status: u16, error: &ApiError) -> bool {
    status == 409 && error.code == ErrorCode::Unauthenticated
}

/// A successful mutation whose owner correlation cannot be confirmed has an unknown outcome.
/// Preserve the operation snapshot for receipt reconciliation while hiding private projection.
pub fn unconfirmed_response() -> ApiError {
    ApiError {
        code: ErrorCode::Unavailable,
        current_revision: None,
        issues: vec![],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionTicket(u64);

/// An authentication action the shell may start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthIntent {
    SignIn,
    Refresh,
    Reauthenticate,
    Logout,
}

/// Whether `intent` may start now. Sign-in, refresh and reauthentication wait for a settled
/// session and for in-flight work (`busy`), so they never race a save, upload or import.
/// Logout never waits for work: closing local authority is a synchronous fence, the draft and
/// every unresolved operation stay with their owner, and a late reply cannot reopen the session.
/// It only needs an account to leave.
pub fn admits(intent: AuthIntent, session: &SessionState, busy: bool) -> bool {
    match intent {
        AuthIntent::Logout => !session.pending() && session.actor().is_some(),
        AuthIntent::SignIn | AuthIntent::Refresh | AuthIntent::Reauthenticate => {
            !session.pending() && !busy
        }
    }
}

/// Browser presentation is fenced independently from retained authoring draft ownership.
/// Only an actual current-session response establishes a displayed actor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionState {
    epoch: u64,
    pending: bool,
    actor: Option<String>,
}

impl SessionState {
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn actor(&self) -> Option<&str> {
        self.actor.as_deref()
    }

    /// Authentication presentation and the owner of retained browser drafts are separate facts.
    /// A failed/unsigned observation never destroys the owner's draft. A different account may
    /// adopt only a clean workspace; otherwise the shell hides it until its owner returns.
    pub fn retained_owner(&self, current: &str, unsaved: bool) -> String {
        match self.actor() {
            Some(actor)
                if !self.pending && (!unsaved || current.is_empty() || actor == current) =>
            {
                actor.into()
            }
            Some(_) | None => current.into(),
        }
    }

    pub fn projects_owner(&self, owner: &str) -> bool {
        !self.pending && !owner.is_empty() && self.actor() == Some(owner)
    }

    pub fn begin(&self) -> Option<(Self, SessionTicket)> {
        let epoch = self.epoch.checked_add(1)?;
        Some((
            Self {
                epoch,
                pending: true,
                actor: None,
            },
            SessionTicket(epoch),
        ))
    }

    pub fn accepts(&self, ticket: SessionTicket) -> bool {
        self.pending && self.epoch == ticket.0
    }

    pub fn settled(&self, ticket: SessionTicket, actor: Option<String>) -> Self {
        if !self.accepts(ticket) {
            return self.clone();
        }
        Self {
            epoch: self.epoch,
            pending: false,
            actor: actor.filter(|actor| !actor.is_empty()),
        }
    }

    pub fn invalidated(&self) -> Self {
        Self {
            epoch: self.epoch.saturating_add(1),
            pending: false,
            actor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_delayed_denial_cannot_hide_b_or_a_newer_observation_of_a() {
        let a = SessionFence::new("00000000-0000-4000-8000-000000000001").unwrap();
        let b = SessionFence::new("00000000-0000-4000-8000-000000000002").unwrap();
        assert!(current_attempt(Some(a), 4, Some(a), 4));
        assert!(!current_attempt(Some(a), 4, Some(b), 5));
        assert!(!current_attempt(Some(a), 4, Some(a), 5));
        assert!(!current_attempt(None, 4, Some(b), 4));
        assert!(!current_attempt(Some(a), 4, None, 4));
    }
    #[test]
    fn successful_response_without_owner_echo_retains_the_original_save_intent() {
        let editor = crate::editor::Editor::default()
            .signed_in("alice".into())
            .select_script("script".into())
            .edit("private draft".into());
        let (sending, intent) = editor.start_save("original-operation".into()).unwrap();
        let uncertain = sending.save_failed(&intent, &unconfirmed_response());
        assert_eq!(uncertain.pending(), Some(&intent));
        assert_eq!(uncertain.retry().unwrap().1, intent);
        assert_eq!(uncertain.draft(), "private draft");
    }
    #[test]
    fn request_snapshot_keeps_account_a_fence_and_rejects_b_or_missing_echo() {
        let a = SessionFence::new("00000000-0000-4000-8000-000000000001").unwrap();
        let b = SessionFence::new("00000000-0000-4000-8000-000000000002").unwrap();
        let attempt = a;
        assert!(attempt.accepts(Some(&a.header())));
        assert!(!attempt.accepts(Some(&b.header())));
        assert!(!attempt.accepts(None));
        assert!(SessionFence::new("cookie-secret").is_none());
    }
    #[test]
    fn identity_failures_map_to_distinct_studio_states_and_never_to_signed_out_by_accident() {
        for (failure, code) in [
            (Failure::SignedOut, ErrorCode::Unauthenticated),
            (Failure::InvalidCredentials, ErrorCode::Unauthenticated),
            (Failure::Forbidden, ErrorCode::Forbidden),
            (Failure::NotFound, ErrorCode::NotFound),
            (Failure::Invalid, ErrorCode::InvalidRequest),
            // Unknown effects: 503, network, a lost renewal, a pending revoke, a rate limit.
            (Failure::RateLimited(30), ErrorCode::Unavailable),
            (Failure::Unavailable, ErrorCode::Unavailable),
            (Failure::Protocol, ErrorCode::Unavailable),
            (Failure::CleanupPending, ErrorCode::Unavailable),
            (Failure::Changed, ErrorCode::Unavailable),
        ] {
            assert_eq!(failure_error(failure).code, code, "{failure:?}");
        }
        // A refused account and a fence mismatch are observed denials that re-read the session;
        // a changed lease mid-request is an unknown outcome and must not look like one.
        assert!(is_session_changed(&failure_error(Failure::AccountMismatch)));
        assert!(!is_session_changed(&failure_error(Failure::Changed)));
        assert!(!is_session_changed(&unauthenticated()));
        assert_eq!(unconfirmed_response().code, ErrorCode::Unavailable);
    }

    #[test]
    fn reload_expiry_and_outages_present_distinct_entries_and_keep_the_retained_draft() {
        use crate::entry::{presentation, Entry};
        let editor = crate::editor::Editor::default()
            .signed_in("alice".into())
            .select_script("script".into())
            .edit("private draft".into());
        for (failure, entry) in [
            // Reload without a cookie, idle or absolute expiry, a revoked or replaced session.
            (Failure::SignedOut, Entry::Login),
            // 503 or a lost renewal: unknown, so never presented as a signed-out login.
            (Failure::Unavailable, Entry::Unavailable),
            (Failure::RateLimited(5), Entry::Unavailable),
            // A 403: wrong Origin or a roster denial; the response cannot tell which.
            (Failure::Forbidden, Entry::Denied),
            // The API has no Studio sign-in at all; there is no fallback.
            (Failure::NotFound, Entry::SignInUnavailable),
        ] {
            let (checking, ticket) = SessionState::default().begin().unwrap();
            let failed = editor
                .checking_session()
                .session_failed(&failure_error(failure));
            let settled = checking.settled(ticket, None);
            assert_eq!(
                presentation(&settled, failed.actor(), failed.status()),
                entry,
                "{failure:?}"
            );
            assert_eq!(failed.draft(), "private draft", "{failure:?}");
            assert!(!settled.projects_owner("alice"));
        }
    }

    #[test]
    fn a_refused_account_never_adopts_or_exposes_the_retained_draft() {
        let editor = crate::editor::Editor::default()
            .signed_in("alice".into())
            .select_script("script".into())
            .edit("private A draft".into());
        // Bob signs in while Alice's draft is retained. The identity client refuses and revokes
        // him (AccountMismatch), so the session never settles on Bob.
        let (checking, ticket) = SessionState::default().begin().unwrap();
        let failed = editor
            .checking_session()
            .session_failed(&failure_error(Failure::AccountMismatch));
        let settled = checking.settled(ticket, None);
        assert_eq!(settled.actor(), None);
        assert_eq!(settled.retained_owner("alice", true), "alice");
        assert!(!settled.projects_owner("alice"));
        assert!(!settled.projects_owner("bob"));
        assert_eq!(failed.actor(), "alice");
        assert_eq!(failed.draft(), "private A draft");
        // A late Bob completion of the same attempt cannot settle it again.
        assert_eq!(settled.settled(ticket, Some("bob".into())), settled);
        // Only Alice returning projects her draft again.
        let (checking, ticket) = settled.begin().unwrap();
        let restored = checking.settled(ticket, Some("alice".into()));
        let editor = failed.signed_in(restored.retained_owner(failed.actor(), true));
        assert!(restored.projects_owner(editor.actor()));
        assert_eq!(editor.draft(), "private A draft");
    }

    #[test]
    fn only_a_409_unauthenticated_reply_is_a_session_fence_mismatch() {
        let error = |code| ApiError {
            code,
            current_revision: None,
            issues: vec![],
        };
        assert!(is_fence_mismatch(409, &error(ErrorCode::Unauthenticated)));
        for (status, code) in [
            (401, ErrorCode::Unauthenticated),
            (403, ErrorCode::Forbidden),
            (409, ErrorCode::StaleRevision),
            (409, ErrorCode::OperationReused),
            (409, ErrorCode::CatalogConflict),
            (503, ErrorCode::Unavailable),
        ] {
            assert!(!is_fence_mismatch(status, &error(code)), "{status}");
        }
    }

    #[test]
    fn logout_is_admitted_while_work_is_busy_but_sign_in_refresh_and_reauthentication_wait() {
        let (checking, ticket) = SessionState::default().begin().unwrap();
        let signed_in = checking.settled(ticket, Some("alice".into()));
        for intent in [
            AuthIntent::SignIn,
            AuthIntent::Refresh,
            AuthIntent::Reauthenticate,
        ] {
            assert!(admits(intent, &signed_in, false), "{intent:?}");
            assert!(
                !admits(intent, &signed_in, true),
                "{intent:?} must wait for work"
            );
        }
        assert!(admits(AuthIntent::Logout, &signed_in, true));
        assert!(admits(AuthIntent::Logout, &signed_in, false));
        // Nothing is admitted while a session check is pending, and there is nobody to log out
        // before an account is confirmed.
        let (pending, _) = signed_in.begin().unwrap();
        let guest = SessionState::default();
        for intent in [AuthIntent::Logout, AuthIntent::SignIn, AuthIntent::Refresh] {
            assert!(!admits(intent, &pending, false), "{intent:?}");
        }
        assert!(!admits(AuthIntent::Logout, &guest, false));
    }

    #[test]
    fn logout_during_a_save_hides_the_workspace_keeps_the_draft_and_fences_the_late_reply() {
        let editor = crate::editor::Editor::default()
            .signed_in("alice".into())
            .select_script("script".into())
            .edit("private A draft".into());
        let (saving, intent) = editor.start_save("operation".into()).unwrap();
        let (checking, ticket) = SessionState::default().begin().unwrap();
        let alice = checking.settled(ticket, Some("alice".into()));
        assert!(admits(AuthIntent::Logout, &alice, saving.busy()));
        // Logout closes the shell's authority at once, while the save is still in flight.
        let (logging_out, logout_ticket) = alice.begin().unwrap();
        let held = saving.checking_session();
        assert!(
            !logging_out.projects_owner("alice"),
            "the private workspace is hidden"
        );
        assert_eq!(
            held.pending(),
            Some(&intent),
            "the unresolved save keeps its identity"
        );
        // The save answers late: the owner reports an unknown outcome, never a success.
        let unknown = held.save_failed(&intent, &unconfirmed_response());
        assert_eq!(unknown.draft(), "private A draft");
        assert_eq!(unknown.pending(), Some(&intent));
        // The logout settles with nobody signed in; draft and pending save survive for A.
        let signed_out = logging_out.settled(logout_ticket, None);
        let after = unknown.session_failed(&unauthenticated());
        assert!(!signed_out.projects_owner("alice"));
        assert_eq!(after.draft(), "private A draft");
        assert_eq!(after.pending(), Some(&intent));
        assert!(after.awaits_reconciliation());
        // Account B signs in: A's draft is neither editable nor projected, and A's late
        // acknowledgement of the same operation changes nothing.
        let for_b = after.signed_in("bob".into());
        assert!(!for_b.can_edit());
        assert_eq!(for_b.draft(), "private A draft");
        let late = cantos_contracts::RevisionResponse {
            script_id: "script".into(),
            revision: 1,
            accepted_by: "alice".into(),
            accepted_at: "2026-10-10T00:00:00.000000Z".into(),
            content_digest: "content".into(),
            export_digest: "export".into(),
            script_json: "canonical".into(),
        };
        assert_eq!(for_b.saved(&intent, late), for_b);
    }

    #[test]
    fn delayed_session_success_and_failure_cannot_restore_account_a() {
        let initial = SessionState::default();
        let (checking, a) = initial.begin().unwrap();
        let signed_in = checking.settled(a, Some("alice".into()));
        let (switching, b) = signed_in.begin().unwrap();
        assert_eq!(switching.actor(), None);
        assert_eq!(switching.settled(a, Some("alice".into())), switching);
        assert_eq!(switching.settled(a, None), switching);
        let account_b = switching.settled(b, Some("bob".into()));
        let logout = account_b.invalidated();
        assert_eq!(logout.actor(), None);
        assert_eq!(logout.settled(b, Some("bob".into())), logout);
        assert_eq!(logout.settled(b, None), logout);
    }

    #[test]
    fn empty_session_response_and_duplicate_completion_never_establish_authority() {
        let (checking, ticket) = SessionState::default().begin().unwrap();
        let settled = checking.settled(ticket, Some(String::new()));
        assert_eq!(settled.actor(), None);
        assert!(!settled.pending());
        assert_eq!(settled.settled(ticket, Some("alice".into())), settled);
    }

    #[test]
    fn session_boundaries_preserve_original_draft_owner_and_ambiguous_intent() {
        let editor = crate::editor::Editor::default()
            .signed_in("alice".into())
            .select_script("script".into())
            .edit("original author text".into());
        let (saving, _) = editor.start_save("operation".into()).unwrap();
        let ambiguous = saving.failed(&ApiError {
            code: ErrorCode::Unavailable,
            current_revision: None,
            issues: vec![],
        });
        let signed_out = ambiguous.signed_in(String::new());
        assert_eq!(signed_out.draft(), "original author text");
        assert_eq!(signed_out.pending(), ambiguous.pending());
        assert!(!signed_out.can_edit());
        let account_b = signed_out.signed_in("bob".into());
        assert!(!account_b.can_edit());
        assert!(account_b.retry().is_none());
        let account_a = account_b.signed_in("alice".into());
        assert!(account_a.can_edit());
        assert_eq!(Some(&account_a.retry().unwrap().1), ambiguous.pending());
    }

    #[test]
    fn checking_failed_logout_or_account_b_cannot_discard_or_project_dirty_account_a() {
        let (checking, ticket) = SessionState::default().begin().unwrap();
        assert_eq!(checking.retained_owner("alice", true), "alice");
        assert!(!checking.projects_owner("alice"));
        let unsigned = checking.settled(ticket, None);
        assert_eq!(unsigned.retained_owner("alice", true), "alice");
        assert!(!unsigned.projects_owner("alice"));
        let (checking, ticket) = unsigned.begin().unwrap();
        let account_b = checking.settled(ticket, Some("bob".into()));
        assert_eq!(account_b.retained_owner("alice", true), "alice");
        assert!(!account_b.projects_owner("alice"));
        assert_eq!(account_b.retained_owner("alice", false), "bob");
        let (checking, ticket) = account_b.begin().unwrap();
        let account_a = checking.settled(ticket, Some("alice".into()));
        assert_eq!(account_a.retained_owner("alice", true), "alice");
        assert!(account_a.projects_owner("alice"));
    }
    #[test]
    fn explicit_dirty_owner_observation_can_restore_a_after_b_without_discarding_the_draft() {
        let editor = crate::editor::Editor::default()
            .signed_in("alice".into())
            .edit("private A draft".into());
        let (checking, ticket) = SessionState::default().begin().unwrap();
        let account_b = checking.settled(ticket, Some("bob".into()));
        assert_eq!(account_b.retained_owner(editor.actor(), true), "alice");
        assert!(!account_b.projects_owner(editor.actor()));
        let (checking, ticket) = account_b.begin().unwrap();
        assert!(!checking.projects_owner(editor.actor()));
        let restored = checking.settled(ticket, Some("alice".into()));
        let editor = editor.signed_in(restored.retained_owner(editor.actor(), true));
        assert!(restored.projects_owner(editor.actor()));
        assert_eq!(editor.draft(), "private A draft");
    }
}
