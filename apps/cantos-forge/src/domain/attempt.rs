//! Producer-side generation attempts: when a billable provider call may start, and what a
//! recorded outcome permits next.
//!
//! A provider call can cost money and cannot be undone, so the rules here are about *not*
//! calling again. An operation is created by the reservation of its first attempt; the shell
//! persists that reservation **before** it starts the one provider call the attempt may make.
//! A recorded outcome is a confirmed success, a confirmed rejection or an *uncertain* result
//! (timeout, lost response, an outcome that could not be written, an owner that vanished).
//! A retry of the same request returns the recorded state; a changed request under the same
//! key conflicts; a new attempt exists only through an explicit regeneration, which is refused
//! while an attempt is open, after an uncertain attempt unless the operator's decision names
//! exactly that attempt, and beyond the operator's attempt or spending limits.
//!
//! Everything here is a pure decision over plain facts. [`Ledger`] is the small integer kernel
//! (phase, attempt count and reserved spend) that carries these rules; [`Operation`] adds the
//! recorded identity and per-attempt facts around it. The model is
//! `specs/behavior/production/forge_attempt.qnt` (capability
//! `cantos.production.forge-attempt`); a bounded proof of the kernel lives at the end of this
//! file. Neither says anything about a provider, a store or a clock: the application starts the
//! call only after an append succeeded, and settles it with exactly one recorded outcome.

use std::{
    fmt,
    num::{NonZeroU32, NonZeroU64},
};

use super::{digest::Sha256Digest, generation::RequestFingerprint, label::OperationKey};

/// The number of an attempt within one operation, counted from 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttemptNumber(NonZeroU32);

impl AttemptNumber {
    pub const FIRST: Self = Self(NonZeroU32::MIN);

    pub fn new(value: u32) -> Option<Self> {
        NonZeroU32::new(value).map(Self)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl fmt::Display for AttemptNumber {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

/// The operator-estimated spend one attempt reserves, in the operator's own units. Never zero:
/// an attempt that reserves nothing could not be bounded by a budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cost(NonZeroU64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZeroCost;

impl Cost {
    pub fn new(units: u64) -> Result<Self, ZeroCost> {
        NonZeroU64::new(units).map(Self).ok_or(ZeroCost)
    }

    pub fn units(self) -> u64 {
        self.0.get()
    }
}

/// The server-controlled limits of one operation. A request cannot raise them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    max_attempts: NonZeroU32,
    budget: NonZeroU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyError {
    NoAttempts,
    NoBudget,
}

impl Policy {
    /// Limits applied while *replaying* recorded history: replay re-checks structure only,
    /// because a limit is a decision made when the event was written, and a later, stricter
    /// policy must not make an honest log unreadable.
    const UNBOUNDED: Self = Self {
        max_attempts: NonZeroU32::MAX,
        budget: NonZeroU64::MAX,
    };

    pub fn new(max_attempts: u32, budget: u64) -> Result<Self, PolicyError> {
        Ok(Self {
            max_attempts: NonZeroU32::new(max_attempts).ok_or(PolicyError::NoAttempts)?,
            budget: NonZeroU64::new(budget).ok_or(PolicyError::NoBudget)?,
        })
    }

    pub fn max_attempts(self) -> u32 {
        self.max_attempts.get()
    }

    pub fn budget(self) -> u64 {
        self.budget.get()
    }
}

/// Where the latest attempt stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Reserved, with no recorded outcome. Whoever wins the reservation owns the one call.
    Open,
    Succeeded,
    Rejected,
    /// The effect is unknown: the call may have been made, billed and even completed.
    Uncertain,
}

/// A recorded outcome class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Rejected,
    Uncertain,
}

impl From<Outcome> for Phase {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Succeeded => Self::Succeeded,
            Outcome::Rejected => Self::Rejected,
            Outcome::Uncertain => Self::Uncertain,
        }
    }
}

