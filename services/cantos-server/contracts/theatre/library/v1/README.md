# Listener Library and Progress v1

Schema version **1.2.0**, additive opt-in resources under `/api/v1/theatre`. This reviewed core
contract is shared by Web/CMP; their current mock reducers are not production consumers.
JSON Schema, strict Rust DTO fixture conformance and HTTP tests accompany it. OpenAPI,
generated/drift-checked Kotlin and real-client adoption remain acceptance work.

## Resources

| Method | Resource | Meaning |
| --- | --- | --- |
| GET | `/library/continue` | one newest progress register per Work, excluding completed; no playback effect |
| GET | `/library/saved` | desired saved Works, newest accepted save first |
| GET, PUT | `/library/saved/{work}` | current revisioned desired state; conditional desired-state mutation |
| GET | `/library/history` | latest confirmed playback-start per Work, newest first, including completed |
| POST | `/library/history/playback-starts` | only an acknowledged actual engine start, not Play intent or page view |
| POST | `/library/history/clear` | explicitly confirmed conditional clear, only history |
| GET, PUT | `/progress/{publication}` | exact publication progress authority, conditional update/reconcile |
| GET | `/library/bookmarks` | live release-bound bookmarks, newest accepted first, with exact audio and required nullable metadata; no playback effect |
| PUT | `/library/bookmarks/{bookmark}` | create-once bookmark at an exact release position, client-minted ID |
| POST | `/library/bookmarks/{bookmark}/delete` | final tombstone of one bookmark; Saved, progress and history are untouched |
| GET | `/operations/{operation}` | original outcome receipt for unknown/lost mutation response |

Account-owned routes require a separate Listener authenticator. Studio cookies, development
session tokens and raw caller actor IDs confer no authority. The default adapter denies all
requests before storage. A concrete issuer verifier must verify signature, issuer, audience,
expiry and session revocation, then resolve the verified issuer/subject to the existing actor
registry. Mapping alone does not authenticate. #34 has no accepted guest-account merge policy:
no guest persistence/import or automatic guest merge is implemented.

GET progress returns the strict `{"resource":"progress","current":record|null}` envelope;
GET Saved returns its register (including false revision0 when absent), not a mutation receipt.
Unknown operation receipts return not_found in the authenticated account scope.

## Presentation metadata v1.1.0 (explicit opt-in)

