# Theatre CMP guest-first account and auth — issue 130 (first slice)

## Scope and invariant

[Issue 130](https://github.com/loveoverflowcom/cantos/issues/130) applies Auth–Account design v2
to the Cantos Theatre Kotlin Compose Multiplatform app. This is its first, bounded slice: the
existing native account owner is composed **into the guest shell** instead of replacing it, with
typed Back/return context, the v2 Login/Register/Account hierarchy and fenced late completion.
It follows the Web slice delivered by [PR 161](https://github.com/loveoverflowcom/cantos/pull/161)
for [issue 129](https://github.com/loveoverflowcom/cantos/issues/129), whose
[evidence report](theatre-web-auth-account.md) holds the Web matrix. Issue 129's remaining
items there are human/provider acceptance, not code in this change.

Invariant: Home stays usable without an account. Login is an explicit listener intent that is
never a wall. Back, provider cancellation and any late callback or completion cannot reopen a
closed auth intent, relabel another account, start a second flow or change playback. A route,
fixture, callback hash or rights claim is not authentication or registration success; only the
existing `NativeAccountController` establishes a session.

Out of scope and unchanged: the player, Library/listening domain, SSO crypto, the native
protocol/storage/HTTP adapters, the Android manifest and callback filter/delivery, Gradle and
dependencies, shared `packages/ui-system`, tokens, Studio, #128 Membership form/receipt/plans,
#122 release isolation and provider/grant/TLS configuration.

## Source and design provenance

- Task base: `develop@f265ae4184be37a4d36fc4dedee195a98141644d` (clean checkout; PR 161 and 162
  already merged). Work was done in an isolated worktree on a branch from that commit; uncommitted
  until the PR. Baseline: the full desktop/JVM suite passed 279 tests before any change.
- Design inputs: the issue text and the Web reference PNGs that PR 161 downloaded
  (`login.png`, `account-guest.png`, `account-signed.png`, opened locally) as hierarchy and copy
  references. The Drive v2 package, README-Handoff and the **mobile HTML were not reachable from
  this environment, so no mobile design file was opened or compared**. Web PNGs are prototype
  Chromium pixels at desktop size; they establish hierarchy, not mobile pixels. Prototype timers,
  Minh An, 03/05, 12:48/18:25 and release 1.2 are fixtures and are not imported.
- Painting artwork from the reference is excluded: its usage rights remain unverified, as in
  the Web slice. The brand block is typography on the tonal auth surface; this is a recorded
  visual difference, not a design-pixel parity claim.
- Owners read: `AGENTS.md`, `cantos-ui-fix`, `cantos-engineering`, `cantos-cmp-mobile`,
  `cantos-ui-design` references, the identity handoff and the layer guard in
  `scripts/check_repository.py`.

## Implemented ownership and guarantees

| Boundary | Behavior | Failure prevented |
| --- | --- | --- |
| `app/Navigation.kt` (pure) | Adds `Login`/`Register`. `NavigationState` accepts auth only as the closed suffix `Login` or `Login, Register`, never the root. `OpenLogin`/`OpenRegister` are idempotent and `OpenRegister` without Login supplies it; `AuthCompleted` pops only auth that is still open; navigating to another destination dismisses auth first. | Open redirect (no caller URL), an auth root restored on cold start, duplicate history entries, a late completion navigating after Back, a destination stacked above Login. |
| `application/identity/AccountView` | Pure projection of the unchanged `NativeAccountState`: `Unavailable`, `SignedOut`, `Pending`, `SignedIn`, `CleanupRequired`, `Failed(reason)`. Carries no actor, session, expiry, name or page count. | Actor UUID shown as a name; a page count shown as a total. |
| `app/AppContainer` | Optional `account` observed and **never closed** (the process owns it). `back()` cancels a pending login before popping; `openLogin()` is refused when signed in and dismisses a stale failure; `beginLogin()` runs only on the open Login route in a resting guest state; `restoreSession()` is explicit and reports an empty device; `logout()` doubles as cleanup retry; a confirmed session dispatches `AuthCompleted`. | Double-tap restarting a flow, a late callback signing in after Back, Login enabled while a previous credential is not acknowledged removed, a second signed-in login. |
| `features/auth/presentation` | Stateless Login, Register-unavailable and guest/signed-in Account over the #127 Button/Surface/Badge/BackButton/Logo variants; emits `AuthIntent` only. Auth hides the tab bar and mini-player; the header prioritizes Back and the theme toggle and shows the logo only in leftover space. | Controls covered or squeezed away by large text; business decisions in composables. |
| `features/shell/presentation` | Account/auth content arrives as slots. Per-stack-entry scroll and Back-from-auth entry focus (`RouteRestoration.kt`). | Back losing the listener's place; focus left on an unrelated control. |
| Android host | `MainActivity` always mounts the guest shell with the Application's container (now built with `nativeIdentity`); `NativeIdentityApp` is removed; iOS `IosNativeOwner` mounts `CantosDemoApp(account = …)`. | Identity replacing Home; OS Back meaning something different in identity mode. |

Registration stays an explained **unavailable** step: no verified Cantos ID self-registration
entry or callback policy exists, so Cantos shows no form, collects no email or password and never
shows success. "Create account" is offered only when the listener can act on it (not pending, not
signed in, not when identity is absent, where the disabled Sign in states the reason once). A
Membership draft is cleared only when a signed-in account ends; a guest draft survives sign-in so
Membership → Login → success resumes the same intent. The two primary destinations remain
**Khám phá / Của tôi** (#35); Account is an avatar entry, not a third tab.

## Reproduction, regression oracles and recovery

This is a feature slice rather than a single reproduced defect: the Account screen was a disabled
placeholder and identity replaced the guest shell. The "before" state is the clean-base capture
below. The regression tests were written before the implementation; they referenced new symbols,
so the first red state was a **compile failure against the base, not an executed assertion
failure**. After implementation, one hand-mutation run (together: no cancel in `back()`,
`AuthCompleted` made inert, no resting-state guard in `beginLogin()`, no stale-failure
dismissal) failed 12 tests naming each mutated behavior; the original sources were restored and
the gate rerun.

Failures met while working, all fixed in this change: two existing Account layout/interaction tests
asserted the old preview card and disabled placeholder, and were updated to the new hierarchy (one
now uses a taller window because `boundsInRoot` is clipped to the scroll viewport); the first
captures showed an invisible avatar (lavender on the lavender profile surface), a logo/toggle
squeezed to nothing at 2× text, and repeated disabled reasons, each repaired and recaptured; and
the repository gate rejected unqualified `FieldHelp`/`FieldError`, so app-level text over canonical
roles replaced them.

## Layered evidence and command ledger

All commands used Rust 1.98.1, `TMPDIR=/private/tmp`, `LC_ALL=C`, Gradle 8.14.3 from the wrapper
distribution and a Python 3.14 shim for the repository gate.

| Layer | Claim / oracle | Result and evidence level |
| --- | --- | --- |
| L1 Architecture | Account/auth is a separate feature behind slots; the container observes the owner; hosts changed minimally; no shared-package edit | `documented`; independent layer guard passes in the repository gate |
| L2 Domain/types/errors | Closed navigation suffix; `AccountView` carries no identity facts; exhaustive `when` over routes, states and intents | `type-enforced`, `example-tested`: `NavigationTest`, `AccountViewTest` |
| L3 Unit | Back/return table, idempotence, inert late completion, hostile/duplicate stacks | `example-tested`: 6 new navigation and 4 new AccountView tests PASS |
| L4 Application/mocked | Real container, navigation and `NativeAccountController` over deterministic ports: explicit open, double tap, provider cancel, Back while pending then late callback, stale failure, restore, cleanup block, registration, no capability | `example-tested`, `interaction-tested`: `AuthFlowTest` (12) and `AuthAccountUiTest` (9) PASS. The "server" is a double: it proves lifecycle and fencing, never live identity |
| L5 JVM renderer | Same-scenario captures of Login/Register/Account | `screenshot-inspected` on the desktop Compose renderer only |
| L5 Android build | The edited host and shared sources compile into the debug APK | `compiled`; not installed or run |
| L5 device/provider | System browser, callback, Keystore, iOS, TalkBack/VoiceOver, IME, OS Back | **NOT_RUN** |

| Exact command | Result | Scope |
| --- | --- | --- |
| `cargo xtask theatre-demo verify` | PASS | Whole desktop/JVM suite: 315 tests, 0 failed (base 279) |
| `cargo xtask theatre-demo android-build` (`ANDROID_HOME=~/Library/Android/sdk`) | PASS | Debug APK assembled; manifest, callback filter and callback delivery untouched |
| `cargo xtask theatre-demo ios-build …` | **UNSUPPORTED** | Kotlin/Native 2.2.20 is not installed. `IosNativeOwner` has a one-line change (`CantosDemoApp(account = account)`) that has not been compiled |
| `cargo xtask verify repository` | PASS | After the final documents and component fix: layer guard, preview-admission freshness, links, generated drift and 3.14-shim Python tests (the default Python 3.9 cannot run this gate on `develop` either) |
| `cargo xtask preflight --changed-only` | PASS: `repository`, `rust-format`, `theatre-cmp-demo` | On the final diff. Its desktop test task was `UP-TO-DATE` from the preceding run on identical inputs, so it reuses rather than repeats that result |

No behavior model is registered for navigation/auth, so no Quint run applies; no model result is
presented as conformance. Script IR, contracts, Rust, Web, PostgreSQL and `packages/ui-system`
are untouched, so those gates do not apply to this diff.

## Opened captures

Captures are generated by `AuthCaptureTest` and the base-tree equivalent, written to the ignored
`artifacts/ui-evidence/issue130/{before,after}/` (PNG, semantics dump and JSON per scene) and
**not committed**. The reviewer (this assistant) opened these images:

- **Before** (clean `develop`, `[Discovery, Account]`, no identity): guest 390×844 light/dark,
  768×1024 and 320×800 at 2× text. A flat guest row, a preview card, a disabled Sign in and a text
  link to Membership.
- **After**: Login light/dark 390×844, English, 768×1024, dark 320×568, light 320×800 at 2× (and
  scrolled to the actions); Login pending, failed and cleanup-required; Register unavailable;
  Account guest without identity (the same four scenarios as the before set), guest with identity,
  and signed-in light/dark 390×844 and 320×800 at 2× (also scrolled to Sign out).
- Observations: vertical primary then tonal action on Login with no tabs, legal links or fields;
  Back names its destination; at 2× on 320 the label wraps, the toggle keeps its size and the logo
  is dropped; the profile avatar is visible on the lavender surface in both themes; disabled
  actions show their reason; pending hides Create account and offers Cancel; errors use the error
  role; Vietnamese marks and English copy are intact; Account shows no name, total or conflict.

These are rendered pixels from the JVM Compose renderer with the desktop font stack. They are not
Android/iOS layout, safe-area, IME, keyboard, system-Back or reader evidence.

## Deferred acceptance and residual risk

| Check | Reason | What would accept it |
| --- | --- | --- |
| Android device/emulator walkthrough, TalkBack | Not run; no device or IDE capture was available | Install the debug APK with operator-supplied `CantosApiOrigin`/`CantosNativeIssuer`; Home → Account → Login → system browser → callback and cancel; OS Back at each step; record the build, font scale and reader |
| System-browser callback, Keystore, live provider, real signup capability | Operator-owned; no live issuer, grants or TLS were touched | The existing identity handoff procedure; signup only through a verified provider entry |
| iOS compile, ASWebAuthenticationSession, VoiceOver, edge-swipe Back | Kotlin/Native absent; no `BackHandler` is wired on iOS | Pinned toolchain and `cargo xtask theatre-demo ios-build`; then simulator/device observation |
| Design fidelity against the mobile HTML | Not accessible here | Owner review of the opened captures against the Drive set |
| `FakeStorage`/`DemoState` naming and the mock in `commonMain` | #122 owns release isolation | Not changed here; the auth presentation model is the neutral `AccountView` |

Residual risks: a successful sign-in does not move focus (only Back restores it); the shell's
library remains the session-only mock and is labeled as not synced; the Application now builds
the mock container even when identity is configured; and wide screens use the same single column
(the design's two-column composition is not built). Membership has no Login entry because #128 owns
its form; the reducer and container already support Membership → Login → success.