/// Why a reservation (the first, or a regeneration) was refused. In order of precedence: an
/// attempt is open, an uncertain one has no decision (or a stale one), a limit is reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReserveRefusal {
    /// The latest attempt has no recorded outcome.
    InFlight {
        attempt: AttemptNumber,
    },
    /// The latest attempt is uncertain and no operator decision was presented.
    UncertaintyUndecided {
        attempt: AttemptNumber,
    },
    /// A decision was presented, but for an attempt other than the latest uncertain one.
    StaleDecision {
        acknowledged: AttemptNumber,
        latest: AttemptNumber,
    },
    AttemptLimit {
        limit: u32,
    },
    BudgetExhausted {
        spent: u64,
        cost: u64,
        budget: u64,
    },
}

/// A settlement was refused because the latest attempt already has an outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotOpen {
    pub attempt: AttemptNumber,
    pub phase: Phase,
}

/// The integer kernel of an operation: the latest attempt's phase, the attempt count and the
/// spend reserved so far. A `Ledger` can only be produced by [`Ledger::open`] and moved by
/// [`Ledger::reserve`] and [`Ledger::settle`], so an attempt that exceeds the policy, a spend
/// that wraps or a retry that skips a decision is not a value this type can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ledger {
    attempts: AttemptNumber,
    spent: u64,
    phase: Phase,
}

impl Ledger {
    /// The first reservation. An attempt limit of at least one always admits it; only the
    /// budget can refuse.
    pub fn open(cost: Cost, policy: Policy) -> Result<Self, ReserveRefusal> {
        if cost.units() > policy.budget() {
            return Err(ReserveRefusal::BudgetExhausted {
                spent: 0,
                cost: cost.units(),
                budget: policy.budget(),
            });
        }
        Ok(Self {
            attempts: AttemptNumber::FIRST,
            spent: cost.units(),
            phase: Phase::Open,
        })
    }

    /// A new attempt after the latest one settled. `acknowledged` is the operator's decision to
    /// move past an uncertain attempt; it is ignored for any other phase and never grants
    /// anything beyond naming exactly that attempt.
    pub fn reserve(
        self,
        cost: Cost,
        acknowledged: Option<AttemptNumber>,
        policy: Policy,
    ) -> Result<Self, ReserveRefusal> {
        match (self.phase, acknowledged) {
            (Phase::Open, _) => {
                return Err(ReserveRefusal::InFlight {
                    attempt: self.attempts,
                })
            }
            (Phase::Uncertain, None) => {
                return Err(ReserveRefusal::UncertaintyUndecided {
                    attempt: self.attempts,
                })
            }
            (Phase::Uncertain, Some(named)) if named != self.attempts => {
                return Err(ReserveRefusal::StaleDecision {
                    acknowledged: named,
                    latest: self.attempts,
                })
            }
            _ => {}
        }
        let limit = policy.max_attempts();
        let next = self
            .attempts
            .get()
            .checked_add(1)
            .filter(|next| *next <= limit)
            .and_then(AttemptNumber::new)
            .ok_or(ReserveRefusal::AttemptLimit { limit })?;
        let spent = self
            .spent
            .checked_add(cost.units())
            .filter(|spent| *spent <= policy.budget())
            .ok_or(ReserveRefusal::BudgetExhausted {
                spent: self.spent,
                cost: cost.units(),
                budget: policy.budget(),
            })?;
        Ok(Self {
            attempts: next,
            spent,
            phase: Phase::Open,
        })
    }

    /// Records the outcome of the latest attempt, once.
    pub fn settle(self, outcome: Outcome) -> Result<Self, NotOpen> {
        if self.phase != Phase::Open {
            return Err(NotOpen {
                attempt: self.attempts,
                phase: self.phase,
            });
        }
        Ok(Self {
            phase: outcome.into(),
            ..self
        })
    }

    pub fn attempts(self) -> AttemptNumber {
        self.attempts
    }

    pub fn spent(self) -> u64 {
        self.spent
    }

    pub fn phase(self) -> Phase {
        self.phase
    }
}

/// Why a provider confirmed that no usable result exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    /// The attempt was closed before any provider call was made.
    NotAttempted,
    /// The provider declined the content or the request.
    Refused,
    RateLimited,
    /// The model or service is not available.
    Unavailable,
    /// The provider answered, but not with audio Forge can use.
    InvalidAudio,
}

/// Why an attempt's effect is unknown. None of these is a failure and none permits a replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UncertainReason {
    TimedOut,
    ConnectionLost,
    /// The provider returned a result that Forge could not keep or record.
    ResultNotRecorded,
    /// The owning invocation vanished before recording any outcome.
    Abandoned,
}

