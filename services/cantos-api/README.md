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
- [Content Identity](../../packages/contracts/content-v1.md): immutable unit content, independent
  content sequence, text/source/script/recording bodies, full revision digest and exact retries.
  [Authority split](../../docs/decisions/0019-content-identity.md);
  [local evidence](../../docs/evidence/content-identity.md).
- [Production Brief v1](../../packages/contracts/production-brief-v1.md): immutable Catalog-bound
  instructions, exact source/Script exports, explicit Opus profiles, current stale comparison
  and independently scoped expiring/revocable read grants. [Evidence](../../docs/evidence/production-brief.md).
- Read-only, bounded RIFF/WAV PCM inspection under `contexts/media/domain/wav_inspection`.
- Development-only, script-owner Studio audio intent/byte/status endpoints: exact revision,
  content digest and explicit Opus profile; actual bounded inspection; private immutable
  filesystem objects; immutable PostgreSQL receipts and exact replay/conflict behavior.
  [Scope and contract](../../packages/contracts/studio-audio-intake-v1.md),
  [ADR 0020](../../docs/decisions/0020-studio-private-audio-intake.md). Required Rust and
  disposable-PostgreSQL evidence was originally **NOT_RUN** in PR69; [Audio Lifecycle evidence](../../docs/evidence/audio-lifecycle.md) records this Mac session.
- Provider-independent private Media storage: checked target/key/integrity values, filesystem
  and generic S3-compatible adapters, durable original target metadata and owner-authorized
  exact GET/HEAD/Range plus refreshable read access. [Architecture](../../docs/architecture/object-storage.md),
  [read contract](../../packages/contracts/private-content-reads-v1.md) and
  [evidence](../../docs/evidence/provider-independent-storage.md) distinguish the implemented
  private capability from separate live-provider/publication acceptance.
- [Publication v1](../../packages/contracts/publication-v1.md): owner distribution decisions,
  exact approval/readiness, immutable release snapshots, atomic CAS active pointers and withdrawal.
  [Core evidence](../../docs/evidence/publication-core.md) is CORE_VERIFIED / IO_ACCEPTANCE_PENDING:
  the initial real PostgreSQL acceptance attempt was blocked and default public delivery is unavailable.
- Separate HTTP surfaces: `/api/v1/studio/*` implemented in part; `/api/v1/theatre/*` serves
  Public Discovery/Playback and default-deny Library/Progress cores. `/api/v1/admin/*` remains
  reserved (404). Pre-split Studio aliases remain deprecated.

The [actor registry](../../docs/architecture/identity-registry.md) is implemented in
`platform/identity`: typed exact keys, atomic registration and explicit operator linking.
The registry itself authenticates no provider. The separate opt-in
[Studio OIDC BFF](../../docs/architecture/studio-oidc.md) provides credential verification, durable
callbacks and hashed Studio sessions on the current route group.
[OIDC evidence](../../docs/evidence/studio-oidc.md) distinguishes signed mocks/real PostgreSQL
from unrun Kanidm/browser/production acceptance.
[Evidence](../../docs/evidence/identity-registry.md) and
[OIDC/Listener handoff](../../docs/work-plan/identity-authentication-handoff.md) separate its scope.

## Planned functionality (not implemented)

Broader Studio Web acceptance, production identity/credential delivery, real-provider/CDN
acceptance, stronger rights/QC policy, public publication delivery and Theatre identity/native
listening acceptance.
[Audio Lifecycle](../../packages/contracts/audio-lifecycle-v1.md) implements development upload permissions/private admission, named review and private candidates. Read the
[external handoff proposal](../../packages/contracts/external-audio-handoff-v1.md) before
creating routes or DTOs. A proposal is not an existing API.

## Module map

```text
src/
  composition.rs            # concrete Store/use-case errors joining pool and Media storage
  script_ir/                # pure structured text contract
  contexts/
    catalog/                # works/units, permissions and immutable history
    editorial/              # revision and review ownership
    ingestion/              # import and source preservation
    adaptation/             # external proposal acceptance
    production/             # planning-only guidance and rights
    media/                  # private intake/review/read/storage and publication authority
      domain/               # pure lifecycle/publication/profile/integrity/range/WAV rules
    listening/              # separate account-owned Library/progress/history
  interfaces/http/          # Studio, Theatre and reserved Admin surfaces
  platform/                 # postgres/session/IDs/migrations/generic incremental
  bin/                      # server, migrate, operator, inspect-wav, validate/diff
```

The former `contexts/synthesis`, `cantos-worker`, `production-jobs`, `probe-*-tts` and
`inspect-tts-receipt` code paths were intentionally removed, not wrapped with a disabled
flag. Deleted provider/worker compatibility reexports are not maintained.

