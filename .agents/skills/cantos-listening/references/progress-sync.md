# Progress and bookmark sync

> **Scope.** Implement and verify durable listening progress and bookmarks across Theatre Web
> and the CMP app: the local-first outbox contract every client obeys, stable device IDs,
> monotonic sequences and idempotency keys, server revisions, the append-only operation log, the
> conditional write, the pure `reconcile` decision, explicit choice for competing sessions,
> bookmark tombstones, release changes, concrete conflict tables, two-device interleaving
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

## What is synchronized

| Thing | Shape | Key | Changes by |
|---|---|---|---|
| Progress | one current value with a server revision | account + episode + immutable publication | conditional replacement; every accepted write appended to the operation log |
| Bookmark | an immutable value | account + client-generated bookmark ID | create once; delete by tombstone; "move" is delete plus create |

Progress is a revisioned register, not a counter: a rewind is a legitimate write. Bookmarks are a
set of immutable values, which removes edit conflicts by construction; an editable label, if the
product adds one, is a conditional update with its own base revision.

## Types — proposed

```rust
// Illustrative, proposed. Pure values; the server clock and the device clock stay in the shell.
pub struct ProgressKey { account: AccountId, episode: EpisodeId, publication: PublicationId }

pub struct ProgressRecord {
    key: ProgressKey,
    position: Position,             // integer milliseconds, ≤ the publication's duration
    completion: Completion,         // decided by the recorded completion rule, never by "ended"
    revision: ProgressRevision,     // server-assigned, +1 per accepted write
    written_by: DeviceId,
    kind: WriteKind,
}

pub struct ProgressCandidate {
    operation: OperationId,         // idempotency key, minted at intent time
    device: DeviceId,               // stable per install, excluded from OS backup
    sequence: DeviceSequence,       // monotonic per device and key
    base: Option<ProgressRevision>, // last revision this device reconciled; None before any
    position: Position,
    completion: Completion,
    kind: WriteKind,
}

pub enum WriteKind { Checkpoint, ExplicitSeek, Restart, Resolution(Choice) }
pub enum Choice { ContinueHere, UseOtherDevice }

pub enum ProgressOutcome {
    Accepted(ProgressChange),
    AlreadyCurrent(ProgressRecord),
    Conflict { current: ProgressRecord, resolution: ConflictResolution },
}

pub enum ConflictResolution { ListenerChooses, KeepServer(KeepReason) }
```

`ProgressCandidate` has no timestamp field. The device's `recorded_at` travels in the request and
lands in the operation log as context, but `reconcile` never receives it, so it **cannot** order
writes by an untrusted clock. That is a type-level guarantee, stronger than a review comment
([types as proofs](../../cantos-engineering/references/types-as-proofs.md)).

## The client contract

Every client — the Leptos Theatre and the CMP app — must satisfy these obligations. They are
part of the listener contract; how each renderer stores and schedules them is its own skill.

1. **Local first.** Record the position locally, then append an operation to a durable outbox
   in the same local transaction. Playback never waits for the server.
2. **Mint identity at intent time.** `operation_id`, `device_id` and the next `sequence` are
   assigned when the operation is appended, never by the HTTP retry layer.
3. **Mark "sent" before sending.** Persist that an operation has been transmitted *before* the
   request leaves. A sent operation may already be committed; from then on it is immutable and
   resent byte-for-byte until its outcome is known.
4. **Coalesce only the unsent.** A never-sent checkpoint or explicit seek for a key may be
   superseded by a newer one (new operation ID). Never coalesce across a `Restart`, a completion
   change or a `Resolution`, and never touch a sent operation.
5. **One in flight per key.** Send the oldest unresolved operation for a key; send the next only
   after the first is resolved. This keeps each device's own writes in order without a server
   merge. Different keys may be in flight concurrently, which is why sequences are per key.
6. **Base means reconciled, not fetched.** `base_revision` is the latest revision this device
   has *reconciled* for the key — its own accepted write, an adopted server value or a resolved
   choice — fixed when the operation is first sent. Fetching a newer revision while a candidate
   is pending does not rebase the candidate; the conflict that follows is the correct outcome.
7. **Bounded cadence.** Checkpoint at a configured interval while playing and on pause, stop,
   seek, end and backgrounding where lifecycle permits. Never per second. The interval is a
   named setting recorded with its rationale, so a property can bound writes.

