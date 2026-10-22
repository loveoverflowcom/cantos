//! Pure Studio email/password sign-in state. The password lives only here, only until the typed
//! intent is built; it is never logged, persisted, or kept for a retry. No state in this module
//! authenticates anyone: only the API-admitted cookie session (`SessionState`) does.
use cantos_web_auth::Failure;
use std::fmt;

const MAX_EMAIL_BYTES: usize = 254;
const MAX_PASSWORD_BYTES: usize = 512;

/// What the sign-in form can tell the person. None of these names the failing factor (password,
/// account, security state): the API never says, and neither does the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignInFailure {
    Incomplete,
    InvalidInput,
    InvalidCredentials,
    /// 403 at login: a wrong Origin or a roster denial; the response cannot tell which, so
    /// this never claims the password was right.
    NotPermitted,
    /// 404: Identity is not configured, or Studio has no roster. No fallback exists.
    NotAvailable,
    RateLimited(u32),
    /// 503, network, timeout or an unreadable reply: whether the login happened is unknown.
    Unavailable,
    /// The API admitted another account than the owner of the retained draft; it was revoked.
    AccountMismatch,
    CleanupPending,
    /// The session changed while signing in (another tab, logout, cancel).
    Superseded,
    /// Login answered, but the confirming session read found no cookie session.
    SessionNotStored,
}

pub fn sign_in_failure(failure: Failure) -> SignInFailure {
    match failure {
        Failure::InvalidCredentials => SignInFailure::InvalidCredentials,
        Failure::Forbidden => SignInFailure::NotPermitted,
        Failure::NotFound => SignInFailure::NotAvailable,
        Failure::RateLimited(seconds) => SignInFailure::RateLimited(seconds),
        Failure::Invalid => SignInFailure::InvalidInput,
        Failure::AccountMismatch => SignInFailure::AccountMismatch,
        Failure::CleanupPending => SignInFailure::CleanupPending,
        Failure::Changed => SignInFailure::Superseded,
        // Login never answers "signed out": it only means the confirming read found nothing.
        Failure::SignedOut => SignInFailure::SessionNotStored,
        Failure::Unavailable | Failure::Protocol => SignInFailure::Unavailable,
    }
}

/// The only value that leaves this module toward the network. Debug is redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct SignInIntent {
    pub email: String,
    pub password: String,
    /// The owner of retained drafts (or of a sensitive-action reauthentication): any other
    /// account is refused and revoked, never adopted.
    pub expected_actor: Option<String>,
}
impl fmt::Debug for SignInIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignInIntent([redacted])")
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SignInPhase {
    #[default]
    Editing,
    Submitting,
    Failed(SignInFailure),
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct SignInForm {
    epoch: u64,
    email: String,
    password: String,
    phase: SignInPhase,
    exhausted: bool,
}
impl fmt::Debug for SignInForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignInForm")
            .field("epoch", &self.epoch)
            .field("phase", &self.phase)
            .field("private_inputs", &"[redacted]")
            .finish()
    }
}

impl SignInForm {
    pub fn email(&self) -> &str {
        &self.email
    }
    pub fn password(&self) -> &str {
        &self.password
    }
    pub fn phase(&self) -> SignInPhase {
        self.phase
    }
    pub fn failure(&self) -> Option<SignInFailure> {
        match self.phase {
            SignInPhase::Failed(failure) => Some(failure),
            SignInPhase::Editing | SignInPhase::Submitting => None,
        }
    }
    pub fn submitting(&self) -> bool {
        self.phase == SignInPhase::Submitting
    }
    fn editable(&self) -> bool {
        !self.exhausted && !self.submitting()
    }
    pub fn set_email(&mut self, email: String) {
        if self.editable() {
            self.email = email;
            self.phase = SignInPhase::Editing;
        }
    }
    pub fn set_password(&mut self, password: String) {
        if self.editable() {
            self.password = password;
            self.phase = SignInPhase::Editing;
        }
    }

