//! RAM-only session scope for shared catalog and operation-bound mailbox claims.
use super::{account::RequestFence, public_discovery::PublicDiscoveryState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimReceipt {
    pub operation_id: String,
    pub grants: Vec<ClaimGrant>,
    pub replayed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimGrant {
    pub work_id: String,
    pub grant_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ClaimPhase {
    #[default]
    Idle,
    Pending(String),
    Unknown(String),
    Done(ClaimReceipt),
    Failed,
}
#[derive(Clone, Default)]
pub struct SharedAccess {
    fence: Option<RequestFence>,
    pub catalog: PublicDiscoveryState,
    pub claim: ClaimPhase,
}
impl SharedAccess {
    pub fn observe(&mut self, fence: Option<RequestFence>) -> bool {
        if self.fence == fence {
            return false;
        }
        self.fence = fence;
        self.catalog.clear_shared();
        self.claim = ClaimPhase::Idle;
        true
    }
    pub fn fence(&self) -> Option<&RequestFence> {
        self.fence.as_ref()
    }
    pub fn accepts(&self, fence: &RequestFence) -> bool {
        self.fence.as_ref() == Some(fence)
    }
    pub fn visible_rows(
        &self,
        account: &super::account_view::AccountViewState,
    ) -> &[crate::contract::discovery::MetadataWorkSummary] {
        use super::public_discovery::{PublicDiscoveryPhase, Tail};
        if self.fence.as_ref().is_some_and(|f| account.accepts(f))
            && matches!(
                self.catalog.phase(),
                PublicDiscoveryPhase::Ready | PublicDiscoveryPhase::Incomplete
            )
            && !matches!(self.catalog.tail(), Tail::Failed(_) | Tail::Expired)
        {
            self.catalog.rows()
        } else {
            &[]
        }
    }
    pub fn begin_claim(&mut self, operation: String) -> Option<(RequestFence, String)> {
        let fence = self.fence.clone()?;
        let operation = match &self.claim {
            ClaimPhase::Pending(_) => return None,
            ClaimPhase::Unknown(previous) => previous.clone(),
            _ => operation,
        };
        crate::core::public_ids::canonical_uuid(&operation).filter(|id| !id.is_nil())?;
        self.claim = ClaimPhase::Pending(operation.clone());
        Some((fence, operation))
    }
    pub fn finish_claim(
        &mut self,
        fence: &RequestFence,
        operation: &str,
        result: Result<ClaimReceipt, bool>,
    ) -> bool {
        if !self.accepts(fence)
            || !matches!(&self.claim, ClaimPhase::Pending(id) if id == operation)
        {
            return false;
        }
        self.claim = match result {
            Ok(receipt)
                if receipt.operation_id == operation
                    && receipt.grants.len() <= 100
                    && receipt.grants.iter().all(|g| {
                        crate::core::public_ids::canonical_uuid(&g.work_id).is_some()
                            && crate::core::public_ids::canonical_uuid(&g.grant_id).is_some()
                    }) =>
            {
                ClaimPhase::Done(receipt)
            }
            Ok(_) | Err(true) => ClaimPhase::Unknown(operation.into()),
            Err(false) => ClaimPhase::Failed,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::account::{AccountScope, AccountState};
    fn enter(state: &mut AccountState, who: &str, n: u128) -> RequestFence {
        state
            .enter(
                AccountScope::from_session(who.into(), uuid::Uuid::from_u128(n).to_string())
                    .unwrap(),
            )
            .unwrap();
        state.fence().unwrap()
    }
    #[test]
    fn account_switch_discards_cursor_claim_receipt_and_every_late_answer_including_errors() {
        let mut account = AccountState::default();
        let old = enter(&mut account, "a", 1);
        let mut access = SharedAccess::default();
        access.observe(Some(old.clone()));
        let op = uuid::Uuid::from_u128(3).to_string();
        access.begin_claim(op.clone()).unwrap();
        let new = enter(&mut account, "b", 2);
        assert!(access.observe(Some(new.clone())));
        for result in [
            Ok(ClaimReceipt {
                operation_id: op.clone(),
                grants: vec![],
                replayed: false,
            }),
            Err(true),
            Err(false),
        ] {
            assert!(!access.finish_claim(&old, &op, result));
        }
        assert_eq!(access.claim, ClaimPhase::Idle);
        assert!(access.catalog.rows().is_empty());
        assert!(access.accepts(&new));
        access.observe(None);
        assert!(!access.accepts(&new));
    }
    #[test]
    fn repeated_pending_claim_is_suppressed_and_unknown_reuses_exact_operation() {
        let mut account = AccountState::default();
        let fence = enter(&mut account, "a", 1);
        let mut access = SharedAccess::default();
        access.observe(Some(fence.clone()));
        let op = uuid::Uuid::from_u128(3).to_string();
        access.begin_claim(op.clone()).unwrap();
        assert!(access
            .begin_claim(uuid::Uuid::from_u128(4).to_string())
            .is_none());
        assert!(access.finish_claim(&fence, &op, Err(true)));
        assert_eq!(
            access
                .begin_claim(uuid::Uuid::from_u128(4).to_string())
                .unwrap()
                .1,
            op
        );
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use crate::application::{
        account::AccountScope, account_view::AccountViewState, public_discovery::DiscoveryQuery,
    };
    #[test]
    fn old_private_rows_are_hidden_before_the_account_change_effect_can_run() {
        let mut account = AccountViewState::default();
        let issued = account.begin_refresh().unwrap();
        account
            .finish_session(
                issued,
                Ok(
                    AccountScope::from_session("a".into(), uuid::Uuid::from_u128(1).to_string())
                        .unwrap(),
                ),
                || uuid::Uuid::from_u128(3).to_string(),
            )
            .unwrap();
        let mut shared = SharedAccess::default();
        shared.observe(account.fence());
        let ticket = shared.catalog.begin(DiscoveryQuery::default()).unwrap();
        let page=serde_json::from_str(include_str!("../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/covered_works.json")).unwrap();
        shared.catalog.finish(&ticket, Ok(page));
        assert!(!shared.visible_rows(&account).is_empty());
        account.begin_refresh().unwrap();
        assert!(shared.visible_rows(&account).is_empty());
        // Same-session route exit invalidates catalog tickets monotonically, too.
        shared.catalog.clear_shared();
        let current = shared.catalog.begin(DiscoveryQuery::default()).unwrap();
        assert_ne!(ticket, current);
        assert_eq!(
            shared.catalog.finish(
                &ticket,
                Err(crate::application::public_discovery::DiscoveryReadError::Unavailable)
            ),
            crate::application::public_discovery::Step::Ignored
        );
    }
}
