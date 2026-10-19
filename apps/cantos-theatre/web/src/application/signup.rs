//! RAM-only email proof and request fencing. No state here authenticates a listener.
use cantos_contracts::theatre_registration::VerificationStatus;
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct VerificationToken(VerificationDispatch);

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum VerificationDispatch {
    Email(String),
    Firebase {
        operation_id: String,
        oob_code: String,
    },
}

impl VerificationToken {
    pub fn from_fragment(fragment: &str) -> Option<Self> {
        let value = fragment.strip_prefix("#register-verify?token=")?;
        (value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(VerificationDispatch::Email(value.to_owned())))
    }

    /// The query is untrusted. Only the fixed same-origin continuation identifies an operation;
    /// neither an API key in the link nor a caller-supplied email is authority.
    pub fn from_firebase_query(query: &str, origin: &str) -> Option<Self> {
        if query.len() > 8192 {
            return None;
        }
        let mut mode = None;
        let mut code = None;
        let mut continuation = None;
        let mut api_key = None;
        let mut language = None;
        for (key, value) in url::form_urlencoded::parse(query.strip_prefix('?')?.as_bytes()) {
            let slot = match key.as_ref() {
                "mode" => &mut mode,
                "oobCode" => &mut code,
                "continueUrl" => &mut continuation,
                "apiKey" => &mut api_key,
                "lang" => &mut language,
                _ => return None,
            };
            if slot.replace(value.into_owned()).is_some() {
                return None;
            }
        }
        if mode.as_deref() != Some("verifyEmail") {
            return None;
        }
        let oob_code = code?;
        if oob_code.is_empty()
            || oob_code.len() > 2048
            || !oob_code.bytes().all(|b| b.is_ascii_graphic())
        {
            return None;
        }
        let raw_continuation = continuation?;
        let target = url::Url::parse(&raw_continuation).ok()?;
        if target.as_str() != raw_continuation
            || target.origin().ascii_serialization() != origin
            || target.path() != "/"
            || !target.username().is_empty()
            || target.password().is_some()
            || target.fragment().is_some()
        {
            return None;
        }
        let pairs: Vec<_> = target.query_pairs().collect();
        let [(key, operation)] = pairs.as_slice() else {
            return None;
        };
        if key != "firebaseEnrollment" {
            return None;
        }
        let parsed = uuid::Uuid::parse_str(operation).ok()?;
        if parsed.is_nil() || parsed.hyphenated().to_string() != operation.as_ref() {
            return None;
        }
        Some(Self(VerificationDispatch::Firebase {
            operation_id: operation.to_string(),
            oob_code,
        }))
    }

    pub fn is_firebase(&self) -> bool {
        matches!(self.0, VerificationDispatch::Firebase { .. })
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn into_dispatch(self) -> VerificationDispatch {
        self.0
    }
}

impl fmt::Debug for VerificationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VerificationToken([redacted])")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SetupLink {
    login: String,
    url: String,
}

impl SetupLink {
    pub fn new(login: String, raw_url: String) -> Option<Self> {
        let parsed = url::Url::parse(&raw_url).ok()?;
        if login.is_empty()
            || login.len() > 256
            || login.chars().any(char::is_control)
            || raw_url.len() > 4096
            || raw_url.chars().any(char::is_control)
            || parsed.as_str() != raw_url
            || parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return None;
        }
        Some(Self {
            login,
            url: raw_url,
        })
    }
}

