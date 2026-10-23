# Porting one Server context from PostgreSQL to embedded Turso (`server.db`)

Audience: an engineer/agent porting ONE context. Read this fully, then the sources it names.
Goal of the whole migration: `cantos-server` serves every use case from ONE embedded Turso file
(`server.db`) with the same domain meaning, API behavior, IDs, revisions, hashes, receipts and
immutability as the PostgreSQL implementation. PostgreSQL is being deleted; there is no fallback,
no dual write, no compatibility shim at the end.

## Checkpoint boundary

This is an implementation guide for an incomplete Draft checkpoint, not a completed Server
port or acceptance record. `Store` and all binaries use the embedded owner. Actual DDL covers
actors, native Identity sessions, development sessions and the
[CMP business API closure](server-turso/cmp-business-api.md) (appended versions 12–15: catalog
revisions, production briefs, the external-audio release chain with its listener index, and the
Listener Library); registered context slices 4–11 still contain comments, not business tables.
Context adapters outside that closure still execute PostgreSQL-specific queries through the
statement facade. Successful compilation or an empty migration does not make those operations work.

- Source retirement is partial: provider auth and PG drivers are removed from the Server build,
  while PostgreSQL schema, business queries, legacy test oracles, scripts and operating assets
  remain. Retain them as migration inputs until the replacement is reviewed.
- Buildability is measured independently by Rust lint/tests. File-backed foundation/session
  tests do not replace the suites moved into `legacy-pg-tests/`; none of those archived tests
  run through Cargo discovery.
- `cargo xtask run` plans explicit migrations, Identity startup and one Server. Its orchestration
  tests use recording runners; they do not establish a two-process authentication journey.
  `cargo xtask seed theatre` refuses before effects while the `catalog_http` seeder is absent.
- `verify postgres`, `verify dbsp`, `verify theatre-auth`, Kanidm/service lifecycle, legacy VPS
  deploy and legacy API release commands refuse with exit 2. Web/Android release tools remain.
- Native business/API parity, real signup/login/revocation and restore acceptance belong to
  #333/#335/#336; OCI activation and live cutover belong to #348 with separate operator authority.
  No deployment, provider call, data conversion or production resource change is part of this PR.

The invariant is unchanged ownership and immutable receipts while swapping storage; the current
foundation tests cover checked rows, transaction rollback, writer ordering, snapshots, session
admission and unknown commit outcomes. Each context port must add its own real-file evidence.
No queue ordering change is needed: context porting remains the next dependency.

## 0. The pieces that already exist (do not rebuild them)

| Piece | Where | Notes |
|---|---|---|
| shared owner, registry, serialized writer, readers, clock, backup | `packages/store-turso` | exact pin `turso =0.8.3`; read `README.md`, `docs/evidence/turso-capability-report.md` (48 probes: what works, what needs a substitute) |
| Server registry | `services/cantos-server/src/platform/turso/mod.rs` (`MIGRATIONS`, DDL in sibling `*.sql`) | append ONE migration per context at your assigned version; never edit an applied version (versions 1 actors, 2 identity sessions, 3 development sessions exist) |
| statement facade | `src/platform/store/sql.rs` (+ `mod.rs`) | `Conn` (from `store.connection()`), `Tx` (`conn.transaction()` = write unit; `conn.read()` = pinned read snapshot), PG-shaped `query/query_opt/query_one/execute/commit/rollback`, `Row::try_get/get` with checked column types, `Param` impls (`&str`, `String`, `i64/i32/..`, `bool`, `Vec<u8>/&[u8]`, `Option<T>`, `Vec<String>`/`Vec<i64>` as a JSON array for `json_each`), `tx.now_micros()` / `tx.now_seconds()` (trusted clock, SQL never reads the wall clock), typed `Error` (`guard()`, `constraint()`, `is_integrity_violation()`, `outcome_unknown()`) |
| admission in the business unit | `platform::store::authenticate(&tx, token)` | Studio `ci1_` Identity cookie or development token, re-admitted in the SAME unit as the write |
| file-backed test fixture | `src/test_support.rs` (`ServerFixture`) | `new()`, `store()`, `restart()`, `exec`, `rows`, `count`, `seed_actors`, `seed_development_session`, `studio_caller`; enabled for tests by the package's self dev-dependency |
| existing worked examples | `src/platform/identity/turso.rs`, `src/platform/identity_auth/turso.rs`, `docs/architecture/server-turso-schema.md` (mapping tables: source rule → Turso mechanism → test) | copy this style of reasoning |

