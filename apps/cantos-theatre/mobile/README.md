# Cantos Theatre CMP

The Theatre listener application is one Kotlin Multiplatform composition. **`CantosTheatreApp`
(in `:shared`) is the application**: it mounts the neutral `CantosPresentation` over `TheatreShell`,
which interprets the familiar controls against the live public catalog, the native account, the
durable SQLDelight Library and the native player, and `projectTheatreShell` projects only those
owners into the screens. The single Android application `:androidApp` and iOS `Cantos`
(`com.cantos.theatre`) compose `:shared` only. Android has exactly two build types and no flavors:
`release` is `com.cantos.theatre` (label **Cantos**) and `debug` is the side-by-side
`com.cantos.theatre.dev` (label **Cantos Dev**, version suffix `-dev`). Both are real-only: with an
admitted API origin they mount `CantosTheatreApp`; otherwise they render the explicit
`CantosListeningUnavailable` state. They never fall back to a mock (#286). Studio is web-only.
See [what the application reads from where](#the-application-over-real-owners).

**Desktop Preview ([#287](https://github.com/loveoverflowcom/cantos/issues/287)).** `:preview-sandbox`
is a JVM-desktop-only module (one `desktop` target; no Android or iOS target, framework or source
set, and no Android SDK or Kotlin/Native needed) for the Desktop Preview, its scenario workbench and
its tests. It mounts the **same** `CantosTheatreApp` over the **same** `TheatreShell`, with the same
dispatch, projection, controllers and navigation as the native hosts. Only the *external* ports are
mocks, wired at one composition boundary (`PreviewOwners`): the public catalog, playback access, a
decoder stand-in, the identity transport and the remote Library API, plus locale storage. The
Library is the production SQLite store on an in-memory database with its real outbox. There is no
second reducer, domain model, navigation or auth handler. See
[the Desktop Preview](#desktop-preview) for its limits; it plays no audio, makes no network call
and establishes no native, device or persistence behavior. The Android and iOS runtimes contain no
preview or mock code, and neither resolves nor packages `:preview-sandbox`.

`cargo xtask theatre-host android-release` builds an isolated unsigned unshrunk release audit followed by optimized release APK/AAB and
inspects the release classpaths, DEX classes, resources and manifest, using the sandbox's compiled
JVM classes only as a forbidden inventory; a Desktop Preview build cannot accept that requirement.
The [#122 record](../../../docs/evidence/issue122-cmp-sandbox-isolation.md) records current checks
separately from the historical results cited below.

## Opt-in native identity

`NativeAccountController` is the one process/application-dispatcher owner of the native
Bearer lifecycle. `NativeIdentityClient` uses `/api/v1/theatre/native/auth/bearer` on the exact
configured HTTPS API origin. The public native client embeds no Studio/Web secret. State,
nonce, S256 and exact system-browser redirect bind one server flow and one exchange. Provider
credentials remain on the API; only scoped Cantos access/refresh credentials reach native.

`SecureNativeCredentialJournal` replaces one complete encrypted OS value, bound to exact API
origin and issuer. Its Active, Flow, Refreshing and Cleared records distinguish usable candidates
from cleanup-only recovery. Flow is persisted before code exchange; Refreshing with the old
family is persisted before rotation. An interrupted or unknown exchange is cancelled, and an
interrupted or unknown refresh revokes the old family; neither consuming request is replayed.
Failed write/readback still attempts known-flow/family cleanup, while the durable denial barrier
blocks adoption. A durable Cleared record makes a crash between revoke and marker deletion
recoverable. Old unbound single-token records are removed locally, never sent to a changed
origin or admitted as new Bearer; their original server lifetime remains bounded to 900 seconds.

The production runtime restores on launch through API confirmation, or one safe rotation when
only access has expired. Local credentials alone never expose an account or its SQLite rows.
The active process timer and all private requests share one refresh flight; cancelling a waiter
does not cancel or repeat rotation. Access is at most 120 seconds; original provider/proof,
idle and absolute authority remain server-owned and never extend through refresh.

Logout hides authority and durably marks intent before network cleanup. Local intent/rotation
leases reject late bodies, callbacks and cleanup from account A after B. Library HTTP borrows
the current access pair from that owner; same-family rotation preserves the player, local
checkpoint and exact outbox operations. Direct same-turn Library changes after logout also
check authority. No account token is put in a media URL: current public playback retains the
existing short-lived release capability and separate native media transport.

Android uses Keystore AES-GCM with full-record AtomicFile/readback/fsync in `noBackupFilesDir`,
ACTION_VIEW and bounded no-cookie/no-redirect HTTP. iOS uses Keychain
`AfterFirstUnlockThisDeviceOnly` with full-record readback, ASWebAuthenticationSession and
ephemeral no-cookie/no-cache/no-redirect URLSession. The synced nonsecret denial marker is
backup-excluded on both platforms. No credential enters Compose/SwiftUI state, SQLite,
preferences, logs or plaintext files. Source implementation and JVM examples do not establish
installed-device secure-store/process-death or native decoder acceptance.

Operator-owned **nonsecret** configuration keys are `CantosApiOrigin` and `CantosNativeIssuer`,
read from Android application metadata / iOS Info.plist. Both must be supplied; the issuer must
be the exact HTTPS `/oauth2/openid/cantos-theatre-native` issuer, and the callback is exactly
`com.cantos.theatre:/oauth2/callback` (production). The Android host resolves its closed
`NativeCallbackProfile` from the final application ID: release uses `com.cantos.theatre`, debug uses
`com.cantos.theatre.dev:/oauth2/callback` and the manifest scheme is `${applicationId}`, so
co-installed builds receive only their own callback
([contract](../../../docs/deployment/native-callback-profiles.md)). An older `.demo` install may
still register the production scheme; it is not removed here. This repository supplies no live values, OAuth registration,
credentials or TLS override. The user authorized the previously held Android INTERNET,
callback-filter and Application/MainActivity source changes on 14 October 2026. Those changes
are now mounted: one process owner, `singleTask` callback delivery and the optional account
boundary when both validated configuration keys are present. That boundary is no longer a separate
screen: it is composed into the [guest shell](#guest-first-account-and-auth) (the #130 change edits
only `MainActivity`'s composition and the Application's container wiring, not the manifest, callback
filter or callback delivery). Missing or invalid identity configuration keeps Sign in disabled and explained; the native
host has no mock fallback, and catalog availability depends on the separate API-origin input. The build inputs below default to empty; the repository supplies no live
origins and changes no OS network/TLS setting.

### Native identity build inputs and operator diagnostics

For Android, set the nonsecret environment variables `CANTOS_API_ORIGIN` and
`CANTOS_NATIVE_ISSUER` before `cargo xtask theatre-host android-debug` / `android-release`.
Alternatively, put `cantos.apiOrigin` and `cantos.nativeIssuer` in an operator-owned Gradle
user-home `gradle.properties`, outside Git. Gradle properties take precedence over the
environment, including an explicit empty value to disable identity. The selected values flow
through manifest placeholders into `CantosApiOrigin` / `CantosNativeIssuer` application metadata.
The Android application (`:androidApp`) reads these two build inputs in both build types; neither
build type hard-codes an origin, so the operator chooses it. Debug (`com.cantos.theatre.dev`) is a
new app-data sandbox: it does not inherit the database, tokens or Library of the former `.demo` or
the production install. Release keeps the `com.cantos.theatre` package, storage and signing/version
upgrade policy.

For iOS, the `Cantos` target's Debug and Release build settings
`CANTOS_API_ORIGIN` / `CANTOS_NATIVE_ISSUER` default to empty. Both configurations consume
`Cantos/Info.plist` through `INFOPLIST_FILE`, expand its two Cantos keys and merge the generated
system keys. Supply values as explicit Xcode build-setting overrides or an operator-owned Xcode
configuration; plain environment variables may lose to the target defaults. The shared `Cantos`
scheme builds the production host (`com.cantos.theatre`); `cargo xtask theatre-host ios-build
--kotlin-native-home /absolute/installed/native-2.2.20` links only the simulator frameworks, and
Xcode runs the host separately when native acceptance is authorized. Framework linking does not
prove the built app's Info.plist. There is no iOS mock host (#287): the former `CantosDemo` project
and its `CantosTheatreDemo` framework were removed, and the launch storyboard, privacy manifest,
asset catalog (icons) and `TheatreIdentity.swift` it shared with `Cantos` now live in `iosApp/Cantos`.

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

The Android and iOS production hosts consume generated native launcher assets from the checksum-pinned
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
`cargo xtask theatre-host android-debug --api-origin ORIGIN` checks APK compilation. See the
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
the production hosts inject SharedPreferences/UserDefaults; the Android unavailable host also
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
identity never replaces Home: the host passes its process-owned `NativeAccountController` into
`NativeSessionRuntime`, which `TheatreShell` only observes and never closes. `AccountView` is the immutable projection (no actor, session, expiry, name or page
count); a missing identity capability is `Unavailable`, a guest shell and not a failure.

- **Navigation** is the existing pure reducer plus `Login`/`Register`: a closed `Login[, Register]`
  suffix, never the root, whose return destination is the typed stack below it. `OpenLogin`,
  `OpenRegister` and `AuthCompleted` are idempotent; a completion that arrives after Back or any
  other destination is inert, and navigating elsewhere dismisses auth first.
- **Lifecycle** stays in `NativeAccountController`. `TheatreShell.back()` cancels a pending login
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
  and Create account actions, a Membership card and the listening settings.
- Của tôi displays **Nghe tiếp, Đã lưu and Lịch sử** directly, without library tabs, for the verified
  account that owns the Library; a guest is asked to sign in. History has a secondary full list and
  a confirmed clear action; clearing it preserves listening positions and saved works.
- Vietnamese search, work details and desired-state **Lưu / Bỏ lưu** actions on cards/details.
  Resume opens the same release at its saved position, paused. History records starts the player
  acknowledged; opening a page creates none.
- Mini/full player: bounded manual seek, speed, a sleep timer that pauses the real player and the
  episode queue. Bookmarks have no API and are not offered.
- Contextual settings from the account screen: light/dark, vi/en UI and speed/timer preferences.

Android system Back, visible Back/close and JVM Escape use the same pure navigation transitions.
Play expresses intent; only a matching engine observation confirms running. The Desktop Preview
shows exactly these screens with sample data and a decoder stand-in; native back/insets and screen
readers still need target evidence.

The [#35 CMP library MVP record](../../../docs/evidence/theatre-cmp-library-mvp.md) scopes this
follow-up to parity with the merged [Web mock PR #83](https://github.com/loveoverflowcom/cantos/pull/83).
Account sync, durable Saved/history/progress, real immutable-release resume and guest/account
policy remain with #11–#17 and #34; this mock does not complete #35.

## Structure

```text
shared/src/commonMain/kotlin/com/cantos/theatre/
  app/               CantosTheatreApp, TheatreShell, NativeSessionRuntime, pure navigation
  domain/            pure publication/playback/listening/catalog models and transitions
  application/       catalog, account, Library, player and preference owners
  ports/             PublicCatalogPort, PlaybackAccessPort, StreamingMediaEngine, identity and Library ports
  adapters/          HTTP, identity and SQLDelight Library boundaries; no mock adapters
  features/          actual stateless UI, CantosPresentation and auth/Membership presentation
  presentation/model/  neutral ShellState, ShellEvent, catalog projection and vi/en copy
shared/src/commonTest/      core/typed/controller and fixture-only codec tests, shared sample presentation values
shared/src/desktopTest/     JVM Compose presentation/layout and production-composition tests over test doubles
shared/src/jvmHttpMain/     JVM/Android HTTP implementation
shared/src/iosMain/         native identity adapter/controller owner
preview-sandbox/src/commonMain/kotlin/com/cantos/theatre/   (JVM desktop only; no androidMain/iosMain)
  adapters/preview/  mock EXTERNAL ports: PreviewCatalogPort, PreviewPlaybackAccess, PreviewStreamingEngine,
                     PreviewIdentity (real NativeAccountController over mock transport), PreviewLibraryApi,
                     PreviewLocalePreferenceStore and the synthetic Discovery fixtures (canonical UUIDs)
  app/               PreviewOwners (the one composition boundary), PreviewTheatreApp, PreviewSeed/PreviewSession
preview-sandbox/src/desktopMain/ Main.kt, the in-memory SQLite store and the scenario workbench
preview-sandbox/src/desktopTest/ owner-level flow/auth tests, JVM interaction, semantics and capture tests
androidApp/                the one Android application: process owner, Activity, Back, system bars; real-only
iosApp/                    the `Cantos` SwiftUI application owner and Xcode scheme; consumes the built framework
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
# Desktop Preview (JVM only: no Android SDK and no Kotlin/Native are consulted).
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview build
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview verify
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview run

# Existing platform 35 + build-tools 35.0.0 in ANDROID_HOME / ANDROID_SDK_ROOT.
# The Android application (shared only): unsigned optimized release APK/AAB, unshrunk audit, pre-shrink and artifact inspection.
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-host android-release

# Full Xcode and already installed Kotlin/Native 2.2.20 distribution: the production frameworks.
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-host ios-build \
  --kotlin-native-home /absolute/path/to/installed/kotlin-native-2.2.20
```

`theatre-demo` is the historical name of `theatre-preview` and runs the same Desktop Preview.
`theatre-preview android-build` (retired by #286) and `theatre-preview ios-build` (retired by
#287) are refused as `UNSUPPORTED` with the owning command named; they never build an Android or
iOS mock. The gate formerly named `theatre-cmp-demo` is now `theatre-cmp-preview` (the old id is
still accepted by `--skip`).

`cargo xtask distribute firebase` selects real-only `development` (`.dev` debug) or
`production` (optimized release), with exact package registration and pinned signer checks.
`demo`/`demo-qa` remain retired and refused. See the [release/distribution runbook](docs/firebase-distribution.md)
for isolated unshrunk audit, migration and independent device/live acceptance.

Build and verification invoke `:shared:assertProductionSafeDependencies` through main compile,
assemble and `desktopTest` dependencies. It checks resolved shared desktop compile/runtime
classpaths and, when Android is enabled, shared Android release compile/runtime classpaths for
sandbox projects and test-only dependencies. Its ignored report is
`shared/build/reports/production-boundary/resolved-dependencies.txt`. This is a library graph
guard; it does not inspect an app's release graph or APK/AAB.

`:androidApp` audits each build type separately: `preDebugBuild` runs
`:androidApp:assertDebugDependencyBoundary` and `preReleaseBuild` runs
`:androidApp:assertReleaseDependencyBoundary` (`assertDependencyBoundaries` aggregates both
for compatibility; a debug build never audits or assembles release). The resolved compile/runtime
classpaths of the variant may contain no local project except `:shared` and no test, mock or
Compose tooling module, transitively; the module has no `:preview-sandbox` dependency in any
configuration. Test classpaths are separate and not inspected. An AGP ALL scoped-class observer
also inspects actual class/JAR inputs before R8 plus source/resource fixture hashes for both
variants. `theatre-host android-release` first builds and inspects an unsigned unshrunk audit of
**release** under `androidApp/build-audit`, then optimized release APK/AAB under `androidApp/build`.
Sandbox source inventory is read without compiling a sandbox target. DEX, manifest, assets,
resources and non-vacuity checks remain separate from graph/pre-shrink checks. Pairing rejects
source/class/graph disagreement and binds exact artifact/mapping hashes in `release-evidence.json`.
Audit bytes carry a marker that distribution must reject. Local/CI builds are unsigned; explicit
operator signing is available only with the supplied release key. Nothing installs or distributes
without the separate operator command. Historical [isolation evidence](../../../docs/evidence/issue122-cmp-sandbox-isolation.md)
retains its original source and results.

Android output: `androidApp/build/outputs/apk/debug/androidApp-debug.apk`. Open this Gradle root
in Android Studio with `-Pcantos.android=true` for sync, then select `androidApp` to run on an
existing device/emulator. The API 23 and iOS 15 floors are library build settings, not a
production support decision. `cantos.android`, `cantos.ios` or `cantos.native` opt `:shared` and
`:androidApp` into native targets; **`:preview-sandbox` ignores all three**, so a Desktop Preview
sync, build or test never needs an Android SDK or Kotlin/Native. iOS refuses a missing/unpinned
installed compiler before target registration; SDK/JDK and native dependency downloads are disabled.

`theatre-host ios-build` links the production `CantosTheatre` simulator frameworks from `:shared`.
Open `iosApp/Cantos.xcodeproj`, choose the shared `Cantos` scheme and an existing Apple Silicon
simulator to build/run after the framework exists. Xcode's `Copy Compose resources` phase copies the
shared fonts/brand resources to `compose-resources/composeResources/com.cantos.theatre.resources`,
the namespace read by pinned Compose 1.9.0 with this manually linked static framework. It does not
invoke or download Gradle and copies no sandbox sample art or provenance (those are JVM resources of
`:preview-sandbox` only). Physical-device linking/signing and distribution are outside this
document. JVM `run` supervises its owned process group, with a foreground-launcher PID guard; it is
not an Android/iOS run. Production `build/run cantos-theatre --mobile` stays `UNSUPPORTED`.

## Membership interest preview

The Account screen has a labeled **Membership · preview** entry. Its default single
name/email form retains a session-only draft and reports real registration unavailable.
It creates no account, receipt, entitlement, payment or audio command. Plans are a separate
screen with unpublished illustrative monthly/annual layouts and no plan selection.
The native hosts never receive the fixture; `TheatreShell` starts Membership unavailable.

The Desktop Preview alone can explicitly opt into manual fixture outcomes (`TheatreShell(initialMembership = …)`):

```sh
CANTOS_MEMBERSHIP_PREVIEW_FIXTURE=1 RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview run
```

Open Account → Membership preview. Fixture controls can confirm a receipt, fail a request,
mark its outcome unknown, display account completion, change plan loading/error states or
clear the private preview session. Nothing completes on a timer. Unknown retry keeps the
same operation and payload with a fresh callback fence; Back preserves the draft and fences
pending results. Session clear removes draft/receipt/errors without reusing request IDs.
The [scope and acceptance record](../../../docs/product/membership-interest.md) separates
these preview rules from deferred backend, native and visual acceptance. Test/capture
declarations become evidence only after the supported command executes and captures are opened.

<a id="desktop-preview"></a>
<a id="mock-storage-simulator"></a>

## Desktop Preview

There is no Android or iOS preview host (#286, #287) and no Android Studio `@Preview` wrapper of the
sandbox: `cargo xtask compose-preview cantos-theatre` has nothing to render until a future,
production-safe stateless preview exists. The preview is the JVM desktop window and its tests.
`PreviewOwners` (`app/`) builds the owners the native hosts build and replaces only their external
ports:

| Owner (production class, unchanged) | External port replaced in the preview |
| --- | --- |
| `PublicCatalogController` | `PreviewCatalogPort` (`PublicCatalogPort`): the sample works and units under canonical UUIDs, source search, paging by cursor, and the `Ready`/`Loading`/`Empty`/`Error` scenarios |
| `NativeListeningController` + `AuthorizingMediaEngine` | `PreviewPlaybackAccess` (`PlaybackAccessPort`, with injectable refusal) and `PreviewStreamingEngine` (`StreamingMediaEngine`): a decoder stand-in that acknowledges commands and reports the resumed position; no audio, no clock |
| `NativeAccountController` | `PreviewIdentity`: a deterministic `NativeIdentityApi`, entropy, system browser, credential journal and logout barrier. `PreviewLoginMode` approves, holds or refuses the provider return; two sample accounts make account switching real |
| `NativeLibraryController` (`NativeLibrarySync`) | `PreviewLibraryApi` (`LibraryApi`): an in-memory remote with idempotent receipts, plus the real SQLite store (`previewLibraryStore()`, `:memory:`) so the transactions, outbox and account partitions are production code |
| `PreferencesController` | `PreviewLocalePreferenceStore` (`LocalePreferenceStore`) |
| `TheatreShell` | none: the same class, navigation, dispatch and `projectTheatreShell` |

Shared values the preview must not re-model are used as is: `PublicWork`/`PublicUnit`/page values,
`AudioIdentity`, `NativeGrant`, the Library wire types and `ShellState`/`ShellEvent`. A
`PreviewSeed` only describes where a scenario starts (route, theme, saved works, positions, whether
the first sample account is signed in); `PreviewOwners` applies it through the shell's own events
and the remote Library data, never by writing state. The only addition to `:shared` for this is the
optional `TheatreShell(initialMembership = …)` argument, which defaults to unavailable.

Limits, stated once: the stand-in engine produces no audio and never advances the position by
itself; the in-memory SQLite store and "Restored" are seeds, not evidence of disk persistence or of
process death; the identity ports prove the lifecycle, fencing and cleanup of the production owner,
not a live provider, secure storage or a system browser; JVM semantics and pixels are not device,
IME, OS Back or screen-reader evidence. Sample artwork and its
[provenance](preview-sandbox/src/commonMain/composeResources/files/provenance.json) stay in the sandbox
resources; the production fonts/brand resources stay in `:shared`.

## Scenario workbench

The desktop-only `preview-sandbox/src/desktopMain/.../app/workbench` host (`com.cantos.theatre.app.workbench`)
orchestrates `PreviewOwners` and mounts `PreviewTheatreApp` (the shared `CantosTheatreApp`) inside a
declared frame. It adds no second screen copy, auth flow, storage simulator or audio engine, and its
controls live outside the product navigation: the app has no "Fresh", "Restored", reset or failure
button.

```sh
CANTOS_THEATRE_WORKBENCH=1 RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview run
```

`TheatreRecipe.select` validates a versioned synthetic fixture, seed, injected `PreviewSettings`,
write latency and ordered `TheatreStep`s. Its fixture IDs are `theatre-fresh-v1` (a guest on the
Library tab), `theatre-restored-v1` (the first sample account, signed in, with saved works, continue
entries and history already on the remote), `theatre-empty-v1`, `theatre-loading-v1`,
`theatre-error-v1` (catalog scenarios) and `theatre-offline-v1` (a signed-in listener whose catalog
and remote Library are unreachable, so a save stays a durable, visibly pending intent). It simulates
no device connectivity or native offline entitlement. Only seed 1 is supported, with at most 40
scripted intents and 0–5000 ms remote Library latency. Steps reuse app intents: `OpenSonHa` opens the
work page and `PlayFirstSonHa` plays a unit the API has listed; the sleep timer is real and pauses
the stand-in player when it fires.

`TheatreWorkbenchController` owns one `TheatreMount` (its `PreviewOwners`, replay scope and counters)
at a time. Select and **Reset to selected** remount that recipe with new owners and a new in-memory
database; **Recreate** starts fresh owners from the current projection. `ShellEvent.Reset` and
`ShellEvent.CatalogMode` arriving through the fenced `callback()` are workbench scenario controls
(remount, and a catalog scenario change plus refresh); they are never forwarded to the product
shell, which has no such events. Appearance settings preserve accepted query, route and listening
values while injecting the new theme, locale, density, scale, constraint and motion preference into
the same owners and composition. The diagnostic controls use the admitted shared `Surface` with zero
insets, an opaque canonical background and light/compact preferences. Snapshot transfer uses the
admitted caller-controlled `TextField`; the event log is read-only text and records workbench
actions and replayed intents, while user taps are observed through the read-only projection.
**Replay** resets the selected fixture and dispatches its supported app intents in order on an
explicit simulation clock, 50 ms apart. Recorded simulation time establishes no native timer,
background, network, audio or process-lifecycle behavior.

Each owned mount has its own child scope, owners and database. Replacement cancels replay and work,
then closes that mount once; callbacks captured before a later selection cannot act on it.
Externally injected `PreviewOwners` remain borrowed and are closed only by their external owner.
Lifecycle controls are explicitly unavailable for borrowed owners: selecting/resetting, changing
settings, replaying, recreating or restoring cannot claim to rebuild an external owner.

`encodeTheatreSnapshot` returns `TheatreSnapshotExport.Exported` or `UnsupportedState`;
`decodeTheatreSnapshot` returns `TheatreSnapshotResult.Accepted`, `UnsupportedVersion`,
`UnknownRecipe` or `Invalid`. The JSON schema is **version 2** (the bookmark list was removed with
the bookmark capability, and `signedIn` was added) and bounds imported payloads at 65,536
characters, navigation at 32 entries and each progress/history list at 40; export validates those
same limits before reporting success. Only declared synthetic query IDs (empty, `Đèn`, `Sơn Hà`) are
exported; other typed search text stays local and export refuses it. Membership state, unsettled
Library writes or failures and transient or inconsistent playback states cannot be exported or
recreated. Settle/reset them first. Supported stable snapshots contain recipe/settings, scripted
intent IDs and synthetic UI values; restore creates owners through the same seed seam. It
serializes no scopes, controllers, services, credential, native path, private text or media.
Invalid payloads create no default live adapter and leave the current mount intact.

The workbench imports only the self-contained shared preview settings directory in `desktopMain`;
`commonMain` does not reverse-import shared preview orchestration. The sandbox has a separate KMP
build boundary, and [#122](https://github.com/loveoverflowcom/cantos/issues/122)'s production release
graph and APK/AAB exclusion are checked on the production host by `cargo xtask theatre-host
android-release`. No Desktop Preview, source-set check or mock build substitutes for that artifact proof.

`cargo xtask theatre-preview verify` runs `:shared:desktopTest` and `:preview-sandbox:desktopTest`.
They are `example-tested` / `mock-integration-tested` (production owners over named mock external
ports) / `DOM`-free `semantics-tested` and `screenshot-captured` evidence on the JVM renderer: they
establish no durable persistence, SQLite migration, Android/iOS process death, device behavior,
audio, live identity or live API behavior.

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
  cancellation. iOS production injects a bounded ephemeral URLSession delegate from
  `iosApp/Cantos/CatalogTransport.swift` into `IosCatalogOwner`; it uses HTTPS, no cookies,
  credentials/cache or redirects and cancels the request with its coroutine. Invalid UTF-8 and
  oversized bodies remain protocol failures.
- `PublicCatalogWireDriftTest` is a **drift check, not code generation**: the build embeds the
  strict schema and every original fixture (`generateDiscoveryCorpus`), and the test pins each DTO's
  property names, required set, JSON types, nullability and enum values to the schema and
  round-trips every fixture. Proposed contract code generation remains unimplemented.
- `application/catalog/PublicCatalogReducer.kt` + `PublicCatalogController.kt`: a pure reducer and
  one sequential `StateFlow` owner with token fencing. A failed page keeps its rows and retries the
  same cursor; an empty intermediate page (or one adding no new id) is followed automatically up
  to 5 times per action; `invalid_cursor` restarts from page one and merges by id (a repeat in the
  same action stops with a retryable "list changed"); a changed `publication_id` replaces its row.
  Validated query/kind changes discard rows and cursors, start `/search?q=...&kind=...` or
  `/works?kind=...` on the server and fence the old response. Pagination and retries retain the
  exact query; refresh rereads page one. Detail refresh rereads the Work and then restarts units,
  so a withdrawn Work cannot keep its old playable-unit projection.
- Catalog intents reach the presentation through `app/TheatreShell.kt`; the retired simplified
  `PublicCatalogScreen`/`NativeCatalogScreen` surface no longer exists. Search, Load more, Retry and
  refresh, empty/not-found/unavailable states and the cached-catalog notice are states of the
  familiar Discover, Search and Work screens (see below).

Origin policy: `androidApp` main declares INTERNET and the explicit
`CantosApiOrigin` metadata for both variants. Runtime admits `https://` in release; only a
debuggable build also admits `http://127.0.0.1|localhost` through its loopback-only network security
config. Missing/rejected configuration renders the unavailable state. There is no default origin,
fixture fallback, playback source or sandbox dependency. API failure/empty data stays a real
failure/empty state. Release keeps `usesCleartextTraffic=false`.

The separate shipping iOS project/scheme is `iosApp/Cantos.xcodeproj` / `Cantos`, bundle identifier
`com.cantos.theatre`. It imports the static `CantosTheatre` framework built directly by `:shared`,
links only `shared/build/bin/...` and copies only shared Compose resources. It is the only iOS
project: the former `CantosDemo` mock host was removed (#287). Shipping `CANTOS_API_ORIGIN` is an explicit Xcode build
setting (empty by default), consumed through `Cantos/Info.plist`; HTTPS is required on every iOS
configuration. It creates no native identity/session or media engine. The application owner closes
all catalog requests with its lifecycle. Xcode/Apple SDK builds and runtime observations require
macOS and are separate from JVM evidence.

```sh
# Debug production host against a local API through adb reverse (API 24+ for loopback http).
cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:8080
adb install -r apps/cantos-theatre/mobile/androidApp/build/outputs/apk/debug/androidApp-debug.apk
adb reverse tcp:8080 tcp:8080
# Production release catalog against an explicit HTTPS origin (no install/distribution).
cargo xtask theatre-host android-release --api-origin https://api.example.org
# Production iOS debug/release simulator frameworks (installed macOS toolchain required).
cargo xtask theatre-host ios-build --kotlin-native-home /absolute/installed/native-2.2.20
# Open iosApp/Cantos.xcodeproj, choose the Cantos scheme and set CANTOS_API_ORIGIN.
# Opt-in live API walk and production Compose flow (skipped unless the variable is set).
CANTOS_THEATRE_LIVE_ORIGIN=http://127.0.0.1:8080 CANTOS_THEATRE_REQUIRE_SEEDED=true cargo xtask theatre-preview verify
```

`android-debug` refuses any origin other than `https://` or loopback `http://` with a port, and
installs nothing. The [seeded catalog runbook](../../../docs/deployment/theatre-seeded-catalog.md)
provides a local API with published synthetic Works.

Evidence boundaries: reducer/domain/drift tests are JVM unit tests; `PublicCatalogHttpTransportTest`
is mock-integration against a JDK loopback server; `PublicCatalogPresentationTest` is the JVM
Compose renderer; `theatre-host android-release` is static packaging inspection. Android device,
TalkBack and iOS runtime remain not run. `PublicCatalogProductionFlowTest` mounts the production
screen/controller/HTTP adapter against a named fake HTTP peer to prove cold state, an arbitrary
UUID, Unicode server query, kind filter, error/retry, changed server title without rebuilding and
honest Library/Account presentation. It is `mock-integration-tested` on the JVM, never native or
live API evidence. `PublicCatalogLivePresentationTest` separately mounts the same production
composition against the configured running API, captures live list/detail/empty states and walks
unit pages and server search. `CANTOS_THEATRE_REQUIRE_SEEDED=true` requires a nonempty initial
catalog. For a backend-change/Refresh proof without rebuilding the client, set
`CANTOS_THEATRE_LIVE_REFRESH_DIR` to a fresh ignored directory: the test writes `catalog-ready`
with the first Work ID, waits up to 60 seconds for `catalog-changed`, clicks Refresh, requires a
changed first-page ID/title projection, captures it and writes `catalog-refreshed`. Only signal
after adding/republishing an eligible release through backend use cases; draft metadata alone
must not alter immutable released metadata. These optional tests do not add an app fallback.

## Listener HTTP boundary

`shared/.../adapters/http` now has a closed GET-only `ListenerClient`, explicit origin
configuration, error taxonomy, cancellation/session generation owner and a JVM HTTP adapter
shared with the opt-in Android source set. It probes only the actual reserved
`/api/v1/theatre` group.404 `not_found` means `Unsupported(ListenerApi)`, and200never supplies
fake catalog or media. Publication, identity, playback, downloads and progress report explicit
unsupported capabilities. No Studio cookie, producer bearer or Studio DTO enters this port.

The Desktop Preview is the only mock host; it replaces this adapter's *consumers' ports* with
`PreviewOwners` and does not mount this adapter as a real player. Constructing a
`ListenerClient(ApiOrigin.parse(...), JvmHttpTransport())` crosses a real HTTP boundary but
cannot fetch any listener resource yet. TLS checks stay platform-default; HTTP is limited to
explicit loopback development. Redirects, caches and ambient cookie runtimes are refused;
responses are bounded to8KiB. The future iOS concrete HTTP adapter and listener session require
the reviewed Theatre backend contract before probe wiring. This legacy probe is independent of
the implemented iOS public-catalog transport above.

`cargo xtask theatre-preview verify` compiles the common/JVM code, executes the shared
[error fixtures](../../../packages/contracts/fixtures/client-boundary/v1/README.md), exercises
noncooperative late responses and drives the JVM adapter against a fresh local HTTP server.
These are common/JVM and mocked HTTP integration evidence, not Android/iOS device or playback
acceptance. See the [client handoff](../../../docs/work-plan/live-client-api-matrix.md).

## Reference UI repair

The [UI repair evidence](../../../docs/evidence/theatre-cmp-ui-repair.md) covers adaptive header/
episode rows, exact reference paintings and OFL Noto font resources, the tonal mini-player,
ordered icon transport and preference dialogs. Independent app owners project theme/language
and playback speed/timer; native hosts additionally persist UI language as described above.
Catalog scenarios and reset are workbench controls, not product navigation. The sleep timer is the
real `TheatreShell` timer. The sandbox's
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


## Production native integration, step2

See the [step-2 implementation/evidence](../../../docs/evidence/theatre-native-step2.md) for the
pinned #247 dependency, real MediaEngine adapters, process-owned mini/full player, API-confirmed
native account, full Library/progress client and account SQLDelight/outbox. The `Cantos` iOS
scheme links the shared production framework and native Swift adapters; there is no demo scheme
(#287). Android debug (`com.cantos.theatre.dev`) is an installable QA artifact of `androidApp`, with actual
package/origin/signature/version/exclusion checks. Native device/audio/force-stop, iOS codec
compatibility and browser-facing identity remain acceptance gates; no device success follows
from JVM tests or packaging. Production distribution is opt-in and requires the named app,
certificate and recipients. Native registration/bookmarks/downloads are still unavailable.

## The application over real owners

`CantosTheatreApp` shows the established screens — Discover, Search, Work, Player, Queue,
Library, History, Account, Settings, Login/Register, Membership preview and the mini-player — with
the same layout, tokens, motion and copy. `ShellCatalogView`/`ShellState` are read-only
projections; identities are the API's lower-case UUIDs, never a sample key.

| Screen / control | Real owner |
| --- | --- |
| Discover, Search | `PublicCatalogController` (server `works`/`search`, cursors, cached offline page) |
| Work, chapters/episodes | `PublicCatalogController` detail + units walk; a count appears only when the walk is complete |
| Player, mini-player, Queue, speed | `NativeListeningController`/`PlaybackController` over `AndroidMediaEngine`/`IosMediaEngine` with API-authorized grants |
| Continue, History, Saved | `NativeLibraryController` (SQLDelight local SSOT, outbox, server sync); guests are asked to sign in |
| Account, Login | `NativeAccountController` (Bearer session, system browser, Keystore/Keychain) |
| Theme, language | `PreferencesController` (language persisted) |
| Sleep timer | local to `TheatreShell`: pauses the real player after 15/30 minutes |

Absent facts are absent, not invented: bookmarks have no API (the control is hidden), the Library
API returns identities without titles (a work or unit the process has not been shown is named
neutrally), artwork has no projection yet (the tonal placeholder remains) and an episode number is
never derived from `position`. Gaps are tracked in the BE issues linked from the PR and #180.

The [Android release/distribution runbook](docs/firebase-distribution.md) owns R8, same-release
audit pairing, exact staged APK/mapping hashes and the explicit development/production policies.
Device and live Firebase acceptance remain independent of source/build/audit results.
