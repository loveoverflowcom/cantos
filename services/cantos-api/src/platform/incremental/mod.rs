//! Engine-independent core of the experimental incremental read model.
//!
//! An incremental engine receives signed member changes from PostgreSQL's committed change feed
//! and returns signed changes of per-group counts. This module owns the values on both sides,
//! the view that applies output changes, whole-transaction paging of the feed and the freshness
//! decision that lets a request use the view only when it provably equals the request's own
//! PostgreSQL snapshot. It performs no I/O and names no engine, table or domain type.
//!
//! # Committed-position contract
//!
//! Every feed row carries its writer's top-level transaction id (`xid8`, ordered, never wraps).
//! A view at [`CommittedPosition`] `{frontier F, bootstrap S0}` reflects exactly the committed
//! rows that are visible in the bootstrap snapshot `S0` or belong to a transaction below `F`.
//! The feed only advances `F` to a snapshot's `xmin`, below which no transaction is still
//! running, and only applies whole transactions; so no committed row is skipped, none is applied
//! twice and no transaction is ever half visible.
//!
//! A request whose snapshot `S` satisfies `F <= xmin(S)` sees every row the view applied. If, in
//! that same snapshot, its group has no row at or after `F` outside `S0`, then the view's rows
//! for the group are exactly the rows `S` sees: [`Freshness::Exact`]. Otherwise the view is not
//! comparable with that request, and the caller must use the authoritative query instead.

#[cfg(feature = "dbsp-experimental")]
pub mod dbsp;

use std::{collections::HashMap, fmt, str::FromStr};

/// Insert or retract one copy of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Sign {
    Insert,
    Retract,
}

impl Sign {
    pub fn weight(self) -> i64 {
        match self {
            Sign::Insert => 1,
            Sign::Retract => -1,
        }
    }

    /// Only ±1 is a change; any other weight is corrupt rather than a silent multiplier.
    pub fn from_weight(weight: i64) -> Option<Self> {
        match weight {
            1 => Some(Sign::Insert),
            -1 => Some(Sign::Retract),
            _ => None,
        }
    }
}

/// The projected facts of one member of a group, such as a step of a job and two of its flags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Member {
    pub group: String,
    pub id: String,
    pub flags: [bool; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MemberChange {
    pub sign: Sign,
    pub member: Member,
}

/// How many members a group has and how many of them carry each flag.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupCounts {
    pub members: u64,
    pub flagged: [u64; 2],
}

impl GroupCounts {
    /// Raw engine/adapter counts must satisfy this before entering an admitted view.
    pub fn is_valid(self) -> bool {
        self.flagged.iter().all(|flagged| *flagged <= self.members)
    }

    pub fn is_empty(&self) -> bool {
        *self == GroupCounts::default()
    }
}

/// One output change of the maintained per-group counts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CountChange {
    pub sign: Sign,
    pub group: String,
    pub counts: GroupCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// A flag count exceeds the number of members in the same group.
    InvalidCounts { group: String },
    /// A retraction named counts that the view does not hold for the group.
    UnmatchedRetraction { group: String },
    /// A group would hold two count rows after the step.
    ConflictingInsertion { group: String },
}

/// Current counts per group, maintained only by applying output changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CountView {
    groups: HashMap<String, GroupCounts>,
}

