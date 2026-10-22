# Listener Library on `server.db` (migration 15) and admission in the business unit

Scope: the account-owned Library the CMP client reads and writes: Saved, progress, history,
bookmarks, per-device sequences, playback sessions and idempotent operation receipts. This is the full
Listening persistence of the pre-port baseline; the reserved placeholder version 11 is unchanged and
the tables arrive in appended version 15 `server_listener_library`. See the [slice record](cmp-business-api.md).

Persistence is `contexts/listening/turso.rs` (renamed from `postgres.rs`). Timestamps and
`listener_order` are the domain's Unix milliseconds and per-account counter.

## Admission inside the unit

The HTTP session middleware performs a *pre-dispatch* check and holds no database writer
(`IdentityAuth::lease`). The business write re-admits the session itself:

1. `PasswordListenerAuthenticator` seals an `AuthenticatedListener` only from a live Identity session
   plus a fresh Identity security-state read. It now also seals a `ListenerAdmission` (access secret,
   channel, session id; Debug redacted, constructible only inside the crate).
2. `LibraryService` passes that principal to the adapter as an `Access`.
3. `TursoLibrary::mutate` opens one `BEGIN IMMEDIATE` unit and, first, calls
   `identity_auth::turso::admit_in(tx, token, channel)`: live session, active actor, unexpired at the
   unit's own stamp. The re-admitted actor **and** session id must equal the principal's. Only then
   does it read facts, decide, and write the effect, the device sequence, the account order and the
   receipt, and commit. Reads re-admit at their pinned snapshot only.
4. No Identity RPC and no media/file I/O happens inside the unit. A principal vouched for by a trusted
   host override (no session) is checked against the active actor only.

Both orderings are tested: a logout/revocation that commits before the unit makes it fail with no
effect; a write that commits first keeps its durable receipt and the logout follows. The
`X-Cantos-Theatre-Session` family fence is unchanged (the request value is an expectation, the stamp is
the admitted session; a wrong family is refused before any effect).

## Mapping

| Source rule (PostgreSQL object / adapter behavior) | Turso mechanism | Test |
| --- | --- | --- |
| `listener_lock_actor` (`FOR SHARE`) and `listener_state … FOR UPDATE` | deleted; the serialized writer orders the unit. Facts are read inside it. The account row is created by a pre-checked insert | `duplicate_and_same_base_first_write_races_…`, `first_write_and_save_unsave_races_…` |
| `listener_actor_active` | `admit_in` (session and active actor) or, without a session, `SELECT active FROM actors` | `admission_binds_the_actor_the_session_and_the_deadline_to_the_unit` |
| `guard_listener_progress`, `guard_listener_saved` | `BEFORE UPDATE` guards: identity unchanged, `revision = OLD.revision + 1`, `listener_order > OLD.listener_order`; plus delete and insert-over-key guards | `library_rows_are_checked_against_their_snapshots_and_only_move_as_their_guards_allow` |
| `guard_listener_bookmark` (value immutable, tombstone final) | `BEFORE UPDATE` guard; `BEFORE DELETE`; insert-over-key | same (second tombstone, resurrection, position change) |
| `listener_*_check1`: `convert_from(snapshot)::jsonb @> jsonb_build_object(…)` | `CHECK (json_valid(…) AND json_extract(CAST(snapshot AS TEXT), '$.…') IS <column> …)`. `IS` makes a missing path fail instead of passing as `NULL` | same (a flipped scalar beside its snapshot) |
| `reject_settled_change` on `listener_operations`, `listener_playback_sessions` | update/delete/insert-over-key guards | same |
| `listener_device_sequences` conditional upsert | the same upsert; a `BEFORE UPDATE` guard refuses a non-increasing sequence | same |
| `INSERT … ON CONFLICT DO UPDATE` for Saved | explicit `INSERT` for revision 0 and `UPDATE … WHERE revision = ?` otherwise (the insert-over-key guard would fire on an upsert); the affected row is proven to be exactly one | `first_write_and_save_unsave_races_…` |
| history upsert (`ON CONFLICT (listener_id, work_id)`) and clear | unchanged upsert (no guard on history, as PostgreSQL); `DELETE` on clear | `clear_preserves_progress_saved_and_sessions_and_the_router_is_account_isolated` |
| continue page `DISTINCT ON (work_id) … listener_order DESC` | `row_number() OVER (PARTITION BY work_id ORDER BY listener_order DESC, publication_id)`; the upper bound and cursor filters apply after it | `latest_completed_release_suppresses_older_unfinished_work_…` |
| keyset `(a, b) > ($3, $4::text COLLATE "C")` | expanded comparison; `BINARY` collation | bookmark and page walks |
| `publication_id = ANY($2)` | `IN (SELECT value FROM json_each(?2))` | history presentation tests |
| catalog `FOR SHARE` fence, `clock_timestamp()` before and after | deleted; one trusted stamp per unit; new identities are re-proven against the finished effects just before COMMIT | `replacement_and_withdrawal_retain_owned_progress_…` |
| `Unavailable` after COMMIT (unknown outcome) | not a rollback: the same operation id returns the committed receipt; an unacknowledged rollback leaves nothing | `a_lost_commit_acknowledgement_is_reconciled_by_replaying_the_operation` |
| receipt failure rolls back every effect | the unit is dropped uncommitted; the owner rolls it back before the next write | `a_failed_receipt_rolls_back_progress_sequences_and_account_state_before_the_retry` |

Semantics preserved from the baseline and covered by the ported suites: the progress reconciliation
policy including intentional rewind, the history epoch fence for delayed old-epoch starts, bookmark
tombstones (never remapped to a replacement release), unavailable publication metadata projecting
`null` while the owned record survives, account-scoped receipts and cursors, and the
`presentation=1` opt-in with the bookmark resource always carrying metadata.

## Trust shift

No roles. The server process is the only writer; its Library SQL is parameterized and every statement
runs inside a unit that first re-admits the session. Receipts hold the exact answered JSON and are
re-validated when read.

## Not established

- A live provider and a device; the CMP client's reconnect scheduler (tracked separately).
- Multi-process access: the owner lock allows one process per file.
- Cursors are process-local by design: a restart invalidates a walk (tested), so no durable cursor
  state exists to migrate.
