# Theatre Web account publication boundary

This is the implementation boundary for [#362](https://github.com/loveoverflowcom/cantos/issues/362),
based on `refactor/cantos-native-turso@34c78b5c43afb0d83bb855ddcb36972aeff7ee20`.
The current source includes #360/#361 through merged #368; the historical audit SHA is not the
implementation base. GitHub retains acceptance/status and the final PR records exact commands,
source identity, check outcomes and remaining acceptance. The work queue order is unchanged.

## Owner and outcome

`AccountWriter` is the host-compilable production signal writer called by `live/runtime.rs` and
the shell callbacks. It returns `Write::Disposed` separately from
`Write::Done { value, published: Published::No | Published::Yes }`. Its private write helper uses
the pinned `try_maybe_update`; the signal guard is released before returning effects. The browser
adapters extract the value independently of `published`, so no-publication does not cancel
admitted effects. Domain bool/Option results and admitted/refused status never stand for change.

`AccountViewState` and `LibrarySyncState` own the comparison facts. Lifecycle/storage writers
compare small control facts; other cold capabilities compare owned presentation snapshots.
These cold snapshots still copy Library rows and reader registers; there is no claim that all
cold-write allocation or projection work is optimized. Playback compares at most its source
publication and supplied presentation keys plus the History command. It does not clone the
account, outbox, register maps or Library. In particular, absent versus newly created register
entries and presentation changes remain observable even without a minted operation.

Playback retains unconditional Library reconciliation: paging can invalidate a projection even
when confirmed sync records are unchanged. `sync_projection` now reports actual observable
assignments/removals/insertions without a bulk Library snapshot; playback combines that result
with its bounded register comparison. The reconciliation decisions and ordering stay the same.
The finite matrix compares against unconditional production reconciliation, including populated
Saved/Continue restart failures and a History restart still loading. The independent reviewer
found the earlier skipped-reconciliation regression; a dedicated writer test reproduced it before
the conservative correction. No projection-skip optimization or reduced projection-work claim
is made by this slice.

## Closed non-presentation state

The exhaustive struct patterns in `library_sync/publication.rs` require each new field to be
classified. Reader progress facts retain record, read-resolution/in-flight flags, command and
presentation; only `unsent` and `last_checkpoint_at_ms` are excluded at that entry boundary.
They control coalescing/throttle, are absent from the durable journal and have no view reader.

At the sync owner boundary, actor/device, sequence and pending-operation bookkeeping belongs to
admission/journal/effect execution; actor/session admission is observed at the account owner.
Active engine session, acknowledged sessions and queued starts control History minting, while
resume generation controls ticket identity. Their visible results are History command/records
and resume attempt. These fields remain in full state equality, fences, durable snapshots and
effect values. No-publication never means that owned state or its required effects are unchanged.
The effect interpreter continues to persist before send, preserve unknown outcomes and recheck
fences after storage reentry and awaited I/O. Disposal starts no new work and erases no durable
operation. No journal format, auth adapter, dependency, API/protocol or preview gate changes.

`ErrorWindow` reads the active ErrorCenter notice. `error_writer::update_errors` compares that
notice before notifying; duplicate reports, irrelevant resolutions and queue-only changes keep
the subscriber asleep. Queue promotion, dismissed-episode rules and selected Retry/SignIn
actions retain their existing semantics. Selected effects are returned after guard release.

## Independent evidence ledger

| Claim | Actual production boundary and independent oracle |
| --- | --- |
| Unchanged/refused/stale/duplicate writes do not notify | Production writer with `AtomicUsize` subscriber counters; exact full-state equality for atomic cases |
| Changed reader facts publish once | Independent test-only `reader_facts` built from actual getters, compared before/after every matrix write; exact 0/1 subscriber reruns |
| State change differs from observable change | Throttled progress has unequal full state, equal reader facts and equal journal; queued engine bookkeeping and resume ticket generation have named regressions |
| Logout false can publish | Dedicated cleanup-in-flight failure followed by a full-state-equal duplicate |
| Tick/mint/seek/flush stay distinct | Synthetic long immutable release; throttle, accepted receipt, duplicate receipt and subsequent mint sequences |
| Effects survive no-publication | Repeated same-target Resume yields distinct valid effects/tickets, interpreted once each outside the guard; earlier ticket is rejected |
| No refetch loop on error/retry | Actual writer plus sync-admission memo and counted fake read port; explicit retry is separate from account notification |
| Disposal and guard order preserve operations | Actual writer storage/send callbacks, storage reentry/disposal, unchanged durable ID/payload and receipt-first restoration |
| ErrorCenter only notifies active changes | Actual error writer subscriber counter, duplicate/queue/promote/dismiss/resolve cases and selected retry effect |

The finite Cartesian matrix contains 25 fixture states × 52 calls × current/stale fence × two
repetitions (5,200 writes). It enumerates every state-writing production writer method and all
five playback observations. It includes idle/checking/guest, signed-in empty/populated/loading,
restore blocked, missing sync, exhausted generation, cleanup in flight/retry, Saved pending/
unknown/failed/conflict, progress pending/read/coalesced/conflict, History confirmation/pending/
failure, Resume preparing and populated paging invalidation. Its explicit key universe includes every entity key writable by
those fixtures/actions, secondary and absent keys. It observes Option presence, account controls,
all three Library lists/state/section phase/has-more, Saved entries, progress reader fields,
History command/confirmation/state and Resume attempt. It calls no production snapshot helper.
This is deterministic finite `property-tested` evidence, not an unbounded or randomized proof.

The initial unchanged-cancel regression failed on the fetched base: full state stayed equal but
the subscriber ran twice instead of once. An earlier test attempt incorrectly treated idle
Resume reset as full-state-equal; reset advances its hidden ticket generation. A later playback
test initially used a one-second fixture and was correctly clamped; its synthetic long release
now exercises the intended throttle boundary. These were test-oracle corrections.

## Limits and deferred acceptance

Host counters establish reactive-graph publication behavior, not browser CPU, latency, focus,
DOM identity, cache behavior, real auth/API/durability or native media acceptance. No markup,
style, copy or component contract changes. Browser performance/focus, DB, native runtime and
Kani are separate unrun layers unless the final PR supplies actual evidence. No PostgreSQL
installation/start, production write, real email, deployment or merge is part of this slice.
Rollback is a source revert; durable operations remain compatible.
