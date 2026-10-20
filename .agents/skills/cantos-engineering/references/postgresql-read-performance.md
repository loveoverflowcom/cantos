# Database read performance and DBSP evaluation

<a id="postgresql-read-performance-and-dbsp-evaluation"></a>

> **Scope.** Mandatory method for database-backed GET/read/fetch work and mutations feeding
> read models. This existing reference keeps its filename/anchors for callers; it now covers
> the [owner-selected native Turso target](../../../../docs/architecture/native-turso-refactor.md)
> as well as the implemented PostgreSQL migration baseline. Assess each affected read and
> prioritize measured hot paths. Do not route every GET through an incremental engine.

## Reality, terminology and authority

At reviewed baseline `cccb547c08585e1266e26b2b07ead3a2c887697e`, PostgreSQL is authoritative
and `dbsp` 0.198.0 is optional behind `dbsp-experimental` in `cantos-api`. Its original TTS job
summary consumer was retired under [ADR 0018](../../../../docs/decisions/0018-external-audio-production.md).
The remaining grouped-count sample and `cargo xtask verify dbsp` are not a live production
read model or a Turso connector. Reinspect the checkout, lockfile, feature graph and current
[platform review](../../../../docs/evidence/dbsp-platform-review.md) before proposing reuse.

The sample at `services/cantos-api/src/platform/incremental` contains signed changes/counts
and pure freshness decisions, but its committed-position explanation uses PostgreSQL
`xid8`, `xmin` and snapshot coverage. These assumptions do not become valid for Turso by
renaming a field or using an AUTOINCREMENT row ID. The prior feed/migrations/benchmark were
retired; preserve their source-bound evidence, never replay them as a new consumer's adapter.

For epic #331, the selected durable authority is **local in-process Turso** in each owning
service. The actual Turso SQL query is the new path's independent result oracle. PostgreSQL
may supply a controlled legacy equivalence/performance baseline, not a required daemon in
new default workflows. Other work follows its actual accepted/implemented database owner.
The target is selected, but integration, engine capabilities and performance remain to be
verified by their issues; this method does not report adoption.

DBSP maintains derived results from changes to inputs. The Rust library has input handles
for streams/weighted collections; this makes an in-process change adapter a possible design,
not evidence that a ready-made Turso connector or safe committed feed exists. Feldera's service,
connectors and fault-tolerance products are separate choices, not implicit dependencies of the
Rust crate. Do not add a broker/Feldera process to satisfy this native refactor.

