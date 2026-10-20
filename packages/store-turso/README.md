# cantos-store-turso

Single-owner embedded [Turso](https://github.com/tursodatabase/turso) foundation for the two native
Cantos databases: `server.db` (Cantos Server) and `identity.db` (Cantos Identity). It supplies what
both services would otherwise build twice and holds **no business table, query or rule**; those stay
in each service's contexts. See the [native Turso decision](../../docs/architecture/native-turso-refactor.md)
and [epic #331](https://github.com/loveoverflowcom/cantos/issues/331).

**Status: implemented library, no production consumer yet.** [#333](https://github.com/loveoverflowcom/cantos/issues/333)
(Server) and [#334](https://github.com/loveoverflowcom/cantos/issues/334) (Identity) are the intended
consumers; it enters `packages/` ahead of them because both need it before either can store
anything ([#332.4](https://github.com/loveoverflowcom/cantos/issues/332)). Nothing in `cantos-server`
or `cantos-identity` calls it yet, and PostgreSQL remains the Server's authority until the #333.9 flip.

## What it provides

| Owner | Guarantee | Evidence |
| --- | --- | --- |
| `OwnedDatabase::open` | One owner per file: a non-blocking advisory lock on a sidecar file, held by the open file description, so it ends with the process (even `kill -9`). The directory must exist, belong to the process user and grant nothing to group/others; a symlinked database path is refused. | file-backed + two-process tests |
| `registry` (pure) | A versioned, checksummed history per file with its own `application_id` (Server vs Identity) and epoch. `plan_startup` decides **before any write**: create an absent/empty file, apply pending migrations (`Intent::Migrate`), or serve a file that is exactly current (`Intent::Serve`). A foreign, unmarked-but-populated, other-epoch, newer-than-binary or diverged file is refused, never adopted. | table-driven unit tests + file-backed refusals |
| creation | Header marker, history table and every migration in **one** transaction; a failed migration leaves an empty unmarked file that can be created again. Upgrades commit one migration per transaction. | file-backed |
| `begin_write` | The only mutation entry: `BEGIN IMMEDIATE` on a serialized writer. The trusted [`Clock`](src/clock.rs) is read **after** the writer is held, so timestamps follow commit order. A dropped or cancelled transaction is rolled back before the next one begins. | interleaving + mutation-checked tests |
| `commit` | `Rejected` (definitely not committed; rolled back) versus `Unknown` (I/O, corruption, interruption: reconcile through the operation receipt before any retry). | unit + file-backed |
| `reader` | A new connection with foreign keys on and `query_only`; it never sees an uncommitted write. | file-backed |
| `backup_to` | Online `VACUUM INTO` on the writer, then integrity check, current-file check, row-count manifest equality with the live file, fsync and a SHA-256 receipt. Never overwrites. | file-backed under concurrent writes |
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
- `VACUUM INTO` on a connection that did not make the newest commits omits them, and on a connection that predates a schema change it fails after creating the output file. `backup_to` refreshes the connection first and removes a destination it created when anything fails.
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
