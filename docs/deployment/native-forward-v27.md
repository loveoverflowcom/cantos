# Proposed forward reconciliation and v24 → v27 native migration

Status: candidate implementation and local fake-host/file tests only. No live operation,
production readiness, operator approval or exact-executor rehearsal is implied. The existing
[v23 → v24 adoption](native-existing-host-migration.md) and [OCI qualification](native-oci-release.md)
remain separate authorities. This lane is intentionally limited to the already adopted layout and
the v24 → v27 Server suffix; it does not change the Identity schema, RPC, security or network.

The adoption was interrupted after `START_INTENT`, so its old cold backup cannot be restored:
either service may have accepted writes. Forward reconciliation requires the original exact
intent, request, adoption approval, sealed original backup, and retained prior forward intent,
receipt and unit-alignment record. It proves the retained unit was changed only by the previously
recorded Container image and one pre-start image replacement. It reobserves the qualified running
pair, process, mounts, peer policy, unit/configuration hashes and existing bridge. It then stops
both owners under an operator-provided ingress/write fence, checks and cold-copies both **current**
database directories, verifies restored copies and checks current histories. After writing a new
start intent, it restarts and reobserves the same pair. A separate truthful receipt and generation
are written before the original journal marker is removed; a byte-identical copy of the original
marker remains in the new attempt. It never restores the old pre-start database.

If a crash occurs after both the new receipt and generation are durable but before removing the
old marker, a replay of the same approved request verifies custody, backup, exact running pair
and the unchanged marker before completing marker removal. Other interrupted states may be resumed only with the exact retained prior forward attempt, a
newly pinned approval and fresh inventory. The executor stops any matching old/target owners,
captures the latest current pair and verifies it with the target binaries; it never restores the
older snapshot. A mismatched attempt, foreign process, partial or corrupt data, stale approval or
unknown state refuses without declaring success.

The v24 → v27 lane requires that forward generation, complete original and current-backup custody,
an independently admitted target, a separately reviewed real executor rehearsal, and an exact historical rehearsal whose baseline **image pair**
equals the actual running v24 pair and whose target equals the admitted v27 pair. A different v24
pair is not equivalent evidence. The rehearsal must also contain the real Podman bridge-adapter
checks. The lane snapshots both current databases, verifies isolated restored copies, migrates the
copies with the target pair, checks current histories, and only then migrates the stopped live pair.
It updates exactly each service's Container `Image=` and its single recorded `ExecStartPre` digest.
Before starting either new owner it records `START_INTENT`. It verifies the started pair and writes
a separate receipt before advancing the generation. No health check establishes business, login,
mail or browser acceptance.

A separate, pinned recovery approval is required to restore the v24 cold pair before start. After
`MIGRATE_INTENT`, the owners must already be inactive and no database files may be held open; an
unexpectedly active owner is a refusal before any restore. The complete backup pair is verified
before either rename, failed live directories are retained, and an interruption between renames can
resume with the same reviewed approval. The recovery writes `ROLLBACK_START_INTENT` before restarting
the old pair. **Both** `START_INTENT` and `ROLLBACK_START_INTENT` prohibit any old snapshot restore:
accepted writes may exist. For migration `START_INTENT`, the separate post-start lane may stop the
observed old/target owners under a fresh approval and fence, copy the latest pair, verify restored
copies using the target binaries, and start only the exact target. It never restores the v24
snapshot. If the completed migration receipt/current pair already exists, it reobserves the target
and may complete only removal of the unchanged marker. A failure leaves the marker and all copies.
`ROLLBACK_START_INTENT` remains a separate, unsupported post-start recovery obligation.

## Operator inputs and boundaries

Both commands default to an offline/read-only plan and require an exact clean executor source.
Each requires a separate, source- and image-pair-bound executor rehearsal record with Linux,
systemd and real Podman/bridge evidence and explicit crash/replay controls. Its producer and
actual execution are not yet implemented; a model, file or fake-host result cannot satisfy it.
They accept private, canonical, owned, hash-pinned JSON files; these are not public PR artifacts.
The existing historical manifest, admission and four exact-image records remain independent of
new operator approval. An operator must review the actual host inventory, maintenance/ingress
fence, exact request, matching qualification/rehearsal, free capacity, backups and recovery plan.
A hash or a self-declared approval is not a signature or a substitute for that review.

