# Cantos API — Editorial and External Audio Authority

The `cantos-api` Rust/Axum package is one modular backend for Studio and Theatre.
[ADR 0018](../../docs/decisions/0018-external-audio-production.md) removes server-side
TTS and workers: no executable voice model, local/cloud TTS transport, provider dispatch
or in-backend mixing should remain.

## Implemented functionality

- Script IR validation, diff and immutable revision store with actor permissions.
- Manuscript import and preserved source/extraction receipts.
- Host-authored adaptation context/proposal/review; Cantos never runs adaptation inference.
- Reference-only casting/voice planning, rights and immutable production-input approval.
  Entries still describing eSpeak/Google/VieNeu are historical **declarative fixtures**, not
  runtime synthesis or evidence of provider availability.
- Internal Catalog domain/application/PostgreSQL foundation: Work/ContentUnit metadata,
  permissions, immutable revisions, archival, guarded reorder, idempotency and bounded queries.
  [Evidence and continuation](../../docs/work-plan/011-studio-catalog-api.md); catalog HTTP
  routes and shared DTOs remain pending.
- Read-only, bounded RIFF/WAV PCM inspection under `contexts/media/domain/wav_inspection`.
- Separate HTTP surfaces: `/api/v1/studio/*` implemented in part; `/api/v1/theatre/*`
  and `/api/v1/admin/*` reserved (404). Pre-split Studio aliases remain deprecated.

## Planned functionality (not implemented)

Studio catalog HTTP/DTO/contracts and Web UI, versioned producer briefs,
external producer tokens, private object-storage prepare/finalize, audio rendition acceptance,
QC review, publication manifests and Theatre listener API. Read the
[external handoff proposal](../../packages/contracts/external-audio-handoff-v1.md) before
creating routes or DTOs. A proposal is not an existing API.

## Module map

```text
src/
  script_ir/                # pure structured text contract
  contexts/
    catalog/                # internal works/units, permissions and immutable history
    editorial/              # revision and review ownership
    ingestion/              # import and source preservation
    adaptation/             # external proposal acceptance
    production/             # planning-only guidance and rights
    media/domain/wav_inspection/ # pure byte inspection
  interfaces/http/          # Studio, reserved Theatre/Admin surfaces
  platform/                 # postgres/session/IDs/migrations/generic incremental
  bin/                      # server, migrate, operator, inspect-wav, validate/diff
```

The former `contexts/synthesis`, `cantos-worker`, `production-jobs`, `probe-*-tts` and
`inspect-tts-receipt` code paths were intentionally removed, not wrapped with a disabled
flag. Deleted provider/worker compatibility reexports are not maintained.

## Migration compatibility

Previously applied SQL migrations 0001–0009 are immutable and replay checks their original
checksums. The old `durable_*`/`speech_clips` tables are legacy data, not current generation
queues. Do not edit old SQL or drop populated tables during this pivot. A future additive
migration 0011 adds catalog tables independently of the separately reserved audio-intake
0010. The migration registry currently has versions 1–9 and 11; upgrade/replay tests preserve
the old bytes/checksums. Catalog application-role grants are deliberately narrow and reproduced
in `tests/catalog_postgres.rs::grant_and_seed`; deployment role provisioning remains separate.

## Local verification

From the repository root, using Rust 1.87 and PostgreSQL 17 where needed:

```sh
cargo xtask verify repository
cargo xtask verify rust
cargo xtask verify contracts
cargo xtask verify postgres
cargo xtask preflight --changed-only
```

`cargo xtask verify postgres` runs a disposable, loopback-only DB test harness. Local
tests, integration mocks and verification must be reported with actual results; CI need
not finish for handoff. No test of historical TTS synthesis may be used as proof that
external audio upload, storage or publication exists.
