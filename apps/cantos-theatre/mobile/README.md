# Cantos Theatre CMP mock

The [PR137 Membership design follow-up](../../../docs/evidence/pr137-cmp-design-v2.md) composes
six reviewed shared primitives through scoped v0.5.0 tokens and permanently labeled tonal
fields. `MembershipPreviews.kt` adds Android layout-renderer specimens around the actual app:
form and receipt in both themes, unpublished periods in both themes and a narrow 2× form.
Build them with `cargo xtask theatre-demo android-build`; capture through `cargo xtask
compose-preview cantos-theatre` only with a ready IDE. JVM image output remains a separate
renderer and does not establish device, IME, OS Back or screen-reader acceptance.

This is an isolated, memory-only **Cantos** listener demo: shared Kotlin screens, an Android
host, an iOS SwiftUI host and a JVM diagnostic window. It does not complete native listening
in [050](../../../docs/work-plan/050-cmp-native-listening.md). A fake engine acknowledges mock
commands; the default mock has no API, authentication, native media engine, audio, downloads,
sync, billing, provider or durable domain persistence. Native hosts persist the nonsecret UI
language in platform key/value preferences; ordinary previews remain session-only unless injected.
The separate opt-in native identity boundary
below never feeds authenticated data into mock state. Studio is web-only.

