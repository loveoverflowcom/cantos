//! RAM-only mailbox proof, chosen password and request fencing for Identity v1 signup. No state
//! here authenticates a listener: completion creates an account and never a session.
use cantos_contracts::identity_auth::{SignupComplete, SignupStatus};
use std::fmt;

pub const MIN_PASSWORD_CHARS: usize = 8;
pub const MAX_PASSWORD_CHARS: usize = 128;
pub const MAX_PASSWORD_BYTES: usize = 512;

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// The mailed capability `#operation=<32 hex>&proof=<32 hex>&secret=<64 hex>`. It lives only in
/// the page's RAM: never in a URL after capture, a log, storage or a derived Debug string.
#[derive(Clone, PartialEq, Eq)]
pub struct ProofLink {
    operation_id: String,
    proof_id: String,
    secret: String,
}

impl ProofLink {
    /// Exactly the three keys, once each, in the contract's order, with lowercase hex of the
    /// exact lengths. Nothing is percent-decoded, trimmed or normalized.
    pub fn from_fragment(fragment: &str) -> Option<Self> {
        let value = fragment.strip_prefix('#')?;
        let mut parts = value.split('&');
        let operation = parts.next()?.strip_prefix("operation=")?;
        let proof = parts.next()?.strip_prefix("proof=")?;
        let secret = parts.next()?.strip_prefix("secret=")?;
        (parts.next().is_none()
            && lower_hex(operation, 32)
            && lower_hex(proof, 32)
            && lower_hex(secret, 64))
        .then(|| Self {
            operation_id: operation.to_owned(),
            proof_id: proof.to_owned(),
            secret: secret.to_owned(),
        })
    }

    /// Any fragment that names a proof field is treated as proof-bearing, valid or not: it is
    /// scrubbed from the address bar and history before anything else happens.
    pub fn is_proof_fragment(fragment: &str) -> bool {
        fragment.strip_prefix('#').is_some_and(|value| {
            value.split('&').any(|part| {
                let key = part.split_once('=').map_or(part, |(key, _)| key);
                ["operation", "proof", "secret"]
                    .into_iter()
                    .any(|name| key.eq_ignore_ascii_case(name))
            })
        })
    }
}

impl fmt::Debug for ProofLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProofLink([redacted])")
    }
}

/// Client hint only; the server's password policy stays authoritative (400 on refusal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordIssue {
    TooShort,
    TooLong,
    Mismatch,
}

