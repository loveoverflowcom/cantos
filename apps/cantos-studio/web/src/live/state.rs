//! Pure request ownership: acknowledgements never cross a context or generation.
use super::{ClientError, Command, Reply};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Pending,
    Ready,
    Failed,
    Ambiguous,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);
#[derive(Clone, Default)]
pub struct RequestState {
    generation: u64,
    actor: String,
    target: String,
    pending_actor: String,
    uncertain: bool,
    reconciling: bool,
    pub phase: Phase,
    pub pending: Option<Command>,
    pub result: Option<Reply>,
    pub error: Option<ClientError>,
}
impl RequestState {
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn input_failed(&self) -> Self {
        if self.phase == Phase::Pending || self.pending.is_some() {
            return self.clone();
        }
        Self {
            phase: Phase::Failed,
            error: Some(ClientError::InvalidInput),
            result: None,
            ..self.clone()
        }
    }
    pub fn actor(&self) -> &str {
        &self.actor
    }
    pub fn context(&self, actor: &str, target: &str) -> Option<Self> {
        if actor == self.actor && target == self.target {
            return Some(self.clone());
        }
        let unknown = self.pending.as_ref().is_some_and(Command::is_write);
        Some(Self {
            generation: self.generation.checked_add(1)?,
            actor: actor.into(),
            target: target.into(),
            phase: if unknown {
                Phase::Ambiguous
            } else {
                Phase::Idle
            },
            pending: if unknown { self.pending.clone() } else { None },
            pending_actor: self.pending_actor.clone(),
            uncertain: unknown,
            error: if unknown {
                Some(ClientError::Cancelled)
            } else {
                None
            },
            result: None,
            reconciling: false,
        })
    }

    pub fn begin_in_session(
        &self,
        command: Command,
        session_pending: bool,
    ) -> Option<(Self, Ticket)> {
        if session_pending {
            None
        } else {
            self.begin(command)
        }
    }
    pub fn begin(&self, command: Command) -> Option<(Self, Ticket)> {
        if self.actor.is_empty() || self.phase == Phase::Pending || self.pending.is_some() {
            return None;
        }
        let generation = self.generation.checked_add(1)?;
        Some((
            Self {
                generation,
                phase: Phase::Pending,
                pending: Some(command),
                pending_actor: self.actor.clone(),
                uncertain: false,
                result: None,
                error: None,
                ..self.clone()
            },
            Ticket(generation),
        ))
    }
    pub fn can_reconcile(&self) -> bool {
        self.can_retry()
            && self
                .pending
                .as_ref()
                .is_some_and(|c| c.reconciliation_read().is_some())
    }
    pub fn begin_reconcile(&self) -> Option<(Self, Ticket, Command)> {
        if !self.can_reconcile() {
            return None;
        }
        let command = self.pending.as_ref()?.reconciliation_read()?;
        let generation = self.generation.checked_add(1)?;
        Some((
            Self {
                generation,
                phase: Phase::Pending,
                reconciling: true,
                error: None,
                ..self.clone()
            },
            Ticket(generation),
            command,
        ))
    }
    pub fn is_reconciling(&self) -> bool {
        self.reconciling
    }
    pub fn can_retry(&self) -> bool {
        self.phase == Phase::Ambiguous
            && self.pending.is_some()
            && self.pending_actor == self.actor
            && !self.actor.is_empty()
    }
    pub fn retry(&self) -> Option<(Self, Ticket)> {
        if !self.can_retry() {
            return None;
        }
        let generation = self.generation.checked_add(1)?;
        Some((
            Self {
                generation,
                phase: Phase::Pending,
                reconciling: false,
                error: None,
                ..self.clone()
            },
            Ticket(generation),
        ))
    }
    pub fn is_empty(&self) -> bool {
        self.phase == Phase::Ready
            && match &self.result {
                Some(Reply::Works(page)) => page.items.is_empty(),
                Some(Reply::Units(page)) => page.items.is_empty(),
                Some(Reply::History(history)) => history.revisions.is_empty(),
                _ => false,
            }
    }
    pub fn accepts(&self, ticket: Ticket) -> bool {
        ticket.0 == self.generation && self.phase == Phase::Pending
    }
    pub fn finish(&self, ticket: Ticket, result: Result<Reply, ClientError>) -> Self {
        if ticket.0 != self.generation || self.phase != Phase::Pending {
            return self.clone();
        }
        let result = match result {
            Ok(reply)
                if self.reconciling
                    && !self
                        .pending
                        .as_ref()
                        .is_some_and(|command| command.reconciles(&reply, &self.pending_actor)) =>
            {
                Err(ClientError::ReceiptNotEstablished)
            }
            result => result,
        };
        let result = match result {
            Ok(reply)
                if self.pending.as_ref().is_some_and(|command| {
                    !reply.acknowledges_actor(command, &self.pending_actor)
                }) =>
            {
                Err(ClientError::SessionChanged)
            }
            result => result,
        };
        match result {
            Ok(reply) => Self {
                phase: Phase::Ready,
                result: Some(reply),
                pending: None,
                uncertain: false,
                reconciling: false,
                error: None,
                ..self.clone()
            },
            Err(error) => {
                // GET is safe to start anew. Writes with no authoritative rejection retain
                // the exact command; a cancelled fetch never establishes server rollback.
                let ambiguous = (self.uncertain || error.ambiguous())
                    && self.pending.as_ref().is_some_and(Command::is_write);
                Self {
                    uncertain: ambiguous,
                    reconciling: false,
                    phase: if ambiguous {
                        Phase::Ambiguous
                    } else {
                        Phase::Failed
                    },
                    pending: if ambiguous {
                        self.pending.clone()
                    } else {
                        None
                    },
                    result: None,
                    error: Some(error),
                    ..self.clone()
                }
            }
        }
    }
    pub fn cancel(&self) -> Option<Self> {
        let generation = self.generation.checked_add(1)?;
        let ambiguous = self.pending.as_ref().is_some_and(Command::is_write);
        Some(Self {
            generation,
            uncertain: ambiguous,
            reconciling: false,
            phase: if ambiguous {
                Phase::Ambiguous
            } else {
                Phase::Idle
            },
            result: None,
            error: if ambiguous {
                Some(ClientError::Cancelled)
            } else {
                None
            },
            pending: if ambiguous {
                self.pending.clone()
            } else {
                None
            },
            ..self.clone()
        })
    }
}
