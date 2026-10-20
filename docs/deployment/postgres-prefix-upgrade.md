# Explicit epoch-2 prefix upgrade and paired recovery

This opt-in lane upgrades the exact epoch-2 registry `[1]` to `[1,2]` without
migrating the live database in place. It is source implementation, not production
acceptance. Ordinary deploy/rollback retain their strict complete-schema checks.
The [schema cutover](postgres-schema-cutover.md) remains the separate epoch-1 owner;
its narrow `--cutover-epoch2` must not be used for this operation.

## Authority and prerequisites

The operator must approve the exact source/image, host and maintenance window,
including pausing the shared edge ingress, stopping both API writers and retaining sibling databases. The command
neither creates identity accounts/credentials nor reconciles OAuth clients, changes
DNS, publishes Web, provisions storage or deletes a database/volume. No `--force` or
registry-edit path exists. The runtime still requires explicit origin/callback and
drop-in validation from the [VPS runbook](vps-production.md).

Use a native **Linux x86_64** build machine, the pinned Rust toolchain, Podman,
`git`, `tar`, `zstd`, `ssh`, `scp`, `sha256sum` and Python 3. macOS may print the plan,
but real execution returns `UNSUPPORTED` before remote work. A Mac ARM image or
emulated build is not a qualified production image. Select an authorized builder
and its cost/runtime limit separately; this command provisions no builder or CI.

