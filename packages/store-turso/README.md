# cantos-store-turso

Reusable single-owner embedded [Turso](https://github.com/tursodatabase/turso) infrastructure.
Consumers supply a nonzero signed `application_id`, schema epoch and ordered migrations. This
crate knows no service database identities, schemas or business rules and imports no apps or
services. It remains an internal Cantos library with a deliberately fixed, compatible storage
format. See the [native Turso decision](../../docs/architecture/native-turso-refactor.md).

The concrete consumers are [`cantos-server`](../../services/cantos-server/src/platform/turso/mod.rs)
and [`cantos-identity`](../../services/cantos-identity/src/persistence/mod.rs). Each owns its marker,
schema registry, migration SQL, row validation and business errors. Server's `ServerDatabase`
wrapper prevents an arbitrary registry from reaching its adapters; Identity opens only its own
registry. Their runtime and acceptance boundaries remain in the service guides.

## Consumer boundary and API

Previously `Registry::new(DatabaseKind::Server, epoch, migrations)` selected one of two service
identities inside this crate. It is now `Registry::new(application_id, epoch, migrations)`;
`Registry::application_id()` replaces `kind().application_id()`. Zero is rejected as
`RegistryError::UnmarkedApplicationId`, because it denotes an unmarked file. All other signed
32-bit markers are accepted; allocating a unique stable marker belongs to the consumer.

```rust
use std::sync::Arc;
use cantos_store_turso::{
    clock::SystemClock,
    registry::{Intent, Migration, Registry},
    OpenRequest, OwnedDatabase,
};

const LEDGER_APPLICATION_ID: i32 = 0x4C45_4447;
const MIGRATIONS: [Migration; 1] = [Migration {
    version: 1,
    name: "create_entries",
    sql: "CREATE TABLE entries(id INTEGER PRIMARY KEY, payload BLOB NOT NULL) STRICT;",
}];

async fn open() -> Result<OwnedDatabase, Box<dyn std::error::Error>> {
    let registry = Registry::new(LEDGER_APPLICATION_ID, 1, &MIGRATIONS)?;
    // The private parent directory must already exist; serving never migrates.
    Ok(OwnedDatabase::open(OpenRequest::new(
        "private/ledger.db", registry, Intent::Serve, Arc::new(SystemClock),
    )).await?)
}
```

`OpenRequest`, owner/read/write/backup APIs, clock/value conversions and error classification
stay shared. Service statement facades and row decoders stay local: Server's cardinality/column
errors and Identity's security-record validation have different contracts. Extracting them here
would introduce business coupling rather than remove duplication. No new dependencies, ORM,
pool, configuration framework or performance optimization is introduced.

## Persisted compatibility

Server retains `CNTS` (`0x434E5453`); Identity retains `CNTI` (`0x434E5449`). Both retain epoch 1,
their exact registered SQL, version/name order and checksums. This API refactor requires no data
migration. The history table/guards (`cantos_schema_history`), checksum domain and framing
(`cantos-turso-migration/1\0`), guard prefix (`cantos:`), time representation and lock sidecars
remain fixed infrastructure conventions. They are not service identities. Making those
configurable would expand format/error compatibility work without a current consumer need;
any future format change needs a separate migration design.

A marked file with an empty history is corrupt/diverged, including under `Intent::Migrate`:
creation writes the marker and first history row atomically, so migration must never rebuild a
missing registry. Native history refusal tests exercise both intents without altering file bytes.

## What it provides

| Owner | Guarantee | Evidence |
| --- | --- | --- |
| `OwnedDatabase::open` | One owner per file: a non-blocking advisory lock on a sidecar file, retained by every reader and query cursor until its engine/file handles close, so it ends with the process (even `kill -9`). The directory must exist, belong to the process user and grant nothing to group/others; a symlinked database path is refused. | file-backed + two-process tests, including escaped reader/writer cursors |
| `registry` (pure) | A versioned, checksummed history per file with a consumer-supplied `application_id` and epoch. `plan_startup` decides **before any write**: create an absent/empty file, apply pending migrations (`Intent::Migrate`), or serve a file that is exactly current (`Intent::Serve`). A foreign, unmarked-but-populated, other-epoch, newer-than-binary or diverged file is refused, never adopted. | table-driven unit tests + file-backed refusals |
| creation | Header marker, history table and every migration in **one** transaction; a definitely rejected migration rolls back. `MigrationCommitUnknown` preserves an uncertain COMMIT and requires the next startup to reconcile header/history before retry. Upgrades commit one migration per transaction. | file-backed, including fault injection after a real COMMIT |
| `begin_write` | The only mutation entry: `BEGIN IMMEDIATE` on a serialized writer. The trusted [`Clock`](src/clock.rs) is read **after** the writer is held, so timestamps follow commit order. A dropped or cancelled transaction is rolled back before the next one begins. | interleaving + mutation-checked tests |
| `commit` | `Rejected` (definitely not committed; rolled back) versus `Unknown` (I/O, corruption, interruption: reconcile through the operation receipt before any retry). | unit + file-backed |
| `reader` | A new connection with foreign keys on and `query_only`; it never sees an uncommitted write. | file-backed |
| `backup_to` | Atomically reserve `<destination>.backup`, copy into its private staging file on the writer, then integrity/current-file/row-count checks, fsync and SHA-256. A hard link publishes without overwriting; cleanup touches only this attempt's staging. | file-backed under concurrent writes, controlled same-destination overlap and foreign destination publication |
| `capability` | Probes of the pinned engine, each with a recorded expected outcome. | [report](../../docs/evidence/turso-capability-report.md) |

## Conventions every schema shares

- **Time:** microseconds since the Unix epoch (UTC) in `INTEGER` ([`UnixMicros`](src/value.rs)); SQL never reads the wall clock.
- **Unsigned numbers:** `u64` is stored only when it is at most `i64::MAX` (`u64_to_integer` / `integer_to_u64`); a larger value is rejected by the typed constructor, never wrapped.
- **Guards:** a trigger rejects with `RAISE(ABORT, 'cantos:<code>')`; the prefix makes it a typed `StoreFault::Guard`. Any other trigger message is an opaque `Engine` fault, not a domain conflict.
- **Regular expressions:** write `CHECK (coalesce(col REGEXP 'pattern', 0))`. A pattern that does not compile makes the plain form evaluate to NULL, which `CHECK` accepts, silently disabling the constraint.
- **Deferrable uniqueness and statement-level triggers do not exist.** Reorder a `UNIQUE` position in two phases inside one transaction; use row-level triggers or a guard in the writing transaction.
- **Foreign keys are off per connection by default.** `OwnedDatabase` turns them on for every connection it hands out and verifies it.

## Hazards the crate absorbs (so adapters do not rediscover them)

- A rejected statement does **not** end a transaction; an adapter must roll back on any rejected step.
- A write on the same connection while a `RETURNING` cursor is open is `Busy`; drain or drop `Rows` first.
- `VACUUM INTO` on a connection that did not make the newest commits omits them, and on a connection that predates a schema change it fails after creating the output file. `backup_to` refreshes first and cleans only its reserved staging on failure/cancellation. It never deletes the final destination. A crash can leave `<destination>.backup`; reconcile that reservation explicitly before reuse. A failure after publication (for example directory fsync) can leave a verified destination without a returned receipt; inspect/reconcile it rather than overwrite it.
- Setup/query/I/O failure is `ERROR`, always unexpected. It cannot satisfy an expected `FAIL` hazard. The stale-schema hazard requires both the exact pinned-engine refusal and a newly left output file; removing that file changes the verdict and forces review.
- A dropped `WriteTransaction` leaves the engine transaction open until the next `begin_write`.

## Not in this crate

No schema, ownership map, adapter, session or credential logic; no `instance_epoch` (Identity, #334);
no restore command (stop the owner, place a verified backup, start with `Intent::Serve`); no
connection pool (one writer, readers on demand); no MVCC or multi-process mode, both off by default
in the engine; no Turso Cloud, `libsql`, sync or server.

## Dependency pin

`turso =0.8.3` with `default-features = false`. The default `mimalloc` feature installs a
process-wide `#[global_allocator]` from inside the library, and `fts` pulls the tantivy search stack;
neither is wanted. Re-run the probes, the backup test and `cargo xtask verify turso` for every pin
change. Review record: [capability report](../../docs/evidence/turso-capability-report.md#dependency-qualification).

## Verify

```bash
cargo xtask verify turso
```

Runs clippy, every test and prints the capability report. It starts no server, container or cloud
service. To probe a specific disk, run `cargo run -p cantos-store-turso --features testing --bin
cantos-turso-capability -- report DIR` with an existing private directory on that filesystem.
The `testing` feature (run-owned temporary directories, a deterministic clock) is for tests and the
report binary only; a runtime build never enables it.
