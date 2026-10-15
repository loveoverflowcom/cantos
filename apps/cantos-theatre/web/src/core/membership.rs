//! Session-only Membership interest preview. This is not an identity or payment contract.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MembershipMode {
    #[default]
    Unavailable,
    Fixture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlansOrigin {
    Intro,
    Account,
    AccountRegistered,
    Received,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MembershipScreen {
    #[default]
    Intro,
    Form,
    Received,
    AccountRegistered,
    Account,
    Plans(PlansOrigin),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PlansStatus {
    #[default]
    Unpublished,
    Loading,
    Empty,
    Unavailable,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SubmissionStatus {
    #[default]
    Idle,
    Pending,
    Failed,
    Unknown,
    Received,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldError {
    NameRequired,
    NameTooLong,
    NameInvalid,
    EmailRequired,
    EmailTooLong,
    EmailInvalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmitRefusal {
    NotOnForm,
    InvalidFields(Vec<FieldError>),
    Unavailable,
    AlreadyPending,
    AlreadyReceived,
    SequenceExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditRefusal {
    NotOnForm,
    Pending,
    OutcomeUnknown,
    AlreadyReceived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinishOutcome {
    Received,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MembershipDraft {
    pub name: String,
    pub email: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationPayload {
    name: String,
    email: String,
}

impl RegistrationPayload {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn email(&self) -> &str {
        &self.email
    }
}

/// Opaque intent plus a per-dispatch fence. Retry keeps the operation and payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionAttempt {
    operation_id: u64,
    request_id: u64,
    payload: RegistrationPayload,
}

impl SubmissionAttempt {
    pub fn operation_id(&self) -> u64 {
        self.operation_id
    }

    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    pub fn payload(&self) -> &RegistrationPayload {
        &self.payload
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MembershipState {
    mode: MembershipMode,
    screen: MembershipScreen,
    draft: MembershipDraft,
    submission: SubmissionStatus,
    field_errors: Vec<FieldError>,
    plans_status: PlansStatus,
    sequence: u64,
    intent: Option<SubmissionAttempt>,
    active_request: Option<u64>,
    receipt: Option<SubmissionAttempt>,
}

impl MembershipState {
    pub fn fixture() -> Self {
        Self {
            mode: MembershipMode::Fixture,
            ..Self::default()
        }
    }

    pub fn mode(&self) -> MembershipMode {
        self.mode
    }

    pub fn screen(&self) -> MembershipScreen {
        self.screen
    }

    pub fn draft(&self) -> &MembershipDraft {
        &self.draft
    }

    pub fn submission(&self) -> SubmissionStatus {
        self.submission
    }

    pub fn field_errors(&self) -> &[FieldError] {
        &self.field_errors
    }

    pub fn plans_status(&self) -> PlansStatus {
        self.plans_status
    }

    pub fn receipt(&self) -> Option<&SubmissionAttempt> {
        self.receipt.as_ref()
    }

    pub fn open_form(&mut self) -> bool {
        if self.screen != MembershipScreen::Intro {
            return false;
        }
        if self.receipt.is_some() {
            self.screen = MembershipScreen::Received;
            return false;
        }
        self.screen = MembershipScreen::Form;
        true
    }

    pub fn open_intro(&mut self) {
        self.fence_request();
        self.screen = MembershipScreen::Intro;
    }

    pub fn show_received(&mut self) -> bool {
        if self.receipt.is_none() {
            return false;
        }
        self.fence_request();
        self.screen = MembershipScreen::Received;
        true
    }

    pub fn set_name(&mut self, name: String) -> Result<(), EditRefusal> {
        self.can_edit()?;
        self.draft.name = name;
        self.edited();
        Ok(())
    }

    pub fn set_email(&mut self, email: String) -> Result<(), EditRefusal> {
        self.can_edit()?;
        self.draft.email = email;
        self.edited();
        Ok(())
    }

    fn can_edit(&self) -> Result<(), EditRefusal> {
        if self.screen != MembershipScreen::Form {
            return Err(EditRefusal::NotOnForm);
        }
        match self.submission {
            SubmissionStatus::Pending => Err(EditRefusal::Pending),
            SubmissionStatus::Unknown => Err(EditRefusal::OutcomeUnknown),
            SubmissionStatus::Received => Err(EditRefusal::AlreadyReceived),
            SubmissionStatus::Idle | SubmissionStatus::Failed => Ok(()),
        }
    }

    fn edited(&mut self) {
        self.field_errors.clear();
        self.submission = SubmissionStatus::Idle;
        self.intent = None;
    }

    pub fn begin_submission(&mut self) -> Result<SubmissionAttempt, SubmitRefusal> {
        if self.screen != MembershipScreen::Form {
            return Err(SubmitRefusal::NotOnForm);
        }
        match self.submission {
            SubmissionStatus::Pending => return Err(SubmitRefusal::AlreadyPending),
            SubmissionStatus::Received => return Err(SubmitRefusal::AlreadyReceived),
            SubmissionStatus::Idle | SubmissionStatus::Failed | SubmissionStatus::Unknown => {}
        }
        self.field_errors = validate(&self.draft);
        if !self.field_errors.is_empty() {
            return Err(SubmitRefusal::InvalidFields(self.field_errors.clone()));
        }
        if self.mode == MembershipMode::Unavailable {
            self.submission = SubmissionStatus::Failed;
            return Err(SubmitRefusal::Unavailable);
        }
        let request_id = self
            .sequence
            .checked_add(1)
            .ok_or(SubmitRefusal::SequenceExhausted)?;
        let attempt = match &self.intent {
            Some(intent) => SubmissionAttempt {
                request_id,
                ..intent.clone()
            },
            None => SubmissionAttempt {
                operation_id: request_id,
                request_id,
                payload: RegistrationPayload {
                    name: self.draft.name.trim().to_owned(),
                    email: self.draft.email.trim().to_owned(),
                },
            },
        };
        self.sequence = request_id;
        self.intent = Some(attempt.clone());
        self.active_request = Some(request_id);
        self.submission = SubmissionStatus::Pending;
        Ok(attempt)
    }

    /// A fixture shell must explicitly complete the exact currently active request.
    pub fn finish(&mut self, attempt: &SubmissionAttempt, outcome: FinishOutcome) -> bool {
        if self.mode != MembershipMode::Fixture
            || self.screen != MembershipScreen::Form
            || self.active_request != Some(attempt.request_id)
            || self.intent.as_ref() != Some(attempt)
        {
            return false;
        }
        self.active_request = None;
        self.submission = match outcome {
            FinishOutcome::Failed => SubmissionStatus::Failed,
            FinishOutcome::Unknown => SubmissionStatus::Unknown,
            FinishOutcome::Received => {
                self.receipt = Some(attempt.clone());
                self.screen = MembershipScreen::Received;
                SubmissionStatus::Received
            }
        };
        true
    }

    pub fn open_account(&mut self) {
        self.fence_request();
        self.screen = MembershipScreen::Account;
    }

    pub fn show_account_registered_fixture(&mut self) -> bool {
        if self.mode != MembershipMode::Fixture {
            return false;
        }
        self.fence_request();
        self.screen = MembershipScreen::AccountRegistered;
        true
    }

    pub fn open_plans(&mut self) -> bool {
        let origin = match self.screen {
            MembershipScreen::Intro => PlansOrigin::Intro,
            MembershipScreen::Account => PlansOrigin::Account,
            MembershipScreen::AccountRegistered => PlansOrigin::AccountRegistered,
            MembershipScreen::Received => PlansOrigin::Received,
            MembershipScreen::Form | MembershipScreen::Plans(_) => return false,
        };
        self.screen = MembershipScreen::Plans(origin);
        true
    }

    pub fn open_plans_deep_link(&mut self) {
        self.fence_request();
        self.screen = MembershipScreen::Plans(PlansOrigin::Account);
    }

    pub fn set_plans_status(&mut self, status: PlansStatus) {
        self.plans_status = status;
    }

    pub fn back(&mut self) {
        self.fence_request();
        self.screen = match self.screen {
            MembershipScreen::Plans(PlansOrigin::Intro) | MembershipScreen::Form => {
                MembershipScreen::Intro
            }
            MembershipScreen::Plans(PlansOrigin::Account) => MembershipScreen::Account,
            MembershipScreen::Plans(PlansOrigin::AccountRegistered) => {
                MembershipScreen::AccountRegistered
            }
            MembershipScreen::Plans(PlansOrigin::Received) => MembershipScreen::Received,
            MembershipScreen::Intro
            | MembershipScreen::Received
            | MembershipScreen::AccountRegistered
            | MembershipScreen::Account => MembershipScreen::Account,
        };
    }

    fn fence_request(&mut self) {
        self.active_request = None;
        if self.submission == SubmissionStatus::Pending {
            self.submission = SubmissionStatus::Unknown;
        }
    }

    /// Logout/switch/401 clear every private projection while preserving request uniqueness.
    pub fn clear_session(&mut self) {
        *self = Self {
            mode: self.mode,
            sequence: self.sequence,
            ..Self::default()
        };
    }
}

/// Modest preview contact hints, not an RFC mailbox or identity-provider policy.
pub fn validate(draft: &MembershipDraft) -> Vec<FieldError> {
    let name = draft.name.trim();
    let email = draft.email.trim();
    let mut errors = Vec::new();
    if name.is_empty() {
        errors.push(FieldError::NameRequired);
    } else if name.chars().count() > 120 {
        errors.push(FieldError::NameTooLong);
    } else if name.chars().any(char::is_control) {
        errors.push(FieldError::NameInvalid);
    }
    if email.is_empty() {
        errors.push(FieldError::EmailRequired);
    } else if email.chars().count() > 254 {
        errors.push(FieldError::EmailTooLong);
    } else if email
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
        || !matches!(email.split_once('@'), Some((local, domain)) if !local.is_empty() && !domain.is_empty() && !domain.contains('@'))
    {
        errors.push(FieldError::EmailInvalid);
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::{
        validate, EditRefusal, FieldError, FinishOutcome, MembershipDraft, MembershipMode,
        MembershipScreen, MembershipState, PlansOrigin, PlansStatus, SubmissionStatus,
        SubmitRefusal,
    };

    fn form() -> MembershipState {
        let mut state = MembershipState::fixture();
        assert!(state.open_form());
        assert_eq!(state.set_name("  Nguyễn Ánh  ".into()), Ok(()));
        assert_eq!(state.set_email("  anh@example.com  ".into()), Ok(()));
        state
    }

    #[test]
    fn default_is_blank_and_unavailable_cannot_make_a_receipt() {
        let mut state = MembershipState::default();
        assert_eq!(state.mode(), MembershipMode::Unavailable);
        assert_eq!(state.draft(), &MembershipDraft::default());
        assert!(state.open_form());
        assert_eq!(state.set_name("Nguyễn Ánh".into()), Ok(()));
        assert_eq!(state.set_email("anh@example.com".into()), Ok(()));
        assert_eq!(state.begin_submission(), Err(SubmitRefusal::Unavailable));
        let attempt = form().begin_submission().expect("valid fixture");
        assert!(!state.finish(&attempt, FinishOutcome::Received));
        assert!(!state.show_account_registered_fixture());
        assert_eq!(state.screen(), MembershipScreen::Form);
        assert_eq!(state.submission(), SubmissionStatus::Failed);
        assert_eq!(state.receipt(), None);
    }

    #[test]
    fn validation_accumulates_independent_fields_in_name_then_email_order() {
        let mut state = MembershipState::fixture();
        assert!(state.open_form());
        assert_eq!(
            state.begin_submission(),
            Err(SubmitRefusal::InvalidFields(vec![
                FieldError::NameRequired,
                FieldError::EmailRequired,
            ]))
        );
        assert_eq!(state.submission(), SubmissionStatus::Idle);
        assert_eq!(state.receipt(), None);
        for (name, email, expected) in [
            (
                "A\nB",
                "two@@example",
                vec![FieldError::NameInvalid, FieldError::EmailInvalid],
            ),
            ("A", "a@b", vec![]),
            ("A", "a @b", vec![FieldError::EmailInvalid]),
            ("A", "@b", vec![FieldError::EmailInvalid]),
            ("A", "a@", vec![FieldError::EmailInvalid]),
        ] {
            assert_eq!(
                validate(&MembershipDraft {
                    name: name.into(),
                    email: email.into()
                }),
                expected
            );
        }
    }

    #[test]
    fn validation_counts_unicode_scalars_and_preserves_trimmed_contact() {
        assert_eq!(
            validate(&MembershipDraft {
                name: "Á".repeat(120),
                email: format!("{}@b", "é".repeat(252))
            }),
            vec![]
        );
        assert_eq!(
            validate(&MembershipDraft {
                name: "Á".repeat(121),
                email: format!("{}@b", "é".repeat(253))
            }),
            vec![FieldError::NameTooLong, FieldError::EmailTooLong]
        );
        let mut state = form();
        let attempt = state.begin_submission().expect("valid fixture");
        assert_eq!(attempt.payload().name(), "Nguyễn Ánh");
        assert_eq!(attempt.payload().email(), "anh@example.com");
        assert_eq!(state.draft().name, "  Nguyễn Ánh  ");
    }

    #[test]
    fn pending_duplicate_and_edits_are_atomic_rejections() {
        let mut state = form();
        let attempt = state.begin_submission().expect("valid fixture");
        let pending = state.clone();
        assert_eq!(state.begin_submission(), Err(SubmitRefusal::AlreadyPending));
        assert_eq!(state.set_name("Khác".into()), Err(EditRefusal::Pending));
        assert_eq!(
            state.set_email("other@example.com".into()),
            Err(EditRefusal::Pending)
        );
        assert_eq!(state, pending);
        assert!(state.finish(&attempt, FinishOutcome::Received));
        assert_eq!(state.screen(), MembershipScreen::Received);
        assert!(!state.finish(&attempt, FinishOutcome::Failed));
        assert_eq!(state.submission(), SubmissionStatus::Received);
    }

    #[test]
    fn unknown_retry_keeps_exact_intent_and_fences_the_older_dispatch() {
        let mut state = form();
        let first = state.begin_submission().expect("valid fixture");
        assert!(state.finish(&first, FinishOutcome::Unknown));
        let unknown = state.clone();
        assert_eq!(
            state.set_email("other@example.com".into()),
            Err(EditRefusal::OutcomeUnknown)
        );
        assert_eq!(state, unknown);
        let retry = state
            .begin_submission()
            .expect("reconcile same fixture intent");
        assert_eq!(retry.operation_id(), first.operation_id());
        assert_eq!(retry.payload(), first.payload());
        assert_ne!(retry.request_id(), first.request_id());
        assert!(!state.finish(&first, FinishOutcome::Received));
        assert_eq!(state.submission(), SubmissionStatus::Pending);
        assert!(state.finish(&retry, FinishOutcome::Received));
        assert_eq!(state.receipt(), Some(&retry));
    }

    #[test]
    fn known_failure_retains_draft_and_retry_but_correction_is_a_new_intent() {
        let mut state = form();
        let first = state.begin_submission().expect("valid fixture");
        assert!(state.finish(&first, FinishOutcome::Failed));
        assert_eq!(state.draft().email, "  anh@example.com  ");
        let retry = state.begin_submission().expect("retry fixture intent");
        assert_eq!(retry.operation_id(), first.operation_id());
        assert!(state.finish(&retry, FinishOutcome::Failed));
        assert_eq!(state.set_email("corrected@example.com".into()), Ok(()));
        let corrected = state.begin_submission().expect("corrected fixture intent");
        assert_ne!(corrected.operation_id(), first.operation_id());
        assert_eq!(corrected.payload().email(), "corrected@example.com");
    }

    #[test]
    fn back_preserves_safe_draft_but_never_accepts_a_late_result() {
        let mut state = form();
        let first = state.begin_submission().expect("valid fixture");
        state.back();
        assert_eq!(state.screen(), MembershipScreen::Intro);
        assert_eq!(state.submission(), SubmissionStatus::Unknown);
        assert_eq!(state.draft().name, "  Nguyễn Ánh  ");
        assert!(!state.finish(&first, FinishOutcome::Received));
        assert!(state.open_form());
        assert!(!state.finish(&first, FinishOutcome::Received));
        let retry = state.begin_submission().expect("reconcile fixture intent");
        assert_eq!(retry.operation_id(), first.operation_id());
        assert!(state.finish(&retry, FinishOutcome::Received));
    }

    #[test]
    fn session_clear_wipes_private_projection_and_never_reuses_attempt_ids() {
        for outcome in [
            FinishOutcome::Received,
            FinishOutcome::Unknown,
            FinishOutcome::Failed,
        ] {
            let mut state = form();
            let first = state.begin_submission().expect("valid fixture");
            assert!(state.finish(&first, outcome));
            state.clear_session();
            assert_eq!(state.screen(), MembershipScreen::Intro);
            assert_eq!(state.draft(), &MembershipDraft::default());
            assert_eq!(state.submission(), SubmissionStatus::Idle);
            assert_eq!(state.receipt(), None);
            assert_eq!(state.plans_status(), PlansStatus::Unpublished);
            assert!(state.open_form());
            assert_eq!(state.set_name("Người khác".into()), Ok(()));
            assert_eq!(state.set_email("other@example.com".into()), Ok(()));
            let next = state.begin_submission().expect("new session fixture");
            assert!(next.operation_id() > first.operation_id());
            assert!(!state.finish(&first, FinishOutcome::Received));
            assert_eq!(state.submission(), SubmissionStatus::Pending);
        }
    }

    #[test]
    fn plan_back_preserves_the_explicit_source_and_receipt_despite_load_failures() {
        let mut state = form();
        let received = state.begin_submission().expect("valid fixture");
        assert!(state.finish(&received, FinishOutcome::Received));
        assert!(state.open_plans());
        assert_eq!(
            state.screen(),
            MembershipScreen::Plans(PlansOrigin::Received)
        );
        for status in [
            PlansStatus::Loading,
            PlansStatus::Empty,
            PlansStatus::Unavailable,
            PlansStatus::Failed,
        ] {
            state.set_plans_status(status);
            assert_eq!(state.receipt(), Some(&received));
            assert_eq!(state.submission(), SubmissionStatus::Received);
        }
        state.back();
        assert_eq!(state.screen(), MembershipScreen::Received);
        assert!(state.show_account_registered_fixture());
        assert!(state.open_plans());
        state.back();
        assert_eq!(state.screen(), MembershipScreen::AccountRegistered);
        state.clear_session();
        assert!(state.open_plans());
        state.back();
        assert_eq!(state.screen(), MembershipScreen::Intro);
        state.open_plans_deep_link();
        state.back();
        assert_eq!(state.screen(), MembershipScreen::Account);
    }

    #[test]
    fn account_registration_fixture_never_submits_interest_or_unlocks_entitlement() {
        let mut state = MembershipState::fixture();
        assert!(state.show_account_registered_fixture());
        assert_eq!(state.submission(), SubmissionStatus::Idle);
        assert_eq!(state.draft(), &MembershipDraft::default());
        assert_eq!(state.receipt(), None);
        assert!(state.open_plans());
        assert_eq!(
            state.screen(),
            MembershipScreen::Plans(PlansOrigin::AccountRegistered)
        );
        assert_eq!(state.submission(), SubmissionStatus::Idle);
    }

    #[test]
    fn navigation_cannot_forge_receipt_or_reopen_a_received_submission() {
        let mut state = form();
        assert!(!state.show_received());
        assert_eq!(state.screen(), MembershipScreen::Form);
        let attempt = state.begin_submission().expect("valid fixture");
        assert!(state.finish(&attempt, FinishOutcome::Received));
        state.open_intro();
        assert!(!state.open_form());
        assert_eq!(state.screen(), MembershipScreen::Received);
        state.open_account();
        assert!(state.show_received());
        assert_eq!(state.receipt(), Some(&attempt));
    }

    #[test]
    fn exhausted_sequence_returns_typed_refusal_without_dispatching_or_wrapping() {
        let mut state = form();
        state.sequence = u64::MAX;
        let before = state.clone();
        assert_eq!(
            state.begin_submission(),
            Err(SubmitRefusal::SequenceExhausted)
        );
        assert_eq!(state, before);
    }
}