impl fmt::Debug for SetupLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SetupLink([redacted])")
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FirebaseVerification {
    SetupReady(SetupLink),
    Unavailable,
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
    SetupReady,
    SetupOpened,
    LinkUnavailable,
    VerificationFailed(SignupFailure),
    CaptureFailed,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct SignupState {
    epoch: u64,
    email: String,
    token: Option<VerificationToken>,
    setup: Option<SetupLink>,
    firebase: bool,
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
    #[cfg(feature = "preview")]
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
        self.setup = None;
        self.firebase = false;
        let _ = self.advance();
        self.phase = SignupPhase::Editing;
        token
    }

    pub fn capture(&mut self, token: VerificationToken) {
        self.clear();
        if !self.exhausted {
            self.firebase = token.is_firebase();
            self.token = Some(token);
            self.phase = SignupPhase::AwaitingConfirmation;
        }
    }

    pub fn capture_failed(&mut self) {
        self.clear();
        self.phase = SignupPhase::CaptureFailed;
    }

    pub fn is_firebase(&self) -> bool {
        self.firebase
    }

    pub fn setup_login(&self) -> Option<&str> {
        self.setup.as_ref().map(|setup| setup.login.as_str())
    }

    /// Remove the capability before navigating: a second click, Back or a late completion
    /// cannot recover it. Failure to reach the issuer needs reconciliation, not blind replay.
    pub fn take_setup_url(&mut self) -> Option<String> {
        if self.phase != SignupPhase::SetupReady {
            return None;
        }
        let setup = self.setup.take()?;
        self.phase = SignupPhase::SetupOpened;
        Some(setup.url)
    }

    pub fn setup_navigation_failed(&mut self) {
        if self.phase == SignupPhase::SetupOpened {
            self.phase = SignupPhase::VerificationFailed(SignupFailure::UnknownOutcome);
        }
    }

    pub fn finish_firebase(
        &mut self,
        issued: u64,
        result: Result<FirebaseVerification, SignupFailure>,
    ) -> bool {
        if self.epoch != issued
            || self.phase != SignupPhase::Verifying
            || self.exhausted
            || !self.firebase
        {
            return false;
        }
        self.phase = match result {
            Ok(FirebaseVerification::SetupReady(setup)) => {
                self.setup = Some(setup);
                SignupPhase::SetupReady
            }
            Ok(FirebaseVerification::Unavailable) => SignupPhase::LinkUnavailable,
            Err(error) => SignupPhase::VerificationFailed(error),
        };
        true
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
        if self.epoch != issued
            || self.phase != SignupPhase::Verifying
            || self.exhausted
            || self.firebase
        {
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

    fn firebase_proof() -> VerificationToken {
        VerificationToken::from_firebase_query(
            "?mode=verifyEmail&oobCode=fixture-secret&apiKey=untrusted-key&continueUrl=https%3A%2F%2Fcantos.example.invalid%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001",
            "https://cantos.example.invalid",
        ).unwrap()
    }

    fn setup() -> SetupLink {
        SetupLink::new(
            "listener_fixture".to_owned(),
            "https://idm.example.invalid/ui/reset?token=private-fixture".to_owned(),
        )
        .unwrap()
    }

    #[test]
    fn firebase_parser_requires_exact_action_mode_operation_and_same_origin_continuation() {
        let valid = "?mode=verifyEmail&oobCode=fixture-secret&apiKey=untrusted-key&continueUrl=https%3A%2F%2Fcantos.example.invalid%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001";
        assert!(firebase_proof().is_firebase());
        assert_eq!(
            format!("{:?}", firebase_proof()),
            "VerificationToken([redacted])"
        );
        for invalid in [
            valid.replace("verifyEmail","resetPassword"),
            format!("{valid}&oobCode=second"),
            format!("{valid}&email=attacker%40example.invalid"),
            valid.replace("fixture-secret", ""),
            valid.replace("fixture-secret", "%0aopaque"),
            valid.replace("cantos.example.invalid", "attacker.invalid"),
            valid.replace("%2F%3FfirebaseEnrollment", "%2Fevil%3FfirebaseEnrollment"),
            valid.replace("firebaseEnrollment", "operation"),
            valid.replace("00000000-0000-4000-8000-000000000001", "00000000-0000-0000-0000-000000000000"),
            format!("{valid}%26firebaseEnrollment%3D00000000-0000-4000-8000-000000000002"),
            format!("{valid}%23ignored"),
            "?mode=verifyEmail&oobCode=fixture".to_owned(),
            "?mode=verifyEmail&oobCode=fixture&continueUrl=https%3A%2F%2Fuser%40cantos.example.invalid%2F%3FfirebaseEnrollment%3D00000000-0000-4000-8000-000000000001".to_owned(),
            format!("{valid}&lang={}","a".repeat(8192)),
        ] { assert_eq!(VerificationToken::from_firebase_query(&invalid,"https://cantos.example.invalid"),None,"invalid action admitted"); }
    }

    #[test]
    fn firebase_proof_and_setup_are_taken_once_and_never_confer_a_session() {
        let mut state = SignupState::default();
        state.capture(firebase_proof());
        assert_eq!(state.phase(), SignupPhase::AwaitingConfirmation);
        assert_eq!(state.take_setup_url(), None);
        let (issued, _) = state.begin_verify().unwrap();
        assert_eq!(state.begin_verify(), None);
        assert!(!state.finish_verify(issued, Ok(VerificationStatus::SetupLinkSent)));
        assert!(state.finish_firebase(issued, Ok(FirebaseVerification::SetupReady(setup()))));
        assert_eq!(state.phase(), SignupPhase::SetupReady);
        assert_eq!(state.setup_login(), Some("listener_fixture"));
        assert!(!format!("{state:?}").contains("private-fixture"));
        assert_eq!(
            state.take_setup_url(),
            Some("https://idm.example.invalid/ui/reset?token=private-fixture".to_owned())
        );
        assert_eq!(state.phase(), SignupPhase::SetupOpened);
        assert_eq!(state.setup_login(), None);
        assert_eq!(state.take_setup_url(), None);
        assert!(!state.finish_firebase(issued, Ok(FirebaseVerification::SetupReady(setup()))));
        state.setup_navigation_failed();
        assert_eq!(
            state.phase(),
            SignupPhase::VerificationFailed(SignupFailure::UnknownOutcome)
        );
        assert_eq!(state.begin_verify(), None);
    }

    #[test]
    fn firebase_cancel_history_and_unknown_outcomes_fence_all_late_secrets_without_retry() {
        for result in [
            Ok(FirebaseVerification::SetupReady(setup())),
            Ok(FirebaseVerification::Unavailable),
            Err(SignupFailure::UnknownOutcome),
        ] {
            let mut state = SignupState::default();
            state.capture(firebase_proof());
            let (old, _) = state.begin_verify().unwrap();
            assert_eq!(state.clear(), None); // raw action was already taken before IO
            state.capture(firebase_proof());
            assert!(!state.finish_firebase(old, result));
            assert_eq!(state.phase(), SignupPhase::AwaitingConfirmation);
            let (current, _) = state.begin_verify().unwrap();
            assert!(state.finish_firebase(current, Err(SignupFailure::UnknownOutcome)));
            assert_eq!(state.begin_verify(), None);
            assert_eq!(state.take_setup_url(), None);
        }
        let mut state = SignupState::default();
        state.capture(firebase_proof());
        let (issued, _) = state.begin_verify().unwrap();
        state.finish_firebase(issued, Ok(FirebaseVerification::SetupReady(setup())));
        state.clear();
        assert_eq!(state.setup_login(), None);
        assert_eq!(state.take_setup_url(), None);
    }

    #[test]
    fn setup_capability_refuses_unsafe_navigation_and_debug_is_redacted() {
        assert_eq!(format!("{:?}", setup()), "SetupLink([redacted])");
        for raw in [
            "http://idm.example.invalid/ui/reset",
            "javascript:alert(1)",
            "https://user@idm.example.invalid/ui/reset",
            "https://idm.example.invalid/ui/reset#secret",
            "https://idm.example.invalid/\nui/reset",
        ] {
            assert_eq!(SetupLink::new("fixture".to_owned(), raw.to_owned()), None);
        }
    }

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