    /// Build the one intent. The password moves out of the form here, so the field is empty
    /// before any await; a failed attempt always asks for it again. A form being submitted
    /// refuses a second intent (no dense retry, no duplicate login).
    pub fn begin(&mut self, expected_actor: Option<String>) -> Option<(u64, SignInIntent)> {
        if !self.editable() {
            return None;
        }
        let email = self.email.trim().to_owned();
        if email.is_empty()
            || self.password.is_empty()
            || email.len() > MAX_EMAIL_BYTES
            || self.password.len() > MAX_PASSWORD_BYTES
            || email.chars().any(char::is_control)
        {
            self.phase = SignInPhase::Failed(SignInFailure::Incomplete);
            return None;
        }
        let Some(issued) = self.epoch.checked_add(1) else {
            self.exhausted = true;
            return None;
        };
        self.epoch = issued;
        self.phase = SignInPhase::Submitting;
        self.email = email.clone();
        let password = std::mem::take(&mut self.password);
        Some((
            issued,
            SignInIntent {
                email,
                password,
                expected_actor: expected_actor.filter(|actor| !actor.is_empty()),
            },
        ))
    }

    /// A late result of a cancelled or superseded attempt is ignored.
    pub fn finish(&mut self, issued: u64, result: Result<(), Failure>) -> bool {
        if self.epoch != issued || !self.submitting() {
            return false;
        }
        match result {
            Ok(()) => {
                self.email.clear();
                self.phase = SignInPhase::Editing;
            }
            Err(failure) => self.phase = SignInPhase::Failed(sign_in_failure(failure)),
        }
        true
    }

    /// Show a failure that happened in another form (a reauthentication that dropped back to the
    /// entry): keep the typed address, never a password.
    pub fn show(&mut self, email: &str, failure: SignInFailure) {
        if self.editable() {
            if !email.is_empty() {
                self.email = email.to_owned();
            }
            self.password.clear();
            self.phase = SignInPhase::Failed(failure);
        }
    }

    /// The attempt never left (no session slot was available): back to editing, nothing sent.
    pub fn abandon(&mut self, issued: u64) {
        if self.epoch == issued && self.submitting() {
            self.phase = SignInPhase::Editing;
        }
    }

