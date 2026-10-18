# Theatre Library presentation metadata (#251)

Status: implemented backend/contract slice; client adoption and live acceptance are separate.
The [Library contract](../../packages/contracts/listener-library-v1.md#presentation-metadata-v110-explicit-opt-in)
and [discovery contract](../../packages/contracts/theatre-discovery-v1.md#presentation-metadata-v110-explicit-opt-in)
own the wire semantics. This record describes the implementation boundary and verification gaps.

## Invariants and compatibility

- Default v1 responses retain their exact field sets. Only `presentation=1` selects the separate
  v1.1.0 DTOs; unknown values are typed `invalid_request`. Existing strict Web/CMP decoders need
  no change until their separate adoption PR. Cursor vaults bind each representation separately.
- Library stores no title snapshot and changes no accepted progress, Saved, history or receipt.
  Continue/History metadata names only the stored exact, currently eligible audio identity;
  replacement or unavailable audio keeps its owned record and returns `presentation:null`.
  Saved names the current public representative chosen by the same release order as Work detail.
- Public metadata comes from Media's resolved `ListenerPublication`, including pinned Catalog
  revisions and the audio language. Mutable Studio titles, rights, private objects, casting and
  draft/unreleased units never become presentation sources.
- `unit_count` counts currently eligible units, before cursor/limit, in the read transaction.
  `ordinal` is the one-based current eligible list rank by immutable position anchor then UUID.
  It can change after withdrawal, expiry or publication; it is not an authored episode number,
  a season, a permanent identity or an editorial total. #177 still owns those authored concepts.

No new persistent schema/migration, endpoint, cache, service, dependency or deployment is added.
This narrow count/rank choice implements #251's requested decision; it does not accept the
proposed P1 metadata/season ADR or alter ADR 0035's omission of inaccessible rows and reasons.

## Read inventory and PostgreSQL ownership

| Consumer / owner | Read and scope | Dependencies / ordering |
| --- | --- | --- |
| `interfaces/http/theatre.rs`, Media `ListenerService` | opt-in works/search, detail and units; anonymous public-only | existing `postgres/listener.rs` eligibility joins; at most 50 selected Works per list page |
| `interfaces/http/theatre_library.rs`, Listening `PgLibrary` | opt-in Continue/Saved/History; authenticated Listener only, limit 1–50 | existing account keysets; bounded unit/Work ID batch passed to Media |
| Media `library_presentations` / `library_work_presentations` | current-public release-pinned projections, no per-item round trips | active pointer/owner, pinned and current Catalog eligibility, latest rights/approval, expiry, withdrawal and submission revocation |
| Media presented aggregate | counts/ranks before page filtering, in the existing repeatable-read transaction | existing public Work/unit indexes and immutable first-position anchors |

The default query path pays no count/rank cost. Enriched Work lists first select the existing
bounded representatives and then aggregate only their Work IDs in one set-based query.
Work detail and unit pages scope their aggregate to one Work. Response cardinality is bounded;
the number of eligible units examined within a selected Work depends on the catalog. Time-only
rights expiry changes the relation without a write. Counts/ranks describe this request's
snapshot and admission clock, not a cross-request frozen catalog or a playback grant.

Write dependencies are existing Catalog head/privacy/archive changes, active release pointer
replacement, rights/approval changes, withdrawal/revocation and actor deactivation. Reads do not
change any of them. Existing fresh-clock release rechecks remain in the Media adapters.

DBSP decision: **defer**. No incremental engine, cache or invalidation feed is introduced.
There is no measured hot-workload benefit justifying an additional authority/freshness boundary.
Before an optimization, capture the actual application-role PostgreSQL plans/timings at
representative Work/unit skew, compare simpler query/index changes and record p50/p95/p99,
concurrency, buffer/sort work and expired/retracted/private-unit equivalence. Performance
targets and measurements remain unrun; this change makes no latency or throughput claim.

## Evidence boundary

The PR and ignored handoff record hold exact commands, final source identity and observed
results. Contract fixtures cover both representations, nullable unavailable metadata, private
field rejection and positive JSON-safe totals/ranks. Focused production-code tests cover exact
audio binding, query opt-in, representation-scoped cursors and the foreign Work UUID guard.

Real PostgreSQL regressions cover restart/pinned titles, account isolation, replacement and
withdrawal without substitution, current privacy/revocation, counts beyond one page, staged and
archived exclusion, duplicate/gapped positions and rank-before-pagination. Their source alone
does not establish execution; the PR must report whether the disposable PostgreSQL gate ran.

Required follow-ups: finish any unrun Rust/PostgreSQL and read-performance qualification, then
adopt the explicit representation in `projectTheatreShell` in a separate client PR. UI,
localization resources, shared components and preview qualifications are unchanged here.
Android/iOS device, live identity, playback, deployment and production acceptance are not
established by these tests. Next acceptance owner is unassigned; assignment is required before
claiming the full capability accepted.
