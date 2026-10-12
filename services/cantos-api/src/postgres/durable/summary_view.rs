//! Experimental, opt-in DBSP job summaries over PostgreSQL's committed step-change feed.
//!
//! A dedicated thread owns one in-memory DBSP engine and one PostgreSQL session with the
//! application role. It bootstraps from a REPEATABLE READ snapshot of `durable_steps`, then
//! applies whole committed transactions from `durable_step_changes` below each poll's `xmin`
//! (see [`crate::incremental`] for the committed-position contract). The published view and its
//! position change together under one lock, so a reader never sees a partial step.
//!
//! PostgreSQL stays the authority. A request is always authenticated and authorized by the
//! existing transaction first; the view is consulted only for that authorized job, and only a
//! per-request fence evaluated in the request's own snapshot lets its counts be used. Any doubt
//! (starting, failed, lagging, ahead) falls back to the authoritative query under a concurrency
//! bound, or fails closed as unavailable. Nothing here is checkpointed: every start rebuilds.
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde::Serialize;
use tokio::sync::Semaphore;
use tokio_postgres::{Client, Config, IsolationLevel, NoTls, Row};

use super::{status, summary, DurableError, JobSummary};
use crate::incremental::{
    dbsp::{DbspCounts, EngineError},
    freshness, plan_page, shadow_compare, CommittedPosition, CountView, FeedError, FeedRow, Fence,
    Freshness, GroupCounts, Member, MemberChange, ReadModel, ShadowOutcome, Sign, Snapshot, TxId,
};
use crate::postgres::StoreError;

const BOOTSTRAP_CURSOR: &str = "DECLARE durable_summary_bootstrap NO SCROLL CURSOR FOR SELECT job_id,id,status='succeeded',reused_clip_id IS NOT NULL FROM durable_steps";
const BOOTSTRAP_FETCH: &str = "FETCH 10000 FROM durable_summary_bootstrap";
/// Whole transactions in `[frontier, settled)` that the bootstrap snapshot did not include.
const PAGE: &str = "WITH page AS (SELECT DISTINCT txid FROM durable_step_changes WHERE txid>=$1::text::xid8 AND txid<$2::text::xid8 AND NOT pg_visible_in_snapshot(txid,$3::text::pg_snapshot) ORDER BY txid LIMIT $4) SELECT c.txid::text,c.job_id,c.step_id,c.weight::bigint,c.succeeded,c.reused FROM durable_step_changes c JOIN page p ON p.txid=c.txid ORDER BY c.txid,c.seq";
/// The job row and the fence, read in one statement and therefore one snapshot.
const FENCE: &str = "SELECT j.id,j.status,j.reason,$2::text::xid8<=pg_snapshot_xmin(pg_current_snapshot()) AS covers_frontier,EXISTS(SELECT 1 FROM durable_step_changes c WHERE c.job_id=j.id AND c.txid>=$2::text::xid8 AND NOT pg_visible_in_snapshot(c.txid,$3::text::pg_snapshot)) AS unapplied_change FROM durable_jobs j WHERE j.run_id=$1";
/// The fence plus the authoritative counts in the same snapshot, for shadow comparison.
const SHADOW: &str = "SELECT j.id,j.status,j.reason,$2::text::xid8<=pg_snapshot_xmin(pg_current_snapshot()) AS covers_frontier,EXISTS(SELECT 1 FROM durable_step_changes c WHERE c.job_id=j.id AND c.txid>=$2::text::xid8 AND NOT pg_visible_in_snapshot(c.txid,$3::text::pg_snapshot)) AS unapplied_change,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id) AS total,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id AND s.status='succeeded') AS done,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id AND s.reused_clip_id IS NOT NULL) AS reused FROM durable_jobs j WHERE j.run_id=$1";

#[derive(Debug, Clone)]
pub struct ViewConfig {
    /// Sleep between polls once the feed has caught up.
    pub poll_interval: Duration,
    /// Whole transactions applied per engine step.
    pub transactions_per_step: usize,
    /// Delay before rebuilding after a failure.
    pub retry_after_failure: Duration,
}

