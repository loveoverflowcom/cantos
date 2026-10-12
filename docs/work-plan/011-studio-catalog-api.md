# Studio Catalog API and remaining Web acceptance — #66

## Outcome and review boundary

Deliver the complete authenticated Studio catalog HTTP API on the same
`feat/studio-catalog-api` branch and draft PR as the internal foundation. The internal
Catalog domain, transactional application commands/queries and additive migration are
implemented in session 1. Session 2 implements [JSON routes and shared contracts](../../packages/contracts/catalog-v1.md),
with [separate HTTP evidence](../evidence/studio-catalog-api.md). GitHub [#66](https://github.com/loveoverflowcom/cantos/issues/66)
owns execution and acceptance status; its Studio Web criteria remain outside this backend PR.

## Why and decisions from inspected code

The current Studio stores immutable Script IR revisions and original sources but now also has a
Work/ContentUnit hierarchy. Use the existing `Store`, development Studio session and
owner-scoped editorial/ingestion entrypoints; create no authentication framework, provider,
repository trait or replacement source store. Preserve ADR 0018 and surface boundaries.

The foundation selects novel/story → chapter and podcast → episode, with immutable kinds.
The issue does not prescribe a complete matrix; this conventional CMS mapping is explicit
and tested. Story chapters are plain metadata and require no scene/cast/script. A future
single-story episode mapping needs a deliberate compatibility decision rather than inference.
`private | unlisted | public` is descriptive catalog metadata; it grants no public access,
listener route, publication or release. Lifecycle is `draft | archived`; archive retains all
snapshots and unit positions. Restoring exists in the pure domain but has no application command.

## Implemented application boundary

Public exports are `cantos_api::contexts::catalog::{CatalogCommand, CatalogOutcome,
CatalogStoreError, MemberRole, CatalogCursor, CatalogPage}` and `catalog::domain::*`.
Application and PostgreSQL modules stay private. Existing `Store` methods are:

- `catalog_execute(token, operation, command) -> CatalogOutcome { work, units }`.
- `catalog_work(token, &WorkId)` and `catalog_unit(token, &ContentUnitId)`.
- `catalog_works(token, limit, include_archived, cursor)`.
- `catalog_units(token, &WorkId, limit, include_archived, cursor)`.
- `catalog_work_history(token, &WorkId, limit, cursor)`.

Commands: `CreateWork`, `EditWork`, `PatchWork`, `ArchiveWork`, `CreateUnit`, `EditUnit`, `PatchUnit`, `ArchiveUnit`,
`Reorder`, `SetMember`. Owner always comes from authentication. Work edits carry expected
Work revision; child edits/archive carry both expected Work and unit revisions. Every child
or membership mutation appends a Work revision. Reorder returns the new Work with empty
`units`; use the bounded units query for the resulting order. It permutes occupied active slots,
preserving archived tombstone positions. A work is capped at 1,000 retained units; pages at 50.

Reader can read; editor can edit metadata/units and reorder. Archive (Work or unit), membership,
rights-reference and visibility changes require owner. Membership removal works for inactive
actors; granting a role requires an active target. Strangers receive `NotFound` and every replay
reauthorizes against current access. Session expiry is checked after waits and before commit.

Optional source links pin the existing source ID and raw SHA-256. Script links pin canonical
UUID, exact revision and `sir-e1` export digest. They require the same owner as the Work even
when the caller is a script member. Optional content digest must match a linked script's `sir-c1`
digest, or the source SHA-256 when only a source exists; without a link it is a caller declaration.
Neither link changes legacy bytes/digests or represents rights clearance.

Idempotency is catalog-wide per actor+operation UUID. The fingerprint includes command kind,
IDs, expected revisions and all payload fields. An exact retry returns its original immutable
outcome; changed payload conflicts. Transactions serialize on the initial catalog clock, lock
current actor/member facts, append snapshots and receipt, bump epoch and commit together.
Cursor text is a bounded hex-encoded internal JSON value scoped to actor/query/filter and the
catalog epoch. Any catalog mutation invalidates outstanding cursors with `StaleCursor`.
The application wire facade validates raw DTOs and collects independent field errors. PATCH intent is
fingerprinted before applying omitted fields under transaction locks; explicit null clears only
optional unit links. The wire contract is JSON-only and treats cursor encoding as opaque.

## Dependencies and migration coordination

Base: `origin/develop` at `23b25a55cbb3b8d03526721835cf0318716b605a`, fetched before worktree creation.
Worktree: `/tmp/cantos-studio-catalog-api`; branch: `feat/studio-catalog-api`.
Migrations 0001–0009 are untouched. #64 independently reserves
`0010_studio_audio_intake.sql`; catalog registers `0011_catalog.sql` as version 11,
with ten registered migrations while 0010 is absent. The migration loop supports gaps.
The upgrade regression seeds the actual 0001–0009 schema and proves exact historical
bytes/checksums survive 0011 and replay. Do not copy unmerged intake code. When #64 merges,
reconcile the migration array/count and grants without rewriting either SQL checksum.
The #64 integration registers the combined ordered versions 1–11, preserves both additive SQL
files verbatim and keeps test-role grants context-scoped. Its authored upgrade regression also
covers a database already at 0011 with the 0010 gap; migration checks remain NOT_RUN in that
implementation environment until the disposable PostgreSQL gate can execute.

## Implementation record and next acceptance

1. Implemented only new `/api/v1/studio/*` routes; keep the deprecated alias, Theatre and Admin
   free of new catalog handlers. Follow existing origin/CSRF/session and JSON error conventions.
2. Added raw request/response DTOs, route constants, JSON schemas, acceptance/rejection fixtures,
   stable typed error mapping and pagination/idempotency/concurrency documentation under
   `packages/contracts`. PATCH semantics must preserve omitted fields explicitly and validate
   expected revisions; do not silently reinterpret an omitted optional link as deletion.
3. HTTP tests exercise real HTTP adapters/application/PG together, denied origins/anonymous/cross-owner
   actors, editor owner-only refusal, exact and conflicting replay, stale cursors/revisions,
   concurrent reorder, bounded body/field sizes, archive/history and server/PG restart.
   Advertise only representations actually implemented and tested.
4. Implementation handoff runs the applicable Rust/contracts/repository/PostgreSQL gates and changed-only preflight,
   independent review, fix confirmed findings, then update the same draft PR and verify remote
   SHA/tree. Do not open a second PR, merge, force-push or wait for remote CI.
5. Next Catalog acceptance keeps #66 open: Studio Web responsive/keyboard UX is a separate
   requested slice. Instructions,
   audio handoff, object storage, QC/publication, Theatre API, native playback and Studio Mobile
   are outside this PR.

## Risks and evidence

See [foundation evidence](../evidence/studio-catalog-foundation.md). The single catalog clock
prioritizes simple stable pages/current permissions at initial scale; all tenants contend and
unrelated mutations invalidate pages. No production throughput/latency acceptance is claimed.
Reassess the clock granularity and per-page membership lookups only against a measured workload.
The API has HTTP routes but no Catalog UI consumer or public delivery behavior.
Membership changes may advance archived Work revisions without changing content, allowing
current access revocation after archive; content edits remain draft-only.
