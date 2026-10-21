# CMP native Identity v1 migration (#335/#336)

Status: implemented client integration; **native device and real-auth acceptance pending**.
Integration base: `9edd9b73246e639af212bea1526e62c6de349f9f`
(`refactor/cantos-native-turso`). This record covers the transport, lifecycle and host/UI slices;
GitHub owns issue execution status. It does not close #335/#336.

## Boundary and invariant

Only an explicit current login, securely persisted as an exact protocol-bound pair and admitted
by the API, exposes private account authority. A password stays in form/command/call RAM and is
never persisted, projected, logged or automatically retried. Unknown single-use refresh is
cleanup-only. Mail initiation, proof completion and local journal bytes never grant authority.

Owner/direction: checked domain values → existing `NativeAccountController`/registration owner
and ports → HTTP/secure journal/platform adapters. Android/iOS retain one process runtime, real
Keystore/Keychain and existing logout barriers. No session engine, backend endpoint, SQLDelight
schema, player policy, app ID, signing namespace or network policy was added/changed.

Transport commit: `94baf927b14a3bd4610e6d9bd19a932952af37ed`.
Lifecycle commit: `7feabbfb57aec6caacd0f3ccbd886ada3fe7437d`.
The host/UI commit selects v1 in both native roots and both offline/explicit-live Desktop Preview
compositions. Only external ports are mocked in `:preview-sandbox`; release never depends on it.
The artifact inspector now rejects retired issuer metadata rather than requiring a fake issuer.

## Migration and dependency

The same checked HTTPS API configuration binds `NativeIdentityV1Client` and version-2
`SecureNativeCredentialJournal` (`protocol: identity-v1`). Canonical same-origin provider
version-1 envelopes are classified for local retirement/relogin only; credentials/flows are
never sent to v1 and local retirement does not claim server revocation. Mixed/foreign/corrupt
records and unresolved durable denial barriers fail closed. Whole-pair readback precedes exact
session admission; same-family rotation preserves private Library partitions and playback.
The final review found an inherited wall-clock rollback defect in that owner: a renewal timer
could return the old lease without scheduling another renewal, and local authority could outlive
its elapsed absolute ceiling. Identity v1 now keeps an application-owned monotonic time floor
across refresh/revalidation/intents. It retains server UNIX deadlines and existing bounds, adds
no TTL policy, and never resets the floor to a rolled-back wall clock. Process restart still
requires API admission. Two deterministic regressions failed before the repair; terminal expiry,
renewal and same-family rotation/revalidation are covered afterward. This is active-process
JVM evidence; native sleep/suspend and secure-store/process-death behavior remain unaccepted.

