//! RAM-only email proof and request fencing. No state here authenticates a listener.
use cantos_contracts::theatre_registration::VerificationStatus;
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct VerificationToken(String);

impl VerificationToken {
    pub fn from_fragment(fragment: &str) -> Option<Self> {
        let value = fragment.strip_prefix("#register-verify?token=")?;
        (value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(value.to_owned()))
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn into_secret(self) -> String {
        self.0
    }
}

impl fmt::Debug for VerificationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerificationToken([redacted])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignupFailure {
    InvalidEmail,
    Unavailable,
    UnknownOutcome,
    Protocol,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SignupPhase {
    #[default]
    Editing,
    Submitting,
    EmailAccepted,
    SubmitFailed(SignupFailure),
    AwaitingConfirmation,
    Verifying,
    SetupLinkSent,
    LinkUnavailable,
    VerificationFailed(SignupFailure),
    CaptureFailed,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct SignupState {
    epoch: u64,
    email: String,
    token: Option<VerificationToken>,
    phase: SignupPhase,
    exhausted: bool,
}

impl fmt::Debug for SignupState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignupState")
            .field("epoch", &self.epoch)
            .field("phase", &self.phase)
            .field("private_inputs", &"[redacted]")
            .finish()
    }
}

impl SignupState {
    pub fn phase(&self) -> SignupPhase {
        self.phase
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    /// Nonsecret request identity for explicitly isolated diagnostic fixture completion.
    pub(crate) fn pending_request(&self) -> Option<u64> {
        matches!(self.phase, SignupPhase::Submitting | SignupPhase::Verifying).then_some(self.epoch)
    }

    pub fn can_edit(&self) -> bool {
        !self.exhausted
            && matches!(
                self.phase,
                SignupPhase::Editing | SignupPhase::SubmitFailed(_)
            )
    }

    pub fn set_email(&mut self, email: String) {
        if self.can_edit() {
            self.email = email;
            self.phase = SignupPhase::Editing;
        }
    }

    fn advance(&mut self) -> Option<u64> {
        match self.epoch.checked_add(1) {
            Some(next) => {
                self.epoch = next;
                Some(next)
            }
            None => {
                self.exhausted = true;
                None
            }
        }
    }

    /// Every close clears secrets and invalidates both late successes and late failures.
    /// A cancellation cannot undo provisioning which the server already started.
    pub fn clear(&mut self) -> Option<VerificationToken> {
        let token = self.token.take();
        self.email.clear();
        let _ = self.advance();
        self.phase = SignupPhase::Editing;
        token
    }

    pub fn capture(&mut self, token: VerificationToken) {
        self.clear();
        if !self.exhausted {
            self.token = Some(token);
            self.phase = SignupPhase::AwaitingConfirmation;
        }
    }

    pub fn capture_failed(&mut self) {
        self.clear();
        self.phase = SignupPhase::CaptureFailed;
    }

    pub fn begin_submit(&mut self) -> Option<(u64, String)> {
        if !self.can_edit() {
            return None;
        }
        if self.email.trim().is_empty() {
            self.phase = SignupPhase::SubmitFailed(SignupFailure::InvalidEmail);
            return None;
        }
        let issued = self.advance()?;
        self.phase = SignupPhase::Submitting;
        Some((issued, self.email.trim().to_owned()))
    }

    pub fn finish_submit(&mut self, issued: u64, result: Result<(), SignupFailure>) -> bool {
        if self.epoch != issued || self.phase != SignupPhase::Submitting || self.exhausted {
            return false;
        }
        self.phase = match result {
            Ok(()) => {
                self.email.clear();
                SignupPhase::EmailAccepted
            }
            Err(error) => SignupPhase::SubmitFailed(error),
        };
        true
    }

    /// A proof is dispatched only by an explicit confirmation intent, once per captured link.
    /// Take it before IO so an ambiguous response cannot expose a blind retry control.
    pub fn begin_verify(&mut self) -> Option<(u64, VerificationToken)> {
        if self.phase != SignupPhase::AwaitingConfirmation || self.exhausted {
            return None;
        }
        let issued = self.advance()?;
        let token = self.token.take()?;
        self.phase = SignupPhase::Verifying;
        Some((issued, token))
    }

    pub fn finish_verify(
        &mut self,
        issued: u64,
        result: Result<VerificationStatus, SignupFailure>,
    ) -> bool {
        if self.epoch != issued || self.phase != SignupPhase::Verifying || self.exhausted {
            return false;
        }
        self.phase = match result {
            Ok(VerificationStatus::SetupLinkSent) => SignupPhase::SetupLinkSent,
            Ok(VerificationStatus::Unavailable) => SignupPhase::LinkUnavailable,
            Err(error) => SignupPhase::VerificationFailed(error),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof() -> VerificationToken {
        VerificationToken::from_fragment(&format!("#register-verify?token={}", "a1".repeat(32)))
            .unwrap()
    }

    #[test]
    fn email_links_are_exact_and_secrets_never_enter_debug_output() {
        let valid = format!("#register-verify?token={}", "a1".repeat(32));
        for invalid in [
            "#register-verify?token=".to_owned(),
            format!("{valid}&from=studio"),
            format!("{valid}&token={}", "a1".repeat(32)),
            valid.replace("a1", "A1"),
            valid.replace("token=", "token=%"),
            valid.replace("#register-verify", "#login"),
        ] {
            assert_eq!(VerificationToken::from_fragment(&invalid), None);
        }
        let mut state = SignupState::default();
        state.set_email("private@example.invalid".to_owned());
        assert!(!format!("{state:?}").contains("private@example.invalid"));
        state.capture(proof());
        assert_eq!(format!("{:?}", proof()), "VerificationToken([redacted])");
        assert!(!format!("{state:?}").contains(&"a1".repeat(32)));
    }

    #[test]
    fn duplicate_submit_is_blocked_and_accepted_is_only_an_email_request_receipt() {
        let mut state = SignupState::default();
        assert_eq!(state.begin_submit(), None);
        assert_eq!(
            state.phase(),
            SignupPhase::SubmitFailed(SignupFailure::InvalidEmail)
        );
        state.set_email(" listener@example.invalid ".to_owned());
        let (issued, email) = state.begin_submit().unwrap();
        assert_eq!(email, "listener@example.invalid");
        assert_eq!(state.begin_submit(), None);
        assert!(state.finish_submit(issued, Ok(())));
        assert_eq!(state.phase(), SignupPhase::EmailAccepted);
        assert_eq!(state.email(), "");
        assert_eq!(state.begin_verify(), None);
        assert!(!state.finish_submit(issued, Err(SignupFailure::Unavailable)));
    }

    #[test]
    fn navigation_logout_and_cancel_clear_email_and_fence_late_results() {
        for result in [Ok(()), Err(SignupFailure::UnknownOutcome)] {
            let mut state = SignupState::default();
            state.set_email("listener@example.invalid".to_owned());
            let (issued, _) = state.begin_submit().unwrap();
            state.clear();
            assert!(!state.finish_submit(issued, result));
            assert_eq!(state.phase(), SignupPhase::Editing);
            assert_eq!(state.email(), "");
        }
        let mut state = SignupState::default();
        state.capture(proof());
        assert_eq!(state.clear(), Some(proof()));
        assert_eq!(state.clear(), None);
        assert_eq!(state.begin_verify(), None);
    }

    #[test]
    fn scanners_never_verify_and_unknown_outcomes_never_offer_blind_retry() {
        let mut state = SignupState::default();
        state.capture(proof());
        assert_eq!(state.phase(), SignupPhase::AwaitingConfirmation);
        let (issued, _) = state.begin_verify().unwrap();
        assert_eq!(state.begin_verify(), None);
        assert!(state.finish_verify(issued, Err(SignupFailure::UnknownOutcome)));
        assert_eq!(
            state.phase(),
            SignupPhase::VerificationFailed(SignupFailure::UnknownOutcome)
        );
        assert_eq!(state.begin_verify(), None);
        assert_eq!(state.clear(), None);
    }

    #[test]
    fn stale_verify_cannot_override_cancel_or_a_new_link_and_all_receipts_stay_non_session() {
        for result in [
            Ok(VerificationStatus::SetupLinkSent),
            Ok(VerificationStatus::Unavailable),
            Err(SignupFailure::Protocol),
        ] {
            let mut state = SignupState::default();
            state.capture(proof());
            let (old, _) = state.begin_verify().unwrap();
            state.clear();
            state.capture(proof());
            assert!(!state.finish_verify(old, result));
            assert_eq!(state.phase(), SignupPhase::AwaitingConfirmation);
            let (current, _) = state.begin_verify().unwrap();
            assert!(state.finish_verify(current, result));
            assert_eq!(state.begin_verify(), None);
        }
    }
}