The KMP build separates production-safe `:shared` from `:preview-sandbox`. The shared module
owns domain/application logic, ports, non-mock adapters and the actual `CantosPresentation`
UI with neutral immutable `ShellState`/`ShellEvent` models. The sandbox owns synthetic data,
mock adapters, `CantosDemoApp`, diagnostic session/projector wiring and previews. It depends on
`:shared`; shared main sources never depend back on it and provide no implicit mock factory.
Demo hosts select the sandbox explicitly. The production Android host `androidProductionApp`
(`com.cantos.theatre`) composes `:shared` only. With an admitted API origin it composes the
[live public catalog](#live-public-catalog) (browse only: no production player, progress or
media adapter exists); otherwise it renders the explicit `CantosListeningUnavailable` state.
It never falls back to a mock.
`cargo xtask theatre-host android-release` builds its unsigned, unshrunk release APK/AAB and
inspects the release classpaths, DEX classes, resources and manifest; a demo APK cannot accept
that requirement. The [#122 record](../../../docs/evidence/issue122-cmp-sandbox-isolation.md)
records current checks separately from the historical results cited below.

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
marker. Swift adapters are included in the Xcode Sources phase and injected into shared
`IosNativeIdentityOwner`; the sandbox's `IosNativeOwner` delegates to it and composes the demo UI.
No credential enters SwiftUI state, preferences or plaintext files.

Operator-owned **nonsecret** configuration keys are `CantosApiOrigin` and `CantosNativeIssuer`,
read from Android application metadata / iOS Info.plist. Both must be supplied; the issuer must
be the exact HTTPS `/oauth2/openid/cantos-theatre-native` issuer, and the callback is exactly
`com.cantos.theatre:/oauth2/callback`. This repository supplies no live values, OAuth registration,
credentials or TLS override. The user authorized the previously held Android INTERNET,
callback-filter and Application/MainActivity source changes on 14 October 2026. Those changes
are now mounted: one process owner, `singleTask` callback delivery and the optional account
boundary when both validated configuration keys are present. That boundary is no longer a separate
screen: it is composed into the [guest shell](#guest-first-account-and-auth) (the #130 change edits
only `MainActivity`'s composition and the Application's container wiring, not the manifest, callback
filter or callback delivery). Missing or invalid configuration keeps the guest-only mock with Sign in
disabled and explained. The build inputs below default to empty; the repository supplies no live
origins and changes no OS network/TLS setting.

### Native identity build inputs and operator diagnostics

For Android, set the nonsecret environment variables `CANTOS_API_ORIGIN` and
`CANTOS_NATIVE_ISSUER` before the existing `cargo xtask theatre-demo android-build` command.
Alternatively, put `cantos.apiOrigin` and `cantos.nativeIssuer` in an operator-owned Gradle
user-home `gradle.properties`, outside Git. Gradle properties take precedence over the
environment, including an explicit empty value to disable identity. The selected values flow
through manifest placeholders into `CantosApiOrigin` / `CantosNativeIssuer` application metadata.
The package remains `com.cantos.theatre.demo`; configured identity does not replace the mock
catalog, player or storage with production adapters.

For iOS, the existing `CantosDemo` target's Debug and Release build settings
`CANTOS_API_ORIGIN` / `CANTOS_NATIVE_ISSUER` default to empty. Both configurations consume the
minimal `CantosDemo/Info.plist` source through `INFOPLIST_FILE`, expand its two Cantos keys and
merge the generated system keys. Supply values as explicit Xcode build-setting overrides or an
operator-owned Xcode configuration; plain environment variables may lose to the target defaults.
The shared scheme builds `com.cantos.theatre.demo`. The existing
`cargo xtask theatre-demo ios-build --kotlin-native-home /absolute/installed/native-2.2.20`
links only the simulator framework; open the existing Xcode project and Run the host separately
when native acceptance is authorized. Framework linking does not prove the built app's Info.plist.
No iOS build or toolchain installation was performed for the #191 slice.

Both hosts call the same pure `NativeClientConfiguration.resolve` before creating a controller.
It retains the existing HTTPS API-origin and exact native-issuer/callback rules. It does not
normalize origins or accept a Studio/Web issuer. Inspect Android Application's
`nativeIdentityAvailability` or iOS owner's `availability` in the debugger:

| Outcome | Meaning / next check |
| --- | --- |
| `Missing(ApiOrigin/Issuer/Both)` | A required key is absent or empty; check build inputs and selected artifact |
| `Invalid(ApiOrigin/Issuer)` | A supplied value fails the existing strict configuration grammar; correct that key |
| `Available` | Client configuration is syntactically valid; API/provider/session acceptance remains separate |

Missing keys take diagnostic precedence: if either key is absent/empty, `Missing` identifies
those keys before validating the other value. Once both are supplied, invalid API origin is
reported before invalid issuer. Every unavailable outcome keeps identity disabled.

The Android sandbox owns its debug-only `ui-tooling:1.9.0` runtime as well as preview annotations.
Android Studio renders the sandbox module directly, so tooling declared only in `androidApp`
does not supply its `ComposeViewAdapter`. This dependency is conditional on the Android opt-in;
it adds nothing to desktop/iOS-only or Android release configurations.

Missing/invalid diagnostics contain enum fields only, never the supplied strings. Android
preserves non-string manifest literals for invalid-value refusal instead of treating them as
missing. iOS creates no account controller or logout-barrier directory for unavailable config.
The existing guest UI retains its generic unavailable explanation; these operator diagnostics
do not add another user-facing auth state machine.

The backend must independently have `CANTOS_THEATRE_NATIVE_ISSUER` configured, an authorized
public client/callback, reachable HTTPS API/provider and an existing account. No client secret
belongs in either app. See the [activation evidence and acceptance boundary](../../../docs/evidence/theatre-auth-activation.md)
for Web configuration, preview instructions and outstanding artifact/device/provider checks.

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

## Native launcher icons

The Android and iOS demo hosts consume generated native launcher assets from the checksum-pinned
[Cantos Logo Kit v2](../../../packages/ui-system/shared/assets/cantos-logo-kit-v2/Logo-Guide.md).
These resources are independent of the launch presentation below. Both Android manifest icon
attributes resolve to `ic_launcher`: opaque density-specific legacy PNGs for API23–25 and a
108dp adaptive descriptor for API26+, with light/night foreground/background and a tintable
monochrome layer for supporting API33+ launchers. The C/bar paths and stroke ratios are unchanged;
the monochrome layer uses the official knockout paths as positive ink. OS masking owns the outer
tile; no rounded tile is nested in the adaptive foreground. The iOS `AppIcon` catalog supplies
opaque 1024px default/dark sources and is selected by both Xcode target configurations. No custom
tinted iOS appearance is supplied; OS tint acceptance remains separate.

Regeneration extends the existing `scripts/ui_logos.py` primitive with `--native-launcher`
(installed `rsvg-convert 2.60.0` required, never downloaded); `--native-launcher --check` verifies
the checked-in resources without a renderer. [Generated provenance](launcher-icons.provenance.json)
binds source and output hashes. The repository gate runs the focused source/resource regressions;
`cargo xtask theatre-demo android-build` checks APK compilation. See the
[launcher evidence and pending native acceptance](../../../docs/evidence/theatre-native-launcher-icons.md).

## Local launch presentation

Android's real launcher installs pinned AndroidX SplashScreen compat before `super.onCreate`,
with canonical light/dark backgrounds and the official symbol in the no-icon-background safe
canvas. There is no keep condition, splash Activity or animation hold. iOS uses a background-only
adaptive-color launch storyboard; the Swift application owner is unchanged.

`CantosLocalLaunch` is an app-owned presentation seam, independent of mock models. Ready content
replaces the optional tonal/logo/tagline frame on the next composition, without an exit wait or
navigation event. The current native owners initialize synchronously before UI creation, so they
always bypass that optional CMP frame. Pending mode is exercised only by static previews and
controlled JVM tests; no fake timer or asynchronous bootstrap is introduced. Destination network
and account failures remain destination concerns. An eventual real local loader must supply its
own terminal readiness/recovery and native OS motion preference before enabling pending mode.

The first diagnostic frame initializes the existing preferences owner from system appearance;
subsequent mounts retain the same owner. Official logo curves are converted into local vector
resources from the issue's supplied kit, with [provenance and evidence](../../../docs/evidence/theatre-cmp-launch.md).
No shared wrapper is newly qualified. Shared Logo Kit/component admission (#127) and
production release artifact isolation (#122) are separate acceptance work; the identity guest-shell
composition (#130) is the [guest-first account slice](#guest-first-account-and-auth) below.
This launcher change does not complete production mobile or those issues.

## Application UI language

`PreferencesController` owns the selected UI language: valid saved canonical tag (`vi-VN` or
`en`) wins over an explicitly injected host/preview seed, whose default is Vietnamese. Device
language negotiation and broad catalog/library adoption remain open under #167. Android/iOS
demo hosts inject SharedPreferences/UserDefaults; the production Android unavailable host also
reads the same app-owned preference boundary within its own application sandbox. Theme remains
memory-only. Native API acceptance schedules persistence; it is not a forced disk-flush guarantee.
Pre-acceptance save refusal preserves the choice, displays localized feedback and permits retry.

Shell/auth/Membership retain their typed catalogs. Five existing launch/unavailable messages
select explicit generated XML IDs by that same locale, because pinned Compose 1.9.0 has no public
subtree resource-locale override. No process locale changes or locale-keyed remounts are used.
This narrow adapter is not a catalog migration or library selection. The
[comparison](../../../docs/evidence/cmp-localization-spike.md) and
[acceptance ledger](../../../docs/evidence/issue169-cmp-locale-acceptance.md) separate JVM,
Android and deferred iOS/product evidence; issue #169 remains open.

## Guest-first account and auth

[Issue #130](https://github.com/loveoverflowcom/cantos/issues/130) composes Auth–Account design
v2 into the one guest shell; the [evidence record](../../../docs/evidence/theatre-cmp-auth-account.md)
holds the matrix, provenance and acceptance gaps. The app still opens on Discovery. Account is an
optional avatar entry and Login is a separate explicit intent, never a wall, so
`NativeIdentityApp` no longer replaces Home when identity is configured: the Android Application
passes its process-owned `NativeAccountController` into `AppContainer`, which only observes it and
never closes it. `AccountView` is the immutable projection (no actor, session, expiry, name or page
count); a missing identity capability is `Unavailable`, a guest shell and not a failure.

- **Navigation** is the existing pure reducer plus `Login`/`Register`: a closed `Login[, Register]`
  suffix, never the root, whose return destination is the typed stack below it. `OpenLogin`,
  `OpenRegister` and `AuthCompleted` are idempotent; a completion that arrives after Back or any
  other destination is inert, and navigating elsewhere dismisses auth first.
- **Lifecycle** stays in `NativeAccountController`. `AppContainer.back()` cancels a pending login
  before popping, so a late callback is fenced; `beginLogin()` is accepted only on the open Login
  route in a resting guest state (a double tap or pending flow cannot restart it) and
  `openLogin()` is refused for a signed-in listener. Provider cancellation leaves Login and its
  failure visible, and the browser is reopened only by a new explicit tap. Restore remains an
  explicit action that says when the device holds nothing. Logout hides the projection first; a failed
  cleanup keeps Login disabled with a retry. A Membership draft is cleared only when a signed-in
  account ends, so Membership → Login → success resumes the guest's draft.
- **Presentation** (`features/auth`): Login uses the vertical **Đăng nhập với Cantos ID** primary and
  **Tạo tài khoản Cantos** tonal actions from the #127 auth variants, with no tabs, legal links or
  consent, and no password field. Native only opens the system browser with PKCE. Registration is an
  explained **unavailable** step because no supported Cantos ID sign-up entry is verified; it never
  collects an email or password or shows success. Guest and signed-in Account use the Profile,
  Membership and AccountGroup surfaces; signed-in shows no invented name, total, conflict or
  entitlement. Auth hides the tab bar and mini-player; the header gives priority to Back and the theme
  toggle and shows the logo only in leftover space. Back (visible, Android system and JVM Escape) is one
  intent; scroll positions are kept per stack entry and Back from auth restores focus to Sign in or
  the avatar.
- Still unrun: Android system-browser/callback/Keystore flows, iOS compilation and ASWebAuthenticationSession
  (the pinned Kotlin/Native compiler is absent), TalkBack/VoiceOver, IME, font scale on a device and the
  live provider. The #35 two primary destinations remain; Account is not a third tab.

## Screens and behavior

- Two primary destinations: **Khám phá / Của tôi**. Search belongs to Discovery;
  the avatar opens a secondary **Không gian của bạn** screen: a guest profile with vertical Sign in
  and Create account actions (a single disabled Sign in with its reason when no identity is
  configured), a Membership card and the listening settings. The shell's session-only library
  and positions are not an account library and sync is not claimed.
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
  app/               explicitly injected AppContainer and pure navigation
  domain/            pure publication/playback/listening models and transitions
  application/       independent catalog/listening/preferences owners and one playback loop
  ports/             CatalogPort, ProgressPort and observation-only MediaEngine
  adapters/          HTTP and identity boundaries; no mock adapters
  features/          actual stateless UI, CantosPresentation and auth/Membership presentation
  presentation/model/  neutral ShellState, ShellEvent, catalog projection and vi/en copy
shared/src/commonTest/      core/typed/controller and fixture-only codec tests
shared/src/jvmHttpMain/     JVM/Android HTTP implementation
shared/src/iosMain/         native identity adapter/controller owner
preview-sandbox/src/commonMain/kotlin/com/cantos/theatre/
  adapters/mock/     synthetic catalog fixtures/mapping, MockCatalogPort and MockMediaEngine
    sandbox/         memory-only mock app sandbox shared by demo hosts, previews and tests
      database/      MockStorageDatabase and MockPersistentProgressPort behind ProgressPort
      sessions/      MockStorageSession presets: Demo, Fresh, Restored
  app/               mockAppContainer, CantosDemoApp, DemoAppSession, diagnostic projectors/slots
  demo/              synthetic seeds/IDs; no duplicate UI models
preview-sandbox/src/commonTest/  mock/port contracts, application/lifetime/reset regressions
preview-sandbox/src/desktopMain/ diagnostic host and workbench around the shared UI
preview-sandbox/src/desktopTest/ JVM interaction/semantics and fixed-size app captures
preview-sandbox/src/androidMain/ thin @Preview wrappers around the same UI
preview-sandbox/src/iosMain/     demo ComposeUIViewController composition root
androidApp/                explicit demo process owner, Activity, Back and system-bar appearance
iosApp/                    SwiftUI application owner and Xcode scheme; consumes built framework
```

The shared module directly compiles the canonical `packages/ui-system/cmp` source
as the isolated Android preview does, excluding its `com/cantos/ui/preview/**` diagnostics.
Shell screens use its theme/tokens; Membership composes the six reviewed shared APIs described
above. No package source, palette, registry or qualification is copied/changed. The explicitly approved shared
[mock playback corpus](../../../packages/contracts/fixtures/theatre/playback/v1/README.md)
is imported unchanged for tests; it is not a listener API DTO. Shell compositions use stable
native Material controls and official outlined icons; no reusable Cantos primitive is introduced.
The current [preview catalog](../../../packages/ui-system/shared/preview-catalog.json) owns
qualification scope/freshness. Any further shared API or variant adoption still requires fresh
[two-renderer admission](../../../.agents/skills/cantos-ui-design/references/preview-catalog.md).

## Local commands

Repository operations use [`cargo xtask`](../../../xtask/README.md). Put installed Gradle
**8.14.3**, a full **JDK 17+** and Rust **1.98.1** on the environment. Existing pins remain Kotlin
**2.2.20**, Compose **1.9.0**, Material3 **1.8.2**, AGP **8.11.1**; JVM bytecode targets Java 17.
No command installs Gradle/JDK/SDK/native compiler or weakens TLS.

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo build
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo verify
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo run

# Existing platform 35 + build-tools 35.0.0 in ANDROID_HOME / ANDROID_SDK_ROOT.
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo android-build
# Production host (shared only): unsigned release APK/AAB plus artifact inspection.
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-host android-release

# Full Xcode and already installed Kotlin/Native 2.2.20 distribution.
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo ios-build \
  --kotlin-native-home /absolute/path/to/installed/kotlin-native-2.2.20
```

To hand the **demo** APK to testers through Firebase App Distribution use the explicit
`cargo xtask distribute firebase` workflow; see [Firebase distribution](docs/firebase-distribution.md).
It distributes this mock only: it establishes no production readiness. The production host's
release is unsigned and has no listening adapter (its catalog browse is debug-only today), so
production APK/AAB distribution stays `UNSUPPORTED` (#14).

Build and verification invoke `:shared:assertProductionSafeDependencies` through main compile,
assemble and `desktopTest` dependencies. It checks resolved shared desktop compile/runtime
classpaths and, when Android is enabled, shared Android release compile/runtime classpaths for
sandbox projects and test-only dependencies. Its ignored report is
`shared/build/reports/production-boundary/resolved-dependencies.txt`. This is a library graph
guard; it does not inspect an app's release graph or APK/AAB.

The production host's `preBuild` runs `:androidProductionApp:assertReleaseDependencyBoundary`:
its resolved release compile/runtime classpaths may contain no local project except `:shared`
and no test, mock or Compose tooling module. `theatre-host android-release` then compiles the
sandbox's Android release classes only as a forbidden inventory and runs
`scripts/inspect_android_release.py`. It checks DEX class definitions, resource bytes/paths,
Compose resource packages and the `aapt2`-decoded manifests of the actual unsigned, unshrunk
APK and AAB. Reports are ignored files under `androidProductionApp/build/reports/release-boundary/`.
The release is never signed, installed or distributed by xtask. Actual execution results are
recorded in the [isolation evidence](../../../docs/evidence/issue122-cmp-sandbox-isolation.md).

Android output: `androidApp/build/outputs/apk/debug/androidApp-debug.apk`. Open this Gradle root
in Android Studio with `-Pcantos.android=true` for sync, then select `androidApp` to run on an
existing device/emulator. Diagnostic Android API 23 and iOS 15 floors are demo build settings,
not a production support decision. Desktop verification registers no native target.
`cantos.android`, `cantos.ios` or `cantos.native` opt into native targets. iOS refuses a missing/
unpinned installed compiler before target registration; SDK/JDK and native dependency downloads
are disabled.

`ios-build` links only the sandbox's `CantosTheatreDemo` simulator framework, which exports
`:shared`; the Swift import stays unchanged. Open `iosApp/CantosDemo.xcodeproj`, choose the
shared `CantosDemo` scheme and an existing Apple Silicon simulator to build/run after the
framework exists. Xcode's `Copy Compose resources` phase copies shared fonts/brand resources to
`compose-resources/composeResources/com.cantos.theatre.resources` and sandbox sample art/provenance
to `compose-resources/composeResources/com.cantos.theatre.preview.resources`, the namespaces
read by pinned Compose 1.9.0 with this manually linked static framework. It does not invoke or
download Gradle. Physical-device linking/signing and
distribution are outside this mock. JVM `run` supervises its owned process group, with a
foreground-launcher PID guard; it is not an Android/iOS run. Production
`build/run cantos-theatre --mobile` stays `UNSUPPORTED`: native listening adapters are absent.

## Membership interest preview

The mock Account screen has a labeled **Membership · preview** entry. Its default single
name/email form retains a session-only draft and reports real registration unavailable.
It creates no account, receipt, entitlement, payment or audio command. Plans are a separate
screen with unpublished illustrative monthly/annual layouts and no plan selection.
The native configured identity screen and Android/iOS hosts are unchanged.

The diagnostic desktop host alone can explicitly opt into manual fixture outcomes:

```sh
CANTOS_MEMBERSHIP_PREVIEW_FIXTURE=1 RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo run
```

Open Account → Membership preview. Fixture controls can confirm a receipt, fail a request,
mark its outcome unknown, display account completion, change plan loading/error states or
clear the private preview session. Nothing completes on a timer. Unknown retry keeps the
same operation and payload with a fresh callback fence; Back preserves the draft and fences
pending results. Session clear removes draft/receipt/errors without reusing request IDs.
The [scope and acceptance record](../../../docs/product/membership-interest.md) separates
these preview rules from deferred backend, native and visual acceptance. Test/capture
declarations become evidence only after the supported command executes and captures are opened.

## Android Studio preview

After Android sync/build in **Quail 2+**, follow the existing
[capture workflow](../../../.agents/skills/cantos-cmp-mobile/references/compose-preview.md):

```sh
cargo xtask compose-preview cantos-theatre \
  --file apps/cantos-theatre/mobile/preview-sandbox/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt \
  --composable TheatreDiscoveryLightPreview --label discovery-light-390
```

Other wrappers cover dark player/error, 840 dp discovery and 320 dp work at 2× text, plus the
`TheatreFreshSessionPreview` / `TheatreRestoredSessionPreview` mock-storage fixtures below. Capture
needs the official Android CLI and correct synced IDE project; the old SDK `android` executable
is insufficient. Open PNGs and inspect exported semantics before asserting evidence.
JVM tests capture the same UI under `preview-sandbox/build/reports/theatre-demo`, or the selected
`CANTOS_THEATRE_DEMO_CAPTURE_DIR`. Those captures are not OS screenshots or native runtime tests.

<a id="fake-storage-simulator"></a>

## Mock storage simulator

The `:preview-sandbox` module's `adapters/mock/sandbox/` is the one local-storage simulation used by
the demo hosts, Android `@Preview` wrappers and tests. It is **memory-only**; process death
discards it.

| Part | Location | Role |
| --- | --- | --- |
| Mock database | `sandbox/database/` | `MockStorageDatabase`, its read-only `MockStorageState` and the `MockPersistentProgressPort` connection behind the unchanged `ProgressPort` |
| Mock sessions / presets | `sandbox/sessions/` | `MockStorageSession` seeds: `Demo` (the sandbox factory default), `Fresh` and `Restored` |
| Mock app documents / directory | absent | no file I/O consumer exists; [ADR 0026](../../../docs/decisions/0026-theatre-mobile-local-storage-sandbox.md) (proposed) keeps a file-system double for #16 download staging, never for SQLite |

`MockStorageDatabase` is a deterministic session store for testing the Nghe tiếp / Đã lưu /
Lịch sử / Dấu trang UX before a local metadata store is chosen.
`MockPersistentProgressPort(database, MockStorageConfig(...))` implements `ProgressPort`;
several ports and recreated `ListeningController`s may share one database, and a recreated
owner sees stored data only after an explicit `reload()`. A new database holds only its seed.
The sandbox-only `mockAppContainer(scope, seed, storage = …)` connects the listening owner to one database
created by `storage` from the controller's exact start snapshot. With its default zero latency
and no faults, that database behaves as the former single-port `MemoryProgressPort` did (load
returns the last committed snapshot, store returns `Stored`), so the separate memory adapter
was removed rather than kept as a second mock. Shared `AppContainer` requires explicit ports
and `AppStartState`; no missing production dependency selects this factory automatically.

- Every read and write, including its opt-in read/write latency (default `0`, never negative),
  runs inside one fair mutex owned by the database. Operations from all ports therefore commit
  in request order: a slow older write cannot land after a newer accepted one.
- `failNextWrites(n)`, `failAllWrites()` (storage full) and `recoverWrites()` are storage-level
  faults consumed only at a write's commit point. A refused write changes nothing and returns
  `Unavailable`; the controller's existing retry semantics apply.
- `reset(seed)` replaces the contents immediately and fences every write requested before it;
  a fenced write returns `Unavailable` and consumes no fault. Cancellation propagates, never
  commits and consumes no fault. `database.state` is a read-only inspection value.
- `preview-sandbox/.../app/MockStorageSessionApp.kt` is diagnostic composition-root wiring:
  `mountMockStorageSession(scope, preset)` builds the container through `mockAppContainer` and
  exposes its database; `MockStorageSessionApp(preset)` mounts `CantosDemoApp`, which renders the
  actual shared `CantosPresentation`. Storage adapters never import the composition root.
  The `Fresh`/`Restored` Android `@Preview` wrappers and the JVM UI/capture tests render that
  function; `CantosDemoApp()` and the Android, iOS and desktop demo hosts use `mockAppContainer(scope)`,
  the `Demo` preset over its own database. There are no interactive storage controls in the app UI.

`ProgressPortContract` in `preview-sandbox/src/commonTest/.../ports` is the adapter-agnostic port behavior from
ADR 0026: exact round trip, whole-snapshot replacement, shared reopened connections, isolated
stores and no commit after cancellation. It is bound to this mock with and without latency;
a future durable adapter must pass the same suite, while latency, faults and reset fencing stay
mock-only. The suite and bindings move together, so shared tests have no reverse sandbox
dependency. The ADR substitutes each I/O layer with its own double: this mock for listening, a
file-system double for staged bytes and the real SQL engine (in memory or a temporary file) for
a metadata store. No storage library or durable adapter is adopted.

`cargo xtask theatre-demo verify` runs these common/JVM tests. They are `example-tested` /
`mock-integration-tested` evidence over a mock: they establish no durable persistence, SQLite
schema or migration, Android/iOS process death or device behavior.

## Scenario workbench

The desktop-only `preview-sandbox/src/desktopMain/.../app/workbench` host (`com.cantos.theatre.app.workbench`)
orchestrates the actual
`CantosDemoApp` over the existing mock ports and sandbox. It adds no second screen copy,
storage simulator, real file/HTTP/SQL adapter, auth flow or native audio engine. The sandbox's
`DemoAppSession` binds route UI inputs and dispatches the neutral `ShellEvent`s; shared screens retain
their immutable projection and intent boundary. Android static previews keep their immutable
seeds and are separate from the desktop's interactive controls.

```sh
CANTOS_THEATRE_WORKBENCH=1 RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-demo run
```

`TheatreRecipe.select` validates a versioned synthetic fixture, seed, injected `PreviewSettings`,
write latency and ordered `TheatreStep`s. Its fixture IDs are `theatre-fresh-v1`,
`theatre-restored-v1`, `theatre-empty-v1`, `theatre-loading-v1`, `theatre-error-v1` and
`theatre-offline-v1`. Fresh and Restored reuse #109's `MockStorageSession` seeds. Empty/Loading/
Error select existing catalog outcomes; Offline combines that unavailable catalog with refused
mock progress writes. It simulates no device connectivity or native offline entitlement.
Only seed 1 is supported, with at most 40 scripted intents and 0–5000 ms write latency.

`TheatreWorkbenchController` owns one `TheatreMount` at a time. Select and **Reset to selected**
remount that recipe; **Recreate** starts fresh controllers over the current synthetic projection.
Appearance settings preserve accepted query, route and listening values while injecting the
new theme, locale, density, scale, constraint and motion preference into the same owner/session
and composition. The diagnostic controls use the admitted shared `Surface` with zero insets,
an opaque canonical background and light/compact preferences; recipe preferences apply only to
the Theatre frame. The composition root supplies the system-theme fact without replacing the
selected owner. Snapshot transfer uses the admitted caller-controlled `TextField`; its compact
single-line viewport accepts the complete versioned payload for copy/paste. The event log is
read-only text. These diagnostics do not bypass shared-component admission.
**Replay** resets the selected fixture and dispatches its supported app intents in order on an
explicit simulation clock, 50 ms apart. The read-only projection and bounded event log expose
the outcome; controls use app intents rather than changing the mock database. Timer choices
still schedule nothing. Recorded simulation time establishes no native timer, background,
network, audio or process-lifecycle behavior.

Each owned mount has its own child scope, session and container. Replacement invalidates old
session callbacks, cancels replay/work, then closes that mount once; callbacks captured before
a later selection cannot act on it. Externally injected containers remain borrowed and are
closed only by their external owner. Verify recipe and lifecycle claims through the existing
`cargo xtask theatre-demo verify` route; Android/iOS behavior requires its own acceptance.
Lifecycle controls are explicitly unavailable for a borrowed container: selecting/resetting,
changing settings, replaying, recreating or restoring it cannot claim to rebuild its external
owner. Closing the workbench invalidates its binding without closing that container.

`encodeTheatreSnapshot` returns `TheatreSnapshotExport.Exported` or `UnsupportedState`;
`decodeTheatreSnapshot` returns `TheatreSnapshotResult.Accepted`, `UnsupportedVersion`,
`UnknownRecipe` or `Invalid`. The JSON schema is version 1 and bounds imported payloads at
65,536 characters, navigation at 32 entries and each bookmark/progress/history list at 40;
export validates those same limits before reporting success.
Only declared synthetic query IDs (empty, `Đèn`, `Sơn Hà`) are exported; other typed search text
stays local and export refuses it. Membership state, storage failures and transient or
inconsistent playback states cannot be exported or recreated. Settle/reset them first.
Supported stable snapshots contain recipe/settings, scripted intent IDs and synthetic UI values;
restore creates owners through the same injection seam. It serializes no scopes, controllers,
services, credential, native path, private text or media. Invalid payloads create no default
live adapter and leave the current mount intact.

The workbench imports only the self-contained shared preview settings directory in `desktopMain`;
`commonMain` does not reverse-import shared preview orchestration. This is the bounded #144
consumer seam. The sandbox has a separate KMP build boundary, and
[#122](https://github.com/loveoverflowcom/cantos/issues/122)'s production release graph and
APK/AAB exclusion are checked on the production host by `cargo xtask theatre-host android-release`.
No desktop preview, source-set check or mock APK build substitutes for that artifact proof.

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

## Live public catalog

Production-safe code in `:shared` reads the anonymous Theatre Discovery v1 catalog
([contract](../../../packages/contracts/theatre-discovery-v1.md), [ADR 0035](../../../docs/decisions/0035-theatre-catalog-unit-availability.md)):
work list → work detail → chapter/episode list, keyed only by the server's UUIDs.

- `domain/catalog/PublicCatalog.kt`: validated `PublicWorkId`/`PublicUnitId`/`PublicationId`/
  `CatalogCursor` and `PublicWork`/`PublicUnit`/page values. It is separate from the sandbox
  `EpisodeKey(work, index)` model; an index or `position` is never identity (`position` is display
  order only and is not shown as a number). Pages refuse a foreign `work_id`, a unit kind that does
  not match the work (novel/story → chapter, podcast → episode), blank titles, non-positive
  durations and duplicate ids.
- `adapters/http/PublicCatalogHttp.kt`: hand-written strict wire DTOs (unknown/missing fields,
  quoted numbers/booleans and unknown enum values fail), wire → domain mapping, the exact v1
  status/code/retryable error table and `PublicCatalogHttpAdapter`. Requests are anonymous GETs with
  `limit=20`, no cookie, credential or redirect, and a 4 MiB body bound. `jvmHttpMain/.../JvmCatalogTransport.kt`
  (HttpURLConnection, shared by desktop and Android) refuses ambient cookies and disconnects on
  cancellation. iOS has no catalog transport yet (honest unsupported; `iosMain` is unchanged).
- `PublicCatalogWireDriftTest` is a **drift check, not code generation**: the build embeds the
  strict schema and every original fixture (`generateDiscoveryCorpus`), and the test pins each DTO's
  property names, required set, JSON types, nullability and enum values to the schema and
  round-trips every fixture. Proposed contract code generation remains unimplemented.
- `application/catalog/PublicCatalogReducer.kt` + `PublicCatalogController.kt`: a pure reducer and
  one sequential `StateFlow` owner with token fencing. A failed page keeps its rows and retries the
  same cursor; an empty intermediate page (or one adding no new id) is followed automatically up
  to 5 times per action; `invalid_cursor` restarts from page one and merges by id (a repeat in the
  same action stops with a retryable "list changed"); a changed `publication_id` replaces its row.
- `features/catalog/presentation/PublicCatalogScreen.kt`: stateless Compose surface (vi/en copy in
  `ShellCopy`), Load more, Retry, empty/not-found/unavailable states and an explicit "listening not
  yet available" note. No row is playable.

Production host origin policy: the debug source set of `androidProductionApp` adds INTERNET, the
`CantosApiOrigin` meta-data and a network security config that permits cleartext only to
`127.0.0.1`/`localhost`. Runtime admits `https://` for any build and loopback `http://` only for a
debuggable build. Release declares neither INTERNET nor an origin and keeps `usesCleartextTraffic=false`,
so it renders the unavailable state: the release inspection admits no network permission yet.

```sh
# Debug production host against a local API through adb reverse (API 24+ for loopback http).
cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:8080
adb install -r apps/cantos-theatre/mobile/androidProductionApp/build/outputs/apk/debug/androidProductionApp-debug.apk
adb reverse tcp:8080 tcp:8080
# Opt-in live walk of a running API from the JVM (skipped unless the variable is set).
CANTOS_THEATRE_LIVE_ORIGIN=http://127.0.0.1:8080 cargo xtask theatre-demo verify
```

`android-debug` refuses any origin other than `https://` or loopback `http://` with a port, and
installs nothing. The [seeded catalog runbook](../../../docs/deployment/theatre-seeded-catalog.md)
provides a local API with published synthetic Works.

Evidence boundaries: reducer/domain/drift tests are JVM unit tests; `PublicCatalogHttpTransportTest`
is mock-integration against a JDK loopback server; `PublicCatalogPresentationTest` is the JVM
Compose renderer; `theatre-host android-release` is static packaging inspection. Android device,
TalkBack and iOS runtime remain not run.

## Listener HTTP boundary

`shared/.../adapters/http` now has a closed GET-only `ListenerClient`, explicit origin
configuration, error taxonomy, cancellation/session generation owner and a JVM HTTP adapter
shared with the opt-in Android source set. It probes only the actual reserved
`/api/v1/theatre` group.404 `not_found` means `Unsupported(ListenerApi)`, and200never supplies
fake catalog or media. Publication, identity, playback, downloads and progress report explicit
unsupported capabilities. No Studio cookie, producer bearer or Studio DTO enters this port.

The existing Android/iOS/desktop app hosts remain **isolated mock previews** through
the sandbox's explicit `mockAppContainer`; they do not mount this adapter as a real player. Constructing a
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
ordered icon transport and preference dialogs. Independent app owners project theme/language
and playback speed/timer; native hosts additionally persist UI language as described above.
Catalog scenarios and reset are under the separate Demo tools dialog.
The simulated timer performs no countdown. The sandbox's
[asset provenance](preview-sandbox/src/commonMain/composeResources/files/provenance.json)
records sample-art hashes and the remaining production artwork-rights review. Native runtime,
TalkBack/VoiceOver and iOS framework/resource acceptance remain separate from JVM evidence.

## Portable Protobuf fixture harness

The Studio pilot codec and its fixtures live only in `commonTest`
(`com.cantos.theatre.studiopilotfixtures`, beside `artworkfixtures`) and exercise the four existing
Studio pilot wire contracts under [ADR 0023](../../../docs/decisions/0023-versioned-protobuf-wire.md).
Shipping `commonMain` neither contains the codec nor links `kotlinx-serialization-protobuf`.
This adds no Theatre endpoint, Studio Mobile client, authentication or player integration.
The [compatibility evidence](docs/protobuf-compatibility.md) records exact version pins, independent
byte/value oracles, hostile-input bounds, reproducible checks and separate JVM/Android/iOS limits.
