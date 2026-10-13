//! Account lifecycle and intent isolation. No credentials or backend policy live here.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountScope {
    actor: String,
    session: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountError {
    InvalidScope,
    Exhausted,
    SignedOut,
    WrongAccount,
    DuplicateIntent,
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
        Ok(Self { actor, session })
    }
    pub fn actor(&self) -> &str {
        &self.actor
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestFence {
    generation: u64,
    scope: AccountScope,
}
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
#[derive(Default)]
pub struct AccountState {
    generation: u64,
    scope: Option<AccountScope>,
    projection: Option<Vec<u8>>,
    pending: BTreeMap<String, PendingIntent>,
}
impl AccountState {
    /// Hide old projections before any asynchronous logout or new sign-in starts.
    pub fn leave(&mut self) -> Result<(), AccountError> {
        self.scope = None;
        self.projection = None;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(AccountError::Exhausted)?;
        Ok(())
    }
    pub fn enter(&mut self, scope: AccountScope) -> Result<(), AccountError> {
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
}
