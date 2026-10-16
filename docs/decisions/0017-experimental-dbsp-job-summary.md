# ADR 0017: Experimental DBSP job-summary read model

> **Historical / superseded for active backend (2026-10-12).** [ADR 0018](0018-external-audio-production.md) removes server-side TTS generation/worker, provider dispatch and synthesis-job status. This document remains for audit and prior experiments only; do not implement it as current Cantos API behavior.


- Status: historical; superseded for active backend by [ADR 0018](0018-external-audio-production.md).
- Original status: accepted for an opt-in local/development experiment; production adoption is **not**
  admitted and remains gated by the revisit conditions below
- Date: 2026-10-12
- Decision owner: Cantos maintainers
- Related issue or PR: [#58](https://github.com/loveoverflowcom/cantos/issues/58)
- Supersedes / superseded by: N/A. Narrows, for the durable job summary only, the DEFER in the
  [durable jobs read assessment](../evidence/durable-tts-jobs.md#affected-read-inventory-and-dbsp-decision).
  The [script-revision DBSP decision](../evidence/script-revision-dbsp-decision.md) still applies to
  revision reads.

## Context

Known: `JobSummary` (`run_id`, `job_id`, `status`, `reason`, `total_steps`, `completed_steps`,
`reused_steps`) is the only aggregate read of the durable foundation. `Store::job_status` computes it
with three correlated `COUNT(*)` subqueries over `durable_steps` after authenticating the session and
authorizing the script owner; its consumer is the development `production-jobs` CLI. A run has 1 to
1,000 steps. Inserts, `succeeded` transitions and reuse selections change the counts; deletes are
rejected by trigger. PostgreSQL is the authority for every fact, and current authorization is
re-evaluated on every read ([ADR 0001](0001-modular-monolith.md),
[persistence](../../.agents/skills/cantos-engineering/references/persistence.md)).

Known: issue #58 asks for a real, runnable Rust DBSP incremental path for this consumer, with a
committed-data feed, an opt-in serving or shadow consumer, real-PostgreSQL differential tests and an
honest measurement, rather than another deferral. The workspace pins Rust 1.87; `dbsp` 0.198.0 is the
newest release declaring `rust-version = 1.87.0` (0.277 and later need 1.91 to 1.96).

Assumed: no measured production workload exists, so this slice can establish correctness and cost but
not production benefit.

## Decision

1. **Engine.** Pin `dbsp = "=0.198.0"` as an optional dependency of `cantos-api`, enabled only by the
   `dbsp-experimental` Cargo feature. One single-worker, in-memory (`Mode::Ephemeral`) runtime per
   process; no Feldera pipeline, service, connector, storage backend or checkpoint. Default builds,
   gates and binaries do not compile it.
2. **Engine-independent core** (`incremental`, always compiled): signed member changes, per-group
   counts, a view that applies engine output deltas as one unit (retractions matched before
   insertions), whole-transaction feed paging, and the freshness and shadow decisions. The DBSP circuit
   (`incremental::dbsp`) uses a linear aggregate per job, so a step applies only its deltas, plus two
   integrity operators (per member row, per member id) that refuse a change applied twice, a retraction
   of a row never inserted or two rows for one member.
3. **Committed feed.** Migration 0009 adds `durable_step_changes`: a trigger on `durable_steps` writes
   `-old/+new` rows for changes of the projected facts (job, succeeded, reused) in the writing
   transaction, stamped with its top-level `xid8`. The trigger function is `SECURITY DEFINER`; the
   application role may only read. Captured rows are immutable to updates.
4. **Committed position.** A view at `{frontier F, bootstrap S0}` reflects every committed row visible
   in its REPEATABLE READ bootstrap snapshot `S0` or belonging to a transaction below `F`. Each poll
   reads whole transactions in `[F, xmin)` of its own snapshot that `S0` did not contain, in `(xid8,
   seq)` order, applies them as one engine step and publishes the counts and the new frontier together.
   No running transaction can still add a row below `xmin`, so out-of-order commits are never skipped;
   a failure before publication re-reads the same window, so no row is applied twice.
5. **Serving.** `job_status` authenticates and authorizes exactly as before. Only then does an attached
   view answer for that job, and only if a fence evaluated in the same statement as the job row proves
   `F <= xmin` of the request's snapshot and no visible change of the job lies at or after `F` outside
   `S0`. `dbsp-serve` otherwise runs the original query under a concurrency bound and fails closed as
   `Unavailable` when the bound is exhausted; `dbsp-shadow` always answers from PostgreSQL in the same
   snapshot and compares. A mismatch invalidates the view. `CANTOS_JOB_SUMMARY_READ_MODEL` selects the
   model for the development CLI (`postgres` default); a build without the feature refuses the DBSP
   values.
6. **Failure and restart.** Engine, feed, integrity, position or view errors publish `failed`; requests
   fall back; the maintainer rebuilds from a new snapshot. Every process start rebuilds.
7. **Verification surface.** `cargo xtask verify dbsp` (also selected by preflight for API changes and run
   in the CI `rust` job) lints and runs every `cantos-api` test with the feature. `cargo xtask verify
   postgres` runs the view's differential, failure and authorization tests. A separate bounded
   benchmark script records measurements.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Logical replication (`pgoutput`, `wal2json`) | No trigger or outbox writes | `wal_level=logical`, a replication slot and retention to operate; `tokio-postgres` 0.7 has no replication protocol | Rejected for this slice; revisit for production CDC |
| Feldera pipeline with a PostgreSQL connector | Managed SQL views, checkpoints | A new deployed service, credentials and connector semantics to verify | Out of scope (#58); no service before a measured need |
| Outbox tailed by sequence number | Simple | Sequences are allocated before commit: a later commit with a lower number is skipped | Rejected: silent missed update |
| Replay the whole outbox on start instead of a snapshot | No snapshot handoff | Replay grows without bound; pruning becomes impossible | Rejected in favour of snapshot plus `pg_visible_in_snapshot` handoff |
| Serve when lag is below a time budget | Fewer fallbacks | Elapsed time does not prove which commits are applied; can serve stale counts | Rejected: the per-request fence is exact |
| LISTEN/NOTIFY as the feed | Low latency | Not durable; lost on disconnect | Rejected as a feed; could be a wake-up hint later |
| Non-optional dependency | One build | Every build compiles about 228 more crates, and `dbsp` forces `serde_json/arbitrary_precision` workspace-wide | Rejected: feature-gated instead |
| Single-pass `count(*) FILTER` query or a partial index | No engine | Still a per-read aggregate | Measured as the improved PostgreSQL comparator; not adopted here (see evidence) |

## Consequences

- Product and user impact: none by default. With the opt-in, `production-jobs status|watch` returns the
  same summary; the counts may come from the view. `watch` is a new development command.
- Compatibility and migrations: migration 0009 is additive and installed for every build, so every
  projected step change writes one or two outbox rows in all deployments. Existing results, errors,
  authorization and idempotency are unchanged. With the feature, `serde_json/arbitrary_precision` and
  `rkyv`'s trait implementations apply to the whole package; five comparisons or collects in existing
  code and tests needed explicit types to compile, and the complete `cantos-api` suite passes with
  the feature (see evidence).
- Operations, storage and cost: the outbox is never pruned (an operator-owned task that is not
  implemented); the view's memory grows with the number of steps; each process holds its own view and
  database session; a long-running transaction holds the frontier back, which only causes fallbacks.
- Implementation and recovery requirements: the core has no I/O; the shell is
  `postgres::durable::summary_view`; DBSP never becomes a write authority and never sees authorization.
- Deferred questions: outbox retention and a consumer registry; checkpointed state instead of rebuild;
  an HTTP route; production configuration, monitoring and capacity; logical-replication CDC; a toolchain
  upgrade to newer `dbsp` releases.

## Validation and revisit conditions

The [evidence report](../evidence/dbsp-job-summary.md) records the differential, failure and
authorization tests, hand-run mutants, the full-suite run with the feature and the bounded benchmark
with its limits. Production adoption requires, at least: a measured workload whose read cost misses a
stated budget on the improved PostgreSQL path; outbox retention; monitored lag, rebuild and fallback
budgets; a reviewed decision on `arbitrary_precision` for the shipped binary; and an operating plan for
per-process state. Remove the experiment if it stops compiling within the pinned toolchain policy and
no consumer needs it.
