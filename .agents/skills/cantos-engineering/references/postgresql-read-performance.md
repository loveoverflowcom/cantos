# PostgreSQL read performance and DBSP evaluation

> **Scope.** Mandatory working method for adding or changing a database-backed GET, read or
> fetch path, its queries, or writes feeding its read model. Prioritize frequent or expensive
> reads across Cantos Studio and Cantos Theatre, including internal fetches behind other HTTP
> verbs. Inventory and assess every affected read; spend optimization effort on measured hot
> paths. Use DBSP incremental views where the comparison earns them. Do not route every GET
> through an engine.

## Reality, terminology and authority

The backend, its PostgreSQL schema and one **implemented experimental** DBSP adapter now exist
([ADR 0017](../../../../docs/decisions/0017-experimental-dbsp-job-summary.md),
[evidence](../../../../docs/evidence/dbsp-job-summary.md)). The `dbsp` crate (0.198.0, selected against the original
Rust 1.87 baseline) is an optional dependency of `cantos-api` behind the `dbsp-experimental` feature; default
builds do not compile it; its former consumer was the opt-in development `JobSummary` read. It is
not a production adoption: there is no Feldera service, connector, checkpoint, outbox retention or
production configuration. Reinspect the actual checkout before every implementation; these
observations do not describe all future branches.

That slice's only consumer, the durable job summary, was removed with the in-process TTS runtime
([ADR 0018](../../../../docs/decisions/0018-external-audio-production.md)). What remains is
reusable evidence for another engine path, not a live read model:

- `services/cantos-api/src/platform/incremental` — engine-independent signed changes, the count view, whole
  transaction paging and the pure fence/shadow decisions; `incremental::dbsp` — the circuit.
- Migration `0009_durable_step_changes.sql` — a trigger-written, read-only transactional outbox
  stamped with `xid8`; positions are `{frontier, bootstrap snapshot}`, never a sequence number.
  It stays applied but unused.
- `cargo xtask verify dbsp`. The `summary_view` maintainer, its PostgreSQL tests and
  `scripts/benchmark_job_summary_dbsp.py` are gone; recover them from commit `2de8c69`.

A new consumer still needs its own fit decision below; the existing slice is evidence about one
small aggregate, not a general license to route reads through the engine.