Native signup accepts email only, calls `/native/identity/signup` once and displays generic
check-mail guidance. The [Web completion contract](../../packages/contracts/identity-auth-v1.md#email-link-and-signup)
is implemented by the backend but its Theatre Web frontend is absent. New-account onboarding is
**unaccepted** until Web proof/password completion is implemented and browser-tested; then the
listener explicitly logs in in CMP. No polling, app-resume login, native proof import or native
password-before-proof flow was introduced. The backend's transitional PostgreSQL session store
is not acceptance of the Turso Server port.

## Verification ledger

Evidence is Kotlin/JVM `example-tested`, `mock-integration-tested`, `semantics-tested` and
`interaction-tested` on the production owners/composables. Library regressions use real
in-memory/disposable SQLite with mock Identity/HTTP/decoder. None is device or real-auth evidence.
No Kotlin formatter/linter is configured; compiler, tests and manual source review apply.

| Layer | Observed result and scope |
| --- | --- |
| COMMON_TESTS | Final PASS: 535 tests, 0 failures/errors, 2 existing opt-in live skips. Includes the 16 transport and 32 lifecycle tests from the first two slices and 6 new signup/copy/redaction/projection tests and 3 final-review clock regressions. |
| PREVIEW | Final PASS: 103 tests, 0 failures/errors/skips. Five new tests cover live composition, form validation/keyboard, secret clearing/visibility, VI/EN generic 401/429/503 and email-only signup; existing navigation/logout/account/Library/player scenarios now use v1 commands. Final 320dp/2× text capture was opened: password label and actions are legible; all auth controls remain reachable by scrolling. |
| ANDROID_BUILD | PASS: final debug build/APK inspection and unsigned release audit/build/APK+AAB inspection; matching pre-shrink inputs and resolved graph. Packaging does not establish runtime acceptance. |
| IOS_BUILD | UNSUPPORTED: installed Kotlin/Native 2.2.0 does not satisfy pinned 2.2.20; no framework/Swift link acceptance. |
| ANDROID_RUNTIME | NOT_RUN: no installed-device fixture authentication, secure-store/process-death, native IME or TalkBack acceptance. |
| IOS_RUNTIME | NOT_RUN: no simulator/device Keychain/process-death, native input or VoiceOver acceptance. |
| REAL_AUTH_INTEGRATION | NOT_RUN: no configured local two-process auth fixture with an existing mailbox/account. No production account, mail, backend fixture mutation, deployment or distribution was used. |

Reproduction (from the checkout; installed Gradle 8.14.3, JDK 21, Rust 1.98.1):

```sh
export RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=target/review CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
export PATH="/Users/manhblue/.local/share/gradle/gradle-8.14.3/bin:$PATH"
export JAVA_HOME=/Users/manhblue/Library/Java/JavaVirtualMachines/ms-21.0.7/Contents/Home
cargo xtask theatre-preview verify
ANDROID_HOME=/Users/manhblue/Library/Android/sdk cargo xtask theatre-host android-debug --api-origin https://api.example
ANDROID_HOME=/Users/manhblue/Library/Android/sdk cargo xtask theatre-host android-release --api-origin https://api.example
cargo xtask theatre-host ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.2.0
cargo xtask preflight --changed-only --base origin/refactor/cantos-native-turso
```

Final unskipped preflight is **UNSUPPORTED**: the changed artifact-inspection script has unknown
impact, so preflight conservatively selects every gate; missing `initdb`, `pg_ctl` and `psql`
leave all other gates **NOT_RUN** in that invocation. The independent repository gate is **FAIL**
with exactly the same 25 UI qualification errors as clean base, with zero introduced final errors.
No qualification digest or guard was relaxed. The final affected Python primitive passes **43**
tests. A broader primitive run executed **1,046** tests: 6 failures, 2 errors, 15 skips. One
introduced architecture failure was corrected and passes in the affected rerun; the other seven
failures/errors reproduce by named test on clean `9edd9b73` (table-count assertion,
Linux-only OCI shell expectations, BSD sed and missing flock). The broad Python run is not a pass.
Full outputs and baseline comparison are retained; these failures are outside this auth scope.

## UI inspection and review

Screen/consumers: existing Cantos Login, email-only Register and Account; production shared CMP,
Android/iOS hosts and Desktop Preview. Guest-first navigation and VI/EN remain. Surface intensity:
Medium, existing lavender hierarchy. Components reused: AuthPrimary/AuthSecondary buttons,
Profile/Auth surfaces, Back/Logo, Material fields over current theme roles. The shared TextField
has no password transformation contract; the app-owned password field uses Material semantics,
IME actions and an explicit localized Show/Hide button. No shared tokens or qualification changed.

Opened JVM captures in `preview-sandbox/build/reports/theatre-auth/` include 390×844 VI light/dark,
EN light, expanded 768×1024, and 320dp/2× text with actions scrolled into view. Initial narrow/large
text capture exposed a squeezed password label; the toggle was moved below the full-width field.
Final same-scenario inspection confirms a full-width legible password label and reachable vertical actions; the header stays visible. Status text uses live-region/error
semantics. Tests drive field validation, keyboard transfer, visibility, Back/focus return, disabled
pending actions, cancel, logout and explicit retry. TalkBack/VoiceOver and native input are not
observed. Motion inherits the existing reduced-motion preview policy; smoothness/OS signals
were not observed. No performance benchmark or production performance claim is made.

Independent `cantos-code-review` covers the entire three-slice diff against the integration base.
It found and prompted fixes for a legacy live-preview owner that made v1 form submission inert,
and obsolete host/artifact assertions requiring issuer metadata. Repository checks additionally
caught direct feature-to-port failure matching and a new-file Button adoption; the fix projects
failures through application-owned display facts and keeps the form in the existing AuthScreens
owner. Independent review rechecked that delta. Review receipt and final
source/test provenance are in ignored `artifacts/agent-runs/cmp-chat3/` and the local handoff.
The fourth review re-read the full three-commit diff and used independent read-only transport,
lifecycle and UI/host passes. Its bounded clock repair follows the existing application owner;
no feature/port coupling, component-qualification waiver or backend scope was introduced. Final
review commands, the failing-before/passing-after regression output and current source provenance
are retained in `artifacts/agent-runs/cmp-chat4/` and its local handoff. The earlier broad Python
failure classification is reused only for unchanged inputs; the affected tests and repository
report are checked again. The GitHub workflow targets `develop`, so an empty PR check list on the
native integration branch is not a remote CI pass.

Residual risk: final repository gate remains subject to its actual report; Web completion,
iOS compiler/linking, both device lifecycles/secure stores, accessibility and real authentication
must be accepted independently. Next owner is unassigned — assignment required before acceptance.
