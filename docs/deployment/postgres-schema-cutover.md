# PostgreSQL schema epoch 2 and controlled cutover

**Implemented source boundary; live cutover not executed.** Issues
[#238](https://github.com/loveoverflowcom/cantos/issues/238) and
[#239](https://github.com/loveoverflowcom/cantos/issues/239) remove retired API TTS execution
and establish one current fresh-install schema. They do not authorize replacing production
data. [ADR 0018](../decisions/0018-external-audio-production.md) preserves external production;
[ADR 0039](../decisions/0039-vps-production-runtime.md) owns VPS runtime/durability;
[ADR 0041](../decisions/0041-direct-hosting-api-boundary.md) owns direct Hosting/API addressing.

## Supported source and deployment boundary

The executable fresh baseline is
[`services/cantos-api/schema/0001_current.sql`](../../services/cantos-api/schema/0001_current.sql).
Its registry identifies **schema epoch 2**; migration version 1 and its checksum belong to that
epoch, rather than replacing the meaning of historical migration 0001. The current migrator
accepts a genuinely empty application database or a matching current registry. It refuses
unmarked populated databases, legacy migration registries, unknown epochs and mismatched
checksums. The API's startup check is read-only and requires an existing current schema.
`cantos-migrate --check` performs that same check without bootstrapping or changing a database.
See the [API guide](../../services/cantos-api/README.md) for commands and the retained legacy
manifest. No baseline is replayed onto a legacy installation.

The [VPS host script](../../infra/vps/bin/cantos-vps) additionally requires the image label
`io.cantos.schema-epoch=2`. Before an ordinary deploy takes a backup or calls the migrator, it
checks the target image against an existing database, or conservatively establishes that the
application database has no user schema, relation, routine or type. The migrator rechecks under
its database lock. The previous image must also declare epoch 2 and pass the current-schema
check before it can be an automatic rollback target. Explicit rollback checks compatibility
before switching images. These checks intentionally refuse an old image with a missing label.

Backups now carry a `.schema-epoch` sidecar. Their SHA-256 manifest must name exactly the dump
and that sidecar; a valid sidecar hash alone never authenticates the dump being restored.
Copy all three finalized files together for off-host recovery. Dump/checksum failure or unknown
schema metadata refuses success before retention/deployment, including when the caller captures
the backup path. An unknown-epoch archive is preserved for investigation, never called usable.
Only the finalized `.dump.sha256` manifest commits a usable backup for retention. A failed
checksum or unknown epoch leaves its checksum manifest `.partial`; later successful backups
exclude those diagnostic archives from the retention count, preserving known recovery points.
The current restore command requires epoch 2 metadata, a compatible current image and a successful
pre-restore schema check; it checks the restored schema again before grants or API restart.
A rejected post-restore check leaves both APIs stopped for operator recovery. Historical
backups without that metadata remain historical artifacts: preserve them with their matching
source/bundle and use a separately reviewed recovery procedure. Do not add metadata to make
an old backup appear current. A checksum and epoch label are integrity/compatibility aids,
not proof of a successful restore or of application-level data validity.

The source CI configuration declares separate PostgreSQL 17 and 18 verification lanes using
the official PGDG repository; this task does not dispatch or read CI. The exact installed minor
is printed by each job and must be recorded before accepting PostgreSQL 18.6.

The VPS and optional developer Podman runtime remain pinned to **PostgreSQL 17.11-bookworm**.
No current bundle changes their major version or attaches an existing volume to PostgreSQL 18.
On 2026-10-17 the official [version policy](https://www.postgresql.org/support/versioning/)
lists PostgreSQL 18.6 and 17.11 as supported current minors; PostgreSQL 19 RC1 is a prerelease.
The [18.6 release notes](https://www.postgresql.org/docs/release/18.6/) are an upgrade input,
not local integration evidence. Record the actual version used by every test run separately.

## Inventory gate — not yet performed on the VPS

The [historical 2026-10-17 deployment record](https://github.com/loveoverflowcom/cantos/blob/3e6c183d3d616e8ec4a782ba788b4b70d0c6e49a/docs/evidence/vps-production-deployment.md) observed
PostgreSQL 17.11 and 28 historical migrations at that time. It does not establish today's
rows, applied migration 0029, sessions, grants, extensions or available recovery media.
**Live inventory, backup restore validation, data transformation and production approval
remain pending.** Source inspection cannot establish that the deployed database is empty.

An operator must name the host/database and authorize read-only inventory before collecting
the following private evidence. Use an existing read-only PostgreSQL service entry/secret
file; do not place connection passwords in argv, chat or Git. Set `ON_ERROR_STOP=1`, enforce a
read-only transaction, use a bounded statement/lock timeout and save results outside Git.
Large exact counts need an operator-approved window; estimates alone do not settle data loss.

| Inventory | Required private evidence and decision |
| --- | --- |
| Database/runtime | Server/client versions, encoding/collation, extensions, exact container image/digest, volume mount/ownership, disk/free space, API image SHAs and active/previous runtime bundles |
| DDL and dependencies | Schema-only `pg_dump`, tables/views/materialized views/sequences, functions/triggers/indexes, FK graph and dependency catalog; include non-public schemas and extension objects |
| Ownership/grants | Database/schema/object owners, role memberships, table/column/function privileges, default privileges and security-definer function search paths; no role passwords |
| Migration identity | Every historical registry version/checksum/timestamp, missing/extra/mismatched entries, epoch marker if any; compare exact checksums with the retained manifest and Git source |
| Data | Exact row counts per table, current-authoritative versus retired classification, rows shared through FKs, immutable digest/reference reconciliation and orphan checks; no manuscripts in reports |
| Activity | Active transactions/prepared transactions/locks, API and external clients, scheduled systemd timers/cron tasks, replication slots/subscriptions and extension schedulers; redact raw SQL/query strings |
| Identity/session state | Active/revoked Studio/Theatre/native sessions, in-flight OIDC/enrollment/registration operations, account ownership/external-subject bindings, Admin revocation/audit requirements; never export credentials into chat |
| Media | Every private storage target/location ID referenced by submissions, renditions and artwork; byte count/hash and object existence checks through authorized private access, plus release-manifest bindings; never publish private bytes |
| Recovery | Full logical dump with SHA-256, globals/role definition backup kept secret, source revision and runtime bundle, storage/identity backups and off-host copy; completed restore drill with constraints, grants and API checks |

The following are **read-only query templates**, not commands executed by this change.
Run them only in the authorized database using the selected read-only connection:

```sql
\set ON_ERROR_STOP on
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout = '30s';
SET LOCAL lock_timeout = '2s';
SELECT current_database(), current_user, version();
SELECT extname, extversion FROM pg_extension ORDER BY extname;
SELECT n.nspname, c.relname, c.relkind, pg_get_userbyid(c.relowner) AS owner,
       pg_total_relation_size(c.oid) AS total_bytes
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname <> 'information_schema' AND n.nspname !~ '^pg_'
ORDER BY n.nspname, c.relname;
SELECT conrelid::regclass AS dependent, confrelid::regclass AS referenced,
       conname, pg_get_constraintdef(oid) AS definition
FROM pg_constraint WHERE contype = 'f' ORDER BY 1, 2, 3;
SELECT event_object_schema, event_object_table, trigger_name, action_statement
FROM information_schema.triggers ORDER BY 1, 2, 3;
SELECT schemaname, tablename, indexname, indexdef
FROM pg_indexes WHERE schemaname <> 'pg_catalog' ORDER BY 1, 2, 3;
SELECT grantee, table_schema, table_name, privilege_type
FROM information_schema.role_table_grants ORDER BY 1, 2, 3, 4;
SELECT pid, usename, application_name, state, xact_start, backend_start
FROM pg_stat_activity WHERE datname = current_database() ORDER BY pid;
SELECT gid, prepared, owner, database FROM pg_prepared_xacts;
SELECT slot_name, slot_type, database, active FROM pg_replication_slots;
-- Inspect registry columns first; query the matching historical/current shape explicitly.
SELECT table_schema, table_name, column_name, data_type
FROM information_schema.columns
WHERE table_name IN ('cantos_migrations', 'cantos_schema') ORDER BY 1, 2, ordinal_position;
COMMIT;
```

For exact counts in a separately budgeted read-only snapshot, `psql` can generate only quoted
count queries with no row payload:

```sql
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout = '60s';
SET LOCAL lock_timeout = '2s';
SELECT format('SELECT %L AS relation, count(*) AS rows FROM %I.%I;',
              n.nspname || '.' || c.relname, n.nspname, c.relname)
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind IN ('r', 'p') AND n.nspname <> 'information_schema'
  AND n.nspname !~ '^pg_' AND NOT c.relispartition
ORDER BY n.nspname, c.relname
\gexec
COMMIT;
```

Record every timed-out/unreadable relation as unknown. Count coverage does not replace DDL,
foreign-key, caller and media reconciliation. Explicitly inventory scheduler extensions if
present; `pg_stat_activity` does not discover offline clients or future scheduled work.

## Source classification and one-time import design

The current baseline and adapters are the source classification oracle. Inventory must resolve
every deployed table and column into an explicit import allowlist, archive-only set or unknown
set. Unknown objects or current-to-retired dependencies block cutover. A wildcard table prefix
is not an executable export/import allowlist.

| Data family | Default classification and required treatment |
| --- | --- |
| `actors`, identity bindings, memberships, current authorization/audit | Current/shared. Preserve IDs, ownership and revocation/audit meaning; include exact referenced parents even when retired rows also reference them |
| Editorial/source/Script IR and review evidence | Current. Preserve immutable bytes/digests, revisions, attribution and evidence links; resolve cyclic script/head FK import ordering |
| Caller-authored adaptation (`input_version='c1'`) | Current. Preserve proposals/submissions/cancellation/acceptance and source bindings; omit the retired `problem` column. Old `a1`, provider attempts/observations and dispatch deadlines are archive-only, not implicitly converted |
| Catalog, Content Identity, briefs, producer grants/revocations and planning rights | Current. Preserve exact pinned revisions, ownership and expiry/revocation; guidance metadata does not authorize provider execution |
| Private audio intake/lifecycle, artwork, publication and listener projections | Current. Preserve receipts, object locations/hashes, approval/release history and active pointers atomically; validate listener projection equivalence and private/public isolation |
| Library/progress/history/device sequences/idempotency operations | Current. Preserve revisions, order, epochs and replay meaning; never synthesize progress from device clocks |
| OIDC/native sessions, registration/enrollment flows and token-bearing rows | Current security state, with a separately approved preserve-or-revoke decision and pending-operation reconciliation; revocation/relogin is observable downtime behavior |
| `durable_*`, `speech_clips`, old DBSP change capture and provider adaptation `a1` | Retired runtime. Preserve complete historical backup/audit source; exclude from the new database. Check inbound/outbound FKs before any future explicit deletion |
| Historical `cantos_migrations` | Archive with exact old checksums. Never import its rows into the epoch 2 registry or rewrite checksums to pretend the old schema is current |

The one-time authoritative-data transformer is **not implemented** by this change. After live
inventory, review a concrete column-by-column mapping with immutable-field preservation,
explicit table order, cyclic/deferred-FK handling, constraints enabled, sequence restoration,
expected counts and digest checks. Do not globally disable triggers or use blind `CASCADE`.
Restore the full historical dump into an isolated PostgreSQL 17 database, create a separate
empty epoch 2 database, run the reviewed transformation there and compare authorization,
catalog/content/brief/audio/publication/library/auth results. A fresh schema test is not this
upgrade acceptance.

If the owner instead chooses reinitialization, the approval must name the exact database,
archived data/retention location, accepted losses and session/publication effects. That is a
destructive operator operation, not a `migrate`, build, Web deployment or verification option.
No current command automatically drops legacy tables or switches a live database to epoch 2.

## Separate PostgreSQL 17 → 18.6 decision

Keep schema conversion and server-major upgrade independently reviewable. The official
[upgrade guide](https://www.postgresql.org/docs/18/upgrading.html) requires dump/reload or
`pg_upgrade` across majors; replacing the image tag over a PostgreSQL 17 data directory is
not an upgrade plan. This source change retains the existing PostgreSQL 17 pins.

Before approving PostgreSQL 18.6, obtain an exact supported image tag/digest and platform,
review extensions/collations/release notes and run current baseline and all affected PostgreSQL
integration tests on 18.6. Provision a new isolated volume/cluster, preserve the old volume,
and rehearse logical dump/restore into it. The
[official image documentation](https://hub.docker.com/_/postgres) changes `PGDATA`/volume layout
for PostgreSQL 18; validate the chosen image's actual mount before use. A `pg_upgrade` alternative
needs its own reviewed `--check`, binaries/extensions/storage requirements and rollback drill.
Do not use link mode without an explicit decision accepting its rollback limitations.

The final operator plan must name downtime/write-freeze steps, disk headroom for both clusters
and backups, stopped clients/jobs, exact image and volume/socket mapping, role/grant restoration,
media integrity, post-import acceptance and a maximum rollback window. At switch time stop both
API writers and any other discovered clients, take the final consistent dump, verify its off-host
copy, rerun the rehearsed import and acceptance, then explicitly switch configuration/images.
There is no dual-write compatibility engine. Writes accepted after switch require a separately
reviewed reconciliation or accepted-loss decision before returning to the old database.

Rollback must restore the matching **image, runtime/grant bundle and database/volume** together.
Before switch, retaining the old installation permits returning to it unchanged. After new writes,
an image-only rollback is insufficient across epochs or PostgreSQL majors; retain both databases
and stop writes until the operator chooses the reviewed recovery path. Test this recovery in a
disposable environment before scheduling the production window. Do not delete old volumes,
backups, historical objects or media as part of this PR.

## Acceptance ledger and authority

| Layer | Evidence required before its claim is accepted |
| --- | --- |
| Source behavior | Focused host-command doubles for empty/current/legacy/refused image/rollback/restore boundaries; these are mock boundary checks, not VPS execution |
| Current database | `cargo xtask verify postgres --suite schema` for bootstrap/replay/epoch refusal, plus the full `cargo xtask verify postgres` for constraints, transactional routes and grants; record actual server version |
| PostgreSQL 18.6 | Full affected integration suite on an isolated 18.6 server; PostgreSQL 17 PASS does not accept this layer |
| Legacy data conversion | Completed inventory, reviewed allowlist/transformer, full backup/restore drill and current behavior/digest comparison on restored representative data |
| Production | Named operator, host/database, approved plan/window, off-host recovery evidence and explicit authority for the state-changing cutover/major upgrade |

This document records no live inventory, production backup verification, schema conversion,
PostgreSQL installation/upgrade, restore, data deletion or deployment result. Those actions
remain **NOT_RUN** until their exact scope is authorized and performed. The current code fails
closed at the legacy boundary so source cleanup can be reviewed without silently crossing it.