impl Default for ViewConfig {
    fn default() -> Self {
        ViewConfig {
            poll_interval: Duration::from_millis(20),
            transactions_per_step: 1000,
            retry_after_failure: Duration::from_millis(500),
        }
    }
}

/// Faults a test can inject at named points of the maintainer, each consumed once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailPoint {
    /// Read the next non-empty page, then drop it unapplied, as a crash before applying would.
    DropNextPage,
    /// Feed the next non-empty page to the engine twice, as a duplicate delivery would.
    DuplicateNextPage,
    /// Fail the session after reading the next page, forcing a rebuild.
    FailNextPage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    Starting,
    Failed,
    Stopped,
}

/// What a request may use: a position-stamped count, or nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    Ready {
        position: CommittedPosition,
        counts: GroupCounts,
    },
    Unavailable(Unavailable),
}

enum State {
    Starting,
    Ready {
        generation: u64,
        position: CommittedPosition,
        view: CountView,
    },
    Failed {
        reason: &'static str,
    },
    Stopped,
}

#[derive(Default)]
struct Counters {
    served: AtomicU64,
    fallback_lagging: AtomicU64,
    fallback_ahead: AtomicU64,
    fallback_unavailable: AtomicU64,
    fallback_rejected: AtomicU64,
    shadow_matches: AtomicU64,
    shadow_mismatches: AtomicU64,
    shadow_not_comparable: AtomicU64,
    bootstraps: AtomicU64,
    bootstrap_rows: AtomicU64,
    last_bootstrap_micros: AtomicU64,
    failures: AtomicU64,
    polls: AtomicU64,
    applied_transactions: AtomicU64,
    applied_changes: AtomicU64,
}

