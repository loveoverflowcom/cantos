//! Cookie-session sign-in at the account owner: refusals, unknown effects, supersession and
//! account switches. The owner never holds a credential; it holds only fenced scope.
use super::*;

fn id(value: u128) -> String {
    uuid::Uuid::from_u128(value).to_string()
}
fn scope(actor: &str, session: u128) -> AccountScope {
    AccountScope::from_session(actor.into(), id(session)).unwrap()
}
fn guest() -> AccountViewState {
    let mut state = AccountViewState::default();
    let issued = state.begin_refresh().unwrap();
    let SessionCompletion::Cleanup(cleanup) = state
        .finish_session(issued, Err(AccountFailure::SignedOut), || id(100))
        .unwrap()
    else {
        panic!("a missing session is cleaned up before a guest may sign in")
    };
    assert!(state.finish_logout(cleanup, Ok(())));
    state
}
fn finish(
    state: &mut AccountViewState,
    issued: AccountGeneration,
    result: Result<AccountScope, AccountFailure>,
) -> Result<SessionCompletion, AccountError> {
    state.finish_session(issued, result, || id(100))
}

#[test]
fn a_definite_refusal_returns_to_guest_without_cleanup_and_the_next_attempt_clears_it() {
    for refusal in [LoginRefusal::Credentials, LoginRefusal::Invalid] {
        let mut state = guest();
        assert!(state.can_login());
        let issued = state.begin_refresh().unwrap();
        assert_eq!(state.phase(), SessionPhase::Checking);
        assert!(
            !state.can_login(),
            "no second attempt while one is in flight"
        );
        assert_eq!(
            finish(
                &mut state,
                issued,
                Err(AccountFailure::LoginRefused(refusal))
            ),
            Ok(SessionCompletion::Ignored)
        );
        // Nothing replaced the cookie: no cleanup, no logout barrier, the form is usable again.
        assert_eq!(state.phase(), SessionPhase::Guest);
        assert!(state.can_login() && !state.cleanup_in_flight());
        assert_eq!(state.login_refusal(), Some(refusal));
        let retry = state.begin_refresh().unwrap();
        assert_eq!(state.login_refusal(), None);
        assert_eq!(
            finish(&mut state, retry, Ok(scope("a", 1)))
                .map(|c| matches!(c, SessionCompletion::Authenticated(_))),
            Ok(true)
        );
        assert_eq!(state.login_refusal(), None);
    }
}

#[test]
fn a_refusal_never_survives_logout_or_a_new_account() {
    let mut state = guest();
    let issued = state.begin_refresh().unwrap();
    finish(
        &mut state,
        issued,
        Err(AccountFailure::LoginRefused(LoginRefusal::Credentials)),
    )
    .unwrap();
    let cleanup = state.begin_logout().unwrap();
    assert_eq!(state.login_refusal(), None);
    assert!(state.finish_logout(cleanup, Ok(())));
    assert_eq!(state.login_refusal(), None);
}

#[test]
fn faults_and_unknown_effects_are_not_refusals_and_never_read_as_signed_out() {
    for failure in [
        AccountFailure::Unavailable,
        AccountFailure::RateLimited(30),
        AccountFailure::NotFound,
        AccountFailure::Protocol,
        AccountFailure::SessionChanged,
        AccountFailure::Diagnostic(crate::application::errors::ErrorKind::Forbidden),
    ] {
        let mut state = guest();
        let issued = state.begin_refresh().unwrap();
        assert_eq!(
            finish(&mut state, issued, Err(failure)),
            Ok(SessionCompletion::Ignored)
        );
        // A fault is its own phase with a recovery action; it is neither Guest nor a cleanup.
        assert_eq!(state.phase(), SessionPhase::Failed(failure));
        assert_eq!(state.login_refusal(), None);
        assert!(!state.can_login() && !state.cleanup_in_flight());
    }
    // Only a genuine 401 asks for cleanup, and it is the only way to reach Guest from a lookup.
    let mut state = AccountViewState::default();
    let issued = state.begin_refresh().unwrap();
    assert!(matches!(
        finish(&mut state, issued, Err(AccountFailure::SignedOut)),
        Ok(SessionCompletion::Cleanup(_))
    ));
}

#[test]
fn a_late_answer_of_a_superseded_attempt_cannot_change_the_current_account() {
    let mut state = guest();
    let first = state.begin_refresh().unwrap();
    // The first attempt is cancelled; a second one starts after acknowledged cleanup.
    let cleanup = state.begin_logout().unwrap();
    assert!(state.finish_logout(cleanup, Ok(())));
    let second = state.begin_refresh().unwrap();
    for stale in [
        Err(AccountFailure::LoginRefused(LoginRefusal::Credentials)),
        Err(AccountFailure::Unavailable),
        Err(AccountFailure::SignedOut),
        Ok(scope("a", 1)),
    ] {
        assert_eq!(
            finish(&mut state, first, stale),
            Ok(SessionCompletion::Ignored)
        );
        assert_eq!(state.phase(), SessionPhase::Checking);
        assert_eq!(state.login_refusal(), None);
    }
    assert!(matches!(
        finish(&mut state, second, Ok(scope("b", 2))),
        Ok(SessionCompletion::Authenticated(_))
    ));
    // A late A success after B was admitted is rejected and the account stays B.
    assert_eq!(
        finish(&mut state, first, Ok(scope("a", 1))),
        Ok(SessionCompletion::Ignored)
    );
    assert_eq!(state.actor(), Some("b"));
}

#[test]
fn a_lookup_that_finds_another_account_hides_the_old_one_before_admitting_the_new_one() {
    let mut state = guest();
    let issued = state.begin_refresh().unwrap();
    finish(&mut state, issued, Ok(scope("a", 1))).unwrap();
    let old = state.fence().unwrap();
    state.seed_sync(|sync, _| sync.begin_saved_read(&id(88)));
    assert!(state.library().is_some() && state.sync().is_some());
    // Another tab signed in as B: this tab's lookup starts, and A's data is gone at once.
    let lookup = state.begin_refresh().unwrap();
    assert!(state.library().is_none() && state.sync().is_none());
    assert_eq!(state.fence(), None);
    assert!(!state.accepts(&old));
    assert!(matches!(
        finish(&mut state, lookup, Ok(scope("b", 2))),
        Ok(SessionCompletion::Authenticated(_))
    ));
    let current = state.fence().unwrap();
    assert!(!state.accepts(&old) && state.accepts(&current));
    assert_eq!(state.actor(), Some("b"));
    assert_eq!(
        state.library().map(|library| library.saved.len()),
        Some(0),
        "B starts with an empty projection, never A's rows"
    );
}
