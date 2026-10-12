//! The DBSP circuit that maintains grouped flag counts from signed member changes.
//!
//! Each step feeds one batch of member changes (+1/-1 Z-set weights) and runs one DBSP
//! transaction. A linear aggregate per group turns those input deltas into output deltas of the
//! group's `(members, flagged…)` row; nothing is recomputed from the full input. Two more
//! operators maintain the integrated weight of each member row and of each member id, and report
//! any that is not exactly one: a change applied twice, a retraction of a row that was never
//! inserted, or two rows for one member. Such a step is refused and the engine must be discarded,
//! never patched.
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
    /// Member rows or member ids whose integrated weight is not exactly one after the step.
    Integrity {
        rows: usize,
        members: usize,
    },
    /// A count row outside the domain of counts, such as a negative member count.
    InvalidCount {
        group: String,
    },
}

/// The whole output of one transaction. A transaction may run several internal steps (DBSP
/// splits a large input), and a plain `output()` handle keeps only the last step's delta.
type Accumulated<K, V> = OutputHandle<SpineSnapshot<OrdIndexedZSet<K, V>>>;

/// One single-worker, in-memory DBSP runtime. Not checkpointed: a lost engine is rebuilt.
pub struct DbspCounts {
    // Dropping the handle stops the runtime's worker thread.
    handle: DBSPHandle,
    input: ZSetHandle<MemberRow>,
    counts: Accumulated<String, CountRow>,
    row_anomalies: Accumulated<MemberRow, ZWeight>,
    member_anomalies: Accumulated<MemberKey, ZWeight>,
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
        })
    }

    /// Feeds one batch and returns only the groups whose counts changed.
    pub fn step(&mut self, changes: Vec<MemberChange>) -> Result<Vec<CountChange>, EngineError> {
        let mut batch: Vec<Tup2<MemberRow, ZWeight>> = changes
            .into_iter()
            .map(|change| Tup2(member_row(change.member), change.sign.weight()))
            .collect();
        self.input.append(&mut batch);
        self.handle
            .transaction()
            .map_err(|error| EngineError::Step(error.to_string()))?;
        // The anomaly views are empty before every step, or the engine was already refused, so
        // any inserted anomaly is a current one.
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
    if counts
        .flagged
        .iter()
        .any(|flagged| *flagged > counts.members)
    {
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
    use crate::incremental::{count_members, CountView};

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

    #[derive(Debug, Clone)]
    enum Op {
        Insert(u8, [bool; 2]),
        Update(usize, [bool; 2]),
        Delete(usize),
    }

    fn op() -> impl Strategy<Value = Op> {
        prop_oneof![
            (0u8..5, any::<[bool; 2]>()).prop_map(|(group, flags)| Op::Insert(group, flags)),
            (any::<usize>(), any::<[bool; 2]>())
                .prop_map(|(index, flags)| Op::Update(index, flags)),
            any::<usize>().prop_map(Op::Delete),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        /// Differential law: after any sequence of inserts, transitions and deletes, split into
        /// arbitrary engine steps, the view maintained from DBSP output deltas equals an
        /// independent recount of the surviving members.
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
                        Op::Delete(index) if !members.is_empty() => {
                            let id = members.keys().nth(index % members.len()).unwrap().clone();
                            changes.push(retract(members.remove(&id).unwrap()));
                        }
                        Op::Update(..) | Op::Delete(_) => {}
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
