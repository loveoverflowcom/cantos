//! Small, immutable presentation facts; the mounted account remains the only state owner.
use crate::application::account_view::{AccountViewState, SessionPhase};
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AccountUi {
    pub phase: SessionPhase,
    pub can_authenticate: bool,
    pub can_retry_logout: bool,
    pub cleanup_in_flight: bool,
}

impl AccountUi {
    fn of(state: &AccountViewState) -> Self {
        Self {
            phase: state.phase(),
            can_authenticate: state.can_authenticate(),
            can_retry_logout: state.can_retry_logout(),
            cleanup_in_flight: state.cleanup_in_flight(),
        }
    }
}

pub(crate) fn account_ui(state: ReadSignal<AccountViewState>) -> Memo<AccountUi> {
    // Borrowing avoids copying Library/sync data; equality stops unrelated notifications
    // before they reach profile/status/control closures. This is not another writable owner.
    Memo::new(move |_| state.with(AccountUi::of))
}
