# Explicit epoch-2 registry-suffix upgrade and paired recovery

This opt-in lane upgrades an exact epoch-2 registry prefix to the candidate image's
reviewed registry without migrating the live database in place: `[1]` → `[1,2]`
(Bearer) or `[1,2]` → `[1,2,3]` (release-bound bookmarks, #299; see
[version 3](#version-3-release-bound-listener-bookmarks-299)). It is source
implementation, not production acceptance. Ordinary deploy/rollback retain their strict
complete-schema checks: `preflight_schema` runs the strict `cantos-migrate --check` and
refuses any registry that is not the image's full set, **before** backup or migration. This
lane is therefore the only reviewed path that applies an additive migration to production.
The [schema cutover](postgres-schema-cutover.md) remains the separate epoch-1 owner;
its narrow `--cutover-epoch2` must not be used for this operation.

The lane is additive-only by construction, not by convention. The live registry must be an
exact, proper prefix of the reviewed registry; every baseline table row and identity
sequence must be byte-identical (SHA-256 fingerprints) after the new migrator runs; every
table the suffix adds must still be empty at acceptance; and the migrated sibling's schema
must equal an independently initialized target oracle. A migration that backfills, alters
or drops existing objects fails these checks and needs its own reviewed protocol.

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
strict read-only schema check on the live registry (`[1]` or `[1,2]`, whichever that
image carries). The candidate image's independently initialized oracle must have exactly
the checksums in
[`epoch2-prefix-registry.tsv`](../../infra/vps/postgres/epoch2-prefix-registry.tsv), and the
live registry must equal that file's leading rows. The file holds one `version|sha256`
row per `schema/NNNN_*.sql` migration, in order; a repository test binds it to those
files, so **adding a migration requires adding its row in the same change**. Editing an
applied migration or any registry row is never an option: a changed checksum is a different
registry and is refused. Each new migration still needs an explicit operator review that it
is additive; the lane's checks refuse it otherwise, but do not replace that review.

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
cargo xtask deploy vps cantos-server --host root@HOST --upgrade-epoch2-prefix --dry-run

# Explicit remote write; approved Linux x86_64 builder and maintenance window only.
cargo xtask deploy vps cantos-server --host root@HOST --upgrade-epoch2-prefix \
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
   restored target oracle. Verify both exact registries and schema. The live registry
   selects the source prefix and must equal that many leading rows of the reviewed
   registry (never equal to, longer than or diverging from it). The canonical
   source must match the fresh old schema; restored copies match the restored oracle
   without rewriting/normalizing database DDL. No general data transformer runs.
4. Persist freeze intent; pause the owned edge (including its identity/legacy Web
   ingress), stop both API units, set the source connection limit to
   zero and refuse remaining clients. Unknown clients are never terminated. Take
   the final custom-format dump and restore it into the new owned sibling. Check it
   with the old image and compare every baseline table and identity sequence.
5. Run the actual candidate migrator on that sibling only. Strictly check the
   expanded registry/schema and compare all baseline row and sequence fingerprints
   again. Existing versions remain unchanged; the migrator applies exactly the missing
   suffix in one transaction under its advisory lock and records each new checksum.
   Tables added by the suffix must be empty. Row bytes flow only through a private
   SHA-256 pipe, never ordinary command output.
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
rollback to the original binary is incompatible with the expanded registry: the previous
binary's strict check rejects any registry longer than its own, so `state/previous` is
removed and `cantos-vps rollback` has nothing to switch to.

Both API surfaces, identity ingress and legacy edge-hosted Web aliases are unavailable
during final freeze, copy, restore, migration, switch and health checks. Independently
hosted static Web remains available with API-dependent actions unavailable. Choose an explicit abort/recovery threshold within the approved window
after the exact environment rehearsal. Host/storage failure may exceed it; retain
the freeze rather than risk data loss. OIDC flows may expire and require retry.

The private recovery tar includes DB/config/runtime snapshots, not OCI image bytes,
Podman secrets, identity data, private CA or media. Retain those recovery dependencies
separately. The old image staying on the same VPS is not off-host disaster recovery.

## Version 3: release-bound listener bookmarks (#299)

`0003_listener_bookmarks.sql` adds the `listener_bookmarks` table, its guard function/triggers and
index; it touches no earlier table, row or checksum. It is deployable only through this lane,
with the previous image carrying `[1,2]` and the candidate carrying `[1,2,3]` (its registry file
must therefore have the third row). Do **not** run the migrator by hand, edit registry rows or
make ordinary deploy accept a shorter registry.

Operator protocol (every step is explicit authority; none is implied by a merged PR or green build):

1. **Authority.** Approve the exact candidate SHA/image, host, maintenance window and abort
   threshold in writing, including pausing the edge and stopping both API writers. Deployment
   authority does not grant identity, DNS, storage or media changes.
2. **Preconditions.** Read-only inventory shows registry `[1,2]` and both healthy APIs on the
   recorded previous image; the previous image passes `cantos-migrate --check`; the candidate
   registry file lists all three checksums; a Linux x86_64 / PostgreSQL 17.11 rehearsal with the
   real images has completed; verified media and off-host backup evidence exists.
3. **Plan then run.** `--dry-run`, then the real command on the approved builder. Prepare
   verifies the exact `[1,2]` source, builds the restored target oracle, freezes writers, takes the
   final dump, restores it into a sibling, runs the **candidate migrator on that sibling only**
   (applying exactly version 3 under the migrator lock) and proves every baseline row/sequence
   unchanged and `listener_bookmarks` empty.
4. **Off-host copy.** Transfer and checksum the recovery archive off-host before acknowledging.
5. **Commit.** After acknowledgement the lane installs the matching runtime, renames the
   databases by recorded OID, applies `runtime-grants.sql` (which grants the bookmark table to the
   runtime role) and health-checks both APIs privately with app writes read-only, then durably
   records `accepted` before enabling writes and reopening the edge.
6. **Acceptance outside the lane.** Run the documented read-only acceptance probes and the
   bookmark flow with a real listener only after `complete`. A green exit code is not acceptance.

Recovery boundary, stated plainly: **before `accepted`** the paired recovery restores the original
database, previous image and runtime and loses nothing (writes were frozen). **From `accepted`
on, image-only rollback is impossible**: the previous binary refuses `[1,2,3]`, and restoring the
retained `[1,2]` database would discard every write after the freeze. Hold writes and repair
forward, or use a separately reviewed reconciliation; never remove registry row 3, drop
`listener_bookmarks` or auto-restore the old snapshot. A new bookmark row is also a post-acceptance
write, so the deployment window must end only when the operator accepts that boundary.

## Verification and manual handoff

Synthetic boundary tests cover preparation order, immutable attempt selection,
off-host checksums, partial install, rename interruption, health failure, retained
data, pending locks/pruning, the accepted-before-write boundary, registry-prefix
selection and refusals (equal, longer, edited, malformed), added-table emptiness and the
retired image-only rollback. A local PostgreSQL 17.11 rehearsal exercised the real helper
functions with a real previous and candidate migrator (`[1,2]` → `[1,2,3]` on a restored
sibling: identical baseline fingerprints, empty added table, strict check refused by the
old binary). It uses no podman, systemd or production host and is scoped local evidence. The existing
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
