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
                     future catalog/media assets/publication
```

`contexts/editorial` owns canonical revisions and permissions. `ingestion` can read editorial
contracts; `adaptation` can read editorial/ingestion contracts; `production` reads editorial
to freeze guidance, rights and budget claims. `contexts/media/domain/wav_inspection` is a
pure inspector, with no provider/model dependency. `interfaces/http` owns routers and maps
errors; `platform` owns pool/session/migrations/IDs and generic incremental computation,
not business-context shortcuts. The source checker in `scripts/check_repository.py`
enforces the allowed context edges.

## API surfaces

| Prefix | Who uses it | State |
| --- | --- | --- |
| `/api/v1/studio/*` | Studio Web editorial/import/adaptation/production planning | Partially implemented |
| `/api/v1/theatre/*` | Listener discovery/playback/progress | Reserved; not implemented |
| `/api/v1/admin/*` | Administrative operations | Reserved; not implemented |
| Proposed Studio audio handoff | Scoped external producer plugin | **Not implemented** |

The legacy alias `/api/v1/*` points to the same Studio handlers; avoid adding a new route
to both spaces by duplicating code. New handler contracts belong in `packages/contracts/api`
only when implemented. Do not invent implemented route claims from proposal documents.

## Current contexts

- `editorial`: accepted Script IR revisions, review and authorization.
- `ingestion`: original manuscript bytes and extraction receipts.
- `adaptation`: caller-owned proposal/acceptance, no model inference.
- `production`: purely declarative settings/rights/casting, immutable snapshots and approvals.
  Existing provider catalog entries are **reference planning metadata**, not live adapters.
- `media`: structural WAV byte inspection, soon to own admission rules for externally
  supplied audio. No WAV file is implicitly QC-approved.

## Next contexts

Implement separately reviewable `catalog` (works and content units), `media` admission
(upload intent, digest/metadata check, private object reference, review), `publication`
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
