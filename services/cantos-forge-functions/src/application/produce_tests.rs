//! The generation use cases against the mock provider and a fault-injecting attempt history.
//!
//! The first group mirrors the named runs of the Quint capability
//! `cantos.production.forge-attempt` (`behavior/forge_attempt.qnt`): each test
//! carries its run ID in a `Quint run:` line, and `tests/behavior_binding.rs` keeps the two
//! lists equal. A test checks the same observable facts the run asserts (attempts, provider
//! calls, spend, the recorded state) through production code; it does not replay the model.

use std::cell::{Cell, RefCell};

use proptest::prelude::*;

use super::{
    memory::{Fault, MemoryAttempts, MemoryStore},
    ports::{AttemptStore, AttemptStoreError, Provider, ProviderResult, Reserved, Store},
    produce::{ProduceError, Produced, Producer, Terms, MAX_PROVIDER_AUDIO_BYTES},
};
use crate::{
    adapters::mock_provider::MockProvider,
    domain::{
        attempt::{
            Acknowledgement, AttemptNumber, AttemptState, Cost, Event, NotOpen, Operation, Phase,
            Policy, ProviderAudio, Rejection, ReplayError, ReserveRefusal, SettleRefusal,
            Settlement, Status, UncertainReason,
        },
        digest::Sha256Digest,
        generation::{testing::request, GenerationRequest},
        label::{OperationKey, VoiceLabel},
    },
};

fn n(value: u32) -> AttemptNumber {
    AttemptNumber::new(value).unwrap()
}

fn key() -> OperationKey {
    OperationKey::parse("chapter-1-take").unwrap()
}

/// The policy of the Quint model: three attempts, a budget of three units.
fn terms(units: u64) -> Terms {
    Terms {
        policy: Policy::new(3, 3).unwrap(),
        cost: Cost::new(units).unwrap(),
    }
}

fn changed_request() -> GenerationRequest {
    GenerationRequest {
        voice: VoiceLabel::parse("Puck").unwrap(),
        ..request()
    }
}

fn take(text: &str) -> ProviderResult {
    ProviderResult::Audio(text.as_bytes().to_vec())
}

fn ack(attempt: u32) -> Option<Acknowledgement> {
    Some(Acknowledgement::of(n(attempt)))
}

struct Lab {
    attempts: MemoryAttempts,
    blobs: MemoryStore,
    provider: MockProvider,
}

impl Lab {
    fn new() -> Self {
        Self::scripted([])
    }

    fn scripted(results: impl IntoIterator<Item = ProviderResult>) -> Self {
        Self {
            attempts: MemoryAttempts::default(),
            blobs: MemoryStore::default(),
            provider: MockProvider::scripted(results),
        }
    }

    fn producer(&self) -> Producer<'_, MemoryAttempts, MemoryStore, MockProvider> {
        Producer {
            attempts: &self.attempts,
            blobs: &self.blobs,
            provider: &self.provider,
        }
    }

    fn generate(&self, units: u64) -> Result<Produced, ProduceError> {
        self.producer().generate(&key(), &request(), terms(units))
    }

    fn regenerate(
        &self,
        acknowledgement: Option<Acknowledgement>,
        units: u64,
    ) -> Result<Produced, ProduceError> {
        self.producer()
            .regenerate(&key(), &request(), acknowledgement, terms(units))
    }

    /// Every provider call so far, by attempt number.
    fn calls(&self) -> Vec<u32> {
        self.provider
            .calls()
            .into_iter()
            .map(AttemptNumber::get)
            .collect()
    }

    fn events(&self) -> Vec<Event> {
        self.attempts.log(&key())
    }

    fn operation(&self) -> Operation {
        Operation::replay(&self.events()).unwrap()
    }

    fn state(&self) -> AttemptState {
        self.operation().latest().state()
    }
}

fn executed(result: Result<Produced, ProduceError>) -> Status {
    match result {
        Ok(Produced::Executed(status)) => status,
        other => panic!("expected an executed attempt, found {other:?}"),
    }
}

fn recorded(result: Result<Produced, ProduceError>) -> Status {
    match result {
        Ok(Produced::Recorded(status)) => status,
        other => panic!("expected a recorded state, found {other:?}"),
    }
}

fn succeeded(status: &Status) -> ProviderAudio {
    match status.latest.state() {
        AttemptState::Succeeded(audio) => audio,
        other => panic!("expected a succeeded attempt, found {other:?}"),
    }
}

