# ADR 0004: Append-only editorial persistence in PostgreSQL

- Status: proposed (implemented in this slice; maintainer acceptance pending)
- Date: 2026-10-09
- Decision owner: Cantos maintainers
- Related issue or PR: [Part of #1](https://github.com/loveoverflowcom/cantos/issues/1)
- Supersedes / superseded by: N/A

## Context

The first backend must preserve private creator scripts across process restarts and concurrent
requests. The remote `develop` base has no implemented Script IR predecessor or database. The
user authorized proceeding from issue #1 and work item 010, so this slice includes the minimum
validated content contract it actually consumes. This is broader than persistence alone, and
still does not complete issue #1's Leptos consumer.

## Decision

Use one `cantos-server` Rust package with a pure Script IR/editorial core, a PostgreSQL adapter
and thin Axum handlers. Axum 0.8.9 and SQLx 0.8.6 are pinned in the manifest; Cargo.lock pins the
remaining graph. SQLx supplies asynchronous queries and embedded, checksummed migrations.
No repository trait is introduced: the only persistence implementation is real PostgreSQL.

Each save appends `script_versions`, retaining actor, operation, base, canonical bytes, digest
and content. Only `scripts.head_version` changes. A per-script row lock orders requests, and a
conditional head update rejects stale bases. The client's expected version remains the
concurrency contract: serialization is not permission to overwrite. Head and version commit
atomically, with a deferred composite foreign key preventing a dangling head.

Creation deduplicates on `(owner_id, create_operation_id)`. Saves deduplicate on
`(script_id, operation_id)`; their request fingerprint includes the submitted base and complete
normalized wire content. Acceptance uses its own operation namespace. Replay checks precede
stale-head checks and return the original result even after subsequent edits. Reuse with a
different request is an explicit conflict. Retrying creation returns the initial version 1 view.

Acceptance names an exact saved version and requires the current head for a new acceptance.
An already accepted version always returns its existing revision, including under a new operation
ID. Revisions reference immutable saved rows rather than duplicating content. A new saved version
with identical content may be accepted separately: it is a distinct editorial act. Production
and approval consumption of revision IDs is deferred; acceptance here grants no production or
publication rights.

Rejecting PostgreSQL triggers protect versions, acceptances, operation receipts, sources and
rights records from UPDATE, DELETE and TRUNCATE. The app exposes no mutation route for them.
Reads revalidate persisted Script IR and compare canonical bytes, the performance digest and a
separate SHA-256 of the complete stored wire value before exposing it. Script views additionally
check preserved source text against its checksum and match the work source/rights references.
All source text is preserved byte-for-byte as UTF-8 text, with SHA-256, provenance and actor.
Rights evidence is a creator assertion stored separately, always `pending_review` in this slice.
Source and rights references cannot change during saves; correcting them requires a new script
until a future explicit provenance/rights revision workflow exists.

## Alternatives

| Option | Benefits | Costs and limitations | Reason |
| --- | --- | --- | --- |
| Mutable JSON draft only | Small schema | Destroys history; fragile retries | Rejected |
| Optimistic version writes with immutable rows | No silent overwrite, complete history | More storage | Selected, row lock makes retry ordering explicit |
| External document store / event service | Independent scaling | Another operational boundary without a consumer | Rejected |
| Database role grants only | Strong runtime privilege boundary | Needs deployment/migration role management | Triggers now; split runtime/migrator roles before shared deployment |

## Consequences

- Product: creators can reopen exact history, resolve conflicts explicitly and accept pinned versions.
- Compatibility: initial forward migration has no old deployed rows to transform. Never edit an
  applied migration; corrections require a new migration. Rollback means stop the new binary and
  restore a tested database backup, not delete settled history. Rerunning migrations is tested.
- Operations: PostgreSQL is required; there is no in-memory fallback. Startup migrates before
  listening. A separate `migrate` command supports future separated deployment credentials.
- Recovery: failures before commit roll back history and pointer together. Ambiguous network
  outcomes are resolved by retrying the same operation ID. No external provider is invoked.
- Deferred: retention/pruning, rights adjudication, organization sharing, durable downstream
  production pins, revision diffs, importers and editor UI.

## Validation and revisit conditions

Use PostgreSQL 17 in a disposable local container and one database per integration test.
Exercise real HTTP routing with real queries, simultaneous requests behind an explicit row-lock
barrier, an injected insert failure, trigger rejection, and killing/restarting the actual server
process. Unit tests cover pure decisions. No fake store is acceptance evidence.

Official documentation checked 2026-10-09: [Axum Router](https://docs.rs/axum/0.8.9/axum/struct.Router.html),
[SQLx migrations](https://docs.rs/sqlx/0.8.6/sqlx/macro.migrate.html),
[PostgreSQL row locks](https://www.postgresql.org/docs/17/explicit-locking.html),
[triggers](https://www.postgresql.org/docs/17/sql-createtrigger.html).
Revisit when multiple creators share a script, migrations run under separate privileges,
source/rights corrections need stable history, or storage growth needs a retention policy.