/// Exact facts about the raw bytes a provider returned. These bytes are not a Cantos
/// rendition: encoding to a profile and inspection are later, separate steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderAudio {
    pub digest: Sha256Digest,
    pub bytes: NonZeroU64,
}

/// What was recorded for an attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Settlement {
    Succeeded(ProviderAudio),
    Rejected(Rejection),
    Uncertain(UncertainReason),
}

impl Settlement {
    fn outcome(self) -> Outcome {
        match self {
            Self::Succeeded(_) => Outcome::Succeeded,
            Self::Rejected(_) => Outcome::Rejected,
            Self::Uncertain(_) => Outcome::Uncertain,
        }
    }
}

/// One attempt as recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptState {
    Open,
    Succeeded(ProviderAudio),
    Rejected(Rejection),
    Uncertain(UncertainReason),
}

impl From<Settlement> for AttemptState {
    fn from(settlement: Settlement) -> Self {
        match settlement {
            Settlement::Succeeded(audio) => Self::Succeeded(audio),
            Settlement::Rejected(reason) => Self::Rejected(reason),
            Settlement::Uncertain(reason) => Self::Uncertain(reason),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptRecord {
    number: AttemptNumber,
    cost: Cost,
    state: AttemptState,
}

impl AttemptRecord {
    pub fn number(&self) -> AttemptNumber {
        self.number
    }

    pub fn cost(&self) -> Cost {
        self.cost
    }

    pub fn state(&self) -> AttemptState {
        self.state
    }
}

/// An operator's explicit decision to move past one uncertain attempt, for example by
/// regenerating even though the provider may already have billed it. It names that attempt: a
/// decision about an earlier attempt grants nothing later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acknowledgement(AttemptNumber);

impl Acknowledgement {
    /// Construct only where an operator decision for exactly `attempt` was actually collected.
    pub fn of(attempt: AttemptNumber) -> Self {
        Self(attempt)
    }

    pub fn attempt(self) -> AttemptNumber {
        self.0
    }
}

/// One write-once fact of an operation's history. The shell appends these in order; a store
/// refuses to write the same position twice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The first reservation: it creates the operation and pins its request.
    Opened {
        key: OperationKey,
        fingerprint: RequestFingerprint,
        cost: Cost,
    },
    /// An explicit new attempt, with the decision that allowed it after an uncertain one.
    Regenerated {
        attempt: AttemptNumber,
        cost: Cost,
        acknowledged: Option<AttemptNumber>,
    },
    Settled {
        attempt: AttemptNumber,
        settlement: Settlement,
    },
}

/// A settlement was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettleRefusal {
    /// Only the latest attempt can be settled.
    NotLatest {
        attempt: AttemptNumber,
        latest: AttemptNumber,
    },
    NotOpen(NotOpen),
}

/// Recorded history that no sequence of decisions could have produced. Reading fails closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    Empty,
    NotOpened,
    OpenedAgain {
        index: usize,
    },
    OutOfOrder {
        index: usize,
        expected: AttemptNumber,
        found: AttemptNumber,
    },
    Reservation {
        index: usize,
        refusal: ReserveRefusal,
    },
    Settlement {
        index: usize,
        refusal: SettleRefusal,
    },
}

/// The latest attempt and the totals, as returned to a caller that retries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub latest: AttemptRecord,
    pub attempts: u32,
    pub spent: u64,
}

/// An operation's recorded state: its key, the request it pinned, every attempt and the ledger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operation {
    key: OperationKey,
    fingerprint: RequestFingerprint,
    ledger: Ledger,
    earlier: Vec<AttemptRecord>,
    latest: AttemptRecord,
    applied: usize,
}

impl Operation {
    /// Rebuilds an operation from its history, re-checking every step with the same kernel the
    /// decisions use. Limits are not re-checked, only structure.
    pub fn replay(events: &[Event]) -> Result<Self, ReplayError> {
        let (first, rest) = events.split_first().ok_or(ReplayError::Empty)?;
        let Event::Opened {
            key,
            fingerprint,
            cost,
        } = first
        else {
            return Err(ReplayError::NotOpened);
        };
        let ledger = Ledger::open(*cost, Policy::UNBOUNDED)
            .map_err(|refusal| ReplayError::Reservation { index: 0, refusal })?;
        let mut operation = Self {
            key: key.clone(),
            fingerprint: *fingerprint,
            ledger,
            earlier: Vec::new(),
            latest: AttemptRecord {
                number: AttemptNumber::FIRST,
                cost: *cost,
                state: AttemptState::Open,
            },
            applied: 1,
        };
        for event in rest {
            operation = operation.apply(event)?;
        }
        Ok(operation)
    }

