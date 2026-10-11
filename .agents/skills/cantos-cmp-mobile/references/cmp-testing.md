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
Gradle target, preview function and a running Android Studio Quail 2+ instance. Both Cantos mobile
directories are still guides, so real capture is currently `UNSUPPORTED`; capture-tool mocks
prove the adapter contract, not Compose rendering. Robolectric remains a separately configured
JVM test candidate, not an automatic fallback.

No Gradle build, test task or device runner exists. Typical Kotlin Multiplatform task shapes to
**look for** once the build lands — not commands that exist today:

```text
./gradlew :<shared>:jvmTest                     common + JVM tests on the desktop JVM
./gradlew :<shared>:testDebugUnitTest           Android host tests (Robolectric if configured)
./gradlew :<shared>:iosSimulatorArm64Test       Kotlin/Native tests on an iOS simulator (macOS only)
./gradlew :<androidApp>:connectedDebugAndroidTest   instrumented tests on a device or emulator
xcodebuild test -scheme <app> -destination '<simulator or device>'   XCTest (macOS only)
```

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

### Shared vectors

`cantos-listening` owns the vector corpus and its location. Every Kotlin target that ships the
reducer runs it; an unknown event kind fails the harness. Loading JSON from `commonTest` is not
uniform across Kotlin/JVM and Kotlin/Native: generate a Kotlin source from the vectors at build
time, or read them through a multiplatform file API with a build-provided path, and record the
choice. Report each platform separately ("Kotlin/JVM: 48/48; iOS simulator: not run — no macOS
host"). Vectors produced by running one implementation are a divergence check, not an oracle.

### Properties

kotest-property is a candidate; verify its Kotlin/Native support for the pinned version. Laws
worth generating for:

- no `Play` engine effect without a listener `Play` event or an interruption ending while intent
  is `Play`;
- position stays within `[0, duration]` across seeks, speed changes and access refreshes;
- for any interleaving of local writes, failures, duplicates and restarts, no outbox operation is
  lost and none is sent under another account scope;
- for any crash point and chunking, `Available` implies bytes whose checksum matches the
  manifest.

Keep the concrete shrunk input as a regression example; a seed alone is not a counterexample.

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
