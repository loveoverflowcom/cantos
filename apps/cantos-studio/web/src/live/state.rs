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

/// The original immutable intent survives retries and receipt reads unchanged.
#[derive(Clone)]
struct Intent {
    actor: String,
    command: Command,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Attempt {
    Fresh,
    Replay,
    ReceiptRead,
}

/// A completed result and an in-flight intent cannot be represented together.
#[derive(Clone, Default)]
enum Exchange {
    #[default]
    Idle,
    Pending {
        intent: Intent,
        attempt: Attempt,
    },
    Ready(Reply),
    Failed(ClientError),
    Ambiguous {
        intent: Intent,
        error: ClientError,
    },
}

/// Phase and results are read-only projections of one exchange authority.
///
/// ```compile_fail
/// use cantos_studio::live::{Phase, RequestState};
/// let mut state = RequestState::default();
/// state.phase = Phase::Ready;
/// ```
#[derive(Clone, Default)]
pub struct RequestState {
    generation: u64,
    actor: String,
    target: String,
    exchange: Exchange,
}
impl RequestState {
    pub fn phase(&self) -> Phase {
        match self.exchange {
            Exchange::Idle => Phase::Idle,
            Exchange::Pending { .. } => Phase::Pending,
            Exchange::Ready(_) => Phase::Ready,
            Exchange::Failed(_) => Phase::Failed,
            Exchange::Ambiguous { .. } => Phase::Ambiguous,
        }
    }
    fn intent(&self) -> Option<&Intent> {
        match &self.exchange {
            Exchange::Pending { intent, .. } | Exchange::Ambiguous { intent, .. } => Some(intent),
            Exchange::Idle | Exchange::Ready(_) | Exchange::Failed(_) => None,
        }
    }
    pub fn pending(&self) -> Option<&Command> {
        self.intent().map(|intent| &intent.command)
    }
    pub fn result(&self) -> Option<&Reply> {
        match &self.exchange {
            Exchange::Ready(reply) => Some(reply),
            Exchange::Idle
            | Exchange::Pending { .. }
            | Exchange::Failed(_)
            | Exchange::Ambiguous { .. } => None,
        }
    }
    pub fn error(&self) -> Option<&ClientError> {
        match &self.exchange {
            Exchange::Failed(error) | Exchange::Ambiguous { error, .. } => Some(error),
            Exchange::Idle | Exchange::Pending { .. } | Exchange::Ready(_) => None,
        }
    }
    pub fn completion(&self) -> Option<Result<&Reply, &ClientError>> {
        match &self.exchange {
            Exchange::Ready(reply) => Some(Ok(reply)),
            Exchange::Failed(error) | Exchange::Ambiguous { error, .. } => Some(Err(error)),
            Exchange::Idle | Exchange::Pending { .. } => None,
        }
    }
    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn actor(&self) -> &str {
        &self.actor
    }
    pub fn input_failed(&self) -> Self {
        if self.intent().is_some() {
            return self.clone();
        }
        Self {
            exchange: Exchange::Failed(ClientError::InvalidInput),
            ..self.clone()
        }
    }
    pub fn context(&self, actor: &str, target: &str) -> Option<Self> {
        if actor == self.actor && target == self.target {
            return Some(self.clone());
        }
        Some(Self {
            generation: self.generation.checked_add(1)?,
            actor: actor.into(),
            target: target.into(),
            exchange: self.interrupted_exchange(),
        })
    }
    fn interrupted_exchange(&self) -> Exchange {
        match self.intent().filter(|intent| intent.command.is_write()) {
            Some(intent) => Exchange::Ambiguous {
                intent: intent.clone(),
                error: ClientError::Cancelled,
            },
            None => Exchange::Idle,
        }
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
        if self.actor.is_empty() || self.intent().is_some() {
            return None;
        }
        self.start_attempt(
            Intent {
                actor: self.actor.clone(),
                command,
            },
            Attempt::Fresh,
        )
    }
    fn start_attempt(&self, intent: Intent, attempt: Attempt) -> Option<(Self, Ticket)> {
        let generation = self.generation.checked_add(1)?;
        Some((
            Self {
                generation,
                exchange: Exchange::Pending { intent, attempt },
                ..self.clone()
            },
            Ticket(generation),
        ))
    }
    pub fn can_retry(&self) -> bool {
        match &self.exchange {
            Exchange::Ambiguous { intent, .. } => {
                intent.actor == self.actor && !self.actor.is_empty()
            }
            Exchange::Idle
            | Exchange::Pending { .. }
            | Exchange::Ready(_)
            | Exchange::Failed(_) => false,
        }
    }
    pub fn retry(&self) -> Option<(Self, Ticket)> {
        if !self.can_retry() {
            return None;
        }
        self.start_attempt(self.intent()?.clone(), Attempt::Replay)
    }
    pub fn can_reconcile(&self) -> bool {
        self.can_retry()
            && self
                .pending()
                .is_some_and(|command| command.reconciliation_read().is_some())
    }
    pub fn begin_reconcile(&self) -> Option<(Self, Ticket, Command)> {
        if !self.can_reconcile() {
            return None;
        }
        let command = self.pending()?.reconciliation_read()?;
        let (state, ticket) = self.start_attempt(self.intent()?.clone(), Attempt::ReceiptRead)?;
        Some((state, ticket, command))
    }
    pub fn is_empty(&self) -> bool {
        match self.result() {
            Some(Reply::Works(page)) => page.items.is_empty(),
            Some(Reply::Units(page)) => page.items.is_empty(),
            Some(Reply::History(history)) => history.revisions.is_empty(),
            _ => false,
        }
    }
    pub fn accepts(&self, ticket: Ticket) -> bool {
        ticket.0 == self.generation && matches!(self.exchange, Exchange::Pending { .. })
    }
    pub fn finish(&self, ticket: Ticket, result: Result<Reply, ClientError>) -> Self {
        if !self.accepts(ticket) {
            return self.clone();
        }
        let Exchange::Pending { intent, attempt } = &self.exchange else {
            return self.clone();
        };
        let result = result.and_then(|reply| {
            if *attempt == Attempt::ReceiptRead && !intent.command.reconciles(&reply, &intent.actor)
            {
                return Err(ClientError::ReceiptNotEstablished);
            }
            if !reply.acknowledges_actor(&intent.command, &intent.actor) {
                return Err(ClientError::SessionChanged);
            }
            if *attempt != Attempt::ReceiptRead {
                intent.command.check_reply(&reply)?;
            }
            Ok(reply)
        });
        let exchange = match result {
            Ok(reply) => Exchange::Ready(reply),
            Err(error)
                if intent.command.is_write()
                    && (*attempt != Attempt::Fresh || error.ambiguous()) =>
            {
                Exchange::Ambiguous {
                    intent: intent.clone(),
                    error,
                }
            }
            Err(error) => Exchange::Failed(error),
        };
        Self {
            exchange,
            ..self.clone()
        }
    }
    pub fn cancel(&self) -> Option<Self> {
        Some(Self {
            generation: self.generation.checked_add(1)?,
            exchange: self.interrupted_exchange(),
            ..self.clone()
        })
    }
}