DBSP is a model of incremental computation over streams, with incremental view maintenance
(IVM) as a database application. Its [original paper](https://docs.feldera.com/vldb23.pdf)
defines maintaining query results from changes to the input. Feldera is a concrete
[incremental computation engine](https://docs.feldera.com/) built on that foundation
([publications](https://docs.feldera.com/literature/papers/)); it uses tables/views and pipelines.
For Cantos, evaluate this as a candidate incremental read-model adapter, not a drop-in
PostgreSQL query planner, index or cache. A paper's result or a vendor benchmark is no evidence
that a Cantos query improved.

PostgreSQL remains the authoritative oracle and source of metadata/durable state under
[the architecture](../../../../docs/architecture/overview.md#data-and-delivery) and
[ADR 0001](../../../../docs/decisions/0001-modular-monolith.md). Any different authority needs
a reviewed architecture decision. Keep domain types independent of the engine; an adapter is
justified only by a concrete consumer. This skill authorizes evaluation, not production
installation, new services, production CDC configuration or migrations.

When an engine is present, inspect its actual implementation, ADR, lockfiles, versions, edition,
connectors, query-serving interface and tests. Reuse those boundaries. Otherwise label the
candidate **proposed / not integrated**; name missing prerequisites instead of inventing a
dependency version, configuration key, command or API.

## Required read workflow

1. **Trace and inventory.** Discover handlers, application fetches, repository/query code,
   migrations, indexes, contracts and tests in the actual repository. Use its real runners.
   Map each affected read to its consumers, query, tables, permission/tenant filters, parameters,
   cardinality, pagination/order, read frequency, write dependencies and consistency contract.
   Find repeated joins/aggregations and N+1 queries; rank by observed cost and user impact.
   Record unknown frequency or missing implementation as unknown, never as measured.
2. **Set workload targets before changing the path.** Choose target p50/p95/p99 end-to-end
   latency, throughput at stated concurrency, CPU/RAM, freshness/lag and rebuild time/resource
   cost. Tie thresholds and regression budgets to this workload; distinguish targets from
   measured results. Include write latency/throughput and maintenance budgets when relevant.
   Do not invent universal thresholds or claim an unmeasured speedup.
3. **Establish the PostgreSQL baseline.** Use synthetic or explicitly permitted non-sensitive
   fixtures at representative scale and skew, with the application role, tenant context,
   authorization filters, settings, parameters and transaction isolation the path really uses.
   Capture ordinary request/query timings and `EXPLAIN (ANALYZE, BUFFERS)` plans on an isolated,
   disposable development/test database under a resource/time budget. Record estimates versus
   actual rows, scans, join/sort work, buffer hits/reads and spills. Match PostgreSQL docs to the
   tested server version. [EXPLAIN](https://www.postgresql.org/docs/current/sql-explain.html)
   executes the statement with `ANALYZE` and adds profiling overhead; its timing is not API
   latency. Inspect SELECTs for side effects too. Do not download production/sensitive data,
   run heavy benchmarks on production, or run write `EXPLAIN ANALYZE` on production; a rollback
   does not waive this restriction.
4. **Compare simpler PostgreSQL improvements first.** Evaluate query shape, indexes matching
   filters/joins/order, statistics, unnecessary columns, N+1 removal and pagination. Measure
   index write/storage cost. Consider caching only with defined scope, invalidation and access
   semantics. Preserve results and rights filters. Re-run the baseline comparison before
   attributing gains to another engine.
5. **Make an explicit DBSP fit decision.** For hot repeated joins/aggregations over changing
   data, evaluate an incremental view against the improved PostgreSQL baseline. Describe the
   maintained query, reuse across consumers, update rate/fan-out, state size, freshness budget,
   serving path and operating/rebuild cost. Small datasets or indexed point lookups can stay on
   PostgreSQL when maintenance/operational cost exceeds benefit. Record `adopt | reject | defer`
   with evidence; deferral states prerequisites. A suitable candidate needs a bounded experiment
   using the actual engine and connectors before adoption. Absent runtime/schema, record that
   experiment as `not-run` or `blocked`, not as successful integration.
6. **Define and test the serving boundary.** Specify the PostgreSQL commit/change position
   represented by a view, how the API determines freshness, read-after-write/transaction needs,
   and atomic visibility of results. Decide when to wait, fall back to the equivalent authorized
   PostgreSQL query, or return a typed unavailable outcome. Unknown lag, engine failure, rebuild
   or an unsupported query must not silently serve data outside the contract. Bound fallback
   load with timeouts/concurrency limits; exercise it under an outage. Run the checks and
   benchmarks below before enabling an incremental serving path.

## GET still depends on writes

A read-only request does not make its inputs immutable. Inventory insert/update/delete,
publication-pointer changes, retractions, permission/entitlement changes and relevant time
changes feeding every derived result. Define how each causes maintenance/invalidation, including
cached responses. Incremental results never grant rights: access is still enforced in the
backend at request time. A stale view must not expose a private draft, withdrawn release or
revoked entitlement. Cache/view keys and partitions include the effective tenant, actor/access
scope, query parameters and revisions where applicable; prove that one scope cannot reuse
another's result.

Evaluate mutations case by case. Preserve the authoritative PostgreSQL transaction,
idempotency, optimistic concurrency and response semantics from
[persistence](persistence.md) and [HTTP boundaries](http-api-boundary.md). Do not reroute a
write or weaken its consistency merely to include DBSP. Measure write amplification,
CDC/outbox/maintenance overhead, commit latency, throughput, WAL/backlog/storage, engine
CPU/RAM and recovery impact at the expected read/write mix. An asynchronous read model may
fit discovery while being unsuitable for an approval, rights or conditional-progress decision.
Record which writes invalidate it even when no mutation uses the engine directly.

Verify delivery/recovery properties for the actual input and output adapters, edition and
configuration: [Feldera connectors](https://docs.feldera.com/connectors/) and
[fault tolerance](https://docs.feldera.com/pipelines/fault-tolerance-overview/) describe separate
requirements and modes. Do not infer end-to-end exactly-once delivery, PostgreSQL transaction
equivalence or checkpoint availability from the DBSP theory or the engine name.

## Differential and recovery checks

Use PostgreSQL's actual authoritative query as an independent oracle over the same fixtures,
actor/tenant context and committed input position. Align a snapshot plus change-log watermark
or another implemented checkpoint/barrier; do not compare two arbitrary live "now" snapshots.
Check both the maintained result and the actual API response, including denial/error bodies,
ordering and pagination. A mock or a rewritten copy of the incremental query cannot prove
PostgreSQL equivalence. Seeded change sequences are suitable for differential/state-machine
tests; keep minimized failures as regressions.

| Edge class | Required comparison or failure injection |
| --- | --- |
| Insert/update/delete and retractions | Insert join matches and non-matches; change join/group keys; remove the last group member; retract a release. Check counts, weights and removed rows, not only additions. |
| Duplicates and delivery ordering | Replay a batch/event after acknowledgement loss; retry application writes with the same idempotency key; exercise the adapter's allowed reordering and late events. Distinguish legitimate SQL bag duplicates from duplicate delivery; preserve committed order where semantics require it. |
| SQL and time semantics | NULLs, empty aggregates, outer joins, numeric precision/overflow and an explicitly justified tolerance if needed; timestamp precision/timezone, event versus processing time, window edges and expiry with no new writes. Check collation/order with Vietnamese titles such as “Ánh đèn cuối sân khấu”. |
| Pagination/sort | Equal sort keys with a stable unique tie-breaker; empty/last pages; concurrent inserts, updates, retractions and invalid cursors. Reapply authorization on every page; preserve the owning contract's duplicate/skip law. |
| Transaction boundaries | Multi-table changes within one commit, concurrent commits and rollback. No partial committed result; read-your-write and read-after-write behavior match the stated API contract. |
| Reconnect/restart | Disconnect input/output, crash before/after offset or result acknowledgement, replay after restart, checkpoint unavailable. Check no lost change or double-applied result; record gaps and supported recovery modes. |
| Snapshot/rebuild | Bootstrap while writes continue; hand off snapshot to CDC without a gap/double count; rebuild after state loss or query/schema revision. Compare at a common watermark before atomic cutover; keep the approved fallback usable. |
| Tenant isolation and authorization | Same local keys in two tenants, denied/anonymous/entitled actors, role revocation, private drafts and withdrawn releases. Assert no cross-scope rows, counts, existence leaks, shared-cache leaks or bypass through fallback. |
| Consistency/freshness and failure | Lag below/above the contract, unknown watermark, stopped engine, backpressure, rebuild and unsupported semantics. Assert the exact fresh/wait/fallback/unavailable outcome and current authoritative access decision. |

Each relevant row maps to a real test path and command or a stated gap. Mark a row inapplicable
only with a workload/adapter reason. An adopted DBSP path needs real PostgreSQL and engine
boundary evidence; pure-model or fake-connector tests remain evidence against doubles.

## Benchmark and evidence record

Compare original PostgreSQL, improved PostgreSQL and a DBSP candidate if run, with equivalent
responses, access rules and freshness requirements. Include the API-to-result serving route,
transport and output materialization cost; engine compute time alone is insufficient.

- Use cold and warm runs with documented cache/state preparation, including initial snapshot
  and steady incremental maintenance. Do not call a run cold merely because the client restarted.
- Cover representative cardinality, skew, selective/broad parameters, tenant sizes, update
  bursts, read/write mixes and concurrency. Include low-read/high-write or small-data cases
  that can make the incremental choice lose.
- Report p50/p95/p99 and throughput with sample count, duration, warm-up, repetitions and
  variance, alongside CPU/RAM for all components, freshness/lag and rebuild duration/resources.
  Measure write amplification/CDC overhead, write latency and fallback capacity; do not hide
  regressions behind a mean or drop inconvenient runs.
- Retain source SHA/dirty state, actual PostgreSQL and engine build/version/edition, connector
  versions, schema/query/index hashes, hardware, seed/generator, non-secret effective config,
  data scale/distribution, workload commands and raw-result references. Store private or large
  artifacts outside Git; commit only safe evidence and reproducible fixture generators.

Extend the foundation's completion report with this record; keep its evidence vocabulary and
the [execution result values](local-execution.md#reporting-execution):

```text
Read inventory:          actual handler/query/migration/contract/test paths; hot paths and gaps
Authority / checkpoint: PostgreSQL oracle query; snapshot/change position; actor/tenant scope
Targets / observations: chosen budgets; original PG → improved PG → candidate; samples/results
DBSP decision:          adopt | reject | defer; implementation/ADR/edition or proposed prerequisites
Write dependencies:     invalidation/retractions; measured write/CDC/maintenance cost
Semantics / failures:   test paths, seeds, edge rows and commands actually run
Freshness / fallback:   contract; measured lag; tested outage/rebuild/cutover/access outcomes
Provenance / artifacts: versions, source/query/config/data scale, commands and safe result links
Execution gaps:         passed | failed | blocked | not-run | unavailable | zero-selected + reason
```

## Review and acceptance

- [ ] The affected GET/read/fetch inventory points to actual code, migrations, contracts and
  tests, or clearly records their absence. Hot joins/aggregations are prioritized with evidence.
- [ ] Workload targets, safe PostgreSQL baseline/plans and the simpler index/query comparison
  are recorded with actor/tenant filters and representative data; numeric gains come from runs.
- [ ] DBSP adoption/rejection/deferral is justified. An adoption names a concrete consumer,
  reviewed decision, actual engine/adapter and end-to-end result; missing prerequisites stay
  explicit. No engine integration is claimed from instructions or a paper.
- [ ] Oracle comparison uses the same committed input position and checks actual response
  semantics. Relevant rows in the differential/recovery table have real evidence or a gap.
- [ ] All feeding writes/retractions and access changes are accounted for. Mutations retain
  their transaction/response semantics and have measured overhead when the candidate affects them.
- [ ] Freshness, current authorization, failure fallback, rebuild and atomic cutover are
  specified and exercised for an adopted path. Security/consistency gaps block enabling it.
- [ ] Cold/warm and cardinality/skew/read-write benchmarks retain versions, seeds/config,
  raw observations, targets and regression budgets. No unrun benchmark is reported as passing.
- [ ] The report distinguishes documented method, implemented adapter and tested behavior;
  remaining work is routed to its owning slice in the [queue](../../../../docs/work-plan/README.md).

**Enforcement:** inventory and fit assessment are manual requirements. The experimental job
summary has real runners (`verify dbsp`, the PostgreSQL gate's DBSP tests and the benchmark
script); other engine automation is proposed until code and runners exist. The repository
checker enforces skill/link hygiene only. A documented exception needs workload rationale and an oracle;
unavailable prerequisites are a gap, not a waiver of adoption evidence.

## Cantos examples (proposed, not endpoints implemented here)

**Good candidate:** Theatre catalog language filters plus episode counts repeatedly join
publication metadata at substantial scale. Compare improved PostgreSQL with a bounded DBSP
view experiment. Pin the same commit position; publish a replacement and retract a release;
assert identical visible IDs/counts, stable pages and cross-tenant denials. A lagging view may
select candidates, but the backend still applies current authoritative release/access checks;
unhealthy maintenance uses the tested fallback. Adopt only within measured freshness, recovery
and write-overhead budgets.

**Exception:** Fetching one immutable publication manifest by ID, with indexed current-access
checks, may have no repeated aggregate to maintain. Record the fit rejection and keep the
PostgreSQL path plus already-authorized caching; immutable bytes do not imply permanent access.
Likewise a conditional progress update keeps its PostgreSQL concurrency/idempotency decision;
evaluate only a separate derived read if the workload earns it.

**Counterexample:** “Every GET uses DBSP, so it is optimized” after adding a dependency, with
no baseline, delete/retraction tests, permission filters or lag-aware fallback. Reject the
claim: the engine name proves neither faster reads nor safe results.