/// Quint run: cantos.production.forge-attempt.run.single-attempt-succeeds
#[test]
fn a_single_attempt_reserves_calls_once_and_keeps_the_exact_bytes() {
    let lab = Lab::new();
    let status = executed(lab.generate(1));
    let bytes = MockProvider::audio_for(&request());
    let audio = succeeded(&status);
    assert_eq!((status.attempts, status.spent), (1, 1));
    assert_eq!(audio.digest, Sha256Digest::of(&bytes));
    assert_eq!(audio.bytes.get(), u64::try_from(bytes.len()).unwrap());
    assert_eq!(lab.blobs.blob(audio.digest).unwrap(), bytes);
    assert_eq!(lab.calls(), [1]);
    assert_eq!(lab.events().len(), 2, "one reservation and one outcome");
}

/// Quint run: cantos.production.forge-attempt.run.retry-after-lost-response-replays
#[test]
fn a_retry_after_a_lost_response_returns_the_record_and_never_calls_again() {
    let lab = Lab::new();
    let first = executed(lab.generate(1));
    let appends = *lab.attempts.appends.borrow();
    for _ in 0..3 {
        assert_eq!(recorded(lab.generate(1)), first);
    }
    assert_eq!(lab.calls(), [1]);
    assert_eq!(lab.events().len(), 2);
    assert_eq!(
        *lab.attempts.appends.borrow(),
        appends,
        "a replay writes nothing"
    );
}

/// Quint run: cantos.production.forge-attempt.run.duplicate-while-open
#[test]
fn an_identical_request_that_loses_the_reservation_race_never_calls() {
    let lab = Lab::new();
    // The competitor's identical reservation lands first; this invocation's append conflicts.
    lab.attempts.plan([Some(Fault::LoseRace)]);
    let status = recorded(lab.generate(1));
    assert_eq!(status.latest.state(), AttemptState::Open);
    assert_eq!((status.attempts, status.spent), (1, 1));
    assert!(lab.calls().is_empty());
    assert_eq!(lab.events().len(), 1);
}

/// A competing writer wins between the first read and the conditional reservation append.
/// The readback may also be unavailable or corrupt; none grants the losing invocation a call.
struct ReservationRace {
    winner: Result<Vec<Event>, AttemptStoreError>,
    lost: Cell<bool>,
    appends: Cell<usize>,
}

impl ReservationRace {
    fn new(winner: Result<Vec<Event>, AttemptStoreError>) -> Self {
        Self {
            winner,
            lost: Cell::new(false),
            appends: Cell::new(0),
        }
    }
}

impl AttemptStore for ReservationRace {
    fn events(&self, _: &OperationKey) -> Result<Vec<Event>, AttemptStoreError> {
        if self.lost.get() {
            self.winner.clone()
        } else {
            Ok(vec![])
        }
    }

    fn append(&self, _: &OperationKey, index: usize, _: &Event) -> Result<(), AttemptStoreError> {
        assert_eq!(index, 0);
        self.appends.set(self.appends.get() + 1);
        self.lost.set(true);
        Err(AttemptStoreError::Conflict)
    }
}

fn opened(request: &GenerationRequest) -> Event {
    Event::Opened {
        key: key(),
        fingerprint: request.fingerprint(),
        cost: Cost::new(1).unwrap(),
    }
}

/// Quint run: cantos.production.forge-attempt.run.conflicting-request-while-open
#[test]
fn a_different_request_that_wins_the_reservation_race_conflicts_without_dispatch() {
    let winner = vec![opened(&changed_request())];
    let attempts = ReservationRace::new(Ok(winner.clone()));
    let blobs = MemoryStore::default();
    let provider = MockProvider::default();
    let producer = Producer {
        attempts: &attempts,
        blobs: &blobs,
        provider: &provider,
    };
    assert_eq!(
        producer.generate(&key(), &request(), terms(1)),
        Err(ProduceError::Conflict {
            recorded: changed_request().fingerprint(),
        })
    );
    assert_eq!(attempts.events(&key()), Ok(winner));
    assert_eq!(attempts.appends.get(), 1);
    assert!(provider.calls().is_empty());
    assert!(blobs.blobs.borrow().is_empty());
}

