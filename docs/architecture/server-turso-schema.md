# Server Turso candidate schema

**Status: implemented candidate, not composed.** This is the #333 `server.db` schema built on the
shared [`cantos-store-turso`](../../packages/store-turso/README.md) owner, one slice at a time.
Nothing serves from it: `Store::new`, `DATABASE_URL`, `cantos-migrate` and the default
`cantos-server` binary still use PostgreSQL until the #333.9 flip. It is not Server schema v1.
See the [ownership map](native-persistence-readiness.md) and [issue #333](https://github.com/loveoverflowcom/cantos/issues/333).

The modules compile only into `cantos-server` unit tests (through the store's dev-dependency,
so the existing `rust-tests` gate runs them) and under the opt-in `turso-candidate` feature; no
default build links the engine. Feature unification is a known blocker for the flip:
`dbsp-experimental` enables `chrono/rkyv-64`, whose `rkyv` impls break type inference in
`turso_core 0.8.3`, so a build with both does not compile (#333.9/#333.10 decide it). The required
`dbsp` gate is therefore already red on the staging base, through the store's dev-dependency.

| Slice | Tables | State |
| --- | --- | --- |
| 333.4 actor registry | `actors`, `actor_external_identities` | candidate adapter [`TursoIdentityRegistry`](../../services/cantos-server/src/platform/identity/turso.rs) with file-backed tests |
| every other `server` disposition | remaining tables of the [manifest](../../services/cantos-server/schema/native-table-ownership.json) | pending |

## One registry, one owner

[`platform::turso`](../../services/cantos-server/src/platform/turso/mod.rs) holds the only Server
registry: `DatabaseKind::Server` (`application_id` `CNTS`), its own epoch 1 and checksummed history.
PostgreSQL epochs, SQL and checksums are never replayed. `ServerDatabase::open` is the only way an
adapter receives the owner, so a file opened with another registry cannot reach one. `Intent::Serve`
never changes schema and refuses a missing, foreign, diverged, newer or other-epoch file, and a
file that is not a database, without changing its bytes.

Each slice appends a migration version; an applied version is never edited. SQL stays in the
owning persistence adapter; the registry module holds DDL only. Adapters share the owner's
serialized `BEGIN IMMEDIATE` writer, its `query_only` readers and the trusted clock. No new
pool, engine owner or daemon exists.

## Invariant mapping

Source rules are PostgreSQL epoch 2 ([`0001_current.sql`](../../services/cantos-server/schema/0001_current.sql))
and the [registry contract](identity-registry.md#identity-contract). Tests live in
[`platform/turso/tests.rs`](../../services/cantos-server/src/platform/turso/tests.rs) (schema) and
[`platform/identity/turso/tests.rs`](../../services/cantos-server/src/platform/identity/turso/tests.rs)
(adapter); each runs on the pinned engine and a run-owned file.

| Source rule | Turso mechanism | Test |
| --- | --- | --- |
| `actors_id_check` (`id ~ '^[a-z0-9_-]{1,64}$'`) and `ActorId::new` | fail-closed `CHECK (coalesce(id REGEXP '…', 0))` | `actor_id_checks_admit_exactly_what_the_domain_constructor_admits` (differential against the constructor) |
| `actors.active boolean NOT NULL DEFAULT true` | STRICT `INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))`; the decoder accepts only 0/1 | `actor_rows_keep_their_id_forever_and_store_only_a_boolean_active_flag`, `corrupt_projections_are_refused_by_the_production_decoders` |
| `actors` primary key, referenced by every owner FK; no delete path exists | primary key plus **added** `actors_id_immutable`, `actors_no_delete` and `actors_no_replace` guards (`cantos:immutable.actor_id`); deactivation remains an `active` update | `actor_rows_keep_their_id_forever_and_store_only_a_boolean_active_flag` |
| issuer/subject `octet_length` 1–1024/1–512 with Unicode whitespace-only and control `!~` classes; `ExternalIdentityKey::new` | `octet_length` bounds and `NOT coalesce(col REGEXP '…', 1)` with the same classes plus NUL | `external_key_checks_admit_exactly_what_the_domain_constructor_admits` (controls, every named space and neighbour, Vietnamese, astral, byte bounds) |
| exact `COLLATE "C"` primary key `(issuer, subject)`; no case, normalization or email alias | `COLLATE BINARY` primary key (byte order, capability `collation.binary_is_byte_order`) | `bindings_are_exact_byte_keys_reference_an_actor_and_never_change`, `registration_lookup_and_linking_follow_the_port_contract_on_a_real_file` |
| `actor_external_identities_actor_id_fkey` | `REFERENCES actors(id)`; the owner enables and verifies foreign keys on every connection | `bindings_are_exact_byte_keys_reference_an_actor_and_never_change` |
| `actor_external_identities_immutable` (row) and `_no_truncate` (statement) | row-level `BEFORE UPDATE`/`BEFORE DELETE` `RAISE(ABORT, 'cantos:immutable.actor_external_identities')`, also fired by an unqualified `DELETE`; `BEFORE INSERT … WHEN EXISTS` refuses an insert over an existing key, because the engine's `REPLACE` deletes the conflicting row without firing delete triggers (observed: without that guard `INSERT OR REPLACE` rebinds the key) | `bindings_are_exact_byte_keys_reference_an_actor_and_never_change` (UPDATE, DELETE, upsert, `INSERT OR REPLACE`, `REPLACE INTO`) |
| `linked_at timestamptz DEFAULT CURRENT_TIMESTAMP` | microsecond `INTEGER CHECK (linked_at >= 0)` without default, written from `WriteTransaction::now()` (trusted clock, read after the writer is held) | `registration_lookup_and_linking_follow_the_port_contract_on_a_real_file`, `bindings_survive_reopen_and_replay_returns_the_original_actor` (`i64::MAX`) |
| per-key `pg_advisory_xact_lock` and `FOR SHARE OF a` against concurrent deactivation, under the 3 s lock and 5 s statement timeouts | facts, decision and writes inside one `BEGIN IMMEDIATE` on the serialized writer; no row lock is promised. The wait for the writer has **no deadline** (the 3 s busy timeout covers only engine-level busy), so one stalled transaction queues every write; a dropped request's open transaction is rolled back by the next `begin_write` | `concurrent_registrations_of_one_key_commit_one_binding_and_no_losing_actor`, `a_held_writer_orders_competing_requests_and_readers_never_see_uncommitted_bindings` |
| actor and binding created atomically; a rejection leaves no losing actor | one transaction rolled back on any rejected step; an insert over an existing key is the schema's `no_replace` guard, mapped to `ActorIdCollision`/`IdentityAlreadyBound`, and `ON CONFLICT DO NOTHING` must still change exactly one row | `a_failed_step_after_the_actor_insert_rolls_back_the_speculative_actor`, `rejections_and_inactive_or_missing_targets_change_no_row` |
| replay returns the bound winner; a conflicting candidate creates nothing | `decide_registration` over facts read in the transaction | `registration_lookup_and_linking_follow_the_port_contract_on_a_real_file`, `bindings_survive_reopen_and_replay_returns_the_original_actor` |
| linking never transfers or reactivates; missing or inactive targets are refused | `decide_link` plus the immutable binding guard | `rejections_and_inactive_or_missing_targets_change_no_row` |
| a lost commit response is `Unavailable`; retry the exact key | `CommitFault::Rejected` and `Unknown` both map to `Unavailable`, never to a rejection or "no effect"; a read-only decision rolls back and needs no acknowledgement; the immutable binding is the receipt the replay reads | `a_lost_acknowledgement_after_a_real_commit_reconciles_by_exact_key_replay`, `a_lost_acknowledgement_without_effect_leaves_no_orphan_and_replay_creates_once` |
| an invalid stored record is `CorruptRecord` | checked decoders; a binding without its actor row (LEFT JOIN) or a second row is corrupt; engine `Corrupt`/`NotDatabase` maps to `CorruptRecord` | `corrupt_projections_are_refused_by_the_production_decoders`, `engine_corruption_is_never_served_and_other_faults_are_unavailable` |
| uncommitted writes are invisible to other sessions | separate `query_only` reader connections | `a_held_writer_orders_competing_requests_and_readers_never_see_uncommitted_bindings` |
| checksummed registry, read-only serving startup, one writer process | the Server registry above and the owner's sidecar lock | `a_new_file_holds_only_the_registry_metadata_and_the_actor_registry_slice`, `foreign_diverged_newer_other_epoch_and_non_database_files_are_refused_unchanged`, `a_second_owner_is_refused_while_the_file_is_open` |

## Trust shift

PostgreSQL granted no registry privilege to `PUBLIC` or `cantos_app`; only explicit operator
database privilege could write. Turso has no roles or grants. The boundary becomes the owner
process, its private `0700` directory and sidecar lock, parameterized SQL and this adapter as the
only write path, with no HTTP route. Any process running as the service user with file access can
write rows and can alter the schema itself (for example drop a trigger). CHECKs and triggers bind
every connection only while the schema is intact; foreign keys bind only connections that enable
them, which the owner does and verifies on each of its own. There is no table-level privilege
model, and application tests do not replace those database barriers.

## Not established

- Runtime composition, `cantos-migrate` for Turso, startup refusal in the binary and error mapping
  into `StoreError` (#333.2); the remaining schema and its foreign keys to `actors` (#333.3, #333.5–8).
- Differential parity against the PostgreSQL adapter under `cargo xtask verify postgres`; importer
  and Tier B conversion (#336).
- A deadline or bounded backpressure for the writer queue: it belongs in the owner API or the
  composition, decided before #333.9 composes a serving path.
- True disk-full, power-loss and fsync-error durability: an unknown commit is observed only through
  the engine's view after the fault and a reopen.
- Throughput, latency and footprint (#333.11).