| Response | Client action |
|---|---|
| `Accepted`, replayed duplicate, `AlreadyCurrent` | store the returned revision as the reconciled base; resolve the operation |
| `Conflict` + `listener_chooses` | keep the local candidate; store the server's current record; show the choice; send nothing more for this key until it is made |
| `Conflict` + `keep_server` | drop the candidate; adopt the server value as base; during playback, offer it — never seek ([playback R5](playback-semantics.md#rules-the-reducer-encodes)) |
| network failure, timeout, `5xx`, `429` | resend the identical body with backoff and jitter |
| `invalid_position`, `idempotency_key_reused` | quarantine the operation and report a client defect; never loop |
| `stale_device_sequence` | the install is likely a restored clone: mint a new device ID; rejected operations had no effect, so re-mint them |
| `operation_expired` | discard; resync the key from the server |

## Competing sessions: an explicit choice

When no automatic rule preserves clear intent, the docs require the listener to choose. Show
both positions in readable time and submit the choice as a new `Resolution` operation against
the **current** server revision:

```text
Tiến độ trên thiết bị khác đã thay đổi.
  [ Tiếp tục tại đây · 12:34 ]      → Resolution(ContinueHere), position 754000, base r4
  [ Dùng vị trí trên thiết bị khác · 15:00 ] → Resolution(UseOtherDevice), position 900000, base r4
```

Both choices are written, so other devices learn that an explicit choice exists. Choosing the
other device's position is a listener gesture, so the playhead may move. Copy, layout, focus and
announcements belong to [`cantos-ui-design`](../../cantos-ui-design/SKILL.md); what dismissing
the prompt means is an open product decision.

## The server write

One transaction, in this order. Claiming the operation ID first makes a concurrent duplicate
wait on the unique index and then replay, instead of racing to a false conflict.

```rust
// Illustrative, proposed shell. The decision in the middle is pure.
pub async fn put_progress(db: &Db, access: &ListenerAccess, write: ProgressWrite)
    -> Result<WriteResponse, ProgressError>
{
    let mut tx = db.begin().await?;
    match operation_log::claim(&mut tx, access.account(), &write).await? {
        Claim::New => {}
        Claim::SameRequest(stored) => return Ok(stored),
        Claim::DifferentRequest => return Err(ProgressError::IdempotencyKeyReused),
    }
    device_sequence::advance(&mut tx, access.account(), write.device(), &write.key(), write.sequence())
        .await?; // Err(StaleDeviceSequence) when sequence ≤ high water
    let current = progress::lock_current(&mut tx, &write.key()).await?; // SELECT … FOR UPDATE
    let outcome = reconcile(current.as_ref(), write.candidate());
    let response = progress::apply(&mut tx, &write.key(), &outcome).await?;
    operation_log::complete(&mut tx, access.account(), write.operation_id(), &response).await?;
    tx.commit().await?;
    Ok(response)
}
```

```sql
-- Illustrative, proposed. The claim never aborts the transaction on a duplicate.
INSERT INTO progress_operations (account_id, operation_id, request_digest, device_id, recorded_at)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (account_id, operation_id) DO NOTHING
RETURNING operation_id;
-- no row returned → SELECT request_digest, response FROM progress_operations WHERE …

-- The accepted change is conditional even under the row lock: a second barrier, not a habit.
UPDATE listening_progress
   SET position_ms = $4, completion = $5, revision = revision + 1,
       written_by = $6, kind = $7, accepted_at = now()
 WHERE account_id = $1 AND publication_id = $2 AND revision = $3
RETURNING revision, accepted_at;
```

A first write uses `INSERT … ON CONFLICT DO NOTHING`; when a concurrent first write wins, retry
the whole transaction once, after which `current` exists and `reconcile` decides. The operation
log is append-only: rows are completed once and never rewritten; duplicates replay from it. Its
retention must cover the clients' maximum retry horizon, a decision to record.

`accepted_at` and the revision come from the server. Device time is stored only as
`recorded_at` context in the log.

## `reconcile`

```rust
// Illustrative, proposed. Total over its inputs; no clock, no I/O.
pub fn reconcile(current: Option<&ProgressRecord>, candidate: &ProgressCandidate) -> ProgressOutcome {
    let Some(current) = current else {
        return ProgressOutcome::Accepted(ProgressChange::first(candidate));
    };
    if candidate.base == Some(current.revision) {
        return ProgressOutcome::Accepted(current.advanced_by(candidate));
    }
    if current.position == candidate.position && current.completion == candidate.completion {
        return ProgressOutcome::AlreadyCurrent(current.clone());
    }
    ProgressOutcome::Conflict { current: current.clone(), resolution: resolve_stale(current, candidate) }
}

fn resolve_stale(current: &ProgressRecord, candidate: &ProgressCandidate) -> ConflictResolution {
    match (current.kind, candidate.kind) {
        // needs-decision: "prefer an explicitly selected resume position" read as an explicit
        // choice outranking a checkpoint produced without seeing it. Confirm in the doc first.
        (WriteKind::Resolution(_), WriteKind::Checkpoint) => ConflictResolution::KeepServer(KeepReason::ExplicitChoice),
        _ => ConflictResolution::ListenerChooses,
    }
}
```

Two refinements are deliberately absent. **No max-position merge**: position never participates
in the accept/conflict classification except the equality short-cut. **No self-chain
fast-forward**: accepting a stale base when every later revision came from the same device would
remove spurious self-conflicts after a client crash, but the client rule "mark sent before
sending" already prevents them; add the refinement only with its own table rows and properties.

## Conflict table

Episode `Một lời hẹn` (`pub_7f3a`, 26:09) of `Ánh đèn cuối sân khấu`. Device A is an Android
phone, B is Theatre Web on a laptop. Times are shown as mm:ss; fixtures use integer
milliseconds. Each row is a named example test of `reconcile` plus the shell, and a fixture
under `packages/contracts/` (proposed) so client response handling decodes the same cases.

| # | Server current | Candidate | Expected | Why |
|---|---|---|---|---|
| C1 | none | A seq 1, base none, checkpoint 02:00 | `Accepted` r1 @02:00 | nothing to overwrite |
| C2 | r1 by A @02:00 | A seq 2, base r1, checkpoint 05:00 | `Accepted` r2 | base is current |
| C3 | r2 by A @05:00 | A seq 3, base r2, explicit seek 01:10 | `Accepted` r3 @01:10 | backward seek is valid |
| C4 | r3 | the C3 operation again | replay of the C3 response; store unchanged | idempotency |
| C5 | r3 | C3's operation ID with position 01:20 | `idempotency_key_reused`; no effect | a key binds one body |
| C6 | r3 | A, new operation ID, seq 3 | `stale_device_sequence`; no effect | monotonic sequence per key |
| C7 | r4 by B @15:00 | A seq 4, base r3, checkpoint 12:34 (offline on the bus) | `Conflict`, `listener_chooses`, current r4 | competing sessions; no blind overwrite |
| C8 | r4 by B @15:00 | A seq 5, base r4, `Resolution(ContinueHere)` 12:34 | `Accepted` r5 @12:34 | chosen value against the current revision |
| C9 | r4 by B @15:00 | A seq 4, base r3, checkpoint 15:00 | `AlreadyCurrent` r4; no prompt | same value |
| C10 | r5 by A, resolution @12:34 | B seq 9, base r4, `Resolution(ContinueHere)` 15:00 | `Conflict`, `listener_chooses` | a choice made before r5 is not a choice about r5 |
| C11 | r5 by A, resolution @12:34 | B seq 10, base r4, checkpoint 15:20 | `Conflict`, `keep_server` — **needs-decision** | prefer an explicitly selected position |
| C12 | r6 completed @26:09 | A seq 6, base r6, `Restart` 00:00 | `Accepted` r7 @00:00; completion per the recorded rule | restart and completion are distinct events |
| C13 | any | position 27:00 (> 26:09) | `invalid_position`; no effect | contract bounds |
| C14 | r4 by B @15:00 | as C7, with `recorded_at` one day in the future | identical to C7 | device clocks are context |
| C15 | none for `pub_9c21` (new release) | A seq 7, `pub_7f3a` (the release it downloaded) | `Accepted` under the `pub_7f3a` key; `pub_9c21` untouched | progress is keyed per publication |

A row marked **needs-decision** ships only after the owning doc records the rule; until then
the implementation returns `listener_chooses` for it and the test says why.

## Bookmarks and tombstones

A bookmark is `{bookmark_id, publication_id, position_ms, created_at}` with a client-generated ID
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

Progress and bookmarks stay keyed to the publication they were made on. When an episode gets a
new active release, the old key keeps its record, and the client applies the
[owning rule](../../../../docs/architecture/mobile.md#durable-listening-progress): resume the prior
release if access allows, otherwise explain that the audio changed and let the listener choose a
start. Compare playable-asset checksums, exposed in the listener projection, to tell whether the
audio actually changed; carrying a position across byte-identical audio is mechanically safe but
still a recorded decision. Mapping positions through dialogue or scene identity is optional
future work that needs validation before any automatic migration.

## Testing

### Example tests

Each conflict and tombstone row is a test named as its theorem, asserting the exact outcome
variant and the store afterwards: `stale_checkpoint_from_offline_phone_returns_current_and_listener_choice`,
`backward_seek_on_current_base_is_accepted`, `replayed_create_after_tombstone_never_resurrects`.

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

### Real PostgreSQL

The pure model cannot show what the database does under concurrency. Run against a real,
disposable PostgreSQL (`integration-tested`):

| Test | Setup | Assertion |
|---|---|---|
| same-base race | A and B write with base r4 on two connections; a barrier after both read | exactly one `Accepted` r5; the other `Conflict` carrying r5 |
| duplicate race | the same operation on two connections | one effect; both responses equal; no error leaks from the unique index |
| first-write race | two first writes, no row yet | one insert; the loser retries once and is reconciled |
| rollback | abort after the progress `UPDATE`, before commit | neither the progress row nor the operation log changed; a retry is a first attempt |
| lost response | commit, drop the response, retry | replay of the committed response |
| model agreement | the interleaving generator at a small case count | every response equals the pure model's, step by step (`differentially-tested`) |

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