#[test]
fn an_untrustworthy_reservation_race_readback_never_dispatches_or_retries_the_append() {
    let unavailable = AttemptStoreError::Unavailable {
        operation: "events",
        detail: "injected race readback failure".to_owned(),
    };
    let foreign = OperationKey::parse("another-operation").unwrap();
    let cases = [
        (Err(unavailable.clone()), ProduceError::Store(unavailable)),
        (Ok(vec![]), ProduceError::UnknownOperation),
        (
            Ok(vec![Event::Settled {
                attempt: n(1),
                settlement: Settlement::Rejected(Rejection::Refused),
            }]),
            ProduceError::Corrupt(ReplayError::NotOpened),
        ),
        (
            Ok(vec![Event::Opened {
                key: foreign.clone(),
                fingerprint: request().fingerprint(),
                cost: Cost::new(1).unwrap(),
            }]),
            ProduceError::ForeignHistory { found: foreign },
        ),
    ];
    for (winner, expected) in cases {
        let attempts = ReservationRace::new(winner.clone());
        let blobs = MemoryStore::default();
        let provider = MockProvider::default();
        let producer = Producer {
            attempts: &attempts,
            blobs: &blobs,
            provider: &provider,
        };
        assert_eq!(
            producer.generate(&key(), &request(), terms(1)),
            Err(expected)
        );
        assert_eq!(attempts.events(&key()), winner);
        assert_eq!(attempts.appends.get(), 1);
        assert!(provider.calls().is_empty());
        assert!(blobs.blobs.borrow().is_empty());
    }
}

/// A settlement was written but its append response was lost. The next read is injected
/// separately, including histories that violate the store contract, to check fail-closed proof.
struct OutcomeReadback {
    inner: MemoryAttempts,
    readback: fn(Vec<Event>) -> Result<Vec<Event>, AttemptStoreError>,
    settled: Cell<bool>,
}

impl AttemptStore for OutcomeReadback {
    fn events(&self, key: &OperationKey) -> Result<Vec<Event>, AttemptStoreError> {
        let events = self.inner.events(key)?;
        if self.settled.get() {
            (self.readback)(events)
        } else {
            Ok(events)
        }
    }

    fn append(
        &self,
        key: &OperationKey,
        index: usize,
        event: &Event,
    ) -> Result<(), AttemptStoreError> {
        self.inner.append(key, index, event)?;
        if index == 1 {
            self.settled.set(true);
            Err(AttemptStoreError::Unavailable {
                operation: "append",
                detail: "lost outcome response".to_owned(),
            })
        } else {
            Ok(())
        }
    }
}

#[test]
fn a_matching_outcome_inside_untrusted_readback_does_not_prove_the_write() {
    type Readback = fn(Vec<Event>) -> Result<Vec<Event>, AttemptStoreError>;
    let cases: [(&str, Readback); 7] = [
        ("foreign key", |mut events| {
            if let Event::Opened { key, .. } = &mut events[0] {
                *key = OperationKey::parse("foreign-operation").unwrap();
            }
            Ok(events)
        }),
        ("changed request", |mut events| {
            if let Event::Opened { fingerprint, .. } = &mut events[0] {
                *fingerprint = changed_request().fingerprint();
            }
            Ok(events)
        }),
        ("changed cost", |mut events| {
            if let Event::Opened { cost, .. } = &mut events[0] {
                *cost = Cost::new(2).unwrap();
            }
            Ok(events)
        }),
        ("corrupt prefix", |mut events| {
            events[0] = events[1].clone();
            Ok(events)
        }),
        ("corrupt suffix", |mut events| {
            events.push(events[1].clone());
            Ok(events)
        }),
        ("missing history", |_| Ok(vec![])),
        ("unreadable history", |_| {
            Err(AttemptStoreError::Unavailable {
                operation: "events",
                detail: "lost readback response".to_owned(),
            })
        }),
    ];
    for (name, readback) in cases {
        let attempts = OutcomeReadback {
            inner: MemoryAttempts::default(),
            readback,
            settled: Cell::new(false),
        };
        let blobs = MemoryStore::default();
        let provider = MockProvider::default();
        let producer = Producer {
            attempts: &attempts,
            blobs: &blobs,
            provider: &provider,
        };
        assert_eq!(
            producer.generate(&key(), &request(), terms(1)),
            Err(ProduceError::OutcomeNotRecorded {
                attempt: n(1),
                error: AttemptStoreError::Unavailable {
                    operation: "append",
                    detail: "lost outcome response".to_owned(),
                },
            }),
            "{name} must not heal a lost outcome response",
        );
        assert_eq!(provider.calls(), [n(1)], "{name}");
        assert_eq!(*attempts.inner.appends.borrow(), 2, "{name}");
        assert_eq!(attempts.inner.log(&key()).len(), 2, "{name}");
    }
}

