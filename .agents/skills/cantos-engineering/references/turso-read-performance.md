# Turso read performance

## Scope and current decision

Apply this method to database-backed reads and writes feeding them. The owner decision in
[#384](https://github.com/loveoverflowcom/cantos/issues/384) is **Turso SQL/index first,
selective cache after measurement; incremental engines DEFER**. DBSP and its CLI feature/gate
are retired. Neither ArcSwap in a lockfile nor fewer joins establishes an application cache or speedup.
The [porting boundary](../../../../docs/architecture/server-turso-porting.md#checkpoint-boundary)
distinguishes composed foundation/session storage from unported business queries.

## Inventory and baseline

1. Trace the actual frontend request through HTTP, application and context persistence to
   tables/indexes. Include Discovery/Search and Work/Units with `presentation=1`, Library,
   playback metadata and the Studio screen's representation. Unknown traffic stays unknown.
2. Pin source, exact Turso version/features, schema/query/index hashes, host, durability,
   build profile, synthetic fixture/seed, pagination and concurrency. The module's schema and
   production query must run on Turso before claiming its native baseline; a PostgreSQL oracle
   or a shared-store test cannot establish business HTTP parity.
3. Set the workload/budget first. Record p50/p95/p99, throughput/errors, statements/request,
   returned rows, decoded/response bytes, SQL vs materialization time, CPU/RSS, writer wait,
   DB/index size and write latency. Report scanned rows only if measured by engine tooling.
4. Inspect the pinned engine's actual plan tools; never infer Turso behavior from SQLite or
   use PostgreSQL EXPLAIN syntax. Separate SQL time from media transfer and object I/O.

## SQL and concurrency before cache

Compare matching indexes, narrow candidate selection, bounded batch hydration, selected columns,
keyset pagination and demonstrated N+1 removal on the same workload. Two-phase reads are an
experiment, not a guaranteed speedup. Keep a common snapshot where consistency requires it.
Measure index storage/write costs and mixed progress/publication/withdrawal writes.

Preserve newest eligible representative selection **before** representation filters, literal
substring/Unicode semantics, full eligible-set count/ordinal before pagination, cursor scope and
tie-breakers. Test Vietnamese NFC/NFD, `%`/`_`, equal keys, empty/deep pages and continuation.
Library Continue selects the latest progress per Work before filtering in-progress.

Do not mechanically replace `transaction()` with `read()`. Re-derive actor/session/member
revocation and admission semantics for the touched use case; preserve late current-time authority
rechecks after external I/O, conditional writes, immutable bindings/digests and exact errors.
Unknown commit is not rollback; never hide unavailable behind empty data or stale results.
See [persistence](persistence.md) and [identity boundaries](identity-and-session-boundaries.md).

## Selective cache after optimized SQL

Measure optimized SQL first, then compare one consumer with cache on/off. ArcSwap is a candidate
for small immutable shared snapshots; Moka for bounded keyed reuse. Neither is mandatory; record
no-cache when benefit is insufficient. Do not add both, an engine, a service or a framework by default.

Turso remains authority. Cache never grants authentication, authorization, session/revocation or
playback access, and does not alter HTTP `no-store`. Keys cover scope, identity/revision,
representation/query/order and epochs where relevant. TTL alone proves no consistency: define
publish/withdraw/revoke/head invalidation, time-only expiry, read-your-write, commit-to-publish
ordering and in-flight readers. Unknown commit, cancellation, refresh failure and restart require
invalidate/unready or bounded authorized SQL fallback. Bound weight/entries, miss concurrency,
refresh and peak old/new snapshot memory. Retain a bypass for recovery and measurement.

## Review and acceptance

Use real file-backed Turso and production query/application seams with independent expected
fixtures. Cover rollback, races, reopen, lost acknowledgement, corrupt bindings, expiry and
cross-account isolation. Cache adds miss/hit/eviction/invalidation/restart/fault cases and equivalent
cache-on/off contracts. Keep benchmark execution opt-in and correctness regressions in existing gates.
Report actual commands, provenance, variation and remaining gaps; no unmeasured speed claims.
Do not restore `verify postgres`, `verify dbsp` or provider-auth lanes. No benchmark/deploy on VPS
follows from this method. #333 owns business porting, #384 optimization, #336 native acceptance.
