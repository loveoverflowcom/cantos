# Theatre mock playback vectors v1

Contract: `theatre-mock-playback@1`; `vector_format: 1`. This is the bounded, requester-selected
**mock playback vocabulary and regression corpus**, shared as data between independent Rust and
Kotlin cores. It is not a listener HTTP DTO, publication manifest, production playback contract,
audio acceptance or generated language binding. No runtime/FFI is shared.

The [schema](../../../../schema/theatre/playback/v1.schema.json) owns the exact JSON shapes;
[index.json](index.json) lists every scenario and its exact SHA-256. The JSON files contain
hand-authored expected states/effects from the rules below; they are not output goldens generated
by either client reducer. Both clients must compare the complete expected state and the complete
ordered effects list, reject unknown versions/fields/types, and report each target that did not run.
Source edits change a reviewed corpus; update the index and both implementations together.

## Identity, state and units

A source contains a **demo fixture key**, `work_id` plus zero-based `episode_index`, and
`duration_ms`. Keys refer only to the three synthetic catalog works. They are not publication IDs
or evidence of availability, rights or backend authorization. There is no URL, credential, private
object key, real asset ID or Studio source. Future production source construction remains a
backend-authorized adapter boundary.

- Integers only: position/duration in milliseconds, speed in permille; `1250` means 1.25×.
- `generation` is a bounded unsigned 32-bit monotonic request identity. No wraparound: exhausted
  identity rejects transactionally. Every engine callback must match both the current source key
  and generation. A stale or unsolicited callback is an equal-state, zero-effect no-op.
- Root state is `{generation, session?}`. A session owns source, position, speed, listener intent,
  observed fake-engine phase and interruption. `status` and `pause_cause` in fixture snapshots
  are **derived projections**, never independent mutable state.
- Intent is `play` or `pause` with cause `user`, `interruption` or `cold_restore`.
- Engine is `preparing`, `ready`, `running`, `paused`, `buffering` or `ended`.
- Interruption is `none` or `active`.
- Derived status is `idle`, `loading`, `paused`, `simulating`, `buffering` or `ended`. Only a
  matching `engine_started` callback can first confirm `simulating`; every UI labels it mock/no audio.

## Event and effect vocabulary

User/application events: `source_selected`, `play_requested`, `pause_requested`, `seek_requested`,
`skip_requested`, `speed_requested`, `restore_requested`, `interruption_began`,
`interruption_ended {resume_allowed}`.

Fake-engine callbacks: `engine_ready`, `engine_started`, `engine_paused`, `engine_buffering`,
`engine_position`, `engine_ended`. Each carries `{work_id, episode_index, generation}` identity.
Position is additionally supplied by `engine_position`.

Effects: `prepare_engine`, `start_engine`, `pause_engine`, `seek_engine`, `set_speed`,
`offer_resume`, `rejected`. Every engine effect contains current identity, complete source,
position and speed, so it is executable without a stale earlier prepare command. `offer_resume`
is a UI suggestion only. `rejected` contains one closed reason: `no_session`, `interrupted`,
`out_of_bounds`, `unsupported_speed`, `generation_exhausted` or `invalid_source`.

## Bounded transition rules

1. Selecting a different valid demo source allocates a new generation, paused user intent and
   preparing engine; effect `prepare_engine`. Selecting the same source again is a no-op that
   preserves position. Unknown/unavailable catalog keys must be refused at the catalog boundary.
2. An accepted Play expresses intent and allocates a new generation with preparing engine;
   it emits `start_engine`. With no active interruption, when intent is already `play` and the
   engine is not `ended`, repeated Play is an equal-state, zero-effect no-op, even if a manual
   seek put position at duration.
   A matching `engine_started` confirms running; UI cannot infer it merely from the button press.
3. Pause allocates a new generation, user-paused intent/engine and a `pause_engine` effect. A
   repeated already-user-paused engine action is a no-op. Late started/ready callbacks cannot
   undo pause. An accepted new Play from pause intent or an observed ended engine resets position
   to zero only when position is exactly duration. Seek alone preserves intent/engine and never
   turns an already-play-intent request into a restart; Pause then Play, or a matching engine end
   then Play, establishes a new start. Play during an active interruption still rejects.
4. Seek rejects an absolute position beyond duration. Relative skip clamps to the bounds. Both
   preserve intent, allocate a generation and invalidate stale callbacks. During preparing they
   renew `prepare_engine` for pause intent or `start_engine` for play intent; on a confirmed engine
   they emit `seek_engine`, retaining its observed phase. No clock advances in the mock.
5. Supported speeds are 750/1000/1250/1500/2000. Changing speed emits `set_speed` without
   replacing the source/generation; unsupported speed returns identical state plus `rejected`.
6. Matching buffering preserves play intent; matching started recovers simulated running.
   Explicit pause always wins over late buffering/started. Matching ready may complete preparing
   but cannot roll a running/paused/ended engine back to ready.
7. Interruption begins once, allocates a generation, marks active and emits `pause_engine` while
   preserving intent. End allocates a generation and clears active. It may emit `start_engine`
   only when `resume_allowed` is explicitly true **and** intent remains play. Otherwise it stays
   paused. User pause during interruption remains final. The Web demo supplies false by default;
   this parameter describes a pinned mock scenario, not a production OS auto-resume policy.
8. Restore allocates a generation from a supplied source/position/speed snapshot, becomes
   cold-paused/preparing and emits ordered `[prepare_engine, offer_resume]`. It never emits start.
   This is an in-memory cold-restore scenario, not disk/account persistence or a release policy.
9. Matched engine end gives `ended`; it grants no completion/progress/rights assertion. Seeking
   to the end alone never marks a completed listen. Invalid input rejects transactionally, with
   one rejected effect and unchanged state. Stale callbacks produce no effects or errors.
10. A generation-changing command is self-contained. Play/seek/pause during preparation must
    establish a viable new-generation path, not wait forever for a discarded ready callback.

## Corpus and client evidence

- [play-pause](play-pause.json): intention, acknowledgement, repeat and late-start guard
- [buffering](buffering.json): stall/recovery and late callback after pause
- [interruption](interruption.json): user pause wins and explicit no-resume scenario
- [stale-response](stale-response.json): source identity plus generation rejection
- [restore-session](restore-session.json): no cold auto-start; explicit play acknowledgement
- [preparing-races](preparing-races.json): play/seek/pause while preparation is pending
- [invalid-controls](invalid-controls.json): transactional rejection, clamped skips and explicit restart

Each platform embeds exactly the index-listed files for tests and must fail on an unknown/unlisted
vector. Rust native/WASM and Kotlin/JVM/Android/iOS results are separate. Source/schema inspection
or one platform's result never counts as both reducers passing. Initial Web authoring is on a cloud
executor without Cargo/rustc/Trunk, so Web reducer/vector tests remain NOT_RUN until an authorized
supported toolchain runs them. Real browser/native engine/background/streaming remains future work.

The production owning constraints remain [Web listening](../../../../../../docs/work-plan/040-theatre-web-listening.md)
and [mobile playback](../../../../../../docs/architecture/mobile.md#playback-behavior).
The [API surface boundary](../../../../../../docs/decisions/0015-api-surface-boundaries.md) remains unchanged;
backend publication authorization never moves into a UI reducer. Saved Work, Bookmark, Listening
Progress and Downloaded Asset remain separate capabilities rather than playback flags.
