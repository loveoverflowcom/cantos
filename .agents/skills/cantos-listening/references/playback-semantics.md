# Playback semantics

> **Scope.** Model what the Theatre player means — on Web and on Android/iOS — as one closed
> state vocabulary, a separation of listener intent from engine facts, and a pure reducer per
> platform language (Rust for Theatre Web, Kotlin for CMP) whose agreement is proven by a shared
> corpus of JSON test vectors rather than shared runtime code. Covers interruptions, route
> changes, sleep timer, cold restore, seeking, speed, completion, release pinning, access
> refresh and checkpoint cadence. Use before adding a player state, event, effect, setting or
> vector, and when a Web and a mobile player disagree about what happened.

Product rules: [mobile § Playback behavior](../../../../docs/architecture/mobile.md#playback-behavior),
[mobile § Durable listening progress](../../../../docs/architecture/mobile.md#durable-listening-progress)
(completion, release changes), [UI system § Theatre](../../../../docs/design/ui-system.md#theatre-compact-listening)
and [business rules 26 and 28](../../../../docs/product/business-rules.md#listening-and-mobile).
This file never decides a product rule; it shows how to encode one so both platforms obey it.

## Three kinds of fact, one derived status

A player bug is usually two facts stored in one field: "paused" meaning both "the listener
paused" and "the engine is not producing audio". Keep them apart:

| Fact | Who changes it | Examples |
|---|---|---|
| **Intent** — what the listener wants | listener gestures, the sleep timer acting on their behalf, route loss, cold restore | `Play`, `Pause(User)`, `Pause(SleepTimer)` |
| **Engine** — what the media engine reports | engine callbacks only | `Preparing`, `Ready`, `Running`, `Stalled`, `Seeking`, `AtEnd`, `Failed(reason)` |
| **Context** — what the OS or network imposes | OS and access callbacks | interruption active, access expiring, refresh attempts |

The displayed status is a **projection** of the three, computed by a pure function, never stored.
Storing it creates a fourth fact that can contradict the other three.

```rust
// Illustrative, proposed. Pure module: no Leptos, web-sys, clock or I/O import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Idle,
    Loading,
    Playing,
    Paused(PauseCause),
    Buffering,
    Seeking,
    Ended,
    Failed(FailureReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseCause { User, SleepTimer, Interruption, RouteLost, ColdRestore }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureReason { AccessExpired, AccessDenied, Offline, PublicationUnavailable, Undecodable }

impl FailureReason {
    /// Temporary failures keep the session and offer retry; final ones explain and stop.
    pub const fn is_recoverable(self) -> bool {
        matches!(self, Self::AccessExpired | Self::Offline)
    }
}

pub fn status(state: &PlayerState) -> PlaybackStatus {
    let PlayerState::Active(s) = state else { return PlaybackStatus::Idle };
    match (s.engine, s.intent, s.interruption) {
        (Engine::Failed(reason), _, _) => PlaybackStatus::Failed(reason),
        (Engine::AtEnd, _, _) => PlaybackStatus::Ended,
        (Engine::Seeking, _, _) => PlaybackStatus::Seeking,
        (_, Intent::Pause(cause), _) => PlaybackStatus::Paused(cause),
        (_, Intent::Play, Interruption::Active) => PlaybackStatus::Paused(PauseCause::Interruption),
        (Engine::Running, Intent::Play, Interruption::None) => PlaybackStatus::Playing,
        (Engine::Stalled, Intent::Play, Interruption::None) => PlaybackStatus::Buffering,
        (Engine::Preparing | Engine::Ready, Intent::Play, Interruption::None) => PlaybackStatus::Loading,
    }
}
```

```kotlin
// Illustrative, proposed. commonMain, no Compose or media-engine import.
sealed interface PlaybackStatus {
    data object Idle : PlaybackStatus
    data object Loading : PlaybackStatus
    data object Playing : PlaybackStatus
    data class Paused(val cause: PauseCause) : PlaybackStatus
    data object Buffering : PlaybackStatus
    data object Seeking : PlaybackStatus
    data object Ended : PlaybackStatus
    data class Failed(val reason: FailureReason) : PlaybackStatus
}

enum class PauseCause { USER, SLEEP_TIMER, INTERRUPTION, ROUTE_LOST, COLD_RESTORE }
```

The asymmetry is deliberate: the status says `Paused` as soon as the listener asks, but never
says `Playing` before the engine reports `Running`. Claiming silence early is safe; claiming
audio early is the bug the [050 acceptance](../../../../docs/work-plan/050-cmp-native-listening.md#acceptance-criteria)
forbids ("the UI reports the real player state").

Both enums are closed and matched exhaustively, without a wildcard arm, so adding a state
breaks every consumer at compile time (`type-enforced` in Rust, `statically-checked` by `when`
exhaustiveness in Kotlin). On the wire the cause flattens to `"paused_because": "sleep_timer"`.

## Events and effects

The reducer reads no clock and performs no I/O. Time arrives as events from the shell; work
leaves as effects the shell executes in order.

| Event (input) | Source |
|---|---|
| `Load { publication, start_at }`, `UserPlay`, `UserPause`, `UserSeek { to }`, `UserSkip { by }`, `UserSetSpeed`, `UserSetSleepTimer`, `UserCancelSleepTimer`, `UserRestart` | listener gestures, from any UI or OS media control |
| `EngineReady { duration }`, `EngineRunning`, `EngineStalled`, `EnginePaused`, `EnginePosition { at }`, `EngineSeekCompleted { at }`, `EngineReachedEnd`, `EngineFailed { reason }` | media engine adapter |
| `InterruptionBegan`, `InterruptionEnded { should_resume }`, `RouteLost` | OS audio session adapter |
| `SleepTimerExpired`, `CheckpointDue` | timers the shell scheduled because of an effect |
| `AccessExpiring`, `AccessRefreshed { generation, source }`, `AccessRefreshFailed { generation, reason }` | access adapter |
| `Restored { saved }` | cold launch after process death |
| `ActiveReleaseChanged { episode, publication }`, `RemoteProgressObserved { record }` | listener API sync |

| Effect (output) | Executed by |
|---|---|
| `LoadSource { publication, source, start_at }`, `EnginePlay`, `EnginePause`, `EngineSeek { to }`, `EngineSetSpeed` | media engine adapter |
| `RequestAccess { publication, generation }` | access adapter (calls playback authorization) |
| `ScheduleSleepTimer { after }`, `CancelSleepTimer`, `ScheduleCheckpoint { after }` | timer adapter |
| `PersistCheckpoint { snapshot }`, `EnqueueProgress { candidate }` | local store, progress outbox ([`progress-sync.md`](progress-sync.md)) |
| `UpdateNowPlaying { title, position, duration, speed, playing }` | lock screen, notification, media session |
| `OfferResume { at }`, `OfferRelease { publication }`, `OfferRemotePosition { record }`, `Rejected { reason }` | UI |

`source` is an opaque `SourceHandle` held by the access adapter. The reducer, its snapshots and
its vectors never contain a URL, so a persisted checkpoint or a logged transition cannot leak a
signed URL — a [types-as-proofs](../../cantos-engineering/references/types-as-proofs.md) barrier,
not a logging convention.

## The reducer

```rust
// Illustrative, proposed. Value in, value out; mutation never escapes.
pub type Transition = (PlayerState, Vec<PlayerEffect>);

pub fn reduce(state: PlayerState, event: PlayerEvent) -> Transition {
    match state {
        PlayerState::Idle => reduce_idle(event),
        PlayerState::Active(session) => session.step(event),
    }
}

impl Session {
    fn step(self, event: PlayerEvent) -> Transition {
        use PlayerEvent as E;
        match event {
            E::UserPlay => self.play(),
            E::UserPause => self.pause(PauseCause::User),
            E::SleepTimerExpired => self.pause(PauseCause::SleepTimer),
            E::RouteLost => self.pause(PauseCause::RouteLost),
            E::InterruptionBegan => self.interrupt(),
            E::InterruptionEnded { should_resume } => self.end_interruption(should_resume),
            E::UserSeek { to } => self.seek_to(to),
            E::ActiveReleaseChanged { publication, .. } => self.offer_release(publication),
            E::RemoteProgressObserved { record } => self.offer_remote_position(record),
            // … every remaining variant named explicitly; no `_ =>` arm.
        }
    }

    fn end_interruption(self, should_resume: bool) -> Transition {
        let resume = should_resume
            && self.intent == Intent::Play
            && self.settings.resume_after_interruption;
        let intent = match self.intent {
            Intent::Play if !resume => Intent::Pause(PauseCause::Interruption),
            unchanged => unchanged,
        };
        let effects = if resume { vec![PlayerEffect::EnginePlay] } else { Vec::new() };
        (PlayerState::Active(Session { intent, interruption: Interruption::None, ..self }), effects)
    }
}
```

```kotlin
// Illustrative, proposed. The same meaning, written idiomatically — not transliterated.
data class Transition(val state: PlayerState, val effects: List<PlayerEffect>)

fun reduce(state: PlayerState, event: PlayerEvent): Transition = when (state) {
    PlayerState.Idle -> reduceIdle(event)
    is PlayerState.Active -> state.session.step(event)
}

private fun Session.endInterruption(shouldResume: Boolean): Transition {
    val resume = shouldResume && intent == Intent.Play && settings.resumeAfterInterruption
    val nextIntent = if (intent == Intent.Play && !resume) Intent.Pause(PauseCause.INTERRUPTION) else intent
    val effects = if (resume) listOf(PlayerEffect.EnginePlay) else emptyList()
    return Transition(PlayerState.Active(copy(intent = nextIntent, interruption = Interruption.NONE)), effects)
}
```

Reducer laws, each checked by vectors and by the properties below:

- **Transactional rejection.** An event that is invalid in context (an absolute seek past the
  duration, an unsupported speed, `UserPlay` with no session) returns the identical state and
  exactly one `Rejected { reason }` effect — never a half-applied change.
- **Ordered effects.** Effects are a list whose order is part of the contract: `EnginePause`
  precedes `PersistCheckpoint`, so a crash between them loses at most audio, never the position.
- **No hidden inputs.** Settings (`resume_after_interruption`, `max_access_refreshes`,
  `auto_advance`, the checkpoint interval) live in the state or arrive in events, so a vector can
  pin them. A reducer that reads a global preference cannot be vector-tested.

## Rules the reducer encodes

Each rule cites its owning sentence; the vector IDs are proposed names for the corpus.

| Rule | Owning sentence | Good | Counterexample | Vectors |
|---|---|---|---|---|
| R1 Cold launch never plays | "offer resume instead of automatically playing after a cold launch" | `Restored` → `Paused(ColdRestore)`, effects `[LoadSource, OfferResume]`, no `EnginePlay` | restoring `intent: Play` from the saved snapshot | `cold-restore-offers-resume`, `cold-restore-then-user-play` |
| R2 Explicit pause and sleep timer are final | "Do not resume unexpectedly after an explicit pause or sleep timer expiry" | `InterruptionEnded { should_resume: true }` after `Pause(SleepTimer)` emits nothing | resuming whenever the OS says `should_resume` | `sleep-timer-expiry-is-not-resumed-by-interruption-end`, `user-pause-during-call-stays-paused` |
| R3 Interruption is not a pause by the listener | "An incoming call … must not make the player claim successful user playback" | status `Paused(Interruption)` while the call lasts; intent still `Play` | setting `Pause(User)` on focus loss, which later blocks a legitimate resume | `call-interrupts-then-resumes`, `call-ends-without-resume-hint` |
| R4 Buffering is not paused | "distinguishes a paused user intent from buffering" | `EngineStalled` with intent `Play` → `Buffering`; recovery needs no gesture | showing a play button during a stall | `stall-and-recover-keeps-intent` |
| R5 Remote progress never moves the playhead | "prefer an explicitly selected resume position" | `RemoteProgressObserved` mid-session → `[OfferRemotePosition]` | `EngineSeek` to another device's position | `remote-progress-while-playing-is-an-offer` |
| R6 A session is pinned to one publication | "active playback does not silently switch to mismatched timing" | `ActiveReleaseChanged` → `[OfferRelease]`; publication and position unchanged | reloading the new asset at the old millisecond offset | `active-release-changed-while-playing` |
| R7 Access refresh preserves position, bounded | "Refresh authorization before expiry … preserve position, and bound retries" | `AccessExpiring` → `[RequestAccess { generation }]`; a matching `AccessRefreshed` → `LoadSource { start_at: position }`; a stale generation is ignored; after `max_access_refreshes` → `Failed(AccessExpired)` with position kept | an unbounded refresh loop, restarting from zero, or a late response for an old publication reloading the engine | `access-expiry-refreshes-in-place`, `stale-access-generation-is-ignored`, `access-refresh-exhausted-fails-recoverably` |
| R8 Completion is its own rule | "Define completion as a product rule independently of media engine 'ended' events" | `EngineReachedEnd` → `Ended`; completion decided by a separate pure function | `completed = engine.ended` | `seek-to-end-is-not-completion` |
| R9 Checkpoints are bounded | "Avoid per-second server writes" | `EnqueueProgress` on `CheckpointDue`, pause, seek, end and backgrounding, only when the position changed | enqueueing on every `EnginePosition` | `positions-between-checkpoints-enqueue-nothing` |
| R10 Seeks are explicit | "explicit seeking is valid and may move progress backward" | relative skips clamp to `[0, duration]`; an absolute seek outside it is rejected | clamping silently and reporting success for a corrupted target | `skip-back-near-start-clamps`, `absolute-seek-past-end-rejected` |

Open product decisions — encode none of them until the owning doc records it:

- whether route loss or a "don't resume" hint should ever auto-resume, and whether a sleep timer
  set to "end of episode" suppresses auto-advance — model them as settings in the state, never
  as hard-coded branches, so the vectors pin whichever default is chosen;
- whether a sleep timer survives process death and whether it keeps counting while paused —
  whatever is chosen, restore never resumes audio, and the checkpoint carries the timer data
  the rule needs (a monotonic deadline is meaningless after a reboot, so persist remaining time);
- the completion rule. One constraint is fixed today — seeking to the end alone is not a
  completed listen unless that rule is chosen — so `seek-to-end-is-not-completion` can exist now;
  every other completion vector waits for the decision.

## Shared contract vectors

The two reducers agree because both pass one **reviewed corpus of data**, not because they
share code. Proposed location: `packages/contracts/listening/player-vectors/<id>.json`, one scenario per
file, versioned with the player contract.

```json
{
  "vector_format": 1,
  "contract": "theatre-player@0.1.0",
  "id": "sleep-timer-expiry-is-not-resumed-by-interruption-end",
  "spec": "docs/architecture/mobile.md#playback-behavior",
  "initial": {
    "session": {
      "publication_id": "pub_7f3a",
      "title": "Một lời hẹn",
      "duration_ms": 1569000,
      "position_ms": 754000,
      "speed_permille": 1000,
      "intent": "play",
      "engine": "running",
      "interruption": "none",
      "sleep_timer": { "mode": "after", "remaining_ms": 1000 },
      "settings": { "resume_after_interruption": true, "max_access_refreshes": 2 }
    }
  },
  "steps": [
    {
      "event": { "type": "sleep_timer_expired" },
      "expect": {
        "status": "paused",
        "paused_because": "sleep_timer",
        "position_ms": 754000,
        "effects": [
          { "type": "engine_pause" },
          { "type": "persist_checkpoint", "position_ms": 754000 },
          { "type": "enqueue_progress", "position_ms": 754000, "kind": "checkpoint" },
          { "type": "update_now_playing", "title": "Một lời hẹn", "position_ms": 754000, "playing": false }
        ]
      }
    },
    {
      "event": { "type": "interruption_began" },
      "expect": { "status": "paused", "paused_because": "sleep_timer", "effects": [] }
    },
    {
      "event": { "type": "interruption_ended", "should_resume": true },
      "expect": { "status": "paused", "paused_because": "sleep_timer", "effects": [] }
    }
  ]
}
```

Comparison rules both harnesses implement identically:

- Snapshot fields present in `expect` are compared; absent fields are not asserted, which keeps
  each vector about one thing. The `effects` list is **always** compared in full and in order, so
  an unexpected `EnginePlay` fails every vector it appears in.
- No floating point anywhere: positions in integer milliseconds, speed in permille (`1250` is
  1.25×). Two languages that print `1.25` differently must not produce two meanings.
- Text is compared as exact NFC strings. `Một lời hẹn` in `update_now_playing` catches a platform
  that normalizes, truncates or mangles diacritics on the way to the lock screen
  ([Vietnamese text](../../cantos-script-ir/references/vietnamese-text.md)).
- An unknown `vector_format`, event `type` or snapshot field **fails** the harness. A skipped
  vector reported as a pass is the failure mode this corpus exists to prevent.

```rust
// Illustrative, proposed harness. Every file runs; nothing is skipped.
#[test]
fn reducer_agrees_with_every_reviewed_player_vector() {
    for vector in player_vectors::load_all().expect("corpus decodes") {
        let mut state = vector.initial_state().expect("initial state decodes");
        for (index, step) in vector.steps.iter().enumerate() {
            let at = format!("{}#{index}", vector.id);
            let (next, effects) = reduce(state, step.event().expect(&at));
            step.expect.assert_snapshot(&Snapshot::of(&next), &at);
            assert_eq!(effects.iter().map(WireEffect::from).collect::<Vec<_>>(), step.expect.effects, "{at}");
            state = next;
        }
    }
}
```

```kotlin
// Illustrative, proposed. commonTest, so the same test runs on JVM, Android and iOS targets.
class PlayerVectorTest {
    @Test
    fun reducerAgreesWithEveryReviewedPlayerVector() {
        for (vector in PlayerVectors.all()) {
            vector.steps.foldIndexed(vector.initialState()) { index, state, step ->
                val at = "${vector.id}#$index"
                val (next, effects) = reduce(state, step.event())
                step.expect.assertSnapshot(Snapshot.of(next), at)
                assertEquals(step.expect.effects, effects.map(PlayerEffect::toWire), at)
                next
            }
        }
    }
}
```

wasm32 and Kotlin/Native test binaries have no repository filesystem. Embed the corpus at build
time (a build script or generated source listing every file) and make an unreadable or unlisted
file a build failure. The test-runner setup belongs to
[web testing](../../cantos-leptos-web/references/web-testing.md) and
[CMP testing](../../cantos-cmp-mobile/references/cmp-testing.md).

### Keeping the corpus an oracle

- **Hand-authored from the spec.** Each vector cites the doc anchor it encodes and is reviewed in
  the PR that changes the behavior. Neither reducer generates the expected values.
- **Both platforms in the same PR,** or the PR states, per platform, "vectors not run" as a gap.
  A vector change that only one reducer passes is a contract break, not a test to quarantine.
- **Divergence hunting is separate.** Random event sequences from a neutral generator, run
  through the Rust reducer and replayed on Kotlin, find disagreements cheaply. They are a
  self-differential check: when they diverge, decide which side matches the doc, then commit the
  minimized case as a reviewed vector. Never promote generated outputs to the corpus unreviewed.
- **Version the meaning.** Changing what an existing vector expects bumps `contract`; adding a
  vector for new behavior does not.

The deeper technique — reference models, golden corpora, cross-target agreement — is in
[`property-and-differential-testing.md`](../../cantos-engineering/references/property-and-differential-testing.md).

## Properties per platform

Run on each reducer over generated event sequences: mostly legal events with a stated hostile
fraction (seeks past the end, engine callbacks with no session, duplicate `InterruptionEnded`),
biased toward long sequences, shrinking the **event list** and rebuilding state from it.
Candidates: proptest for Rust, kotest-property for Kotlin common code.

| Law | Statement |
|---|---|
| No unrequested start | if intent before an event was not `Play`, its effects contain `EnginePlay` only when the event is `UserPlay` |
| Sleep timer is final | after `SleepTimerExpired`, no `EnginePlay` appears until a `UserPlay` |
| Honest interruption | between `InterruptionBegan` and the next `InterruptionEnded`, status is never `Playing` |
| Position bounds | every reachable state has `0 ≤ position ≤ duration` |
| Transactional rejection | a rejected event yields an equal state and exactly one `Rejected` effect |
| Pinned publication | the publication changes only through `Load` |
| Bounded writes | count of `EnqueueProgress` ≤ count of `CheckpointDue` plus pause, seek, end and background events |
| Determinism | the same sequence twice yields equal states and effect lists |

On failure, record the seed, commit the shrunk sequence as a deterministic test and, when it
reveals a spec question, as a reviewed vector for both platforms.

## The imperative shell

- **One media session owns the engine** ([mobile § Boundaries](../../../../docs/architecture/mobile.md#boundaries)).
  Screens observe the reducer's state; navigation never creates or destroys an engine.
- **Adapters translate, never decide.** An Android focus-loss callback, an iOS interruption
  notification and a browser `pause` event each become one `PlayerEvent`; the decision about what
  that means is in the reducer. A `when` over OS callbacks that also decides whether to resume is
  a rule trapped in the shell.
- **Platform quirks become events.** A browser that refuses `play()` without a gesture, or an OS
  that pauses the engine itself during a call, is reported as an event, and the vector corpus
  gets the case. Do not paper over it inside the adapter.
- Web specifics (media element events, autoplay policy, Media Session API) belong to
  [media element interop](../../cantos-leptos-web/references/dom-and-media-interop.md); native
  audio sessions, focus, routes, lock screen and background modes to
  [native playback](../../cantos-cmp-mobile/references/native-playback.md). How each status looks
  and is announced belongs to [component states](../../cantos-ui-design/references/component-states.md).

## Evidence

| Claim | Evidence level (foundation vocabulary) |
|---|---|
| Reducer passes the corpus on Rust native / wasm32 | `differentially-tested`, naming each target |
| Reducer passes the corpus on Kotlin/JVM, Android or iOS test targets | `differentially-tested`, naming each target; still not `device-tested` |
| Laws above hold over generated sequences | `property-tested`, with case counts and seed |
| Exhaustive matches over closed enums | `type-enforced` (Rust), `statically-checked` (Kotlin `when`) |
| Lock screen, interruption, route change and background behave as the vectors say | `device-tested` on a named device or simulator, through the real adapter |

A vector passing proves the reducer's meaning, not the adapter's translation. An interruption
that the adapter never reports is invisible to every vector; only a device run shows it.
