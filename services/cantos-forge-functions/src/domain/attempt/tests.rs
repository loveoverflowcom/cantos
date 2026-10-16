use proptest::prelude::*;

use super::*;
use crate::domain::digest::Sha256Digest;

fn n(value: u32) -> AttemptNumber {
    AttemptNumber::new(value).unwrap()
}

fn cost(units: u64) -> Cost {
    Cost::new(units).unwrap()
}

fn policy(max_attempts: u32, budget: u64) -> Policy {
    Policy::new(max_attempts, budget).unwrap()
}

fn key() -> OperationKey {
    OperationKey::parse("chapter-1-take").unwrap()
}

fn fingerprint(seed: u8) -> RequestFingerprint {
    RequestFingerprint::from_digest(Sha256Digest::of(&[seed]))
}

fn audio() -> ProviderAudio {
    ProviderAudio {
        digest: Sha256Digest::of(b"provider bytes"),
        bytes: NonZeroU64::new(14).unwrap(),
    }
}

/// The policy of the Quint model: three attempts, a budget of three units.
const MODEL: fn() -> Policy = || policy(3, 3);

fn open(units: u64) -> Event {
    Event::Opened {
        key: key(),
        fingerprint: fingerprint(1),
        cost: cost(units),
    }
}

fn settled(attempt: u32, settlement: Settlement) -> Event {
    Event::Settled {
        attempt: n(attempt),
        settlement,
    }
}

fn regenerated(attempt: u32, units: u64, acknowledged: Option<u32>) -> Event {
    Event::Regenerated {
        attempt: n(attempt),
        cost: cost(units),
        acknowledged: acknowledged.map(n),
    }
}

fn operation(events: &[Event]) -> Operation {
    Operation::replay(events).unwrap()
}

#[test]
fn factories_refuse_exactly_zero() {
    assert_eq!(Cost::new(0), Err(ZeroCost));
    assert_eq!(Cost::new(1).map(Cost::units), Ok(1));
    assert_eq!(Cost::new(u64::MAX).map(Cost::units), Ok(u64::MAX));
    assert_eq!(Policy::new(0, 5), Err(PolicyError::NoAttempts));
    assert_eq!(Policy::new(0, 0), Err(PolicyError::NoAttempts));
    assert_eq!(Policy::new(2, 0), Err(PolicyError::NoBudget));
    let limits = Policy::new(2, 5).unwrap();
    assert_eq!((limits.max_attempts(), limits.budget()), (2, 5));
    assert_eq!(AttemptNumber::new(0), None);
    assert_eq!(AttemptNumber::new(1), Some(AttemptNumber::FIRST));
}

#[test]
fn attempt_numbers_are_written_as_their_value() {
    assert_eq!(AttemptNumber::FIRST.to_string(), "1");
    assert_eq!(n(12).to_string(), "12");
}

#[test]
fn the_first_reservation_is_refused_only_by_the_budget() {
    let ledger = Ledger::open(cost(3), MODEL()).unwrap();
    assert_eq!(
        (ledger.attempts(), ledger.spent(), ledger.phase()),
        (AttemptNumber::FIRST, 3, Phase::Open)
    );
    assert_eq!(
        Ledger::open(cost(4), MODEL()),
        Err(ReserveRefusal::BudgetExhausted {
            spent: 0,
            cost: 4,
            budget: 3
        })
    );
    // One attempt allowed is enough for the first reservation.
    assert!(Ledger::open(cost(1), policy(1, 1)).is_ok());
}

fn ledger_in(phase: Phase) -> Ledger {
    let open = Ledger::open(cost(1), MODEL()).unwrap();
    match phase {
        Phase::Open => open,
        Phase::Succeeded => open.settle(Outcome::Succeeded).unwrap(),
        Phase::Rejected => open.settle(Outcome::Rejected).unwrap(),
        Phase::Uncertain => open.settle(Outcome::Uncertain).unwrap(),
    }
}