#[test]
fn a_lost_outcome_is_confirmed_even_when_a_valid_later_attempt_is_already_reserved() {
    let attempts = OutcomeReadback {
        inner: MemoryAttempts::default(),
        readback: |mut events| {
            events.push(Event::Regenerated {
                attempt: n(2),
                cost: Cost::new(1).unwrap(),
                acknowledged: None,
            });
            Ok(events)
        },
        settled: Cell::new(false),
    };
    let blobs = MemoryStore::default();
    let provider = MockProvider::default();
    let producer = Producer {
        attempts: &attempts,
        blobs: &blobs,
        provider: &provider,
    };
    let own_status = executed(producer.generate(&key(), &request(), terms(1)));
    assert_eq!((own_status.attempts, own_status.spent), (1, 1));
    let audio = succeeded(&own_status);
    let later = Operation::replay(&attempts.events(&key()).unwrap()).unwrap();
    assert_eq!(later.latest().state(), AttemptState::Open);
    assert_eq!((later.status().attempts, later.status().spent), (2, 2));
    assert_eq!(
        later.attempts().next().unwrap().state(),
        AttemptState::Succeeded(audio)
    );
    assert_eq!(provider.calls(), [n(1)]);
    assert_eq!(
        blobs.blob(audio.digest).unwrap(),
        MockProvider::audio_for(&request())
    );
    assert_eq!(*attempts.inner.appends.borrow(), 2);
}

/// Quint run: cantos.production.forge-attempt.run.conflicting-request
#[test]
fn the_same_key_with_another_request_conflicts_and_changes_nothing() {
    let lab = Lab::new();
    let first = executed(lab.generate(1));
    let before = lab.events();
    let result = lab
        .producer()
        .generate(&key(), &changed_request(), terms(1));
    assert_eq!(
        result,
        Err(ProduceError::Conflict {
            recorded: request().fingerprint()
        })
    );
    assert_eq!(lab.events(), before);
    assert_eq!(lab.calls(), [1]);
    assert_eq!(recorded(lab.generate(1)), first);
}

/// Quint run: cantos.production.forge-attempt.run.crash-before-dispatch
#[test]
fn a_reservation_whose_response_is_lost_never_calls_and_needs_a_decision() {
    let lab = Lab::new();
    lab.attempts.plan([Some(Fault::LoseResponse)]);
    let result = lab.generate(1);
    assert!(
        matches!(
            result,
            Err(ProduceError::NotReserved(
                AttemptStoreError::Unavailable { .. }
            ))
        ),
        "{result:?}"
    );
    assert!(lab.calls().is_empty());
    assert_eq!(lab.state(), AttemptState::Open);
    // A retry sees the stored reservation and cannot tell whether it owns it: it reports.
    assert_eq!(recorded(lab.generate(1)).latest.state(), AttemptState::Open);
    assert!(lab.calls().is_empty());
    let status = lab.producer().recover(&key()).unwrap();
    assert_eq!(
        status.latest.state(),
        AttemptState::Uncertain(UncertainReason::Abandoned)
    );
    assert_eq!(
        lab.regenerate(None, 1),
        Err(ProduceError::Refused(
            ReserveRefusal::UncertaintyUndecided { attempt: n(1) }
        ))
    );
    assert_eq!(
        recorded(lab.generate(1)).latest.state(),
        AttemptState::Uncertain(UncertainReason::Abandoned)
    );
    assert!(lab.calls().is_empty());
}

/// Quint run: cantos.production.forge-attempt.run.crash-after-dispatch
#[test]
fn a_call_whose_outcome_was_never_written_is_not_replayed() {
    let lab = Lab::new();
    // The reservation is written; the outcome write is refused, as if the process died after the call.
    lab.attempts.plan([None, Some(Fault::Refuse)]);
    let result = lab.generate(1);
    assert!(
        matches!(
            result,
            Err(ProduceError::OutcomeNotRecorded { attempt, error: AttemptStoreError::Unavailable { .. } })
                if attempt == n(1)
        ),
        "{result:?}"
    );
    assert_eq!(lab.calls(), [1]);
    for _ in 0..2 {
        assert_eq!(recorded(lab.generate(1)).latest.state(), AttemptState::Open);
    }
    assert_eq!(lab.calls(), [1], "no retry may call again");
    let status = lab.producer().recover(&key()).unwrap();
    assert_eq!(
        status.latest.state(),
        AttemptState::Uncertain(UncertainReason::Abandoned)
    );
    assert_eq!(
        lab.regenerate(None, 1),
        Err(ProduceError::Refused(
            ReserveRefusal::UncertaintyUndecided { attempt: n(1) }
        ))
    );
    assert_eq!(lab.calls(), [1]);
}

