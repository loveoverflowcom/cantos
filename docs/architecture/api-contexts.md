# API Bounded Contexts and Dependency Policy

> [ADR 0018](../decisions/0018-external-audio-production.md) is the active ownership rule.
> This architecture removes the old server-side `synthesis` bounded context.

## Dependency direction

```text
              Script IR (pure shared kernel)
                        ▲
 editorial ◄── ingestion       adaptation (externally authored proposals)
       ▲              ▲        ▲
       └───────── production (planning/rights/instructions only)
                        ▲
                     future media assets/publication

 catalog ──► editorial
    └──────► ingestion
```

`contexts/editorial` owns canonical revisions and permissions. `ingestion` can read editorial
contracts; `adaptation` can read editorial/ingestion contracts; `production` reads editorial
to freeze guidance, rights and budget claims. Production Briefs additionally consume Catalog
transactional owner/content facts and Ingestion receipts through their owning entrypoints;
production remains downstream of Catalog and adds no table shortcut or generation engine. `catalog` uses editorial and ingestion entrypoints
to validate owner-bound references to existing revisions, rights records and manuscript sources;
it does not read those contexts' tables directly. `contexts/media/domain/wav_inspection` is a
pure inspector, with no provider/model dependency. Media also reads editorial entrypoints for
owner/revision authorization in its narrow private development intake; it does not import
production adapters or move context-specific storage into platform plumbing. `interfaces/http`
owns routers and maps
errors; `platform` owns pool/session/migrations/IDs and generic incremental computation,
not business-context shortcuts. The source checker in `scripts/check_repository.py`
enforces the allowed context edges.

## API surfaces

| Prefix | Who uses it | State |
| --- | --- | --- |
| `/api/v1/studio/*` | Studio Web catalog/editorial/import/adaptation/planning; owner-session development intake | Partially implemented |
| `/api/v1/theatre/*` | Listener discovery/playback/progress | Reserved; not implemented |
| `/api/v1/admin/*` | Administrative operations | Reserved; not implemented |
| Studio producer instructions | Exact read-only external bearer grant | [Brief v1](../../packages/contracts/production-brief-v1.md) implemented for development |
| Proposed Studio audio intake/release | Future upload/publisher scope | **Not implemented on develop** |

Catalog has pure domain, transactional application and PostgreSQL modules plus thin JSON HTTP
adapters exclusively under `/api/v1/studio`. Its [contract](../../packages/contracts/catalog-v1.md)
defines work/content-unit mutations, deterministic pages and immutable Work history.
[Content Identity](../../packages/contracts/content-v1.md) adds separate immutable content,
plain text/recording support and exact owned source/Script IR bindings in the same context.

The legacy alias `/api/v1/*` points to the same Studio handlers; avoid adding a new route
to both spaces by duplicating code. New handler contracts belong in `packages/contracts/api`
only when implemented. Do not invent implemented route claims from proposal documents.

## Current contexts

- `editorial`: accepted Script IR revisions, review and authorization.
- `ingestion`: original manuscript bytes and extraction receipts.
- `adaptation`: caller-owned proposal/acceptance, no model inference.
- `catalog`: work/content-unit identities, metadata, authorization and immutable revisions through
  application use cases, PostgreSQL persistence and the authenticated Studio HTTP API.
- `production`: declarative settings/rights/casting, immutable snapshots and approvals, plus
  Catalog-bound [Production Briefs](../../packages/contracts/production-brief-v1.md) and scoped
  read-only producer grants. Source/export bytes are copied through owning-context entrypoints.
  Existing provider catalog entries are **reference planning metadata**, not live adapters.
- `media`: structural WAV inspection plus explicit profile/binding rules, bounded Opus-byte
  inspection, owner-session private development intake and filesystem objects. No admitted file
  is implicitly QC/rights-reviewed or published; see [ADR 0020](../decisions/0020-studio-private-audio-intake.md).

## Next contexts

Implement Studio catalog Web acceptance against the real API separately. Implement catalog-bound
briefs, general scoped-producer `media` admission and review, `publication`
(approval and atomic manifest pointer) and Theatre read models. Favor typed IDs, pure state
machines and immutable versioned records. No TTS worker queue, local neural runtime,
commercial provider adapter, synthesis retry system or speech-clip registry belongs here.

## Transactions and external effects

Creator authorization, upload intent, finalize idempotency, observed storage bytes,
review permissions and publication must not race across owners or revisions. Database
transactions lock the scoped authoritative rows; object uploads complete outside them and
are verified before a separate finalized commit. All media stays private until approval.

The historical `durable_*` and `speech_clips` SQL objects remain in applied migrations
for upgrade integrity, but none of the active code writes generation jobs. A future schema
migration should add content/asset tables, not rewrite old migration checksums.

## Compatibility and verification

Keep surviving `revisions`, `production`, `postgres` and read-only `wav_inspection`
aliases temporarily. Deleted provider/`tts`/`worker` legacy library reexports intentionally
have no replacement: callers should integrate the external-audio API, not vendor SDKs.

Use `cargo xtask verify repository`, `verify rust`, `verify contracts`,
`verify postgres` and `preflight --changed-only`. Tests of old TTS dispatch are removed
with the retired runtime; write new tests for upload replay, ownership, byte integrity,
stale-brief decisions and publication instead.