#[test]
fn a_reservation_is_refused_in_the_documented_precedence() {
    let at_limit = {
        let two = ledger_in(Phase::Rejected)
            .reserve(cost(1), None, MODEL())
            .unwrap();
        let three = two
            .settle(Outcome::Rejected)
            .unwrap()
            .reserve(cost(1), None, MODEL())
            .unwrap();
        three.settle(Outcome::Rejected).unwrap()
    };
    assert_eq!(at_limit.attempts(), n(3));
    let cases: Vec<(&str, Result<Ledger, ReserveRefusal>)> = vec![
        (
            "open attempt",
            ledger_in(Phase::Open).reserve(cost(1), Some(n(1)), MODEL()),
        ),
        (
            "uncertain, no decision",
            ledger_in(Phase::Uncertain).reserve(cost(1), None, MODEL()),
        ),
        (
            "uncertain, stale decision",
            ledger_in(Phase::Uncertain).reserve(cost(1), Some(n(2)), MODEL()),
        ),
        (
            "attempt limit before budget",
            at_limit.reserve(cost(9), None, MODEL()),
        ),
        (
            "budget",
            ledger_in(Phase::Succeeded).reserve(cost(3), None, MODEL()),
        ),
        (
            "budget on wrapping sum",
            ledger_in(Phase::Succeeded).reserve(cost(u64::MAX), None, policy(3, u64::MAX)),
        ),
    ];
    let expected = [
        ReserveRefusal::InFlight { attempt: n(1) },
        ReserveRefusal::UncertaintyUndecided { attempt: n(1) },
        ReserveRefusal::StaleDecision {
            acknowledged: n(2),
            latest: n(1),
        },
        ReserveRefusal::AttemptLimit { limit: 3 },
        ReserveRefusal::BudgetExhausted {
            spent: 1,
            cost: 3,
            budget: 3,
        },
        ReserveRefusal::BudgetExhausted {
            spent: 1,
            cost: u64::MAX,
            budget: u64::MAX,
        },
    ];
    for ((name, result), expected) in cases.into_iter().zip(expected) {
        assert_eq!(result, Err(expected), "{name}");
    }
}

#[test]
fn a_new_attempt_follows_a_settled_one_or_an_exactly_acknowledged_uncertain_one() {
    for (phase, acknowledged) in [
        (Phase::Succeeded, None),
        (Phase::Rejected, None),
        (Phase::Uncertain, Some(n(1))),
        // A decision is ignored when it is not needed; it never widens anything.
        (Phase::Succeeded, Some(n(7))),
    ] {
        let next = ledger_in(phase)
            .reserve(cost(2), acknowledged, MODEL())
            .unwrap();
        assert_eq!(
            (next.attempts(), next.spent(), next.phase()),
            (n(2), 3, Phase::Open),
            "{phase:?}"
        );
    }
    // Exactly the budget is allowed; one unit more is not.
    assert!(ledger_in(Phase::Succeeded)
        .reserve(cost(2), None, MODEL())
        .is_ok());
}

#[test]
fn an_outcome_is_recorded_once() {
    for (outcome, phase) in [
        (Outcome::Succeeded, Phase::Succeeded),
        (Outcome::Rejected, Phase::Rejected),
        (Outcome::Uncertain, Phase::Uncertain),
    ] {
        let settled = ledger_in(Phase::Open).settle(outcome).unwrap();
        assert_eq!(settled.phase(), phase);
        for again in [Outcome::Succeeded, Outcome::Rejected, Outcome::Uncertain] {
            assert_eq!(
                settled.settle(again),
                Err(NotOpen {
                    attempt: n(1),
                    phase
                }),
                "{phase:?} then {again:?}"
            );
        }
    }
}

fn edge_u64() -> impl Strategy<Value = u64> {
    prop_oneof![
        1 => Just(1_u64),
        1 => Just(u64::MAX),
        1 => Just(u64::MAX - 1),
        4 => 1_u64..,
    ]
}

fn edge_u32() -> impl Strategy<Value = u32> {
    prop_oneof![
        1 => Just(1_u32),
        1 => Just(u32::MAX),
        1 => Just(u32::MAX - 1),
        4 => 1_u32..,
    ]
}

