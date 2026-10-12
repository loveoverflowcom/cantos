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
- Studio Catalog JSON HTTP API and domain/application/PostgreSQL persistence: Work/ContentUnit metadata,
  permissions, immutable revisions, archival, guarded reorder, idempotency and bounded queries.
  [API contract](../../packages/contracts/catalog-v1.md), [evidence](../../docs/evidence/studio-catalog-api.md)
  and [remaining Catalog acceptance](../../docs/work-plan/011-studio-catalog-api.md).
- Read-only, bounded RIFF/WAV PCM inspection under `contexts/media/domain/wav_inspection`.
- Development-only, script-owner Studio audio intent/byte/status endpoints: exact revision,
  content digest and explicit Opus profile; actual bounded inspection; private immutable
  filesystem objects; immutable PostgreSQL receipts and exact replay/conflict behavior.
  [Scope and contract](../../packages/contracts/studio-audio-intake-v1.md),
  [ADR 0019](../../docs/decisions/0019-studio-private-audio-intake.md). Required Rust and
  disposable-PostgreSQL execution is **NOT_RUN** in this implementation environment.
- Separate HTTP surfaces: `/api/v1/studio/*` implemented in part; `/api/v1/theatre/*`
  and `/api/v1/admin/*` reserved (404). Pre-split Studio aliases remain deprecated.

## Planned functionality (not implemented)

Studio catalog Web UI, versioned producer briefs,
external producer tokens, S3 prepare/finalize, general content-unit audio renditions,
QC review, publication manifests and Theatre listener API. Read the
[external handoff proposal](../../packages/contracts/external-audio-handoff-v1.md) before
creating routes or DTOs. A proposal is not an existing API.

## Module map

```text
src/
  script_ir/                # pure structured text contract
  contexts/
    catalog/                # works/units, permissions and immutable history
    editorial/              # revision and review ownership
    ingestion/              # import and source preservation
    adaptation/             # external proposal acceptance
    production/             # planning-only guidance and rights
    media/                  # private development intake/application/storage/inspection
      domain/               # pure profile/binding rules and WAV inspection
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
queues. Do not edit old SQL or drop populated tables during this pivot. Additive migration 0010
introduces immutable `audio_upload_intents` and `audio_intake_receipts`; independent additive
migration 0011 adds Catalog tables. The migration registry now contains versions 1–11. Neither
SQL checksum is rewritten; historical 0001–0009 bytes/checksums remain unchanged. Catalog and
audio application-role grants are deliberately narrow and reproduced in their respective
PostgreSQL test harnesses; deployment role provisioning remains separate.

## Development audio storage

Intake writes require an explicit absolute `CANTOS_AUDIO_ROOT`. The Unix adapter creates only
its final root directory and `objects` child; existing directories must be owned by the process
user and mode 0700. An intake-enabled API host disables all same-process static serving,
including nested public symlinks; `CANTOS_WEB_DIST` is unused for delivery in that mode.
Serve Studio separately through its existing Trunk proxy and set the matching
`CANTOS_WEB_ORIGIN` (normally `http://127.0.0.1:8081`). Keep that public host's tree and links
away from private objects. Unconfigured hosts retain existing editorial/static behavior but
fail audio writes closed.

Stored keys are SHA-256-derived, immutable and absent from wire responses. Files remain private
and read-only; every accepted receipt read checks stored integrity. Objects are synchronized
before metadata acceptance, but filesystem/SQL are not one transaction. Crash/rollback may
leave private orphans; no request deletes a shared object. Reference-aware cleanup, retention,
production isolation and S3/CDN integration are deferred. No producer should receive a Studio
cookie and no uploaded object becomes a release.

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