[#251](https://github.com/loveoverflowcom/cantos/issues/251) adds the closed query
`presentation=1` to the Continue, Saved and History **page reads**. Omitting it retains the
exact default v1 shapes, including strict decoding by current clients. Other values return
`invalid_request`. Cursor identity binds the opt-in as well as the existing account, route,
limit and history state; changing it during a walk returns `invalid_cursor`.
No mutation, receipt, individual progress read or individual Saved register changes shape.

The separate `PresentedContinuePage`, `PresentedSavedPage` and `PresentedHistoryPage` contain
the same register/audio/resume fields as their default counterparts plus one **required,
nullable** `presentation` on each item. Null retains the owned record without revealing
whether a private, withdrawn, expired or absent publication exists. Missing is not null and
does not conform to the contract.

- `WorkPresentation` contains only `work_id`, the metadata source `publication_id`, released
  Work `title`, `kind` (`novel|story|podcast`) and actual released audio `language`.
- `AudioPresentation` contains `work:WorkPresentation`, `unit_id` and released `unit_title`.
  Continue/History use this only for the **exact stored publication**, after current public
  eligibility is rechecked; unavailable or superseded audio has `presentation:null`. Another
  unit or a replacement release never supplies fallback metadata for stored audio.
- Saved uses `WorkPresentation` from the same deterministic currently eligible representative
  as public Work discovery. If none is eligible, its retained Saved register has
  `presentation:null`. The representative's `publication_id` makes that source explicit.

These projections never expose editable head titles, storage keys, signed URLs, ownership,
rights/QC evidence, provider data or other private fields. They are read snapshots, not new
stored registers or authorization: metadata may disappear or change on refresh as eligibility,
rights-window time or the current public representative changes. Continue/History preserve
their exact `AudioIdentity`, resume status and progress even when metadata is null; no automatic
publication substitution, progress migration or playback is introduced.

Schemas and fixtures use separate `Presented*` definitions because adding even an optional
response field breaks existing strict readers. This capability is additive only for explicitly
opted-in callers; the API change itself switched no client. The CMP Library client adopts the
representation in [#297](https://github.com/loveoverflowcom/cantos/issues/297) (JVM/source evidence; a
Page whose metadata names another record or release is a protocol failure). #177's genres,
credits, seasons and numbering remain separate follow-ups.

## Release-bound bookmarks v1.2.0

[#299](https://github.com/loveoverflowcom/cantos/issues/299) adds a **bookmark**: an immutable
`(bookmark_id, exact AudioIdentity, position_ms)` value owned by the authenticated Listener. It
is a different resource from Saved Work (a Work-level desire) and from progress (one revisioned
register per release); creating, deleting or listing bookmarks never changes either, and Save/Unsave,
progress, history and clear-history never create or delete a bookmark. There is no label, note,
edit, playlist or ordering other than acceptance order.

- **Create** `PUT /library/bookmarks/{bookmark}` carries the standard operation envelope, the
  same `bookmark_id` as the path, the exact audio identity and `position_ms` in `[0,duration_ms]`.
  A new bookmark requires the audio to be **currently public**, exactly as a new progress key does;
  position is checked against the identity's verified duration. A bookmark grants no listening
  permission: playback still goes through Discovery's authorization for the exact publication.
- **Outcomes.** A first accepted create returns `accepted`. The same ID with an identical live
  value returns `already_current` (a replayed intent with a new operation ID cannot duplicate).
  The same ID with a different value is `idempotency_key_reused`. An ID whose tombstone exists
  returns `409 bookmark_deleted` (resolution `keep_server`, `deleted:true`): **a replayed or
  re-sent create never resurrects a deletion**. A replayed operation ID returns its stored original
  receipt even after the bookmark was deleted, so a client applies a list read after draining its outbox
  and never treats a replayed create as proof the bookmark is live.
- **Delete** `POST /library/bookmarks/{bookmark}/delete` turns a live bookmark into its tombstone
  (`accepted`, `deleted:true`); a repeat is `already_current`. A bookmark ID the account never created
  is `404 not_found` and writes nothing: a client's outbox is FIFO per bookmark key, so a delete can
  never overtake its own create. Tombstones are retained indefinitely (no horizon is approved).
- **List** `GET /library/bookmarks?limit=1..50&cursor=` returns live bookmarks only, descending
  acceptance order then ID, with the same opaque, account/route/limit/frontier-bound cursor as the
  other pages. Each item carries `resume` and a **required, nullable** `presentation` computed only for
  the **exact stored release** after current eligibility is rechecked: replaced, withdrawn or private
  audio keeps the bookmark with `presentation:null` and `release_replaced`/`access_unavailable`, and
  no newer release ever supplies fallback metadata or position. Opening a bookmark is a client
  action that names the stored identity; the API never substitutes a release.
- Sequence and receipt rules are unchanged: resource key `bookmark:<id>` (device sequence per key),
  immutable body across retries, receipts committed atomically with the effect.

Fixtures live in `fixtures/theatre/library/v1/bookmarks/` (a separate corpus so the default one,
and its strict readers, are unchanged); the schema/DTO additions are strictly additive:
`WriteOutcome::bookmark_deleted` and `MutationState::bookmark` occur only for bookmark operations.

## Identity, concurrency and receipts

Work and ContentUnit IDs are stable continuity. Publication, rendition, SHA-256 and duration
bind **exact audio timing**. Progress is keyed by Listener + ContentUnit + Publication;
Continue is a projection of these registers, never a second progress store. No signed URL
is stored. Account ownership is derived from authenticated context, never a request field.

All mutations carry a canonical UUID operation ID minted and retained at intent, stable UUID
device ID, positive per-device/per-resource sequence, and immutable body across retries. The
same Listener + operation ID with identical canonical request replays the original response;
a changed route/body is `idempotency_key_reused`. Receipts (including semantic conflicts),
sequence high-water and effects commit atomically. No receipt expiry/GC is implemented: retain
indefinitely until an approved horizon exists. Transport failure may mean outcome unknown:
read the receipt or resend the identical operation, never mint a replacement key blindly.

`base_revision=0` means no existing register. Revision increments exactly one on acceptance,
bounded at signed 64-bit integer maximum. Stale divergent progress returns the current
record and explicit `listener_chooses`, or `keep_server` for a delayed checkpoint after explicit
seek/restart/resolution/completion. There is no max-position, arrival-time, device-clock or
same-device fast-forward merge. Reconciliation is a new explicit resolution operation against
the current revision; it is never an automatic seek. Same stale position/completion can return
`already_current`, preserving the current revision. Reusing a device sequence for a new
operation is rejected. Device-reported timestamps are context only.

Saves name only a Work and `saved=true|false`; they never toggle. A stale divergent desire
conflicts with current state; a repeated same desire is `already_current`. Unsave retains its
revisioned false record and never erases progress/history/bookmarks/downloads. Cards/details
read the same register. Replayed responses are historical receipts, not current state: clients
fence responses by account/session and local intent generation, ignore older server revisions
and reread after resolving outbox entries. Logout clears account projections, pending views
and credentials; account A's queued operations must never be submitted as B.

## Playback, completion, history and clear

Positions are integer milliseconds in `[0,duration]`. Checkpoint or seek to end does not imply
completion. `mark_completed` is an explicit listener intent at the exact end; `restart` requires
zero and resets completion. Completed progress stays in history and remains individually readable,
but is excluded from Continue. Backward `explicit_seek` is valid on the current revision.

The history-start contract has a closed `event=playback_started`, an engine-session UUID,
exact audio identity and the last observed history epoch. It must be emitted only after the
real engine has acknowledged actual start for the current account/session/source generation.
The server cannot prove physical playback from this client acknowledgement; runtime/native
adapter evidence is required. Discovery/detail/grant/progress/saved requests have no history
side-effect. Duplicate session starts cannot reorder history, even with a new operation ID.

Clear requires `confirmation=clear_history`, the observed history revision and epoch. It
increments the history epoch and revision, removes history entries only, and retains the seen
engine-session tombstones. Delayed pre-clear acknowledgements conflict as `history_epoch_conflict`;
replayed old starts return their old receipt without recreating entries. New actual starts with
the new epoch may create history. Concurrent new starts invalidate a stale clear revision and
must be shown for fresh confirmation. No analytics pipeline is introduced.

## Resume and changed access

Continue/history expose stored exact audio, server revision and a closed resume status:
`exact_available`, `release_replaced`, or `access_unavailable`. A replaced active release is
never silently substituted, even with byte-identical SHA. Until approved retained-release
access exists, superseded exact releases are unavailable to resume. New-release offers require
an explicit listener choice and a distinct progress key; no automatic migration/clamping.

When current playable access disappears, accepted progress/history/Saved are retained. A
checkpoint for an already owned exact audio register can still be recorded, but a new key
requires current-public eligibility. History requires current-public eligibility on a new
actual start. Playback bytes and per-fetch revocation remain Discovery/Publication-owned.
Read responses grant no playback permission and never autoplay on page open or sign-in.

## Pages, errors and recovery

Pages are bounded `limit=1..50` (default20). Opaque random 256-bit server-side cursor references
bind route/version/Listener/limit/history epoch, immutable ordering frontier and a 15-minute
expiry. No internal IDs/base64 payload appears in the token. Process restart expires cursors;
restart paging. Keyset order is descending accepted Listener order then C-collated Work UUID.
A scan freezes an upper order bound; updated/deleted items can disappear from that scan, and
new/updated items above the bound appear on refresh. Unchanged continuously eligible records
appear exactly once. Every page rechecks ownership and current access.

Responses are no-store. Closed server errors map exhaustively: invalid_request400,
invalid_cursor400, unauthenticated401, not_found404, invalid_position422,
idempotency_key_reused422, stale_device_sequence409, revision_exhausted409,
unavailable503/retryable, corrupt_state500/non-retryable. Mutation conflicts return409 with
original operation ID, outcome, current revisioned state and explicit resolution. Successful
receipts carry accepted/already_current and original outcome; retry replay has identical bytes.