proptest! {
    /// The same proposition Kani checks over full-width integers, sampled here against an
    /// independently written specification with widened arithmetic. The boundaries (exact
    /// budget fit, the last attempt, the maximum integers) are drawn on purpose.
    #[test]
    fn reserve_matches_an_independent_specification(
        spent_before in edge_u64(),
        attempts in edge_u32(),
        max_attempts in edge_u32(),
        budget in edge_u64(),
        units in edge_u64(),
        fit_budget_exactly in any::<bool>(),
        last_attempt in any::<bool>(),
        phase in 0_u8..4,
        decision in 0_u8..3,
        other in edge_u32(),
    ) {
        let phase = [Phase::Open, Phase::Succeeded, Phase::Rejected, Phase::Uncertain][usize::from(phase)];
        let budget = if fit_budget_exactly { spent_before.saturating_add(units) } else { budget };
        let max_attempts = if last_attempt { attempts.saturating_add(1) } else { max_attempts };
        let acknowledged = match decision {
            0 => None,
            1 => Some(n(attempts)),
            _ => Some(n(other)),
        };
        let ledger = ledger_with(attempts, spent_before, phase);
        let policy = policy(max_attempts, budget);
        let result = ledger.reserve(cost(units), acknowledged, policy);

        let widened = u128::from(spent_before) + u128::from(units);
        let allowed = match phase {
            Phase::Open => false,
            Phase::Uncertain => acknowledged == Some(n(attempts)),
            Phase::Succeeded | Phase::Rejected => true,
        };
        let within = attempts < max_attempts && widened <= u128::from(budget);
        prop_assert_eq!(result.is_ok(), allowed && within);
        match result {
            Ok(next) => {
                prop_assert_eq!(next.attempts().get(), attempts + 1);
                prop_assert_eq!(u128::from(next.spent()), widened);
                prop_assert_eq!(next.phase(), Phase::Open);
            }
            Err(ReserveRefusal::AttemptLimit { limit }) => {
                prop_assert!(allowed && attempts >= max_attempts && limit == max_attempts);
            }
            Err(ReserveRefusal::BudgetExhausted { spent, cost: refused, budget: limit }) => {
                prop_assert!(allowed && attempts < max_attempts && widened > u128::from(budget));
                prop_assert_eq!((spent, refused, limit), (spent_before, units, budget));
            }
            Err(_) => prop_assert!(!allowed),
        }
    }
}

/// A ledger at an arbitrary attempt count and spend. `reserve` must be total over every value
/// the type can hold, so the property below builds them directly instead of through `open`.
fn ledger_with(attempts: u32, spent: u64, phase: Phase) -> Ledger {
    Ledger {
        attempts: n(attempts),
        spent,
        phase,
    }
}

#[test]
fn replay_rebuilds_the_recorded_state() {
    let events = [
        open(1),
        settled(1, Settlement::Uncertain(UncertainReason::TimedOut)),
        regenerated(2, 2, Some(1)),
        settled(2, Settlement::Succeeded(audio())),
    ];
    let operation = operation(&events);
    assert_eq!(operation.key(), &key());
    assert_eq!(operation.fingerprint(), fingerprint(1));
    assert_eq!(operation.next_index(), 4);
    assert_eq!(
        operation.status(),
        Status {
            latest: AttemptRecord {
                number: n(2),
                cost: cost(2),
                state: AttemptState::Succeeded(audio())
            },
            attempts: 2,
            spent: 3
        }
    );
    let numbers: Vec<(u32, AttemptState)> = operation
        .attempts()
        .map(|attempt| (attempt.number().get(), attempt.state()))
        .collect();
    assert_eq!(
        numbers,
        [
            (1, AttemptState::Uncertain(UncertainReason::TimedOut)),
            (2, AttemptState::Succeeded(audio()))
        ]
    );
}

