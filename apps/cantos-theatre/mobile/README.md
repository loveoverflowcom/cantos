# Cantos Theatre CMP mock

This is an isolated, memory-only **Cantos** listener demo: shared Kotlin screens, an Android
host, an iOS SwiftUI host and a JVM diagnostic window. It does not complete native listening
in [050](../../../docs/work-plan/050-cmp-native-listening.md). A fake engine acknowledges mock
commands; no API, authentication, native media engine, audio, downloads, sync, billing, provider
or durable persistence is connected. Studio is web-only.

## Screens and behavior

- Discovery, Vietnamese search, work details and episode selection.
- Library: continuing, saved and bookmark views; empty/search/loading/error recovery.
- Mini/full player: bounded manual seek, duplicate-safe bookmarks and episode queue.
- Contextual settings: light/dark, vi/en UI, speed/timer preferences, manually selected
  discovery scenarios and reset of the complete demo state.

Transport explicitly **simulates** state. Time never advances on its own; timer and speed
schedule nothing. State disappears on restart. Android system Back, visible Back/close and
JVM Escape use the same pure navigation transitions. Play expresses intent; only a matching
fake-engine acknowledgement confirms simulated running. Native back/insets and screen readers
still need target evidence.

## Structure

```text
shared/src/commonMain/kotlin/com/cantos/theatre/
  app/               AppContainer, navigation, read-only projection and keyed vi/en copy
  domain/            pure publication/playback/listening models and transitions
  application/       independent catalog/listening/preferences owners and one playback loop
  ports/             CatalogPort, ProgressPort and observation-only MediaEngine
  adapters/mock/     synthetic fixtures, catalog/memory adapters and fake acknowledgements
  features/          stateless app-screen presentation
  demo/              immutable diagnostic seeds and presentation metadata
shared/src/commonTest/      canonical vectors, reducers, fake-port/lifetime/reset regressions
shared/src/desktopMain/     diagnostic host around the same app
shared/src/desktopTest/     JVM interaction/semantics and fixed-size app captures
shared/src/androidMain/     thin @Preview wrappers around the same app
shared/src/iosMain/         ComposeUIViewController composition root
androidApp/                process owner, Activity, native Back and system-bar appearance
iosApp/                    SwiftUI application owner and Xcode scheme; consumes built framework
```

The mobile Gradle project directly compiles the canonical `packages/ui-system/cmp` source
as the isolated Android preview does. Only its theme/tokens are used. No package source,
palette, registry or qualification is copied/changed. The explicitly approved shared
[mock playback corpus](../../../packages/contracts/fixtures/theatre/playback/v1/README.md)
is imported unchanged for tests; it is not a listener API DTO. App compositions use stable
native Material controls and official outlined icons; no reusable Cantos primitive is introduced.
Registered Cantos wrappers remain unused and **0/29 qualified**. Their future adoption still
requires fresh [two-renderer admission](../../../.agents/skills/cantos-ui-design/references/preview-catalog.md).

## Local commands

Repository operations use [`cargo xtask`](../../../xtask/README.md). Put installed Gradle
**8.14.3**, a full **JDK 17+** and Rust **1.87** on the environment. Existing pins remain Kotlin
**2.2.20**, Compose **1.9.0**, Material3 **1.8.2**, AGP **8.11.1**; JVM bytecode targets Java 17.
No command installs Gradle/JDK/SDK/native compiler or weakens TLS.

```sh
RUSTUP_TOOLCHAIN=1.87.0 cargo xtask theatre-demo build
RUSTUP_TOOLCHAIN=1.87.0 cargo xtask theatre-demo verify
RUSTUP_TOOLCHAIN=1.87.0 cargo xtask theatre-demo run

# Existing platform 35 + build-tools 35.0.0 in ANDROID_HOME / ANDROID_SDK_ROOT.
RUSTUP_TOOLCHAIN=1.87.0 cargo xtask theatre-demo android-build

# Full Xcode and already installed Kotlin/Native 2.2.20 distribution.
RUSTUP_TOOLCHAIN=1.87.0 cargo xtask theatre-demo ios-build \
  --kotlin-native-home /absolute/path/to/installed/kotlin-native-2.2.20
```

Android output: `androidApp/build/outputs/apk/debug/androidApp-debug.apk`. Open this Gradle root
in Android Studio with `-Pcantos.android=true` for sync, then select `androidApp` to run on an
existing device/emulator. Diagnostic Android API 23 and iOS 15 floors are demo build settings,
not a production support decision. Desktop verification registers no native target.
`cantos.android`, `cantos.ios` or `cantos.native` opt into native targets. iOS refuses a missing/
unpinned installed compiler before target registration; SDK/JDK and native dependency downloads
are disabled.

`ios-build` links only the simulator framework. Open `iosApp/CantosDemo.xcodeproj`, choose the
shared `CantosDemo` scheme and an existing Apple Silicon simulator to build/run after the
framework exists. Xcode does not invoke/download Gradle. Physical-device linking/signing and
distribution are outside this mock. JVM `run` supervises its owned process group, with a
foreground-launcher PID guard; it is not an Android/iOS run. Production
`build/run cantos-theatre --mobile` stays `UNSUPPORTED`: native listening adapters are absent.

## Android Studio preview

After Android sync/build in **Quail 2+**, follow the existing
[capture workflow](../../../.agents/skills/cantos-cmp-mobile/references/compose-preview.md):

```sh
cargo xtask compose-preview cantos-theatre \
  --file apps/cantos-theatre/mobile/shared/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt \
  --composable TheatreDiscoveryLightPreview --label discovery-light-390
```

Other wrappers cover dark player/error, 840 dp discovery and 320 dp work at 2× text. Capture
needs the official Android CLI and correct synced IDE project; the old SDK `android` executable
is insufficient. Open PNGs and inspect exported semantics before asserting evidence.
JVM tests capture the same app under `shared/build/reports/theatre-demo`, or the selected
`CANTOS_THEATRE_DEMO_CAPTURE_DIR`. Those captures are not OS screenshots or native runtime tests.

## Design and acceptance

The [Drive Mobile folder](https://drive.google.com/drive/folders/1i34c_09-Ccq1K21yjxFXyarBk1lgvhy0)
contains the inspected board and readable listener handoff. The repo's
[UI system](../../../docs/design/ui-system.md) remains token authority: warm paper, lavender,
serif editorial hierarchy, tonal grouping and official navigation/action icons. Reference art,
fonts, manuscripts and audio are not redistributed in Git. The player shows an artwork
placeholder; canonical theme typography retains its documented platform fallback. Exact art/font
fidelity is not claimed. Work titles and `Người giữ tiếng chuông` are reference labels; other
fixture text/durations are synthetic. UI copy is keyed in vi/en; content titles stay Vietnamese.
See the [phase1 checkpoint](docs/mock-skeleton.md) and [architecture/evidence record](docs/architecture.md)
for actual commands, vector provenance, reset fences and gaps.

Production retains the [mobile architecture](../../../docs/architecture/mobile.md) and 050 gates:
listener adapters, one native media owner, background controls, verified rights-aware downloads,
durable progress, lifecycle, native text scaling and TalkBack/VoiceOver. This mock supplies no
playback-quality or production-acceptance evidence.