    /// Logout, account change or success elsewhere: wipe both fields and fence late results.
    pub fn clear(&mut self) {
        self.email.clear();
        self.password.clear();
        self.phase = SignInPhase::Editing;
        match self.epoch.checked_add(1) {
            Some(next) => self.epoch = next,
            None => self.exhausted = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(email: &str, password: &str) -> SignInForm {
        let mut form = SignInForm::default();
        form.set_email(email.into());
        form.set_password(password.into());
        form
    }

    #[test]
    fn password_leaves_the_form_with_the_intent_and_is_never_in_debug_output() {
        let mut form = filled(" owner@example.invalid ", "correct horse battery");
        assert!(!format!("{form:?}").contains("owner@example.invalid"));
        let (issued, intent) = form.begin(None).unwrap();
        assert_eq!(intent.email, "owner@example.invalid");
        assert_eq!(intent.password, "correct horse battery");
        assert_eq!(form.password(), "");
        assert!(form.submitting());
        assert_eq!(format!("{intent:?}"), "SignInIntent([redacted])");
        assert!(!format!("{form:?}").contains("correct horse"));
        // A second press while submitting is refused: no duplicate login, no dense retry.
        assert_eq!(form.begin(None), None);
        assert!(form.finish(issued, Err(Failure::InvalidCredentials)));
        assert_eq!(
            form.password(),
            "",
            "a failed attempt asks for the password again"
        );
        assert_eq!(form.email(), "owner@example.invalid");
        assert_eq!(form.failure(), Some(SignInFailure::InvalidCredentials));
    }

    #[test]
    fn incomplete_or_oversized_input_never_builds_an_intent() {
        for (email, password) in [
            ("", "pw"),
            ("   ", "pw"),
            ("a@b.example", ""),
            ("a\u{0}@b.example", "pw"),
        ] {
            let mut form = filled(email, password);
            assert_eq!(form.begin(None), None);
            assert_eq!(form.phase(), SignInPhase::Failed(SignInFailure::Incomplete));
            assert!(!form.submitting());
        }
        let mut form = filled(&format!("{}@b.example", "a".repeat(250)), "pw");
        assert_eq!(form.begin(None), None);
        let mut form = filled("a@b.example", &"p".repeat(513));
        assert_eq!(form.begin(None), None);
        let mut form = filled("a@b.example", &"p".repeat(512));
        assert!(form.begin(None).is_some());
    }

    #[test]
    fn the_retained_draft_owner_is_pinned_and_an_empty_owner_is_no_owner() {
        let mut form = filled("a@b.example", "password");
        let (_, intent) = form.begin(Some("alice".into())).unwrap();
        assert_eq!(intent.expected_actor.as_deref(), Some("alice"));
        let mut form = filled("a@b.example", "password");
        let (_, intent) = form.begin(Some(String::new())).unwrap();
        assert_eq!(intent.expected_actor, None);
    }

    #[test]
    fn an_account_mismatch_is_a_failure_that_keeps_no_secret_and_adopts_nothing() {
        let mut form = filled("bob@example.invalid", "bobs password");
        let (issued, intent) = form.begin(Some("alice".into())).unwrap();
        assert_eq!(intent.expected_actor.as_deref(), Some("alice"));
        assert!(form.finish(issued, Err(Failure::AccountMismatch)));
        assert_eq!(form.failure(), Some(SignInFailure::AccountMismatch));
        assert_eq!(form.password(), "");
    }

    #[test]
    fn late_results_of_a_cleared_or_superseded_attempt_are_ignored() {
        for result in [Ok(()), Err(Failure::Unavailable), Err(Failure::Forbidden)] {
            let mut form = filled("a@b.example", "password");
            let (old, _) = form.begin(None).unwrap();
            form.clear(); // logout / actor change / success in another form
            assert_eq!(form.email(), "");
            assert!(!form.finish(old, result));
            assert_eq!(form.phase(), SignInPhase::Editing);
            form.set_email("a@b.example".into());
            form.set_password("password".into());
            let (current, _) = form.begin(None).unwrap();
            assert!(
                !form.finish(old, result),
                "the older attempt cannot settle the newer"
            );
            assert!(form.submitting());
            assert!(form.finish(current, Ok(())));
            assert_eq!(form.email(), "");
        }
    }

    #[test]
    fn editing_is_closed_while_submitting_and_an_unsent_attempt_can_be_abandoned() {
        let mut form = filled("a@b.example", "password");
        let (issued, _) = form.begin(None).unwrap();
        form.set_email("other@b.example".into());
        form.set_password("typed during flight".into());
        assert_eq!(form.email(), "a@b.example");
        assert_eq!(form.password(), "");
        form.abandon(issued);
        assert_eq!(form.phase(), SignInPhase::Editing);
        form.set_password("again".into());
        assert_eq!(form.password(), "again");
    }

    #[test]
    fn a_failure_from_another_form_keeps_the_address_but_never_a_password() {
        let mut entry = SignInForm::default();
        entry.set_password("stale".into());
        entry.show("owner@example.invalid", SignInFailure::InvalidCredentials);
        assert_eq!(entry.email(), "owner@example.invalid");
        assert_eq!(entry.password(), "");
        assert_eq!(entry.failure(), Some(SignInFailure::InvalidCredentials));
        // A form mid-submission is never overwritten by an unrelated failure.
        let mut busy = filled("a@b.example", "password");
        busy.begin(None).unwrap();
        busy.show("other@b.example", SignInFailure::Unavailable);
        assert!(busy.submitting());
        assert_eq!(busy.email(), "a@b.example");
    }

    #[test]
    fn every_web_auth_failure_has_one_distinct_user_visible_meaning() {
        use SignInFailure::*;
        for (failure, expected) in [
            (Failure::InvalidCredentials, InvalidCredentials),
            (Failure::Forbidden, NotPermitted),
            (Failure::NotFound, NotAvailable),
            (Failure::RateLimited(7), RateLimited(7)),
            (Failure::Invalid, InvalidInput),
            (Failure::AccountMismatch, AccountMismatch),
            (Failure::CleanupPending, CleanupPending),
            (Failure::Changed, Superseded),
            (Failure::SignedOut, SessionNotStored),
            (Failure::Unavailable, Unavailable),
            (Failure::Protocol, Unavailable),
        ] {
            assert_eq!(sign_in_failure(failure), expected);
        }
    }
}
