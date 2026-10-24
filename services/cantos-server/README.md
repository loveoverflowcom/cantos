# Cantos API — Editorial and External Audio Authority

> **Native business port; production acceptance pending.** `Store`, `cantos-migrate`,
> `cantos-operator` and the host use `CANTOS_SERVER_DATABASE` (`server.db`), never `DATABASE_URL`.
> Appended migrations 12–23 now cover all 69 Server-owned business tables, development sessions
> and infrastructure history. Studio Catalog, Adaptation, Production, Media/artwork, Admin audit
> and publication adapters execute native SQL. See the [checkpoint](../../docs/architecture/server-turso-porting.md#checkpoint-boundary)
> and [17-file/183-oracle retirement](../../docs/architecture/server-turso/remaining-pg-retirement.md).
> Native Identity v1 owns authentication; production Admin still has no verifier and denies
> admission. Local tests do not establish host activation, PG data conversion or browser/device acceptance.

## Current source

The Server is one Rust/Axum listener for Studio/Theatre/Admin, exclusively owning Turso
`server.db`. `platform/store` provides checked statements, rows and errors; `platform/turso`
owns the append-only registry over `cantos-store-turso`. Pure reads use pinned snapshots and
mutations use the bounded serialized writer. No registered migration may be edited.
The merged PR #395 retired six files / 68 tests; the final 17 files / 183 tests are retired here with [explicit dispositions](../../docs/architecture/server-turso/remaining-pg-oracles.json).
The direct PostgreSQL runner refuses before setup and no longer retains unreachable orchestration.
Retained PG operating material below is historical recovery input, not a runnable native workflow.

### Module map

- `contexts/*/domain`: pure immutable decisions/types.
- `contexts/*/application*`: use cases, current authority and transaction coordination.
- Context persistence adapters (`turso`): native SQL and checked hydration over the single owner.
- `platform/identity`, `platform/identity_auth`, `platform/bearer`: actor binding, private RPC,
  native sessions and shared SessionPolicy/rotation decisions.
- `adapters/inbound/http`: separate surface routers, wire DTOs and error projection.
- `platform/store`, `platform/turso`: one file owner, readers/writer and explicit migrations.

The [Identity v1 contract](contracts/auth/identity-auth/v1/README.md) owns email/password,
Web cookie and CMP/native Bearer behavior. Identity returns fresh facts; the Server binds actor,
purpose and session. Keep exact origins, CSRF, replay/revocation/absolute lifetimes, atomic receipts
and late authority rechecks. Provider enrollment/OIDC adapters are removed. Admin has no native
verifier and fails closed; no config flag grants it authority.

## Current schema and explicit cutover

`CANTOS_SERVER_DATABASE` names the local Turso file. `cantos-migrate` explicitly creates/upgrades;
serve refuses missing, foreign or unmigrated files. `cantos-operator` requires the Server stopped
because it shares the exclusive file owner. See the [porting guide](../../docs/architecture/server-turso-porting.md#checkpoint-boundary).
Retained [PG schema](schema/README.md), checksums and old cutover records are migration oracles,
not commands for the native Server. User-data conversion/restore remains #336 work.

Business domain/application/HTTP cores preserve Catalog/Content/Brief, revision/rights,
private audio/publication and Listening behavior. Their prior PG evidence is historical;
no functionality described below establishes completed native business acceptance.

### Isolated prepared catalog

The opt-in [sample lane](../../docs/deployment/theatre-seeded-catalog.md),
`cargo xtask seed theatre --sample-input JSON --sample-media-source DIR --sample-root DIR`,
uses the current registry and existing Studio catalog/artwork handlers to create six private
Works, twelve draft chapters and six admitted PNG covers in a run-owned `server.db` outside
Git. The Server-owned `sample_catalog_turso` target's exact ignored `sample_catalog` test is
the primitive; ordinary gates never import the provided media. Stable identities and operation
receipts preserve replay, and the fixture closes/reopens the actual Turso owner.

The manifest preserves author/narrator/genre/tags and mock MP3 input without inventing those
production catalog fields or audio acceptance. `--sample-serve` serves this test-hosted Server
router on loopback after verification; it starts no Identity and does not qualify the standalone
binary or Web/CMP login. Public discovery still requires an accepted release, and
[ADR 0033](../../docs/decisions/0033-release-pinned-public-cover.md)'s public-cover disclosure
and wire decisions remain proposed. Private image admission never implies public display.
Reset archives only a marker-bound sample root after taking both exclusive DB owner locks; it
never deletes DB/WAL/media/evidence or opens Identity's file.

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
writes never migrates old bytes. The [VPS Studio composition](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/vps-production.md#development-private-audio-on-the-vps)
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
  [seeded catalog runbook](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/theatre-seeded-catalog.md#local-tls-front-end-for-browser-playback)).
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

[#348](../../docs/deployment/native-oci-release.md) selects GHCR digest pairs and Podman/Quadlet.
Source packaging/admission, actual images, host activation and deployed acceptance are separate.
No native activation command exists. Legacy VPS assets are recovery/migration inputs; documentation
cleanup changes no runtime, data, account, media or installed service.

## Rename from `cantos-api`

`services/cantos-server` is canonical. `services/cantos-api` source links in pinned evidence refer
to history; do not recreate a second Server implementation. Business adapters and archived test
oracles may legitimately retain PostgreSQL vocabulary while the port is incomplete.

## Local verification

Use `cargo xtask verify repository`, scoped Rust checks and `cargo xtask verify turso`.
`cargo xtask run cantos-server` starts Server + Identity with private development files and
explicit migrations. `app cantos-studio|cantos-theatre --web` starts only Trunk against an existing
API. `seed theatre` currently refuses without the native `catalog_http` fixture. Native checks
require no PostgreSQL, Kanidm, container engine or Auth Emulator. Hidden legacy commands refuse.

## Theatre discovery and playback boundary

Keep immutable publication/rendition/digest binding, current eligibility, Range/expiry/withdrawal,
exact actor/session authority and native playback tests when porting. No empty/mock success
may replace an unported query. Read optimization follows
[#384](../../.agents/skills/cantos-engineering/references/turso-read-performance.md); no speedup
or native HTTP parity is claimed from source cleanup.

## Admin authorization and audit

Admin remains a separate surface and capability/audit boundary. Its provider verifier is removed,
so current composition fails closed. No Studio/Theatre credential, roster or origin grants Admin.
Native Admin verification is an explicit remaining owner gap, not an invitation to restore OIDC.

## Separate Theatre native identity

CMP/native consumes first-party Identity v1 JSON login and opaque Bearer sessions. Web uses
cookies and exact-origin/CSRF. No OIDC browser callback or Firebase enrollment is configured.
Preserve client journal/single-flight/persist-before-admission, unknown-refresh cleanup and
late-result fencing; pure model/tests alone do not accept device secure storage or live login.

## Firebase mailbox enrollment

Retired with provider authentication. [Historical contract](contracts/theatre/identity/v1/README.md)
keeps the stable source trail; current mailbox proof is Identity-owned. Firebase Hosting,
Functions and App Distribution are independent and remain supported within their own scope.

## Direct Hosting browser transport

Configure explicit Studio/Theatre origins; each router keeps its own CORS/CSRF/purpose policy.
Preflight does not authenticate. Native Bearer requests follow their cookie-free contract.
The [composition contract](../../docs/architecture/server-http-composition.md) and
[Identity auth v1](contracts/auth/identity-auth/v1/README.md) own behavior. Static delivery,
browser acceptance and deployed-state observation require separate evidence.