    /// The operation after `event` was appended at [`Operation::next_index`].
    pub fn apply(&self, event: &Event) -> Result<Self, ReplayError> {
        let index = self.applied;
        let mut next = self.clone();
        match event {
            Event::Opened { .. } => return Err(ReplayError::OpenedAgain { index }),
            Event::Regenerated {
                attempt,
                cost,
                acknowledged,
            } => {
                let ledger = self
                    .ledger
                    .reserve(*cost, *acknowledged, Policy::UNBOUNDED)
                    .map_err(|refusal| ReplayError::Reservation { index, refusal })?;
                if *attempt != ledger.attempts() {
                    return Err(ReplayError::OutOfOrder {
                        index,
                        expected: ledger.attempts(),
                        found: *attempt,
                    });
                }
                next.earlier.push(self.latest);
                next.latest = AttemptRecord {
                    number: *attempt,
                    cost: *cost,
                    state: AttemptState::Open,
                };
                next.ledger = ledger;
            }
            Event::Settled {
                attempt,
                settlement,
            } => {
                let refusal = |refusal| ReplayError::Settlement { index, refusal };
                if *attempt != self.ledger.attempts() {
                    return Err(refusal(SettleRefusal::NotLatest {
                        attempt: *attempt,
                        latest: self.ledger.attempts(),
                    }));
                }
                next.ledger = self
                    .ledger
                    .settle(settlement.outcome())
                    .map_err(|error| refusal(SettleRefusal::NotOpen(error)))?;
                next.latest.state = (*settlement).into();
            }
        }
        next.applied += 1;
        Ok(next)
    }

    pub fn key(&self) -> &OperationKey {
        &self.key
    }

    pub fn fingerprint(&self) -> RequestFingerprint {
        self.fingerprint
    }

    pub fn ledger(&self) -> Ledger {
        self.ledger
    }

    /// Every attempt in order, the latest last.
    pub fn attempts(&self) -> impl Iterator<Item = &AttemptRecord> {
        self.earlier.iter().chain(std::iter::once(&self.latest))
    }

    pub fn latest(&self) -> AttemptRecord {
        self.latest
    }

    /// The position the next event must be appended at: how many events were applied.
    pub fn next_index(&self) -> usize {
        self.applied
    }

    pub fn status(&self) -> Status {
        Status {
            latest: self.latest,
            attempts: self.ledger.attempts().get(),
            spent: self.ledger.spent(),
        }
    }
}

/// What the shell found for a key.
#[derive(Clone, Copy, Debug)]
pub enum Lookup<'a> {
    Absent(&'a OperationKey),
    Found(&'a Operation),
}

/// The decision for a request that names an operation key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Submission {
    /// Append this event; only after the append succeeds may the one provider call be made.
    Reserve(Event),
    /// The same key and the same request: the recorded state. Nothing is reserved or called.
    Replay(Status),
    /// The same key with a different request. Nothing is reserved or called.
    Conflict {
        recorded: RequestFingerprint,
    },
    Refused(ReserveRefusal),
}

/// Decides what a request for `lookup`'s key means. A retry is recognized by the fingerprint
/// alone; policy and cost estimates may differ between retries without making a new decision.
pub fn submit(
    lookup: Lookup<'_>,
    fingerprint: RequestFingerprint,
    cost: Cost,
    policy: Policy,
) -> Submission {
    match lookup {
        Lookup::Absent(key) => match Ledger::open(cost, policy) {
            Ok(_) => Submission::Reserve(Event::Opened {
                key: key.clone(),
                fingerprint,
                cost,
            }),
            Err(refusal) => Submission::Refused(refusal),
        },
        Lookup::Found(operation) if operation.fingerprint != fingerprint => Submission::Conflict {
            recorded: operation.fingerprint,
        },
        Lookup::Found(operation) => Submission::Replay(operation.status()),
    }
}

