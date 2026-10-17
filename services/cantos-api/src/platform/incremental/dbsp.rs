//! The DBSP circuit that maintains grouped flag counts from signed member changes.
//!
//! Each step feeds one batch of member changes (+1/-1 Z-set weights) and runs one DBSP
//! transaction. A linear aggregate per group turns those input deltas into output deltas of the
//! group's `(members, flagged…)` row; nothing is recomputed from the full input. Two more
//! operators maintain the integrated weight of each member row and of each member id, and report
//! any that is not exactly one: a change applied twice, a retraction of a row that was never
//! inserted, or two rows for one group-scoped member id. Any failed step permanently poisons
//! the instance; its owner must drop it and rebuild from authority, never patch it.
use dbsp::{
    typed_batch::SpineSnapshot,
    utils::{Tup2, Tup3, Tup4},
    DBSPHandle, OrdIndexedZSet, OutputHandle, Runtime, ZSetHandle, ZWeight,
};

use super::{CountChange, GroupCounts, Member, MemberChange, Sign};

/// `(group, member id, first flag, second flag)`.
type MemberRow = Tup4<String, String, bool, bool>;
/// `(members, first flagged, second flagged)` as linear-aggregate weights.
type CountRow = Tup3<i64, i64, i64>;
/// `(group, member id)`.
type MemberKey = Tup2<String, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    Start(String),
    Step(String),
    /// Member rows or group-scoped member ids whose integrated weight is not exactly one.
    Integrity {
        rows: usize,
        members: usize,
    },
    /// A count row outside the domain of counts, such as a negative member count.
    InvalidCount {
        group: String,
    },
    /// A previous step failed; this instance must be dropped and rebuilt from authority.
    Poisoned,
}

/// The whole output of one transaction. A transaction may run several internal steps (DBSP
/// splits a large input), and a plain `output()` handle keeps only the last step's delta.
type Accumulated<K, V> = OutputHandle<SpineSnapshot<OrdIndexedZSet<K, V>>>;

/// One single-worker, in-memory DBSP runtime. Not checkpointed: a lost or poisoned engine is
/// dropped and rebuilt. A failed step does not roll back the circuit's internal state.
pub struct DbspCounts {
    // Dropping the handle stops the runtime's worker thread.
    handle: DBSPHandle,
    input: ZSetHandle<MemberRow>,
    counts: Accumulated<String, CountRow>,
    row_anomalies: Accumulated<MemberRow, ZWeight>,
    member_anomalies: Accumulated<MemberKey, ZWeight>,
    poisoned: bool,
}

impl DbspCounts {
    pub fn start() -> Result<Self, EngineError> {
        let (handle, (input, counts, row_anomalies, member_anomalies)) =
            Runtime::init_circuit(1, |circuit| {
                let (members, input) = circuit.add_input_zset::<MemberRow>();
                let counts = members
                    .map_index(|Tup4(group, _, first, second)| {
                        (group.clone(), Tup2(*first, *second))
                    })
                    .aggregate_linear(|Tup2(first, second): &Tup2<bool, bool>| {
                        Tup3(1, i64::from(*first), i64::from(*second))
                    });
                // Separate outputs: a union could cancel one anomaly against another.
                let row_anomalies = members
                    .map_index(|row| (row.clone(), ()))
                    .weighted_count()
                    .filter(|(_, weight)| *weight != 1);
                let member_anomalies = members
                    .map_index(|Tup4(group, id, _, _)| (Tup2(group.clone(), id.clone()), ()))
                    .weighted_count()
                    .filter(|(_, weight)| *weight != 1);
                Ok((
                    input,
                    counts.accumulate_output(),
                    row_anomalies.accumulate_output(),
                    member_anomalies.accumulate_output(),
                ))
            })
            .map_err(|error| EngineError::Start(error.to_string()))?;
        Ok(DbspCounts {
            handle,
            input,
            counts,
            row_anomalies,
            member_anomalies,
            poisoned: false,
        })
    }