Use primary documentation matched to the tested versions:
[Turso compatibility](https://docs.turso.tech/sql-reference/compatibility),
[transactions](https://docs.turso.tech/sql-reference/statements/transactions),
[DBSP inputs](https://docs.rs/dbsp/latest/dbsp/operator/input/index.html),
[Feldera documentation](https://docs.feldera.com/).
Latest documentation is not permission to upgrade the pinned crate/toolchain.

## Required read workflow

1. **Trace and inventory.** Map actual handler, use case, query, tables/indexes, migration,
   filters, actor/tenant scope, parameters, cardinality, ordering/pagination, read frequency,
   write dependencies and consistency. Find repeated joins, aggregates and N+1 work. Mark
   unknown traffic as unknown, not measured. Preserve context-owned persistence boundaries.
2. **Set workload budgets first.** State latency p50/p95/p99, throughput/error rate at a
   concurrency, total CPU/RAM, freshness, rebuild, write-overhead and binary-size targets.
   Tie them to a realistic fixture/usage mix; do not invent universal speed or memory claims.
3. **Measure the authoritative SQL baseline.** Run the actual pinned engine with representative,
   non-sensitive fixtures, effective authorization, query parameters, schema and durability.
   Inspect supported query-plan tools; do not issue PostgreSQL EXPLAIN syntax to Turso or
   infer performance from its compatibility label. Legacy PostgreSQL may use version-matched
   `EXPLAIN (ANALYZE, BUFFERS)` on an isolated authorized database; ANALYZE executes queries,
   including possible effects, and its timings are not API latency. No production load test
   or sensitive-data download is implied. Missing legacy measurements remain an explicit gap.
4. **Try simpler improvements.** Compare query shape, matching indexes, selected columns,
   statistics where supported, stable pagination and N+1 removal. Measure index write/storage
   cost. A bounded RAM or immutable-release cache needs scope/invalidation/current-access rules.
   Re-run the baseline before attributing improvement to another engine.
5. **Record DBSP fit.** Choose `adopt | reject | defer` for a concrete frequent join/aggregate,
   including update/fan-out rate, state size, reuse, serving path, rebuild/freshness budget and
   total resource cost. Indexed point reads/small data may not justify it. A rejected/deferred
   path does not enter default release artifacts. Do not route identity/session/security
   decisions through an eventual view. Adoption requires a bounded actual-engine experiment;
   a dependency, diagram, paper or fabricated benchmark is not acceptance.
6. **Define the serving boundary.** State the exact committed input represented by a result,
   how callers establish freshness and when they wait, fall back to equivalent authorized SQL,
   or return a typed unavailable outcome. Bound fallback load/time/concurrency. Unknown lag,
   rebuild or an engine fault never silently widens visibility or bypasses current permission.

## GET still depends on writes

Read-only requests do not make their inputs immutable. Track inserts/updates/deletes, publication
pointer changes, retractions, role/entitlement/security changes and relevant expiry without new
writes. Define maintenance/invalidation for each dependency. Cache/view keys include actor or
access scope, parameters and revisions as appropriate. Immutable media bytes do not mean access
is permanent; a view cannot grant rights or expose a private/withdrawn release after revocation.

Mutations keep authoritative transactions, CAS, idempotency receipts and errors under
[persistence](persistence.md). Do not weaken them to fit DBSP. Measure change capture, index/view
write amplification, commit latency, backlog/checkpoint bytes and CPU/RAM. An asynchronous
catalog view may fit where an approval or conditional-progress decision must remain authoritative.

## Turso committed-feed and checkpoint design

A proposed Turso adapter must first establish a recoverable source of truth:

- Store each business mutation and its required durable change record in the **same** tested
  transaction. Do not feed the engine before commit or rely on an in-memory after-commit callback
  that can lose an event during a crash.
- Specify complete-transaction ordering/visibility for the selected engine/isolation/writer
  model. A row ID or sequence allocated before commit is not automatically commit ordering under
  concurrent writers. A serialized writer may simplify the design only if its ownership and
  ordering are enforced and measured, not assumed from a connection-pool setting.
- Capture an aligned snapshot plus watermark with no gap or double application during bootstrap.
  Preserve signed additions/retractions, updates of keys and multi-table transaction boundaries.
  Source rollback contributes no committed delta. Publish a view and its position atomically.
- Replays after lost acknowledgement need defined deduplication/checkpoint behavior. Record
  retention, bounded catch-up, schema/query versioning, checkpoint loss and a full rebuild path.
  Do not invent checkpoint support or exactly-once semantics from the engine name.
- In-process consumers use the owning service's safe access path, not a second process opening
  its database. There is no cross-service ATTACH or shared-file read to feed an aggregate.
- Treat unknown freshness or unhealthy maintenance as an explicit state; fallback rechecks the
  same current access/expiry rules. Security/permission state is never inferred from stale counts.

Reuse appropriate existing pure signed-change/circuit code, but do not reuse PostgreSQL's
snapshot frontier as a Turso proof. A selected pure watermark/state kernel can earn bounded
Kani evidence under [the formal method](mutation-and-formal.md#bounded-model-checking); actual
snapshot/transaction/crash behavior still needs real-engine integration and fault injection.

## Differential and recovery checks

Compare the real maintained output and public API with actual authoritative SQL over the same
fixtures, authorization context and committed position. Align snapshot/checkpoint first; two
arbitrary concurrent `now` queries are not an oracle. A copied circuit or mock connector cannot
prove equivalence. Keep minimized counterexamples as deterministic regressions.

| Edge class | Required comparison or fault injection |
| --- | --- |
| Insert/update/delete/retraction | Match/non-match joins, changed grouping keys, last-member removal and withdrawn releases; check negative weights/removal, not additions only |
| Duplicate/order/rollback | Replay after lost ACK, allowed reordering, multiple events per transaction and rolled-back mutations; distinguish legitimate bag duplicates from delivery duplicates |
| SQL/time semantics | NULLs, empty aggregates, outer joins, integer overflow/precision, timestamp boundaries and expiry without writes; Vietnamese collation such as “Ánh đèn cuối sân khấu” |
| Pagination/sort | Equal-key stable tiebreakers, empty/last pages, insert/retract while paging and invalid cursors; authorization on every page |
| Transaction visibility | Concurrent commits and multi-table changes, no half-published transaction, required read-your-write behavior |
| Restart/catch-up | Crash before/after commit, input/output/ACK/checkpoint failure, unavailable checkpoint; no silently missing/double-applied result |
| Snapshot/rebuild | Bootstrap under writes, gap-free handoff, query/schema changes, full rebuild and atomic cutover at a common watermark |
| Scope/authorization | Same keys across actors/tenants, anonymous/denied/entitled callers, disabled actors, revoked roles, private drafts and withdrawn releases; no count/existence/cache leak |
| Freshness/failure | Lag below/above budget, unknown position, backpressure, engine outage and rebuild; exact wait/fallback/unavailable outcomes |

Each applicable class maps to a production test and command or a stated execution gap.
Adoption needs real Turso plus actual DBSP circuit/adapter evidence. In-memory repository doubles
and pure-model tests remain distinct; a real engine memory-only test does not prove disk recovery.

## Benchmark and evidence record

Compare baseline SQL, improved SQL/cache and DBSP only if run, with equivalent response,
permissions, durability and freshness. Include API-to-result transport/materialization, change
maintenance and recovery, not just circuit compute time. Migration-wide comparison also counts
removed and added components; excluding old recovery copies changes steady-state, not peak usage.

- Document cold/warm state preparation, snapshot/bootstrap and steady-state maintenance.
  Restarting the client alone does not make database/OS caches cold.
- Use representative cardinality, skew, parameter selectivity, tenant sizes, write bursts,
  progress/login traffic and concurrency; include cases where DBSP loses.
- Record sample count, duration, warm-up, repetitions/variance, p50/p95/p99, throughput/error
  rate, CPU/RSS/PSS, binary/dependency footprint, lag, rebuild cost and write amplification.
- Retain source/dirty state, actual database/DBSP versions and features, lockfile/toolchain,
  schema/query/index hashes, filesystem, hardware, seeds, non-secret config and raw-result paths.
  Keep private/large artifacts outside Git; commit safe reproducible fixture generators.

Extend the foundation's vocabulary/report without creating a new evidence level:

```text
Read inventory:          actual handler/query/schema/contract/test paths; workload and gaps
Authority / checkpoint: actual SQL oracle, aligned snapshot/committed position, access scope
Targets / observations: baseline -> improved SQL/cache -> candidate; samples and budgets
DBSP decision:          adopt | reject | defer; actual adapter/features or missing prerequisites
Write dependencies:    retractions/invalidation; measured capture, maintenance and recovery cost
Semantics / failures:  production test paths, seeds, differential and crash outcomes
Freshness / fallback:  bounded contract; lag/outage/rebuild/access evidence
Provenance / artifacts: source, engine, toolchain, fixture/config hashes and raw-result paths
Execution gaps:         actual PASS/FAIL/NOT_RUN/UNSUPPORTED plus reasons
```

## Review and acceptance

- [ ] Inventory/targets cite actual queries, consumers and authorization semantics.
- [ ] Actual pinned authoritative-engine baseline and simpler query/index/cache comparison
  exist, or missing measurements are explicitly NOT_RUN.
- [ ] DBSP decision is explicit. Adoption has a concrete consumer, actual adapter and measured
  total benefit; reject/defer does not leave an unconditional release dependency.
- [ ] Durable capture, ordering, snapshot alignment and full-transaction atomic visibility are
  established for this database; no copied `xid8`/`xmin` assumption or unsupported connector.
- [ ] Differential/fault tests cover relevant table rows at a shared committed position,
  including retractions, rollback, restart, rebuild, authorization and fallback.
- [ ] Mutation consistency, receipt/idempotency and security boundaries are not weakened.
- [ ] Benchmarks report complete comparable resource/latency/write cost, not an unrun speedup.
- [ ] Source/model/mock/real engine and production evidence remain separate; remaining work
  stays with its issue/owner rather than being declared implemented by documentation.

**Enforcement:** read inventory and fit assessment are review obligations. Inspect actual xtask
commands before claiming they run an adapter; the retained DBSP sample tests do not test a Turso
feed. Repository link checks prove hygiene, not performance or database semantics. A documented
exception needs an oracle and workload reason; missing infrastructure is not an adoption waiver.

## Cantos examples (proposed, not endpoints implemented here)

**Candidate:** public catalog language counts and episode aggregates repeatedly join release
metadata at measured scale. Compare optimized Turso SQL/cache with a bounded in-process DBSP
experiment; replace and retract releases, align positions, verify rows/counts/pages and current
access. Reject it if maintenance/RAM/binary cost exceeds its measured value.

**Exception:** one immutable manifest lookup with indexed current-access checks may need no
incremental aggregate. Keep authoritative lookup and scoped caching. Conditional progress/session
writes retain their transaction decisions even if a separate derived read later earns DBSP.

**Counterexample:** adding DBSP to every GET, or replaying raw row IDs as commit positions, with
no baseline/retractions/permissions/lag/rebuild proof. Reject: engine choice proves neither safe
results nor faster reads.