#[test]
fn history_no_decision_could_have_produced_is_refused_with_its_position() {
    let uncertain = || settled(1, Settlement::Uncertain(UncertainReason::Abandoned));
    let rejected = || settled(1, Settlement::Rejected(Rejection::Refused));
    let cases: Vec<(&str, Vec<Event>, ReplayError)> = vec![
        ("empty", vec![], ReplayError::Empty),
        (
            "not opened",
            vec![settled(1, Settlement::Rejected(Rejection::Refused))],
            ReplayError::NotOpened,
        ),
        (
            "opened twice",
            vec![open(1), open(1)],
            ReplayError::OpenedAgain { index: 1 },
        ),
        (
            "regenerated while open",
            vec![open(1), regenerated(2, 1, None)],
            ReplayError::Reservation {
                index: 1,
                refusal: ReserveRefusal::InFlight { attempt: n(1) },
            },
        ),
        (
            "regenerated past uncertainty without a decision",
            vec![open(1), uncertain(), regenerated(2, 1, None)],
            ReplayError::Reservation {
                index: 2,
                refusal: ReserveRefusal::UncertaintyUndecided { attempt: n(1) },
            },
        ),
        (
            "regenerated past uncertainty on a stale decision",
            vec![open(1), uncertain(), regenerated(2, 1, Some(2))],
            ReplayError::Reservation {
                index: 2,
                refusal: ReserveRefusal::StaleDecision {
                    acknowledged: n(2),
                    latest: n(1),
                },
            },
        ),
        (
            "attempt number skipped",
            vec![open(1), rejected(), regenerated(3, 1, None)],
            ReplayError::OutOfOrder {
                index: 2,
                expected: n(2),
                found: n(3),
            },
        ),
        (
            "settled twice",
            vec![open(1), rejected(), rejected()],
            ReplayError::Settlement {
                index: 2,
                refusal: SettleRefusal::NotOpen(NotOpen {
                    attempt: n(1),
                    phase: Phase::Rejected,
                }),
            },
        ),
        (
            "settled an attempt that does not exist",
            vec![
                open(1),
                settled(2, Settlement::Rejected(Rejection::Refused)),
            ],
            ReplayError::Settlement {
                index: 1,
                refusal: SettleRefusal::NotLatest {
                    attempt: n(2),
                    latest: n(1),
                },
            },
        ),
    ];
    for (name, events, expected) in cases {
        assert_eq!(Operation::replay(&events), Err(expected), "{name}");
    }
}

#[test]
fn replay_does_not_apply_todays_limits_to_yesterdays_history() {
    // Three attempts and 30 units were allowed when these events were written.
    let events = [
        open(10),
        settled(1, Settlement::Rejected(Rejection::Refused)),
        regenerated(2, 10, None),
        settled(2, Settlement::Rejected(Rejection::Refused)),
        regenerated(3, 10, None),
    ];
    assert_eq!(operation(&events).status().spent, 30);
}

#[test]
fn a_request_is_reserved_replayed_conflicted_or_refused() {
    let absent = key();
    let reserve = submit(Lookup::Absent(&absent), fingerprint(1), cost(2), MODEL());
    assert_eq!(
        reserve,
        Submission::Reserve(Event::Opened {
            key: key(),
            fingerprint: fingerprint(1),
            cost: cost(2)
        })
    );
    assert_eq!(
        submit(Lookup::Absent(&absent), fingerprint(1), cost(4), MODEL()),
        Submission::Refused(ReserveRefusal::BudgetExhausted {
            spent: 0,
            cost: 4,
            budget: 3
        })
    );

    let existing = operation(&[open(1), settled(1, Settlement::Succeeded(audio()))]);
    let replay = submit(Lookup::Found(&existing), fingerprint(1), cost(1), MODEL());
    assert_eq!(replay, Submission::Replay(existing.status()));
    // A retry is recognized by the fingerprint alone, whatever today's estimate or policy says.
    assert_eq!(
        submit(
            Lookup::Found(&existing),
            fingerprint(1),
            cost(99),
            policy(1, 1)
        ),
        Submission::Replay(existing.status())
    );
    assert_eq!(
        submit(Lookup::Found(&existing), fingerprint(2), cost(1), MODEL()),
        Submission::Conflict {
            recorded: fingerprint(1)
        }
    );
}

