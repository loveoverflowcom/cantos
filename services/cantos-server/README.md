# Cantos API — Editorial and External Audio Authority

> **Incomplete native checkpoint.** `Store`, `cantos-migrate`, `cantos-operator` and the host now
> use `CANTOS_SERVER_DATABASE` (a local Turso file), never `DATABASE_URL`. Only actors,
> Identity sessions and development sessions have schema. Business adapters still contain
> PostgreSQL SQL and will fail against the missing tables; their former integration tests are
> retained outside Cargo discovery in `legacy-pg-tests/`. The capability descriptions and PG
> operating instructions below describe the pre-port baseline, not working native acceptance.
> Provider authentication is removed; native Identity v1 is the remaining auth owner, and
> Admin has no verifier and denies admission. Follow the [checkpoint boundary](../../docs/architecture/server-turso-porting.md#checkpoint-boundary)
> before using any command. This checkpoint is not deployable.

The [#253 Bearer foundation](../../packages/contracts/bearer-v1.md) implements scoped opaque access/refresh, durable rotation/revoke and two-leg Web exchange. [#295](https://github.com/loveoverflowcom/cantos/issues/295) selects Web cookies on the custom domains with migration pending; native retains Bearer. Existing cookie handlers require review before reuse; live issuer/gateway/browser/device acceptance remains separate.

The [session-policy boundary](../../docs/architecture/api-contexts.md#session-policy-and-credential-boundary)
keeps checked immutable lifetime decisions in `platform/bearer/core.rs`, trusted registration/
policy mapping in `bearer/client.rs` and opaque-token/protocol syntax in `bearer/credential.rs`.

The `cantos-server` Rust/Axum package is one modular backend for Studio and Theatre.
[ADR 0018](../../docs/decisions/0018-external-audio-production.md) removes server-side
TTS and workers: no executable voice model, local/cloud TTS transport, provider dispatch
or in-backend mixing should remain.

## Implemented functionality

- Script IR validation, diff and immutable revision store with actor permissions. Editorial's
  shared use cases/SQL return internal facts; HTTP and immutable document adapters project the
  existing versioned contracts. Safe diagnostic classification/redaction is shared by Editorial
  and Adaptation; see the [ownership map](../../docs/architecture/api-contexts.md#editorial-application-and-document-boundaries).
- Manuscript import and preserved source/extraction receipts.
- Host-authored adaptation context/proposal/review; Cantos never runs adaptation inference.
- Reference-only casting/voice planning, rights and immutable production-input approval.
  Entries still describing eSpeak/Google/VieNeu are historical **declarative fixtures**, not
  runtime synthesis or evidence of provider availability.
- Studio Catalog JSON HTTP API and domain/application/PostgreSQL persistence: Work/ContentUnit metadata,
  permissions, immutable revisions, archival, guarded reorder, idempotency and bounded queries.
  [API contract](../../packages/contracts/catalog-v1.md), [evidence](../../docs/evidence/studio-catalog-api.md)
  and [remaining Catalog acceptance](../../docs/archive/handoffs/011-studio-catalog-api.md).
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
- [Private Work artwork v1](../../packages/contracts/artwork-v1.md): one owner PNG/JPEG cover,
  bounded full-byte inspection, immutable intent/asset receipts, private GET/HEAD preview,
  operation-identified revision-CAS select/replace/clear and exact historical reads.
  New metadata defaults to JSON with opt-in Protobuf; encoded bytes remain unchanged.
  The existing private storage configuration also enables these current Studio routes and
  keeps the host API-only. No separate image storage/provider setting or public projection
  exists. [Artwork evidence](../../docs/evidence/private-work-artwork.md) keeps real SQL,
  authorized HTTP/restart and provider acceptance gaps explicit.
- [Publication v1](../../packages/contracts/publication-v1.md): owner distribution decisions,
  exact approval/readiness, immutable release snapshots, atomic CAS active pointers and withdrawal.
  [Core evidence](../../docs/evidence/publication-core.md) is CORE_VERIFIED / IO_ACCEPTANCE_PENDING:
  default public delivery remains unavailable unless the separate origin-delivery configuration below is complete.
- Opt-in origin public media: `FilesystemPublicationStorage` promotes inspected audio to a
  distinct immutable byte class; the actual host composes publication readiness, HMAC grants
  and GET/HEAD/Range with per-request expiry and current-release checks.
  [ADR 0032](../../docs/decisions/0032-theatre-origin-playback-delivery.md) separates source/local
  evidence from deployed TLS/proxy, throughput and native playback acceptance.
- Separate HTTP surfaces: `/api/v1/studio/*` implemented in part; `/api/v1/theatre/*`
  contains Discovery/Library and separate opt-in Web/native identity routes; `/api/v1/admin/*`
  provides the independent opt-in [Admin capability/revocation and audit](../../docs/architecture/admin-authorization.md)
  slice. Pre-split Studio aliases remain deprecated.

The [actor registry](../../docs/architecture/identity-registry.md) is implemented in
`platform/identity`: typed exact keys, atomic registration and explicit operator linking.
The registry itself authenticates no provider. The separate opt-in
[Studio OIDC BFF](../../docs/architecture/studio-oidc.md) provides credential verification, durable
callbacks and hashed Studio sessions on the current route group. Studio OIDC requires
`CANTOS_STUDIO_ADMIN_SUBJECTS`, an operator-owned JSON array of one or two exact issuer-scoped
subjects; callback and each authenticated request refuse all other identities. This admission
policy grants no separate Admin capability or Work permission, exposes no public Studio signup
and provisions no account or credentials. See the [operator contract](../kanidm/README.md#studio-administrator-provisioning-contract).
[OIDC evidence](../../docs/evidence/studio-oidc.md#independent-integrated-acceptance-13-october-2026)
records the latest signed mocks/real PostgreSQL and observed Studio browser checks separately
from deferred Kanidm, unobserved runtime/accessibility and production acceptance.
[Evidence](../../docs/evidence/identity-registry.md) and
[OIDC/Listener handoff](../../docs/archive/handoffs/identity-authentication-handoff.md) separate its scope.

## Planned functionality (not implemented)

Broader Studio Web acceptance, production identity/credential delivery, real-provider/CDN
acceptance, stronger rights/QC policy, S3 publication promotion and Theatre identity/native
listening acceptance.
[Audio Lifecycle](../../packages/contracts/audio-lifecycle-v1.md) implements development upload permissions/private admission, named review and private candidates. Read the
[external handoff proposal](../../packages/contracts/external-audio-handoff-v1.md) before
creating routes or DTOs. A proposal is not an existing API.

## Module map

```text
src/
  application.rs            # concrete Store/use-case errors joining pool and Media storage
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
  adapters/inbound/http/    # separate Studio, Theatre and Admin surfaces
    assembly.rs            # trusted concrete service/capability construction
    router.rs              # route graph and layer order, consuming assembled services
    state.rs               # HTTP request state
    errors.rs              # safe JSON application/storage error projection
    policy.rs              # cross-surface Origin/deadline/cache middleware
    studio/admission.rs    # Studio credentials, fences and admission lease
  platform/                 # postgres/session/IDs/migrations/optional grouped-count core
    turso/                  # uncomposed candidate server.db registry (#333; actor registry slice)
  bin/                      # server, migrate, operator, inspect-wav, validate/diff
```

The canonical HTTP namespace is `cantos_server::adapters::inbound::http`; `cantos_server::http`
remains a compatibility alias. See the [assembly/host matrix](../../docs/architecture/api-contexts.md#http-assembly-and-transport-owners)
for the existing constructor seams and fail-closed capability/static policy. `application::Store`
is concrete application glue, with retained `platform::postgres::{Store, StoreError}` aliases;
it is not an abstract repository. Persistence and transaction orchestration stay context-owned.

The former `contexts/synthesis`, `cantos-worker`, `production-jobs`, `probe-*-tts` and
`inspect-tts-receipt` code paths were intentionally removed, not wrapped with a disabled
flag. Deleted provider/worker compatibility reexports are not maintained.

## Current schema and explicit cutover

The only executable fresh-install schema is [epoch 2 baseline](schema/0001_current.sql).
It contains current Catalog/Content, editorial/source/caller adaptation, production guidance,
private audio/publication/listener state and separate auth/registration boundaries. There are
no provider attempts/observations, durable TTS jobs, speech clips or DBSP capture tables.
Current caller adaptation admits `c1` only; the [runtime inventory](../../docs/evidence/legacy-runtime-inventory.md)
records retained consumers and removed handlers. The generic optional incremental engine
remains independent of PostgreSQL authority and has no production consumer.

`cantos-migrate` serializes initialization under an advisory transaction lock. It creates epoch
and checksummed version-1 registries only in a genuinely empty database, in the same transaction
as the baseline. A matching current registry is checked without replaying DDL. An old
`cantos_migrations` registry, unmarked user objects, missing/foreign epochs, missing/extra registry
versions or checksum mismatch fail closed. No command implicitly upgrades or resets old data.
`cantos-migrate --check` and server startup perform read-only schema checks; the app role receives
only registry SELECT in the [runtime grants](../../infra/vps/postgres/runtime-grants.sql).

The [legacy manifest](schema/legacy-epoch-1-manifest.json) records the exact SHA-256 of all 29
historical migrations at `develop@3e6c183`. Their executable SQL is retired from this tree;
original bytes remain at that immutable Git revision. Deployed historical checksums and rows
are never rewritten. [Controlled cutover](../../docs/deployment/postgres-schema-cutover.md)
requires a live inventory, verified backup/restore, reviewed authoritative-data transformation
or specifically approved reinitialization, and a matching database/image rollback plan.
Ordinary VPS deploy and rollback refuse crossing epochs. This source branch must not be
rolled onto the existing VPS database as an automatic migration.

Supported server majors are PostgreSQL 17 and 18. The existing VPS/developer Podman pins stay
17.11; PostgreSQL 18.6 needs its own isolated acceptance and approved major upgrade. Rust client
pins are `tokio-postgres 0.7.18` and `deadpool-postgres 0.14.2`; actual local test versions and
outcomes belong in the PR. PostgreSQL 19 is refused. Current fresh/replay/refusal and least-privilege
acceptance runs through `cargo xtask verify postgres --suite schema`; it does not qualify live
cutover, the full media/restart campaign or an untested server version.

Private storage location IDs such as `legacy-filesystem` are still current authority: they
identify original bytes, not a retired TTS workflow. Preserve their roots and target mappings
when planning data conversion. A schema change never moves media or rewrites immutable digests.

## Private audio storage

Set an absolute `CANTOS_STORAGE_CONFIG` path to checked non-secret TOML; see
[storage.example.toml](config/storage.example.toml) (a Long Vân target profile is in
[storage.longvan.example.toml](config/storage.longvan.example.toml), unaccepted until its
[live acceptance](../../docs/evidence/longvan-s3-acceptance.md) runs) and the
[configuration contract](../../docs/architecture/object-storage.md#checked-configuration-and-switching).
Credentials use named environment inputs outside Git. Every configured target is validated at
startup, including historical read targets, with no provider fallback. The preceding absolute
`CANTOS_AUDIO_ROOT` option remains compatible and selects `legacy-filesystem`; setting both is
an error. Without either option, private storage is unavailable.

For filesystem targets, the Unix adapter creates only
its final root directory, immutable class marker and `objects` child; existing directories must be owned by the process
user and mode 0700. An intake-enabled API host disables all same-process static serving,
including nested public symlinks; `CANTOS_WEB_DIST` is unused for delivery in that mode.
Serve Studio separately through its existing Trunk proxy and set the matching
`CANTOS_STUDIO_WEB_ORIGIN` (normally `http://127.0.0.1:8081`). Keep that public host's tree and links
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
writes never migrates old bytes. The [VPS Studio composition](../../docs/deployment/vps-production.md#development-private-audio-on-the-vps)
selects the existing filesystem driver on a persistent host mount during initial development,
without Long Vân credentials; the separate Theatre API process is not given private file access. No producer receives a Studio cookie and no upload/preview
becomes a release. Migration0014 adds immutable Content/Brief submissions, revocations,
renditions, reviews and candidates; its application-role grants are SELECT/INSERT only.
Brief read-grant issuance also needs SELECT on `audio_submissions` for credential-separation checks.

## Origin public media storage

The default host grants no public playback. An operator can opt in by supplying all three:

- `CANTOS_PUBLICATION_ROOT`: absolute owner-only filesystem root, disjoint from every configured
  private filesystem root. Ancestors and descendants are refused too. Only the final root and
  its `objects` child are created; existing directory ownership/mode must already be correct.
- `CANTOS_PUBLIC_MEDIA_ORIGIN`: the exact bare HTTPS API origin as listeners reach it, without
  a trailing slash, path, query or credentials. This is trusted configuration, never inferred
  from a request's Host/Forwarded headers.
  For local browser playback `scripts/serve_studio.py` sets it to `https://localhost:8443`, which
  its `tls-front` command terminates in front of this HTTP host (see the
  [seeded catalog runbook](../../docs/deployment/theatre-seeded-catalog.md#local-tls-front-end-for-browser-playback)).
- `CANTOS_PLAYBACK_KEY_FILE`: absolute regular file, owned by the process user, mode 0400 or
  0600, containing exactly 32 raw key bytes (the all-zero key is refused). Symlink paths and
  partial configuration fail startup. Keep this file outside Git; key rotation invalidates
  outstanding grants and requires clients to refresh authorization.

This selects the production source composition, with no same-process static serving. It does
not deploy or provision storage, credentials, TLS, a proxy or a CDN. Production private intake
still uses `CANTOS_STORAGE_CONFIG`, independently of this publication class.

The origin-enabled `PublicationService` prepares a copy during authenticated Studio readiness
or publish, only when every existing candidate, rights, QC, approval and private-object gate
passes apart from delivery. It reads the exact accepted private rendition under a 60-second
bounded stream, checks its complete SHA-256/length and re-inspects the profile off the async
executor before calling `FilesystemPublicationStorage::promote(rendition_id, VerifiedAudio)`.
Promotion copies only fully inspected Ogg/Opus bytes, conditionally
creates and synchronizes a read-only content-addressed object, and checks exact replay. A crash
before metadata commit leaves an unreferenced object; no request deletes it. Publication
readiness reopens and hashes the actual bytes and checks the versioned audio profile before
returning short-lived delivery evidence. This does not create clearance, review, approval or
an active release: the existing Studio use case and commit-time gates still own all of them.
The commit transaction re-reads all authority and repeats every gate after promotion; a revoked
right or edited input can leave only an inaccessible unreferenced copy. Listener requests and
active-release reads never prepare copies. An S3 publication adapter remains separate work.

The root carries one atomically created `.cantos-storage-class-v1` marker. A private root can
never be reused as a publication root, or vice versa; an unmarked legacy `objects` directory
is adopted only by private storage. The public object's internal identity is
`publication-{rendition UUID}-{sha256}`, and never appears in a grant URL or public manifest.

The listener path is immutable manifest → playback authorization → `/api/v1/theatre/media/{grant}`.
Each media GET/HEAD rechecks current public eligibility and authenticates the complete immutable
identity. Byte opens check exact SHA-256/length, retain the hashed descriptor for streaming and
repeat eligibility after the open. Existing strong ETag, single Range/206/416 and cache policy
apply: `private, no-store, no-transform`, `audio/ogg`, no content encoding. Expiry, withdrawal,
supersession and owner disablement deny subsequent requests; this is not remote deletion of
bytes already received. No listener operation invokes a decoder, synthesis or transcode.
Read-only decoder inspection occurs only during admission/readiness, outside listening.

Local loopback HTTP acceptance may fetch the exact path from an HTTPS grant against the local
host; that proves API/storage semantics, without claiming TLS or deployed-proxy acceptance.
Real HTTPS/proxy logs, CORS, restart/key rotation, capacity and startup/seek measurements retain
[ADR 0032](../../docs/decisions/0032-theatre-origin-playback-delivery.md)'s deployment gates.
Native playback belongs to the following CMP step.

## Production host

`CANTOS_ENV=production` ([`platform::host`](src/platform/host.rs), [ADR 0039](../../docs/decisions/0039-vps-production-runtime.md))
requires an explicit `CANTOS_BIND_ADDRESS` and separate exact HTTPS `CANTOS_STUDIO_WEB_ORIGIN`
and `CANTOS_THEATRE_WEB_ORIGIN` (optional `CANTOS_ADMIN_WEB_ORIGIN`), serves no
static files, refuses development Studio tokens and token login, `CANTOS_WEB_DIST`,
`CANTOS_AUDIO_ROOT` and a private issuer CA, and keeps PostgreSQL local (a Unix socket in the
deployed image). Without Studio OIDC a production host has no Studio sign-in and
`/api/v1/studio/auth/configuration` answers 503. The deployed runtime grants for `cantos_app`
are [`infra/vps/postgres/runtime-grants.sql`](../../infra/vps/postgres/runtime-grants.sql);
`tests/production_grants_postgres.rs` proves they alone serve Studio and Theatre requests.
The [single-process source composition](../../docs/architecture/server-http-composition.md)
requires a future explicit configuration migration. The deployed two-process assets and live
runtime remain unchanged; this source PR does not accept deployment or cutover.

## Rename from `cantos-api`

[#332](https://github.com/loveoverflowcom/cantos/issues/332) (checkpoint 1/5 of
[#331](https://github.com/loveoverflowcom/cantos/issues/331)) renamed the package and directory
`cantos-api` → `cantos-server` in one change. This is a naming change only: no route,
DTO, schema, SQL checksum, session policy or public URL moved, and the `cantos-server` binary
already existed. The Rust library path (and so every `tracing` target and `RUST_LOG` filter) is
now `cantos_server`. `Cargo.lock` only renames and reorders the package entry; no dependency
version changed.

| Surface | Result |
| --- | --- |
| directory, package, lib path, workspace member, lockfile entry, `-p` selectors | renamed |
| `cargo xtask build\|run\|deploy vps\|probe` selector, Kani `--package`, preflight graph, `services/manifest.toml` | renamed; the old selector is not kept as an alias |
| release build/verify scripts' source paths, repository guards, tests, active docs | renamed |
| historical records (`docs/evidence`, `docs/archive`, `docs/decisions`, `*-handoff.md`) | only markdown link targets follow the moved files; prose keeps the name it was written under |
| deployed legacy VPS runtime: Quadlet units `cantos-api-studio`/`cantos-api-theatre`, their container names and network aliases, image `localhost/cantos-api` and its title label, `cantos-api.tar(.zst)` bundle name, the Caddy `cantos_api` snippet | **kept** until the operator-authorized cutover of #336; renaming them would change what a live host and its rollback/retention logic find |
| product wording "Cantos API", `CANTOS_API_ORIGIN` and `CantosApiOrigin` frontend configuration | kept; they are product/FE contracts, not the crate name |

`scripts/check_repository.py` fails when `services/cantos-api/Cargo.toml` reappears (a second
canonical implementation) or when the guarded `services/cantos-server/src` tree has no Rust
files, so a later move cannot leave the architecture guards passing over an empty walk.

## Local verification

From the repository root, using Rust 1.98.1 and PostgreSQL 17 or 18 where needed:

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

The same harness owns the synthetic Theatre development catalog
(`tests/support/theatre_dev_seed_http_postgres.rs`). It publishes novel, story and podcast
Works through the real Studio routes, app-role SQL and private Opus intake, with only the
test-only public-delivery observer substituted. The PostgreSQL gate runs it as a regression;
`cargo xtask seed theatre` retains its database for local Web/CMP work
([runbook](../../docs/deployment/theatre-seeded-catalog.md)). `cantos-server` never composes the
fake observer, so a seeded host keeps playback `rendition_unavailable`.

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

## Separate Theatre native identity

The development host opt-in `CANTOS_THEATRE_NATIVE_ISSUER` selects only the exact public client
`cantos-theatre-native` and `com.cantos.theatre:/oauth2/callback`; it takes no client secret.
Native flow/exchange/session routes and the strict [identity contract](../../packages/contracts/theatre-identity-v1.md)
use bounded same-origin discovery/JWKS, real ID/access signatures, current introspection and
separate SQL/native opaque session authority. Native resource admission never reuses a Web,
Studio or Admin credential. Normal configuration remains disabled and production hosting,
provider registration/TLS, native device and OS storage acceptance remain separate.
The Bearer native route additionally admits the exact debug callback
`com.cantos.theatre.dev:/oauth2/callback` only when `CANTOS_THEATRE_NATIVE_DEVELOPMENT_CALLBACK=admit`
(any other value refuses startup); see [callback profiles](../../docs/deployment/native-callback-profiles.md).

Migration 0024 preserves 0021 and adds native tables/narrow issuance plus Web final-expiry rollback.
Operator-applied development grants are SELECT on native sessions, UPDATE only revoked/
revocation_pending, SELECT/INSERT/UPDATE on native flows and EXECUTE on the narrow native finish
function and existing listener actor admission helpers. No API startup provisions these grants.
Use the existing disposable `cargo xtask verify postgres` gate; it includes the signed public-client
HTTP/PG suite. Recorded command results belong to the [evidence ledger](../../docs/evidence/theatre-identity.md).

## Firebase mailbox enrollment

The existing registration owner can select a default-disabled Firebase proof provider while
retaining Kanidm credentials/OIDC. Generic start, action-bound confirmation/cancellation and
one-shot setup delivery issue no session or Studio/Admin authority. Migration0028 preserves
binding/effect receipts and stores no action/provider/setup secrets. See
[ADR0036](../../docs/decisions/0036-firebase-enrollment-proof.md) and the
[operator runbook](../../docs/deployment/firebase-enrollment-proof.md) for all explicit inputs,
privilege/origin gates and unknown-outcome recovery. Local registration-only SQL evidence uses
`cargo xtask verify postgres --suite theatre-firebase`; full PostgreSQL acceptance remains separate.
The explicit `cargo xtask verify theatre-auth --suite firebase-emulator` lane composes the
existing HTTP handlers, PostgreSQL repository and installed Auth Emulator with a recording
mock issuer. See the [local test runbook](../../docs/deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing)
for prerequisites, isolation and the outstanding real Kanidm/browser acceptance.

## Direct Hosting browser transport

The executable serves Studio/Theatre/Admin through one listener with exact per-surface CORS,
CSRF, state and credential admission. Configure `CANTOS_STUDIO_WEB_ORIGIN`,
`CANTOS_THEATRE_WEB_ORIGIN` and optional `CANTOS_ADMIN_WEB_ORIGIN`; Admin still requires its own
verified capability and audit. Preflight runs before authentication and exposes only each
surface's declared headers/methods. Studio aliases retain Studio policy. Native requests omit
Origin under their existing cookie-free contract. The [route-policy inventory and compatibility
rules](../../docs/architecture/server-http-composition.md) explain the startup refusal for
`CANTOS_BROWSER_API_SURFACE` and production `CANTOS_WEB_ORIGIN`.
With optional runtime `CANTOS_API_ORIGIN`, OIDC callbacks use the shared API destination and
return to their own configured Web origin. No issuer account, roster, token or registration
authority is created by origin configuration.

See [ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md) and the
[current production runbook](../../docs/deployment/vps-production.md). Browser authentication
between web.app and DuckDNS remains unaccepted until the domain/gateway gates are met.