The VPS must already run PostgreSQL **17.11** and both healthy API units from the
same recorded previous image. The old image must declare epoch 2 and pass the
read-only schema check on `[1]`. The candidate image's independently initialized
oracle must have exactly the two checksums in
[`epoch2-prefix-registry.tsv`](../../infra/vps/postgres/epoch2-prefix-registry.tsv).
Changing a migration or adding a third requires a separately reviewed protocol. Version 3
(`0003_listener_bookmarks.sql`, #299) is **not** part of this lane: an image that carries it has a
three-checksum oracle, so the lane refuses it. Ordinary deploy does not apply it either: its
`preflight_schema` runs the strict `cantos-migrate --check`, which refuses a `[1,2]` database for an
image that carries three migrations before any backup or migration. **No reviewed rollout path for
version 3 exists yet**; it needs its own operator-owned protocol (see
[vps-production](vps-production.md)). Do not bypass the guard or edit registry rows.

Before starting, qualify live browser/callback configuration independently. The
lane refuses changes to non-API Quadlets, systemd units or Caddy, and database-level
role settings. Global/systemd overrides, unexpected schemas, extensions, large
objects, prepared transactions, replication slots, RLS, foreign ownership and
privileged runtime roles are refused. It checks space on the recovery and database
filesystems for eight database sizes plus a 1 GiB reserve, in addition to the
already loaded images. This bound is conservative, not a capacity prediction;
inventory actual WAL, images, concurrent use and quota before choosing the window.

Private media bytes and identity/secret/CA recovery remain independent. **`pg_dump`
does not back up filesystem or object-store audio.** Obtain and verify the owning
media backup/restore evidence before a real cutover. This lane does not establish
#309/#179 media durability or #170 publication acceptance. Reconcile overlapping
VPS changes such as #311 and its #313–#315 packaging/preparation/recovery reviews
before combining them; do not treat this lane as fixing those separate defects.

## Plan and execution

```bash
# Pure plan; permitted on Mac. Starts no runtime, SSH or build.
cargo xtask deploy vps cantos-api --host root@HOST --upgrade-epoch2-prefix --dry-run

# Explicit remote write; approved Linux x86_64 builder and maintenance window only.
cargo xtask deploy vps cantos-api --host root@HOST --upgrade-epoch2-prefix \
  --smoke-origin https://api.example.org
```

The flag conflicts with bootstrap, reload-browser, rollback, status and epoch-1
cutover. Artifact-promotion flags are refused: this lane builds committed `HEAD`
and its matching host bundle, not a mixed artifact/runtime revision. It uses the
existing SSH agent/config and never reads, prints, installs or uploads a key.

1. Generate/persist a fresh 32-hex attempt ID, including file and ancestor-directory
   synchronization. Archive committed source; build/export/compress locally before
   upload. Each remote path is `/root/cantos-services/incoming/SHA/ATTEMPT/tree`.
2. Under the operation lock, validate prerequisites and snapshot the old runtime,
   installed helper, configuration and exact managed system paths. Persist the
   journal under `state/epoch2-prefix-upgrades/SHA12-ATTEMPT/`. A pending wrapper
   blocks old installed helpers and the backup timer from mutating a prepared host.
3. Initialize an old-image baseline oracle, restore it independently and create a
   restored target oracle. Verify both exact registries and schema. The canonical
   source must match the fresh old schema; restored copies match the restored oracle
   without rewriting/normalizing database DDL. No general data transformer runs.
4. Persist freeze intent; pause the owned edge (including its identity/legacy Web
   ingress), stop both API units, set the source connection limit to
   zero and refuse remaining clients. Unknown clients are never terminated. Take
   the final custom-format dump and restore it into the new owned sibling. Check it
   with the old image and compare every baseline table and identity sequence.
5. Run the actual candidate migrator on that sibling only. Strictly check the
   expanded registry/schema and compare all baseline row and sequence fingerprints
   again. Version 1 remains unchanged; the migrator earns version 2 transactionally.
   Row bytes flow only through a private SHA-256 pipe, never ordinary command output.
6. Keep both APIs stopped and transfer the recovery archive/manifest off-host.
   Verify SHA-256 and synchronize file/directory entries before acknowledging the
   exact checksum. The commit phase refuses restarted edge ingress, an absent/mismatched acknowledgement,
   changed OIDs, schema drift or source/candidate row drift after the copy.
   Recheck temporarily admits one migration-owner connection while `PUBLIC` and
   the runtime role lack `CONNECT`, then refreezes the sibling before installation.
   The runtime's connection grant returns only after the canonical rename.
7. Persist install/rename intent, install the matched runtime, set the candidate
   app's default transactions read-only, then rename by the recorded identities:
   original `cantos` → retained name; staged sibling → `cantos`. Apply runtime grants
   and health-check both APIs privately inside their containers while app transactions
   remain read-only and edge ingress stays stopped. Public mutation/identity requests
   must not reach this health window; read-only SQL alone would not prevent external
   side effects.
   Known reviewed API code must respect the read-only default; it is not protection
   against an arbitrary SQL client overriding transaction mode.
8. Record `state/current`, remove the incompatible ordinary `state/previous`, and
   durably record **accepted before enabling writes**. Reset read-only settings and
   restart both APIs, then reopen/health-check the edge; mark complete and remove the
   pending pointer. An edge failure at this point is after acceptance and can only
   finalize forward. Retain the old
   database/image, all sibling oracles, runtime snapshots, journal and off-host copy.

The source database is never migrated, reset or dropped. All sibling names include
the full attempt, and recovery validates full target revision, attempt and recorded
database OIDs. Any retained prefix-journal directory conservatively disables all
automatic release pruning; retirement requires a separate retention decision.

## Failure, retry and recovery

A failed prepare/commit retains its original `FAIL`. Once prepare has been attempted,
xtask invokes state-aware recovery from the **same incoming source bundle**. If the
connection is interrupted, do not start a fresh attempt: inspect status and the
private attempt/journal first. Exit 0 or a build alone is not deployment acceptance.

```text
# Operator recovery shape; substitute the exact recorded values, never "latest".
/root/cantos-services/incoming/SHA/ATTEMPT/tree/infra/vps/bin/cantos-vps \
  prefix-recover SHA /root/cantos-services/incoming/SHA/ATTEMPT/tree ATTEMPT
```

Before `accepted`, recovery restores original DB selection and old image/runtime
as a pair, restores the connection limit and restarts the old APIs. A crash between
renames is resolved by OID, not assumed names. Failed early preparation restores
only its helper wrapper and does not stop healthy source APIs. Unknown journal,
clients or identities retain the freeze/pending pointer for operator investigation.

At/after `accepted`, recovery **only finalizes the new installation**. New writes
may exist, so returning to the retained database could lose every write after the
freeze point, including old-table changes and Bearer state. Hold writes and use
forward repair or separately reviewed reconciliation; never remove registry rows,
drop Bearer tables or automatically restore the old snapshot. Ordinary image-only
rollback to the original binary is incompatible with registry `[1,2]`.

Both API surfaces, identity ingress and legacy edge-hosted Web aliases are unavailable
during final freeze, copy, restore, migration, switch and health checks. Independently
hosted static Web remains available with API-dependent actions unavailable. Choose an explicit abort/recovery threshold within the approved window
after the exact environment rehearsal. Host/storage failure may exceed it; retain
the freeze rather than risk data loss. OIDC flows may expire and require retry.

The private recovery tar includes DB/config/runtime snapshots, not OCI image bytes,
Podman secrets, identity data, private CA or media. Retain those recovery dependencies
separately. The old image staying on the same VPS is not off-host disaster recovery.

## Verification and manual handoff

Synthetic boundary tests cover preparation order, immutable attempt selection,
off-host checksums, partial install, rename interruption, health failure, retained
data, pending locks/pruning and the accepted-before-write boundary. The existing
`cargo xtask verify postgres --suite schema` includes real disposable row/sequence
fingerprint negative controls; it does not connect to production or prove the full
host cutover protocol.

Before real use, complete a Linux x86_64/PostgreSQL 17.11 dress rehearsal with the
actual OCI images, systemd/Quadlet, final backup/off-host fsync, crash/recovery,
origin/callback/grant and API health checks. Native PostgreSQL 17.4 results on Mac
are scoped local evidence, not 17.11 or production acceptance. Full PostgreSQL,
browser/auth, media restoration and exact deployed SHA/image readback remain
separate acceptance. A Draft PR, merged source or successful build performs none
of these operator steps.
