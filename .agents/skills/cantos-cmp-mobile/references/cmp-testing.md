# CMP testing and evidence

> **Scope.** Choosing and running evidence for the Theatre CMP app: pure reducer and controller
> tests, shared contract vectors, properties and fault injection, Compose UI tests with a named
> renderer, device/emulator/simulator runs, what counts as blocked, and how the mobile
> acceptance matrix maps onto evidence levels. Use before adding a test, claiming a mobile
> behavior works, or writing a mobile completion report.

The evidence vocabulary is the foundation's
[§ 6](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified), used
verbatim. UI evidence selection and the visual pass belong to
[`cantos-ui-design`](../../cantos-ui-design/SKILL.md#evidence-selection) and its
[visual review](../../cantos-ui-design/references/visual-review.md); vector reporting per
platform belongs to [`cantos-listening`](../../cantos-listening/SKILL.md#evidence-for-listening-work).
This reference selects among them for CMP. Test technique in general lives in
[verification-strategy.md](../../cantos-engineering/references/verification-strategy.md) and
[property-and-differential-testing.md](../../cantos-engineering/references/property-and-differential-testing.md).

## Discover before executing

For a static pixel/semantics check without an emulator, use the implemented
[`cargo xtask compose-preview` workflow](compose-preview.md). It requires an existing Android
Gradle target, preview function and a running Android Studio Quail 2+ instance. Theatre's isolated
mock has Android preview wrappers; missing CLI/IDE prerequisites produce `UNSUPPORTED`.
Studio Mobile has no target. The shared
`ui-system-cmp` Android library is a separate supported preview host. Capture-tool mocks
prove the adapter contract, not Compose rendering. Robolectric remains a separately configured
JVM test candidate, not an automatic fallback.

Theatre has an isolated Gradle mock. Actual repository commands:

```text
cargo xtask theatre-demo build          # :shared:compileKotlinDesktop
cargo xtask theatre-demo verify         # :shared:desktopTest, common + desktop tests
cargo xtask theatre-demo android-build  # mock debug APK, not device tests
cargo xtask theatre-demo ios-build --kotlin-native-home /absolute/installed/native-2.2.20
```

The iOS command links a simulator framework; it executes neither commonTest on Native nor
XCTest/device tests. No instrumented/Robolectric/native runtime gate is configured. Discover
tasks before proposing a new xtask gate; the actual JVM target is `desktop`, not `jvm`.

Read the build files to learn which source sets each task compiles and runs. Record selected,
executed and skipped counts per target from the test reports. `NO-SOURCE`, `UP-TO-DATE` from a
different revision, or a filter that matched nothing is not a pass.

## Evidence selection

| Claim | Cheapest evidence | Level | Where it runs | Does not prove |
|---|---|---|---|---|
| a playback, download or outbox decision | reducer table test with exact `Transition` | `example-tested` | `commonTest` on Kotlin/JVM first, then every target that ships the code | engine or OS behavior |
| Theatre Web and mobile mean the same thing | shared vector corpus from `cantos-listening` | `differentially-tested` per named platform | Kotlin/JVM, Android host, Kotlin/Native iOS simulator | any platform not named |
| a law over any event sequence (no unexpected `Play`, no lost outbox operation) | generated sequences, shrunk counterexamples | `property-tested` | Kotlin/JVM | real storage or engine |
| download staging survives crashes | crash hook at each named point over a fake file system | `fault-injected` | Kotlin/JVM | device file-system durability |
| local schema and migrations | the real SQLite driver for the target | `integration-tested` | host driver, then on device | an in-memory fake never counts |
| the generated client decodes the contract | contract fixtures decoded per client platform | `example-tested` | each Kotlin target | server behavior |
| progress sync against the server | real listener API build and database | `integration-tested` | host or device against a test server | other devices' timing |
| names, roles, states, actions | Compose UI test on the production composable | `semantics-tested` (renderer named) | JVM desktop, Android host or device, iOS simulator | pixels, screen readers |
| a flow works by input | events driven through the production route | `interaction-tested` (renderer named) | same | OS lifecycle |
| layout, theme, locale, font scale | capture **and open** each image | `screenshot-inspected`, `cross-theme-inspected` | the renderer named | interaction, motion, other renderers |
| static Compose preview pixels and exported tree | `cargo xtask compose-preview`, open PNG, then assert observed semantics | `screenshot-inspected`; `semantics-tested` only after assertions | Android Studio layout renderer | device, iOS, interaction, motion or app build |
| background, lock screen, interruptions, routes, process death, OS downloads | scripted run with a recording and logs | `device-tested` | named device, emulator or simulator | other OS versions or hardware |
| TalkBack or VoiceOver can operate the player | a walkthrough by a named person | `screen-reader-walked` | named device and reader | other readers |

## Pure core tests in `commonTest`

- Use `kotlin.test`. Name tests as theorems in `snake_case`
  (`listener_pause_during_interruption_survives_interruption_end`); backticked names with spaces
  are not portable to every Kotlin target.
- Assert the exact resulting state **and** the exact effect list. `assertTrue(state.intent is
  Paused)` alone lets a stray `Play` effect through.
- Drive time with `runTest`, a `StandardTestDispatcher` and virtual time (`advanceTimeBy`, the
  test time source) injected into the controller
  ([KT-CONCURRENCY](kotlin-conventions.md#kt-concurrency--structured-injected-controllable)).
  Never wait with real delays.
- Fakes implement ports: `FakeMediaEngine` records commands and emits scripted `EngineEvent`s;
  `FakeListenerApi` scripts accepted, duplicate, stale, `401` and timeout responses; a fake file
  system (Okio's is a candidate) supports crash hooks. Never mock the reducer under test.
- Keep the oracle independent: expected values come from the owning document or vector, never
  from calling the reducer to compute them.

## Verification rules

MUST/SHOULD/MAY and exception recording follow [Kotlin conventions](kotlin-conventions.md).
The examples here illustrate assertions, not newly installed test helpers.

### VER-01 — compile-time and static checks

**MUST** for touched type/visibility/decision boundaries: run `cargo xtask verify repository`
(Theatre layer import guard, lexical) and compile affected targets and inspect
sealed exhaustiveness, distinct identities, inaccessible constructors and relevant diagnostics.
Good: `openEpisode(episodeId)`; negative probe `openEpisode(publicationId)` must fail.
New sealed cases must expose missing `when` branches.
**Oracle/status:** pinned Kotlin compiler diagnostics; review ARC-01 imports. The repository
import guard is bounded and lexical; no compiler-backed negative harness, warning-as-error
policy or Kotlin lint gate exists today.
**Exception:** manual import review remains legitimate outside the guard's declared lexical
scope, reported as review.
Compilation cannot establish every runtime invariant; docs-only changes need no app compile.

### VER-02 — factories, invariants and transactional rejection

**MUST** for changed refined values/commands: cover valid, invalid and boundary values, public
operations, allowed copies, decoding, restoration, defaults and fixture construction.
Good: `assertEquals(PlaybackTransition(before, listOf(PlaybackEffect.Rejected(reason))), actual)`.
Counterexample: `assertTrue(actual.effects.isNotEmpty())` misses partial state/extra commands.
**Oracle/status:** independent complete state/ordered effects in `PlaybackReducerTest` cover
source/identity factories, session/command construction, Uint32 exhaustion, invalid seek/restore,
supported speeds and stale callbacks.
**Exception:** cite an existing test that fails for the changed invariant rather than duplicate it.
Raw invalid inputs are useful; invalid settled state is not a valid generator shortcut.

For composed factories/error projections, apply foundation
[ERR-02/03](../../cantos-engineering/references/typed-errors-and-composition.md): assert exact
first failure and that dependent stages do not execute, deterministic independent findings
when accumulation is promised, and the invariant after each successful transformation.
Exercise every sealed error in UI/retry projections; a generic “failed” assertion misses lost
correction facts. Compiler exhaustiveness and runtime predicate tests establish different claims.

### VER-03 — property-based testing

**SHOULD** for critical invariant families with large input/event spaces; **MAY** evaluate
[Kotest Property](cmp-tooling.md) without replacing the test framework.
Good illustrative law: `checkAll(eventsArb) { replayAndAssertBounds(it) }`.
Counterexample: random events without shrinking, reproducible seeds or per-transition assertions.
**Oracle:** independent laws; failing seed, generator/library version, iteration/trace limits and
shrunk event trace. Promote minimized failures to ordinary regressions; a seed alone is not enough.
**Exception/status:** deterministic tables/bounded loops may suffice for a small finite domain;
record why. Kotest is not installed; listing laws is not an executed property suite:

- Position stays in `0..source.durationMs`; generation cannot overflow and exhaustion rejects.
- Stale source/generation callbacks yield identical state and no effects.
- Repeated **contract-idempotent** commands yield no duplicate effects; do not generalize to
  every command (Restore intentionally allocates a generation).
- Explicit Pause cannot be undone by late EngineStarted; cold restore never emits StartEngine.
- Invalid input preserves complete state and emits exactly the specified rejection.
- Running/simulating status needs a matching accepted observation; Play alone is intent.
  Assert the corpus's actual pause/preparation rules instead of inventing stronger guarantees.

For future download/outbox capabilities also generate crash/account/duplicate sequences:
no accepted operation lost or sent under another account; available bytes match the manifest.
These remain proposed tests for unimplemented capabilities.

### VER-04 — state-machine and bounded exploration

**SHOULD** when combinations/order dominate critical behavior: build a small independent model
from the reviewed specification and compare complete state/ordered effects after each event.
Good: `assertEquals(model.step(event), implementation.step(event))` with independent decisions.
Counterexample: `model.step = ::reducePlayback` or copying its branches.
**Oracle:** generated sequences with shrinking; bounded exhaustive runs record finite states,
source identities, position representatives, generation boundary classes, event alphabet,
maximum trace length, explored count and environment assumptions.
**Exception/status:** focused tables suffice if exploration adds no distinct assurance. No Kotlin
model runner exists today. Finite exploration is not proof for every execution.
Use `bounded-model-checked` only with an executed exhaustive/verifier result and stated model/
bounds; random sequence sampling is `property-tested`.

### VER-05 — concurrency and lifecycle

**MUST** for changed ownership, async effects, cleanup or fencing: use `runTest`, injected
scheduler/dispatcher and scripted deferred observations; assert order, not elapsed sleeps.
Good: `player.toggleIntent(); player.toggleIntent(); advanceUntilIdle()` then exact commands/state.
Counterexample: `delay(500); assertTrue(player.state.value != null)`.
**Oracle/status:** `PlaybackControllerTest` covers rapid toggles/skips, ordered speed/deferred
Start, restore fencing, bookmark capture ordering, typed mock failure and close;
`AppContainerTest` covers cross-owner reset. Add delayed old-account/reset callbacks and
cancellation tests where those capabilities change. Expected cancellation must propagate.
At a changed foreign-call containment seam, inject each documented failure and cancellation;
assert the exact typed outcome/recovery decision and fenced late results. Cancellation propagates
without automatic retry. A fake throwing a scripted exception tests translation, not native
recovery or real I/O.
**Exception:** JVM-only Lincheck is optional for a real shared-memory concern; single-thread
virtual tests alone do not establish multi-writer thread safety. JVM concurrency evidence never
substitutes for Android/iOS lifecycle/audio validation.

### VER-06 — canonical differential vectors

**MUST** for shared playback meaning/harness changes: preserve the canonical
[mock specification and corpus](../../../../packages/contracts/fixtures/theatre/playback/v1/README.md).
Good: `assertEquals(step.effects, transition.effects)` **and** complete snapshot equality.
Counterexample: comparing only status or regenerating expected outputs from either reducer.
**Oracle/status:** hand-authored specification outputs, independently consumed by Rust/Kotlin.
Gradle `generatePlaybackVectors` embeds SHA-256-checked JSON bytes into commonTest Kotlin strings;
it never generates expectations. `PlaybackVectorTest` validates seven files/51 steps and fails
unknown versions, fields, events and inventory changes.
**Exception:** intentional reviewed contract changes update specification/index and both consumers
together; no fixture edits merely to make tests pass. The production listener protocol remains
separate. Every shipping target runs the corpus; report Rust native/WASM, Kotlin/JVM, Android
and Kotlin/Native selected/executed/skipped counts independently, including unrun targets.

### VER-07 — assertion strength and mutation

**SHOULD** for critical assertions where compatible tooling exists; coverage is not correctness.
Good: removing `state.accepts(identity)` makes the stale-observation test fail.
Counterexample: `assertNotNull(reducePlayback(...))` survives that mutation.
**Oracle:** killed/surviving/invalid mutant inventory, executed tests and replayable inputs;
see [mutation/formal methods](../../cantos-engineering/references/mutation-and-formal.md).
**Exception/status:** existing mutation tooling targets Rust Script IR, **not Kotlin playback**.
No Kotlin mutation runner is configured; require a concrete gap and compatible tool before adding
one. Kani applies to supported Rust targets, not Kotlin/CMP directly.

### VER-08 — evidence honesty

**MUST** for every result: use foundation levels and name exact revision, target, runner,
corpus hashes, source sets and counts.
Good: "Kotlin/JVM `example-tested`/`differentially-tested`; iOS runtime `NOT_RUN`".
Counterexample: "CMP formally proven/native audio verified" from desktop tests or a linked framework.
**Oracle:** attributable compiler/test/verifier/device output for these exact inputs.
**Exception:** none for fabricated evidence; missing checks remain gaps.

Relevant foundation levels include `type-enforced`, `example-tested`, `property-tested`,
`differentially-tested`, `mutation-tested`, `bounded-model-checked`, `mock-integration-tested`,
`integration-tested` and `device-tested`. Their definitions stay in the foundation.
Execution status is separate: xtask prints `PASS`/`FAIL`/`NOT_RUN`/`UNSUPPORTED`.
A manually identified prerequisite gap may be `BLOCKED` (foundation `blocked`); do not invent a
`BLOCKED` xtask status. Native audio/formal proof need corresponding executed evidence.

## Compose UI tests

- Common UI tests (`runComposeUiTest`, an experimental API behind an opt-in) run on whichever
  targets the build configures. Verify for the pinned version which of JVM desktop, Android
  host, Android instrumented and iOS simulator actually execute them, and name the renderer in
  every report.
- Test the **production** composable with injected state and callbacks, or the production route
  with fake ports. A test-only copy of a screen proves nothing about the shipped one.
- Find nodes by accessible name, role and state using localized Vietnamese text, then assert the
  state description, enabled state and available actions. A `testTag` locates; it does not name.
  Choose merged or unmerged trees deliberately.
- For intermediate animation frames, take control of the test clock; for final states, default
  synchronization is enough. Platform I/O is not covered by Compose idling: wait for an
  observable condition with a bounded timeout, never a sleep.
- Host screenshot tools (Roborazzi and Paparazzi for Android, desktop capture) are candidates.
  Their images are `screenshot-captured` until someone opens them, and a desktop image is never
  evidence for Android or iOS rendering.

## Device, emulator and simulator runs

The commands below are candidates to verify for the tool versions in use.

| Scenario | Android | iOS | Evidence to keep |
|---|---|---|---|
| background and lock screen | emulator and a physical device; play, lock, operate lock-screen and notification controls | physical device preferred; simulator lock-screen and Now Playing support is limited | recording; state log with secrets redacted |
| interruption and focus loss | emulator console call simulation; another app taking audio focus | physical device call, alarm or another audio app | intent and position before/after; no unexpected resume |
| headset removal, route change | physical wired or Bluetooth headset | physical headset | pause observed; no resume on reconnect |
| process death and cold launch | kill the backgrounded process (`adb shell am kill` candidate; check behavior with a running media service) | stop from Xcode while backgrounded is not the same as OS termination; record which was used | resume offered, no autoplay |
| downloads | airplane mode mid-transfer, small data partition, kill during transfer, test server serving corrupt bytes | same, plus background `URLSession` relaunch | manifest row and file state after each case |
| large text and reduced motion | system font scale and remove-animations settings | Dynamic Type and Reduce Motion | opened captures at default and largest scale |
| screen reader | TalkBack walk: discovery → play → seek → speed → timer → download | VoiceOver walk of the same flow | named reviewer, device, reader version |
| cross-device sync | two devices plus a Theatre Web session | same | conflict prompt and final server revision |

Record device model, OS version, emulator or physical, app build and source revision for every
run. A simulator result is labeled simulator; it never becomes "physical device".

## Blocked is never passed

- Zero selected tests, a filter that matched nothing, or a skipped required suite is
  **blocked** (or failed), with the reason.
- Kotlin/Native Apple targets build only on macOS. On a Linux host, every iOS claim — even
  `compiled` — is blocked; say so on the residual-risk line instead of omitting iOS.
- A shared test source set does not mean every target executed it. Report per target.
- JVM desktop and Robolectric results are host evidence; an Apple framework that linked is
  `compiled`; neither is `device-tested`.
- Results from another revision, or images captured before the last edit, are stale.
- A required gate that cannot run (no device, no headset, no second account) stays listed under
  native gates as blocked; [050](../../../../docs/work-plan/050-cmp-native-listening.md#acceptance-criteria)
  treats an unexercised platform as an explicit release gap.

## Acceptance matrix → evidence

Rows come from [mobile.md § Future acceptance matrix](../../../../docs/architecture/mobile.md#future-acceptance-matrix)
and the CMP rows of the [UI validation matrix](../../../../docs/design/ui-system.md#future-validation-matrix).

| Area | Deterministic evidence first | Native evidence to close the row |
|---|---|---|
| native playback | reducer tables; Now Playing projection | `device-tested` per platform with recording |
| interruption | interruption × intent reducer table; vectors | `device-tested`, physical hardware for calls and routes |
| delivery | access-refresh and retry reducer tests; fake API | `integration-tested` against real storage/CDN, then `device-tested` offline transition |
| downloads | `fault-injected` staging; `property-tested` lifecycle | `device-tested` process death, low storage, corrupt bytes |
| rights/account | entitlement and account-switch reducer tables | `device-tested` logout and switch with two accounts |
| progress | vectors; outbox property | `integration-tested`; two devices plus Theatre Web |
| lifecycle | restore reducer tests | `device-tested` OS termination and restart |
| accessibility/UI | `semantics-tested` production player | `screen-reader-walked`, `screenshot-inspected` at largest text, both themes |

## Inspection scenarios

Reusable audit scenarios for the player live with the Inspector:
[`cmp-theatre-playback-v1.json`](../../cantos-ui-inspector/scenarios/cmp-theatre-playback-v1.json).
Reuse its steps for a same-scenario recheck after a fix instead of inventing a new path. Running
an audit is [`cantos-ui-inspector`](../../cantos-ui-inspector/SKILL.md)'s job and stays
report-only.

## Formatting and static analysis

No Kotlin formatter or linter is configured. Report manual review, the compiler and tests that
actually ran, and the missing static gate. Candidates and the rule for choosing them are in
[KT-STYLE](kotlin-conventions.md#kt-style--readable-mechanically-formatted). A Web or Markdown
check that inspected zero Kotlin files is not Kotlin evidence.
