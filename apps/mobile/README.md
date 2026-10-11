# Cantos Theatre mobile

Compose Multiplatform listener skeleton, separate from the creator Studio. `shared` owns theme,
Vietnamese/English copy, synthetic display fixtures, immutable navigation and Compose screens.
`androidApp` hosts it with an edge-to-edge Activity; `iosApp` hosts its static Kotlin framework
in SwiftUI. The JVM target compiles shared UI and runs common tests; it is not a shipping app.

There is no WebView, network client, account, entitlement, audio engine, background service,
download store or synchronization adapter. Player actions are disabled with a visible reason.
`PlaybackBoundary.state` explicitly remains `NotImplemented`. Later platform adapters must
consume reviewed publication/listening contracts.

## Toolchain and local commands

Pinned: Gradle 9.7.0 with official checksum, Kotlin/Compose compiler 2.4.20, CMP 1.12.0,
AGP 9.3.1, Java target 17, compile SDK 37, target SDK 36 and minimum Android API 24. CMP's
resolved Android dependencies require compile SDK ≥37; SDK36 cannot assemble this app. The iOS host
deployment target is 16.0; Kotlin targets are device arm64 and simulator arm64. Material3 is
the upstream companion `1.12.0-alpha03`, a prerelease dependency using ordinary Material
components here. Validate native targets before upgrading or adding expressive animation APIs.

```sh
cd apps/mobile
./gradlew --offline --no-daemon :shared:jvmTest
# Existing Android SDK and dependencies; no SDK auto-download:
ANDROID_HOME=/path/to/installed/sdk ./gradlew --no-daemon :androidApp:assembleDebug
# Installed Xcode / arm64 simulator toolchain and dependencies:
./gradlew --no-daemon :shared:compileKotlinIosSimulatorArm64
open iosApp/CantosTheatre.xcodeproj
```

Choose the shared **CantosTheatre** Xcode scheme. Its build phase calls
`:shared:embedAndSignAppleFrameworkForXcode` with one worker and `--no-daemon`. Both host
configurations declare arm64, matching the Kotlin targets; Intel/x86_64 simulators are unsupported.
Simulator builds need no signing team. Configure
your own team locally for a device. The wrapper may download its pinned Gradle distribution
if absent; `--offline` only controls dependency resolution. On constrained machines use an
installed distribution and do not install SDKs/emulators to force a pass.

`gradle.properties` disables SDK/JDK auto-download, caps workers at two and the heap at 1536 MiB.
Android build-tools 36.0.0 must already exist. Dependency downloads, native framework linking
and simulator runtime need separate disk budgets. Local SDK paths, build output, caches and
Xcode user settings stay outside Git.

Offline resolution does not prevent Kotlin/Native from trying to provision its compiler.
For a strictly preinstalled native toolchain pass `-Pkotlin.native.home=/path/to/compiler`
for Kotlin 2.4.20. Do not embed a machine-specific compiler/SDK path in this project.

## Boundary and validation

Discovery → work → player, search and library are navigation shells. All content is labeled demo.
The state menu exposes loading/empty/error/not-implemented examples and reset. Theme and density
map shared semantic tokens to native components; fonts use system fallbacks. No borrowed cover
art or embedded reference HTML is distributed.

The Debug SwiftUI host and static framework have compiled/linked for arm64 iOS Simulator with
installed Xcode 16.3 / SDK18.4. This is build evidence, not a simulator launch. Android assembly
remains blocked by absent SDK37; no SDK was installed to force a result. See the continuation
linked evidence record for exact commands and the two corrected SDK/architecture mismatches.

Three JVM tests exercise back/root navigation, unavailable playback and Vietnamese search
against actual common code. This host compiles all common UI but does not prove native rendering,
safe areas, gestures, TalkBack/VoiceOver or large fonts. The
[evidence record](../../docs/evidence/client-shells.md) lists Android/iOS build blockers and
**NOT_RUN** native interaction gates. This skeleton does not complete work-plan 050.

Follow the [mobile architecture](../../docs/architecture/mobile.md),
[surface map](../../docs/architecture/client-surfaces.md) and [UI system](../../docs/design/ui-system.md).
