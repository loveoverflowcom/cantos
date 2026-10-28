//! Presentation only: current-cookie authority and retained draft ownership stay in SessionState.
use crate::{editor::Status, session::SessionState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Checking,
    Login,
    Denied,
    Unavailable,
    /// 404: Studio sign-in is not configured on the API. There is no fallback sign-in.
    SignInUnavailable,
    RetainedOwner,
    /// The API confirmed a session but its protected routes keep denying it: sign in again.
    SessionRefused,
    Workspace,
}

pub fn presentation(session: &SessionState, owner: &str, status: Status) -> Entry {
    if session.pending() {
        Entry::Checking
    } else if session.projects_owner(owner) {
        Entry::Workspace
    } else if session.actor().is_some() {
        Entry::RetainedOwner
    } else if session.held() {
        Entry::SessionRefused
    } else {
        match status {
            Status::Forbidden => Entry::Denied,
            Status::NotFound => Entry::SignInUnavailable,
            Status::Unavailable | Status::CorruptRevision => Entry::Unavailable,
            _ => Entry::Login,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checking_and_late_callbacks_never_open_a_private_workspace() {
        let (checking, old) = SessionState::default().begin().unwrap();
        assert_eq!(
            presentation(&checking, "alice", Status::SignedIn),
            Entry::Checking
        );
        let cancelled = checking.invalidated();
        assert_eq!(
            presentation(
                &cancelled.settled(old, Some("alice".into())),
                "alice",
                Status::SignedIn
            ),
            Entry::Login
        );
        let (new_check, new) = cancelled.begin().unwrap();
        let confirmed = new_check.settled(new, Some("alice".into()));
        assert_eq!(
            presentation(&confirmed, "alice", Status::SignedIn),
            Entry::Workspace
        );
    }

    #[test]
    fn a_held_session_shows_its_own_entry_and_never_projects_the_workspace() {
        let (checking, ticket) = SessionState::default().begin().unwrap();
        let (hidden, _) = checking.settled(ticket, Some("alice".into())).denied();
        let (checking, ticket) = hidden.begin().unwrap();
        let (held, _) = checking.settled(ticket, Some("alice".into())).denied();
        // Whatever the editor's status, the refusal is explained rather than a silent login.
        for status in [Status::Unauthenticated, Status::Dirty, Status::Unavailable] {
            assert_eq!(presentation(&held, "alice", status), Entry::SessionRefused);
        }
        // Reading the session again leaves the held entry; the budget is not refunded.
        let (checking, _) = held.begin().unwrap();
        assert_eq!(
            presentation(&checking, "alice", Status::Unauthenticated),
            Entry::Checking
        );
    }

    #[test]
    fn denied_or_different_account_never_projects_the_retained_draft() {
        let (checking, ticket) = SessionState::default().begin().unwrap();
        let bob = checking.settled(ticket, Some("bob".into()));
        assert_eq!(
            presentation(&bob, "alice", Status::Dirty),
            Entry::RetainedOwner
        );
        assert_eq!(
            presentation(&bob, "bob", Status::Forbidden),
            Entry::Workspace
        );
        let guest = checking.settled(ticket, None);
        assert_eq!(
            presentation(&guest, "alice", Status::Forbidden),
            Entry::Denied
        );
        assert_eq!(
            presentation(&guest, "alice", Status::Unavailable),
            Entry::Unavailable
        );
        assert_eq!(
            presentation(&guest, "alice", Status::Unauthenticated),
            Entry::Login
        );
        // A missing sign-in capability (404) is its own state, never a silent login form or
        // a fallback; a retained draft stays hidden behind it.
        assert_eq!(
            presentation(&guest, "alice", Status::NotFound),
            Entry::SignInUnavailable
        );
    }
}