fn bump(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

/// A point-in-time report of the view and its counters; no counts or identifiers.
#[derive(Debug, Clone, Serialize)]
pub struct ViewStatus {
    pub state: &'static str,
    pub failure: Option<&'static str>,
    pub generation: u64,
    pub frontier: Option<u64>,
    pub groups: usize,
    pub served: u64,
    pub fallback_lagging: u64,
    pub fallback_ahead: u64,
    pub fallback_unavailable: u64,
    pub fallback_rejected: u64,
    pub shadow_matches: u64,
    pub shadow_mismatches: u64,
    pub shadow_not_comparable: u64,
    pub bootstraps: u64,
    pub bootstrap_rows: u64,
    pub last_bootstrap_micros: u64,
    pub failures: u64,
    pub polls: u64,
    pub applied_transactions: u64,
    pub applied_changes: u64,
}

struct Shared {
    state: RwLock<State>,
    counters: Counters,
    failpoint: Mutex<Option<FailPoint>>,
    stop: AtomicBool,
}

impl Shared {
    fn publish(&self, state: State) {
        if let Ok(mut current) = self.state.write() {
            *current = state;
        }
    }

    fn take_failpoint(&self) -> Option<FailPoint> {
        self.failpoint
            .lock()
            .ok()
            .and_then(|mut point| point.take())
    }
}

/// The running maintainer. Stopping or dropping it stops and joins the thread.
pub struct SummaryView {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl SummaryView {
    /// Starts the maintainer with the application-role configuration of the [`super::Store`].
    pub fn start(config: Config, options: ViewConfig) -> Result<Self, StoreError> {
        let shared = Arc::new(Shared {
            state: RwLock::new(State::Starting),
            counters: Counters::default(),
            failpoint: Mutex::new(None),
            stop: AtomicBool::new(false),
        });
        let thread_shared = shared.clone();
        let thread = std::thread::Builder::new()
            .name("cantos-summary-view".into())
            .spawn(move || maintain(&thread_shared, &config, &options))
            .map_err(|_| StoreError::Unavailable)?;
        Ok(SummaryView {
            shared,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// The counts of one group and the exact position they reflect.
    pub fn lookup(&self, group: &str) -> Lookup {
        match self.shared.state.read().as_deref() {
            Ok(State::Ready { position, view, .. }) => Lookup::Ready {
                position: position.clone(),
                counts: view.get(group),
            },
            Ok(State::Starting) => Lookup::Unavailable(Unavailable::Starting),
            Ok(State::Failed { .. }) | Err(_) => Lookup::Unavailable(Unavailable::Failed),
            Ok(State::Stopped) => Lookup::Unavailable(Unavailable::Stopped),
        }
    }

    /// The whole view and its position as one consistent copy, for oracles and diagnostics.
    pub fn snapshot(&self) -> Option<(CommittedPosition, Vec<(String, GroupCounts)>)> {
        match self.shared.state.read().as_deref() {
            Ok(State::Ready { position, view, .. }) => Some((
                position.clone(),
                view.groups()
                    .map(|(group, counts)| (group.to_owned(), counts))
                    .collect(),
            )),
            _ => None,
        }
    }

    pub fn status(&self) -> ViewStatus {
        let counter = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        let counters = &self.shared.counters;
        let (state, failure, generation, frontier, groups) =
            match self.shared.state.read().as_deref() {
                Ok(State::Starting) => ("starting", None, 0, None, 0),
                Ok(State::Ready {
                    generation,
                    position,
                    view,
                }) => (
                    "ready",
                    None,
                    *generation,
                    Some(position.frontier().get()),
                    view.len(),
                ),
                Ok(State::Failed { reason }) => ("failed", Some(*reason), 0, None, 0),
                Ok(State::Stopped) => ("stopped", None, 0, None, 0),
                Err(_) => ("failed", Some("lock_poisoned"), 0, None, 0),
            };
        ViewStatus {
            state,
            failure,
            generation,
            frontier,
            groups,
            served: counter(&counters.served),
            fallback_lagging: counter(&counters.fallback_lagging),
            fallback_ahead: counter(&counters.fallback_ahead),
            fallback_unavailable: counter(&counters.fallback_unavailable),
            fallback_rejected: counter(&counters.fallback_rejected),
            shadow_matches: counter(&counters.shadow_matches),
            shadow_mismatches: counter(&counters.shadow_mismatches),
            shadow_not_comparable: counter(&counters.shadow_not_comparable),
            bootstraps: counter(&counters.bootstraps),
            bootstrap_rows: counter(&counters.bootstrap_rows),
            last_bootstrap_micros: counter(&counters.last_bootstrap_micros),
            failures: counter(&counters.failures),
            polls: counter(&counters.polls),
            applied_transactions: counter(&counters.applied_transactions),
            applied_changes: counter(&counters.applied_changes),
        }
    }

    /// Waits until the view is ready, up to `timeout`.
    pub async fn wait_ready(&self, timeout: Duration) -> bool {
        self.wait_until(timeout, |status| status.state == "ready")
            .await
            .is_some()
    }

    /// Polls [`Self::status`] until `done` holds, up to `timeout`.
    pub async fn wait_until(
        &self,
        timeout: Duration,
        done: impl Fn(&ViewStatus) -> bool,
    ) -> Option<ViewStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            let status = self.status();
            if done(&status) {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    pub fn inject(&self, point: FailPoint) {
        if let Ok(mut slot) = self.shared.failpoint.lock() {
            *slot = Some(point);
        }
    }

    /// Marks the view unusable until the maintainer rebuilds it from a new snapshot.
    pub fn invalidate(&self, reason: &'static str) {
        bump(&self.shared.counters.failures);
        self.shared.publish(State::Failed { reason });
    }

    /// Stops the maintainer; later lookups report it as stopped. Idempotent.
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        let thread = self.thread.lock().ok().and_then(|mut thread| thread.take());
        if let Some(thread) = thread {
            let _ = thread.join();
        }
        self.shared.publish(State::Stopped);
    }
}

impl Drop for SummaryView {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Why a maintainer session ended; the next session rebuilds from a new snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    Feed,
    Engine,
    Position,
    View,
    Injected,
}

impl Failure {
    fn reason(self) -> &'static str {
        match self {
            Failure::Feed => "feed_unavailable",
            Failure::Engine => "engine_failed",
            Failure::Position => "position_invalid",
            Failure::View => "view_inconsistent",
            Failure::Injected => "injected",
        }
    }
}

impl From<tokio_postgres::Error> for Failure {
    fn from(_: tokio_postgres::Error) -> Self {
        Failure::Feed
    }
}

impl From<EngineError> for Failure {
    fn from(_: EngineError) -> Self {
        Failure::Engine
    }
}

impl From<FeedError> for Failure {
    fn from(_: FeedError) -> Self {
        Failure::Position
    }
}

fn maintain(shared: &Shared, config: &Config, options: &ViewConfig) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        shared.publish(State::Failed {
            reason: "runtime_unavailable",
        });
        return;
    };
    runtime.block_on(async {
        let mut generation = 0;
        while !shared.stop.load(Ordering::Relaxed) {
            generation += 1;
            if let Err(failure) = session(shared, config, options, generation).await {
                bump(&shared.counters.failures);
                shared.publish(State::Failed {
                    reason: failure.reason(),
                });
                pause(shared, options.retry_after_failure).await;
            }
        }
    });
}

/// Sleeps in short slices so a stop request is honoured promptly.
async fn pause(shared: &Shared, duration: Duration) {
    let deadline = Instant::now() + duration;
    while !shared.stop.load(Ordering::Relaxed) && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5).min(duration)).await;
    }
}

