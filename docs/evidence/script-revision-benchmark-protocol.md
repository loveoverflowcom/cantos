# Prompt-3 bounded read benchmark protocol

> **Historical PostgreSQL protocol.** The direct benchmark runner is now a refusal-only stub;
> the commands below describe the source-bound campaign, not a supported native workflow.
> Its former implementation is preserved at
> [the pre-retirement source](https://github.com/loveoverflowcom/cantos/blob/84eea4c3843908f4f21da9760381c129e21d5263/scripts/benchmark_revision_reads.py).
> Results/hashes remain unchanged. Native file/HTTP regressions are mapped in the
> [oracle ledger](../architecture/server-turso/legacy-test-oracles-content.md); comparable
> production query measurements remain #384 work.


Protocol fixed before measurement, on `626416500599b852dc985683fa3a4ac905f9401f` plus
the new benchmark harness. This is a local sizing experiment for the **implemented**
development API, not a production traffic forecast or SLO. The
[read inventory](script-revision-reads.md) owns the actual consumers and consistency contract.

## Invariant and oracle

GET head/history must return the complete accepted export, identity, actor, UTC acceptance time
and digests after current authorization. An index must preserve that result and conceal denied
and missing scripts with the same error. PostgreSQL is the sole authority; no async view,
response cache, CDC or engine is enabled. No pure domain rule changes, so core extraction and
schema/digest changes are inapplicable.

`scripts/benchmark_revision_reads.py` creates a new disposable loopback PostgreSQL cluster,
runs the actual migration binary, seeds synthetic validated exports, and launches the real
Axum binary using the restricted `cantos_app` role. It ignores inherited DB/service/credential
configuration. Admission uses the existing validator CLI; bulk seeding is operator fixture
setup, not an API throughput result. Oracle checks independently project complete rows from
PostgreSQL at quiescent committed positions and compare exact HTTP responses for owner,
reader and editor; outsider/foreign-owner/missing-history/anonymous denials, membership
revocation and reader-write rejection are asserted separately. Existing PG tests own stale
write/replay/crash/restart coverage. No invented pagination workload is benchmarked.

## Workload and candidate

- Fixed seed `20261010`; original Vietnamese two-scene fixture; 4 or 512 dialogues.
- Two separately created clusters: 100 and 5,000 scripts, 32 owners, 2 members/script.
  Half the scripts belong to owner0; the rest spread over 31 owners. Script0 has 2,000
  revisions, other scripts 5; 10% carry the larger export. Distinct operation/revision identities
  remain distinct even when c1/e1 match. Evidence links retain their owners.
- Default 2,400 measured requests per warm scenario and 32 unmeasured operations/worker;
  three repetitions. Warm owner reads C=1, member/head C=4, pinned history C=4, large payload C=4,
  90/10 and 20/80 read/write mixes C=4, denied C=4. Closed-loop clients, persistent HTTP/1.1,
  full response receipt + JSON decode; exclude oracle assertions from latency, but include
  transport and materialization. Throughput uses the entire wall interval **including warmup**;
  it is conservative completed measured requests/s. It is not an arrival-rate capacity test.
- Cold-start member batches: stop host, fast-restart PG, start host with TCP-only readiness;
  no app query warmup, 60 requests C=1 per repetition. PG shared buffers reset, OS page cache
  is uncontrolled. The batch warms common pages; retain first-request latency and all samples.
  This is **not disk-cold**, and cold p99 from 60 samples is the maximum, not a stable tail SLO.
- Simpler candidate: redundant membership B-tree `(script_id,actor_id) INCLUDE(role)`;
  compare baseline → candidate → dropped-index baseline to expose time/order drift.
  Each phase uses its own database cloned from the same committed seed template; prior phase
  writes cannot grow the next phase's history. Repetitions within a phase intentionally retain
  the earlier repetitions' committed workload, with the same sequence in every phase.
  Capture plans after vacuum and after role updates because visibility/HOT maintenance matters.
  Existing primary keys already match every point lookup. No query rewrite or auth consolidation
  is adopted without evidence and equivalent authorization timing.
- Measure committed API saves (5 evidence reads + 5 link inserts per acceptance), WAL deltas,
  table/index storage and 12 operator membership-update transactions affecting all scripts.
  Operator timings include fresh psql process/connection; WAL includes unrelated local
  background work. These costs are bounded observations, not pure engine CPU time.

## Targets and resource budget (chosen, not measured)

Investigative budgets: warm small owner/member/history p95 ≤ 50 ms, p99 ≤ 100 ms; 512-dialogue
p95 ≤ 150 ms, p99 ≤ 250 ms; mixed-save p95 ≤ 200 ms. C=4 warm small reads should exceed
100 completed measured requests/s. Target aggregate sampled Axum+PG CPU below four core
equivalents and summed RSS below 1 GiB (RSS double-counts shared PG pages). Bound the cluster
to 128 MB shared buffers, 4 MB work_mem, 32 connections; keep fsync, synchronous_commit and
full_page_writes enabled. No production freshness/CDC/rebuild budget exists for absent views.

Adopt an extra index only if member-read p95 improves ≥15% consistently across repetitions
and baseline-return comparison without weakening permissions, save p95 regression >10%, or
unjustified storage/permission-write amplification. Otherwise keep the existing path.
Stop if disk free falls below 5 GiB or a task-owned server stalls; keep generated data below
1 GiB per cluster. One benchmark/build lane at a time; do not kill unrelated processes.
Resource sampling uses owned process trees only, at approximately 100 ms; CPU deltas miss
short-lived process lifetimes and sub-sample work, and client peak RSS is process-lifetime.
No background machine isolation or thermal control is claimed.

## Reproduction and records

```sh
PATH=/opt/homebrew/opt/postgresql@17/bin:$PATH python3 scripts/benchmark_revision_reads.py --scripts 100
PATH=/opt/homebrew/opt/postgresql@17/bin:$PATH python3 scripts/benchmark_revision_reads.py --scripts 5000
```

Each run records source/dirty paths, harness/SQL/schema hashes, versions, hardware, non-secret
effective config, indexes, fixture cardinality/bytes, three-phase raw request/resource samples,
JSON `EXPLAIN (ANALYZE, BUFFERS)` plans and summary in ignored `target/read-benchmarks/run-*`.
No existing database URL is accepted. The finally block stops owned HTTP/PG processes; data
remains local for diagnosis. A failed run is not performance evidence for a passing claim.
The benchmark does not assert a timing gate on a shared developer machine.
Use normal Python assertions; `-O`/`PYTHONOPTIMIZE` mode fails before starting a cluster.

PostgreSQL17 [EXPLAIN documentation](https://www.postgresql.org/docs/17/using-explain.html)
distinguishes profiled plan time from output/transport costs. Its
[covering-index documentation](https://www.postgresql.org/docs/17/indexes-index-only-scans.html)
explains visibility-map-dependent heap avoidance and included-column storage costs.
These sources informed the experiment; only local results can justify a Cantos speedup.

**Residual risk:** actual traffic/cardinality/mix remain unknown; debug build and Python client
can dominate observed latency. Shared-buffer cold-start batches do not establish disk-cold
performance. No engine, CDC, recovery/fallback or production deployment is exercised.