    /// Feeds one batch and returns only the groups whose counts changed. The first failure
    /// retains its exact cause; every later call refuses input with `EngineError::Poisoned`.
    /// The owner must also discard this engine if its output is rejected by `CountView`.
    pub fn step(&mut self, changes: Vec<MemberChange>) -> Result<Vec<CountChange>, EngineError> {
        if self.poisoned {
            return Err(EngineError::Poisoned);
        }
        let result = self.step_once(changes);
        self.poisoned = result.is_err();
        result
    }

    fn step_once(&mut self, changes: Vec<MemberChange>) -> Result<Vec<CountChange>, EngineError> {
        let mut batch: Vec<Tup2<MemberRow, ZWeight>> = changes
            .into_iter()
            .map(|change| Tup2(member_row(change.member), change.sign.weight()))
            .collect();
        self.input.append(&mut batch);
        self.handle
            .transaction()
            .map_err(|error| EngineError::Step(error.to_string()))?;
        // Every earlier step succeeded, so the integrated anomaly views were empty. Any
        // inserted anomaly is current; an error prevents every subsequent circuit step.
        let rows = inserted(&self.row_anomalies.concat().consolidate());
        let members = inserted(&self.member_anomalies.concat().consolidate());
        if rows > 0 || members > 0 {
            return Err(EngineError::Integrity { rows, members });
        }
        self.counts
            .concat()
            .consolidate()
            .iter()
            .map(|(group, row, weight)| count_change(group, row, weight))
            .collect()
    }
}

fn inserted<K: dbsp::DBData>(anomalies: &OrdIndexedZSet<K, ZWeight>) -> usize {
    anomalies
        .iter()
        .filter(|(_, _, weight)| *weight > 0)
        .count()
}

fn member_row(member: Member) -> MemberRow {
    let [first, second] = member.flags;
    Tup4(member.group, member.id, first, second)
}

