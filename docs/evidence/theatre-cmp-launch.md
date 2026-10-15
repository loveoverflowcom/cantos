# Theatre CMP local launch — issue133

Bounded implementation on macOS, 14 October 2026. Base: `develop`
`b6066acc659b6d4cd030badb9c1abdad08b6dcf2`, independently matched to remote before branching.
Branch: `feat/issue133-theatre-native-splash`. The initial checkout was clean; unrelated refs,
worktrees, services, private story/audio data and the existing power configuration were preserved.

## Review boundary and sources

Delivers platform launch resources and app-owned optional CMP presentation for the existing
hosts. It does not activate production listening, complete release isolation, unify the opt-in
identity account boundary with guest Home, or qualify a shared UI-system component.
[Issue133](https://github.com/loveoverflowcom/cantos/issues/133) remains broader than this slice:
[#127](https://github.com/loveoverflowcom/cantos/issues/127),
[#130](https://github.com/loveoverflowcom/cantos/issues/130) and
[#122](https://github.com/loveoverflowcom/cantos/issues/122) retain their separate acceptance.
The implementation consumes already-present canonical token v0.3.0, not proposed auth variants.

Read the current issue/body, splash handoff, QA, source MJS/JSON, actual overview and layout PNGs,
and the linked Auth v2 handoff/Logo Guide. Reference images are browser design evidence.

- [Splash bundle](https://drive.google.com/file/d/1b4CYbkRCPg_QZrrHvZrX6wDiNahdb4v-/view)
- [Handoff](https://drive.google.com/file/d/12gpn5Cv5aflqrzziLyv5r8qNF9i4CEhr/view)
- [Design QA](https://drive.google.com/file/d/16hX8HsiI6NmAQtDvtmc56iZOyBdLEP65/view)
- [Auth v2 handoff](https://drive.google.com/file/d/1ETmdL9w1ywC10OqjPIrhV9ERy2l59tvJ/view)
- [Logo Guide](https://drive.google.com/file/d/1kdOLYIXvpOkyYmK3MJtyIgIeP57b7Kj1/view)

Original SVG checksums (SHA256), before vector-resource conversion:

| Source | Hash |
| --- | --- |
| cantos-symbol.svg | `5845decd2d648d92be5a8d11dde69525c31e58058e0d8f5819804fb827941cbd` |
| cantos-symbol-dark.svg | `2a8e60f2bfda6b26ff0b409ee149e14e7bb1acb29ef23c3ed3b22bfdb351ce53` |
| cantos-wordmark-cropped.svg | `1a99766b5b1d2cfa9ff0cf91cd1d27e7c687a49553e5020a6a9e8ed82bae6507` |
| cantos-wordmark-cropped-dark.svg | `2b1798faa65892d0d20c4b4e879e851bb0a1be87b49d29e1b7c4e36a1a3ae4c9` |

Existing symbol/wordmark curves and palette are retained. The SVG rounded rectangle is mapped
to equivalent native path geometry; the wordmark's 18-unit crop is a group translation.
No screenshot or new font is imported into launch resources. The existing licensed Noto Sans
resource supplies running-app copy. No redistribution license for the repository is inferred.

## Invariants and ownership

| Claim | Owner / oracle / evidence |
| --- | --- |
| Ready content wins over decoration | `CantosLocalLaunch`; next-frame JVM regression interrupts entrance well before completion |
| No invented wait or network gate | Launcher has no keep/exit condition; current local constructors finish before composition; offline guest test mounts real diagnostic Home |
| Launch never resets navigation | No navigation event/route is added; retained-container remount test preserves the Work destination; Android runtime retains Library across resume/configuration |
| Existing native identities stay separate | Callback/singleTask/process owner unchanged; account theme uses system appearance without changing protocol or credentials |
| Appearance and logo continuity | Source tests compare native mappings to canonical `color.background`; icon uses the 288 canvas with 128 symbol inside the 192 safe circle |
| iOS launch is background only | Adaptive named-color storyboard, no image/text/spinner; source contract and project parsing, not iOS build evidence |

The actual native owners are synchronous and always ready before their first UI composition.
Their `localReady = true` therefore **skips** the optional CMP frame. Pending presentation is
only a controlled preview/test input. There is no new readiness owner, startup framework,
timer, provider/auth/catalog hold, player creation, logout, automatic restore or autoplay.
No new `DemoState` dependency is added to the native hosts for launch. Existing mock composition
remains diagnostic, with production `--mobile` unsupported.

If a future real loader can remain pending or fail, its owner must supply terminal readiness,
typed recovery/retry and platform motion preference. This slice does not manufacture such a
loader or claim stale-completion/recovery acceptance for one that does not exist.

Android uses stable `core-splashscreen:1.0.1`, compatible with the existing API23 floor;
[release provenance](https://developer.android.com/jetpack/androidx/releases/core)
and [launcher ordering](https://developer.android.com/develop/ui/views/launch/splash-screen/migrate).
The compat call precedes `super.onCreate`. Native exit is OS-owned; there is no second Activity
or mandatory CMP brand frame. iOS follows the
[launch configuration](https://developer.apple.com/documentation/xcode/specifying-your-apps-launch-screen)
and [cache troubleshooting](https://developer.apple.com/documentation/technotes/tn3118-debugging-your-apps-launch-screen)
guidance; its Swift application owner remains unchanged.

## Evidence and commands

Environment: Rust 1.98.1, installed Gradle 8.14.3, JDK21 with Java17 target; existing Kotlin2.2.20,
Compose1.9.0, Material3 1.8.2, AGP8.11.1 and Android SDK35 pins retained. Commands use
`RUSTUP_TOOLCHAIN=1.98.1`, `TMPDIR=/private/tmp`, the installed Gradle/JDK/SDK paths and existing
local Gradle cache. Full logs and uncommitted media stay under ignored
`artifacts/agent-runs/issue133/`.

| Layer / gate | Result and evidence |
| --- | --- |
| L1 architecture | Reviewed app-owned presentation and unchanged process/container boundaries; no backend, shared wrapper, schema, permissions or new mock adapter |
| L2 domain/types | Inapplicable: no durable rule or new domain state; readiness decisions remain with existing synchronous owners |
| L3 repository | `cargo xtask verify repository`: PASS, 395 Python tests; canonical native colors, iOS background-only structure, vector placement and launcher ordering checked |
| L4 JVM presentation | `cargo xtask theatre-demo verify`: PASS, 252 tests /35 suites, zero failures/errors/skips; four new tests exercise the real common composable/diagnostic root |
| Preflight | `cargo xtask preflight --changed-only`: PASS, repository/rust-format/theatre-cmp-demo, no skips |
| Android compile | `cargo xtask theatre-demo android-build`: PASS, diagnostic debug APK; packaged launcher theme and original callback filter inspected |
| L5 Android diagnostic runtime | PASS for the bounded observations below, `device-tested` on API36 emulator; production identity/native playback and other targets remain unaccepted |
| iOS compile | `cargo xtask theatre-demo ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.4.20`: UNSUPPORTED; installed compiler is not the required2.2.20; no framework/app build ran |
| Android static preview | `cargo xtask compose-preview cantos-theatre --file apps/cantos-theatre/mobile/shared/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt --composable TheatreLocalLaunchLightPreview --label issue133-launch-light`: UNSUPPORTED, official Android CLI unavailable; no IDE was started |
| DB/API/provider | Inapplicable to this scope; none invoked or changed |

Initial failures are retained: one implicit layout receiver compiler error, two invalid test
imports, and a readiness test with a frozen recomposition clock. These were corrected and the
final suite passed; no application wait was introduced. The initial native video missed launch
and is inconclusive; the synchronized emulator-shell recordings are the actual launch evidence.
An unprivileged ADB read could not access the server socket; the authorized read succeeded.
No automatic approval rejection or pending approval remains.

## UI evidence

Surface: short local launch, Medium expression. Tokens: background, primary container and
on-surface-variant from v0.3.0; no shared token/output change. Geometry is app-local reference
mapping. Entrance uses existing `motion.emphasis.duration`260ms (within the320ms reference),
stable cubic easing; readiness removes it immediately. Reduced preference projects the final
frame synchronously; exit delay is zero. A real pending native loader/OS-motion adapter remains
unimplemented and cannot be inferred from the preview.

Visible/accessibility strings use Compose resources, default Vietnamese and English:
`splash_tagline`, `splash_category`, `splash_status`. Only the opening root is named; decorative
symbol, halo, wordmark/copy group and category produce no repeated accessible announcements,
live region, focus request, CTA or Back target. TalkBack/VoiceOver acceptance is NOT_RUN.

JVM renderer, density1: all16 final PNGs opened by Codex, light/dark ×vi/en ×390×844,
320×568,844×390 and320×568 at text scale2. Essential copy bounds are asserted on every render.
Names: `captures-final/splash-{light|dark}-{vi|en}-{width}x{height}-text{1.0|2.0}.png`.
Observations: intact official paths, landscape left-symbol/right-copy hierarchy, readable
Vietnamese diacritics, no copy clipping; nonessential category absent at2×. Compact selection
was corrected for inner padding, then the same matrix recaptured. These are
`screenshot-inspected`, `cross-theme-inspected`, `cross-viewport-inspected` and JVM
`semantics-tested`/`interaction-tested`, not native screenshots or screen-reader proof.

## Android native observations

Actual runner: installed `Medium_Phone_API_36` emulator, Android16/API36,1080×2400 physical
pixels,420dpi. The task started it with `-read-only -no-snapshot -no-window -no-audio -port5556`;
only this transient instance received the debug APK and test setting changes. No AVD was wiped,
no SDK installed, no original snapshot saved and no Mac power setting changed. Airplane mode
was enabled for the observations. No account/provider configuration or login was supplied.

| Driven observation | Evidence / result / limit |
| --- | --- |
| Offline cold guest | Native UI tree contains Cantos/Khám phá and diagnostic catalog, no login wall/startup status; compiled common/mock owners, not a live API |
| Light launch | Synchronized `screenrecord`/`am start -W` shell records one OS symbol layer then Home; opened `native-cold-light-frame-11.png`: complete C/sound bars, paper background, no cropped mask |
| Dark cold launch / OS reduced motion | Night mode yes and three OS animation scales0; opened `native-cold-dark-frame-03.png` and frame07: dark symbol layer and dark Home, no observed white app frame; pending CMP animation is skipped, not device-tested |
| Hot resume | Tap observed Của tôi node, Home key, reopen MainActivity. `native-library-before.xml` and `native-library-after.xml` have identical text, including paused12:48 fixture and Library; no navigation/owner reset observed |
| Configuration change | Night/font-scale2 change while Library open; `native-library-configuration.xml` retains Library/paused position, no startup status; large Library PNG inspected. In-memory app theme choice remains with preferences owner |
| Root Back / warm return | Back from Discovery, reopen: Android reports WARM; `native-back-return.xml` remains Discovery, with no navigable splash or startup status |

Opened video-derived OS frames and final JVM captures separately. Raw videos, PNGs, UIAutomator
XML and `am start -W` logs remain local under the ignored evidence directory. Video recording
limits are8s; encoded idle streams can have shorter timestamp duration and are not startup
performance measurements. Recorded activity times2280ms light/1324ms dark and warm174ms are
single emulator observations, not benchmarks or guarantees. The early unsynchronized12s
recording missed launch; it is retained as inconclusive, never used to accept a mask.

These observations are `device-tested`/`interaction-tested` for this diagnostic APK. Native
XML assertions cover destination presence and route retention, not TalkBack, screen-reader
focus or callback verification. Content deep-link routing and durable restore do not exist
in this diagnostic host; JVM remount preserves a supplied Work destination without claiming
OS link/restore acceptance. No fake link/router or identity configuration was added to fill
that gap.

## Deferred acceptance and residual risk

- iOS framework/native launch, clean install vs cached snapshot, orientation/appearance,
  Dynamic Type, Reduce Motion and VoiceOver: NOT_RUN. Requires the pinned installed compiler
  and Xcode host, then actual simulator/device install and launch. Owner: unassigned.
- Android API23 fallback and other device masks: NOT_RUN; no corresponding installed AVD.
  Real signed-in restore, content deep links and provider callbacks/secure-store acceptance:
  NOT_RUN, retain #130/identity acceptance; no identity configuration was supplied.
- Configured identity still mounts its separate account boundary. Guest-first production
  composition, async return fencing and shared logo/component admission remain #130/#127;
  this draft does not close those issues or issue133's full acceptance.
- Release/sandbox isolation remains #122; new presentation imports no demo models, but the
  inherited diagnostic host graph remains unqualified for release.
- No Kotlin formatter/linter is configured. Manual syntax/import review, compiler and tests
  are reported; Rust formatting does not establish Kotlin lint or iOS compilation.
- No startup performance certification, native pending-animation proof, real media session,
  durable state or network integration is claimed. Required production/native gates retain
  their owners; local passes and a draft PR do not accept them.
