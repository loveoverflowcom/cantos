//! Thin browser effects for shared consumer state. Every response is fenced before publication.
use crate::application::{
    account::RequestFence, account_view::AccountViewState, public_discovery::DiscoveryTicket,
    shared_access::SharedAccess,
};
use leptos::prelude::*;
pub(crate) type Requests = StoredValue<crate::live::shared::Requests, LocalStorage>;

pub(crate) fn run_catalog(
    state: RwSignal<SharedAccess>,
    account: RwSignal<AccountViewState>,
    ticket: DiscoveryTicket,
    fence: RequestFence,
    requests: Requests,
) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        let mut ticket = ticket;
        loop {
            if !account.with_untracked(|a| a.accepts(&fence))
                || !state.with_untracked(|s| s.accepts(&fence))
            {
                return;
            }
            let result = match requests.with_value(|r| r.start()) {
                Ok(controller) => {
                    crate::live::shared::browser::catalog(&ticket, &fence, controller).await
                }
                Err(_) => {
                    Err(crate::application::public_discovery::DiscoveryReadError::Unavailable)
                }
            };
            if !account.with_untracked(|a| a.accepts(&fence)) {
                return;
            }
            let Some(step) = state.try_update(|s| {
                if s.accepts(&fence) {
                    s.catalog.finish(&ticket, result)
                } else {
                    crate::application::public_discovery::Step::Ignored
                }
            }) else {
                return;
            };
            match step {
                crate::application::public_discovery::Step::Continue(next) => ticket = next,
                _ => return,
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (state, account, ticket, fence, requests);
}

pub(crate) fn claim(
    state: RwSignal<SharedAccess>,
    account: RwSignal<AccountViewState>,
    requests: Requests,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some((fence, operation)) = state
            .try_update(|s| s.begin_claim(crate::application::library_sync::generate_uuid_string()))
            .flatten()
        else {
            return;
        };
        leptos::task::spawn_local(async move {
            if !account.with_untracked(|a| a.accepts(&fence)) {
                return;
            }
            let result = match requests.with_value(|r| r.start()) {
                Ok(controller) => {
                    crate::live::shared::browser::claim(&operation, &fence, controller).await
                }
                Err(_) => Err(true),
            };
            if !account.with_untracked(|a| a.accepts(&fence)) {
                return;
            }
            let _ = state.try_update(|s| s.finish_claim(&fence, &operation, result));
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (state, account, requests);
}
