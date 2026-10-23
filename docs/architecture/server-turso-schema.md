# Server Turso foundation schema

**Status: composed foundation, business port incomplete.** `Store`, `cantos-migrate`, operator
and host use the one Server-owned embedded Turso file through `platform/store` and
`platform/turso`. This is not complete business schema v1. Actor, native Identity-session and
development-session tables exist; registered business migration slots remain incomplete.
See the [checkpoint](server-turso-porting.md#checkpoint-boundary).

`turso-candidate` and `dbsp-experimental` no longer exist. Foundation/session tests run through
ordinary Rust tests; the real Identity journey requires its prebuilt executable. Prior feature-
unification/DBSP failures remain historical evidence, not current prerequisites. The invariant
mapping below retains the actual foundation guarantees and its unaccepted HTTP/restore gaps.

| Slice | Tables | State |
| --- | --- | --- |
| 333.4 actor registry (version 1) | `actors`, `actor_external_identities` | composed adapter [`TursoIdentityRegistry`](../../services/cantos-server/src/platform/identity/turso.rs) with file-backed tests |
| #335 provider-free sessions (version 2) | `identity_sessions`, `identity_session_tokens` | composed adapter [`TursoSessions`](../../services/cantos-server/src/platform/identity_auth/turso.rs) behind the shared [`SessionStore`](../../services/cantos-server/src/platform/identity_auth/session.rs) port, with file-backed tests and a [real Identity journey](../../services/cantos-server/tests/identity_auth_candidate.rs) |
| every other `server` disposition | remaining tables of the [manifest](../../services/cantos-server/schema/native-table-ownership.json) | pending |

## One registry, one owner

[`platform::turso`](../../services/cantos-server/src/platform/turso/mod.rs) holds the only Server
registry: service-owned `SERVER_APPLICATION_ID` (`application_id` `CNTS`, `0x434E5453`), its own
epoch 1 and checksummed history.
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

## Identity session mapping

Source rules are [`0004_identity_sessions.sql`](../../services/cantos-server/schema/0004_identity_sessions.sql),
the historical [PostgreSQL adapter](https://github.com/loveoverflowcom/cantos/blob/c3acd81bfa94fe29bf4944dcafd5520dd0d50cdd/services/cantos-server/src/platform/identity_auth/postgres.rs) and
[Identity auth v1](../../packages/contracts/identity-auth-v1.md). The orchestration
(`IdentityAuth`: fact validation, roster, fresh security reads, `decide_issue`/`decide_admission`,
`SessionPolicy` and rotation) is shared: both adapters implement the one `SessionStore` port, so
no decision is copied. Schema tests live in [`platform/turso/tests.rs`](../../services/cantos-server/src/platform/turso/tests.rs),
adapter tests in [`identity_auth/turso/tests.rs`](../../services/cantos-server/src/platform/identity_auth/turso/tests.rs);
deadlines are Unix **seconds** (the `SessionPolicy` unit), taken from the trusted clock by a
checked `div_euclid` of its microseconds.

| Source rule | Turso mechanism | Test |
| --- | --- | --- |
| `id uuid`; the shell parses a client's session id with `Uuid::parse_str` and PostgreSQL compared the cast | canonical lowercase hyphenated `TEXT` with a fail-closed `REGEXP` CHECK; the adapter canonicalizes every spelling both parsers admit (case, braces, no hyphens) before comparing, and names no family for `urn:uuid:` (PostgreSQL's cast refused it with `503`; Turso answers as an unknown credential); `revoke` of an unparsable id is a no-op where PostgreSQL errored, and only stored canonical ids reach it; the decoder refuses a non-canonical row | `identity_session_rows_admit_only_bound_well_formed_values`, `wrong_subject_actor_or_channel_and_an_inactive_actor_never_admit`, `corrupt_projections_are_refused_by_the_production_decoders` |
| channel, issuer, subject `^[0-9a-f]{32}$`, assurance, `authenticated_at > 0`, version `> 0`, 16-byte epoch, `idle <= absolute`, `revoked = (reason IS NOT NULL)`, the reason set | the same CHECKs on STRICT columns, plus **added** nonzero subject and epoch (the decoders already refused zero) | `identity_session_rows_admit_only_bound_well_formed_values` (each CHECK reached past the binding guard) |
| `FOREIGN KEY (issuer, subject)` to the registry and `actor_id` to `actors` | both foreign keys, plus an **added** `identity_sessions_bound_actor` guard: the session's actor is exactly the one its issuer/subject is bound to (`cantos:identity_sessions.binding`) | `identity_session_rows_admit_only_bound_well_formed_values` |
| `identity_session_register` (advisory lock, `FOR SHARE` actor, inactive → no session) | the Actor Registry's own transaction-aware [`register_in`](../../services/cantos-server/src/platform/identity/turso.rs) inside the session's `BEGIN IMMEDIATE`, so binding, session and first secret commit or roll back together; `ActorInactive` → `Inactive` | `binding_and_issue_are_one_unit_and_any_failed_write_rolls_both_back`, `wrong_subject_actor_or_channel_and_an_inactive_actor_never_admit` |
| only SHA-256 digests; Web has no refresh digest; `NOT used OR refresh_hash IS NOT NULL` | 32-byte `BLOB` CHECKs and the same CHECK; an **added** `identity_session_tokens_issue` guard: a new secret is unspent and belongs to a live session of its own channel shape | `stored_rows_hold_only_digests_of_the_secrets`, `identity_session_rows_admit_only_bound_well_formed_values` |
| `identity_session_guard`: revocation permanent, binding/time/snapshot/absolute immutable, idle never decreases, consumption permanent, token binding immutable | row-level `BEFORE UPDATE` guards with the same predicates, plus **added** no-delete guards and a non-decreasing token expiry; `BEFORE INSERT … WHEN EXISTS` refuses an insert over an existing id, access digest or refresh digest, because `REPLACE` deletes without firing delete triggers | `identity_session_rows_never_move_resurrect_shrink_or_disappear` (UPDATE, DELETE, upsert, `INSERT OR REPLACE`, `REPLACE INTO`), `every_deadline_is_exclusive_and_no_renewal_moves_the_absolute_one` (a clock read that went back cannot shorten idle) |
| `FOR UPDATE`/`FOR SHARE` of the family and actor on renew, peek and rotate | facts, `core::rotate` and writes inside one `BEGIN IMMEDIATE`; guarded `UPDATE`s must change exactly one row | `concurrent_refreshes_spend_once_and_the_loser_revokes_the_family`, `issue_access_renewal_rotation_and_logout_follow_the_port_contract_on_a_real_file` |
| single-use refresh; spent-secret replay revokes the family | `used` flag spent in the rotation unit; replay at peek or rotate commits `revoked_reason='replay'` before answering | `issue_access_renewal_rotation_and_logout_follow_the_port_contract_on_a_real_file`, `revocation_is_idempotent_keeps_its_first_reason_and_survives_reopen` |
| `clock_timestamp()` in SQL; post-lock clock recheck | the transaction's trusted stamp, taken after the writer is held; nothing waits inside a unit, so the recheck has no counterpart; reads use one clock reading | `every_deadline_is_exclusive_and_no_renewal_moves_the_absolute_one` |
| PostgreSQL's admission lease (`admit`: family/actor share-locked through `dispatch`) | **not** translated into a writer held across a callback. `admit_in` re-admits inside a business write's own transaction, which the serialized writer orders against logout and deactivation; a read-only dispatch is admitted at its reader snapshot only | `a_business_write_admits_inside_its_unit_and_logout_linearizes_around_it` |
| commit failures map to `Unavailable` | `CommitFault::Rejected` → `Unavailable`; `Unknown` → the **added** `SessionError::Unknown` (HTTP `503`), never retried: a lost rotation acknowledgement may have spent the secret, and the client's retry is a replay | `a_lost_acknowledgement_is_unknown_and_reconciles_without_a_retry` |
| a rejected statement leaves no partial effect | every unit rolls back on any failed step; a failed revoke write is reported and leaves the session unrevoked | `binding_and_issue_are_one_unit_and_any_failed_write_rolls_both_back` |
| invalid stored rows are never served | checked decoders through `ActorId`, `channel_of`, `Subject`, `Epoch` and nonnegative storage integers; a second row is corrupt | `corrupt_projections_are_refused_by_the_production_decoders` |
| version 1 files | the registry appends version 2; `Serve` refuses a version-1 file as `PendingMigrations` without changing it, and `Migrate` upgrades it in place keeping its rows | `an_actor_registry_file_upgrades_to_the_session_slice_and_keeps_its_rows` |

The [journey](../../services/cantos-server/tests/identity_auth_candidate.rs) composes the real
`cantos-identity` executable (its own `identity.db`, authenticated UDS, run-owned loopback SMTP)
with the production `IdentityAuth` and these adapters on a real `server.db`: signup by the mailed
link, explicit completion and its replay, password login, a fresh security read, the exact
binding, Web renewal, native rotation and replay, the Studio roster, logout, refresh cleanup, an
Identity outage that fails closed without revoking, and reopen. The same orchestration over the
facts suite's scripted Identity (mock-integration, in the adapter tests) covers durable
revocation with its reason for every security change, a change between fact and issue,
concurrent first logins, renewal racing logout and a slow Identity. The journey needs a prebuilt
`CANTOS_TEST_IDENTITY_BINARY` and runs under the [isolation guard](../assurance/native-isolation.md#native-auth-linux-qualification-campaign).
It is not HTTP, cookie transport or production Server binary evidence.

The [inventory mismatches](../../specs/behavior/inventory.md) are unchanged by this slice: a
reply naming another subject is still a protocol failure without revocation, a failed revoke
write is redone only while Identity still differs, and roster removal/re-add is undecided.

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

- The remaining business schema/adapters and foreign keys to `actors` (#333). Owner composition,
  explicit Turso migration/startup refusal and typed StoreError mapping already exist.
- Session admission inside real business writes: `admit_in` has no consumer until the context
  adapters are ported. Native writer/read-snapshot admission must retain the old policy assertions;
  the read-only linearization difference requires explicit native HTTP acceptance.
- Server HTTP, cookie and Bearer transport over the Turso sessions, and the production binary.
- Differential parity against the retained PostgreSQL oracle (the old gate is retired); importer
  and Tier B conversion (#336).
- Quantified request/queue behavior at HTTP under contention. The shared writer already has
  a bounded wait; that foundation mechanism does not accept business load or HTTP deadlines.
- True disk-full, power-loss and fsync-error durability: an unknown commit is observed only through
  the engine's view after the fault and a reopen.
- Throughput, latency and footprint (#333.11).