```sh
cargo xtask release native-forward --request PRIVATE_REQUEST --request-digest SHA256 \
  --inventory PRIVATE_INVENTORY --inventory-digest SHA256 --executor-source FULL_SHA
cargo xtask release native-migrate-v27 --request PRIVATE_REQUEST --request-digest SHA256 \
  --inventory PRIVATE_INVENTORY --inventory-digest SHA256 --executor-source FULL_SHA
```

A separately reviewed and hash-pinned `--approval PRIVATE_APPROVAL --approval-digest SHA256`
and explicit `--execute` are required for any mutation. `--recover-before-start` selects the
separately approved pre-start recovery; `--reconcile-after-start` selects the separately approved
forward-only reconciliation. Both require `--execute`; the main migration approval never
authorizes a post-start restore. Do not run either command on a host before exact-source
integration, real Linux/bridge execution and failure-boundary rehearsal, human review and a
specific maintenance/execution decision. Neither command connects via SSH, creates credentials,
changes networking or publishes Web/APK artifacts.

The forward request contains exactly `schema_version`, `operation`, `observed_at`, `attempt`,
`marker_sha256`, `request_sha256`, `prior_intent_sha256`, `prior_receipt_sha256`,
`prior_alignment_sha256`, `inventory_sha256`, the two corrected `units` hashes,
`executor_source` and `executor_rehearsal` path/hash. The forward
approval binds its request, inventory and current manifest, plus `BOTH_NATIVE_WRITERS_FENCED`
and `PRESERVE_POST_START_WRITES`. A resume additionally binds `prior_attempt`, the original
request and marker, fresh inventory, manifest and a fresh separate approval. Its recorded
`--resume-attempt`, `--resume-approval`, `--resume-approval-digest`, `--resume-inventory` and
`--resume-inventory-digest` are required together with `--execute`.

The migration request contains exactly `schema_version`, `operation`, `observed_at`,
`inventory_sha256`, `current_sha256`, `generation`, `current_manifest_sha256`,
`forward_request_sha256`, the two `units` hashes, the complete pinned `next` release, a
`rehearsal` path/hash and the separately pinned `executor_source` and `executor_rehearsal`. Its approval binds the request, inventory, rehearsal and target manifest,
plus `BOTH_NATIVE_WRITERS_FENCED` and `RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START`. The separate
recovery approval also binds the exact interrupted attempt, marker, prior generation, backup and
fresh recovery inventory, and explicitly asserts `RESTORE_ONLY_BEFORE_ANY_START`. Post-start
approval binds the original request and marker, previous generation, target manifest and fresh
inventory, and asserts `PRESERVE_POST_START_WRITES`.

Local file/host-double tests exercise refusals and failures; they do not establish actual engine,
Linux/Podman/systemd, crash durability, network isolation, production recovery or device behavior.
The historical workflow rehearses two independently pinned public v24 pairs separately; neither
is an executor rehearsal. The applicable pair and final source must match exactly. A separate
real systemd/Podman rehearsal of these transition owners and its evidence producer are still required.
The workflow also runs a read-only disposable-runner prerequisite probe for systemd, cgroup v2,
Quadlet and reserved fixture names. Its `PREREQUISITES_ONLY` or `NOT_RUN` result does not run
the executor and cannot satisfy the executor-rehearsal gate.
On a runner that passes those prerequisites, a separate synthetic fixture may start and stop
the exact target pair through real Quadlet units and inspect its private bridge and owner
processes. This is reported as `PARTIAL_SYSTEMD_FIXTURE_PASS` or `FAIL`; transition execution,
crash/replay, operator admission and activation remain `NOT_RUN`. It is not the missing
executor-rehearsal producer. The fixture refuses known existing paths and services and runs
only on a disposable GitHub-hosted VM, never on the VPS.
