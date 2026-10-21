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

These are independently reviewable parts of PR A, **not all of PR A**. Local and Actions
publishers, exact-image layer/pair fixture execution and the VPS consumer are not implemented
here. The two producer identities are supported inputs, not claims that either
producer has run. No production release can be made from this verifier alone. Legacy
`deploy vps` still has its existing archive-transfer behavior; it is not the native OCI lane.
Do not pass the new manifest to that legacy executor.

The `cantos-identity` executable (`migrate`/`serve`, private authenticated Unix-socket RPC) exists
behind the crate's `service` feature; Server still serves from PostgreSQL, so there is no
two-service native acceptance. Keep legacy runtime, principal, PG/Kanidm, ingress, media and
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
The verifier checks their binding and declarations; it neither retrieves nor executes those
tests and cannot infer their truth. Synthetic tests of this parser are not native evidence.

Output reports only `offline-contract=PASS`. `registry-access`, `native-qualification`,
`activation` and `cleanup` remain `NOT_RUN`, with operation `PLAN_ONLY`. It is not `PREPARED`,
`ACCEPTED`, an activation receipt or permission to stop legacy owners. A future executor must
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
| Publish | Only with `--publish --authfile PATH` (private, current-user-owned, no helper, never implicit). One `podman push --digestfile` per image to a `sha-<commit>` tag; **a failed push is never retried** because the registry may have changed: reconcile first |
| Readback | The registry digest from the digestfile becomes `ghcr.io/loveoverflowcom/SERVICE@sha256:…`; the existing manifest/config readback must pass, so the config digest is the OCI config, never the image ID of another format |
| Candidate | `manifest.json`, `runtime.tar` (committed host assets with Git modes, deterministic) and `evidence.json` plus `gate-records/*.json`, written once under `artifacts/release-build/oci/SHA/CONFIG/ATTEMPT/` and re-verified against `release_oci` |

Without `--publish` the command builds and checks locally, prints `BUILD_ONLY` and writes nothing.
The **candidate is never eligible**: all four gates (`native-isolation`, `native-e2e`,
`migration-restore`, `image-pair`) are recorded `NOT_RUN`, so `release oci-verify` refuses it with
"producer gate did not PASS" until real gate records replace them and an operator admits the
producer. Nothing a producer writes is an admission, and labels or a pushed tag prove no provenance.

`infra/oci/compatibility.json` holds the declared RPC, Server schema, Identity schema and
security-state identifiers; the Turso version is read from the locked `Cargo.lock`. They are
declarations of this commit, not tested rollback authority. The Server image keeps
`cantos-migrate` while it still owns a PostgreSQL schema. Server runs as UID/GID 10001 and Identity
as 10002 so a bind-mounted database directory belongs to exactly one service; mounts, sockets and
Quadlet units are PR B.

Evidence: unit regressions run on real Git/tar data with a named command double for Podman
(`mock-integration-tested`). A real local build of both images was run once and matched the
readback contract; **no GHCR push, Actions run or image-pair fixture has been executed**.

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