impl CountView {
    /// A group the view does not hold has no members.
    pub fn get(&self, group: &str) -> GroupCounts {
        self.groups.get(group).copied().unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.groups.len()
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn groups(&self) -> impl Iterator<Item = (&str, GroupCounts)> {
        self.groups
            .iter()
            .map(|(group, counts)| (group.as_str(), *counts))
    }

    /// Applies one engine step as a unit. Retractions are matched against the current rows
    /// before any insertion, so an engine's `-old, +new` pair for a group is order independent.
    /// On error nothing changes; the caller must discard the engine and rebuild.
    pub fn apply(&mut self, changes: Vec<CountChange>) -> Result<usize, ViewError> {
        let staged = stage(&self.groups, changes)?;
        let touched = staged.len();
        for (group, counts) in staged {
            match counts {
                Some(counts) if !counts.is_empty() => {
                    self.groups.insert(group, counts);
                }
                _ => {
                    self.groups.remove(&group);
                }
            }
        }
        Ok(touched)
    }
}

/// The resulting row of every touched group, or `None` when the group no longer has one.
fn stage(
    current: &HashMap<String, GroupCounts>,
    changes: Vec<CountChange>,
) -> Result<HashMap<String, Option<GroupCounts>>, ViewError> {
    for change in &changes {
        if !change.counts.is_valid() {
            return Err(ViewError::InvalidCounts {
                group: change.group.clone(),
            });
        }
    }
    let (retractions, insertions): (Vec<_>, Vec<_>) = changes
        .into_iter()
        .partition(|change| change.sign == Sign::Retract);
    let mut staged: HashMap<String, Option<GroupCounts>> = HashMap::new();
    for change in retractions {
        let row = staged
            .entry(change.group.clone())
            .or_insert_with(|| current.get(&change.group).copied());
        if *row != Some(change.counts) {
            return Err(ViewError::UnmatchedRetraction {
                group: change.group,
            });
        }
        *row = None;
    }
    for change in insertions {
        let row = staged
            .entry(change.group.clone())
            .or_insert_with(|| current.get(&change.group).copied());
        if row.is_some() {
            return Err(ViewError::ConflictingInsertion {
                group: change.group,
            });
        }
        *row = Some(change.counts);
    }
    Ok(staged)
}

/// A PostgreSQL `xid8` transaction id: ordered and never wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TxId(u64);

impl TxId {
    pub fn new(value: u64) -> Self {
        TxId(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }

    /// The smallest id after this one, used as an exclusive frontier.
    pub fn successor(self) -> Option<TxId> {
        self.0.checked_add(1).map(TxId)
    }
}

impl FromStr for TxId {
    type Err = FeedError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse()
            .map(TxId)
            .map_err(|_| FeedError::MalformedPosition)
    }
}

impl fmt::Display for TxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A PostgreSQL `pg_snapshot` in its text form `xmin:xmax:xip,...`, checked on construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Snapshot {
    text: String,
    xmin: TxId,
    xmax: TxId,
}

impl Snapshot {
    pub fn parse(text: &str) -> Result<Self, FeedError> {
        let mut parts = text.splitn(3, ':');
        let (Some(xmin), Some(xmax), Some(running)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(FeedError::MalformedPosition);
        };
        let (xmin, xmax): (TxId, TxId) = (xmin.parse()?, xmax.parse()?);
        let running = running
            .split(',')
            .filter(|id| !id.is_empty())
            .map(TxId::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        if xmin > xmax || running.iter().any(|id| *id < xmin || *id >= xmax) {
            return Err(FeedError::MalformedPosition);
        }
        Ok(Snapshot {
            text: text.to_owned(),
            xmin,
            xmax,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn xmin(&self) -> TxId {
        self.xmin
    }

    pub fn xmax(&self) -> TxId {
        self.xmax
    }
}

/// The committed changes a view reflects; see the module documentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedPosition {
    frontier: TxId,
    bootstrap: Snapshot,
}

impl CommittedPosition {
    /// A bootstrap that read its rows in snapshot `S0` reflects every row visible in it, and so
    /// every transaction below `xmin(S0)`.
    pub fn bootstrap(snapshot: Snapshot) -> Self {
        CommittedPosition {
            frontier: snapshot.xmin(),
            bootstrap: snapshot,
        }
    }

    pub fn frontier(&self) -> TxId {
        self.frontier
    }

    pub fn bootstrap_snapshot(&self) -> &Snapshot {
        &self.bootstrap
    }

    /// The position after applying `page`, which must start at this frontier.
    pub fn advance(&self, page: &Page) -> Result<Self, FeedError> {
        if page.start != self.frontier {
            return Err(FeedError::PageStartMismatch);
        }
        Ok(CommittedPosition {
            frontier: page.frontier,
            bootstrap: self.bootstrap.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedError {
    MalformedPosition,
    /// The settled bound moved behind the frontier, which a correct feed never does.
    SettledBehindFrontier,
    /// A row lies outside `[frontier, settled)` or out of transaction order.
    RowOutsideWindow,
    InvalidWeight,
    InvalidLimit,
    /// A page was planned from another position, including replay of an advancing page.
    PageStartMismatch,
}

/// One row of the committed change feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedRow {
    pub txid: TxId,
    pub change: MemberChange,
}

/// An immutable page admitted by `plan_page`, including the frontier it was planned from.
/// This does not certify that a database returned every row or that an engine applied it.
///
/// ```compile_fail
/// use cantos_api::platform::incremental::{Page, TxId};
/// let forged = Page { changes: vec![], transactions: 0, frontier: TxId::new(9), start: TxId::new(10) };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    changes: Vec<MemberChange>,
    transactions: usize,
    frontier: TxId,
    start: TxId,
}

impl Page {
    pub fn changes(&self) -> &[MemberChange] {
        &self.changes
    }

    pub fn transactions(&self) -> usize {
        self.transactions
    }

    pub fn frontier(&self) -> TxId {
        self.frontier
    }
}

/// Plans the next step from the rows of at most `limit` whole transactions in
/// `[frontier, settled)`, read in transaction order. When fewer than `limit` transactions came
/// back the window is exhausted and the frontier reaches `settled`; otherwise it stops just after
/// the last transaction read, so the next page starts with the first transaction not applied.
pub fn plan_page(
    frontier: TxId,
    settled: TxId,
    rows: Vec<FeedRow>,
    limit: usize,
) -> Result<Page, FeedError> {
    if settled < frontier {
        return Err(FeedError::SettledBehindFrontier);
    }
    if limit == 0 {
        return Err(FeedError::InvalidLimit);
    }
    let mut last: Option<TxId> = None;
    let mut transactions = 0;
    for row in &rows {
        if row.txid < frontier || row.txid >= settled || last.is_some_and(|last| row.txid < last) {
            return Err(FeedError::RowOutsideWindow);
        }
        if last != Some(row.txid) {
            transactions += 1;
            last = Some(row.txid);
        }
    }
    if transactions > limit {
        return Err(FeedError::RowOutsideWindow);
    }
    let next = match last {
        Some(last) if transactions == limit => last.successor().unwrap_or(settled),
        _ => settled,
    };
    Ok(Page {
        changes: rows.into_iter().map(|row| row.change).collect(),
        transactions,
        frontier: next,
        start: frontier,
    })
}

/// The fence a request evaluates in its own PostgreSQL snapshot, for one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fence {
    /// `frontier <= xmin` of the request's snapshot: it sees everything the view applied.
    pub covers_frontier: bool,
    /// The group has a visible change at or after the frontier that the bootstrap did not read.
    pub unapplied_change: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// The view's rows for the group are exactly the rows the request's snapshot sees.
    Exact,
    /// The request sees a change the view has not applied yet.
    Lagging,
    /// The view may include a transaction the request's snapshot cannot see.
    Ahead,
}

pub fn freshness(fence: Fence) -> Freshness {
    match fence {
        Fence {
            covers_frontier: false,
            ..
        } => Freshness::Ahead,
        Fence {
            unapplied_change: true,
            ..
        } => Freshness::Lagging,
        Fence {
            covers_frontier: true,
            unapplied_change: false,
        } => Freshness::Exact,
    }
}

/// The result of comparing the view with the authoritative query in one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowOutcome {
    Match,
    Mismatch,
    NotComparable(Freshness),
}

pub fn shadow_compare(
    authoritative: GroupCounts,
    view: GroupCounts,
    freshness: Freshness,
) -> ShadowOutcome {
    match freshness {
        Freshness::Exact if authoritative == view => ShadowOutcome::Match,
        Freshness::Exact => ShadowOutcome::Mismatch,
        other => ShadowOutcome::NotComparable(other),
    }
}

/// Which path answers a read: the authoritative query alone, the query compared against the
/// view, or the view whenever it is provably exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadModel {
    Postgres,
    DbspShadow,
    DbspServe,
}

impl ReadModel {
    pub const VARIABLE: &'static str = "CANTOS_JOB_SUMMARY_READ_MODEL";

    pub fn as_str(self) -> &'static str {
        match self {
            ReadModel::Postgres => "postgres",
            ReadModel::DbspShadow => "dbsp-shadow",
            ReadModel::DbspServe => "dbsp-serve",
        }
    }

    pub fn uses_view(self) -> bool {
        self != ReadModel::Postgres
    }
}

impl FromStr for ReadModel {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "postgres" => Ok(ReadModel::Postgres),
            "dbsp-shadow" => Ok(ReadModel::DbspShadow),
            "dbsp-serve" => Ok(ReadModel::DbspServe),
            _ => Err(()),
        }
    }
}