PostgreSQL source of truth for your tables, constraints, functions and triggers:
`services/cantos-server/schema/0001_current.sql` (+ `0002`/`0003`/`0004`). Your context's adapter
today is `src/contexts/<ctx>/postgres*.rs` (name varies). PG-era integration tests were moved to
`services/cantos-server/legacy-pg-tests/**` (if present): they are your regression oracles.
The [coverage ledger](server-turso/legacy-test-oracles.md) inventories all retained files,
assertions and partial native mappings. It establishes no whole-file parity. The direct
`scripts/test_postgres.py` entrypoint refuses before setup; its historical implementation is
porting input, not a runnable gate. Keep unmatched oracles until the owning native boundary runs.

## 1. Rules that never bend

1. Same meaning: IDs, revision numbers, digests/hashes, immutable history, idempotent operation
   receipts, authorization decisions, error mapping and HTTP outputs do not change. Do not edit
   public contracts (`packages/contracts`) or HTTP handlers' wire behavior.
2. Every PG constraint/trigger/function for your tables has a Turso mechanism AND a test that
   reaches it, recorded in a mapping table (section 6). A rule that cannot be expressed in SQL moves
   to the writing transaction (decision from facts read in the same unit) and gets a test. Never drop
   a rule silently; if you decide a PG rule is dead, say why with evidence.
3. Ordering replaces locks. All writes are `BEGIN IMMEDIATE` on one serialized writer. Facts a decision
   depends on are read inside the unit that writes. `FOR SHARE/UPDATE`, `SELECT … FOR`, advisory locks,
   `SECURITY DEFINER` lock helpers (`catalog_lock_actor`, `production_lock_actor`, `listener_lock_actor`,
   `*_lock_member`) become plain reads in the unit. Conditional updates still prove their affected-row
   count (`!= 1` is a conflict/corruption, never success). A zero-row UPDATE is not success.
4. Never hold the writer across anything that is not the database: RPC, password hashing, mail, S3/object
   storage, file I/O of media, long CPU. Do those before or after the unit. There is ONE writer: opening
   a second `conn.transaction()` while a first is alive deadlocks until the bounded wait (5 s) fails it as
   `Unavailable`. Audit every use case in your context for nested/overlapping units and for I/O inside a
   unit; fix by restructuring (read facts → do I/O → re-check facts & write in one short unit).
5. Reads: read-only use cases use `conn.read()` (pinned snapshot, never waits for the writer).
   A read that must be consistent with a following write uses the write unit instead.
6. Unknown commit outcome is not rollback. `Error::outcome_unknown()` / `StoreError::Unavailable` after
   COMMIT means the write may be durable: use-cases with operation receipts reconcile by exact-key replay;
   never report "rolled back" or retry blindly. Add a lost-acknowledgement test for each receipt-bearing write
   (see `Acknowledgement::LostAfterCommit` in `platform/turso/mod.rs`, cfg(test): set `store.acknowledgement`
   in a unit test inside the crate).
7. Integers: engine INTEGER is signed 64-bit. Unsigned domain numbers go through `checked_number`
   (`u64`→`i64`, rejects > i64::MAX) and back through a checked conversion; no `as` casts. Timestamps are
   microsecond INTEGER from `tx.now_micros()` unless the domain already uses millis/seconds (keep the
   unit the adapter uses today and say so). Booleans are `INTEGER CHECK (x IN (0,1))`; the decoder accepts
   only 0/1.
8. Parameterized SQL only; identifiers never spliced from input. `format!` into SQL only for static
   column lists/constants.
9. Append migrations; never edit an applied version. DDL only in the migration; queries live in the
   context's persistence adapter. STRICT tables. `PRAGMA foreign_keys` is on per connection (owner does it).
10. No new dependency, no new pool/engine owner, no new daemon, no `ATTACH`, no second database file.
11. Behavior preservation of domain code: do not change pure domain modules except to remove PostgreSQL
    vocabulary. Do not delete a domain use case to make the build pass. Never answer "empty/success"
    when the backend cannot do the task.

## 2. PostgreSQL → Turso translation table

