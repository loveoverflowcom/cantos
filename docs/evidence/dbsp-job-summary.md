# DBSP job-summary read model — implementation evidence

- Date: 2026-10-12
- Issue: [#58](https://github.com/loveoverflowcom/cantos/issues/58); decision:
  [ADR 0017](../decisions/0017-experimental-dbsp-job-summary.md)
- Base: `develop` at `5d26d9b6077bbba09e4e9315ae06661658c53e8d` when the work started; later merged
  with `db1433906e0caeab2b3bc2239476d4e993bf5b28` (see Gate results)
- Status: **implemented experimental** DBSP read model, feature-gated and local/development only.
  **Production adoption is not admitted.** PostgreSQL remains the authority and the default path.

## What is implemented, and what is not

Implemented and exercised here:

- `dbsp = "=0.198.0"` (Feldera's crate; MIT OR Apache-2.0) as an optional dependency of `cantos-api`
  behind `dbsp-experimental`, compiled and tested on Rust 1.87.0. One single-worker, in-memory
  runtime per process.
- `incremental` (always compiled, engine-independent) and `incremental::dbsp` (the circuit).
- Migration 0009: `durable_step_changes`, a trigger-written transactional outbox stamped with `xid8`.
- `postgres::durable::summary_view`: bootstrap/poll maintainer, exact committed position, fenced
  `dbsp-serve`, `dbsp-shadow`, bounded fallback, invalidation and rebuild.
- The opt-in consumer: `production-jobs status|watch` with `CANTOS_JOB_SUMMARY_READ_MODEL`.
- `cargo xtask build cantos-api --dbsp-experimental`, `cargo xtask verify dbsp`, the DBSP tests in
  the PostgreSQL gate and `scripts/benchmark_job_summary_dbsp.py`.

Not implemented: a Feldera service or connector, checkpointed engine state, outbox pruning or a
consumer registry, logical-replication CDC, an HTTP route, production configuration or monitoring,
multi-worker DBSP, and any other read model. Nothing here is `provider-live-tested`,
`device-tested` or `production-observed`.

## Read inventory

| Path / consumer | Query and bounds | Authority / feeding writes |
| --- | --- | --- |
| `production-jobs status` and new `watch` → `Store::job_status` | Session, run point read, owner `Review` authorization, then `summary`: job point read plus three correlated `COUNT(*)` over one job's steps (1 to 1,000) | Current session/actor/script permission on every read |
| `enqueue_job`, `control_job`, `resolve_not_executed` → `summary` | Same counts inside their own write transaction | Unchanged: they must read their own uncommitted writes, so they never use the view |
| Feeding writes of the counts | Step inserts (enqueue), `succeeded` (accept), `succeeded`+`reused_clip_id` (reuse selection) | Leases, heartbeats, pauses, cancels, failures and retries change no projected fact and write no feed row; deletes and truncation are rejected by trigger |

The view keys counts by job id and never stores authorization. `job_status` resolves the run →
job binding (immutable by trigger) only after authorization succeeds.

## Committed position and freshness contract

The contract is stated once, in [`incremental`](../../services/cantos-api/src/platform/incremental/mod.rs):
a view at `{frontier F, bootstrap S0}` reflects every committed row visible in its REPEATABLE READ
bootstrap snapshot or written by a transaction below `F`. Each poll applies whole transactions in
`[F, xmin)` of its own snapshot that `S0` did not contain, ordered by `(xid8, seq)`, as one engine
transaction, and publishes counts and frontier under one lock. A request uses the view only when,
in the same statement as its job row, `F <= xmin` holds and the job has no visible change at or
after `F` outside `S0` (`Exact`). `Lagging` and `Ahead` fall back; `Ahead` is a defensive case that
cannot arise while the lookup precedes the statement under READ COMMITTED. A long transaction
holds `xmin`, and so the frontier, back: that costs fallbacks, never stale answers.

## Claims ledger

| Claim / invariant | Failure mode | Cheapest oracle | Evidence level |
| --- | --- | --- | --- |
| A served summary equals the PostgreSQL summary at the same committed position | Stale or partial counts served as authoritative | Grouped recount in the same snapshot as the fence; API response equality | integration-tested, differentially-tested |
| DBSP applies deltas, not recomputation | An "incremental" path that recounts | Linear aggregate; `applied_changes` equals the feed rows (12 + 2·8 in the transition test) and `bootstraps` stays 1 | integration-tested, example-tested |
| No committed change is skipped, none applied twice | Out-of-order commit skipped; replay double counts | Older open transaction test; dropped-page replay; handoff race test | integration-tested, fault-injected, mutation-tested (hand) |
| No transaction is ever half applied | A multi-row commit partially visible | Whole-transaction pages (`plan_page`); concurrent 4-row commits | example-tested, integration-tested |
| Duplicate delivery or an unmatched retraction never reaches the view | Silent double count | Integrity operators; duplicate-page injection forces a rebuild | example-tested, fault-injected |
| Engine output is complete for large steps | Partial deltas after DBSP splits a step | `a_step_larger_than_the_engine_split_returns_its_complete_delta` | example-tested, mutation-tested (hand) |
| The view never answers before authentication and owner authorization | Cross-tenant or revoked read | Non-member, editor and revoked-session cases with view counters unchanged | integration-tested |
| Unavailable, failed or stopped views fall back within a bound, else fail closed | Stale answer or unbounded fallback | Invalidated, zero-capacity and stopped-view cases | integration-tested, fault-injected |
| The application role cannot forge or alter the feed | Forged counts served | `INSERT`/`UPDATE`/`DELETE` as `cantos_app` → `42501`; a superuser forgery is caught by shadow and repaired by rebuild | integration-tested |
| The opt-in consumer works end to end | A CLI that leaks identifiers or answers differently by read model | The real `production-jobs` binary (built with the feature) for `status` under all three models and `watch` under `dbsp-serve`, against the PostgreSQL result; an invalid model is refused | integration-tested |
| The feature does not change existing behavior | `arbitrary_precision`/`rkyv` side effects | Full `cantos-api` suite with the feature | example-tested (whole suite), compiled |

## Differential and recovery checklist

Tests are in [`tests/durable_postgres.rs`](../../services/cantos-api/tests/durable_postgres.rs)
module `dbsp_summary_view` (12 tests on real PostgreSQL with the real DBSP runtime, including
`the_cli_prints_the_same_summary_under_every_read_model`, which spawns the real binary) and in
`incremental`/`incremental::dbsp` (15 unit and property tests).

| Edge class | Real test path, or reason inapplicable |
| --- | --- |
| Insert/update/delete and retractions | `dbsp_view_applies_inserts_transitions_and_reuse_as_deltas_without_rebuilding` (worker transitions, accepted and reused steps, pause/cancel); property `maintained_counts_equal_a_recount_of_the_current_members` (inserts, updates, deletes, arbitrary step splits); `retracting_the_last_member_removes_the_group`. Deletes are rejected in PostgreSQL, so deletion is engine-level only |
| Duplicates and delivery ordering | `a_dropped_page_is_replayed_once_and_duplicate_delivery_or_failure_rebuilds`; `dbsp_view_holds_its_frontier_behind_an_older_open_transaction_and_falls_back` (a later commit below an open older one); `a_change_delivered_twice_or_never_inserted_is_an_integrity_failure` |
| SQL and time semantics | `reused_clip_id IS NOT NULL` and status are projected to booleans in SQL; empty groups read as zero (unit); counts convert from `i64` only through checked conversion (`InvalidCount`). No timestamp, window, collation or text ordering enters this result, so time and Vietnamese ordering rows are inapplicable |
| Pagination/sort | Inapplicable: one job's summary is a point read with no page or order |
| Transaction boundaries | `concurrent_multi_row_commits_are_each_applied_whole_and_once` (6 writers × 5 commits × 4 rows); `dbsp_view_never_applies_a_rolled_back_or_still_open_transaction`; `plan_page` unit tests |
| Reconnect/restart | `a_restarted_view_rebuilds_while_writes_continue_without_gap_or_double_count`; injected failure after reading a page. A dropped TCP connection takes the same `feed_unavailable` → rebuild path but was not injected at the network level |
| Snapshot/rebuild | `a_transaction_committed_above_the_bootstrap_xmin_is_counted_once_by_the_snapshot`; `dbsp_view_bootstraps_existing_steps_and_serves_the_exact_authoritative_summary`; restart while writing |
| Tenant isolation and authorization | `the_view_never_answers_before_authentication_and_owner_authorization`. Job ids are global UUID keys reached only through an authorized run, so equal local keys across tenants cannot occur |
| Consistency/freshness and failure | Older-open-transaction test (`Lagging` serve fallback, shadow not comparable); `an_unavailable_view_falls_back_within_its_bound_and_otherwise_fails_closed`; `shadow_mode_detects_a_forged_feed_row_and_the_rebuild_repairs_the_view`; `Ahead` is unit-tested only |

## Hand-run mutants

Single mutants applied by hand to the shell, each run against the PostgreSQL DBSP suite and then
restored (a recorded procedure, not a repository harness):

| Mutant | Result |
| --- | --- |
| Remove the bootstrap-snapshot filter from the feed page | First **survived** the original ten tests (no test created the race and the integrity operators repaired it by rebuild). Added `a_transaction_committed_above_the_bootstrap_xmin_is_counted_once_by_the_snapshot` and no-rebuild assertions; then **killed** |
| Fence ignores unapplied changes (serve a lagging view) | **Killed** by the older-open-transaction test |
| Plain `output()` instead of `accumulate_output()` | **Killed** by `a_step_larger_than_the_engine_split_returns_its_complete_delta` |

The third was a real defect, found by the first benchmark attempt: with 81,000 bootstrap rows,
DBSP split a transaction into internal steps and `output()` returned only the last one. The view's
own retraction check refused the incomplete delta, so the view stayed `failed` and every read fell
back; nothing wrong was served. The fix accumulates the whole transaction's output, and the
regression test fails on the old code.

## Feature unification

`dbsp` forces `serde_json/arbitrary_precision` and brings `rkyv` trait implementations into the
package when the feature is on. Five existing expressions became ambiguous to type inference: two
zip-signature comparisons in `imports.rs` (production code; now compared as slices, same behavior)
and three test collects/comparisons (`jobs.rs`, `tests/script_diff.rs`). `cargo xtask verify dbsp`
then ran clippy with warnings denied and every non-ignored `cantos-api` test with the feature on
Rust 1.87.0: **PASS**, 371 passed, 0 failed, 125 ignored (database, process, live-provider and
benchmark tests) across 42 targets, in the final preflight below. A default build does not enable either crate feature.

## Benchmark

### Budgets, declared before the first completed run

Local observational budgets for this experiment, not production SLOs:

| Id | Budget |
| --- | --- |
| B1 | Correctness gates pass and the benchmark sees zero shadow mismatches |
| B2 | Read benefit worth considering adoption: `dbsp-serve` p95 at concurrency 1 at least 15% below the best PostgreSQL candidate for 1,000-step runs, and no more than 10% above it for 4-step runs |
| B3 | Write budget: the capture trigger adds at most 25% to single-step commit p95 and at most 100% to WAL bytes per commit |
| B4 | Freshness: commit-to-served p95 at most 100 ms with the default 20 ms poll |
| B5 | Rebuild: ready within 10 s for about 80,000 steps |

### Protocol

`python3 scripts/benchmark_job_summary_dbsp.py --runs 3` created three new disposable clusters and
ran `bench_job_summary_read_models` in each, built with the release test profile. Apple M4
(10 logical CPUs, 16 GiB), macOS 26.5.2, PostgreSQL 17.11 (Homebrew), Rust 1.87.0, `dbsp` 0.198.0
with one worker, default view configuration (20 ms poll, fallback bound 8). The source was
`5d26d9b` plus this change, uncommitted; the record lists the dirty paths.

- Fixture: synthetic only. 81,000 steps in three profiles (200 runs × 4, 200 × 100, 60 × 1,000
  steps), half of each run `succeeded` through complete attempt/observation chains, no reused
  steps, plus 50 runs that receive the write phases. `VACUUM (ANALYZE)` after loading.
- API reads: per profile, read model and repetition, 400 warm-up then 400 sequential
  `Store::job_status` calls with random runs, each authenticated, authorized and serialized to
  JSON; then two seconds at concurrency 8 (pool of 8). Three interleaved repetitions per cluster.
- Statements: 400 prepared executions per repetition on the application role.
- Writes: 1,500 single-row commits per phase, alternating capture on/off, `CHECKPOINT` before
  each phase so full-page images do not favour one.
- Raw record: `target/dbsp-evidence/20261012T164200/record.json` (SHA-256
  `c22c5dd3adcc6905fbd4066ce939f8916d9662916e458aa1bf12192116345343`, kept outside Git); its
  generated summary is [dbsp-job-summary-results.json](dbsp-job-summary-results.json).

Tables give the median across the nine repetitions. Ranges are in the summary file; tails on this
laptop were wide (several p95 ranges span 0.5 to 3 ms), so only differences larger than that
spread are read as results.

### Results

Statement time on the application role, p50 / p95 ms:

| Statement | 4 steps | 100 steps | 1,000 steps |
| --- | --- | --- | --- |
| Original correlated counts | 0.036 / 0.050 | 0.066 / 0.089 | 0.270 / 0.322 |
| Single-pass `count(*) FILTER` | 0.033 / 0.049 | 0.065 / 0.082 | 0.304 / 0.365 |
| Original with a partial index on reused steps | 0.034 / 0.058 | 0.041 / 0.080 | 0.078 / 0.113 |
| `dbsp-serve` fence (replaces the counts) | 0.029 / 0.049 | 0.030 / 0.061 | 0.031 / 0.057 |
| Original again after dropping the index (control) | 0.036 / 0.055 | 0.068 / 0.141 | 0.274 / 0.368 |

After `VACUUM` the step counts are index-only; the reused count still reads every step's heap row
(`EXPLAIN (ANALYZE, BUFFERS)` in the record), which the partial index removes.

`job_status` at concurrency 1, p50 / p95 ms:

| Read model | 4 steps | 100 steps | 1,000 steps |
| --- | --- | --- | --- |
| `postgres` (original) | 0.524 / 0.904 | 0.582 / 1.084 | 0.819 / 1.139 |
| `postgres` with the partial index | 0.588 / 1.397 | 0.589 / 1.211 | 0.725 / 1.866 |
| `postgres` after dropping it (control) | 0.514 / 0.754 | 0.573 / 1.039 | 0.802 / 1.506 |
| `dbsp-serve` | 0.626 / 1.313 | 0.561 / 0.851 | 0.575 / 1.052 |
| `dbsp-shadow` | 0.845 / 2.250 | 0.681 / 1.335 | 0.964 / 1.792 |

`job_status` at concurrency 8, requests per second / p95 ms:

| Read model | 4 steps | 100 steps | 1,000 steps |
| --- | --- | --- | --- |
| `postgres` (original) | 7,087 / 1.442 | 6,985 / 1.556 | 5,746 / 1.836 |
| `postgres` with the partial index | 6,302 / 1.894 | 6,000 / 2.319 | 6,190 / 1.964 |
| `postgres` after dropping it (control) | 7,806 / 1.250 | 6,749 / 1.590 | 5,333 / 2.112 |
| `dbsp-serve` | 6,715 / 1.647 | 7,030 / 1.441 | 7,040 / 1.477 |
| `dbsp-shadow` | 5,889 / 1.945 | 5,860 / 1.890 | 4,591 / 2.471 |

Capture-trigger write cost, single-row commits:

| Phase | p50 ms | p95 ms | WAL bytes per commit | Outbox growth |
| --- | --- | --- | --- | --- |
| Capture on (1) | 0.137 | 0.210 | 3,838 | 280 KiB |
| Capture off (1) | 0.124 | 0.229 | 3,646 | 0 |
| Capture on (2) | 0.134 | 0.254 | 4,317 | 352 KiB |
| Capture off (2) | 0.126 | 0.290 | 3,755 | 0 |

Per-run observations (three runs):

- Rebuild: 87,000 steps in 510 jobs ready in 0.18–0.22 s with 0.21–0.23 s of process CPU.
  Resident memory rose from 10–13 MiB to about 114 MiB during bootstrap and was 70–72 MiB later.
- Freshness, commit to a served read reflecting it (150 samples each): p50 21.8–24.8 ms, p95
  26.4–30.6 ms, p99 26.6–37.7 ms, dominated by the 20 ms poll.
- Burst: 2,000 single-row commits written and applied in 0.29–0.60 s.
- Hot job: one 1,000-step run receiving 5,400–7,900 commits while it was read. `dbsp-serve`
  served 0 of 400 reads (all fell back as lagging) at p50 1.76–1.80 ms, against 1.10–1.37 ms
  for `postgres`.
- View stopped: `dbsp-serve` answered through the bounded fallback at p50 0.53–0.89 ms, with no
  rejection at the bound of 8.
- Correctness under load: 0 shadow mismatches in 304,236 aligned comparisons, 366,589 served
  reads, 0 failures and one bootstrap per run.
- Storage: 162,000 outbox rows after loading took 27.5 MB including indexes (about 170 bytes per
  row), beside 56.5 MB of `durable_steps`. The partial index was 8 KiB because the fixture has
  no reused steps; it would grow with reuse.

### Against the budgets

| Budget | Result |
| --- | --- |
| B1 | **Met.** All correctness gates passed; no shadow mismatch |
| B2 | **Not met.** At 1,000 steps, `dbsp-serve` p95 (1.052 ms) is 8% below the best PostgreSQL p95 (1.139 ms), not 15%; its p50 is 30% lower. At 4 steps its p95 is 45% above the original PostgreSQL p95 (74% above the later control). Each request still spends roughly 0.45–0.5 ms on session and authorization queries that no read model removes |
| B3 | **Met.** Commit p50 +6–11%; p95 showed no increase beyond noise; WAL +5–15% per commit |
| B4 | **Met.** p95 at most 30.6 ms |
| B5 | **Met.** Ready within 0.22 s |

Where the experiment loses: small jobs, `dbsp-shadow` (both computations), and any job that is
being written while it is read, where every read pays the fence and then the full query. Where
it helps: large jobs, where it removes per-read aggregate work from PostgreSQL. Throughput at
concurrency 8 for 1,000 steps was 7,040 against 5,746 requests per second.

**DBSP decision for this consumer:** implemented as an experiment; production adoption
**deferred**. B2 is not met, the simpler partial index removes most of the statement cost, and
the read is dominated by authorization. ADR 0017 lists the revisit conditions. Adopting the
partial index on the default path is a separate change that needs its own write-cost check with
reused steps present.

Superseded runs, kept for transparency and not used above:

- `20261012T162705`: three clean runs before the explicit `VACUUM`. Original PostgreSQL read about
  1.43 ms p50 at 1,000 steps because the step counts still fetched heap pages; autovacuum set the
  visibility map partway through, which also flattered the partial-index phase. Rerun with
  `VACUUM` and the post-index control.
- `20261012T163354`: disturbed by a local build started during run 1; discarded.
- The first attempt failed in bootstrap on the `accumulate_output` defect described above.

## Gate results

Run on this branch with `LC_ALL=C TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.87.0` (the CI toolchain):

```sh
cargo xtask preflight --changed-only --skip repository --skip cmp-ui-preview
```

Impact was every gate (`Cargo.lock` is a shared input). Results as xtask printed them:

| Gate | Result | Detail |
| --- | --- | --- |
| repository | NOT_RUN (skipped) | Run separately below |
| rust-format | PASS | |
| script-ir-reference | PASS | |
| rust-lint (workspace) | PASS | clippy `-D warnings`, default features |
| rust-tests (workspace) | PASS | 64 targets: 669 passed, 0 failed, 113 ignored |
| dbsp | PASS | clippy with the feature; 42 targets: 371 passed, 0 failed, 125 ignored |
| wav-reference | PASS | |
| web-lint, web-build (Studio, Theatre), ui-preview | PASS | |
| cmp-ui-preview | NOT_RUN (skipped) | No Gradle on this machine (`cargo xtask doctor`: UNSUPPORTED) |
| mutation | PASS | 9 killed, 0 survived |
| postgres | PASS | New disposable cluster: revisions 17, imports 9, adaptations 21, production 11, durable 50 and DBSP view 11 tests passed, plus the HTTP/restart journeys |

After adding the real-binary CLI test, the affected gates ran again on the final tree:
`cargo fmt --all -- --check` passed, `cargo xtask verify dbsp` **PASS** (42 targets: 371 passed,
0 failed, 126 ignored) and `cargo xtask verify postgres` **PASS** (durable 50 and DBSP view 12
tests, with every other suite and journey as above).

The repository gate's steps, run separately: `scripts/check_repository.py` passed;
`git diff --check` and `git diff --cached --check` were clean; `python3 -m unittest discover -s
scripts -p 'test_*.py'` **failed** with 4 failures and 1 error (`test_ui_preview_catalog`,
`test_mutate_script_ir`) that are identical on a clean `develop` worktree on this machine's
Python 3.9.6, which lacks `tomllib`. The new `test_benchmark_job_summary_dbsp` passes. So the
repository gate as a whole is `FAIL` here for a pre-existing environment reason, not this change.
`cargo test -p xtask` (88 tests) covers the new gate, CI job, preflight selection and build flag.
GitHub Actions results were not awaited, per the local handoff policy.

`develop` then advanced to `db14339` (#59, Studio web-only), which touched xtask and documents but
nothing under `services/` or the scripts the database gates run. After merging it (one import
conflict in `xtask/src/build.rs`), `cargo xtask verify rust` **PASS** (669 passed, 0 failed,
113 ignored), `scripts/check_repository.py` passed, and the Python script tests showed the same
five pre-existing failures out of 216. One `cargo test -p xtask` run failed
`probe::tests::port_ownership_reflects_an_existing_listener` (an ephemeral-port race in a test
this change does not touch); three reruns passed 88 of 88. The `dbsp` and `postgres` gates were
not rerun after the merge.

## Completion report

- **Invariant:** a job summary served from the DBSP view equals the PostgreSQL summary at the
  same committed position, for an authorized request only; otherwise the authoritative query
  answers or the request fails closed.
- **Owner / boundary:** `incremental` (engine-independent core) and `incremental::dbsp`
  (circuit); `postgres::durable::summary_view` (feed, maintainer, serving); migration 0009 (feed);
  `production-jobs` (opt-in consumer). PostgreSQL remains the write and authorization authority.
- **Evidence level:** `integration-tested` and `differentially-tested` against real PostgreSQL
  17.11 with the real DBSP runtime; `fault-injected` (dropped, duplicated and failed pages,
  invalidation, stop, restart while writing); `property-tested` (core and circuit);
  `mutation-tested` by three hand mutants; `example-tested` for the full suite with the feature;
  bounded local benchmark observations. Not `production-observed`.
- **Evidence:** the tests, gates and benchmark above → pass, except the pre-existing Python
  unit-test failures and the budget B2 miss, both stated.
- **Command:** `cargo xtask preflight --changed-only --skip repository --skip cmp-ui-preview`;
  `cargo xtask verify dbsp`; `cargo xtask verify postgres`;
  `python3 scripts/benchmark_job_summary_dbsp.py --runs 3`.
- **Provenance:** base `5d26d9b`; synthetic fixtures only; real PostgreSQL and real DBSP, no mocks
  in the differential tests; raw benchmark record outside Git with its checksum above.
- **Edge classes covered:** see the differential and recovery checklist; pagination and time or
  collation semantics are inapplicable to this point read.
- **Not covered / residual risk:** no production workload, capacity or monitoring; the outbox is
  never pruned and grows with every projected change, in every build, because the migration is
  unconditional; each process holds its own view (memory about 1.2 KiB per step during bootstrap
  here) and its own polling session (about 50 small transactions per second at the 20 ms poll); a
  long-running transaction delays freshness and turns serving into fallbacks; a network-level
  connection drop was not injected; the `Ahead` freshness outcome is only unit-tested; a
  privileged role can still forge the feed (shadow detects it, `dbsp-serve` would not); with the
  feature on, `serde_json/arbitrary_precision` changes number handling package-wide and only the
  existing suite guards it; the benchmark is one laptop, synthetic, no reused steps, with wide
  latency tails; CMP preview and the pre-existing Python failures are unverified here; and newer
  `dbsp` releases need a toolchain upgrade.