/// Quint run: cantos.production.forge-attempt.run.unknown-provider-outcome
#[test]
fn an_unknown_provider_outcome_is_recorded_and_moves_on_only_by_an_acknowledged_regeneration() {
    let lab = Lab::scripted([ProviderResult::Unknown(UncertainReason::TimedOut)]);
    let status = executed(lab.generate(1));
    assert_eq!(
        status.latest.state(),
        AttemptState::Uncertain(UncertainReason::TimedOut)
    );
    assert_eq!(
        lab.regenerate(None, 1),
        Err(ProduceError::Refused(
            ReserveRefusal::UncertaintyUndecided { attempt: n(1) }
        ))
    );
    let regenerated = executed(lab.regenerate(ack(1), 1));
    assert_eq!((regenerated.attempts, regenerated.spent), (2, 2));
    assert!(matches!(
        regenerated.latest.state(),
        AttemptState::Succeeded(_)
    ));
    assert_eq!(lab.calls(), [1, 2]);
}

/// Quint run: cantos.production.forge-attempt.run.stale-decision
#[test]
fn a_decision_about_an_earlier_attempt_grants_nothing_later() {
    let lab = Lab::scripted([ProviderResult::Rejected(Rejection::Refused)]);
    executed(lab.generate(1));
    // Attempt 2 is called and then its outcome write fails, so the operator marks it uncertain.
    lab.attempts.plan([None, Some(Fault::Refuse)]);
    assert!(matches!(
        lab.regenerate(None, 1),
        Err(ProduceError::OutcomeNotRecorded { .. })
    ));
    lab.producer().recover(&key()).unwrap();
    assert_eq!(
        lab.regenerate(ack(1), 1),
        Err(ProduceError::Refused(ReserveRefusal::StaleDecision {
            acknowledged: n(1),
            latest: n(2)
        }))
    );
    let status = executed(lab.regenerate(ack(2), 1));
    assert_eq!(status.attempts, 3);
    assert_eq!(lab.calls(), [1, 2, 3]);
}

/// Quint run: cantos.production.forge-attempt.run.regeneration-while-open
#[test]
fn a_regeneration_is_refused_while_an_attempt_has_no_outcome() {
    let lab = Lab::new();
    lab.attempts.plan([None, Some(Fault::Refuse)]);
    assert!(lab.generate(1).is_err());
    let open = Err(ProduceError::Refused(ReserveRefusal::InFlight {
        attempt: n(1),
    }));
    assert_eq!(lab.regenerate(None, 1), open);
    // A decision about the open attempt does not help: only a recorded outcome does.
    assert_eq!(lab.regenerate(ack(1), 1), open);
    assert_eq!(lab.calls(), [1]);
}

/// Quint run: cantos.production.forge-attempt.run.explicit-regeneration-keeps-result
#[test]
fn an_explicit_regeneration_adds_an_attempt_and_keeps_the_earlier_result() {
    let lab = Lab::scripted([take("take one"), take("take two")]);
    let first = succeeded(&executed(lab.generate(1)));
    let second = succeeded(&executed(lab.regenerate(None, 1)));
    assert_ne!(first.digest, second.digest);
    let operation = lab.operation();
    let kept: Vec<AttemptState> = operation
        .attempts()
        .map(|attempt| attempt.state())
        .collect();
    assert_eq!(
        kept,
        [
            AttemptState::Succeeded(first),
            AttemptState::Succeeded(second)
        ]
    );
    assert_eq!(lab.blobs.blob(first.digest).unwrap(), b"take one");
    assert_eq!(lab.blobs.blob(second.digest).unwrap(), b"take two");
    assert_eq!(operation.status().spent, 2);
}

/// Quint run: cantos.production.forge-attempt.run.budget-refusal
#[test]
fn spending_beyond_the_budget_is_refused_before_any_call() {
    let lab = Lab::new();
    executed(lab.generate(2));
    assert_eq!(
        lab.regenerate(None, 2),
        Err(ProduceError::Refused(ReserveRefusal::BudgetExhausted {
            spent: 2,
            cost: 2,
            budget: 3
        }))
    );
    assert_eq!(lab.calls(), [1]);
    assert_eq!(lab.events().len(), 2);
}

/// Quint run: cantos.production.forge-attempt.run.attempt-limit-refusal
#[test]
fn the_attempt_limit_is_refused_before_any_call() {
    let rejected = || ProviderResult::Rejected(Rejection::Refused);
    let lab = Lab::scripted([rejected(), rejected(), rejected()]);
    executed(lab.generate(1));
    executed(lab.regenerate(None, 1));
    executed(lab.regenerate(None, 1));
    assert_eq!(
        lab.regenerate(None, 1),
        Err(ProduceError::Refused(ReserveRefusal::AttemptLimit {
            limit: 3
        }))
    );
    assert_eq!(lab.calls(), [1, 2, 3]);
}