| PostgreSQL | Turso |
|---|---|
| `$1,$2` | `?1,?2` (numbered; reuse allowed) |
| `FOR SHARE / FOR UPDATE [OF t]` | delete (serialized writer) — keep the read inside the unit |
| `x::text`, `::bigint`, `::uuid` casts | delete or `CAST(x AS TEXT)`; columns are typed by the schema |
| `boolean`, `true/false` | `INTEGER` 0/1 + CHECK; SQL `=1`/`=0` |
| `bytea` | `BLOB` (`length(x)=32` CHECKs stay) |
| `uuid` | canonical lowercase hyphenated `TEXT` + `CHECK (coalesce(id REGEXP '^[0-9a-f]{8}-…$',0))`; adapters canonicalize with `normalized_uuid` before comparing |
| `timestamptz`, `now()`, `clock_timestamp()`, `CURRENT_TIMESTAMP`, `extract(epoch …)` | `INTEGER` micros; bind `tx.now_micros()`; never read time in SQL |
| `jsonb`, `->`, `->>`, `to_jsonb(row)` | `TEXT` + `CHECK (json_valid(x))`, `json_extract(x,'$.a.b')`, `json_each`, `json_object` |
| `= ANY($1)`, `unnest($1)` | `IN (SELECT value FROM json_each(?1))` with `Vec<String>`/`Vec<i64>` param (JSON array) |
| `COLLATE "C"` | `COLLATE BINARY` |
| regex CHECK `~`, `!~` | `coalesce(col REGEXP 'p', 0)` ALWAYS (a broken pattern otherwise disables the CHECK); each regex CHECK needs a rejection test; ideally a differential test against the domain constructor |
| `octet_length` CHECK | works as is (bytes) |
| `serial`/sequences | `INTEGER PRIMARY KEY AUTOINCREMENT` only if the domain needs it; otherwise explicit values |
| `ON CONFLICT … DO UPDATE/NOTHING`, `RETURNING` | supported (drain `RETURNING` before the next statement — facade already collects rows) |
| `UNIQUE … DEFERRABLE` | unsupported: two-phase reorder in one transaction (shift out of range then renumber) |
| `DEFERRABLE INITIALLY DEFERRED` FK | supported (head↔revision cycles); violation surfaces at COMMIT as a rejected commit |
| partial unique indexes | supported |
| `FOR EACH STATEMENT` triggers, `reject_settled_change` statement triggers | unsupported → row-level guards, or a check in the writing unit |
| row triggers raising exceptions (`RAISE EXCEPTION … ERRCODE`) | `BEFORE … BEGIN SELECT RAISE(ABORT,'cantos:<lowercase.code>'); END` (typed `Error::guard()`); recursion not supported |
| immutable/settled rows (`reject_settled_change`: UPDATE/DELETE forbidden) | `BEFORE UPDATE` + `BEFORE DELETE` guards **and** `BEFORE INSERT … WHEN EXISTS(same key)` guard (engine `REPLACE` deletes without firing delete triggers; test `INSERT OR REPLACE`, `REPLACE INTO`, upsert) |
| trigger reading other tables / cross-row checks | same SQL in `WHEN`/`SELECT RAISE(ABORT…)`; recheck in the unit if not expressible |
| `GRANT`s / roles / `SECURITY DEFINER` | no equivalent: the boundary is the owner process + parameterized adapter; note the trust shift in your mapping doc; do not invent roles |
| `pg_advisory_xact_lock` | delete (writer ordering) |
| `SQLSTATE 23505` unique violation | `Error::constraint() == Some(ConstraintKind::Unique)` |
| `SQLSTATE 23514/23503/42*/55000` | `Error::constraint()` (Check/ForeignKey) or `Error::guard()` code; `is_integrity_violation()` for "corrupt/invalid" mapping |
| `RepeatableRead` read-only tx | `conn.read()` |
| `statement_timeout/lock_timeout` | none; the writer wait is bounded (5 s) and a request deadline exists in the HTTP layer |

Row/column notes: facade `Row::try_get::<_, T>(idx)` fails (`Error::Column`) on a type mismatch;
never `unwrap_or_default` a column. `i32/i16/u32` decode through checked narrowing.

## 3. Your work product (in this order)