/// An independent recomputation from the current members, for oracles in tests.
#[cfg(test)]
pub(crate) fn count_members<'a>(
    members: impl IntoIterator<Item = &'a Member>,
) -> HashMap<String, GroupCounts> {
    let mut counts: HashMap<String, GroupCounts> = HashMap::new();
    for member in members {
        let entry = counts.entry(member.group.clone()).or_default();
        entry.members += 1;
        for (flag, set) in member.flags.iter().enumerate() {
            entry.flagged[flag] += u64::from(*set);
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn counts(members: u64, first: u64, second: u64) -> GroupCounts {
        GroupCounts {
            members,
            flagged: [first, second],
        }
    }

    fn change(sign: Sign, group: &str, value: GroupCounts) -> CountChange {
        CountChange {
            sign,
            group: group.into(),
            counts: value,
        }
    }

    fn row(txid: u64, id: &str) -> FeedRow {
        FeedRow {
            txid: TxId::new(txid),
            change: MemberChange {
                sign: Sign::Insert,
                member: Member {
                    group: "job".into(),
                    id: id.into(),
                    flags: [false, false],
                },
            },
        }
    }

    #[test]
    fn only_unit_weights_are_changes() {
        assert_eq!(Sign::from_weight(1), Some(Sign::Insert));
        assert_eq!(Sign::from_weight(-1), Some(Sign::Retract));
        for weight in [0, 2, -2, i64::MAX, i64::MIN] {
            assert_eq!(Sign::from_weight(weight), None);
        }
    }

    #[test]
    fn a_step_replaces_a_group_row_whatever_order_its_retraction_arrives_in() {
        let mut view = CountView::default();
        view.apply(vec![change(Sign::Insert, "a", counts(4, 0, 0))])
            .unwrap();
        view.apply(vec![
            change(Sign::Insert, "a", counts(4, 1, 0)),
            change(Sign::Retract, "a", counts(4, 0, 0)),
        ])
        .unwrap();
        assert_eq!(view.get("a"), counts(4, 1, 0));
        view.apply(vec![change(Sign::Retract, "a", counts(4, 1, 0))])
            .unwrap();
        assert_eq!(view.get("a"), GroupCounts::default());
        assert!(view.is_empty());
    }

    #[test]
    fn an_unmatched_retraction_or_second_row_is_rejected_and_leaves_the_view_unchanged() {
        let mut view = CountView::default();
        view.apply(vec![change(Sign::Insert, "a", counts(2, 1, 0))])
            .unwrap();
        let before = view.clone();
        assert_eq!(
            view.apply(vec![
                change(Sign::Insert, "b", counts(1, 0, 0)),
                change(Sign::Retract, "a", counts(2, 0, 0)),
            ]),
            Err(ViewError::UnmatchedRetraction { group: "a".into() })
        );
        assert_eq!(
            view.apply(vec![change(Sign::Insert, "a", counts(3, 1, 0))]),
            Err(ViewError::ConflictingInsertion { group: "a".into() })
        );
        assert_eq!(
            view.apply(vec![
                change(Sign::Insert, "c", counts(1, 0, 0)),
                change(Sign::Insert, "c", counts(2, 0, 0)),
            ]),
            Err(ViewError::ConflictingInsertion { group: "c".into() })
        );
        assert_eq!(view, before);
    }

    #[test]
    fn impossible_count_replacement_rejects_the_whole_step() {
        let mut view = CountView::default();
        view.apply(vec![change(Sign::Insert, "a", counts(2, 1, 0))])
            .unwrap();
        let before = view.clone();
        assert_eq!(
            view.apply(vec![
                change(Sign::Retract, "a", counts(2, 1, 0)),
                change(Sign::Insert, "a", counts(1, 2, 0)),
                change(Sign::Insert, "b", counts(1, 0, 0)),
            ]),
            Err(ViewError::InvalidCounts { group: "a".into() })
        );
        assert_eq!(view, before);
    }

    #[test]
    fn a_zero_capacity_feed_read_cannot_establish_an_exhausted_window() {
        assert_eq!(
            plan_page(TxId(10), TxId(20), vec![], 0),
            Err(FeedError::InvalidLimit)
        );
    }

    #[test]
    fn admitted_pages_advance_only_their_exact_source_frontier() {
        let position = CommittedPosition::bootstrap(Snapshot::parse("10:20:").unwrap());
        let page = plan_page(TxId(10), TxId(20), vec![row(11, "a")], 1).unwrap();
        let advanced = position.advance(&page).unwrap();
        assert_eq!(advanced.frontier(), TxId(12));
        assert_eq!(advanced.bootstrap_snapshot(), position.bootstrap_snapshot());
        assert_eq!(advanced.advance(&page), Err(FeedError::PageStartMismatch));
        let foreign = plan_page(TxId(9), TxId(10), vec![], 1).unwrap();
        assert_eq!(
            position.advance(&foreign),
            Err(FeedError::PageStartMismatch)
        );
        let idle = plan_page(TxId(12), TxId(12), vec![], 1).unwrap();
        assert_eq!(advanced.advance(&idle).unwrap(), advanced);
    }

    #[test]
    fn impossible_count_insertions_cannot_seed_an_admitted_view() {
        for raw in [counts(0, 1, 0), counts(0, 0, 1), counts(1, 0, 2)] {
            let mut view = CountView::default();
            assert_eq!(
                view.apply(vec![change(Sign::Insert, "a", raw)]),
                Err(ViewError::InvalidCounts { group: "a".into() }),
            );
            assert_eq!(view, CountView::default());
        }
        assert!(counts(u64::MAX, u64::MAX, u64::MAX).is_valid());
    }

    #[test]
    fn snapshots_are_parsed_and_inconsistent_ones_refused() {
        let snapshot = Snapshot::parse("100:105:101,103").unwrap();
        assert_eq!((snapshot.xmin(), snapshot.xmax()), (TxId(100), TxId(105)));
        assert_eq!(snapshot.as_str(), "100:105:101,103");
        assert_eq!(Snapshot::parse("7:7:").unwrap().xmin(), TxId(7));
        for bad in [
            "", "1", "1:2", "a:2:", "5:4:", "1:5:0", "1:5:5", "1:5:x", "-1:5:",
        ] {
            assert_eq!(
                Snapshot::parse(bad),
                Err(FeedError::MalformedPosition),
                "{bad}"
            );
        }
        let position = CommittedPosition::bootstrap(snapshot);
        assert_eq!(position.frontier(), TxId(100));
    }

    #[test]
    fn an_exhausted_window_advances_the_frontier_to_the_settled_bound() {
        let page = plan_page(TxId(10), TxId(20), vec![row(11, "x"), row(11, "y")], 3).unwrap();
        assert_eq!((page.transactions(), page.frontier()), (1, TxId(20)));
        assert_eq!(page.changes().len(), 2);
        let idle = plan_page(TxId(10), TxId(20), vec![], 3).unwrap();
        assert_eq!((idle.transactions(), idle.frontier()), (0, TxId(20)));
        assert!(idle.changes().is_empty());
    }

    #[test]
    fn a_full_page_stops_after_its_last_whole_transaction() {
        let page = plan_page(
            TxId(10),
            TxId(20),
            vec![row(10, "a"), row(12, "b"), row(12, "c")],
            2,
        )
        .unwrap();
        assert_eq!((page.transactions(), page.frontier()), (2, TxId(13)));
    }

    #[test]
    fn rows_outside_the_window_or_order_and_a_regressing_bound_are_refused() {
        let cases = [
            (
                TxId(10),
                TxId(20),
                vec![row(9, "a")],
                FeedError::RowOutsideWindow,
            ),
            (
                TxId(10),
                TxId(20),
                vec![row(20, "a")],
                FeedError::RowOutsideWindow,
            ),
            (
                TxId(10),
                TxId(20),
                vec![row(12, "a"), row(11, "b")],
                FeedError::RowOutsideWindow,
            ),
            (
                TxId(10),
                TxId(20),
                vec![row(11, "a"), row(12, "b"), row(13, "c")],
                FeedError::RowOutsideWindow,
            ),
            (TxId(10), TxId(9), vec![], FeedError::SettledBehindFrontier),
        ];
        for (frontier, settled, rows, error) in cases {
            assert_eq!(plan_page(frontier, settled, rows, 2), Err(error));
        }
    }

    #[test]
    fn only_an_exact_fence_lets_a_view_answer_or_be_compared() {
        let fence = |covers_frontier, unapplied_change| Fence {
            covers_frontier,
            unapplied_change,
        };
        assert_eq!(freshness(fence(true, false)), Freshness::Exact);
        assert_eq!(freshness(fence(true, true)), Freshness::Lagging);
        assert_eq!(freshness(fence(false, false)), Freshness::Ahead);
        assert_eq!(freshness(fence(false, true)), Freshness::Ahead);
        let (pg, view) = (counts(4, 2, 1), counts(4, 2, 0));
        assert_eq!(
            shadow_compare(pg, pg, Freshness::Exact),
            ShadowOutcome::Match
        );
        assert_eq!(
            shadow_compare(pg, view, Freshness::Exact),
            ShadowOutcome::Mismatch
        );
        assert_eq!(
            shadow_compare(pg, view, Freshness::Lagging),
            ShadowOutcome::NotComparable(Freshness::Lagging)
        );
    }

    #[test]
    fn read_models_have_one_spelling_each() {
        for model in [
            ReadModel::Postgres,
            ReadModel::DbspShadow,
            ReadModel::DbspServe,
        ] {
            assert_eq!(model.as_str().parse(), Ok(model));
        }
        for bad in ["", "dbsp", "DBSP-SERVE", "postgresql"] {
            assert_eq!(bad.parse::<ReadModel>(), Err(()));
        }
        assert!(!ReadModel::Postgres.uses_view());
    }

    /// Applying the row changes between consecutive member states equals recounting the latest.
    fn diff(
        before: &HashMap<String, GroupCounts>,
        after: &HashMap<String, GroupCounts>,
    ) -> Vec<CountChange> {
        let mut changes = Vec::new();
        for (group, counts) in before {
            if after.get(group) != Some(counts) {
                changes.push(change(Sign::Retract, group, *counts));
            }
        }
        for (group, counts) in after {
            if before.get(group) != Some(counts) {
                changes.push(change(Sign::Insert, group, *counts));
            }
        }
        changes
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_013), ..ProptestConfig::default() })]
        #[test]
        fn admitted_pages_match_a_sorted_set_frontier_oracle(
            first in any::<u64>(), second in any::<u64>(),
            txids in proptest::collection::vec(any::<u64>(), 0..9), limit in 1_usize..9,
        ) {
            let start = first.min(second);
            let settled = first.max(second);
            let mut txids: Vec<_> = txids.into_iter()
                .filter(|id| *id >= start && *id < settled).collect();
            txids.sort_unstable();
            let distinct: std::collections::BTreeSet<_> = txids.iter().copied().collect();
            let rows = txids.iter().map(|id| row(*id, "a")).collect();
            let actual = plan_page(TxId(start), TxId(settled), rows, limit);
            if distinct.len() > limit {
                prop_assert_eq!(actual, Err(FeedError::RowOutsideWindow));
            } else {
                let page = actual.unwrap();
                let next = if distinct.len() == limit {
                    *distinct.last().unwrap() as u128 + 1
                } else { settled as u128 };
                prop_assert_eq!(page.frontier().get() as u128, next);
                prop_assert_eq!(page.transactions(), distinct.len());
                prop_assert_eq!(page.changes().len(), txids.len());
                prop_assert!(start <= page.frontier().get() && page.frontier().get() <= settled);
                let position = CommittedPosition::bootstrap(
                    Snapshot::parse(&format!("{start}:{start}:")).unwrap(),
                );
                let advanced = position.advance(&page).unwrap();
                prop_assert_eq!(advanced.frontier(), page.frontier());
                prop_assert_eq!(advanced.bootstrap_snapshot(), position.bootstrap_snapshot());
                if page.frontier() != position.frontier() {
                    prop_assert_eq!(advanced.advance(&page), Err(FeedError::PageStartMismatch));
                }
            }
        }
    }

    proptest! {
        #[test]
        fn applying_successive_differences_tracks_a_recount(
            states in proptest::collection::vec(
                proptest::collection::vec((0u8..4, any::<[bool; 2]>()), 0..12),
                1..8,
            )
        ) {
            let mut view = CountView::default();
            let mut previous = HashMap::new();
            for state in states {
                let members: Vec<Member> = state.iter().enumerate().map(|(index, (group, flags))| Member {
                    group: format!("g{group}"),
                    id: format!("m{index}"),
                    flags: *flags,
                }).collect();
                let current = count_members(&members);
                let mut changes = diff(&previous, &current);
                changes.reverse();
                view.apply(changes).unwrap();
                let held: HashMap<String, GroupCounts> =
                    view.groups().map(|(group, counts)| (group.to_owned(), counts)).collect();
                prop_assert_eq!(&held, &current);
                previous = current;
            }
        }
    }
}