fn password_issue(password: &str, repeat: &str) -> Option<PasswordIssue> {
    let scalars = password.chars().count();
    if scalars < MIN_PASSWORD_CHARS {
        Some(PasswordIssue::TooShort)
    } else if scalars > MAX_PASSWORD_CHARS || password.len() > MAX_PASSWORD_BYTES {
        Some(PasswordIssue::TooLong)
    } else if password != repeat {
        Some(PasswordIssue::Mismatch)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignupFailure {
    InvalidEmail,
    Unavailable,
    RateLimited(u32),
    /// 403/404: the account service is not open to this site (Identity is not configured).
    NotConfigured,
    UnknownOutcome,
    Protocol,
}

/// Definite refusals (`Invalid`, `Refused`, `NotConfigured`, `Forbidden`) had no effect.
/// `Unavailable`, `RateLimited` and `UnknownOutcome` may have committed: only the identical
/// request is ever repeated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionFailure {
    /// 400: the password or request shape was refused. Nothing happened; the proof is intact.
    Invalid,
    /// 401: the link/proof is expired, spent, or does not match the replayed request.
    Refused,
    /// 403: the browser origin is not admitted.
    Forbidden,
    /// 404: Identity is not configured.
    NotConfigured,
    /// 429 with a bounded Retry-After.
    RateLimited(u32),
    /// 503: the effect is unknown.
    Unavailable,
    /// Network loss, deadline or an unreadable answer: the effect is unknown.
    UnknownOutcome,
}

impl CompletionFailure {
    fn retains_request(self) -> bool {
        matches!(
            self,
            Self::RateLimited(_) | Self::Unavailable | Self::UnknownOutcome
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SignupPhase {
    #[default]
    Editing,
    Submitting,
    /// A generic receipt: identical for every address, it never says whether an account exists.
    EmailAccepted,
    SubmitFailed(SignupFailure),
    /// A proof was captured. Opening the link alone never reaches the API.
    AwaitingConfirmation,
    Completing,
    /// The identical request is retained and may be sent again, never a new one.
    CompleteRetry(CompletionFailure),
    /// The API refused the request outright; `Invalid` keeps the proof for a new password.
    CompleteRefused(CompletionFailure),
    /// An account exists (or already existed from this exact request). There is no session.
    Completed(SignupStatus),
    CaptureFailed,
}

/// The exact serialized request. It is the only place the secret and password live once the
/// user confirmed, and is dropped on success or a definite refusal.
#[derive(Clone, PartialEq, Eq)]
struct Snapshot(String);

#[derive(Clone, Default, PartialEq, Eq)]
pub struct SignupState {
    epoch: u64,
    email: String,
    proof: Option<ProofLink>,
    password: String,
    repeat: String,
    confirmed: bool,
    snapshot: Option<Snapshot>,
    phase: SignupPhase,
    exhausted: bool,
    /// Earliest instant (ms) the retained request may be sent again after a 429.
    hold_until_ms: u64,
    /// Earliest instant (ms) a new signup start may leave after a 429. Unlike every other field
    /// it survives `clear`: leaving the page, editing the email or starting over must not
    /// bypass the wait the API asked for.
    submit_hold_until_ms: u64,
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

    /// Controlled-input mirrors. They exist only while the owner is typing a password.
    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn repeat(&self) -> &str {
        &self.repeat
    }

    pub fn confirmed(&self) -> bool {
        self.confirmed
    }

    /// Nonsecret request identity for explicitly isolated diagnostic fixture completion.
    #[cfg(feature = "preview")]
    pub(crate) fn pending_request(&self) -> Option<u64> {
        matches!(
            self.phase,
            SignupPhase::Submitting | SignupPhase::Completing
        )
        .then_some(self.epoch)
    }

    pub fn can_edit(&self) -> bool {
        !self.exhausted
            && matches!(
                self.phase,
                SignupPhase::Editing | SignupPhase::SubmitFailed(_)
            )
    }

    fn can_edit_password(&self) -> bool {
        !self.exhausted && self.phase == SignupPhase::AwaitingConfirmation && self.proof.is_some()
    }

    pub fn set_email(&mut self, email: String) {
        if self.can_edit() {
            self.email = email;
            self.phase = SignupPhase::Editing;
        }
    }

    pub fn set_password(&mut self, password: String) {
        if self.can_edit_password() {
            self.password = password;
            self.confirmed = false;
        }
    }

    pub fn set_repeat(&mut self, repeat: String) {
        if self.can_edit_password() {
            self.repeat = repeat;
            self.confirmed = false;
        }
    }

    /// The owner's explicit affirmation that this mailbox creates this account with this password.
    pub fn set_confirmed(&mut self, confirmed: bool) {
        if self.can_edit_password() {
            self.confirmed = confirmed;
        }
    }

    pub fn password_issue(&self) -> Option<PasswordIssue> {
        password_issue(&self.password, &self.repeat)
    }

    /// Completion needs a captured proof, a policy-shaped matching password and the explicit
    /// confirmation, together.
    pub fn can_confirm(&self) -> bool {
        self.can_edit_password() && self.confirmed && self.password_issue().is_none()
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

    fn wipe(&mut self) {
        self.email.clear();
        self.proof = None;
        self.password.clear();
        self.repeat.clear();
        self.confirmed = false;
        self.snapshot = None;
        self.hold_until_ms = 0;
    }

    /// Every close clears secrets and invalidates both late successes and late failures. A
    /// close cannot undo an account the API may already have created.
    pub fn clear(&mut self) {
        self.wipe();
        let _ = self.advance();
        self.phase = SignupPhase::Editing;
    }

    pub fn capture(&mut self, link: ProofLink) {
        self.clear();
        if !self.exhausted {
            self.proof = Some(link);
            self.phase = SignupPhase::AwaitingConfirmation;
        }
    }

    pub fn capture_failed(&mut self) {
        self.clear();
        self.phase = SignupPhase::CaptureFailed;
    }

    /// Whole seconds (rounded up) until a signup start may be sent again, if a 429 holds it.
    pub fn submit_wait_seconds(&self, now_ms: u64) -> Option<u32> {
        let remaining = self.submit_hold_until_ms.checked_sub(now_ms)?;
        (remaining > 0).then(|| remaining.div_ceil(1000).min(u64::from(u32::MAX)) as u32)
    }

    /// Refused inside a 429 hold, whatever the email, key or button that asked.
    pub fn begin_submit(&mut self, now_ms: u64) -> Option<(u64, String)> {
        if !self.can_edit() || self.submit_wait_seconds(now_ms).is_some() {
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

    pub fn finish_submit(
        &mut self,
        issued: u64,
        result: Result<(), SignupFailure>,
        now_ms: u64,
    ) -> bool {
        // A 429 is a fact about the API, so it holds even when the page was left meanwhile.
        if let Err(SignupFailure::RateLimited(seconds)) = result {
            let until = now_ms.saturating_add(u64::from(seconds) * 1000);
            self.submit_hold_until_ms = self.submit_hold_until_ms.max(until);
        }
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

    /// Only the mailbox owner's explicit confirmation builds the request. The serialized body is
    /// retained as the immutable snapshot; the editable copies of the password are cleared
    /// here, before any I/O.
    pub fn begin_complete(&mut self) -> Option<(u64, String)> {
        if !self.can_confirm() {
            return None;
        }
        let link = self.proof.as_ref()?;
        let body = serde_json::to_string(&SignupComplete {
            operation_id: link.operation_id.clone(),
            proof_id: link.proof_id.clone(),
            proof_secret: link.secret.clone(),
            password: self.password.clone(),
            explicit_confirmation: true,
        })
        .ok()?;
        let issued = self.advance()?;
        self.password.clear();
        self.repeat.clear();
        self.confirmed = false;
        self.snapshot = Some(Snapshot(body.clone()));
        self.phase = SignupPhase::Completing;
        Some((issued, body))
    }

    /// Repeat the identical retained request after an unknown outcome. Never a new operation,
    /// and never before a 429's Retry-After has passed.
    pub fn begin_retry(&mut self, now_ms: u64) -> Option<(u64, String)> {
        if !matches!(self.phase, SignupPhase::CompleteRetry(_)) || now_ms < self.hold_until_ms {
            return None;
        }
        let body = self.snapshot.as_ref()?.0.clone();
        let issued = self.advance()?;
        self.phase = SignupPhase::Completing;
        Some((issued, body))
    }

    pub fn finish_complete(
        &mut self,
        issued: u64,
        result: Result<SignupStatus, CompletionFailure>,
        now_ms: u64,
    ) -> bool {
        if self.epoch != issued || self.phase != SignupPhase::Completing || self.exhausted {
            return false;
        }
        match result {
            Ok(status) => {
                self.wipe();
                self.phase = SignupPhase::Completed(status);
            }
            Err(failure) if failure.retains_request() => {
                if let CompletionFailure::RateLimited(seconds) = failure {
                    self.hold_until_ms = now_ms.saturating_add(u64::from(seconds) * 1000);
                }
                self.phase = SignupPhase::CompleteRetry(failure);
            }
            Err(CompletionFailure::Invalid) => {
                // Nothing happened: the proof is intact and the owner may choose another password.
                self.snapshot = None;
                self.phase = SignupPhase::CompleteRefused(CompletionFailure::Invalid);
            }
            Err(failure) => {
                self.wipe();
                self.phase = SignupPhase::CompleteRefused(failure);
            }
        }
        true
    }

    /// After a 400 the same proof accepts a fresh, explicit attempt with a new password.
    pub fn edit_after_refusal(&mut self) -> bool {
        if self.phase != SignupPhase::CompleteRefused(CompletionFailure::Invalid)
            || self.proof.is_none()
        {
            return false;
        }
        self.phase = SignupPhase::AwaitingConfirmation;
        true
    }

    #[cfg(test)]
    pub(crate) fn retained_request(&self) -> Option<&str> {
        self.snapshot.as_ref().map(|snapshot| snapshot.0.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OP: &str = "0123456789abcdef0123456789abcdef";
    const PROOF: &str = "fedcba9876543210fedcba9876543210";

    fn secret() -> String {
        "ab".repeat(32)
    }
    fn fragment() -> String {
        format!("#operation={OP}&proof={PROOF}&secret={}", secret())
    }
    fn link() -> ProofLink {
        ProofLink::from_fragment(&fragment()).unwrap()
    }
    fn awaiting() -> SignupState {
        let mut state = SignupState::default();
        state.capture(link());
        state
    }
    fn ready() -> SignupState {
        let mut state = awaiting();
        state.set_password("correct horse battery".into());
        state.set_repeat("correct horse battery".into());
        state.set_confirmed(true);
        state
    }

    #[test]
    fn proof_parser_requires_exactly_three_lowercase_hex_fields_in_contract_order() {
        assert!(ProofLink::from_fragment(&fragment()).is_some());
        assert_eq!(format!("{:?}", link()), "ProofLink([redacted])");
        let valid = fragment();
        let invalid = [
            String::new(),
            "#".to_owned(),
            format!("operation={OP}&proof={PROOF}&secret={}", secret()),
            format!("{valid}&secret={}", secret()),
            format!("{valid}&from=studio"),
            format!("{valid}&"),
            format!("#proof={PROOF}&operation={OP}&secret={}", secret()),
            format!("#operation={OP}&proof={PROOF}"),
            format!("#operation={OP}&secret={}&proof={PROOF}", secret()),
            valid.replace(OP, &OP.to_uppercase()),
            valid.replace(OP, &OP[..31]),
            valid.replace(PROOF, &format!("{PROOF}0")),
            valid.replace(&secret(), &"ab".repeat(31)),
            valid.replace(&secret(), &format!("{}%", "ab".repeat(31))),
            valid.replace(&secret(), &format!("{}zz", "ab".repeat(31))),
            valid.replace("operation=", "operation=%30"),
            valid.replace("#operation", "#register-verify?token"),
            format!("#login?from=home&{}", &valid[1..]),
        ];
        for fragment in invalid {
            assert!(ProofLink::from_fragment(&fragment).is_none(), "{fragment}");
        }
    }

    #[test]
    fn any_fragment_naming_a_proof_field_is_scrubbed_even_when_invalid() {
        assert!(ProofLink::is_proof_fragment(&fragment()));
        for scrubbed in [
            "#operation=",
            "#operation",
            "#proof=zz",
            "#secret=00",
            "#x=1&secret=00",
            "#OPERATION=abc",
            "#login?from=home&proof=1",
        ] {
            assert!(ProofLink::is_proof_fragment(scrubbed), "{scrubbed}");
        }
        for ordinary in [
            "",
            "#",
            "#login",
            "#login?from=home",
            "#register?from=account",
            "#discover",
            "#membership?from=home",
            "#library",
            "#work/1?query=operation",
        ] {
            assert!(!ProofLink::is_proof_fragment(ordinary), "{ordinary}");
        }
    }

    #[test]
    fn opening_a_link_never_builds_a_request_until_the_owner_confirms_a_valid_password() {
        let mut state = awaiting();
        assert_eq!(state.phase(), SignupPhase::AwaitingConfirmation);
        // Capture/prefetch/polling are not intents: nothing can complete.
        assert_eq!(state.begin_complete(), None);
        assert_eq!(state.begin_retry(u64::MAX), None);
        state.set_password("correct horse battery".into());
        state.set_repeat("correct horse battery".into());
        // A typed password without the explicit confirmation is still not a request.
        assert!(!state.can_confirm());
        assert_eq!(state.begin_complete(), None);
        state.set_confirmed(true);
        assert!(state.can_confirm());
        // Any edit withdraws the confirmation.
        state.set_repeat("correct horse batter".into());
        assert!(!state.confirmed() && !state.can_confirm());
        assert_eq!(state.password_issue(), Some(PasswordIssue::Mismatch));
        state.set_repeat("correct horse battery".into());
        assert_eq!(state.begin_complete(), None);
    }

    #[test]
    fn password_hints_follow_the_documented_policy_without_trimming() {
        let mut state = awaiting();
        for (password, issue) in [
            ("", Some(PasswordIssue::TooShort)),
            ("1234567", Some(PasswordIssue::TooShort)),
            ("12345678", None),
            // Scalars, not bytes: eight Vietnamese letters are valid, trailing space counts.
            ("Việt Nam", None),
            ("        ", None),
        ] {
            state.set_password(password.into());
            state.set_repeat(password.into());
            assert_eq!(state.password_issue(), issue, "{password:?}");
        }
        state.set_password("a".repeat(129));
        state.set_repeat("a".repeat(129));
        assert_eq!(state.password_issue(), Some(PasswordIssue::TooLong));
        // The limit is in scalars: 128 four-byte emoji are exactly 512 bytes and valid.
        state.set_password("😀".repeat(128));
        state.set_repeat("😀".repeat(128));
        assert_eq!(state.password_issue(), None);
    }

    #[test]
    fn the_request_is_the_exact_contract_body_and_inputs_are_cleared_before_io() {
        let mut state = ready();
        let (issued, body) = state.begin_complete().unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).unwrap(),
            serde_json::json!({
                "operation_id": OP,
                "proof_id": PROOF,
                "proof_secret": secret(),
                "password": "correct horse battery",
                "explicit_confirmation": true,
            })
        );
        assert_eq!(state.phase(), SignupPhase::Completing);
        assert_eq!(state.password(), "");
        assert_eq!(state.repeat(), "");
        assert!(!state.confirmed());
        assert_eq!(state.retained_request(), Some(body.as_str()));
        // A second press during flight cannot create another request or edit the password.
        state.set_password("another password".into());
        assert_eq!(state.password(), "");
        assert_eq!(state.begin_complete(), None);
        assert!(state.finish_complete(issued, Ok(SignupStatus::Completed), 0));
    }

    #[test]
    fn lost_or_unavailable_responses_retry_the_identical_request_only() {
        for failure in [
            CompletionFailure::Unavailable,
            CompletionFailure::UnknownOutcome,
            CompletionFailure::RateLimited(0),
        ] {
            let mut state = ready();
            let (first, body) = state.begin_complete().unwrap();
            assert!(state.finish_complete(first, Err(failure), 1_000));
            assert_eq!(state.phase(), SignupPhase::CompleteRetry(failure));
            // The password cannot be edited, and no new operation can be built.
            state.set_password("silently different".into());
            assert_eq!(state.password(), "");
            assert_eq!(state.begin_complete(), None);
            let (second, retried) = state.begin_retry(1_000).unwrap();
            assert_ne!(first, second);
            assert_eq!(retried, body, "the identical serialized request");
            // A late answer of the first attempt cannot settle the second.
            assert!(!state.finish_complete(first, Ok(SignupStatus::Completed), 0));
            assert_eq!(state.phase(), SignupPhase::Completing);
            assert_eq!(state.begin_retry(1_000), None);
            assert!(state.finish_complete(second, Ok(SignupStatus::Replayed), 0));
            assert_eq!(
                state.phase(),
                SignupPhase::Completed(SignupStatus::Replayed)
            );
            assert_eq!(state.retained_request(), None);
        }
    }

    #[test]
    fn retry_after_is_honoured_without_dense_retries() {
        let mut state = ready();
        let (issued, _) = state.begin_complete().unwrap();
        state.finish_complete(issued, Err(CompletionFailure::RateLimited(30)), 10_000);
        assert_eq!(state.begin_retry(39_999), None);
        assert_eq!(
            state.phase(),
            SignupPhase::CompleteRetry(CompletionFailure::RateLimited(30))
        );
        assert!(state.begin_retry(40_000).is_some());
    }

    #[test]
    fn success_never_confers_a_session_and_drops_every_secret() {
        for status in [SignupStatus::Completed, SignupStatus::Replayed] {
            let mut state = ready();
            let (issued, _) = state.begin_complete().unwrap();
            assert!(state.finish_complete(issued, Ok(status), 0));
            assert_eq!(state.phase(), SignupPhase::Completed(status));
            assert_eq!(state.retained_request(), None);
            assert_eq!(state.begin_complete(), None);
            assert_eq!(state.begin_retry(u64::MAX), None);
            assert!(!state.finish_complete(issued, Err(CompletionFailure::Refused), 0));
        }
    }

    #[test]
    fn definite_refusals_drop_the_request_and_only_a_400_keeps_the_proof() {
        for failure in [
            CompletionFailure::Refused,
            CompletionFailure::Forbidden,
            CompletionFailure::NotConfigured,
        ] {
            let mut state = ready();
            let (issued, _) = state.begin_complete().unwrap();
            assert!(state.finish_complete(issued, Err(failure), 0));
            assert_eq!(state.phase(), SignupPhase::CompleteRefused(failure));
            assert_eq!(state.retained_request(), None);
            assert!(!state.edit_after_refusal());
            assert_eq!(state.begin_retry(u64::MAX), None);
        }
        let mut state = ready();
        let (issued, body) = state.begin_complete().unwrap();
        assert!(state.finish_complete(issued, Err(CompletionFailure::Invalid), 0));
        assert_eq!(
            state.phase(),
            SignupPhase::CompleteRefused(CompletionFailure::Invalid)
        );
        assert_eq!(state.retained_request(), None);
        assert!(state.edit_after_refusal());
        state.set_password("a different password".into());
        state.set_repeat("a different password".into());
        state.set_confirmed(true);
        let (_, next) = state.begin_complete().unwrap();
        assert_ne!(next, body);
        assert!(next.contains(&secret()), "the intact proof is reused");
    }

    #[test]
    fn leave_and_new_links_fence_late_results_and_forget_everything() {
        let mut state = ready();
        let (old, _) = state.begin_complete().unwrap();
        state.clear();
        assert_eq!(state.phase(), SignupPhase::Editing);
        assert_eq!(state.retained_request(), None);
        state.capture(link());
        assert!(!state.finish_complete(old, Ok(SignupStatus::Completed), 0));
        assert_eq!(state.phase(), SignupPhase::AwaitingConfirmation);
        assert_eq!(state.begin_retry(u64::MAX), None);
        let mut state = ready();
        state.clear();
        assert_eq!(
            (state.password(), state.repeat(), state.confirmed()),
            ("", "", false)
        );
        assert_eq!(state.begin_complete(), None);
    }

    #[test]
    fn debug_output_never_contains_proof_password_or_email() {
        let mut state = SignupState::default();
        state.set_email("private@example.invalid".to_owned());
        assert!(!format!("{state:?}").contains("private@example.invalid"));
        let mut state = ready();
        let (_, body) = state.begin_complete().unwrap();
        let debug = format!("{state:?}");
        for hidden in [OP, PROOF, &secret(), "correct horse battery", &body] {
            assert!(!debug.contains(hidden), "{hidden}");
        }
    }

    #[test]
    fn duplicate_submit_is_blocked_and_accepted_is_only_a_generic_receipt() {
        let mut state = SignupState::default();
        assert_eq!(state.begin_submit(0), None);
        assert_eq!(
            state.phase(),
            SignupPhase::SubmitFailed(SignupFailure::InvalidEmail)
        );
        state.set_email(" listener@example.invalid ".to_owned());
        let (issued, email) = state.begin_submit(0).unwrap();
        assert_eq!(email, "listener@example.invalid");
        assert_eq!(state.begin_submit(0), None);
        assert!(state.finish_submit(issued, Ok(()), 0));
        assert_eq!(state.phase(), SignupPhase::EmailAccepted);
        assert_eq!(state.email(), "");
        assert_eq!(state.begin_complete(), None);
        assert!(!state.finish_submit(issued, Err(SignupFailure::Unavailable), 0));
    }

    #[test]
    fn navigation_and_cancel_clear_email_and_fence_late_submit_results() {
        for result in [Ok(()), Err(SignupFailure::UnknownOutcome)] {
            let mut state = SignupState::default();
            state.set_email("listener@example.invalid".to_owned());
            let (issued, _) = state.begin_submit(0).unwrap();
            state.clear();
            assert!(!state.finish_submit(issued, result, 0));
            assert_eq!(state.phase(), SignupPhase::Editing);
            assert_eq!(state.email(), "");
        }
    }

    #[test]
    fn an_unreadable_link_is_a_capture_failure_that_holds_no_secret() {
        let mut state = ready();
        state.capture_failed();
        assert_eq!(state.phase(), SignupPhase::CaptureFailed);
        assert_eq!(state.begin_complete(), None);
        assert_eq!(state.retained_request(), None);
    }

    #[test]
    fn a_429_holds_every_new_start_until_its_deadline_whatever_asks() {
        let mut state = SignupState::default();
        state.set_email("listener@example.invalid".to_owned());
        let (issued, _) = state.begin_submit(10_000).unwrap();
        assert!(state.finish_submit(issued, Err(SignupFailure::RateLimited(30)), 10_000));
        assert_eq!(
            state.phase(),
            SignupPhase::SubmitFailed(SignupFailure::RateLimited(30))
        );
        assert_eq!(state.submit_wait_seconds(10_000), Some(30));
        assert_eq!(state.submit_wait_seconds(39_001), Some(1));
        // The same email again (button or Enter) is refused at the command boundary.
        assert_eq!(state.begin_submit(20_000), None);
        // Another email does not reset the wait ...
        state.set_email("another@example.invalid".to_owned());
        assert_eq!(state.phase(), SignupPhase::Editing);
        assert_eq!(state.begin_submit(39_999), None);
        assert_eq!(state.submit_wait_seconds(39_999), Some(1));
        // ... leaving the page and starting over does not either ...
        state.clear();
        state.set_email("third@example.invalid".to_owned());
        assert_eq!(state.begin_submit(30_000), None);
        // ... only the deadline does.
        assert_eq!(state.submit_wait_seconds(40_000), None);
        assert!(state.begin_submit(40_000).is_some());
    }

    #[test]
    fn a_late_429_of_a_page_that_was_left_still_holds_and_a_shorter_one_never_shortens() {
        let mut state = SignupState::default();
        state.set_email("listener@example.invalid".to_owned());
        let (issued, _) = state.begin_submit(0).unwrap();
        state.clear();
        assert!(!state.finish_submit(issued, Err(SignupFailure::RateLimited(60)), 1_000));
        assert_eq!(state.submit_wait_seconds(1_000), Some(60));
        state.set_email("listener@example.invalid".to_owned());
        assert_eq!(state.begin_submit(30_000), None);
        let (issued, _) = state.begin_submit(61_000).unwrap();
        assert!(state.finish_submit(issued, Err(SignupFailure::RateLimited(1)), 61_000));
        assert_eq!(state.submit_wait_seconds(61_000), Some(1));
        // Other failures never arm a wait.
        state.set_email("listener@example.invalid".to_owned());
        let (issued, _) = state.begin_submit(70_000).unwrap();
        assert!(state.finish_submit(issued, Err(SignupFailure::Unavailable), 70_000));
        assert_eq!(state.submit_wait_seconds(70_000), None);
    }

    #[test]
    fn a_completion_retry_is_the_identical_serialized_request_of_the_first_attempt() {
        let mut state = ready();
        let (first, body) = state.begin_complete().unwrap();
        assert!(state.finish_complete(first, Err(CompletionFailure::RateLimited(5)), 1_000));
        // A cooling retry builds nothing and a new password cannot replace the snapshot.
        assert_eq!(state.begin_retry(5_999), None);
        state.set_password("silently different".into());
        assert_eq!(state.begin_complete(), None);
        let (_, retried) = state.begin_retry(6_000).unwrap();
        assert_eq!(retried, body);
    }
}