// Native-only boundaries the model abstracts away.

/// A provider that reads the durable history at the moment of the call.
struct Probe<'a> {
    attempts: &'a MemoryAttempts,
    seen: RefCell<Vec<(AttemptNumber, Vec<Event>)>>,
}

impl Provider for Probe<'_> {
    fn synthesize(&self, reserved: &Reserved, request: &GenerationRequest) -> ProviderResult {
        let events = self.attempts.log(&key());
        let operation = Operation::replay(&events).unwrap();
        assert_eq!(operation.latest().number(), reserved.attempt());
        assert_eq!(operation.latest().state(), AttemptState::Open);
        self.seen.borrow_mut().push((reserved.attempt(), events));
        ProviderResult::Audio(MockProvider::audio_for(request))
    }
}

#[test]
fn the_reservation_is_durable_before_the_provider_is_called() {
    let attempts = MemoryAttempts::default();
    let blobs = MemoryStore::default();
    let probe = Probe {
        attempts: &attempts,
        seen: RefCell::default(),
    };
    let producer = Producer {
        attempts: &attempts,
        blobs: &blobs,
        provider: &probe,
    };
    let first = executed(producer.generate(&key(), &request(), terms(1)));
    executed(producer.regenerate(&key(), &request(), None, terms(1)));
    let seen = probe.seen.borrow();
    assert_eq!(seen.len(), 2);
    assert_eq!((seen[0].0, seen[0].1.len()), (n(1), 1));
    assert_eq!(seen[0].1, [opened(&request())]);
    assert_eq!((seen[1].0, seen[1].1.len()), (n(2), 3));
    assert_eq!(
        seen[1].1,
        [
            opened(&request()),
            Event::Settled {
                attempt: n(1),
                settlement: Settlement::Succeeded(succeeded(&first)),
            },
            Event::Regenerated {
                attempt: n(2),
                cost: Cost::new(1).unwrap(),
                acknowledged: None,
            },
        ]
    );
}

#[test]
fn a_first_reservation_over_the_budget_writes_nothing_and_never_calls() {
    let lab = Lab::new();
    assert_eq!(
        lab.generate(4),
        Err(ProduceError::Refused(ReserveRefusal::BudgetExhausted {
            spent: 0,
            cost: 4,
            budget: 3
        }))
    );
    assert!(lab.calls().is_empty());
    assert!(lab.events().is_empty());
    assert_eq!(*lab.attempts.appends.borrow(), 0);
}

#[test]
fn an_unreadable_history_blocks_every_use_case_before_any_call() {
    let lab = Lab::new();
    *lab.attempts.unreadable.borrow_mut() = true;
    let error = |result: Result<Produced, ProduceError>| {
        assert!(matches!(result, Err(ProduceError::Store(_))), "{result:?}");
    };
    error(lab.generate(1));
    error(lab.regenerate(None, 1));
    assert!(matches!(
        lab.producer().recover(&key()),
        Err(ProduceError::Store(_))
    ));
    assert!(lab.calls().is_empty());
}

#[test]
fn a_history_no_decision_could_have_produced_is_refused_before_any_call() {
    let lab = Lab::new();
    lab.attempts.logs.borrow_mut().insert(
        key(),
        vec![Event::Settled {
            attempt: n(1),
            settlement: Settlement::Rejected(Rejection::Refused),
        }],
    );
    assert_eq!(
        lab.generate(1),
        Err(ProduceError::Corrupt(ReplayError::NotOpened))
    );
    assert!(lab.calls().is_empty());
}

#[test]
fn the_history_of_another_operation_is_refused() {
    let lab = Lab::new();
    let other = OperationKey::parse("another-operation").unwrap();
    lab.attempts.logs.borrow_mut().insert(
        key(),
        vec![Event::Opened {
            key: other.clone(),
            fingerprint: request().fingerprint(),
            cost: Cost::new(1).unwrap(),
        }],
    );
    assert_eq!(
        lab.generate(1),
        Err(ProduceError::ForeignHistory { found: other })
    );
    assert!(lab.calls().is_empty());
}