/// Decides an explicit regeneration. It is never a retry: a retry is [`submit`] with the same
/// fingerprint.
pub fn regenerate(
    operation: &Operation,
    acknowledgement: Option<Acknowledgement>,
    cost: Cost,
    policy: Policy,
) -> Result<Event, ReserveRefusal> {
    let acknowledged = acknowledgement.map(Acknowledgement::attempt);
    let next = operation.ledger.reserve(cost, acknowledged, policy)?;
    Ok(Event::Regenerated {
        attempt: next.attempts(),
        cost,
        acknowledged,
    })
}

/// Decides the one recorded outcome of `attempt`.
pub fn settle(
    operation: &Operation,
    attempt: AttemptNumber,
    settlement: Settlement,
) -> Result<Event, SettleRefusal> {
    let latest = operation.ledger.attempts();
    if attempt != latest {
        return Err(SettleRefusal::NotLatest { attempt, latest });
    }
    operation
        .ledger
        .settle(settlement.outcome())
        .map_err(SettleRefusal::NotOpen)?;
    Ok(Event::Settled {
        attempt,
        settlement,
    })
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    fn fixed_cost(units: u64) -> Cost {
        Cost::new(units).unwrap_or(Cost(NonZeroU64::MIN))
    }

    /// One symbolic step from `before`: either a settlement of a symbolic outcome or a
    /// reservation with a symbolic cost and a symbolic (or absent) decision. The result is
    /// checked against the rules restated here with widened arithmetic, not copied code.
    fn step(before: Ledger, policy: Policy) -> Ledger {
        let choice: u8 = kani::any();
        if choice % 4 < 3 {
            let outcome = match choice % 4 {
                0 => Outcome::Succeeded,
                1 => Outcome::Rejected,
                _ => Outcome::Uncertain,
            };
            return match before.settle(outcome) {
                Ok(after) => {
                    assert!(before.phase() == Phase::Open);
                    assert!(after.phase() == Phase::from(outcome));
                    assert!(
                        after.attempts() == before.attempts() && after.spent() == before.spent()
                    );
                    after
                }
                Err(refusal) => {
                    assert!(before.phase() != Phase::Open);
                    assert!(
                        refusal.attempt == before.attempts() && refusal.phase == before.phase()
                    );
                    before
                }
            };
        }
        let cost = fixed_cost(kani::any());
        let named: u32 = kani::any();
        let acknowledged = if kani::any() {
            AttemptNumber::new(named)
        } else {
            None
        };
        let latest = before.attempts();
        let widened = u128::from(before.spent()) + u128::from(cost.units());
        let phase_allows = match before.phase() {
            Phase::Open => false,
            Phase::Uncertain => acknowledged == Some(latest),
            Phase::Succeeded | Phase::Rejected => true,
        };
        let within = latest.get() < policy.max_attempts() && widened <= u128::from(policy.budget());
        match before.reserve(cost, acknowledged, policy) {
            Ok(after) => {
                assert!(phase_allows && within);
                assert!(after.phase() == Phase::Open);
                assert!(u64::from(after.attempts().get()) == u64::from(latest.get()) + 1);
                assert!(u128::from(after.spent()) == widened);
                assert!(after.spent() <= policy.budget());
                kani::cover!(
                    before.phase() == Phase::Uncertain,
                    "reserved after an uncertain attempt with its exact decision"
                );
                kani::cover!(
                    u128::from(after.spent()) == u128::from(policy.budget()),
                    "reserved exactly up to the budget"
                );
                after
            }
            Err(refusal) => {
                assert!(!(phase_allows && within));
                match refusal {
                    ReserveRefusal::InFlight { attempt } => {
                        assert!(before.phase() == Phase::Open && attempt == latest);
                        kani::cover!(true, "refused while the attempt is open");
                    }
                    ReserveRefusal::UncertaintyUndecided { attempt } => {
                        assert!(
                            before.phase() == Phase::Uncertain
                                && acknowledged.is_none()
                                && attempt == latest
                        );
                        kani::cover!(true, "refused: uncertain attempt without a decision");
                    }
                    ReserveRefusal::StaleDecision {
                        acknowledged: named,
                        latest: reported,
                    } => {
                        assert!(
                            before.phase() == Phase::Uncertain
                                && acknowledged == Some(named)
                                && named != latest
                                && reported == latest
                        );
                        kani::cover!(true, "refused: decision names another attempt");
                    }
                    ReserveRefusal::AttemptLimit { limit } => {
                        assert!(
                            phase_allows
                                && latest.get() >= policy.max_attempts()
                                && limit == policy.max_attempts()
                        );
                        kani::cover!(true, "refused at the attempt limit");
                    }
                    ReserveRefusal::BudgetExhausted {
                        spent,
                        cost: refused,
                        budget,
                    } => {
                        assert!(
                            phase_allows
                                && latest.get() < policy.max_attempts()
                                && widened > u128::from(budget)
                                && spent == before.spent()
                                && refused == cost.units()
                                && budget == policy.budget()
                        );
                        kani::cover!(
                            widened <= u128::from(u64::MAX),
                            "refused by the budget within u64"
                        );
                        kani::cover!(
                            widened > u128::from(u64::MAX),
                            "refused where the reserved sum exceeds u64"
                        );
                    }
                }
                before
            }
        }
    }

    /// Facts true after every step, whatever it was.
    fn invariants(before: Ledger, after: Ledger, policy: Policy) {
        assert!(after.spent() >= before.spent());
        assert!(after.spent() <= policy.budget());
        assert!(after.attempts().get() <= policy.max_attempts());
        let added = after.attempts().get() - before.attempts().get();
        assert!(added <= 1);
        if added == 1 {
            assert!(after.phase() == Phase::Open && before.phase() != Phase::Open);
            assert!(after.spent() > before.spent());
        } else {
            assert!(after.spent() == before.spent());
            assert!(before.phase() == after.phase() || before.phase() == Phase::Open);
        }
    }

    /// Proposition: from any admitted first reservation, every four-step sequence of
    /// settlements and reservations keeps spend and attempts inside the policy, never wraps,
    /// reserves only after a settled (or explicitly acknowledged uncertain) attempt, and refuses
    /// for exactly the stated reason. The factories refuse exactly zero.
    /// Domain: independent full symbolic u32 attempt limit, u64 budget and costs, symbolic
    /// outcomes and decisions. Assumptions/stubs: none; a zero cost is mapped to one only after
    /// the factory's exact refusal is asserted. Unwind: 2; no loops (steps are unrolled).
    /// Calls the production `Policy`, `Cost`, `Ledger::{open,reserve,settle}` kernel that every
    /// operation decision and replay uses. It proves nothing about the fingerprint, a store, a
    /// provider, a clock or the billable call itself.
    #[kani::proof]
    #[kani::unwind(2)]
    fn attempt_ledger_traces_never_overspend_or_blindly_retry() {
        let max_attempts: u32 = kani::any();
        let budget: u64 = kani::any();
        let first: u64 = kani::any();
        let (Ok(policy), Ok(first_cost)) = (Policy::new(max_attempts, budget), Cost::new(first))
        else {
            assert!(
                Policy::new(max_attempts, budget).is_err() == (max_attempts == 0 || budget == 0)
            );
            assert!(Cost::new(first).is_err() == (first == 0));
            return;
        };
        assert!(matches!(Cost::new(first), Ok(cost) if cost.units() == first));
        let mut ledger = match Ledger::open(first_cost, policy) {
            Ok(ledger) => ledger,
            Err(refusal) => {
                assert!(first > budget);
                assert!(
                    refusal
                        == ReserveRefusal::BudgetExhausted {
                            spent: 0,
                            cost: first,
                            budget
                        }
                );
                kani::cover!(true, "first reservation refused by the budget");
                return;
            }
        };
        assert!(first <= budget);
        assert!(ledger.attempts() == AttemptNumber::FIRST && ledger.phase() == Phase::Open);
        assert!(ledger.spent() == first);
        let before = ledger;
        ledger = step(ledger, policy);
        invariants(before, ledger, policy);
        let before = ledger;
        ledger = step(ledger, policy);
        invariants(before, ledger, policy);
        let before = ledger;
        ledger = step(ledger, policy);
        invariants(before, ledger, policy);
        let before = ledger;
        ledger = step(ledger, policy);
        invariants(before, ledger, policy);
    }
}
