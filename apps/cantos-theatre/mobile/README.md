# Cantos Theatre CMP mock

This is an isolated, memory-only **Cantos** listener demo: shared Kotlin screens, an Android
host, an iOS SwiftUI host and a JVM diagnostic window. It does not complete native listening
in [050](../../../docs/work-plan/050-cmp-native-listening.md). A fake engine acknowledges mock
commands; the default mock has no API, authentication, native media engine, audio, downloads,
sync, billing, provider or durable persistence. The separate opt-in native identity boundary
below never feeds authenticated data into mock state. Studio is web-only.

## Opt-in native identity

`NativeAccountController` owns the native protocol and account lifecycle on the process/main
dispatcher. `NativeIdentityClient` calls separate `/api/v1/theatre/native/*` endpoints over
the exact configured HTTPS API origin. The native client is public: no Studio/Web client
secret is embedded. Fresh state/nonce/verifier and S256 bind the system-browser callback to a
durable server flow. The client consumes its pending flow before one exchange and never retries
a code after an unknown outcome. Provider credentials remain on the backend; the OS store
contains only a distinct opaque Cantos native-session credential.

Restore is an explicit user action, never automatic on launch. It requires an absent durable
logout marker, a successful OS-store read and a fresh native session GET. Logout clears the
account projection immediately and writes a **nonsecret** durable denial marker before HTTP
revocation or OS deletion. Unknown HTTP revocation retains the same credential for an idempotent
DELETE retry. Failed deletion retains the marker and blocks restore/new login until cleanup is
acknowledged. Returning account requests carry `X-Cantos-Theatre-Session`; a mismatched response
is discarded before decoding and triggers session revalidation without revoking another account.
Signed-in mode reads the actual Saved page only; no mock list, history event or playback is
created by signing in.

Android uses Keystore AES-GCM and an encrypted AtomicFile under `noBackupFilesDir`, external
ACTION_VIEW, bounded/no-cookie/no-redirect HttpURLConnection and a separately synced nonsecret
marker. iOS uses Keychain `AfterFirstUnlockThisDeviceOnly`, ASWebAuthenticationSession, bounded
ephemeral URLSession with no cookies/cache/redirects, and a synced backup-excluded nonsecret
marker. Swift adapters are now included in the Xcode Sources phase and injected into the shared
`IosNativeOwner`; no credential enters SwiftUI state, preferences or plaintext files.

Operator-owned **nonsecret** configuration keys are `CantosApiOrigin` and `CantosNativeIssuer`,
read from Android application metadata / iOS Info.plist. Both must be supplied; the issuer must
be the exact HTTPS `/oauth2/openid/cantos-theatre-native` issuer, and the callback is exactly
`com.cantos.theatre:/oauth2/callback`. This repository supplies no live values, OAuth registration,
credentials or TLS override. Android host/manifest mounting is still blocked by automatic
approval review of the ordinary INTERNET permission and callback filter; the unapplied
source-only proposal is retained outside Git at `/tmp/cantos-native-android-manifest-proposal.md`.

Common regression tests exercise actual controller/client code with deterministic HTTP/OS
ports, including one-time exchange, unknown outcomes, logout deletion/restart barriers, late
account results and strict session/Library decoding. The current patch has not yet run native
builds, Keychain/Keystore IO, device/browser flows or UI/accessibility acceptance; those are
separate gates, not established by source integration or fake ports. Native playback, durable
Library synchronization, downloads and broad issue acceptance remain deferred.

## Screens and behavior

- Two primary destinations: **Khám phá / Của tôi**. Search belongs to Discovery;
  the avatar opens a secondary **Không gian của bạn** screen: guest profile, a preview notice that
  saved works and positions last only for this session, a disabled sign-in with its reason, and a
  row into the listening settings. Nothing there claims durable storage or sync.
- Của tôi displays **Nghe tiếp, Đã lưu and Lịch sử** directly, without library tabs.
  History has a secondary full list and confirmed clear action; clearing it preserves
  listening positions, saved works, bookmarks and the player.
- Vietnamese search, work details and desired-state **Lưu / Bỏ lưu** actions on cards/details.
  Resume opens the same synthetic episode at its saved position, paused. History records only
  acknowledged simulated starts, including episodes at their end; opening a page creates none.
- Mini/full player: bounded manual seek, duplicate-safe bookmarks and episode queue.
- Contextual settings from the account screen: light/dark, vi/en UI, speed/timer preferences, manually selected
  discovery scenarios and reset of the complete demo state.

Transport explicitly **simulates** state. Time never advances on its own; timer and speed
schedule nothing. State disappears on restart. Android system Back, visible Back/close and
JVM Escape use the same pure navigation transitions. Play expresses intent; only a matching
fake-engine acknowledgement confirms simulated running. Native back/insets and screen readers
still need target evidence.

The [#35 CMP library MVP record](../../../docs/evidence/theatre-cmp-library-mvp.md) scopes this
follow-up to parity with the merged [Web mock PR #83](https://github.com/loveoverflowcom/cantos/pull/83).
Account sync, durable Saved/history/progress, real immutable-release resume and guest/account
policy remain with #11–#17 and #34; this mock does not complete #35.

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
framework exists. Xcode's `Copy Compose resources` phase copies the common font/art/license
resources into `compose-resources/composeResources/com.cantos.theatre.resources`, the location
read by pinned Compose 1.9.0 with this manually linked static framework. It does not invoke or
download Gradle. Physical-device linking/signing and
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

## Listener HTTP boundary

`shared/.../adapters/http` now has a closed GET-only `ListenerClient`, explicit origin
configuration, error taxonomy, cancellation/session generation owner and a JVM HTTP adapter
shared with the opt-in Android source set. It probes only the actual reserved
`/api/v1/theatre` group.404 `not_found` means `Unsupported(ListenerApi)`, and200never supplies
fake catalog or media. Publication, identity, playback, downloads and progress report explicit
unsupported capabilities. No Studio cookie, producer bearer or Studio DTO enters this port.

The existing Android/iOS/desktop app hosts remain **isolated mock previews** through
`AppContainer.mock`; they do not mount this adapter as a real player. Constructing a
`ListenerClient(ApiOrigin.parse(...), JvmHttpTransport())` crosses a real HTTP boundary but
cannot fetch any listener resource yet. TLS checks stay platform-default; HTTP is limited to
explicit loopback development. Redirects, caches and ambient cookie runtimes are refused;
responses are bounded to8KiB. The future iOS concrete HTTP adapter and listener session require
the reviewed Theatre backend contract before app wiring. No iOS transport integration is claimed.

`cargo xtask theatre-demo verify` compiles the common/JVM code, executes the shared
[error fixtures](../../../packages/contracts/fixtures/client-boundary/v1/README.md), exercises
noncooperative late responses and drives the JVM adapter against a fresh local HTTP server.
These are common/JVM and mocked HTTP integration evidence, not Android/iOS device or playback
acceptance. See the [client handoff](../../../docs/work-plan/live-client-api-matrix.md).

## Reference UI repair

The [UI repair evidence](../../../docs/evidence/theatre-cmp-ui-repair.md) covers adaptive header/
episode rows, exact reference paintings and OFL Noto font resources, the tonal mini-player,
ordered icon transport and preference dialogs. Theme/language/speed/timer still live in the one
app-owned mock state; catalog scenarios and reset are under the separate Demo tools dialog.
The simulated timer performs no countdown. The [asset provenance](shared/src/commonMain/composeResources/files/provenance.json)
records exact hashes and the remaining production artwork-rights review. Native runtime,
TalkBack/VoiceOver and iOS framework/resource acceptance remain separate from JVM evidence.