#[test]
fn regeneration_and_settlement_are_decided_against_the_latest_attempt() {
    let open_operation = operation(&[open(1)]);
    assert_eq!(
        regenerate(&open_operation, None, cost(1), MODEL()),
        Err(ReserveRefusal::InFlight { attempt: n(1) })
    );
    let uncertain = operation(&[
        open(1),
        settled(1, Settlement::Uncertain(UncertainReason::ConnectionLost)),
    ]);
    assert_eq!(
        regenerate(&uncertain, None, cost(1), MODEL()),
        Err(ReserveRefusal::UncertaintyUndecided { attempt: n(1) })
    );
    assert_eq!(
        regenerate(
            &uncertain,
            Some(Acknowledgement::of(n(1))),
            cost(1),
            MODEL()
        ),
        Ok(regenerated(2, 1, Some(1)))
    );
    // A success may be regenerated explicitly; the earlier result stays recorded.
    let succeeded = operation(&[open(1), settled(1, Settlement::Succeeded(audio()))]);
    assert_eq!(
        regenerate(&succeeded, None, cost(1), MODEL()),
        Ok(regenerated(2, 1, None))
    );

    let rejected = Settlement::Rejected(Rejection::RateLimited);
    assert_eq!(
        settle(&open_operation, n(1), rejected),
        Ok(settled(1, rejected))
    );
    assert_eq!(
        settle(&open_operation, n(2), rejected),
        Err(SettleRefusal::NotLatest {
            attempt: n(2),
            latest: n(1)
        })
    );
    assert_eq!(
        settle(&succeeded, n(1), rejected),
        Err(SettleRefusal::NotOpen(NotOpen {
            attempt: n(1),
            phase: Phase::Succeeded
        }))
    );
}

#[derive(Clone, Copy, Debug)]
enum Command {
    SettleSucceeded,
    SettleRejected,
    SettleUncertain,
    Regenerate { acknowledge: u8, units: u64 },
    RetrySame,
    RetryChanged,
}

fn command() -> impl Strategy<Value = Command> {
    prop_oneof![
        Just(Command::SettleSucceeded),
        Just(Command::SettleRejected),
        Just(Command::SettleUncertain),
        (0_u8..3, 1_u64..=2)
            .prop_map(|(acknowledge, units)| Command::Regenerate { acknowledge, units }),
        Just(Command::RetrySame),
        Just(Command::RetryChanged),
    ]
}

proptest! {
    /// Any sequence of decisions, applied as the shell would apply them, leaves a history that
    /// replays to the same operation, stays inside the policy and never reserves behind an open
    /// or undecided uncertain attempt.
    #[test]
    fn decisions_applied_in_order_replay_to_the_same_operation(
        commands in proptest::collection::vec(command(), 0..24),
    ) {
        let limits = MODEL();
        let key = key();
        let Submission::Reserve(opened) = submit(Lookup::Absent(&key), fingerprint(1), cost(1), limits) else {
            panic!("a fresh key must reserve");
        };
        let mut events = vec![opened];
        let mut current = Operation::replay(&events).unwrap();
        for command in commands {
            let before = current.ledger();
            let decided = match command {
                Command::SettleSucceeded => settle(&current, before.attempts(), Settlement::Succeeded(audio())).ok(),
                Command::SettleRejected => settle(&current, before.attempts(), Settlement::Rejected(Rejection::Refused)).ok(),
                Command::SettleUncertain => settle(&current, before.attempts(), Settlement::Uncertain(UncertainReason::TimedOut)).ok(),
                Command::Regenerate { acknowledge, units } => {
                    let acknowledgement = match acknowledge {
                        0 => None,
                        1 => Some(Acknowledgement::of(before.attempts())),
                        _ => AttemptNumber::new(before.attempts().get().saturating_sub(1)).map(Acknowledgement::of),
                    };
                    regenerate(&current, acknowledgement, cost(units), limits).ok()
                }
                Command::RetrySame => {
                    prop_assert_eq!(
                        submit(Lookup::Found(&current), fingerprint(1), cost(1), limits),
                        Submission::Replay(current.status())
                    );
                    None
                }
                Command::RetryChanged => {
                    prop_assert_eq!(
                        submit(Lookup::Found(&current), fingerprint(2), cost(1), limits),
                        Submission::Conflict { recorded: fingerprint(1) }
                    );
                    None
                }
            };
            if let Some(event) = decided {
                if matches!(event, Event::Regenerated { .. }) {
                    prop_assert!(!matches!(before.phase(), Phase::Open));
                }
                current = current.apply(&event).unwrap();
                events.push(event);
            }
            let after = current.ledger();
            prop_assert!(after.spent() <= limits.budget());
            prop_assert!(after.attempts().get() <= limits.max_attempts());
            prop_assert_eq!(current.next_index(), events.len());
        }
        prop_assert_eq!(Operation::replay(&events), Ok(current));
    }
}
