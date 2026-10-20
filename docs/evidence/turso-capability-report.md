# Turso engine capability report

Source-bound probe record for the embedded engine pinned by `packages/store-turso`
([#332.4](https://github.com/loveoverflowcom/cantos/issues/332), input to the #333.3 invariant mapping
table). **It is evidence about one engine pin on named hosts, not a schema, not an adapter and not a
production durability claim.** PostgreSQL remains the Server's authority; nothing here changes runtime.

| Field | Value |
| --- | --- |
| Engine | `turso =0.8.3`, `default-features = false` (SQLite compatibility version reported: 3.50.4) |
| Source | `refactor/333-server-turso-persistence` on integration `26d957b3` (20 October 2026) |
| Hosts run | Linux x86_64, filesystem `tmpfs` (default `$TMPDIR`) and `ext4` (`target/`), Rust 1.98.1 |
| Hosts **not** run | macOS arm64 (required by #332.4): `NOT_RUN`, no such host available in this session |
| Command | `cargo xtask verify turso` (clippy, tests, then this report); `cargo run -p cantos-store-turso --features testing --bin cantos-turso-capability -- report [DIR]` |
| Result | 48 probes: 42 PASS, 3 UNSUPPORTED, 3 FAIL; **0 differ from the recorded expectation** on both filesystems |

A probe reports `PASS` (the behavior the schema needs holds), `UNSUPPORTED` (the construct is absent;
a substitute is chosen) or `FAIL` (the engine silently weakens an invariant). The three `FAIL` rows
are *recorded hazards with a mitigation*, deliberately expected: if an engine upgrade removes one,
the probe turns unexpected and forces a review. The expected outcome of every probe lives next to
its code in [`capability.rs`](../../packages/store-turso/src/capability.rs).

## Findings that change the plan

| # | Finding | Consequence |
| --- | --- | --- |
| 1 | The default `mimalloc` feature makes the `turso` library install a process-wide `#[global_allocator]` (`turso-0.8.3/src/lib.rs`), and `fts` pulls tantivy. | Pin with `default-features = false`; an allocator is a binary-level decision (#333.11 footprint), not a side effect of a database dependency. |
| 2 | Recursive CTEs work (`sql.recursive_cte`). The planning notes listed them as absent. | No rewrite needed where the PostgreSQL SQL uses them. |
| 3 | An unknown collation is refused in queries, columns and indexes (`collation.unknown_is_loud`). The planning notes feared a silent fallback. | The guard test stays, as the contract to re-check on upgrade; no extra workaround. |
| 4 | `REGEXP` works in `CHECK`, but a pattern that does not compile evaluates to NULL and the plain `CHECK` accepts every row (`check.regexp_invalid_pattern`, expected FAIL). | Every regex `CHECK` is written `coalesce(col REGEXP 'p', 0)` (`check.regexp_coalesce_closed`), and each needs its own rejection test (already required by #333.3). |
| 5 | `UNIQUE … DEFERRABLE` and `FOR EACH STATEMENT` are syntax errors. | Two-phase reorder inside one transaction (`unique.reorder_two_phase`); the 57 statement triggers become row-level or a guard in the writing transaction. |
| 6 | `RAISE(ABORT, …)` arrives as a plain engine error, not a constraint. | Guard triggers use the `cantos:<code>` message prefix, mapped to a typed `StoreFault::Guard`. |
| 7 | Foreign keys are **off** on every new connection. | `OwnedDatabase` enables and verifies them per connection. |
| 8 | A rejected statement keeps the transaction open; a deferred-FK violation surfaces at `COMMIT` as a constraint with the transaction still open. | Adapters roll back on any rejected step; a failed constraint at commit is `Rejected` (definite), an I/O failure is `Unknown`. |
| 9 | `VACUUM INTO` copies the snapshot its own connection last saw: every one of 5 rounds omitted the newest commits of another connection (a longer manual 30-round run, not committed, did the same), and a connection that predates a schema change fails after creating the output file (`backup.stale_*`). | `backup_to` runs on the writer after a refreshing read, compares a row-count manifest with the live file and removes a destination it created on failure. A naive `VACUUM INTO` from any other connection would silently produce a stale backup. |
| 10 | `VACUUM INTO` needs no experimental flag; the flag only enables in-place `VACUUM`. | No experimental feature is enabled anywhere. |
| 11 | Disk exhaustion was **simulated** with a file-size limit (`EFBIG`), not `ENOSPC`: the commit failed as `Io(FileTooLarge)` (`Unknown`), 2 acknowledged commits survived and `integrity_check` was ok. | Evidence for the error path only; true full-volume behavior is `NOT_RUN`. |

## Probes

### Constraints

| Probe | Result | PostgreSQL need | Observed on this pin | Substitute / rule |
| --- | --- | --- | --- | --- |
| `check.octet_length` | PASS | CHECK on octet_length(text) (~130 uses) | counts bytes: 4 ASCII bytes accepted, 3 characters / 7 bytes rejected | — |
| `check.regexp` | PASS | CHECK with regular expressions (~100 `~`/`!~` uses) | anchors, classes and {m,n} are enforced by REGEXP in CHECK | — |
| `check.regexp_invalid_pattern` | FAIL | a broken pattern must not disable a CHECK | a CHECK whose pattern does not compile evaluates to NULL and accepts every row | write every regexp CHECK as coalesce(col REGEXP 'p', 0) |
| `check.regexp_coalesce_closed` | PASS | fail-closed regexp CHECK form | coalesce(a REGEXP 'p', 0) rejects every row when the pattern is broken, so a typo cannot disable the constraint unnoticed | — |
| `check.glob` | PASS | CHECK with identifier-shape patterns | GLOB with negated classes enforces ASCII identifier shapes | — |
| `check.nonnegative_integer` | PASS | CHECK on u64-range integers | non-negative integer domain enforced | — |
| `fk.enabled_per_connection` | PASS | foreign keys enforced for every connection | OFF by default and per connection: a dangling row is accepted until each connection enables it; OwnedDatabase enables and verifies it | — |
| `fk.actions` | PASS | ON DELETE RESTRICT / CASCADE / SET NULL | RESTRICT blocks, CASCADE deletes children, SET NULL clears the reference | — |
| `fk.deferred` | PASS | FK DEFERRABLE INITIALLY DEFERRED (head <-> revision) | the head<->revision cycle commits; a dangling reference fails at COMMIT as a constraint, the transaction stays open and must be rolled back (Rejected, not Unknown) | — |
| `unique.deferrable` | UNSUPPORTED | UNIQUE catalog_units(work_id, position) DEFERRABLE | refused at DDL: near "DEFERRABLE": syntax error | two-phase reorder inside one transaction (probe unique.reorder_two_phase) |
| `unique.reorder_two_phase` | PASS | atomic reorder of a UNIQUE position | a single-statement swap collides row by row; shifting out of range then renumbering inside one transaction reorders atomically | — |
| `index.partial_unique` | PASS | 5 partial indexes | a partial UNIQUE index constrains only the matching rows | — |
| `collation.unknown_is_loud` | PASS | no silent fallback for an unknown collation | an unknown collation is refused in queries, column definitions and indexes (no silent fallback to BINARY) | — |
| `collation.binary_is_byte_order` | PASS | COLLATE "C" ordering | BINARY orders by UTF-8 byte value, as PostgreSQL COLLATE "C" does | — |

### Triggers

| Probe | Result | PostgreSQL need | Observed on this pin | Substitute / rule |
| --- | --- | --- | --- | --- |
| `trigger.guard_update_delete` | PASS | 106 reject_settled_change triggers | row-level BEFORE UPDATE/DELETE with RAISE(ABORT) replaces reject_settled_change; the message arrives as a typed Guard | — |
| `trigger.when_and_case` | PASS | guards with WHEN / column conditions | WHEN clauses and CASE-wrapped RAISE express column-level immutability and conditional guards | — |
| `trigger.statement_atomic` | PASS | a rejected multi-row statement changes nothing | a guard aborting the third row leaves none of the three rows changed | — |
| `trigger.for_each_statement` | UNSUPPORTED | 57 FOR EACH STATEMENT triggers | refused at DDL: near "STATEMENT": syntax error | row-level triggers, or a guard checked inside the writing transaction |
| `trigger.recursive` | UNSUPPORTED | trigger re-entry | PRAGMA recursive_triggers returns nothing; a trigger must not rely on re-entry | no trigger may rely on recursion |

### Sql

| Probe | Result | PostgreSQL need | Observed on this pin | Substitute / rule |
| --- | --- | --- | --- | --- |
| `sql.upsert_and_ignore` | PASS | ON CONFLICT upsert and idempotent receipts | ON CONFLICT DO UPDATE / DO NOTHING; DO NOTHING reports 0 affected rows, so a replayed receipt is detectable | — |
| `sql.returning_and_open_cursor` | PASS | INSERT ... RETURNING | RETURNING works; a second write on the same connection while the cursor is open is Busy until it is drained | — |
| `sql.json` | PASS | jsonb_* / unnest replacements | json_extract, json_each and json_object cover jsonb_* / unnest uses | — |
| `sql.window_functions` | PASS | listener discovery windows | count(*) OVER (PARTITION BY) and row_number() for listener discovery pagination | — |
| `sql.recursive_cte` | PASS | WITH RECURSIVE | recursive CTEs run on this pin (the planning notes assumed they did not) | — |

### Types

| Probe | Result | PostgreSQL need | Observed on this pin | Substitute / rule |
| --- | --- | --- | --- | --- |
| `type.strict` | PASS | typed columns | STRICT refuses TEXT in INTEGER, BLOB in TEXT and TEXT in BLOB | — |
| `type.u64_boundary` | PASS | bigint / u64 above i64::MAX | i64::MAX round-trips; a literal above it becomes REAL and is refused; negatives fail the CHECK: u64 above i64::MAX must be rejected by the typed constructor, never wrapped | — |
| `type.identifier_columns` | PASS | uuid columns | both BLOB(16) and canonical lowercase TEXT UUID columns can be length- and shape-checked; the representation is chosen in the server schema | — |
| `type.timestamp_micros` | PASS | timestamptz | microsecond INTEGER timestamps keep exact value and order up to i64::MAX | — |
| `type.null_in_unique` | PASS | NULL semantics in UNIQUE | NULLs are distinct in UNIQUE (as in PostgreSQL); `IS` compares NULLs | — |
| `type.text_not_normalized` | PASS | Vietnamese NFC identity | NFC and NFD spellings are stored byte for byte as different values: normalization stays in the domain constructor | — |

### Transactions

| Probe | Result | PostgreSQL need | Observed on this pin | Substitute / rule |
| --- | --- | --- | --- | --- |
| `tx.second_writer_busy` | PASS | single serialized writer | a second BEGIN IMMEDIATE waited 301.228632ms (busy_timeout 300ms) then reported Busy | — |
| `tx.deferred_upgrade_stale` | PASS | why every mutation is BEGIN IMMEDIATE | a read transaction upgraded to a write after another commit fails as StaleSnapshot: every mutation starts BEGIN IMMEDIATE | — |
| `tx.reader_snapshot` | PASS | readers do not see uncommitted writes | a reader never sees an uncommitted write and sees it once committed | — |
| `tx.ddl_transactional` | PASS | atomic file creation and migration | DDL, application_id and user_version roll back together: file creation can be one atomic transaction | — |
| `tx.savepoints` | PASS | partial rollback | SAVEPOINT / ROLLBACK TO / RELEASE | — |
| `tx.failed_statement_keeps_transaction` | PASS | adapter must roll back on a rejected step | a rejected statement does not end the transaction: the earlier change commits unless the adapter rolls back, so adapters must roll back on any rejected step | — |
| `tx.mvcc_not_enabled` | PASS | no MVCC / BEGIN CONCURRENT in v1 | the default journal is wal and BEGIN CONCURRENT is refused: MVCC stays off in v1 | — |

### Durability

| Probe | Result | PostgreSQL need | Observed on this pin | Substitute / rule |
| --- | --- | --- | --- | --- |
| `durability.defaults` | PASS | FULL synchronous write-ahead journal | synchronous = FULL and journal_mode = wal by default; OwnedDatabase refuses to open otherwise | — |
| `durability.reopen_and_integrity` | PASS | committed data survives reopen | 200 committed rows survive close and reopen; integrity_check is ok | — |
| `durability.checkpoint_truncate` | PASS | log checkpoint | wal_checkpoint(TRUNCATE) shrank the log from 482072 to 0 bytes | — |
| `backup.no_experimental_flag` | PASS | online backup path without experimental features | VACUUM INTO produces a readable copy with no experimental feature enabled; in-place VACUUM stays off | — |
| `backup.stale_for_another_connection` | FAIL | VACUUM INTO must include every committed row | VACUUM INTO on a connection that did not make the newest commits omitted them (rows missing per round: [3, 6, 9, 12, 15]) | copy on the writer connection after a refreshing read, then compare a row-count manifest |
| `backup.stale_schema_partial_file` | FAIL | a failed VACUUM INTO must not leave a half-written copy | a connection that predates the schema change failed with `Parse error: output file already exists: <run>/backup_schema_copy.db` and left the output file behind | refresh with a read first; remove a destination this process created when the copy fails |
| `backup.after_refresh_read` | PASS | refreshing read before VACUUM INTO | a read on the copying connection just before VACUUM INTO makes the copy include every committed row | — |
| `backup.consistent_under_writes` | PASS | VACUUM INTO while another connection writes | 26 pairs were committed before the copy and 46 by the end of the load; the copy holds 27 t rows and 27 twin rows, integrity ok | — |

### Process

| Probe | Result | PostgreSQL need | Observed on this pin | Substitute / rule |
| --- | --- | --- | --- | --- |
| `process.kill9_after_ack` | PASS | every acknowledged commit survives SIGKILL; the owner lock dies with the process | killed after 60 acknowledged commits; reopened with 60 rows, integrity_check ok, lock re-acquired | — |
| `process.second_owner_refused` | PASS | a second process cannot open a file that already has an owner | refused while owned (exit 10); accepted after the owner closed | — |
| `process.file_size_limit` | PASS | a write refused for lack of space (simulated by a file-size limit, not ENOSPC) leaves a consistent file | 2 commits acknowledged, then `fault commit Unknown(Io(FileTooLarge))`; reopened with 2 rows, integrity_check ok | — |

## Dependency qualification

Reviewed 20 October 2026 for #332.7 as far as it concerns the one new database dependency.

| Check | Result |
| --- | --- |
| Pin | `turso =0.8.3` (latest stable on crates.io at review; `0.9.0-pre.1` exists and was not chosen), MIT, `default-features = false` |
| Size of the change | 77 crate names new to `Cargo.lock`; the normal dependency tree is about 233 crate versions. Binary size and RSS are **not measured** (#333.11) |
| Advisories | `cargo deny check advisories` over the new tree found RUSTSEC-2026-0009 in `time 0.3.45` (via `tracing-appender`); `Cargo.lock` now resolves `time 0.3.55` and the check is clean. No advisory DB is committed or pinned; this is a dated manual run, `cargo-deny 0.20.2` |
| Licenses | Only permissive licenses (MIT, Apache-2.0, Unicode-3.0, BSD-3-Clause, Zlib, Unlicense and their OR-combinations); `cfg_block 0.1.1` declares no license field in its manifest (upstream dual MIT/Apache-2.0 by README, not verified here) |
| Lockfile side effects | `icu_*` 2.2 → 2.3 (required by `turso_core`'s `icu_collator`), `num-conv`, `time*`; `prost 0.14.4` now coexists with the API's `prost =0.13.5` because `turso_sdk_kit` uses the newer one |
| MSRV and OS | Builds and tests on Rust 1.98.1 Linux x86_64. macOS arm64 build and test: `NOT_RUN` |
| `cargo-deny` / `cargo-audit` in the repository | Not added; a repository policy is a separate decision (#332.7) |

## Not established

- macOS arm64 behavior and any filesystem other than `tmpfs` and `ext4`.
- True `ENOSPC`, power-loss or fsync-reordering durability; SIGKILL durability was shown only at the OS-buffered level a process kill can reach.
- MVCC / `BEGIN CONCURRENT` (kept off), engine multi-process access (kept off; ownership is the sidecar lock) and Windows.
- Throughput, latency percentiles, RSS or binary size; behavior of the 82-table schema (this report probes constructs, not the schema).
- That the SQLite-compatible surface equals SQLite beyond the probed constructs.
- The Server schema, adapters, the invariant mapping table and the PostgreSQL → Turso data conversion (#333.3–#333.9, #336).