#[test]
fn corrupt_and_foreign_histories_block_every_use_case_without_mutating_or_dispatching() {
    let first = opened(&request());
    let settled = Event::Settled {
        attempt: n(1),
        settlement: Settlement::Rejected(Rejection::Refused),
    };
    let regenerated = Event::Regenerated {
        attempt: n(2),
        cost: Cost::new(1).unwrap(),
        acknowledged: None,
    };
    let other = OperationKey::parse("foreign-operation").unwrap();
    let cases = [
        (
            vec![settled.clone()],
            ProduceError::Corrupt(ReplayError::NotOpened),
        ),
        (
            vec![first.clone(), first.clone()],
            ProduceError::Corrupt(ReplayError::OpenedAgain { index: 1 }),
        ),
        (
            vec![first.clone(), regenerated.clone()],
            ProduceError::Corrupt(ReplayError::Reservation {
                index: 1,
                refusal: ReserveRefusal::InFlight { attempt: n(1) },
            }),
        ),
        (
            vec![first, settled.clone(), settled.clone()],
            ProduceError::Corrupt(ReplayError::Settlement {
                index: 2,
                refusal: SettleRefusal::NotOpen(NotOpen {
                    attempt: n(1),
                    phase: Phase::Rejected,
                }),
            }),
        ),
        (
            vec![
                Event::Opened {
                    key: key(),
                    fingerprint: request().fingerprint(),
                    cost: Cost::new(u64::MAX).unwrap(),
                },
                settled,
                regenerated,
            ],
            ProduceError::Corrupt(ReplayError::Reservation {
                index: 2,
                refusal: ReserveRefusal::BudgetExhausted {
                    spent: u64::MAX,
                    cost: 1,
                    budget: u64::MAX,
                },
            }),
        ),
        (
            vec![Event::Opened {
                key: other.clone(),
                fingerprint: request().fingerprint(),
                cost: Cost::new(1).unwrap(),
            }],
            ProduceError::ForeignHistory { found: other },
        ),
    ];
    for (history, expected) in cases {
        let lab = Lab::new();
        lab.attempts
            .logs
            .borrow_mut()
            .insert(key(), history.clone());
        assert_eq!(lab.generate(1), Err(expected.clone()));
        assert_eq!(lab.regenerate(None, 1), Err(expected.clone()));
        assert_eq!(lab.producer().recover(&key()), Err(expected));
        assert_eq!(lab.events(), history);
        assert_eq!(*lab.attempts.appends.borrow(), 0);
        assert!(lab.calls().is_empty());
        assert!(lab.blobs.blobs.borrow().is_empty());
    }
}

#[test]
fn regenerating_or_recovering_an_unknown_operation_is_refused() {
    let lab = Lab::new();
    assert_eq!(lab.regenerate(None, 1), Err(ProduceError::UnknownOperation));
    assert_eq!(
        lab.producer().recover(&key()),
        Err(ProduceError::UnknownOperation)
    );
    assert!(lab.calls().is_empty());
}

#[test]
fn regenerating_a_changed_request_conflicts_and_recovering_a_settled_attempt_is_refused() {
    let lab = Lab::new();
    executed(lab.generate(1));
    let changed = lab
        .producer()
        .regenerate(&key(), &changed_request(), None, terms(1));
    assert_eq!(
        changed,
        Err(ProduceError::Conflict {
            recorded: request().fingerprint()
        })
    );
    assert_eq!(
        lab.producer().recover(&key()),
        Err(ProduceError::Settle(SettleRefusal::NotOpen(NotOpen {
            attempt: n(1),
            phase: Phase::Succeeded
        })))
    );
    assert_eq!(lab.calls(), [1]);
}

#[test]
fn a_lost_outcome_response_is_healed_from_the_recorded_event() {
    let lab = Lab::new();
    lab.attempts.plan([None, Some(Fault::LoseResponse)]);
    let status = executed(lab.generate(1));
    assert!(matches!(status.latest.state(), AttemptState::Succeeded(_)));
    assert_eq!(lab.events().len(), 2);
    assert_eq!(lab.calls(), [1]);
}

#[test]
fn the_provider_response_limit_is_32_mib() {
    assert_eq!(MAX_PROVIDER_AUDIO_BYTES, 33_554_432);
    // The limit itself is usable; one byte more is not (see the test below).
    let lab = Lab::scripted([ProviderResult::Audio(vec![
        0;
        usize::try_from(
            MAX_PROVIDER_AUDIO_BYTES
        )
        .unwrap()
    ])]);
    let status = executed(lab.generate(1));
    assert_eq!(succeeded(&status).bytes.get(), MAX_PROVIDER_AUDIO_BYTES);
}

#[test]
fn provider_answers_that_are_not_usable_audio_are_rejected_not_kept() {
    let oversized = vec![0_u8; usize::try_from(MAX_PROVIDER_AUDIO_BYTES).unwrap() + 1];
    for response in [vec![], oversized] {
        let lab = Lab::scripted([ProviderResult::Audio(response)]);
        let status = executed(lab.generate(1));
        assert_eq!(
            status.latest.state(),
            AttemptState::Rejected(Rejection::InvalidAudio)
        );
        assert!(lab.blobs.blobs.borrow().is_empty());
    }
}

