//! A token exchange is not authenticated until the browser's owner cookie is accepted.
use cantos_contracts::{ApiError, ErrorCode, SessionResponse, StudioAuthenticationMode};

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

/// Browser presentation is fenced independently from retained authoring draft ownership.
/// Only an actual current-session response establishes a displayed actor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionState {
    epoch: u64,
    pending: bool,
    mode: Option<StudioAuthenticationMode>,
    actor: Option<String>,
}

impl SessionState {
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn mode(&self) -> Option<StudioAuthenticationMode> {
        self.mode
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
                ..self.clone()
            },
            SessionTicket(epoch),
        ))
    }

    pub fn accepts(&self, ticket: SessionTicket) -> bool {
        self.pending && self.epoch == ticket.0
    }

    pub fn configured(&self, ticket: SessionTicket, mode: StudioAuthenticationMode) -> Self {
        if !self.accepts(ticket) {
            return self.clone();
        }
        Self {
            mode: Some(mode),
            ..self.clone()
        }
    }

    pub fn settled(&self, ticket: SessionTicket, actor: Option<String>) -> Self {
        if !self.accepts(ticket) {
            return self.clone();
        }
        Self {
            pending: false,
            actor: actor.filter(|actor| !actor.is_empty()),
            ..self.clone()
        }
    }

    pub fn invalidated(&self) -> Self {
        Self {
            epoch: self.epoch.saturating_add(1),
            pending: false,
            actor: None,
            ..self.clone()
        }
    }
}

pub fn confirm_exchange(
    expected: SessionResponse,
    observed: SessionResponse,
) -> Result<SessionResponse, ApiError> {
    if !expected.actor_id.is_empty() && expected.actor_id == observed.actor_id {
        Ok(observed)
    } else {
        Err(ApiError {
            code: ErrorCode::Unauthenticated,
            current_revision: None,
            issues: vec![],
        })
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
        let mut editor = crate::editor::Editor::default().signed_in("alice".into());
        editor.script = "script".into();
        editor = editor.edit("private draft".into());
        let (sending, intent) = editor.start_save("original-operation".into()).unwrap();
        let uncertain = sending.save_failed(&intent, &unconfirmed_response());
        assert_eq!(uncertain.pending.as_ref(), Some(&intent));
        assert_eq!(uncertain.retry().unwrap().1, intent);
        assert_eq!(uncertain.draft, "private draft");
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
    fn login_acknowledgement_requires_the_same_nonempty_cookie_actor() {
        for (expected, observed, accepted) in [
            ("alice", "alice", true),
            ("alice", "bob", false),
            ("alice", "", false),
            ("", "", false),
        ] {
            let result = confirm_exchange(
                SessionResponse {
                    actor_id: expected.into(),
                },
                SessionResponse {
                    actor_id: observed.into(),
                },
            );
            if accepted {
                assert_eq!(result.unwrap().actor_id, expected);
            } else {
                assert_eq!(result.err().unwrap().code, ErrorCode::Unauthenticated);
            }
        }
    }

    #[test]
    fn delayed_session_success_failure_and_configuration_cannot_restore_account_a() {
        let initial = SessionState::default();
        let (checking, a) = initial.begin().unwrap();
        let checking = checking.configured(a, StudioAuthenticationMode::Oidc);
        let signed_in = checking.settled(a, Some("alice".into()));
        let (switching, b) = signed_in.begin().unwrap();
        assert_eq!(switching.actor(), None);
        assert_eq!(switching.settled(a, Some("alice".into())), switching);
        assert_eq!(switching.settled(a, None), switching);
        assert_eq!(
            switching.configured(a, StudioAuthenticationMode::Development),
            switching
        );
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
        let mut editor = crate::editor::Editor::default().signed_in("alice".into());
        editor.script = "script".into();
        editor = editor.edit("original author text".into());
        let (saving, _) = editor.start_save("operation".into()).unwrap();
        let ambiguous = saving.failed(&ApiError {
            code: ErrorCode::Unavailable,
            current_revision: None,
            issues: vec![],
        });
        let signed_out = ambiguous.signed_in(String::new());
        assert_eq!(signed_out.draft, "original author text");
        assert_eq!(signed_out.pending, ambiguous.pending);
        assert!(!signed_out.can_edit());
        let account_b = signed_out.signed_in("bob".into());
        assert!(!account_b.can_edit());
        assert!(account_b.retry().is_none());
        let account_a = account_b.signed_in("alice".into());
        assert!(account_a.can_edit());
        assert_eq!(account_a.retry().unwrap().1, ambiguous.pending.unwrap());
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
        assert_eq!(account_b.retained_owner(&editor.actor, true), "alice");
        assert!(!account_b.projects_owner(&editor.actor));
        let (checking, ticket) = account_b.begin().unwrap();
        assert!(!checking.projects_owner(&editor.actor));
        let restored = checking.settled(ticket, Some("alice".into()));
        let editor = editor.signed_in(restored.retained_owner(&editor.actor, true));
        assert!(restored.projects_owner(&editor.actor));
        assert_eq!(editor.draft, "private A draft");
    }
}