## Migration compatibility

Previously applied SQL migrations 0001–0009 are immutable and replay checks their original
checksums. The old `durable_*`/`speech_clips` tables are legacy data, not current generation
queues. Do not edit old SQL or drop populated tables during this pivot. Catalog
additive migration 0011 adds Catalog tables independently of audio-intake0010, now integrated with original SQL. Content Identity migration0012 is additive after Catalog0011. The migration registry has
versions1–20,25 (21 migrations), including private storage0015.
Standalone Publication0016 does not depend on storage0015. Discovery0017, Identity0018 and
Library/Progress0019 originals are inherited from develop. Studio0020 and0025 add its separate
credential issuance and session/browser-flow fences, with operator-applied narrow grants in
services/kanidm. Identity0018 alone grants no HTTP-role registry access. Production Brief
migration0013 is additive; upgrade/replay tests preserve
the old bytes/checksums. Catalog application-role grants are deliberately narrow and reproduced
in `tests/catalog_postgres.rs::grant_and_seed`; Content grants in
`tests/catalog_http_postgres.rs::Harness::new` add only SELECT/INSERT on content revisions/heads
and UPDATE(revision_id,revision) on heads. Brief grants add SELECT/INSERT on `production_briefs`, `production_brief_heads`,
`production_producer_grants`, `production_producer_revocations` and UPDATE(brief_id,version)
on Brief heads only. Deployment role provisioning remains separate.

Migration0015 adds `storage_target_id` to receipt/rendition location metadata without rewriting
immutable snapshots or digests. Existing rows name `legacy-filesystem`; new rows record the
configured target explicitly. Preserve the original root for that historical target. The old
filesystem-only application can still write local rows using the default, but cannot read new
S3 assets; rollback after remote writes requires a compatible reader.

## Private audio storage

Set an absolute `CANTOS_STORAGE_CONFIG` path to checked non-secret TOML; see
[storage.example.toml](config/storage.example.toml) and the
[configuration contract](../../docs/architecture/object-storage.md#checked-configuration-and-switching).
Credentials use named environment inputs outside Git. Every configured target is validated at
startup, including historical read targets, with no provider fallback. The preceding absolute
`CANTOS_AUDIO_ROOT` option remains compatible and selects `legacy-filesystem`; setting both is
an error. Without either option, private storage is unavailable.

For filesystem targets, the Unix adapter creates only
its final root directory and `objects` child; existing directories must be owned by the process
user and mode 0700. An intake-enabled API host disables all same-process static serving,
including nested public symlinks; `CANTOS_WEB_DIST` is unused for delivery in that mode.
Serve Studio separately through its existing Trunk proxy and set the matching
`CANTOS_WEB_ORIGIN` (normally `http://127.0.0.1:8081`). Keep that public host's tree and links
away from private objects. Unconfigured hosts retain existing editorial/static behavior but
fail audio writes closed.

Object identities are SHA-256-derived and private; metadata exposes existing content IDs rather
than keys. A short-lived authorized S3 grant can reveal its scoped location to its holder and
must never be logged or persisted. Files remain private/read-only; accepted receipt reads check
stored integrity. Filesystem objects are synchronized
before metadata acceptance, but filesystem/SQL are not one transaction. Crash/rollback may
leave private orphans; no request deletes a shared object. Reference-aware cleanup, retention,
production isolation and real-provider/CDN acceptance are deferred. S3 conditional writes verify
actual bytes; protected reads pin by version or strong ETag. Direct grants require a usable
version ID, otherwise refreshable access redirects to the protected API stream. Switching new
writes never migrates old bytes. No producer receives a Studio cookie and no upload/preview
becomes a release. Migration0014 adds immutable Content/Brief submissions, revocations,
renditions, reviews and candidates; its application-role grants are SELECT/INSERT only.
Brief read-grant issuance also needs SELECT on `audio_submissions` for credential-separation checks.

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

## Theatre discovery and playback boundary

The [Theatre v1 contract](../../packages/contracts/theatre-discovery-v1.md) exposes real
public-only work/search/detail/ordered-unit/manifest reads and anonymous playback authorization
at `/api/v1/theatre`. Media owns current eligibility; Catalog supplies exact batched snapshots
and Platform supplies owner activity. Studio cookies grant nothing here. Migration 0017 indexes
public releases in the existing atomic publication transaction. The default playback adapter
is explicitly unavailable; this is not real public delivery or production acceptance.
See [evidence](../../docs/evidence/theatre-discovery.md).
