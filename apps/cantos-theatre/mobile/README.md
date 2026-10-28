# Cantos Theatre CMP

The Theatre listener application is one Kotlin Multiplatform composition. **`CantosTheatreApp`
(in `:shared`) is the application**: it mounts the neutral `CantosPresentation` over `TheatreShell`,
which interprets the familiar controls against the live public catalog, the native account, the
durable SQLDelight Library and the native player, and `projectTheatreShell` projects only those
owners into the screens. The single Android application `:androidApp` and iOS `Cantos`
(`com.cantos.theatre`) compose `:shared` only. Android has exactly two build types and no flavors:
`release` is `com.cantos.theatre` (label **Cantos**) and `debug` is the side-by-side
`com.cantos.theatre.dev` (label **Cantos Dev**, version suffix `-dev`). Both are real-only: with an
admitted API origin (an explicit override, else the production default `https://api.cantos.cc`)
they mount `CantosTheatreApp`; a refused override or packaging renders `CantosListeningUnavailable`
with a typed configuration diagnostic. They never fall back to a mock (#286, #298). Studio is web-only.
See [what the application reads from where](#the-application-over-real-owners).

Discovery uses compact keyed Work cards, with one title/arrow navigation action and a separate
save action. Catalog completeness, filtered no-results and retained partial data remain distinct.
The app-owned empty artwork uses the supplied transparent Library PNG and theme-paired
Works/History XML through Compose resources; its provenance is bundled in
`shared/src/commonMain/composeResources/files/theatre_empty_artwork_provenance.json`.
The header theme action dispatches the existing preference intent, and Discovery opens the
existing query-on-change Search route. These compositions retain the current Standard Material
presentation. Qualified Logo Symbol, FloatingSearch and the Library Button are adopted in the
bounded continuation below. Avatar, tonal IconButton, FilterChip and UiRoot/Theatre expression
still await admission; see the [bounded #453 evidence](../../../docs/evidence/theatre-discovery-library-453.md).

The Search route now uses the shared FloatingSearch dock: one shell owns its stable input
header, trailing search action and the existing catalog result/state composition. Typing keeps
the existing query-on-change owner; the magnifier and IME Search dispatch `SubmitSearch`.
Collapse retains the raw query and restores input focus; clear keeps focus. The body scrolls
within the measured viewport after safe/IME insets. JVM desktop tests exercise the production
composition over declared preview ports; they do not establish Android/iOS keyboard acceptance.

**Public cover ([#159](https://github.com/loveoverflowcom/cantos/issues/159), ADR 0033).** Work
list, search and detail reads send `presentation=3` and strictly decode the metadata-capable DTOs
(unit reads stay on default v1). `PublicCover` admits only the exact
`https://{origin}/api/v1/theatre/works/{work}/cover/{sha256}` delivery shape; any other shape
refuses the page. `PublicCoverHttp` reads the image anonymously, bounded by `byte_length`, and
verifies status, content type, length and SHA-256 before decoding; `CoverImages` (3 concurrent,
32 MiB in-memory LRU, nothing on disk) feeds the `WorkArtwork` seam on Discovery cards and the Work
detail. `null`, loading, 404/503/mismatch/decode failure all keep the decorative tonal fallback
and never block the title, navigation or playback; preview mock artwork is never a production
fallback. Android decodes with `BitmapFactory`; **iOS has no cover boundary yet** (its URLSession
bridge carries `String` bodies), so iOS shows the fallback. Player, mini-player and Library artwork
are unchanged. Only desktop JVM tests and an Android debug compile were run; no emulator, device or
real-server image read.
The separate [metadata cover proposal](../../../services/cantos-server/contracts/studio/cover-disclosure/v1/metadata-proposal.md)
does not enable images for the fictional metadata-only catalog; no client-side workaround or
fallback release is created.

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
The default sample catalog lists released units only. Two fictional metadata Works (one mixed, one
with no released unit) are listed only when a test sets `PreviewCatalogPort.disclosesMetadata`, the
preview counterpart of the API's explicit metadata opt-in. A unit without a release has no audio
identity in the sandbox: no manifest, grant, player restore, progress, history, bookmark or Saved
presentation is derived from it, and a workbench snapshot that names one is refused as `Invalid`
([#580](https://github.com/loveoverflowcom/cantos/issues/580); JVM sandbox evidence only).

`cargo xtask theatre-host android-release` builds an isolated unsigned unshrunk release audit followed by optimized release APK/AAB and
inspects the release classpaths, DEX classes, resources and manifest, using the sandbox's compiled
JVM classes only as a forbidden inventory; a Desktop Preview build cannot accept that requirement.
The [#122 record](../../../docs/evidence/issue122-cmp-sandbox-isolation.md) records current checks
separately from the historical results cited below.

## Generated UI resources

The accepted [ADR0050](../../../docs/decisions/0050-localization-semantic-catalog.md) catalog
owns shell, auth, Membership, optional launch and host-fault **text** resources. Run the existing
`python3 scripts/localization_catalog.py --resources` primitive from the repository root;
repository verification rejects incomplete translations and stale Kotlin/Rust projections.
Generated plain keys and sealed named-argument messages live in `:shared`; there is no Web app
or service import. `UiLocale` and locale-choice autonyms come from the same registry, and the read-only `ShellLocale` representation and exhaustive projection are generated from
that registry. `PreferencesController` remains the only mutable locale authority. Canonical saved preference > explicit preview
seed > generated Vietnamese default remains unchanged; there is no device-language negotiation.
Native key/value ports, pre-acceptance refusal/retry, async durability limits and controller
lifetimes remain intact. Launch uses generated text directly, so no second Compose XML resolver
selects an implicit OS locale. Fonts/drawables still use Compose Resources.

The [#507 evidence](../../../docs/evidence/cmp-localization-507.md) names actual target results.
`LegacyShellCopy.kt`, `clockTime` and `LegacyRetryAfter.kt` preserve existing numeric behavior
until actual ICU4X Kotlin/native bindings are qualified. A numeric/plural resource submitted to
the text projection fails generation; it never silently acquires an English formatter. Test-only
Japanese text is generated into `commonTest`, excluded from release choices and production
source sets. JVM render/input evidence never establishes iOS, Android-device or screen-reader
acceptance. Shared UI-system primitives continue receiving caller-injected labels.

## Native identity (Identity v1 only)

Android and iOS compose first-party **Identity v1** through `NativeIdentityV1Client` and the
existing `NativeAccountController`. Login is explicit email/password inside the existing CMP Login
route; signup sends only the email to `/native/identity/signup`. The native JSON surface is
`/api/v1/theatre/native/identity/*` with `ci1_n_`/`ci1_r_` credentials, the
`x-cantos-theatre-session` fence and no cookie jar or browser `Origin`. The app has **no** OIDC
issuer, PKCE/state/nonce, system-browser or `ASWebAuthenticationSession` hook, custom URL scheme or
auth callback intent filter, provider fallback, Firebase/Kanidm enrollment or confirm/setup endpoint.
The only `VIEW` filter is the verified signup App Link below; it opens completion, never a session.
A provider credential found on a device is retired locally and the listener signs in again; it is
never sent to the API and is never a fallback.

### Identity v1 lifecycle integration

The account owner accepts `(scope, NativeIdentityV1Api, journal, barrier, now)` and explicit
`login(RegistrationEmail, password)`. The form password exists only in
ordinary composition RAM and the necessary command/in-flight call; it is cleared before dispatch
and on leaving. It never enters saveable state, navigation, shell projections, preview snapshots,
SQLite, preferences, diagnostics or the credential journal. There is no automatic password retry.
Validation, pending/double-submit, generic credential refusal, `429` with bounded Retry-After,
retry-later and cleanup-required states use the existing VI/EN copy and account projection: a
`401`/`403` is "cannot sign in with these credentials", `429` and `503` are retry-later (a `503`
never claims the server revoked or logged out anything), and a `409` fence is never shown as
"session expired". Every navigation away from pending Login fences its result through the same
account owner.

The complete pair is written/read back before `GET /native/identity/session` admission. Exact
actor/family/deadline/authentication-time match precedes `verifiedSession`/`SignedIn`. Restore
runs once at process start and checks the API or performs one safe refresh within the original
deadlines; timer and private requests share one flight. A nonconsuming session-check outage hides
authority and retains a candidate for explicit restore. Refresh failure/unknown outcome is
cleanup-only, never replayed with the old secret.
Identity v1 time checks use a monotonic elapsed-time floor over the supplied UNIX clock, so
moving the device clock backwards cannot postpone renewal or extend local access/absolute
authority. The floor survives rotation and revalidation; restart still requires API admission.
Native suspend/time-source behavior remains a device acceptance item.
Logout hides authority, marks the durable denial barrier, then cleans the exact known family.
Late A results cannot change B's journal, Library, player or navigation. No auth token enters a
media URL; public playback keeps its separate release capability. SQLDelight and playback policy
are unchanged, and signing out, retiring a credential or switching account never deletes an
account's progress, bookmarks or outbox (they are partitioned by actor).

`SecureNativeCredentialJournal(store, origin)` writes version 2 bound to the exact `api_origin`
and `protocol: identity-v1`; it can no longer write or parse a provider credential. An exact
canonical retired record at this origin (a version-1 issuer-bound provider journal of any kind, or
the unbound 64-hex pre-provider token) reads as `NativeCredentialRecord.Retired`: it is removed
locally and the listener must sign in again. That is not server revocation. Anything else
unreadable, foreign, mixed, ambiguous or from another origin is `Corrupt` and fails closed, and OS
secure-storage failures (`Unavailable`/`Denied`) stay distinct from both. A pending denial barrier
cannot be cleared by inventing an empty record. Interrupted refresh and Active-plus-pending-barrier
recover through cleanup, never credential replay.

Android keeps Keystore AES-GCM + full-record AtomicFile/readback/fsync in `noBackupFilesDir`;
iOS keeps Keychain `AfterFirstUnlockThisDeviceOnly` + full-record readback. Both keep their
backup-excluded logout barrier, app IDs, signing and storage namespaces. One process owner and one
native player survive navigation/Activity recreation.

### Durable Library recovery at the API boundary

`NativeAuthorizedHttpPort` borrows the process account owner's credential lease and admits only
closed Theatre Library/progress/receipt routes at the exact host-approved scheme, host and port.
The Android debug loopback exception passes through this same production adapter. Foreign origins,
Studio/Admin/auth routes, malformed paths and unknown bearer/family inputs are refused before dispatch.
Release and iOS origin selection remain HTTPS-only; the native transports refuse redirects.

`LibraryResponse` classifies local transport/authority failures before inspecting HTTP payloads.
A real pre-admission `503` with no session stamp is retryable only with the strict Library
`{"code":"unavailable","retryable":true}` JSON envelope. It grants no account payload authority
and does not clear the account, player or local Library. A real `401` or explicit different stamp
still fences authority. Successful pages and receipts require the real matching response stamp,
JSON content type, byte limit, schema and record binding; a missing stamp is never filled from the
request expectation. Other statuses and error bodies retain their contracted, closed mappings.

`NativeSessionRuntime.foregrounded()` and `networkRestored()` ask the existing account flusher to
drain. After six automatic backoffs are exhausted, a new opportunity starts another bounded episode;
callbacks during an active drain share it. These signals neither revalidate/login nor reset/start
the player. Android registers one process `ConnectivityManager` callback (requires
`android.permission.ACCESS_NETWORK_STATE`) and Activity foreground hook. The callback follows the
current default network from API 24, where a new default replaces the old one without a guaranteed
loss for it; API 23 follows all networks. `NetworkRecoveryTracker` decides, once per outage, when a
usable network returns; iOS uses the retained `NWPathMonitor` and active scene hook, delivering to the main owner.
Network availability is a hint, never proof of admission or synchronization. Frozen sent intents
keep their operation ID and body and query the original receipt before replay; account/generation
and credential-lease fences still reject old callbacks after logout or switching accounts.

Focused tests exercise the production owners over disposable SQLite with controlled Identity/HTTP
and decoder doubles. The explicit `cmp_authorized_library_against_real_identity_and_http` test in
`services/cantos-server/tests/theatre_business_journey.rs` adds real Identity login/admission, TCP HTTP,
file-backed Server storage, and an actual Identity stop/restart. Its Kotlin side uses
`NativeAccountController → NativeAuthorizedHttpPort → JDK HTTP → NativeLibraryClient → SQLite/sync/runtime`.
The older `cmp_production_client_against_the_real_api` trusted-host fixture is synthetic, not auth
acceptance; it stamps at the fixture server and the JDK transport reads that actual header.
See the [slice evidence and remaining native gates](../../../docs/architecture/server-turso/cmp-business-api.md#cmp-authorized-boundary-and-recovery-follow-up).

### Native identity build inputs and operator diagnostics

Only `CantosApiOrigin` is required. Origin selection preserves the existing precedence:
Android `cantos.apiOrigin` Gradle property, then `CANTOS_API_ORIGIN`, then the packaged absence
marker selecting `https://api.cantos.cc`. iOS uses the `CANTOS_API_ORIGIN` Xcode build setting
expanded into Info.plist, followed by the same shared default selection. Explicit invalid inputs
never fall back: the host shows a typed `CantosListeningUnavailable` diagnostic instead of the app.

One owner decides which origin the app talks to: `selectCatalogOrigin`/`selectPackagedApiOrigin`
produce the `ApiOrigin` that the catalog, Library, playback access **and** Identity v1 client and
journal all use, so they can never disagree. Release builds and iOS admit HTTPS only. The Android
**debug** host additionally admits `http://127.0.0.1:<port>` or `http://localhost:<port>` (for
`adb reverse`) for catalog and auth JSON alike; its debug-only network policy and
`cantos_loopback_origin_allowed` resource are the single switch, and the release manifest forbids
cleartext. A cleartext origin to any other host, or on iOS, is refused with a typed fault. An
HTTP loopback origin therefore carries real credentials over the local USB/loopback path only; use
HTTPS for anything beyond a developer's own machine.

`CantosNativeIssuer`/`CANTOS_NATIVE_ISSUER`, `--native-issuer` and the callback scheme no longer
exist. Guest discovery/listening remain independent of authentication. The backend must
independently configure the real Identity service and Server consumer; no client secret belongs
in an app. Existing-account login can be accepted independently of signup completion.

### Signup Web dependency and acceptance boundary

Native signup collects **only email**. A generic `202` shows check-mail guidance; it establishes
neither delivery, account existence, mailbox proof nor a session, and a `429` is shown as
rate-limited while a `503`/timeout is an unknown effect that asks the listener to check the inbox
before starting a new request. There is no verified polling, resume-triggered login or password
collection before proof; opening the mail never creates a session.
[ADR 0048](../../../docs/decisions/0048-signup-email-link-return-to-app.md) selects native-first
completion through verified links ([#422](https://github.com/loveoverflowcom/cantos/issues/422)).
Stage 0 is the check-mail guidance that names the browser step and the return to this app.

Stage 1 on Android is implemented in source, not accepted on a device. `TheatreActivity` declares
one `autoVerify` App Link filter for exactly `https://cantos.cc/verify-email`. It reads a `VIEW`
URL once, removes it and the extras from the kept Intent, and ignores relaunches from recents and
the original Intent of a recreated or restored Activity. The shell's `EmailVerificationController`
checks the URL against the pinned `ProductionVerifyEmailHandler`, never against the URL that
arrived, with the Web parser's rules. It keeps the action only in process RAM; process death
loses it. The completion screen asks for a password twice and an explicit confirmation, then posts
the anonymous `POST /native/identity/signup/complete`. It retries only the identical request after
a 503, 429 or lost answer, and treats `replayed` as success. Both native hosts measure the 429
hold with monotonic elapsed time; their UNIX clocks still validate token deadlines. A signed-out listener continues to
Login. A signed-in listener keeps the current session and is pointed to Account for the explicit
sign-out. Nothing signs in, out or switches automatically.

Verified opening needs a reviewed `assetlinks.json` on `cantos.cc` with the actual approved
certificate fingerprint for each Android package (`com.cantos.theatre` and/or
`com.cantos.theatre.dev`). No association file or approved fingerprints are committed yet.
A local debug keystore is not implicitly approved. The operator must also configure the Firebase
template and `CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER` for the pinned URL. When both apps are
installed Android's verification, user defaults and resolver determine which opens; the URL does
not identify the app that initiated signup. With no app or no OS handoff, Web remains the fallback.
An explicit intent tests intake only and does not prove association. Record each installed APK's
package, version, hash and public signer, then inspect `adb shell pm get-app-links` separately for
each package and test actual email taps cold and warm. Do not log action URLs or proof values.
The configured build-admitted API origin, never the incoming URL, selects the backend. A dev build
using a local API cannot complete an operation issued by production Identity. The link does not
identify its issuing backend, so the app cannot detect that mismatch before posting the password
to its configured origin. Use a matching issuer/backend; verified association alone does not
establish this binding. A pre-submit mismatch guard needs a separately reviewed contract.
iOS source now feeds the same controller from both `NSUserActivityTypeBrowsingWeb.webpageURL`
and SwiftUI's direct Universal Link delivery (`onOpenURL`). The App's one `CatalogOwner` is
shared by its scenes; it forwards synchronously without a URL queue, scene storage or navigation
arguments. The activity's URL/user-info are cleared and Handoff/search eligibility disabled before
delivery. `IosNativeIdentityOwner` reuses its existing admitted-origin `NativeIdentityV1Client`
for anonymous completion; an invalid origin composes no verification owner. Only Xcode Release
has `Cantos/Release.entitlements` with `applinks:cantos.cc`; Debug stays unassociated. The shared
parser still admits only the exact HTTPS host/path, so direct URL delivery adds no custom scheme.
Process death loses the link; another tap is required. Signed-in completion keeps the existing
session, Library, outbox and player. AASA and signed-device association remain operator work:
no verified Apple Team ID is available and no placeholder association file is shipped.

The [#435 acceptance record](../../../docs/evidence/native-signup-435-20261024.md) separates the
local Server/Identity/Auth Emulator journey, JVM checks, Android runtime and iOS source/toolchain
boundaries. The supported local composition uses Firebase verification; Identity also retains
`lettre` SMTP (the [test-host record](../../../docs/evidence/native-test-vps-resend-mail-20261023.md)
selected Resend SMTP). There is no `resend-rs` dependency. SMTP fragment proof links retain their
Web completion route; the native-first parser accepts Firebase action links only. Password reset
is deferred in the [Identity auth contract](../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md).

The [Web completion contract](../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md#email-link-and-signup)
is owned by the API and Web work. The Theatre Web completion page is merged
([#378](https://github.com/loveoverflowcom/cantos/pull/378)), but end-to-end new-account onboarding
stays **unaccepted** until that page is browser-verified against a real Server and Identity.
This CMP change does not implement a Web frontend or the Server's Turso migration.

The offline Desktop Preview mounts the same shell/account/signup owners and explicit commands,
with only external ports mocked in `:preview-sandbox`. Release never depends on that module.
See the [source-aligned evidence](../../../docs/evidence/cmp-native-identity-v1.md) for the earlier
migration record and the [identity-only cleanup section](../../../docs/evidence/cmp-native-identity-v1.md#identity-only-cleanup)
for what was removed and what ran. Desktop renders/builds do not
accept Keystore/Keychain durability, process death, native IME, TalkBack/VoiceOver or real auth.
Historical provider observations remain in the [earlier identity record](../../../docs/evidence/theatre-identity.md).

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
  before popping, so a late response is fenced; `login(email, password)` is accepted only on the open Login
  route in a resting guest state (a double tap or pending flow cannot restart it) and
  `openLogin()` is refused for a signed-in listener. A refusal leaves Login and its
  generic failure visible; another request requires a fresh explicit submit. Restore remains an
  explicit action that says when the device holds nothing. Logout hides the projection first; a failed
  cleanup keeps Login disabled with a retry. A Membership draft is cleared only when a signed-in
  account ends, so Membership → Login → success resumes the guest's draft.
- **Presentation** (`features/auth`): Login uses the vertical **Đăng nhập** primary and
  **Tạo tài khoản Cantos** tonal actions from the #127 auth variants, with no tabs, legal links or
  consent. Email/password fields dispatch through the account owner. When its
  signup owner is configured, Register collects only email and gives Web completion guidance; otherwise
  it explains the missing capability. Enrollment never authenticates the account. Guest and signed-in Account use the Profile,
  Membership and AccountGroup surfaces; signed-in shows no invented name, total, conflict or
  entitlement. Auth hides the tab bar and mini-player; the header gives priority to Back and the theme
  toggle and shows the logo only in leftover space. Back (visible, Android system and JVM Escape) is one
  intent; scroll positions are kept per stack entry and Back from auth restores focus to Sign in or
  the avatar.
- Still unrun: Android Keystore/lifecycle flows, iOS compilation and Keychain
  (the pinned Kotlin/Native compiler is absent), TalkBack/VoiceOver, IME, font scale on a device and the
  real Identity service. The #35 two primary destinations remain; Account is not a third tab.

## Screens and behavior

- Two primary destinations: **Khám phá / Của tôi**. Search belongs to Discovery;
  the avatar opens a secondary **Tài khoản** screen: a guest profile with vertical Sign in
  and Create account actions, a Membership card and the listening settings. A signed-in listener sees
  a compact profile, one grouped list (membership, library and history, appearance, listening
  settings, support information) and a separate Sign out entry that opens a modal bottom sheet:
  only its confirm action asks the account owner to sign out, and Back closes the sheet before the
  route. With no typed name/email projection the profile reads **Người nghe Cantos · Đã đăng nhập**;
  support is information until a verified destination exists.
- Của tôi displays **Nghe tiếp, Đã lưu and Lịch sử** directly, without library tabs, for the verified
  account that owns the Library; a guest is asked to sign in. History has a secondary full list and
  a confirmed clear action; clearing it preserves listening positions and saved works. The three
  sections read, page, fail and retry independently: a failed next page keeps the rows already
  shown, a section never read is "loading" or "failed" rather than empty, and a row that cannot
  resume (release replaced, access unavailable, needs refresh) says why. Metadata is stored per exact
  publication in the existing SQLite store (no second store, no per-Work fetch).
- Vietnamese search, work details and desired-state **Lưu / Bỏ lưu** actions on cards/details.
  Resume opens the same release at its saved position, paused. History records starts the player
  acknowledged; opening a page creates none.
- Mini/full player: bounded manual seek, speed, a sleep timer that pauses the real player and the
  episode queue.
- **Release-bound bookmarks** ([Library v1.2](../../../services/cantos-server/contracts/theatre/library/v1/README.md#release-bound-bookmarks-v120), #299).
  The player's *Bookmark this moment* control takes the exact audio identity and the position from **one
  snapshot of the player owner**, records both in the existing SQLite store with its outbox intent in one local
  transaction, and shows the row at once as pending (offline when the last attempt could not be delivered, failed
  when the server refused it for good, silent once synced). Pressing again at the same release position is the
  same bookmark. The list lives in the player and in the Library, reads pages from the API with each item's own
  exact-release metadata, and survives restart and sign-in on another device. Deleting hides the row at once and is
  synced; the account remembers its deletions, so neither a stale page nor a replayed response brings one back.
  Opening a bookmark restores its exact release at its position through the one player owner, **paused**, or moves
  the loaded release; a replaced or unavailable release is explained and never substituted. A guest is taken to sign
  in and nothing is saved. Evidence: JVM tests, the production authorized HTTP port's route allow-list, and a loopback run of the sync adapter
  and client over a JDK transport against a file-backed Server API. The separate authorized
  journey above adds real Identity; neither is device, audio or deployed acceptance.
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
[mock playback corpus](../conformance/playback/v1/vectors/README.md)
is imported unchanged for tests; it is not a listener API DTO. Shell compositions use stable
native Material controls and official outlined icons; no reusable Cantos primitive is introduced.
The current [preview catalog](../../../packages/ui-system/shared/preview-catalog.json) owns
qualification scope/freshness. Any further shared API or variant adoption still requires fresh
[two-renderer admission](../../../.agents/skills/cantos-ui-design/references/preview-catalog.md).

The #453 Discovery/Search/Library/History header adopts the qualified official `Logo.Symbol`;
Search adopts the qualified `TextField.FloatingSearch` with its editor-owned tag and existing
query-on-change/Clear contract. Library's invitation uses the qualified shared `Button`.
The app-owned search containment reads the dedicated Theatre hero/foreground and heading-weight
tokens. It does not enable the global Theatre expression or change account/Auth/Membership
presentation. The one app-owned top bar (#532) composes the official Logo Symbol, a 48 dp tonal
theme square and a 48 dp circular text-avatar account action from canonical Theatre roles and
stable Material controls in the Shell feature; the avatar keeps the generic `C` fallback because
no display name is held. Auth and Membership keep their focused-task headers (the layer guard
forbids a cross-feature import), so their theme action awaits a shared owner. This is not the
shared Avatar or tonal IconButton. Avatar, tonal shared IconButton, filters and expression adoption remain separate
until their exact renderer scopes are admitted. Component qualification does not establish
Android/iOS or production-flow acceptance.

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

### Live enrollment integration (#296)

The separately selected live Desktop Preview mounts Identity v1 and the durable Library through the same
`CantosTheatreApp`, `TheatreShell`, `RegistrationController`, `NativeAccountController`, checked
origin and client/journal protocol as the native hosts. `LiveEnrollmentOwners` builds the composition
the Android and iOS hosts build, only with different platform ports:
`NativeAccountController` → `NativeAuthorizedHttpPort` → `NativeLibraryClient` →
`nativeLibraryController` → `NativeSessionRuntime` → `TheatreShell`, with the account-scoped
`AccountCatalogCache` over the same store. It uses real HTTP (`PreviewNativeHttp`: exact origin, bounded body, no
redirect, no ambient cookie, cancellable), transient RAM credentials/barrier, an **in-memory SQLDelight
store** (neither disk persistence nor process-death evidence), no browser callback, issuer, mocked
fallback or native audio decoder: playback requests fail as unavailable instead of simulating audio.
Missing capabilities remain explained.

```sh
CANTOS_THEATRE_PREVIEW_API_ORIGIN=https://api.local.example \
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview run
```

A supplied invalid origin is refused, never replaced by production or offline recipes. The preview
uses the same origin owner as the Android debug host, so an explicit `http://127.0.0.1:<port>` or
`http://localhost:<port>` origin also mounts live login/signup (RAM store only); any other
cleartext origin is refused. Use an already owned isolated service fixture; the command provisions
no API, database, certificates or mail.
Do not submit signup to a real-mail environment without separate recipient/operator authority.
Web proof/password completion remains an unaccepted dependency described above.

Escape is the shell's own Back in the offline window. It is installed on the window
(`Window(onKeyEvent = PreviewBackKeys::onKeyEvent)`), not on a composable: a window with no focused control
holds focus at the scene root, which never visits a descendant's key handler, so a handler inside the content
stopped working after pointer-only navigation. The window's `onKeyEvent` only sees keys the content did not
consume, so a focused control or dialog that handles Escape keeps it. The live window has no Escape binding.

The live window has no desktop connectivity or foreground signal (`NativeSessionRuntime.networkRestored()` and
`foregrounded()` are not called), so an exhausted retry episode there resumes only on the next Library action;
Android and iOS register those signals.

Evidence, kept separate by layer (record: [CMP Desktop Preview live acceptance](../../../docs/evidence/cmp-desktop-live-acceptance.md)):

- `LiveLibraryPreviewTest`, `LiveDesktopUiTest`, `LivePreviewOriginTest` (`:preview-sandbox:desktopTest`): the live composition over a
  **loopback protocol fixture** (stamped Identity v1 and Library routes, injected 401/503/lost response/oversized
  body, a held response across logout). Mocked server evidence for the preview's wiring and transport, run by
  `theatre-preview verify`.
- `DesktopAcceptanceJourneyTest`: product controls driven by semantics only, no screenshot or window.
- `DesktopWindowEscapeTest` (explicit: `CANTOS_PREVIEW_WINDOW_TESTS=1`, skipped headless): a short-lived real AWT
  `ComposeWindow` with static content, proving that Escape reaches `Window.onKeyEvent` with nothing focused and
  that a focused control that handles it keeps precedence. An AWT event, not an OS keystroke.
- `cmp_live_desktop_preview_against_real_identity_and_http` (explicit, `--ignored`): `LiveEnrollmentOwners` in
  `LiveDesktopPreviewRealServerTest` over the real Server, the real public catalog and the real Identity
  process; the harness then reads the Server rows. Not part of any gate; needs Gradle/JDK and the built `cantos-identity`.
  No window is driven and audio stays unavailable.

Static captures use offline ports and never initiate network or mail. V1 controller/form/host
fixtures test email-only signup, explicit login, generic failures, duplicate submit, Back and
late responses, and `RegistrationPreviewUiTest` drives the real v1 client against a loopback
protocol fixture (202, 429, 503 and a refused connection). The retired provider enrollment
adapter, controller and fixtures no longer exist; the historical
[#296 ledger](../../../docs/evidence/issue296-cmp-enrollment.md) remains source-bound to that earlier
implementation, and the [v1 record](../../../docs/evidence/cmp-native-identity-v1.md)
covers the migration.

### Offline recipes

There is no Android or iOS preview host (#286, #287) and no Android Studio `@Preview` wrapper of the
sandbox: `cargo xtask compose-preview cantos-theatre` has nothing to render until a future,
production-safe stateless preview exists. The preview is the JVM desktop window and its tests.
`PreviewOwners` (`app/`) builds the owners the native hosts build and replaces only their external
ports:

| Owner (production class, unchanged) | External port replaced in the preview |
| --- | --- |
| `PublicCatalogController` | `PreviewCatalogPort` (`PublicCatalogPort`): the sample works and units under canonical UUIDs, source search, paging by cursor, and the `Ready`/`Loading`/`Empty`/`Error` scenarios |
| `NativeListeningController` + `AuthorizingMediaEngine` | `PreviewPlaybackAccess` (`PlaybackAccessPort`, with injectable refusal) and `PreviewStreamingEngine` (`StreamingMediaEngine`): a decoder stand-in that acknowledges commands and reports the resumed position; no audio, no clock |
| `NativeAccountController` | `PreviewIdentity`: a deterministic `NativeIdentityV1Api`, credential journal and logout barrier. `PreviewLoginMode` approves, holds or refuses the explicit login response; two sample accounts make account switching real |
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

Library-specific recipes use `theatre-library-empty-v1`, `theatre-library-partial-v1`,
`theatre-library-loading-v1`, `theatre-library-error-v1`, `theatre-library-stale-v1` and
`theatre-history-empty-v1`. These start a signed-in synthetic listener. Loading holds the external
read port; error refuses it; stale first admits real pages to the in-memory production Library
store and then fails an actual refresh. None forces a UI phase or replaces the shell/controller.
All recipes accept the existing VI/EN, light/dark, narrow frame, text-scale and motion preferences.
The Library capture suite exercises both languages/themes and 320dp at 200% text, including
scrolling to Discover and the last section. These are JVM presentation observations only.

A completed empty Library uses one tonal invitation plus compact contextual summaries. A partial
Library keeps local explanations. Only completed sections without retained rows are empty;
loading, retry, paging, stale rows, missing metadata and disabled bookmarks keep their distinct
meaning. Full History retains its own recovery actions. See the
[bounded implementation/evidence record](../../../docs/evidence/theatre-cmp-library-empty-417.md).

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
`UnknownRecipe` or `Invalid`. The JSON schema is **version 2** (the old memory-only bookmark list was removed;
real bookmarks live in the account Library, and `signedIn` was added) and bounds imported payloads at 65,536
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
([contract](../../../services/cantos-server/contracts/theatre/discovery/v1/README.md), [ADR 0035](../../../docs/decisions/0035-theatre-catalog-unit-availability.md)):
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
`CantosApiOrigin` metadata for both variants. Runtime resolution is one owner
(`selectCatalogOrigin`, #298): an explicit override, else `DefaultProductionApiOrigin`
(`https://api.cantos.cc`) in both build types; a build type never selects another backend. Packaging
preserves absence with `__CANTOS_DEFAULT_API_ORIGIN__`; an empty `CantosApiOrigin`/`CANTOS_API_ORIGIN` value is refused;
a value that is present but empty, malformed or `http://` outside the debug loopback exception is a
typed `HostConfigurationFault` shown before any request, and it never silently falls back to the
default. Runtime admits `https://` in release; only a debuggable build also admits
`http://127.0.0.1|localhost` through its loopback-only network security config. There is no fixture
fallback, playback source or sandbox dependency. API failure/empty data stays a real failure/empty
state. Release keeps `usesCleartextTraffic=false`. Local integration picks its API explicitly
(`android-debug --api-origin`, `CANTOS_THEATRE_LIVE_ORIGIN`) and never inherits production by omission.

The separate shipping iOS project/scheme is `iosApp/Cantos.xcodeproj` / `Cantos`, bundle identifier
`com.cantos.theatre`. It imports the static `CantosTheatre` framework built directly by `:shared`,
links only `shared/build/bin/...` and copies only shared Compose resources. It is the only iOS
project: the former `CantosDemo` mock host was removed (#287). Shipping `CANTOS_API_ORIGIN` is an explicit Xcode build
setting (the absence marker selects the production default), consumed through `Cantos/Info.plist`
and resolved by the same Kotlin owner (`packagedApiOriginText`); HTTPS is required on every iOS
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
[error fixtures](../conformance/client-boundary/v1/README.md), exercises
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
certificate and recipients. Native registration and downloads are still unavailable; bookmarks have API, JVM and loopback evidence only (#299), no device run.

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
| Account, Login | `NativeAccountController` (Identity v1 Bearer session, Keystore/Keychain) |
| Theme, language | `PreferencesController` (language persisted) |
| Sleep timer | local to `TheatreShell`: pauses the real player after 15/30 minutes |

Absent facts are absent, not invented: the bookmark control is offered when the durable Library owner exists (a guest is guided to sign in, never shown a fake save), Library pages
are read with the opt-in `presentation=1` (Library v1.1, #297) so Continue, Saved and History rows
are named by the API's released Work/unit titles even for works the catalog never listed; a row
whose metadata is `null` (replaced, withdrawn or private release) keeps its record and exact audio and is named
neutrally, never by another release's title, artwork has no projection yet (the tonal placeholder remains) and an episode number is
never derived from `position`. Gaps are tracked in the BE issues linked from the PR and #180.

The [Android release/distribution runbook](docs/firebase-distribution.md) owns R8, same-release
audit pairing, exact staged APK/mapping hashes and the explicit development/production policies.
Device and live Firebase acceptance remain independent of source/build/audit results.

The [client error contract](../conformance/client-error-presentation-v1.md) also
owns the paired Web/CMP taxonomy and safe actions. Catalog 401/403 and HTTP/JVM deadlines survive
as typed diagnostics into the shared shell ErrorCenter. Unknown Library outcomes keep their
existing receipt owner and never gain a generic modal Retry. Native identity configuration
refusals now reach the Account view as typed reasons while public catalog remains available.