1. Read: this guide; your tables in `0001_current.sql` (and later versions) incl. every function/trigger
   that touches them; your adapter files; your application use cases (`src/contexts/<ctx>/application*`);
   the owner docs for the domain (`docs/**` named in AGENTS.md and the matching `.agents/skills/cantos-*`
   `SKILL.md` that owns the domain: engineering, and the context's owner skill); the legacy PG tests.
2. Inventory: for your tables list every PG constraint/trigger/function/index and its Turso mechanism
   (section 6), plus every PG-ism in your adapter SQL.
3. Migration: `src/platform/turso/<context>.sql` at your assigned version; register in `MIGRATIONS`.
   Fail-closed CHECKs, FKs, guards, indexes with the same predicates as PG (add only what is needed).
4. Adapter: rewrite SQL in your context's persistence adapter; rename the adapter module from `postgres`
   to `turso` (file `turso.rs` / `turso/` dir) and fix imports; remove `LockMode` usage and all `FOR …`.
   Keep function signatures/decisions where possible so application code changes stay small.
5. Application: restructure units only where required by rule 4 (no I/O in a unit, no nested units),
   replace `clock_timestamp()` style time by `tx.now_micros()`, keep session admission in the unit.
6. Tests (file-backed, no PostgreSQL, no network):
   - schema tests: each CHECK/guard/FK/immutability reached through the engine (not through the adapter),
     including REPLACE/upsert bypass attempts;
   - adapter tests over `ServerFixture`: success, conflict/zero-row, replay/idempotency, rollback after a
     failed step, restart (`restart()`), lost-acknowledgement for receipt writes, corrupt-row decoders
     (write a bad row with fixture SQL → `CorruptRevision`/`Corrupt`, never served);
   - port the legacy PG integration suites: keep every assertion/oracle that still has meaning, translate SQL
     fixtures, replace PG-only checks (roles/grants/`SHOW server_version`) with their Turso equivalents or drop
     them with a stated reason. Move each ported file back out of `legacy-pg-tests/` (delete the legacy copy).
     HTTP-level suites build the router with `ServerFixture` (see `tests/support/*` for what exists).
   - race tests where PG tests had concurrency (two tasks, one winner).
7. Docs: write `docs/architecture/server-turso/<context>.md` (new file; do not edit other docs) with the mapping
   table (source rule → mechanism → test), trust shift, what is not established. Do NOT edit
   `docs/architecture/server-turso-schema.md`, `native-table-ownership.json`, `0001-0004` SQL, AGENTS.md,
   skills or guards; report statements elsewhere that your work makes false.
8. Verify (cheap first): `cargo check -p cantos-server --lib --tests`, then
   `cargo test -p cantos-server --lib <your module filter>`, then the specific `--test <name>` targets you
   ported; `cargo clippy -p cantos-server --all-targets --locked -- -D warnings` (touched files clean);
   `cargo fmt -p cantos-server -- --check`. Env: `env CARGO_INCREMENTAL=0 TMPDIR=/private/tmp`.
   Do not run the whole suite repeatedly.

## 4. Anti-patterns to refuse

- translating SQL by text substitution without re-deriving the invariant ("do not translate PostgreSQL
  locks/functions by text substitution");
- sleeping/retrying around a busy writer; swallowing `Unavailable`; catching an error to return default data;
- a trigger that relies on recursion or `FOR EACH STATEMENT`;
- regex CHECK without `coalesce`;
- editing an old migration or reusing another context's version number;
- `ATTACH`, a second `Store`, a second owner of the same file in one process;
- tests that mutate rows through the adapter's own write path to "prove" a database barrier — use raw SQL
  through the fixture for barrier tests;
- leaving `#[ignore]` PG-gated tests: everything you keep must run in the default test run on a laptop.

## 5. Ownership, version numbers, branches

Migration versions (registry order = FK dependency order): 4 editorial+ingestion, 5 catalog, 6 admin,
7 adaptation, 8 production, 9 media artwork/images, 10 media audio (intake, lifecycle, publication, release views),
11 listening. Only add your own version. Touch only your context's files plus the small shared places
named in your brief; for the shared places keep edits minimal/additive (e.g. one line in `MIGRATIONS`,
one `mod` line in `platform/turso/mod.rs` for your schema test module). If you need a facade change
(`platform/store/sql.rs`), keep it additive and mention it in your report.

## 6. Mapping table format (for `docs/architecture/server-turso/<context>.md`)

| Source rule (PG object / adapter behavior) | Turso mechanism | Test |
|---|---|---|
| … | … | `test_name` |

Then sections: Trust shift, Concurrency/ordering notes (which use cases changed shape and why), Not established.

## 7. Report (final message)

Commit SHAs; migration version; tables ported (with counts); table: PG object → mechanism → test; use cases
restructured (I/O out of units, nested units removed); tests ported/added/dropped with reasons;
commands run with cwd/result (PASS/FAIL/NOT_RUN); facade changes; statements elsewhere made false;
residual gaps and exact blockers.
