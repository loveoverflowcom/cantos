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
- [Publication v1](../../packages/contracts/publication-v1.md): owner distribution decisions,
  exact approval/readiness, immutable release snapshots, atomic CAS active pointers and withdrawal.
  [Core evidence](../../docs/evidence/publication-core.md) is CORE_VERIFIED / IO_ACCEPTANCE_PENDING:
  real PostgreSQL acceptance is blocked and default public delivery is unavailable.
- Separate HTTP surfaces: `/api/v1/studio/*` implemented in part; `/api/v1/theatre/*`
  and `/api/v1/admin/*` with an opt-in [Admin capability/revocation and audit](../../docs/architecture/admin-authorization.md) slice. Pre-split Studio aliases remain deprecated.

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

Studio catalog Web UI, production identity/credential delivery, S3/CDN storage, stronger rights/QC policy, real publication delivery and Theatre listener API. [Audio Lifecycle](../../packages/contracts/audio-lifecycle-v1.md) implements development upload permissions/private admission, named review and private candidates. Read the
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
    media/                  # private intake, inspection, review and publication authority
      domain/               # pure lifecycle/publication/profile rules and WAV inspection
  interfaces/http/          # Separate Studio, Theatre and Admin surfaces
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
versions1–14 plus0016–0021 (20 migrations); private storage0015 remains separate.
Publication/Discovery/Library originals are inherited from develop, identity0018/Studio0020 from the published identity dependency.
Theatre0021 adds separate flows/sessions and narrow issuer/subject issuance; no Studio credential grants Listener authority.
[Theatre identity](../../docs/architecture/theatre-identity.md) records activation and client acceptance gaps.
Production Brief migration0013 is additive; upgrade/replay tests preserve
the old bytes/checksums. Catalog application-role grants are deliberately narrow and reproduced
in `tests/catalog_postgres.rs::grant_and_seed`; Content grants in
`tests/catalog_http_postgres.rs::Harness::new` add only SELECT/INSERT on content revisions/heads
and UPDATE(revision_id,revision) on heads. Brief grants add SELECT/INSERT on `production_briefs`, `production_brief_heads`,
`production_producer_grants`, `production_producer_revocations` and UPDATE(brief_id,version)
on Brief heads only. Deployment role provisioning remains separate.

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
production isolation and S3/CDN integration are deferred. No producer should receive a Studio cookie and no uploaded object becomes a release. Migration0014 adds immutable Content/Brief submissions, revocations, renditions, reviews and candidates; its application-role grants are SELECT/INSERT only. Brief read-grant issuance also needs SELECT on `audio_submissions` for credential-separation checks.

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

## Admin authorization and audit

The [Admin policy](../../docs/architecture/admin-authorization.md) and
[v1 contract](../../packages/contracts/admin-v1.md) define dedicated resource credentials,
current system-group observation, exact registry actor and append-only atomic audit. Optional
`CANTOS_ADMIN_OIDC_ISSUER` / `CANTOS_ADMIN_OIDC_SECRET` configure only the fixed Admin client;
no Studio/Theatre/producer credential grants access. Missing config yields503 unavailable.
Operator-applied function grants remain separate; see [Kanidm guide](../kanidm/README.md).
There is no Admin UI, login client, role-grant/unblock endpoint or resource ACL bypass. Live
Kanidm/custom current-group contract is NOT_RUN/unaccepted. Existing local-only host policy stays.
