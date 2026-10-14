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
credentials or TLS override. The user authorized the previously held Android INTERNET,
callback-filter and Application/MainActivity source changes on 14 October 2026. Those changes
are now mounted: one process owner, `singleTask` callback delivery and an opt-in identity screen
when both validated configuration keys are present. Missing or invalid configuration keeps the
memory-only mock. No live origins are supplied in the manifest, and no app was installed or OS
network/TLS setting changed by this source integration.

The cleanup controller retains the exact exchanged credential even when OS persistence fails
or revocation has an unknown outcome. A pending durable denial marker with a missing, corrupt
or invalid credential stays closed: it cannot manufacture a revoke acknowledgement or clear
its own barrier to start another account. Controlled recovery for a lost credential is still
unassigned; no automatic reset or deletion workaround is supplied.

Common/JVM regression tests exercise actual controller/client code with deterministic HTTP/OS
ports, including one-time exchange, unknown outcomes, logout deletion/restart barriers, late
account results and strict session/Library decoding. The 14 October resumed run passed all 207
common/JVM tests, including 28 identity tests. Four production callback-query tests reject
encoded field-name aliases before Android URI value decoding; platform callback dispatch still
needs OS evidence. The canonical Android gate passed as **Theatre mock Android build** and the
packaged manifest confirms INTERNET, callback routing and `singleTask`. The iOS simulator
framework gate is **UNSUPPORTED** without the required installed Kotlin/Native 2.2.20 compiler.
Keychain/Keystore IO, device/system-browser/lifecycle flows and UI/accessibility acceptance
remain **NOT_RUN**. See the [resumed evidence](../../../docs/evidence/theatre-identity.md#resumed-native-identity-acceptance--14-october-2026).
Native playback, durable Library synchronization, downloads and broad issue acceptance remain
deferred.

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
  adapters/mock/     synthetic fixtures, catalog/memory/fake-storage adapters and fake acknowledgements
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

Other wrappers cover dark player/error, 840 dp discovery and 320 dp work at 2× text, plus the
`TheatreFreshSessionPreview` / `TheatreRestoredSessionPreview` fake-storage fixtures below. Capture
needs the official Android CLI and correct synced IDE project; the old SDK `android` executable
is insufficient. Open PNGs and inspect exported semantics before asserting evidence.
JVM tests capture the same app under `shared/build/reports/theatre-demo`, or the selected
`CANTOS_THEATRE_DEMO_CAPTURE_DIR`. Those captures are not OS screenshots or native runtime tests.

## Fake storage simulator

`adapters/mock/FakeStorageDatabase` is a deterministic, **memory-only** session store for testing
the Nghe tiếp / Đã lưu / Lịch sử / Dấu trang UX before a local metadata store is chosen.
`FakePersistentProgressPort(database, FakeStorageConfig(...))` implements the unchanged
`ProgressPort`; several ports and recreated `ListeningController`s may share one database, and
a recreated owner sees stored data only after an explicit `reload()`. A new database starts
empty, like `MemoryProgressPort`, and process death discards both.

- Every read and write, including its opt-in read/write latency (default `0`, never negative),
  runs inside one fair mutex owned by the database. Operations from all ports therefore commit
  in request order: a slow older write cannot land after a newer accepted one.
- `failNextWrites(n)`, `failAllWrites()` (storage full) and `recoverWrites()` are storage-level
  faults consumed only at a write's commit point. A refused write changes nothing and returns
  `Unavailable`; the controller's existing retry semantics apply.
- `reset(seed)` replaces the contents immediately and fences every write requested before it;
  a fenced write returns `Unavailable` and consumes no fault. Cancellation propagates, never
  commits and consumes no fault. `database.state` is a read-only inspection value.
- `FakeStorageSessionApp(Fresh | Restored)` mounts the production `CantosDemoApp` through
  `AppContainer.mock(..., progressPort = ...)` with a database seeded from exactly the
  controller's start snapshot. The two Android `@Preview` wrappers and the JVM UI/capture tests
  render that same function. There are no interactive storage controls in the app UI.

`cargo xtask theatre-demo verify` runs these common/JVM tests. They are `example-tested` /
`mock-integration-tested` evidence over a fake: they establish no durable persistence, SQLite
schema or migration, Android/iOS process death or device behavior.

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

## Portable Protobuf fixture harness

The isolated `commonMain` transport adapter and `commonTest` fixtures exercise the four existing
Studio pilot wire contracts under [ADR 0023](../../../docs/decisions/0023-versioned-protobuf-wire.md).
This adds no Theatre endpoint, Studio Mobile client, authentication or player integration.
The [compatibility evidence](docs/protobuf-compatibility.md) records exact version pins, independent
byte/value oracles, hostile-input bounds, reproducible checks and separate JVM/Android/iOS limits.
