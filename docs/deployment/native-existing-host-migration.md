# Existing native host: Server v23 to v24

This is a separate, explicit adoption lane for #348. `release native-adopt` is implemented
for the known rootful `/root/cantos-native` layout; it never fabricates an admitted previous
release or relaxes `native-upgrade`'s same-schema equality guard. This document describes code
and the operator procedure, **not production acceptance or authority to execute it**.

The review boundary is one additive Server migration, epoch 1/version 23 to 24, with identical
Identity schema, RPC, security-state and Turso identifiers. Registered migration SQL stays
immutable. Unknown layouts, other schema transitions, rootless hosts, changed runtime policy,
missing provenance or unqualified images refuse. PostgreSQL dumps cannot restore this native
pair. First-native installation, legacy cutover, credential/IAM changes, ingress edits, data
reset and media conversion remain outside this lane.

Version 24 adds the cover-disclosure table with no inferred consent or backfilled public-display
authority. Existing rows/assets stay intact; public-cover/business acceptance and any owner
disclosures remain separate from successful schema migration.

## Evidence before effects

Use the [native OCI contract](native-oci-release.md) for the target manifest, source/configuration
pins, four real exact-image qualification records and producer admission. A new executor source
needs its own built/qualified target; old source-bound receipts cannot be relabelled. The
observed old pair is a baseline inventory, **not a backfilled release admission**.

`infra/oci/historical-v23.json` pins the previously observed public image identities at
`d069c3895d03f47894c8a69a7259c284cc417518`. It contains no live DB observation or credential.
Recheck the actual host before use. The candidate at
`2ee9360817a207eb89a8f05ee9ed42102d714229` had only fresh-schema/current-schema restore
qualification; those results do not establish a historical upgrade or this executor's behavior.

`release native-rehearse` uses the existing disposable Linux/Podman fixture to:

1. Pull/read back both exact pairs and verify source/configuration/platform/image identities.
2. Create **v23 with the old Server image**, create Identity v4 with the old Identity image,
   and populate actual synthetic SMTP/signup/password/session/actor state through both services.
3. Stop both owners, copy and hash the complete cold pair, and prove the new Server refuses
   serving the pending v23 schema without changing its bytes.
4. Restore the pair into fresh roots, migrate with both new images, repeat migration, check
   engine integrity/foreign keys, and exercise the preserved HTTP session and password.
5. Restore **both original v23/Identity roots** into different fresh roots and exercise the old
   image pair's session/password without running any migration on the restored pair.

The same run additionally exercises the adoption adapter with the new exact pair on a uniquely
named private Podman bridge, without published ports. It checks actual inspect metadata and
NUL-delimited `/proc` argv, observes container-bound HTTP, and refuses a foreign network identity.
The report's separate `host_adapter` result must be PASS and bind the exact target source/images;
historical evidence without that result does not admit this adapter.

It writes `historical-upgrade.json` separately from the four OCI gate records. A failure retains
FAIL; interruption retains only an unfinished attempt. It never admits images, activates a host
or establishes live readiness. The manual `native-oci-publish` workflow runs both current-schema
qualification and this historical rehearsal, retaining their separate artifacts. No ordinary
preflight/CI job activates containers or a host.

```sh
cargo xtask release native-rehearse --manifest PATH --manifest-digest SHA256 \
  --source-sha FULL_SHA --configuration-digest SHA256 --platform linux/amd64 \
  --baseline infra/oci/historical-v23.json --baseline-digest SHA256 \
  --output artifacts/release-qualification/historical-v23-v24 --authfile PRIVATE_AUTHFILE
```