fn count_change(group: String, row: CountRow, weight: ZWeight) -> Result<CountChange, EngineError> {
    let invalid = || EngineError::InvalidCount {
        group: group.clone(),
    };
    let sign = Sign::from_weight(weight).ok_or_else(invalid)?;
    let Tup3(members, first, second) = row;
    let counts = GroupCounts {
        members: u64::try_from(members).map_err(|_| invalid())?,
        flagged: [
            u64::try_from(first).map_err(|_| invalid())?,
            u64::try_from(second).map_err(|_| invalid())?,
        ],
    };
    if !counts.is_valid() {
        return Err(invalid());
    }
    Ok(CountChange {
        sign,
        group,
        counts,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use proptest::prelude::*;

    use super::*;
    use crate::platform::incremental::{count_members, CountView};

    fn member(group: &str, id: &str, flags: [bool; 2]) -> Member {
        Member {
            group: group.into(),
            id: id.into(),
            flags,
        }
    }

    fn insert(member: Member) -> MemberChange {
        MemberChange {
            sign: Sign::Insert,
            member,
        }
    }

    fn retract(member: Member) -> MemberChange {
        MemberChange {
            sign: Sign::Retract,
            member,
        }
    }

    fn counts(members: u64, first: u64, second: u64) -> GroupCounts {
        GroupCounts {
            members,
            flagged: [first, second],
        }
    }

    #[test]
    fn a_transition_emits_only_the_changed_group_as_a_retraction_and_an_insertion() {
        let mut engine = DbspCounts::start().unwrap();
        let mut initial = engine
            .step(vec![
                insert(member("a", "1", [false, false])),
                insert(member("a", "2", [false, false])),
                insert(member("b", "3", [true, true])),
            ])
            .unwrap();
        initial.sort();
        assert_eq!(
            initial,
            [
                CountChange {
                    sign: Sign::Insert,
                    group: "a".into(),
                    counts: counts(2, 0, 0)
                },
                CountChange {
                    sign: Sign::Insert,
                    group: "b".into(),
                    counts: counts(1, 1, 1)
                },
            ]
        );
        let mut transition = engine
            .step(vec![
                retract(member("a", "2", [false, false])),
                insert(member("a", "2", [true, false])),
            ])
            .unwrap();
        transition.sort();
        assert_eq!(
            transition,
            [
                CountChange {
                    sign: Sign::Insert,
                    group: "a".into(),
                    counts: counts(2, 1, 0)
                },
                CountChange {
                    sign: Sign::Retract,
                    group: "a".into(),
                    counts: counts(2, 0, 0)
                },
            ]
        );
        assert_eq!(engine.step(vec![]).unwrap(), []);
    }

    #[test]
    fn a_group_move_retracts_both_old_counts_and_inserts_both_new_counts() {
        let mut engine = DbspCounts::start().unwrap();
        let mut view = CountView::default();
        let moving = member("a", "1", [true, false]);
        view.apply(
            engine
                .step(vec![
                    insert(moving.clone()),
                    insert(member("a", "2", [false, false])),
                    insert(member("b", "3", [false, true])),
                    insert(member("c", "4", [true, true])),
                ])
                .unwrap(),
        )
        .unwrap();

        let mut changes = engine
            .step(vec![
                retract(moving),
                insert(member("b", "1", [true, false])),
            ])
            .unwrap();
        changes.sort();
        assert_eq!(
            changes,
            [
                CountChange {
                    sign: Sign::Insert,
                    group: "a".into(),
                    counts: counts(1, 0, 0),
                },
                CountChange {
                    sign: Sign::Insert,
                    group: "b".into(),
                    counts: counts(2, 1, 1),
                },
                CountChange {
                    sign: Sign::Retract,
                    group: "a".into(),
                    counts: counts(2, 1, 0),
                },
                CountChange {
                    sign: Sign::Retract,
                    group: "b".into(),
                    counts: counts(1, 0, 1),
                },
            ]
        );
        assert_eq!(view.apply(changes).unwrap(), 2);
        assert_eq!(view.get("a"), counts(1, 0, 0));
        assert_eq!(view.get("b"), counts(2, 1, 1));
        assert_eq!(view.get("c"), counts(1, 1, 1));
        assert_eq!(engine.step(vec![]).unwrap(), []);
    }

    #[test]
    fn groups_can_reuse_a_local_id_without_coupling_their_member_rows() {
        let mut engine = DbspCounts::start().unwrap();
        let mut view = CountView::default();
        let first = member("a", "local-1", [true, false]);
        let second = member("b", "local-1", [false, true]);
        view.apply(
            engine
                .step(vec![insert(first.clone()), insert(second.clone())])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(view.len(), 2);
        assert_eq!(view.get("a"), counts(1, 1, 0));
        assert_eq!(view.get("b"), counts(1, 0, 1));

        view.apply(
            engine
                .step(vec![
                    retract(first),
                    retract(second),
                    insert(member("b", "local-1", [true, true])),
                ])
                .unwrap(),
        )
        .unwrap();
        assert_eq!(view.len(), 1);
        assert_eq!(view.get("a"), counts(0, 0, 0));
        assert_eq!(view.get("b"), counts(1, 1, 1));
    }

    #[test]
    fn count_rows_preserve_unit_signs_and_the_full_nonnegative_i64_range() {
        for (weight, sign) in [(-1, Sign::Retract), (1, Sign::Insert)] {
            for (row, expected) in [
                (Tup3(0, 0, 0), counts(0, 0, 0)),
                (Tup3(1, 0, 1), counts(1, 0, 1)),
                (
                    Tup3(i64::MAX, i64::MAX, i64::MAX - 1),
                    counts(i64::MAX as u64, i64::MAX as u64, (i64::MAX - 1) as u64),
                ),
            ] {
                assert_eq!(
                    count_change("a".into(), row, weight),
                    Ok(CountChange {
                        sign,
                        group: "a".into(),
                        counts: expected,
                    })
                );
            }
        }
    }

    #[test]
    fn count_rows_reject_nonunit_weights_negative_counts_and_flags_above_members() {
        for weight in [i64::MIN, -2, 0, 2, i64::MAX] {
            assert_eq!(
                count_change("bad-weight".into(), Tup3(1, 0, 0), weight),
                Err(EngineError::InvalidCount {
                    group: "bad-weight".into(),
                })
            );
        }
        for row in [
            Tup3(i64::MIN, 0, 0),
            Tup3(-1, 0, 0),
            Tup3(1, i64::MIN, 0),
            Tup3(1, -1, 0),
            Tup3(1, 0, i64::MIN),
            Tup3(1, 0, -1),
            Tup3(0, 1, 0),
            Tup3(0, 0, 1),
            Tup3(1, 2, 0),
            Tup3(1, 0, 2),
            Tup3(i64::MAX - 1, i64::MAX, 0),
            Tup3(i64::MAX - 1, 0, i64::MAX),
        ] {
            for weight in [-1, 1] {
                assert_eq!(
                    count_change("bad-count".into(), row, weight),
                    Err(EngineError::InvalidCount {
                        group: "bad-count".into(),
                    })
                );
            }
        }
    }

    /// Regression: a step above DBSP's 10,000-record split runs as several internal steps; the
    /// output of every one of them must reach the view.
    #[test]
    fn a_step_larger_than_the_engine_split_returns_its_complete_delta() {
        let mut engine = DbspCounts::start().unwrap();
        let mut view = CountView::default();
        let members: Vec<Member> = (0..25_000)
            .map(|index| {
                member(
                    &format!("g{}", index % 3),
                    &format!("m{index}"),
                    [false, false],
                )
            })
            .collect();
        view.apply(
            engine
                .step(members.iter().cloned().map(insert).collect())
                .unwrap(),
        )
        .unwrap();
        let mut changes = Vec::new();
        let mut current = Vec::new();
        for (index, old) in members.into_iter().enumerate() {
            let new = Member {
                flags: [index % 2 == 0, index % 5 == 0],
                ..old.clone()
            };
            changes.push(retract(old));
            changes.push(insert(new.clone()));
            current.push(new);
        }
        view.apply(engine.step(changes).unwrap()).unwrap();
        let held: HashMap<String, GroupCounts> = view
            .groups()
            .map(|(group, counts)| (group.to_owned(), counts))
            .collect();
        assert_eq!(held, count_members(&current));
    }

    #[test]
    fn retracting_the_last_member_removes_the_group() {
        let mut engine = DbspCounts::start().unwrap();
        engine
            .step(vec![insert(member("a", "1", [true, false]))])
            .unwrap();
        assert_eq!(
            engine
                .step(vec![retract(member("a", "1", [true, false]))])
                .unwrap(),
            [CountChange {
                sign: Sign::Retract,
                group: "a".into(),
                counts: counts(1, 1, 0)
            }]
        );
    }

    #[test]
    fn a_change_delivered_twice_or_never_inserted_is_an_integrity_failure() {
        let mut duplicated = DbspCounts::start().unwrap();
        let row = member("a", "1", [false, false]);
        duplicated.step(vec![insert(row.clone())]).unwrap();
        assert_eq!(
            duplicated.step(vec![insert(row.clone())]),
            Err(EngineError::Integrity {
                rows: 1,
                members: 1
            })
        );
        let mut unmatched = DbspCounts::start().unwrap();
        unmatched
            .step(vec![insert(member("a", "1", [false, false]))])
            .unwrap();
        assert_eq!(
            unmatched.step(vec![
                retract(member("a", "1", [true, false])),
                insert(member("a", "1", [true, true])),
            ]),
            Err(EngineError::Integrity {
                rows: 1,
                members: 0
            })
        );
        // Each row looks valid on its own, but one member now has two rows.
        let mut forked = DbspCounts::start().unwrap();
        forked
            .step(vec![insert(member("a", "1", [false, false]))])
            .unwrap();
        assert_eq!(
            forked.step(vec![insert(member("a", "1", [true, false]))]),
            Err(EngineError::Integrity {
                rows: 0,
                members: 1
            })
        );
    }

    #[test]
    fn an_integrity_failure_permanently_refuses_empty_repair_and_valid_steps() {
        let row = member("a", "1", [false, false]);
        let wrong = member("a", "1", [true, false]);
        let replacement = member("a", "1", [true, true]);
        for (initial, invalid, repair, expected) in [
            (
                vec![insert(row.clone())],
                vec![insert(row.clone())],
                vec![retract(row.clone())],
                EngineError::Integrity {
                    rows: 1,
                    members: 1,
                },
            ),
            (
                vec![],
                vec![retract(row.clone())],
                vec![insert(row.clone())],
                EngineError::Integrity {
                    rows: 1,
                    members: 1,
                },
            ),
            (
                vec![insert(row.clone())],
                vec![retract(wrong.clone()), insert(replacement.clone())],
                vec![insert(wrong.clone()), retract(replacement)],
                EngineError::Integrity {
                    rows: 1,
                    members: 0,
                },
            ),
            (
                vec![insert(row.clone())],
                vec![insert(wrong.clone())],
                vec![retract(wrong)],
                EngineError::Integrity {
                    rows: 0,
                    members: 1,
                },
            ),
        ] {
            let mut engine = DbspCounts::start().unwrap();
            engine.step(initial).unwrap();
            assert_eq!(engine.step(invalid), Err(expected));
            for changes in [vec![], repair, vec![insert(member("b", "2", [true, true]))]] {
                assert_eq!(engine.step(changes), Err(EngineError::Poisoned));
            }
        }

        let mut rebuilt = DbspCounts::start().unwrap();
        assert_eq!(
            rebuilt.step(vec![insert(row)]).unwrap(),
            [CountChange {
                sign: Sign::Insert,
                group: "a".into(),
                counts: counts(1, 0, 0),
            }]
        );
    }

    #[derive(Debug, Clone)]
    enum Op {
        Insert(u8, [bool; 2]),
        Update(usize, [bool; 2]),
        Move(usize, u8),
        Delete(usize),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (0u8..5, any::<[bool; 2]>()).prop_map(|(group, flags)| Op::Insert(group, flags)),
            (any::<usize>(), any::<[bool; 2]>())
                .prop_map(|(index, flags)| Op::Update(index, flags)),
            (any::<usize>(), 0u8..5).prop_map(|(index, group)| Op::Move(index, group)),
            any::<usize>().prop_map(Op::Delete),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// Differential law: after any sequence of inserts, flag transitions, group moves and
        /// deletes, split into arbitrary engine steps, the view maintained from DBSP output
        /// deltas equals an independent recount of the surviving members.
        #[test]
        fn maintained_counts_equal_a_recount_of_the_current_members(
            steps in proptest::collection::vec(proptest::collection::vec(op(), 0..10), 1..8)
        ) {
            let mut engine = DbspCounts::start().unwrap();
            let mut view = CountView::default();
            let mut members: BTreeMap<String, Member> = BTreeMap::new();
            let mut next = 0;
            for ops in steps {
                let mut changes = Vec::new();
                for op in ops {
                    match op {
                        Op::Insert(group, flags) => {
                            let row = member(&format!("g{group}"), &format!("m{next}"), flags);
                            next += 1;
                            members.insert(row.id.clone(), row.clone());
                            changes.push(insert(row));
                        }
                        Op::Update(index, flags) if !members.is_empty() => {
                            let id = members.keys().nth(index % members.len()).unwrap().clone();
                            let old = members[&id].clone();
                            let new = Member { flags, ..old.clone() };
                            members.insert(id, new.clone());
                            changes.push(retract(old));
                            changes.push(insert(new));
                        }
                        Op::Move(index, group) if !members.is_empty() => {
                            let id = members.keys().nth(index % members.len()).unwrap().clone();
                            let old = members[&id].clone();
                            let new = Member { group: format!("g{group}"), ..old.clone() };
                            members.insert(id, new.clone());
                            changes.push(retract(old));
                            changes.push(insert(new));
                        }
                        Op::Delete(index) if !members.is_empty() => {
                            let id = members.keys().nth(index % members.len()).unwrap().clone();
                            changes.push(retract(members.remove(&id).unwrap()));
                        }
                        Op::Update(..) | Op::Move(..) | Op::Delete(_) => {}
                    }
                }
                view.apply(engine.step(changes).unwrap()).unwrap();
                let held: HashMap<String, GroupCounts> =
                    view.groups().map(|(group, counts)| (group.to_owned(), counts)).collect();
                prop_assert_eq!(held, count_members(members.values()));
            }
        }
    }
}