#[test]
fn a_result_forge_cannot_keep_is_uncertain_not_a_failure_and_needs_a_decision() {
    let lab = Lab::new();
    *lab.blobs.fail_next_blob.borrow_mut() = true;
    let status = executed(lab.generate(1));
    assert_eq!(
        status.latest.state(),
        AttemptState::Uncertain(UncertainReason::ResultNotRecorded)
    );
    assert_eq!(
        lab.regenerate(None, 1),
        Err(ProduceError::Refused(
            ReserveRefusal::UncertaintyUndecided { attempt: n(1) }
        ))
    );
    assert_eq!(lab.calls(), [1]);
}

#[test]
fn the_mock_is_deterministic_and_scripted_answers_come_first() {
    let lab = Lab::scripted([ProviderResult::Rejected(Rejection::RateLimited)]);
    let first = executed(lab.generate(1));
    assert_eq!(
        first.latest.state(),
        AttemptState::Rejected(Rejection::RateLimited)
    );
    let second = succeeded(&executed(lab.regenerate(None, 1)));
    assert_eq!(
        second.digest,
        Sha256Digest::of(&MockProvider::audio_for(&request()))
    );
    assert_ne!(
        MockProvider::audio_for(&request()),
        MockProvider::audio_for(&changed_request())
    );
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Generate,
    GenerateChanged,
    Regenerate { acknowledge: u8, units: u64 },
    Recover,
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => Just(Step::Generate),
        1 => Just(Step::GenerateChanged),
        3 => (0_u8..3, 1_u64..=2).prop_map(|(acknowledge, units)| Step::Regenerate { acknowledge, units }),
        1 => Just(Step::Recover),
    ]
}

fn fault() -> impl Strategy<Value = Option<Fault>> {
    prop_oneof![
        4 => Just(None),
        1 => Just(Some(Fault::Refuse)),
        1 => Just(Some(Fault::LoseResponse)),
        1 => Just(Some(Fault::LoseRace)),
    ]
}

fn provider_result() -> impl Strategy<Value = ProviderResult> {
    prop_oneof![
        2 => (0_u8..3).prop_map(|seed| take(&format!("audio {seed}"))),
        1 => Just(ProviderResult::Rejected(Rejection::Unavailable)),
        1 => Just(ProviderResult::Unknown(UncertainReason::ConnectionLost)),
    ]
}

proptest! {
    /// Under any mix of requests, regenerations, recoveries, refused or lost writes and provider
    /// answers: a provider call needs a durable reservation, one attempt makes at most one call,
    /// the history always replays, and spend and attempts stay inside the policy.
    #[test]
    fn no_fault_schedule_makes_a_second_call_for_one_attempt_or_overspends(
        steps in proptest::collection::vec((step(), proptest::collection::vec(fault(), 0..3)), 0..14),
        script in proptest::collection::vec(provider_result(), 0..24),
    ) {
        let lab = Lab::scripted(script);
        for (step, faults) in steps {
            lab.attempts.plan(faults);
            let _ = match step {
                Step::Generate => lab.generate(1).map(drop),
                Step::GenerateChanged => lab.producer().generate(&key(), &changed_request(), terms(1)).map(drop),
                Step::Regenerate { acknowledge, units } => {
                    let acknowledgement = match acknowledge {
                        0 => None,
                        1 => lab.attempts.logs.borrow().get(&key()).and_then(|log| Operation::replay(log).ok())
                            .map(|operation| Acknowledgement::of(operation.ledger().attempts())),
                        _ => ack(1),
                    };
                    lab.regenerate(acknowledgement, units).map(drop)
                }
                Step::Recover => lab.producer().recover(&key()).map(drop),
            };
            // Only the writes of the next step may be faulted, never a later step's.
            lab.attempts.faults.borrow_mut().clear();

            let calls = lab.calls();
            let mut distinct = calls.clone();
            distinct.sort_unstable();
            distinct.dedup();
            prop_assert_eq!(&distinct, &calls, "one attempt, one call");
            let log = lab.events();
            if log.is_empty() {
                prop_assert!(calls.is_empty(), "a call needs a durable reservation");
            } else {
                let operation = Operation::replay(&log);
                prop_assert!(operation.is_ok(), "history must replay: {:?}", operation);
                let operation = operation.unwrap();
                let reserved = operation.ledger().attempts().get();
                prop_assert!(calls.iter().all(|attempt| *attempt <= reserved));
                prop_assert!(operation.ledger().spent() <= 3 && reserved <= 3);
            }
        }
    }
}