This requires a clean exact-source checkout, native rootful disposable Linux and the fixture's
existing tools. No production SSH key, provider secret or production configuration belongs in
the runner. Rust file-backed tests additionally cover populated v23 migration, unchanged prefix
checksums, opaque Vietnamese revision bytes, live/revoked sessions, old-binary refusal and real
DDL failure/repeated refusal. They do not replace the two-image rehearsal. The engine's
`foreign_key_check` pragma is not a usable oracle on Turso 0.8.3: a dangling-row negative control
returned no violations. Cold checks use bounded metadata-driven anti-joins, with real negative,
composite-key and NULL controls; unknown/implicit parent-key forms refuse. See the
[store boundary](../../packages/store-turso/README.md#explicit-cold-integrity-check).

## Reviewed host inventory and request

Collect a new nonsecret inventory under separately authorized read-only access. Bind the machine
ID hash, exact image/config digests and source, actual unit/configuration hashes, all effective
environment files, mounts, users, ownership, socket policy, other writers, capacity and service
state. Do not print environment/configuration payloads or keys. An image label alone does not
establish the on-disk schema. No additional production observation is implicit in a PR request.

Version-1 request fields are exact (unknown/missing fields refuse):

| Field | Meaning |
| --- | --- |
| `schema_version`, `operation` | Integer `1`, `ADOPT_NATIVE_V23_V24` |
| `host_machine_sha256`, `observed_at`, `inventory_sha256` | Machine binding, integer Unix time within one hour, operator-reviewed inventory digest; the digest is an audit binding, not a self-authenticating inventory |
| `baseline` | Exact `source_sha`, `configuration_sha256`, `platform`, `compatibility`, `images`; each service image has exactly `ref`, `config_digest`; actual source registry and pulled labels/configs must match |
| `next` | Normal complete OCI release pins: `manifest`, `manifest_digest`, `source_sha`, `configuration_digest`, `platform`, `admission`, `admission_digest` |
| `databases` | Exactly Server `/root/cantos-native/server-db`, Identity `/root/cantos-native/identity-db`; no relocation |
| `socket_directory` | Exactly `/root/cantos-native/run`, Identity 10002:10001, mode 2750 |
| `units` | Exact SHA-256 per service for `/etc/containers/systemd/<service>.container`; drop-ins/foreign effective units refuse |
| `configuration_files` | Complete bounded path-to-SHA-256 mapping; root-owned non-symlink files in `/root/cantos-native/config/<filename>` or `/root/cantos-native/<filename>.env`; all effective EnvironmentFiles must be bound |
| `mounts` | Complete per-service list of `source`, `destination`, `read_only`; only that service's DB, the socket, bound configs and Server's exact `/root/cantos-native/storage` root are accepted |
| `health_url` | Loopback HTTP URL as before, or the exact container-bound bridge selector below; no operator-selected address, redirect or ambient proxy |
| `peer_policy` | Explicit existing `direct_peer` or `trusted_proxy`; actual container environment must match and the executor never rewrites it |
| `rehearsal` | `path`, `sha256` of PASS historical evidence for these exact old/new image/source/config/platform identities; mock/current-schema records refuse |

The runtime adapter validates the actual host again: machine/platform, rootful cgroup v2,
generated-unit provenance, configuration bytes, exact container images/UIDs/commands/environment,
complete mounts and private socket/DB ownership. Other container DB binds/open descriptors or
remaining owner processes refuse. DB roots must be 0700 with service UID/GID 10001 or 10002;
regular engine files may be 0644 inside those protected directories, while `.owner` is 0600.
Only the DB, WAL, SHM and owner sidecars are accepted; links, nested mounts, foreign files/owners,
writable group/world bits and execute bits refuse. No chmod is applied to existing live files.

The path and proxy reconciliation is preservation with exact validation. It does not force-fit
the host to the current-schema fixture. A different effective layout needs another reviewed
profile. The ordinary `native-upgrade` path retains its canonical-path/direct-peer guards;
this initial adopted layout does not silently gain admission to that separate lane.

For an existing bridge-only Server, `health_url` may instead contain this descriptor. Values
below are synthetic; obtain the actual network name, 64-hex ID and canonical RFC1918 IPv4 subnet
from fresh operator-reviewed inventory. Port 8080 and this read-only route are fixed:

```json
{
  "kind": "podman-bridge",
  "network": "example-network",
  "network_id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "subnet": "10.42.0.0/24",
  "port": 8080,
  "path": "/api/v1/theatre/works"
}
```

The adapter requires exactly that one bridge network and subnet, no published host port, the
network's matching ID/gateway/prefix, and the running exact Server's non-gateway/non-broadcast
address. It derives the address anew after restart, never pins an ephemeral IP or accepts a URL
to an arbitrary host. It verifies container/process/network identity again after the bounded
HTTP 200. Readiness is also checked before PREPARE/pulls/stops. No port, ingress, DNS, interface,
firewall or peer policy is changed. The independent same-schema lane remains loopback-only.
Podman 4 reports the endpoint's `NetworkID` as its exact network name; newer releases report the
64-hex ID. Both representations still require the separate network inspection's real approved
ID and exact name, sole endpoint map, bridge driver and subnet. An arbitrary endpoint name or
an otherwise matching name whose real network ID changed refuses.

Podman may report Server `Args` either as `[]` or as the one executable already in `Path` and
`Config.Entrypoint`. Adoption accepts only those two metadata forms with an empty/null Cmd,
and the entrypoint itself may be the exact executable string or its one-element array (Podman
versions differ; no command string parsing or splitting is performed). It
then requires the actual running process argv to be exactly that one executable. Identity still
requires exactly its executable plus `serve`. Extra flags, shell wrappers, migration commands,
entrypoint/Cmd disagreement or a changed PID/container/start identity refuse.

## Pinned executor delivery without a host compiler change

`cargo xtask release native-executor-build --output FRESH_DIR` is a **disposable Linux amd64
builder** action. It requires the exact repository Rust pin, its musl target, musl-gcc/readelf,
and a pre-created writable canonical `/opt/cantos-native-executor` parent on that builder.
It builds only xtask from a clean local clone at `/opt/cantos-native-executor/FULL_SOURCE_SHA`.
It verifies a static amd64 ELF with no loader/shared dependencies, records compiler/source/hash,
and reaches the real Python adoption refusal from another cwd with no Rust/Cargo on PATH.
The manual OCI workflow retains `artifacts/native-executor/{xtask,executor.json}` and uses that
exact prebuilt binary for the historical/bridge rehearsal on disposable Linux.

For a separately authorized operator deployment, verify the trusted same-workflow artifact's
source and ZIP/binary hashes. Prepare the clean, root-owned exact Git checkout (including locally
present historical baseline objects) at the descriptor's **exact** `required_checkout` path.
Validate HEAD and clean tracked/untracked state before using the verified binary. Existing
compile-time workspace discovery is deliberately preserved: placing the checkout at another
path or changing only cwd does not relocate the executor. Keep the compiler build tree and any
Git alternates on the builder; deliver neither to the host. The operator's checkout must be
self-contained. Keep request/admission/approval and evidence outside it.

Invoke the verified `xtask` binary directly with `release native-adopt ...`; do not prefix it
with `cargo run` or invoke rustup on the host. Runtime needs Python 3.11+, Git with the existing
no-lazy-fetch guard, rootful Podman/systemd and procfs, plus the unchanged private-input/admission
checks. Installed Rust/Cargo versions are irrelevant to this route. No toolchain installation,
compiler downgrade, privileged executor container or root-override flag is provided. A missing
or wrong checkout refuses; a successful build/rehearsal does not authorize live execution.

## Operator admission and maintenance

Default invocation is offline, with no host lock, pull, stop, migration or generation write:

```sh
cargo xtask release native-adopt --request PRIVATE_REQUEST --request-digest SHA256
```

For later live use, an operator must separately approve the actual target and downtime/write
fence, retain working old images/units/configuration/media and recovery resources, and create an
empty root-owned 0700 `/var/lib/cantos-release` directory through an authorized operator step.
Do **not** create `current.json` or declare NATIVE_READY to bypass adoption. Existing generation
or intent records refuse adoption. Keep ingress and external writers in maintenance through
business/auth checks. The tool stops owners but does not establish an ingress fence; the
maintenance declaration is an operator assertion, never probe-derived acceptance.

The pinned private adoption approval contains exactly:

```json
{
  "schema_version": 1,
  "operation": "ADOPT_NATIVE_V23_V24",
  "request_sha256": "EXACT_REQUEST_SHA256",
  "inventory_sha256": "REVIEWED_INVENTORY_SHA256",
  "rehearsal_sha256": "ACTUAL_HISTORICAL_REPORT_SHA256",
  "target_manifest_sha256": "ADMITTED_TARGET_MANIFEST_SHA256",
  "maintenance": "BOTH_NATIVE_WRITERS_FENCED",
  "recovery": "RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START"
}
```

This approval is additional to target producer admission. Execute only the clean source whose
SHA equals the admitted target; private request/approval/evidence files stay outside the source
tree. An operator can later append `--execute --approval PATH --approval-digest SHA256` to the
offline command **after separately authorizing that host write**. No execution is part of this PR.

## Cold pair, migration and generation order

Under the exclusive host release lock, the executor rechecks the observed baseline and saves
original units with ownership/modes, private configuration bytes/metadata and the exact request.
It durably writes `in-progress.json` **before** pulling either pair or stopping services.
Interruption/failure blocks replay. Both old and target images are digest/label checked.

After Server then Identity are stopped and no owner/file descriptors remain, the old Server
checks actual v23 history and the new Identity's non-migrating `check` verifies unchanged v4 and
its initialized security epoch. Both complete DB directories are cold copied and fsynced into
the retained private attempt. `backup.json` binds both file inventories, hashes, modes/owners,
baseline, request and one stopped-owner cut. Hashes are compared before/after copying; partial
backup cannot authorize migration. Capacity includes backup, rehearsal and recovery copies.

The executor restores both backups into fresh private roots, verifies the old pair and migrates
the copies with the target's real executables before the first active-root migration. It then
records `MIGRATE_INTENT`, migrates/checks each active root explicitly, rechecks original unit and
configuration bytes, and changes only each unit's exact Image directive. It does not change
mounts, secrets, peer policy, media or ingress.

Before starting Identity then Server it fsyncs `START_INTENT`. Exact runtime/socket/bound HTTP
health checks follow. Only then does it create generation **1**, referencing the admitted target,
observed baseline, request and preserved runtime layout; a receipt binds the cold backup.
There is no invented prior manifest/admission/generation. Live business acceptance remains
`NOT_RUN`; operators retain the maintenance fence until the separate auth/business checks pass.

## Failure and recovery

Never rerun a failed adoption automatically. Preserve the private marker, units, config, backups,
active data/media and all failed copies. Inspect the exact phase and reconcile any unknown pull,
stop, migration, start or receipt outcome before acting.

For `PREPARE` or `MIGRATE_INTENT`, with no current generation and the original exact request and
approval, explicit `--execute --recover-attempt attempt-HEX` may restore the baseline. It fences
units/configuration/host again and stops both owners. In `PREPARE`, active DBs were never migrated.
In `MIGRATE_INTENT`, it verifies **both** backup inventories/provenance before restoring either,
stages full copies in fresh sibling roots, retains failed active directories as
`<db-root>.failed-attempt-HEX`, and restores both canonical roots without deleting any data.
Named staging/retention paths permit reconciliation of a crash between pair renames; partial or
foreign staging refuses. It checks the baseline, restores exact unit bytes and durably records
`RECOVERY_START_INTENT` before starting the old pair. Success is `BASELINE_RESTORED_UNADOPTED`,
with generation `UNESTABLISHED`, never a manufactured trusted release.

At `START_INTENT`, `RECOVERY_START_INTENT`, an existing current generation, or an uncertain
start, this restore command refuses **before stopping or restoring anything**. A service may
already have accepted revocations, passwords or sessions even if health failed. Restoring an
older Identity/Server cut could revive authority or lose writes. Maintain/re-establish the
operator write fence, preserve both current roots and pursue reviewed forward recovery. A
later decision to discard accepted writes or rotate restored security epochs requires its own
explicit authority and procedure; this executor offers no bypass switch or image-only rollback.

The deterministic FakeHost/file tests exercise interrupted/repeated operations, partial live
migration, corrupt/foreign backup refusal, restore-rename interruption, drift, and unknown start.
They establish orchestration and real-file recovery semantics only. The historical Linux report,
approved private-copy rehearsal, actual host execution, business/auth acceptance, reboot and
off-host recovery retention remain independent evidence and operator gates.
