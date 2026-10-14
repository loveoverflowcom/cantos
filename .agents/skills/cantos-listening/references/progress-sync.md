# Progress and bookmark sync

> **Scope.** Implement and verify durable listening progress, Saved and History registers and
> (proposed) bookmarks across Theatre Web and the CMP app: the local-first outbox contract every
> client obeys, stable device IDs, monotonic sequences and idempotency keys, server revisions, the
> append-only operation log, the conditional write, the pure `reconcile_progress` decision,
> explicit choice for competing sessions, bookmark tombstones, release changes, concrete conflict tables, two-device interleaving
> properties and the real-PostgreSQL concurrency tests. Use before changing how progress or
> bookmarks are stored, sent, accepted, conflicted or displayed after a conflict.

Product rules: [mobile § Durable listening progress](../../../../docs/architecture/mobile.md#durable-listening-progress),
[business rule 28](../../../../docs/product/business-rules.md#listening-and-mobile), the
invariant "Offline progress arrives late" in
[product invariants](../../../../docs/product/business-rules.md#product-invariants-to-verify) and
the [040](../../../../docs/work-plan/040-theatre-web-listening.md#acceptance-criteria) and
[050](../../../../docs/work-plan/050-cmp-native-listening.md#acceptance-criteria) acceptance
criteria. Client durability and scheduling are implemented per renderer:
[CMP downloads and local state](../../cantos-cmp-mobile/references/downloads-and-local-state.md)
and [Leptos async](../../cantos-leptos-web/references/leptos-async.md). Transactions and
concurrency control in general: [`persistence.md`](../../cantos-engineering/references/persistence.md).

## Repository reality

The server half is implemented in `services/cantos-api/src/contexts/listening` under the
[Library v1 contract](../../../../packages/contracts/listener-library-v1.md): pure registers in
`domain/`, use cases in `application.rs`, PostgreSQL in `postgres.rs` (migration
`0019_listener_library_progress.sql`). No client sends these mutations yet: the client contract
below is binding for the future Web/CMP outboxes, and **bookmarks are not implemented**.

## What is synchronized

| Thing | Shape | Key | Changes by | Status |
|---|---|---|---|---|
| Progress | one current register with a server revision and exact `AudioIdentity` | listener + publication (`listener_progress` PK); work, unit, rendition, SHA-256 and duration are immutable per row (trigger) | conditional replacement | implemented |
| Saved | desired state `saved: bool` with a revision; unsave keeps a `false` record | listener + work | conditional replacement; never a toggle | implemented |
| History | latest confirmed playback start per work, plus account `history_revision` and `history_epoch` | listener + work; engine-session tombstones per listener + device + session | start, or explicit clear that bumps the epoch | implemented |
| Bookmark | an immutable value | account + client-generated bookmark ID | create once; delete by tombstone; "move" is delete plus create | proposed |

Progress is a revisioned register, not a counter: a rewind is a legitimate write. Continue is a
projection of progress registers (`projection::continue_records`), never a second store.
Bookmarks, when added, are a set of immutable values, which removes edit conflicts by
construction; an editable label would be a conditional update with its own base revision.

## Types — as implemented

```rust
// listening/domain/mod.rs, abridged. Private fields; constructors validate wire DTOs.
pub struct ListenerId(ActorId);                   // account scope; only from AuthenticatedListener
pub struct Intent { operation: OperationId, device: DeviceId, sequence: u64, reported_at: Option<i64> }
pub struct AudioIdentity { work, unit, publication, rendition, sha256, duration }
pub struct ProgressCandidate { intent: Intent, audio: AudioIdentity, base: Revision, position: u64, kind: dto::ProgressKind }
pub struct ProgressRecord(dto::ProgressRecord);   // only restore() or reconcile_progress() constructs it

// dto::ProgressKind: Checkpoint | ExplicitSeek | Restart | ResolveHere | ResolveOther | MarkCompleted
pub enum ProgressOutcome {
    Accepted(ProgressRecord),
    AlreadyCurrent(ProgressRecord),
    Conflict { current: Option<ProgressRecord>, resolution: dto::Resolution }, // ListenerChooses | KeepServer
}
```

`base` is `Revision` (`0` = no register), never `Option`. Construction rejects
`restart` with a position other than `0`, `mark_completed` at any position but the exact
duration, any position beyond the duration, a zero or out-of-range sequence and a negative
device time. The device's `reported_at` lives in `Intent` as context; `reconcile_progress` never
reads it. That is `documented` and `example-tested`
(`device_clocks_are_context_and_audio_replacement_never_changes_stored_identity`), not
`type-enforced`: moving the device time out of the candidate would make it a type guarantee.

## The client contract

Every client — the Leptos Theatre and the CMP app — must satisfy these obligations. They are
part of the listener contract; how each renderer stores and schedules them is its own skill.
No client implements them yet.

1. **Local first.** Record the position locally, then append an operation to a durable outbox
   in the same local transaction. Playback never waits for the server.
2. **Mint identity at intent time.** `operation_id`, `device_id` and the next `device_sequence`
   are assigned when the operation is appended, never by the HTTP retry layer.
3. **Mark "sent" before sending.** Persist that an operation has been transmitted *before* the
   request leaves. A sent operation may already be committed; from then on it is immutable and
   resent byte-for-byte until its outcome is known.
4. **Coalesce only the unsent.** A never-sent checkpoint or explicit seek for a key may be
   superseded by a newer one (new operation ID). Never coalesce across a `restart`, a
   `mark_completed` or a resolution, and never touch a sent operation.
5. **One in flight per key.** Send the oldest unresolved operation for a key; send the next only
   after the first is resolved. Sequences are per device **and resource key** on the server
   (`progress:<publication>`, `saved:<work>`, `history`), so different keys may be in flight.
6. **Base means reconciled, not fetched.** `base_revision` is the latest revision this device
   has *reconciled* for the key — its own accepted write, an adopted server value or a resolved
   choice — fixed when the operation is first sent. Fetching a newer revision while a candidate
   is pending does not rebase the candidate; the conflict that follows is the correct outcome.
7. **Bounded cadence.** Checkpoint at a configured interval while playing and on pause, stop,
   seek, end and backgrounding where lifecycle permits. Never per second. The interval is a
   named setting recorded with its rationale, so a property can bound writes.
8. **Fence the account.** Responses are fenced by account/session and local intent generation;
   account A's queued operations are never submitted as B
   ([Theatre identity v1](../../../../packages/contracts/theatre-identity-v1.md)).

| Response | Client action |
|---|---|
| `accepted`, replayed receipt, `already_current` | store the returned revision as the reconciled base; resolve the operation |
| `revision_conflict` + `listener_chooses` | keep the local candidate; store the server's current record; show the choice; send nothing more for this key until it is made |
| `revision_conflict` + `keep_server` | drop the candidate; adopt the server value as base; during playback, offer it — never seek ([playback R5](playback-semantics.md#rules-the-reducer-encodes)) |
| `history_epoch_conflict` | drop the acknowledgement; it predates a clear |
| network failure, timeout, `unavailable` | resend the identical body with backoff and jitter, or read `GET /operations/{operation}` |
| `invalid_position`, `invalid_request`, `idempotency_key_reused` | quarantine the operation and report a client defect; never loop |
| `stale_device_sequence` | the install is likely a restored clone: mint a new device ID; rejected operations had no effect, so re-mint them |
| `operation_expired` (proposed) | discard; resync the key from the server |

## Competing sessions: an explicit choice

When no automatic rule preserves clear intent, the docs require the listener to choose. Show
both positions in readable time and submit the choice as a new resolution operation against
the **current** server revision:

```text
Tiến độ trên thiết bị khác đã thay đổi.
  [ Tiếp tục tại đây · 12:34 ]               → kind resolve_here,  position 754000, base r4
  [ Dùng vị trí trên thiết bị khác · 15:00 ] → kind resolve_other, position 900000, base r4
```

Both choices are written, so other devices learn that an explicit choice exists. Choosing the
other device's position is a listener gesture, so the playhead may move. Copy, layout, focus and
announcements belong to [`cantos-ui-design`](../../cantos-ui-design/SKILL.md); what dismissing
the prompt means is an open product decision.

## The server write

`PgLibrary::mutate` runs one read-write transaction per mutation, in this order:

1. `listener_lock_actor(listener_id)` takes a share lock on the actor row; an inactive or missing
   actor is `unauthenticated`.
2. `INSERT INTO listener_state … ON CONFLICT DO NOTHING`, then `SELECT … FOR UPDATE`: the account
   row serializes every mutation, first write and duplicate claim of one listener.
3. Prior receipt for `(listener_id, operation_id)`: same request digest → commit and return it;
   different digest → `idempotency_key_reused`, no effect.
4. Device sequence for `(listener_id, device_id, resource_key)`: a sequence not above the
   high-water mark → `stale_device_sequence`, no effect.
5. `listener_order` = account revision + 1; time from `clock_timestamp()`.
6. A new progress key or a first Saved desire requires current-public eligibility; then the pure
   decision (`reconcile_progress`, `reconcile_saved`, `HistoryState::start/clear`) runs on the
   locked current value.
7. An accepted change is a conditional write (`… WHERE revision = $base`; triggers
   `guard_listener_progress`/`guard_listener_saved` refuse identity changes, a revision step
   other than +1 and a non-increasing order), followed by a conditional `listener_state` bump.
8. The sequence high-water mark advances and the receipt (accepted, already current or conflict)
   is inserted into the immutable `listener_operations`.
9. New identities are re-admitted with fresh database time, then the transaction commits.

Any error before commit rolls back every effect, receipt and sequence. Device time is never stored
as ordering; `accepted_at_ms` and the revision come from the server.

## `reconcile_progress`

```rust
// Actual decision, abridged. classify is the Kani-checked kernel in domain/reconcile.rs.
let completion = match candidate.kind {
    Restart => InProgress,
    MarkCompleted => Completed,
    Checkpoint | ExplicitSeek | ResolveHere | ResolveOther => current.map_or(InProgress, |c| c.completion),
};
let same = current.is_some_and(|c| c.position_ms == candidate.position && c.completion == completion);
let protected = current.is_some_and(|c| c.kind != Checkpoint);
match classify(candidate.base, current_revision, same, protected, candidate.kind == Checkpoint)? {
    Accept(next) => Accepted(record at revision next),
    AlreadyCurrent => AlreadyCurrent(current),
    KeepServer | ListenerChooses => Conflict { current, resolution: KeepServer if protected && checkpoint, else ListenerChooses },
}
// classify: base == current → Accept(current + 1) (RevisionExhausted at i64::MAX);
//           current > 0 && same → AlreadyCurrent; current > 0 && protected && checkpoint → KeepServer;
//           otherwise ListenerChooses.
```

An audio identity different from the current record's is `BindingMismatch` (`invalid_request`).
Two refinements are deliberately absent. **No max-position merge**: position never participates
in the accept/conflict classification except the equality short-cut. **No self-chain
fast-forward**: accepting a stale base when every later revision came from the same device would
remove spurious self-conflicts after a client crash, but the client rule "mark sent before
sending" already prevents them; add the refinement only with its own table rows and properties.

## Conflict table

Episode `Một lời hẹn` (`pub_7f3a`, 26:09) of `Ánh đèn cuối sân khấu`. Device A is an Android
phone, B is Theatre Web on a laptop. Times are shown as mm:ss; fixtures use integer
milliseconds. Rows map to the implemented decision; the domain and application tests named in
§ Testing cover each class, not each row by ID.

| # | Server current | Candidate | Expected | Why |
|---|---|---|---|---|
| C1 | none | A seq 1, base 0, checkpoint 02:00 | `accepted` r1 @02:00 | nothing to overwrite |
| C2 | r1 by A @02:00 | A seq 2, base r1, checkpoint 05:00 | `accepted` r2 | base is current |
| C3 | r2 by A @05:00 | A seq 3, base r2, explicit seek 01:10 | `accepted` r3 @01:10 | backward seek is valid |
| C4 | r3 | the C3 operation again | replay of the C3 receipt; store unchanged | idempotency |
| C5 | r3 | C3's operation ID with position 01:20 | `idempotency_key_reused`; no effect | a key binds one body |
| C6 | r3 | A, new operation ID, seq 3 | `stale_device_sequence`; no effect | monotonic sequence per key |
| C7 | r4 by B, checkpoint @15:00 | A seq 4, base r3, checkpoint 12:34 (offline on the bus) | `revision_conflict`, `listener_chooses`, current r4 | competing sessions; no blind overwrite |
| C7b | r4 by B, explicit seek @15:00 | A seq 4, base r3, checkpoint 12:34 | `revision_conflict`, `keep_server` | a delayed checkpoint never outranks an explicit act |
| C8 | r4 by B @15:00 | A seq 5, base r4, `resolve_here` 12:34 | `accepted` r5 @12:34 | chosen value against the current revision |
| C9 | r4 by B @15:00 | A seq 4, base r3, checkpoint 15:00 | `already_current` r4; no prompt | same value |
| C10 | r5 by A, `resolve_here` @12:34 | B seq 9, base r4, `resolve_here` 15:00 | `revision_conflict`, `listener_chooses` | a choice made before r5 is not a choice about r5 |
| C11 | r5 by A, `resolve_here` @12:34 | B seq 10, base r4, checkpoint 15:20 | `revision_conflict`, `keep_server` | decided in Library v1: explicit seek/restart/resolution/completion outranks a delayed checkpoint |
| C12 | r6 `mark_completed` @26:09 | A seq 6, base r6, `restart` 00:00 | `accepted` r7 @00:00, `in_progress` | restart and completion are distinct events |
| C12b | r5 checkpoint @25:00 | A seq 6, base r5, checkpoint 26:09 (the end) | `accepted` r6 @26:09, still `in_progress` | reaching the end is not completion |
| C13 | any | position 27:00 (> 26:09) | `invalid_position`; no effect | contract bounds |
| C14 | r4 by B @15:00 | as C7, with `device_reported_at_ms` one day in the future | identical to C7 | device clocks are context |
| C15 | none for `pub_9c21` (new release) | A seq 7, `pub_7f3a` (the release it played) | `accepted` under the `pub_7f3a` key if that audio is current-public, else `not_found`; `pub_9c21` untouched | progress is keyed per publication |

## Bookmarks and tombstones (proposed)

Nothing in this section is implemented. The only tombstones that exist today are
`listener_playback_sessions` rows (append-only, kept across history clear) and the revisioned
`saved = false` register. A bookmark would be `{bookmark_id, publication_id, position_ms, created_at}` with a client-generated ID
(a UUID variant is a candidate), stable across retries. Primary key `(account_id, bookmark_id)`,
so an ID collision across accounts cannot alias. Deletion writes a tombstone with the same key
and a change sequence. Clients sync with `GET …/bookmarks?since=<change_seq>`, which returns live
bookmarks and tombstones in change order.

| # | Server | Operation | Expected |
|---|---|---|---|
| B1 | none | create `bm-1` @03:15 | `Accepted`, change 41 |
| B2 | `bm-1` live | the B1 operation again | replay of B1 |
| B3 | `bm-1` live | delete `bm-1` | tombstone, change 42 |
| B4 | `bm-1` tombstone | the B1 operation replayed from an old outbox | replay of B1's original response; the client's next delta sync applies the tombstone |
| B5 | `bm-1` tombstone | create `bm-1` with a new operation ID (local restore) | `bookmark_deleted`; no resurrection |
| B6 | none | delete `bm-9` (never seen by the server) | tombstone; a later create of `bm-9` is refused |
| B7 | `bm-1` tombstone | delete `bm-1` again | idempotent; tombstone unchanged |

B4 is the subtle one: idempotency correctly replays a response that is no longer the current
state, so a client must apply delta sync after draining its outbox rather than trusting replayed
creates. Tombstones must outlive the longest offline period the product supports; garbage
collection needs a horizon rule that rejects older operations with `operation_expired`. Keeping
tombstones indefinitely is the simple default until a retention decision exists.

## Release changes

Progress stays keyed to the publication it was made on. When an episode gets a new active
release, the old key keeps its record. Continue and History expose the stored exact audio and a
closed `resume_status` (`exact_available`, `release_replaced`, `access_unavailable`) computed by
the pure `domain::resume_status`; a replaced release is never silently substituted, even with
byte-identical SHA-256, and a new release needs an explicit choice and a new progress key.
Superseded releases are `access_unavailable` until retained-release access is approved. Mapping
positions through dialogue or scene identity is optional future work that needs validation
before any automatic migration.

## Testing

### Example tests

Each conflict class is a test named as its theorem, asserting the exact outcome variant and the
store afterwards. Implemented in `listening/domain/tests.rs` (pure) and
`listening/application/tests.rs` (production `LibraryService` over a named
transactional-memory double, `mock-integration-tested` at most), for example
`current_revision_admits_legitimate_rewind_and_stale_write_preserves_it`,
`competing_sessions_require_choice_and_equal_stale_write_is_already_current`,
`end_position_is_not_completion_and_only_explicit_restart_resets_completion`,
`device_clocks_are_context_and_audio_replacement_never_changes_stored_identity`,
`ten_identical_retries_have_one_record_one_revision_and_one_receipt_after_restart` and
`changed_intent_key_and_old_sequence_are_exact_atomic_rejections`. Bookmark rows (B1–B7) have
no tests because bookmarks do not exist.

### Interleaving properties

Generate scripts for two devices — A on CMP, B on Web — of local intents (checkpoint, explicit
seek, restart, bookmark create and delete, go offline, crash and restart with the outbox intact)
and a network schedule that delivers, drops a request, drops a response after commit,
duplicates, delays one device behind the other and lets a simulated listener answer pending
choices. Run them through a test-only **reference client** that implements the contract above
and a **pure server model** composed of the operation log, the sequence guard and `reconcile`.
Bias toward long schedules; shrink the schedule, rebuilding state from it. proptest is the
candidate.

| Invariant | Statement |
|---|---|
| I1 No blind overwrite | every acceptance at revision r+1 had `base == r`, or there was no record |
| I2 Idempotence | an operation delivered k ≥ 1 times leaves the store as one delivery would, and every response is identical |
| I3 Revision order | per key, revisions increase by exactly one per acceptance; no two acceptances share one |
| I4 Conflicts carry the truth | every conflict's `current` equals the store at that moment |
| I5 Tombstones are final | once `bm-x` is tombstoned, no later state lists it live |
| I6 Clock independence | rewriting every `recorded_at` (skew, reversal) yields identical server responses |
| I7 No position ordering | replacing a stale candidate's position with any other value different from current leaves the outcome kind unchanged |
| I8 Drainage | once the network stops failing and choices are answered, every outbox empties within a bounded number of steps |

The property proves the **protocol** over reference models — `property-tested`. It does not
prove that the Kotlin or Leptos outbox implements the contract; those renderers run the same
schedules against their own outbox code.

**Implemented so far** (server model only; no reference client exists):
`revision_classification_matches_independent_partition` (full-u64 property of `classify`, I1/I3),
`arbitrary_two_device_schedule_accepts_only_observed_current_base` (two-device schedule over
`reconcile_progress`, I1/I3/I4/I7), `saved_protocol_schedule_matches_independent_desire_revision_oracle`
(Saved against an independent model) and the Kani harness
`progress_reconciliation_never_blindly_overwrites_or_wraps` over the production `classify`
(`cargo xtask verify kani --harness listener-reconcile`, `bounded-model-checked`). I2, I5, I6 and
I8 as generated properties, network fault schedules and a reference client are proposed; I2 and
I6 have example tests only.

### Real PostgreSQL

The pure model cannot show what the database does under concurrency. The implemented sources
are the `#[ignore]`d tests in `listening/postgres/tests.rs`, run only by
`cargo xtask verify postgres` against a disposable cluster (`integration-tested` only when that
gate actually ran): `duplicate_and_same_base_first_write_races_commit_one_revision_and_identical_receipts`,
`clear_preserves_progress_saved_and_sessions_and_router_is_account_isolated`,
`receipt_failure_rolls_back_progress_sequences_and_account_state_before_retry`,
`replacement_and_withdrawal_retain_owned_progress_without_silent_substitution`,
`first_write_and_save_unsave_races_preserve_one_authority_and_old_receipts`,
`snapshot_scalar_constraints_prevent_hidden_saved_and_completion_corruption` and
`latest_completed_release_suppresses_older_unfinished_work_without_erasing_either_register`.
The target matrix:

| Test | Setup | Assertion |
|---|---|---|
| same-base race | A and B write with base r4 on two connections; a barrier after both read | exactly one `Accepted` r5; the other `Conflict` carrying r5 |
| duplicate race | the same operation on two connections | one effect; both responses equal; no error leaks from the unique index |
| first-write race | two first writes, no row yet | one insert; the account-row lock serializes the loser, which is then reconciled |
| rollback | abort after the progress `UPDATE`, before commit | neither the progress row nor the operation log changed; a retry is a first attempt |
| lost response | commit, drop the response, retry | replay of the committed response |
| model agreement (proposed) | the interleaving generator at a small case count | every response equals the pure model's, step by step (`differentially-tested`) |

Name the fault points (after claim, after update, after commit before response; client after
append before send, after "sent" mark before transmit) and report them as `fault-injected`
only when a test actually crashed there.

### Across clients

The [050 review boundary](../../../../docs/work-plan/050-cmp-native-listening.md#review-boundary)
requires conflict handling demonstrated between a device and Theatre Web. Drive C7 → C8 with a
real phone or emulator and a real browser session against the real backend; that is the only
evidence for "cross-device sync" (`integration-tested` plus `device-tested` on the named device).

## Common mistakes

- Reconciling by `max(position)`, which erases every rewind ([invariant](../../../../docs/product/business-rules.md#product-invariants-to-verify)).
- `ORDER BY recorded_at`, or last-arrival-wins, which lets a delayed phone win.
- Minting the operation ID in the retry interceptor; coalescing an operation already sent.
- One global device sequence with several keys in flight, which rejects valid writes.
- Restoring a device ID from an OS backup, which turns two phones into one device.
- Treating a replayed receipt as current state (it is a historical receipt; reread after resolving).
- Treating a replayed create as proof the bookmark is live.
- Testing conflicts with an in-memory map and calling it `integration-tested`.

## Report additions

```text
Contract version:     progress/bookmark operations <version>; schema diff: …
Conflict scenarios:   C<n>… and B<n>… run by ID; needs-decision rows and their owning doc
Properties:           I1–I8 run? cases PR/nightly, seed; reference client + pure server model
PostgreSQL:           races and rollback run against <version>: yes | no
Fault points:         <named points actually crashed>
Cross-client demo:    <device or emulator> + <browser> against <backend build>, or "not run"
```
