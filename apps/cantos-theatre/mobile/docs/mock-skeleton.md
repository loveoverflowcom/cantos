# Theatre CMP mock: decisions and evidence

Status: historical phase1 checkpoint at `2a425a87ab20aba4061d2910bf02df62c32652e9`.
The subsequent [architecture/evidence record](architecture.md) owns current implementation and
results; relative source links below describe the checkpoint and may now contain refactored code.
This scoped record follows
the [ADR template](../../../../templates/adr.md) and engineering/UI/CMP evidence vocabulary;
it selects no production media, storage, credentials or API library.

Current paths: this checkpoint predates the layered packages and #122 module extraction.
Neutral presentation models now live in shared `presentation/model`; mock adapters, fixture
seeds, previews and their tests live in `:preview-sandbox`, wired by explicit `mockAppContainer`.
The source links below point to those current owners; the counts, captures and build paths
in this checkpoint remain historical. The [mock app sandbox](architecture.md#mock-app-sandbox)
section owns the current tree.

## Context and invariant

Android, iOS and the JVM diagnostic host mount the same deterministic Kotlin presentation.
Every action changes memory-only state or explains an unavailable integration. UI simulation
never implies audible playback. Navigation, seek and repeated actions preserve the selected
fixture/position; reset restores the complete initial value.

`commonMain/demo` owns immutable fixture identity and pure transitions; `commonMain/app` owns
screens and keyed vi/en resources. Hosts own platform Back/bars, iOS embedding and the desktop
window. No media port is invented for an absent engine. Future native media/versioned listener
contracts keep the owners in the existing mobile architecture.

## Scoped decisions

- Retain Kotlin 2.2.20 / Compose 1.9.0 / stable Material3 1.8.2 / AGP 8.11.1 / Gradle 8.14.3.
  The [official compatibility guide](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)
  requires the Compose compiler plugin to match Kotlin;
  [AGP 8.11 documentation](https://developer.android.com/build/releases/agp-8-11-0-release-notes)
  specifies Gradle 8.13+, JDK 17 and build-tools 35.0.0. No Expressive alpha API or
  auto-downloaded toolchain is used.
- Desktop validation is independent; native target registration is explicit. Missing/unpinned
  local Kotlin/Native fails before provisioning. Android API 23/iOS 15 are diagnostic floors,
  not an approved production platform-support decision.
- Consume canonical theme/tokens and native Material controls in private app-screen compositions.
  Registered Cantos wrappers stay unused while **0/29 qualified**; no admission gate,
  compatibility baseline or registry is relaxed. Official outlined icon vectors use the pinned
  common/JVM/Android/iOS `material-icons-extended:1.7.3` artifact.
- Use a small closed resource enum with both vi/en strings and parameter tests. Story labels
  remain Vietnamese. Other fixture content is original/synthetic. Art/fonts/audio stay outside
  Git; canonical platform-font fallback stays explicit.
- No clock/media effect exists: seek clamps manually; seeking to the end implies no completed
  listening. Speed/timer are preferences only. Explicit episode selection keeps the current
  episode's point or starts a different fixture at zero. Bookmarks are duplicate-safe memory
  values, not durable account/publication/synchronization records.
- Loading/empty/error are manually chosen scenarios; retry preserves selected content and point.
  No download, signed URL, progress write, session, provider, job or release is constructed.

## Design provenance

Authenticated metadata and the readable handoff were retrieved from the
[Mobile folder](https://drive.google.com/drive/folders/1i34c_09-Ccq1K21yjxFXyarBk1lgvhy0).
The [Static Board](https://drive.google.com/file/d/1LpUFy2_hWBx8fJWd8VdV8dC7z8urp7XI/view)
was opened in this session: warm paper, purple, serif hierarchy, mini-player and three bottom
tabs. It labels itself a manually composed board, not a browser/native capture.
The [readable handoff](https://drive.google.com/file/d/1U55XW5txGamBYOm_MGW3itBH_BehJtfj/view)
was also read. These and canonical repo tokens are the verified design inputs.

The supplemental existing local ZIP has 11,225,818 bytes, matching Drive metadata; SHA-256
`bf55c2651c3db82fb1b7d07a44658c930d27175b0ebadc083f58dff865871fc9`. Embedded HTML has
15,930,341 UTF-8 bytes, also matching metadata; extracted handoff matches the local handoff.
Fresh full ZIP retrieval failed with transport closed, so remote/local archive byte identity
has not been certified. No archive, raw HTML, art, fonts or audio is committed.

## Assurance ledger

| Invariant | Failure mode | Oracle / owner | Evidence |
| --- | --- | --- | --- |
| UI simulation is never an audio claim | fake native/playback acceptance | explicit copy; no media/network imports or permissions | `statically-checked`: source review confirms the mock boundary; `semantics-tested`: the passing app JVM suite asserts simulation/no-audio status |
| Close/back/repeat preserve fixture and point | growing stack/lost position | [current presentation model](../shared/src/commonMain/kotlin/com/cantos/theatre/presentation/model/ShellState.kt) and [app interaction suite](../preview-sandbox/src/desktopTest/kotlin/com/cantos/theatre/app/DemoInteractionTest.kt) | `example-tested` and JVM `interaction-tested`: repeated routes, queue/close, contextual Settings Back, focused-control Escape and reset pass; no native Back claim |
| Seek/bookmarks remain within fixture | invalid position/duplicates | [current fixture/projection regressions](../preview-sandbox/src/commonTest/kotlin/com/cantos/theatre/demo/DemoStateTest.kt), including integer extremes at the checkpoint | `example-tested`: negative, oversized and overflowing deltas clamp; duplicate bookmarks and seek-to-end rules pass. JVM slider semantics change the displayed point without starting simulation |
| Caller collection views cannot mutate state | constructor/copy bypass | snapshotting construction/copy and alias regressions | independent source finding fixed; construction, copy and no-op snapshot tests pass in the 23-test reducer suite |
| Both UI locales/parameters stay complete | missing/blank/wrong text | [closed resources and parity tests](../preview-sandbox/src/commonTest/kotlin/com/cantos/theatre/demo/DemoCopyTest.kt) | `example-tested`: eight tests pass, including nonblank vi/en values, exact placeholder multiplicity, localized times/speeds and listener naming |
| Captures exercise exact app and dimensions | alternate preview hides defects | [fixed-size app capture suite](../preview-sandbox/src/desktopTest/kotlin/com/cantos/theatre/app/DemoCaptureTest.kt) and opened-image ledger below | seven capture tests pass and produce eight PNGs; exact pixel dimensions are asserted, and all eight fresh images were opened and inspected |
| Run signals only its owned group, including descendants | orphan/foreign cleanup | [mock launcher guard](../../../../xtask/src/theatre_demo.rs), [supervisor](../../../../xtask/src/process.rs) and regression tests | `statically-checked` and Rust `example-tested`: ownership/version/provisioning regressions pass; actual launch/interrupt inspection confirms the guard PID, owned group and absence of its processes after shutdown |

Independent phase 1 source review found no actionable findings after the immutable-collection,
transport-name, native-version-proof and Android system-bar fixes. Its 44-file snapshot used
base `8eb7f495fd07936295bda328b7392e83ed97ffa5` and fingerprint
`596e8132e98080edd061c1aa96d38563b09335d83d45ecc7b519712d9c079329`.
The separate wiring recheck also found no actionable findings. These are source-review results;
they do not establish native rendering, playback or component qualification.

## Execution and observations

This is the phase 1 mock checkpoint. The fresh JVM reports dated `2026-10-12T11:48:10–12Z`
record **48 executed tests, zero failures/errors/skips**: 23 reducer, eight copy/resource,
ten app interaction/semantics and seven capture tests. The latter produce eight images because
the large-text work test captures both the initial viewport and a scrolled episode. Reports
remain under ignored `apps/cantos-theatre/mobile/shared/build/test-results/desktopTest/`.

The earlier full preflight's 48-test mock task had two interaction failures: generic transport-name expectations did
not match the action-plus-episode names observed in the app. Assertions now require the full
localized names, and the fresh 48-test execution passes. No product label or availability
condition was weakened to make the tests pass.

| Command / observation | Recorded result | Boundary / final handoff note |
| --- | --- | --- |
| `cargo xtask theatre-demo build` | **PASS** | pinned common/JVM compilation; no Android/iOS runtime claim |
| `theatre-cmp-demo` gate in `cargo xtask preflight --changed-only` | **PASS** | fresh `:shared:desktopTest` completed in eight seconds; 48/48 tests as above |
| `cargo xtask theatre-demo verify` | **PASS**, earlier standalone execution | 48 passing tests after a stale English singular/plural expectation was corrected; the latest label-aware suite also passes through the preflight gate |
| `theatre-cmp-demo` gate in an earlier full `cargo xtask preflight --changed-only` | **FAIL**, retained | the two obsolete generic-name assertions above; the latest gate subsequently passed with full episode-aware names |
| `cargo xtask theatre-demo android-build` | **PASS**, latest copy | 16 seconds, 55 tasks: nine executed and 46 up-to-date. APK at ignored `androidApp/build/outputs/apk/debug/androidApp-debug.apk`; no device/emulator was run |
| `cargo xtask theatre-demo ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.2.0` | **UNSUPPORTED** | installed compiler 2.2.0 is refused against exact 2.2.20; no framework compilation or compiler download |
| `cargo xtask compose-preview cantos-theatre` with the existing Theatre preview wrapper | **UNSUPPORTED** | official Android CLI was absent; no current Android Studio image or semantics is claimed |
| `xcodebuild -list` / `-showBuildSettings` for `iosApp/CantosDemo.xcodeproj` | project/configuration inspection succeeded | target and shared scheme are discoverable; simulator framework search path resolves to `shared/build/bin/iosSimulatorArm64/debugFramework`. No Swift/framework app build |
| `cargo xtask verify rust` | **PASS** | fresh format, lint and test subgates; no remote-CI substitute |
| `cargo xtask verify contracts` | **PASS** | all three selected subgates |
| `cargo xtask preflight --changed-only` | **PASS** | all 15 selected gates, with no skips; `/tmp/cantos-cmp-preflight-final.log` |
| `cargo xtask verify repository` on the final evidence record | **PASS** | 352 text files / 15 skills, 216 Python tests, links/JSON/skill structure and both whitespace checks; `/tmp/cantos-cmp-repository-phase1-final.log` |
| `cargo xtask theatre-demo run`: actual startup/shutdown ownership | **PASS**, bounded process observation | `:shared:run` reached; launcher/guard PID 8684 and `MainKt` PID 8706 belonged to PGID 8684. Ctrl-C exited zero; inspection found neither PID nor any descendant in the owned group afterward |
| Desktop OS-window input; Android/iOS runtime and screen readers | **NOT_RUN** | computer-use discovery reported the Mac locked and exposed no app surface; JVM test input is the only executed app interaction renderer |

The final preflight passed repository, rust-format, script-ir-reference, rust-lint (workspace),
rust-tests (workspace), dbsp, wav-reference, web-lint, both web-build gates, ui-preview,
cmp-ui-preview, theatre-cmp-demo, mutation and postgres. Mutation killed all nine mutations,
with zero survivors/timeouts/unviable cases. PostgreSQL used a new disposable cluster, completed
restart and mocked HTTP checks, and stopped; no inherited database URL was substituted.
Earlier preflight failures included the corrected Rust `useless_format` lint and a port-ownership
probe race; the final execution ran Rust tests serially and passed all gates. Failed logs remain
preserved, including `/tmp/cantos-cmp-preflight.log`. GitHub CI was not polled.

The latest APK SHA-256 is
`1cfcea16b1ba37086feb1a79a8ee95feab200ddcc01bcd7d5ce44e05d8c6b5bb`;
its repeat build log is `/tmp/cantos-cmp-android-phase1-final.log`. Process observations remain
under ignored `target/theatre-cmp-evidence/phase1-process.md`. Successful process start and
cleanup do not prove window readiness, input or native runtime behavior. The final phase1
repository gate passed in the full preflight recorded above.

### Opened app captures

All paths in the following tables are relative to ignored
`apps/cantos-theatre/mobile/shared/build/reports/theatre-demo/`. Each PNG has a sibling JSON
record and `.semantics.txt` dump. Captures stay outside Git; no downloaded art/font/audio or
generated image is published by this document.

The renderer is **JVM Desktop Compose UI test**, mounted at `CantosDemoApp`, with density **1**
and base font scale **1**. Requested dp dimensions therefore equal asserted PNG pixels.
Every image uses Vietnamese UI, fixture `SonHa03` at **768 seconds / 12:48** and manually
selected ready/error state. The two work images request **2× app text scale**; this is not
Android font scale, iOS Dynamic Type or an OS-window observation. All eight images were opened
again after the successful run; their hashes match the previously inspected pixel content.
Capture JSON retains `screenshot-captured; inspection pending`; this separate record documents
the later inspection instead of rewriting capture output.

| PNG | Scenario / actual pixels | Observed pixels | Limits |
| --- | --- | --- | --- |
| `jvm-light-discovery-390x844.png` | discovery, light, 1×; **390×844** | warm paper, lavender groups and serif Vietnamese titles; resume capsule and 12:48 mini-player are visible; three outlined navigation actions fit | lower catalogue content continues below the scroll viewport; no native insets or focus observation |
| `jvm-light-discovery-840x840.png` | discovery, light, 1×; **840×840** | two-column work groups, all three synthetic works, wide resume/mini-player and evenly distributed bottom navigation are visible | this is a sized JVM surface, not a tablet/native layout run |
| `jvm-dark-player-390x844.png` | player, dark, simulating, 1×; **390×844** | full `Người giữ tiếng chuông` title, 12:48/28:42 slider labels, pause/seek/bookmark/queue actions and explicit no-audio explanation are readable | purple artwork placeholder is intentional; no audio, elapsed-time advancement or motion is observed |
| `jvm-dark-error-390x844.png` | discovery error, dark, 1×; **390×844** | localized simulated-error heading/reason and retry action remain visible; resume point and mini-player persist | pixels alone do not prove retry; the corresponding JVM interaction test does |
| `jvm-light-library-390x844.png` | saved library, light, 1×; **390×844** | selected `Đã lưu` filter, both saved synthetic works and selected `Tủ sách` navigation are visible | bookmark/empty variants are covered by tests, not by this saved-view image |
| `jvm-light-settings-390x844.png` | contextual Settings from search, light, 1×; **390×844** | Back, memory-only warning, selected theme/locale/speed/timer and wrapping speed/timer choices are readable | catalogue-state/reset tools continue below the viewport; no timer execution or OS preference change |
| `jvm-light-work-text2-320x800.png` | work detail, light, 2× app text; **320×800** | `Sơn Hà Ký`, Vietnamese description and episode metadata wrap; header warning and bottom navigation labels remain readable | large header/footer reduce the content viewport; mini-player title is visibly ellipsized, and episode/actions require scrolling |
| `jvm-light-work-text2-episodes-320x800.png` | same work, scrolled to `episode-SonHa03`, light, 2×; **320×800** | current episode title, number, 28-minute duration, current-episode label and play glyph are visible after scroll; navigation labels wrap fully | previous episode is intentionally partially outside the viewport; mini-player still ellipsizes its title; no native touch or reader claim |

No actionable overlap or clipping defect was observed in the eight selected JVM viewports.
This permits intentional ellipsis and scroll clipping described above; it is not a complete
state, contrast, font-metrics or reference-fidelity audit. The source uses canonical
paper/lavender roles, serif hierarchy and tonal grouping; reference artwork and Noto resources
remain unbundled. Exact art/font fidelity is not claimed.

| PNG | SHA-256 |
| --- | --- |
| `jvm-light-discovery-390x844.png` | `06ff685585a9eff3f5ebca8101ec4a77650c9ac9f7e91276a8be622ab7931ea7` |
| `jvm-light-discovery-840x840.png` | `57d2f42b0b3aa8cbd8e5db2f84cb8f07230308eef41b0539dcb77f501375f536` |
| `jvm-dark-player-390x844.png` | `f2854f6bd7f3a99b140da72ff9b17bbdf87a93809b496996aa2d109d3f3b8cf3` |
| `jvm-dark-error-390x844.png` | `b264516c957e9ac681d97f736fe6a85ae5f05ce42c108450c6675ddb69b94422` |
| `jvm-light-library-390x844.png` | `c2d59ec40f509ec8c190edfbf81684a3f7c098c99c98ff719fa663d6feda6188` |
| `jvm-light-settings-390x844.png` | `aa17bedf309033dbbedfddb693e55d25541963edabff1cc513147728b004a0c4` |
| `jvm-light-work-text2-320x800.png` | `a6ee9726b7285321716d67319d1f2ee2d65fc3304e7837d0b1d7e8b9ee31caf1` |
| `jvm-light-work-text2-episodes-320x800.png` | `9cd0afe7dd58818f7c84d06991d3411ac236594f566e9df9325c4211a3d57f3c` |

The ten passing app tests separately drive search/clear, saved-work toggling, loading/error
recovery, contextual Back, focused-control Escape, queue/close, bounded slider semantics,
bookmarks and complete reset. At 320×800/2×, the selected episode action is scrolled into view,
has its full localized accessible name and Button role, stays within horizontal bounds and
retains a height of at least 48 dp at this renderer's density. This is bounded JVM
`interaction-tested`, `semantics-tested` and focus evidence. No full keyboard-order/focus-return
walk, TalkBack/VoiceOver, native text input or screen-reader traversal ran. Static captures
establish `screenshot-inspected`, `cross-viewport-inspected` and `cross-theme-inspected` only.
Motion/profile and native reduced-motion observations remain **NOT_RUN**.

## Native gaps and next work

This host exposes Xcode 16.3, an iOS 18.4 simulator runtime, Android SDK 35, Gradle 8.14.3 and
JDK 21. Kotlin/Native 2.2.20 and the official Android CLI/Quail 2+ were not found in the
inspected current-host locations. The Android mock APK build is passing, while iOS framework
build and current Studio render remain **UNSUPPORTED** for the recorded prerequisites.
Historical PR43 Rabbit/CLI captures are separate evidence.
No toolchain, TLS, credential, truststore or system-permission setting was changed.

Next: obtain missing native build/runtime/preview evidence on the current inputs. The
[phase2 architecture record](architecture.md) records its rerun/recapture against the refactored
owners; these phase1 images cannot certify changed inputs by themselves.
Qualify shared components in both renderers before adoption. Contract/native-media adapters, rights-aware downloads and
durable progress follow 050 prerequisites. Native insets/back, TalkBack/VoiceOver, OS text
scaling, background/lock-screen controls, interruptions, motion and audio quality remain gaps.

## Pre-merge acceptance run (2026-10-12)

Linux host without Gradle 8.14.3, so `cargo xtask theatre-demo verify` is **UNSUPPORTED** here.
As a diagnostic only, installed Gradle 9.7.0 with JDK 17 ran `:shared:desktopTest`:
48 tests, 0 failures/errors/skips, and all eight captures were opened. Two UI defects were fixed
and recaptured: multi-line episode rows and the continue card used the default pill button
shape, which became an oval at 2× text; they now use the theme's large card shape. Language
choices now use endonyms (Tiếng Việt / English), matching Theatre Web. `cargo xtask preflight
--changed-only` passed every gate except the explicitly skipped postgres, cmp-ui-preview and
theatre-cmp-demo. Native Android/iOS runtime evidence is unchanged from the record above.