/// One engine lifetime: bootstrap, then follow the feed until stopped or failed.
async fn session(
    shared: &Shared,
    config: &Config,
    options: &ViewConfig,
    generation: u64,
) -> Result<(), Failure> {
    let (mut client, connection) = config.connect(NoTls).await?;
    let connection = tokio::spawn(async move {
        let _ = connection.await;
    });
    let result = follow(shared, &mut client, options, generation).await;
    drop(client);
    let _ = connection.await;
    result
}

async fn follow(
    shared: &Shared,
    client: &mut Client,
    options: &ViewConfig,
    generation: u64,
) -> Result<(), Failure> {
    let started = Instant::now();
    let mut engine = DbspCounts::start()?;
    let (mut position, view, rows) = bootstrap(client, &mut engine).await?;
    let counters = &shared.counters;
    counters.bootstrap_rows.store(rows, Ordering::Relaxed);
    counters.last_bootstrap_micros.store(
        u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    bump(&counters.bootstraps);
    shared.publish(State::Ready {
        generation,
        position: position.clone(),
        view,
    });
    while !shared.stop.load(Ordering::Relaxed) {
        let full = poll(
            shared,
            client,
            &mut engine,
            &mut position,
            options,
            generation,
        )
        .await?;
        if !full {
            pause(shared, options.poll_interval).await;
        }
    }
    Ok(())
}

/// Loads every step visible in one REPEATABLE READ snapshot; that snapshot is the position.
async fn bootstrap(
    client: &mut Client,
    engine: &mut DbspCounts,
) -> Result<(CommittedPosition, CountView, u64), Failure> {
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await?;
    let snapshot: String = tx
        .query_one("SELECT pg_current_snapshot()::text", &[])
        .await?
        .get(0);
    let position = CommittedPosition::bootstrap(Snapshot::parse(&snapshot)?);
    tx.batch_execute(BOOTSTRAP_CURSOR).await?;
    let mut view = CountView::default();
    let mut loaded = 0_u64;
    loop {
        let rows = tx.query(BOOTSTRAP_FETCH, &[]).await?;
        if rows.is_empty() {
            break;
        }
        loaded += rows.len() as u64;
        let changes = rows
            .iter()
            .map(|row| MemberChange {
                sign: Sign::Insert,
                member: member(row, 0),
            })
            .collect();
        view.apply(engine.step(changes)?)
            .map_err(|_| Failure::View)?;
    }
    tx.commit().await?;
    Ok((position, view, loaded))
}

fn member(row: &Row, first: usize) -> Member {
    Member {
        group: row.get(first),
        id: row.get(first + 1),
        flags: [row.get(first + 2), row.get(first + 3)],
    }
}

fn feed_row(row: &Row) -> Result<FeedRow, Failure> {
    let txid: TxId = row.get::<_, String>(0).parse()?;
    let sign = Sign::from_weight(row.get(3)).ok_or(FeedError::InvalidWeight)?;
    Ok(FeedRow {
        txid,
        change: MemberChange {
            sign,
            member: Member {
                group: row.get(1),
                id: row.get(2),
                flags: [row.get(4), row.get(5)],
            },
        },
    })
}

/// Reads and applies at most one page of whole transactions. Returns whether the page was full,
/// in which case the caller polls again without waiting.
async fn poll(
    shared: &Shared,
    client: &mut Client,
    engine: &mut DbspCounts,
    position: &mut CommittedPosition,
    options: &ViewConfig,
    generation: u64,
) -> Result<bool, Failure> {
    bump(&shared.counters.polls);
    let tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .await?;
    let settled: TxId = tx
        .query_one("SELECT pg_snapshot_xmin(pg_current_snapshot())::text", &[])
        .await?
        .get::<_, String>(0)
        .parse()?;
    let frontier = position.frontier();
    let limit = options.transactions_per_step.max(1);
    let rows = if settled > frontier {
        let limit_param = i64::try_from(limit).unwrap_or(i64::MAX);
        tx.query(
            PAGE,
            &[
                &frontier.to_string(),
                &settled.to_string(),
                &position.bootstrap_snapshot().as_str(),
                &limit_param,
            ],
        )
        .await?
    } else {
        Vec::new()
    };
    tx.commit().await?;
    let rows = rows.iter().map(feed_row).collect::<Result<Vec<_>, _>>()?;
    let page = plan_page(frontier, settled, rows, limit)?;
    if page.frontier == frontier {
        return Ok(false);
    }
    let changes = if page.changes.is_empty() {
        Vec::new()
    } else {
        match shared.take_failpoint() {
            Some(FailPoint::DropNextPage) => return Ok(false),
            Some(FailPoint::FailNextPage) => return Err(Failure::Injected),
            Some(FailPoint::DuplicateNextPage) => {
                engine.step(page.changes.clone())?;
                engine.step(page.changes.clone())?
            }
            None => engine.step(page.changes.clone())?,
        }
    };
    let next = position.advance(&page);
    let mut state = shared.state.write().map_err(|_| Failure::View)?;
    match &mut *state {
        State::Ready {
            generation: current,
            position: published,
            view,
        } if *current == generation => {
            view.apply(changes).map_err(|_| Failure::View)?;
            *published = next.clone();
        }
        // Invalidated by a reader (shadow mismatch); rebuild from a new snapshot.
        _ => return Err(Failure::View),
    }
    drop(state);
    *position = next;
    let counters = &shared.counters;
    counters
        .applied_transactions
        .fetch_add(page.transactions as u64, Ordering::Relaxed);
    counters
        .applied_changes
        .fetch_add(page.changes.len() as u64, Ordering::Relaxed);
    Ok(page.transactions == limit)
}

/// A view attached to a [`super::Store`] for `job_status`, with its read model and the bound on
/// concurrent fallbacks to the authoritative query.
pub struct AttachedView {
    view: Arc<SummaryView>,
    model: ReadModel,
    fallback: Semaphore,
    fallback_wait: Duration,
}

impl AttachedView {
    pub fn new(
        view: Arc<SummaryView>,
        model: ReadModel,
        fallback_limit: usize,
        fallback_wait: Duration,
    ) -> Self {
        AttachedView {
            view,
            model,
            fallback: Semaphore::new(fallback_limit),
            fallback_wait,
        }
    }

    /// The summary of an already authorized run, inside the caller's transaction.
    pub(super) async fn summary(
        &self,
        tx: &deadpool_postgres::Transaction<'_>,
        run: &str,
    ) -> Result<JobSummary, DurableError> {
        let counters = &self.view.shared.counters;
        // The run→job binding is immutable, so it may be read outside the fence's snapshot.
        let job: String = tx
            .query_opt("SELECT id FROM durable_jobs WHERE run_id=$1", &[&run])
            .await?
            .ok_or(DurableError::Store(StoreError::NotFound))?
            .get(0);
        let (position, counts) = match self.view.lookup(&job) {
            Lookup::Ready { position, counts } => (position, counts),
            Lookup::Unavailable(_) => {
                bump(&counters.fallback_unavailable);
                return self.fallback(tx, run).await;
            }
        };
        let frontier = position.frontier().to_string();
        let bootstrap = position.bootstrap_snapshot().as_str();
        match self.model {
            ReadModel::DbspServe => {
                let row = tx.query_one(FENCE, &[&run, &frontier, &bootstrap]).await?;
                match freshness(fence(&row)) {
                    Freshness::Exact => {
                        bump(&counters.served);
                        job_summary(run, &row, counts)
                    }
                    Freshness::Lagging => {
                        bump(&counters.fallback_lagging);
                        self.fallback(tx, run).await
                    }
                    Freshness::Ahead => {
                        bump(&counters.fallback_ahead);
                        self.fallback(tx, run).await
                    }
                }
            }
            ReadModel::DbspShadow => {
                let row = tx.query_one(SHADOW, &[&run, &frontier, &bootstrap]).await?;
                let authoritative = GroupCounts {
                    members: count(&row, "total")?,
                    flagged: [count(&row, "done")?, count(&row, "reused")?],
                };
                match shadow_compare(authoritative, counts, freshness(fence(&row))) {
                    ShadowOutcome::Match => bump(&counters.shadow_matches),
                    ShadowOutcome::Mismatch => {
                        bump(&counters.shadow_mismatches);
                        self.view.invalidate("shadow_mismatch");
                    }
                    ShadowOutcome::NotComparable(_) => bump(&counters.shadow_not_comparable),
                }
                job_summary(run, &row, authoritative)
            }
            ReadModel::Postgres => Ok(summary(tx, run).await?),
        }
    }

    /// The authoritative query, at most `fallback_limit` at a time; otherwise fail closed.
    async fn fallback(
        &self,
        tx: &deadpool_postgres::Transaction<'_>,
        run: &str,
    ) -> Result<JobSummary, DurableError> {
        if self.model == ReadModel::DbspShadow {
            return summary(tx, run).await;
        }
        let Ok(Ok(_permit)) =
            tokio::time::timeout(self.fallback_wait, self.fallback.acquire()).await
        else {
            bump(&self.view.shared.counters.fallback_rejected);
            return Err(DurableError::Store(StoreError::Unavailable));
        };
        summary(tx, run).await
    }
}

fn fence(row: &Row) -> Fence {
    Fence {
        covers_frontier: row.get("covers_frontier"),
        unapplied_change: row.get("unapplied_change"),
    }
}

fn count(row: &Row, column: &str) -> Result<u64, DurableError> {
    u64::try_from(row.get::<_, i64>(column)).map_err(|_| DurableError::Corrupt)
}

fn job_summary(run: &str, row: &Row, counts: GroupCounts) -> Result<JobSummary, DurableError> {
    let [completed, reused] = counts.flagged;
    Ok(JobSummary {
        run_id: run.into(),
        job_id: row.get("id"),
        status: status(row)?,
        reason: row.get("reason"),
        total_steps: counts.members,
        completed_steps: completed,
        reused_steps: reused,
    })
}
