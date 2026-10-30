//! Account lifecycle and intent isolation. No credentials or backend policy live here.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountScope {
    actor: String,
    session: String,
    presentation: Option<crate::contract::account_presentation::AccountPresentation>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountError {
    InvalidScope,
    Exhausted,
    SignedOut,
    WrongAccount,
    DuplicateIntent,
    LogoutPending,
    RequestPending,
}
impl AccountScope {
    pub fn from_session(actor: String, session: String) -> Result<Self, AccountError> {
        if actor.is_empty()
            || actor.len() > 128
            || actor.chars().any(char::is_control)
            || uuid::Uuid::parse_str(&session).is_err()
        {
            return Err(AccountError::InvalidScope);
        }
        Ok(Self {
            actor,
            session,
            presentation: None,
        })
    }
    pub fn with_presentation(
        mut self,
        presentation: crate::contract::account_presentation::AccountPresentation,
    ) -> Self {
        self.presentation = presentation.valid().then_some(presentation);
        self
    }
    pub fn presentation(
        &self,
    ) -> Option<&crate::contract::account_presentation::AccountPresentation> {
        self.presentation.as_ref()
    }
    pub fn actor(&self) -> &str {
        &self.actor
    }
    pub fn session(&self) -> &str {
        &self.session
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestFence {
    generation: u64,
    scope: AccountScope,
}
impl RequestFence {
    pub fn session(&self) -> &str {
        self.scope.session()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountGeneration(u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovery {
    SendIntent,
    ReadReceiptFirst,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingIntent {
    owner: String,
    operation: String,
    body: Vec<u8>,
    recovery: Recovery,
}
impl PendingIntent {
    pub fn operation(&self) -> &str {
        &self.operation
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn recovery(&self) -> Recovery {
        self.recovery
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccountState {
    generation: u64,
    scope: Option<AccountScope>,
    projection: Option<Vec<u8>>,
    pending: BTreeMap<String, PendingIntent>,
    logout_pending: bool,
}
impl AccountState {
    #[cfg(test)]
    pub(super) fn set_generation_for_test(&mut self, generation: u64) {
        self.generation = generation;
    }
    pub fn scope(&self) -> Option<&AccountScope> {
        self.scope.as_ref()
    }
    pub fn logout_pending(&self) -> bool {
        self.logout_pending
    }
    pub fn begin_logout(&mut self) -> Result<AccountGeneration, AccountError> {
        self.leave()?;
        self.logout_pending = true;
        Ok(self.generation())
    }
    pub fn finish_logout(&mut self, issued: AccountGeneration) -> bool {
        if issued != self.generation() || !self.logout_pending {
            return false;
        }
        self.logout_pending = false;
        true
    }
    /// A server-observed cookie rotation cancels cleanup, without granting a new account.
    pub fn observe_session_change(&mut self, issued: AccountGeneration) -> bool {
        if issued != self.generation() || !self.logout_pending {
            return false;
        }
        self.logout_pending = false;
        true
    }
    pub fn generation(&self) -> AccountGeneration {
        AccountGeneration(self.generation)
    }
    pub fn finish_session(
        &mut self,
        issued: AccountGeneration,
        scope: AccountScope,
    ) -> Result<bool, AccountError> {
        if issued != self.generation() {
            return Ok(false);
        }
        self.enter(scope)?;
        Ok(true)
    }
    /// Hide old projections before any asynchronous logout or new sign-in starts.
    pub fn leave(&mut self) -> Result<(), AccountError> {
        let next = self.generation.checked_add(1);
        self.scope = None;
        self.projection = None;
        let Some(next) = next else {
            // No new request identity can be issued. Cancel cleanup too, so a previous
            // MAX-generation completion cannot acknowledge a retry that never started.
            self.logout_pending = false;
            return Err(AccountError::Exhausted);
        };
        self.generation = next;
        Ok(())
    }
    pub fn enter(&mut self, scope: AccountScope) -> Result<(), AccountError> {
        if self.logout_pending {
            return Err(AccountError::LogoutPending);
        }
        self.leave()?;
        self.scope = Some(scope);
        Ok(())
    }
    pub fn fence(&self) -> Result<RequestFence, AccountError> {
        Ok(RequestFence {
            generation: self.generation,
            scope: self.scope.clone().ok_or(AccountError::SignedOut)?,
        })
    }
    pub fn accepts(&self, fence: &RequestFence) -> bool {
        self.generation == fence.generation && self.scope.as_ref() == Some(&fence.scope)
    }
    pub fn project(&mut self, fence: &RequestFence, body: Vec<u8>) -> bool {
        if !self.accepts(fence) {
            return false;
        }
        self.projection = Some(body);
        true
    }
    pub fn projection(&self) -> Option<&[u8]> {
        self.projection.as_deref()
    }
    pub fn enqueue(&mut self, operation: String, body: Vec<u8>) -> Result<(), AccountError> {
        let actor = &self.scope.as_ref().ok_or(AccountError::SignedOut)?.actor;
        if uuid::Uuid::parse_str(&operation).is_err() {
            return Err(AccountError::InvalidScope);
        }
        if self.pending.contains_key(&operation) {
            return Err(AccountError::DuplicateIntent);
        }
        self.pending.insert(
            operation.clone(),
            PendingIntent {
                owner: actor.clone(),
                operation,
                body,
                recovery: Recovery::SendIntent,
            },
        );
        Ok(())
    }
    /// A stable body/operation survives unknown outcomes, but never changes account owner.
    pub fn dispatch(
        &mut self,
        operation: &str,
    ) -> Result<(RequestFence, PendingIntent), AccountError> {
        let fence = self.fence()?;
        let intent = self
            .pending
            .get_mut(operation)
            .ok_or(AccountError::InvalidScope)?;
        if intent.owner != fence.scope.actor {
            return Err(AccountError::WrongAccount);
        }
        let outgoing = intent.clone();
        intent.recovery = Recovery::ReadReceiptFirst;
        Ok((fence, outgoing))
    }
    pub fn acknowledge(&mut self, fence: &RequestFence, operation: &str) -> bool {
        if !self.accepts(fence) {
            return false;
        }
        if self
            .pending
            .get(operation)
            .is_none_or(|intent| intent.owner != fence.scope.actor)
        {
            return false;
        }
        self.pending.remove(operation);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope(actor: &str, session: u128) -> AccountScope {
        AccountScope::from_session(actor.into(), uuid::Uuid::from_u128(session).to_string())
            .unwrap()
    }
    #[test]
    fn logout_switch_and_reauthentication_fence_every_old_callback_and_keep_original_intent() {
        let mut state = AccountState::default();
        let operation = uuid::Uuid::from_u128(3).to_string();
        state.enter(scope("A", 1)).unwrap();
        let old = state.fence().unwrap();
        assert!(state.project(&old, b"private A".to_vec()));
        state
            .enqueue(operation.clone(), b"save original body".to_vec())
            .unwrap();
        let (_, first) = state.dispatch(&operation).unwrap();
        assert_eq!(first.recovery(), Recovery::SendIntent);
        state.leave().unwrap();
        assert_eq!(state.projection(), None);
        state.enter(scope("B", 2)).unwrap();
        assert!(!state.project(&old, b"late success OR failure".to_vec()));
        assert!(!state.acknowledge(&old, &operation));
        assert_eq!(
            state.dispatch(&operation).err(),
            Some(AccountError::WrongAccount)
        );
        state.enter(scope("A", 4)).unwrap();
        let (current, recovered) = state.dispatch(&operation).unwrap();
        assert_eq!(recovered.body(), b"save original body");
        assert_eq!(recovered.recovery(), Recovery::ReadReceiptFirst);
        assert!(!state.acknowledge(&old, &operation));
        assert!(state.acknowledge(&current, &operation));
    }
    #[test]
    fn bounded_switch_schedules_never_admit_a_previous_session() {
        for switches in 1..64 {
            let mut state = AccountState::default();
            state.enter(scope("A", 1)).unwrap();
            let old = state.fence().unwrap();
            for n in 0..switches {
                state
                    .enter(scope(if n % 2 == 0 { "B" } else { "A" }, n + 2))
                    .unwrap();
            }
            assert!(!state.accepts(&old));
        }
    }
    #[test]
    fn delayed_signed_out_session_lookup_cannot_restore_account_after_logout() {
        let mut state = AccountState::default();
        let issued = state.generation();
        state.leave().unwrap();
        assert_eq!(state.finish_session(issued, scope("A", 1)), Ok(false));
        assert_eq!(state.fence(), Err(AccountError::SignedOut));
    }
    #[test]
    fn new_login_waits_for_acknowledged_cookie_cleanup_and_unknown_logout_stays_closed() {
        let mut state = AccountState::default();
        state.enter(scope("A", 1)).unwrap();
        let first = state.begin_logout().unwrap();
        assert_eq!(state.enter(scope("B", 2)), Err(AccountError::LogoutPending));
        let retry = state.begin_logout().unwrap();
        assert!(!state.finish_logout(first));
        assert!(state.logout_pending());
        assert!(state.finish_logout(retry));
        state.enter(scope("B", 2)).unwrap();
    }
    #[test]
    fn exhausted_security_invalidations_purge_without_reissuing_a_generation() {
        for action in 0..4 {
            let mut state = AccountState::default();
            state.enter(scope("A", 1)).unwrap();
            state.set_generation_for_test(u64::MAX);
            let old = state.fence().unwrap();
            state.project(&old, vec![1, 2]);
            let result = match action {
                0 => state.leave(),
                1 => state.enter(scope("B", 2)),
                2 => state.begin_logout().map(|_| ()),
                3 => state
                    .finish_session(state.generation(), scope("B", 2))
                    .map(|_| ()),
                _ => unreachable!(),
            };
            assert_eq!(result, Err(AccountError::Exhausted));
            assert_eq!(state.generation(), AccountGeneration(u64::MAX));
            assert_eq!(state.fence(), Err(AccountError::SignedOut));
            assert_eq!(state.projection(), None);
            assert!(!state.accepts(&old));
            assert!(!state.finish_logout(state.generation()));
        }
        let mut state = AccountState::default();
        state.begin_logout().unwrap();
        let before = state.clone();
        assert_eq!(state.enter(scope("A", 1)), Err(AccountError::LogoutPending));
        assert_eq!(state, before);
    }
}
