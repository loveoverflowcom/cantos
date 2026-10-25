# Native OCI release contract

[#348](https://github.com/loveoverflowcom/cantos/issues/348) selects GHCR distribution by registry
digest and Podman Quadlet supervision for the two native Rust executables. Native development,
offline E2E, Turso qualification and Kani remain independent of containers. This changes only
the earlier production no-container selection in the [native architecture](../architecture/native-turso-refactor.md).

## Implemented boundary

`cargo xtask release oci-verify` is an **offline, existing-artifact contract verifier**. It reuses
the release owner's bounded JSON, digest, regular-file, archive and verified-copy primitives.
It checks the admitted Server/Identity pair and a private snapshot of its small host bundle.
By default it never extracts or executes a helper, builds an image, logs in, contacts GHCR/SSH, writes a
deployment receipt or changes a host. `--dry-run` performs the same offline checks and prints
the same `PLAN_ONLY` result. Its temporary snapshot is removed on success or refusal.
The explicit `--registry-readback` option adds read-only GHCR manifest/config observation after
all offline guards; combined with `--dry-run` it remains entirely offline, including no authfile read.

These are independently reviewable parts of PR A, **not all of PR A**. Local/Actions candidate producers now exist (see [producer contract](#producers-release-oci-build));
actual publish, exact-image pair qualification and native host activation require their own
evidence. Producer identities are inputs, never proof that either ran. No production release can be made from this verifier alone. Legacy
`deploy vps` is a hidden retired refusal, not an archive-transfer or native activation lane.

The `cantos-identity` executable (`migrate`/`serve`, private authenticated Unix-socket RPC) exists
behind the crate's `service` feature; Server now composes embedded Turso foundation/sessions,
with business schema/SQL incomplete. There is no full two-production-binary HTTP acceptance. Keep legacy runtime, principal, PG/Kanidm, ingress, media and
recovery resources until #332–#336 provide actual accepted native journeys and recovery.
Source review or an offline plan cannot classify a real host as `NEW`, `LEGACY` or `NATIVE_READY`.
An inaccessible host or missing inventory is `UNKNOWN` and blocks live mutation.

## Explicit verification input

```sh
cargo xtask release oci-verify \
  --manifest /approved/release/manifest.json --manifest-digest FULL_SHA256 \
  --source-sha FULL_COMMIT_SHA --configuration-digest FULL_SHA256 \
  --platform linux/amd64 \
  --admission /approved/admission.json --admission-digest FULL_SHA256 \
  --dry-run
```

These paths and pins are operator inputs, not defaults or discovered latest artifacts. The
source commit must exist in local Git objects; no checkout or fetch occurs. The bundle's
`REVISION`, every committed path, full file bytes and executable mode must match that commit.
The SHA must identify a commit object, not a tree/tag/blob. All Git object reads use the
explicit `--no-lazy-fetch` global option, disable optional locks/replacement objects and deny
transports. Git must support that option (verified here with Git 2.49); an older unsupported
Git refuses before object access, with no unguarded fallback. Missing promisor objects refuse
even in dry-run rather than being fetched into the local object store. See
[Git's offline object-read option](https://git-scm.com/docs/git#Documentation/git.txt---no-lazy-fetch).
No host IP or credential is included. Configuration identity is an explicitly admitted digest;
the verifier does not establish whether that configuration is correct for a host.

## Native release manifest v1

This is a separate `kind=cantos-native-oci`, `schema_version=1` contract. Existing v2 API/Web/
Android archives and CircleCI promotion contracts keep their schema and consumers. Unknown or
missing fields, duplicate JSON keys, symlink/hardlink inputs and invalid identities are refused.

| Field | Contract |
| --- | --- |
| `repository` | `https://github.com/loveoverflowcom/cantos` |
| `source_sha`, `configuration_sha256`, `platform` | Full lowercase commit SHA, separately admitted SHA-256 configuration identity, exactly `linux/amd64` or `linux/arm64` |
| `producer` | Local: `kind=local`, bounded nonsecret `build_id`. Actions: `kind=github-actions`, full `workflow_sha`, repository `.github/workflows/*.yml` or `.yaml` path, positive string `run_id`/`run_attempt`, bounded `job_id` |
| `compatibility` | Nonempty, bounded identifiers for `rpc`, `server_schema`, `identity_schema`, `security_state`, `turso`; these describe the producer's declared compatibility, not tested rollback authority |
| `images` | Exactly `cantos-server` and `cantos-identity`, each with `ref` and `config_digest` |
| `runtime_bundle` | `path=runtime.tar`, bounded byte length, SHA-256 and exact `entries` with `path`, `bytes`, `sha256`, numeric `mode` (`420` for 0644, `493` for 0755) |
| `evidence` | `path=evidence.json`, bounded byte length and SHA-256 |

Each image `ref` is `ghcr.io/loveoverflowcom/SERVICE@sha256:HEX`. Tags, different registries,
swapped service packages and a config/image ID substituted for the registry manifest digest
are refused. V1 supports single-platform manifests; index fields are refused rather than
conflated with selected-platform or config digests. Producer readback must establish which
digest type was actually published. See the [OCI manifest specification](https://github.com/opencontainers/image-spec/blob/v1.1.1/manifest.md).

The host bundle is an uncompressed tar, at most 16 MiB and 256 regular members. This is a
contract limit for small host assets, **not a VPS capacity estimate**. Its exact declared
members include `REVISION` and a VPS asset; accepted paths are the Caddyfile, named Cantos
helpers, Quadlet/systemd units and tracked configuration templates. DB, media, private env
files, links, traversal, duplicate members, prefix collisions and special modes are refused.
Every member must also match committed source. `infra/vps/bin/cantos-vps` specifically retains
0755 as required by #313. No archive extraction or current/previous mutation occurs.

## Admission and evidence are separate from runtime observation

`admission.json` contains exactly `schema_version=1`, `manifest_sha256` and the exact `producer`
object. Its independently supplied digest pins that operator-reviewed admission. Obtain it
through the existing owner review process; creating a checksum locally does not create trust.
This is not a signature/key-management system and cannot prove who approved a file.

`evidence.json` contains `schema_version=1`, the same `source_sha`, `configuration_sha256`,
`platform`, complete `images`, `producer` and `gates`. Each named gate has `status=PASS` and a
`record_sha256` identifying the immutable producer record: `native-isolation`, `native-e2e`,
`migration-restore`, `image-pair`. The operator must review those records before admission.
The verifier now requires the actual bounded regular `gate-records/GATE.json` file for each
`record_sha256`, and checks its bytes plus `gate`, `status=PASS`, `source_sha`,
`configuration_sha256`, `platform` and complete `images` against the manifest. Other record
fields can carry the producer's command, environment and evidence references for operator review.
Previously admitted declarations without these records must be completed and re-admitted; a
hash string alone no longer passes. The verifier never executes those tests or authenticates
who wrote them. Operator review remains necessary; synthetic fixtures are never production PASS.

Output reports only `offline-contract=PASS`. `registry-access`, `native-qualification`,
`activation` and `cleanup` remain `NOT_RUN`, with operation `PLAN_ONLY`. It is not `PREPARED`,
`ACCEPTED`, an activation receipt or permission to stop legacy owners. The existing same-schema executor below and any future cutover executor must
independently revalidate registry bytes/metadata, inventory freshness, runtime principal/store,
disk/inodes, source-bound assets, admission, locks/generation and native migration/security
recovery before any mutation. Do not trust this plan as an activation token.

## Producers: `release oci-build`

`cargo xtask release oci-build --platform linux/amd64|linux/arm64 --producer local --build-id ID`
(or `--producer github-actions`) is the one producer owner ([`scripts/release_oci_build.py`](../../scripts/release_oci_build.py)),
shared by the local operator and the opt-in [`native-oci-publish`](../../.github/workflows/native-oci-publish.yml)
workflow (`workflow_dispatch` only; it never runs on push/PR and never deploys).

| Stage | Guard |
| --- | --- |
| Preflight | Linux host that natively matches `--platform` (no emulation, no VPS build), full HEAD SHA, clean tracked **and** untracked tree |
| Build | `git archive` of that commit; [`infra/oci/*/Containerfile`](../../infra/oci) with digest-pinned bases; `--format oci`; revision and configuration identity as required build arguments |
| Local image check | Before any push: platform, source/revision/configuration/binary labels, numeric non-root `User`, exact entrypoint with no `Cmd`, no credential-named environment |
| Publish | Only with `--publish --authfile PATH` (private, current-user-owned, no helper, never implicit). A private validated snapshot outside the build context is checked before building and used only at the publisher boundary. One `podman push --digestfile` of the inspected immutable local image ID per service (never a moving local tag) to a `sha-<commit>` tag; **a failed push is never retried** because the registry may have changed: reconcile first |
| Readback | The registry digest from the digestfile becomes `ghcr.io/loveoverflowcom/SERVICE@sha256:…`; the existing manifest/config readback must pass, so the config digest is the OCI config, never the image ID of another format |
| Candidate | `manifest.json`, `runtime.tar` (committed host assets with Git modes, deterministic) and `evidence.json` plus `gate-records/*.json`, written once under `artifacts/release-build/oci/SHA/CONFIG/ATTEMPT/candidate/` and re-verified against `release_oci` |

Without `--publish` the command builds and checks locally, prints `BUILD_ONLY` and writes nothing.
The **candidate is never eligible**: all four gates (`native-isolation`, `native-e2e`,
`migration-restore`, `image-pair`) are recorded `NOT_RUN`, so `release oci-verify` refuses it with
"producer gate did not PASS" until real gate records replace them and an operator admits the
producer. Nothing a producer writes is an admission, and labels or a pushed tag prove no provenance.

`infra/oci/compatibility.json` holds the declared RPC, Server schema, Identity schema and
security-state identifiers; the Turso version is read from the locked `Cargo.lock`. They are
declarations of this commit, not tested rollback authority. Server's declaration is
`server-turso-epoch-1-v24`; `cantos-migrate` operates on `server.db`, and serving never migrates at startup. Server runs as UID/GID 10001 and Identity
as 10002 so a bind-mounted database directory belongs to exactly one service; mounts, sockets and
Quadlet units are PR B.

Evidence: unit regressions run on real Git/tar data with a named command double for Podman
(`mock-integration-tested`). A real local build of both images was run once and matched the
readback contract; **no GHCR push, Actions run or image-pair fixture has been executed**.

### Publication uncertainty and workflow provenance

Before the first push, the producer creates a private attempt directory and atomically writes
`publish-attempt.json`; the file and directory are fsynced before each remote write. Each service
starts `not_started`, moves to `unknown` before its one push, then `confirmed` only after a valid
registry digest is read. The journal retains both config digests, exact tags, source/config/platform
and producer identity, with no credential path, token or remote body. A crash or second-push failure
preserves the first digest. Readback starts `unknown`; failure retains the unadmitted candidate and
both published digests. There is no resume/retry command. Reconcile exact digests and registry state
before deciding on a new explicitly authorized attempt; a mutable SHA tag is only a lookup hint.
A failure before the journal directory exists means no push began in this invocation.

Build subprocesses receive an allow-list of process/runtime locations, excluding provider tokens,
ambient registry-auth overrides and CI credentials. The validated publisher authfile is snapshotted
outside the source context and deleted with the private temporary directory. This does not inspect
image layers or prove the host's Podman configuration safe; layer/secret qualification remains open.

Actions requires a manual `workflow_dispatch`, source SHA equal to both checkout and workflow SHA,
and `run_attempt=1`. A rerun refuses; reconcile before a new dispatch. The workflow explicitly uses
Bash with `pipefail`, so a failing producer piped through `tee` fails the step. Candidate and attempt
artifacts are retained with `always()`, and authfile cleanup also runs with `always()`. Runner loss
can still prevent artifact upload; missing records mean UNKNOWN, never an automatic retry.

## Offline host inventory plan

Add `--host-inventory PATH --host-inventory-digest SHA256` to `release oci-verify`. Both flags are
required together. The same admission, source-bound bundle and qualification-record checks run
first. This optional local JSON input is **operator-declared**, not a host observation, SSH inventory,
activation token or deployment receipt. The command reads no lock/journal on a host and creates no
native Quadlet. The source bundle still contains retained legacy recovery assets; first-native rendering/cutover
and the existing same-schema upgrade consumer must be qualified independently.

Inventory schema 1 accepts exactly these fields (missing/unknown fields refuse):

| Field | Required value |
| --- | --- |
| `schema_version`, `host_id`, `observed_at` | `1`, bounded nonsecret host alias, integer Unix seconds within the preceding 24 hours; future/stale records refuse |
| `state`, `platform`, `cgroup_version` | `UNKNOWN`, `NEW`, `LEGACY` or `NATIVE_READY`; exact OCI platform; integer `2` for a compatible plan |
| `services` | Exactly `cantos-server` and `cantos-identity`; each has `runtime_mode` (`rootful`/`rootless`), `principal`, nonsecret `image_store` identity, `quadlet_scope` (`system`/`user`) and absolute canonical `database_directory` |
| `socket_directory` | A narrow absolute canonical shared UDS directory, separate from both non-overlapping DB directories |
| `journal`, `lock` | `UNKNOWN`/`IDLE`/`IN_PROGRESS`/`RECOVERY_REQUIRED`; `UNKNOWN`/`AVAILABLE`/`HELD`; only idle + available permits further review |
| `current_manifest_sha256`, `previous_manifest_sha256` | Full digests or JSON null; native-ready requires a known current release; no pointer is changed |
| `compatibility` | Exact manifest compatibility object; mismatch blocks planning rather than authorizing migration |

Rootful requires principal `root` and system scope; rootless requires a named non-root principal
and user scope. Each service retains its declared mode/principal/store. The plan pairs those facts
with its exact registry digest; it does not infer UID mappings or grant permission to chown data.
UNKNOWN blocks preparation; LEGACY requires separate cutover/native-recovery acceptance; NEW requires
explicit bootstrap and data-tier approval. Platform/cgroup/schema/current-release/operation blockers
are reported separately. Even a consistent declared snapshot returns `REVIEW_REQUIRED`,
`release_eligible=false`, activation/cleanup `NOT_RUN`. Fresh authenticated inventory, version/capacity,
UID/UDS isolation, native bundle renderer, lock/generation, recovery and separate authority remain.

## Release lane design directions

| Direction | Benefits and testability | Blast radius / compatibility | Unresolved decisions |
| --- | --- | --- | --- |
| A. Extend the existing separate release owners (selected) | Small deterministic cores and command/HTTP doubles; reuse OCI verifier/producer and the single Pages publisher | Changes stay within each provider lane; old API/aggregate selectors explicitly refuse, Web/Firebase remain separate | Real provider qualification and native host consumer |
| B. Add a cross-provider xtask coordinator | Central dependency/artifact identity and common reporting | Couples approvals, leases and partial outcomes across GHCR/VPS/Pages; requires a new migration contract | Whether cross-provider atomicity or ordering is actually needed |
| C. Sequential qualification checkpoints | Accept GHCR exact-image evidence first, then native host rehearsal; test Pages independently | Lowest activation risk, slower complete rollout; no replacement publisher | Operator evidence and capacity for each checkpoint |

Use A for this bounded implementation and C for acceptance ordering. Existing `release` helpers
own Pages CI intent/freshness while the existing deploy owner remains the only publisher. No new
cross-provider coordinator, generic plugin system or deployment engine is introduced. This is a
scoped implementation choice; native cutover/rollback policy remains deferred to #348/#336.

## Dependency-correct next slices

1. PR A's local and opt-in Actions producers now exist (above) but have not published. Still open:
   live GHCR publish/readback, exact-image layer and pair fixtures, and
   qualify both native executables and health/migration tools, isolated fixture commands,
   platform builder, pinned base images and clean source context. Use GHCR namespace authority,
   minimum token scopes, stdin authentication and private authfiles; never provision secrets or
   change package visibility implicitly. Publish once, obtain registry digests, pull/read back
   those exact digests and test the pair. An uncertain push needs reconciliation before retry.
2. PR B adds the inventory/planner and host activation consumer to the existing deploy/VPS
   owners after PR A's contract and producers are accepted. Preserve locks, attempt journals,
   current/previous and bundle checks. Implement the issue's `UNKNOWN`, `NEW`, `LEGACY` and
   `NATIVE_READY` lanes; no default tar/scp/load image distribution or Rust build on the VPS.
3. Run authorized Linux rehearsal separately from native container-free acceptance. Live
   bootstrap/activation/restore/reboot/cleanup each retain their specific authority and evidence.
   In the LEGACY lane, incomplete native acceptance permits preparation/rehearsal only.
   Cleanup stays plan-only until exact resources, retention, fresh simulation, recovery and
   owner approval satisfy #348. No automatic DB restore, package purge or broad prune.

GHCR permissions/visibility are independent from repository visibility; see the
[registry documentation](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry).
The offline verifier needs no GHCR credential and never tests registry access.

## Explicit registry manifest/config readback

Add `--registry-readback` to the same pinned `release oci-verify` command to read the exact
admitted pair from GHCR. Optional `--authfile PATH` selects an existing private, current-user-owned
Podman/Docker-format JSON file with `auths.ghcr.io.auth`. It must be a bounded regular file with
no group/other permissions or links. No implicit auth path, environment token, login, credential
helper, provisioning, package creation or visibility change is supported. Keep authfiles outside Git.

The offline manifest/admission, source/bundle and declared qualification guards run first.
Missing native evidence refuses before any network client or authfile opens. A `--dry-run`
remains `PLAN_ONLY` with no network/authfile read, even when readback was requested.

The adapter is reusable by the future mandatory local and Actions producers. Its current CLI
consumer re-observes an existing admitted release; it does not create a producer manifest or
bootstrap missing native evidence. A producer must obtain/test its actual digests before final
admission; this observer cannot be used to create a circular claim of native qualification.

Readback verifies at most 2 MiB per response, hashes the **raw registry manifest bytes**, checks
the exact OCI single-platform manifest/config media types, the config descriptor's digest/size
against the admitted config ID, and hashes the raw config bytes. Tag lookup, OCI indexes,
Docker-format manifests and foreign layer URLs are refused in this v1 lane. HTTP config blobs
may use `application/octet-stream`; their manifest descriptor must still identify OCI config.
Missing/wrong media types, conflicting/duplicate digest headers, malformed/duplicate-key JSON, truncated and
oversized responses cannot become a successful observation.
The optional `Docker-Content-Digest` header never substitutes for hashing the bytes.

Config must match `linux/amd64` or `linux/arm64` without an additional variant, exact
`org.opencontainers.image.source`/`revision`, `io.cantos.release.configuration` and `io.cantos.binary`
labels, a positive numeric UID:GID, exact `/usr/local/bin/SERVICE` entrypoint and no command
override. The rootfs must declare one valid uncompressed `diff_id` per layer; those IDs are not
conflated with compressed layer digests or verified without the layers. Labels and this metadata check do not prove executable contents, absence of secrets
in layers, private DB isolation or actual health/auth/migration behavior.

Authentication is a bounded [Distribution Bearer challenge](https://distribution.github.io/distribution/spec/auth/token/):
only `https://ghcr.io/token`, service `ghcr.io`, and `repository:loveoverflowcom/SERVICE:pull`
are admitted. Basic credentials go only to that token endpoint; Bearer tokens stay in memory
and in their exact repository client. No push/admin scope or cross-service token reuse.
401/403/404 and other HTTP outcomes remain distinct safe codes, with no tag/producer fallback.

TLS verification remains enabled. Automatic redirects are disabled. Token/manifest redirects
are refused; a config-blob redirect may go only to canonical HTTPS
`pkg-containers.githubusercontent.com`, at most two hops, with **no Authorization/cookie
forwarding**. Other origins, userinfo, ports, fragments or unsafe URL characters refuse before
another request. Signed CDN URLs and remote bodies are never included in errors/receipts.
GitHub documents this CDN in its [package networking reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners#communication-requirements-for-github-hosted-runners).

Output is a sanitized `OBSERVED` record: `registry-access` and `registry-manifest-config` may
be `PASS`; `release_eligible` is always false. Image layers/pair fixtures, native qualification,
activation and cleanup remain `NOT_RUN`. It writes no release or host state. An error returns
a stable code (`HTTP_401`, `HTTP_403`, `HTTP_404`, `TLS_VERIFICATION`, `NETWORK_*`, `RESPONSE_SIZE`,
`AUTH_*`, `REDIRECT_*`, `DIGEST_*`, `MEDIA_TYPE` or specific metadata refusal) without remote text.

Parser/client regressions exercise production code with a named HTTP transport double and a
real standard-library HTTP response reader. They are `mock-integration-tested`, not real GHCR
integration. No real publish, private registry login, digest pull, image run or native release
acceptance follows from these fixtures; those remain explicit producer/operator gates.

## Existing native-host executor (review boundary)

`cargo xtask release native-upgrade --request /private/request.json --request-digest SHA256`
validates both admitted releases and a strict request offline. Add `--execute` only on the
explicitly authorized target host to upgrade it. This is **host-local**, rootful Linux, cgroup v2,
existing **NATIVE_READY**, same schema/RPC/security/Turso identifiers and unchanged runtime
configuration only. No SSH connection, NEW bootstrap, LEGACY cutover, rootless conversion,
credential setup, live seed, schema upgrade or database restore is implemented by this lane.
The retired `deploy vps` command remains a refusal.

The release build configuration digest may change between versions. Runtime configuration is
bound separately by the exact existing Quadlet bytes and `configuration_files` hashes. Every
EnvironmentFile and mounted regular file must be bound; services with systemd drop-ins refuse.
The adapter preserves each unit's observed owner/group/mode and every Quadlet byte except its single `[Container] Image=` value. It does
not extract or execute the retained legacy runtime bundle.

Both old and new releases must pass the existing source-bound bundle/evidence/admission
verifier, match their actual committed schema registries, and have all four qualification records
marked `evidence_level: integration-tested` and
`execution_environment: linux-native-exact-images`. These are **reviewed execution claims**,
not cryptographic proof of execution. The executor never manufactures a qualification PASS or
an admission. Model, fake-host and file-only tests cannot provide these records. The producer
continues to emit NOT_RUN/unadmitted candidates. No production qualification is claimed here.

The producer now compares declared Server/Identity schema identifiers with the actual registered
migration versions before building. Current source selects Server epoch 1/version 24 and
Identity version 4. A stale declaration refuses; old published v23-declared candidates whose
source registers v24 remain unsuitable for this executor. Registered SQL is unchanged.

### Request and existing state

The request is a private regular, single-link JSON file pinned by SHA-256. Its exact fields are:

| Field | Required value |
| --- | --- |
| `schema_version`, `host_state`, `observed_at` | `1`, `NATIVE_READY`, integer Unix seconds within one hour (recovery may use the original request) |
| `host_machine_sha256` | Independently verified SHA-256 of this host's `/etc/machine-id` bytes |
| `generation`, `current_manifest_sha256` | Exact existing generation and previous manifest digest; reobserved under lock |
| `previous`, `next` | Each: `manifest`, `manifest_digest`, `admission`, `admission_digest`, `source_sha`, `configuration_digest`, `platform`; absolute pinned local records as for `oci-verify` |
| `databases` | Exactly `cantos-server` and `cantos-identity`: distinct canonical existing directories below `/var/lib/cantos/`, owned UID/GID 10001 and 10002 respectively, no other permissions |
| `socket_directory` | Existing canonical directory below `/run/cantos/`, Identity-owned UID 10002/group 10001, setgid, no other permissions |
| `mounts` | Exactly the two service names, each mapping to a complete list of 2–16 objects with `source`, `destination`, `read_only`; bind mounts only, canonical paths, no overlapping destinations or foreign DB roots. DB mount writable, config-file mounts explicitly bound. Runtime DB env must resolve to that mount's `server.db`/`identity.db`; both use `identity.sock` under their socket mount. Numeric peer UID/caller and 0660 socket policy are checked. |
| `units` | Exactly the two service names mapped to SHA-256 of their existing `/etc/containers/systemd/SERVICE.container` bytes |
| `configuration_files` | Explicit map of 1–32 canonical `/etc/cantos/` regular files to their byte hashes, all operator-private and bounded to 2 MiB per file; never put their contents in the request |
| `health_url` | Exact `http://127.0.0.1:PORT/PATH`; redirects/proxies refused, bounded read/time budget; observation only, not authentication acceptance |

`/var/lib/cantos-release` must already exist, root-owned/private. `current.json` contains
`generation`, `manifest_sha256`, `images` (the exact admitted manifest's full image objects),
and optionally `previous_manifest_sha256`. Creating that trusted state is part of separately
reviewed first-native acceptance, **not** something this upgrade command bootstraps. A guessed
inventory/current.json is not native acceptance. The request requires operator-reviewed host
identity and exact running native pair; source-only examples never establish it.

### Effects, backup and recovery

Under one nonblocking host lock, generation/current images, machine/platform, live native
containers, mounts/UIDs, unit provenance and runtime configuration hashes are reobserved.
The executor retains original units/current state/request in a fresh private attempt directory,
then fsyncs `in-progress.json` with `RECOVERY_REQUIRED` **before the first pull**. A failed pull
or interruption never silently replays. Digest pulls are inspected for actual image/config/source
identity; tags never choose a release. Backup capacity/inodes are checked before stopping owners.

Stop order is Server then Identity, with inactive/no-running-owner checks. Both complete DB
directories (at most 4096 entries each), including WAL/sidecars, are cold-copied, ownership/modes preserved, source/copy
streaming hashes compared and files/directories fsynced before unit replacement. Symlinks, hardlinks,
foreign owners, filesystem boundaries and partial backups refuse. Backups are never substituted
for production files. After another unit/config hash check, replace only the image directives,
reload systemd, start Identity then Server and observe exact running pair/mount/UID plus bounded
loopback HTTP readiness. Commit the generation/current pointer only after those checks, retain
an immutable receipt and remove the intent marker last.

On any failure/crash, keep units, backups and marker for explicit reconciliation. The old
`current.json` can remain even when one new unit has started; **it is not then a running-state
claim**. Inspect the attempt and actual processes before recovery. An activation receipt reports
that limited host observation only; separate full HTTP/auth/mail/browser evidence stays required.

For explicit image-only rollback of that exact attempt:

```sh
cargo xtask release native-upgrade --request /private/original-request.json \
  --request-digest SHA256 --rollback-attempt attempt-HEX --execute
```

Recovery locks and rechecks the host, exact request, old/new generation, original unit hashes,
allowed unit/image pair and unchanged runtime configuration. It pulls the admitted old digests,
stops both owners, restores the original unit bytes, starts Identity then Server and increments
generation monotonically. It **never restores DB bytes**, so writes made after activation remain.
Any schema/config change, foreign generation/unit/container or previous rollback refuses.
An interrupted rollback retains its marker; never manually remove it to force a retry. A DB
restore needs its own reviewed consistent recovery operation and authorization.

The source-level tests exercise actual private files/sidecars/locks/hash failures with a named
`FakeHost` for systemd/Podman. They are executable recovery/admission regressions, not Linux
image-pair, Turso restore, host reboot or live VPS evidence. Run them through
`cargo xtask release self-test`; they also belong to the existing repository gate. Actual
same-schema Podman/Quadlet acceptance and the four exact-image qualification records remain
mandatory before any operational use. No production candidate is admitted by this PR.
