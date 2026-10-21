# Native OCI release contract

[#348](https://github.com/loveoverflowcom/cantos/issues/348) selects GHCR distribution by registry
digest and Podman Quadlet supervision for the two native Rust executables. Native development,
offline E2E, Turso qualification and Kani remain independent of containers. This changes only
the earlier production no-container selection in the [native architecture](../architecture/native-turso-refactor.md).

## Implemented boundary

`cargo xtask release oci-verify` is an **offline, existing-artifact contract verifier**. It reuses
the release owner's bounded JSON, digest, regular-file, archive and verified-copy primitives.
It checks the admitted Server/Identity pair and a private snapshot of its small host bundle.
It never extracts or executes a helper, builds an image, logs in, contacts GHCR/SSH, writes a
deployment receipt or changes a host. `--dry-run` performs the same offline checks and prints
the same `PLAN_ONLY` result. Its temporary snapshot is removed on success or refusal.

This is the first independently reviewable part of PR A, **not all of PR A**. Local and Actions
publishers, registry readback/post-publish fixture execution and the VPS consumer are not
implemented here. The two producer identities are supported inputs, not claims that either
producer has run. No production release can be made from this verifier alone. Legacy
`deploy vps` still has its existing archive-transfer behavior; it is not the native OCI lane.
Do not pass the new manifest to that legacy executor.

The current Identity crate has no executable/credential verifier/authenticated RPC; Server
still serves from PostgreSQL. Keep legacy runtime, principal, PG/Kanidm, ingress, media and
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

## Dependency-correct next slices

1. Finish PR A's mandatory local and opt-in Actions producers using the existing release owner.
   First qualify both native executables and health/migration tools, isolated fixture commands,
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
