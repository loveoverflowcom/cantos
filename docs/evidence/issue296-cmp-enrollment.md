# Shared CMP enrollment and live preview — issue 296

Implementation and development evidence record, 2026-10-20. This is not registration activation,
full-stack signup acceptance, native device acceptance or acceptance of proposed ADR 0044.
Base: `develop@23ac242147f6e5e66bcacf48aa64d36ce92e14af`; this record travels with the implementation PR and its source diff.
The [contract](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/packages/contracts/theatre-identity-v1.md#native-cmp-admission-296-development-opt-in),
[ADR 0044](../decisions/0044-native-enrollment-preview.md) and
[preview runbook](../../apps/cantos-theatre/mobile/README.md#live-enrollment-integration-296)
own behavior, decision status and reproduction.

## Invariant and ownership

Only the existing native account owner and API-confirmed session can authenticate a listener.
Accepted mail requests, imported action links, successful proof and setup handoff never do.
The new native admission surface reuses the existing Firebase enrollment service, repository,
proof consumption, quotas and Kanidm issuer adapter. Web Origin protections remain on Web routes;
only the three exact native POST handlers own originless admission. Their runtime flag defaults off.

| Claim / failure mode | Enforcement / oracle | Evidence boundary |
| --- | --- | --- |
| Native cannot forge Web admission or use ambient credentials | Header rejection matrix, exact-path middleware classifier, default-disabled mounted routes | Production Rust router/service with isolated repository, bootstrap and issuer doubles; real action HTTP fixture |
| No second workflow or duplicate provisioning after native/Web replay | Both surfaces share the same `FirebaseEnrollmentService`; one create/setup and closed replay outcome | Rust mocked integration; existing SQL fences remain separately checked |
| Imported proof cannot select a host, operation continuation or account authority | Private factories; hostile query/continuation/setup URL tables; generated strict wire types from the shared schema | Kotlin/JVM examples and shared independent fixtures; no proof of mailbox ownership |
| Duplicate click, timeout, Cancel, Back or late completion cannot reopen setup or claim sign-in | Closed phases, command eligibility, request fencing, one-shot capability removal before browser I/O | Controller tests with controlled coroutine time; native HTTP handle cancellation; no process-death claim |
| Register renders the same code used by native hosts | `CantosTheatreApp` → `TheatreShell` → shared registration owner, `NativeRegistrationHttp`; Android/iOS external browser wiring | Compiled targets and rendered JVM interactions reported independently |
| Dependency failure stays an actionable failure | Real loopback connection refusal through the desktop transport; explicit capability retry; no fallback composition | JVM interaction over real HTTP, no full Cantos stack |
| Email input survives Back; secrets do not enter saved state | RAM state only, action input cleared on import; capability cleared on leaving/disposal/account admission; redacted representations | Source review, regression tests and captures containing synthetic/nonsecret state only |

Ownership before: Register was an explained-unavailable destination. After: the platform root
owns `RegistrationController` and the HTTP/browser ports; `TheatreShell` borrows it and binds
route entry/exit. Screens receive values/intents only. Domain values import no UI, HTTP or OS
types. The API remains the only proof/provisioning authority. No migration, entitlement,
Library reconciliation, publication or playback policy changes.

Request lifetimes are route-scoped within the platform's process scope. Back/navigation cancels
the current request and changes its fence; cancellation never promises remote rollback. A lost
start can still inspect mail without a resend. Lost confirmation/setup delivery discards its
capability and requires reconciliation. Only explicit new intent begins another request.

## Reproduction and execution boundary

Installed development toolchain: Rust 1.98.1, Gradle 8.14.3, JDK 21.0.7, Kotlin 2.2.20,
Compose Multiplatform 1.9.0 and Material3 1.8.2 on macOS 26.5.2. The JVM has no native audio decoder or secure
credential persistence. No library/toolchain was installed. Native code stays in `:shared`;
the live desktop host/transport and diagnostic strip stay in `:preview-sandbox`.

Commands use the already installed Gradle/JDK on PATH and `TMPDIR=/private/tmp` for canonical
macOS temporary paths. Full logs remain ignored under `artifacts/agent-runs/issue296/`.
The PR carries final gate summaries and independently reported native compilation/isolation.

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-preview verify
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify rust
TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.98.1 cargo xtask preflight --changed-only
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask theatre-host android-release \
  --api-origin https://api.example.invalid \
  --native-issuer https://idm.example.invalid/oauth2/openid/cantos-theatre-native
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify theatre-auth --suite firebase-emulator
```

The first deterministic core checkpoint passed before application/HTTP wiring. Canonical
`TMPDIR=/private/tmp` avoids macOS symlinked temporary paths in filesystem tests; no media source
was changed. Final gate summaries belong to the PR and are not inferred from intermediate runs.

Read-only assurance review covered the native/Web middleware classifier, default-off startup
configuration, shared service/receipt authority, factory construction and decoding paths, secret
representations, route/request lifetime fences and native/sandbox import direction. The resulting
implementation keeps public factory failures typed and validates HTTPS origins inside the proof
and setup factories, including when a caller supplies an invalid origin. No SQL, publication or
provider algorithm was changed. Required evidence gaps remain the full isolated journey and
native/Swift acceptance listed below; they are not waived by the review.

## UI observations and limits

Screen: shared Register/Login auth suffix; Medium-Low intensity, one primary action. Reused
catalog APIs: `button` AuthPrimary/AuthOutlined, `text-field` AuthTonal, existing auth Surface,
BackButton and Logo. No shared API, token role, palette, font, dependency or qualification was
changed. The existing app-owned `AuthCopy` was extended in Vietnamese and English; resource
parity tests cover all keys. Token path remains shared tokens → generated Kotlin → CantosTheme.

JVM Desktop Compose UI tests drive the production root, controller, generated wire and HTTP
adapter through the real desktop transport. Their loopback server is a **protocol fixture**;
catalog/identity preview ports remain deterministic and signed out. Setup-browser execution is
a recording port. These tests never inject a successful session and are **mock-integration-tested**,
not Firebase/Kanidm/PostgreSQL or complete enrollment evidence. The separately selected live mode
uses the actual native identity HTTP client, cryptographic entropy/S256 and system browser, with
RAM-only desktop credentials; its complete local journey is not claimed here.

Opened captures under `apps/cantos-theatre/mobile/preview-sandbox/build/reports/theatre-registration/`:

| Frame (dp) | Locale | Theme | App text scale | Observed state |
| --- | --- | --- | --- | --- |
| 390 × 844 | Vietnamese | Light | 1 | Email and primary send action; setup-ready explicitly says not signed in |
| 390 × 844 | English | Dark | 1 | Readable tonal field, primary action and outlined return controls |
| 320 × 800 | Vietnamese | Light | 2 | Heading/help wrap; the form is reached by scrolling |
| 320 × 800 | English | Dark | 2 | Header Back wraps without losing the theme control; content scrolls |
| 768 × 1024 | Vietnamese | Light | 1 | Field/actions fill the existing auth content frame without overlap |

Each PNG has a sidecar recording actual renderer density, dp constraint, locale/theme/text scale,
reduced-motion input and SHA-256. The baseline Register capture was opened before implementation.
Selected unmodified synthetic [captures and sidecars](issue296-cmp-enrollment/) travel with this
record: [VI/light form](issue296-cmp-enrollment/register-Light-Vietnamese-390-text1.0.png),
[EN/dark form](issue296-cmp-enrollment/register-Dark-English-390-text1.0.png),
[VI large-text action](issue296-cmp-enrollment/register-actions-Light-Vietnamese-320-text2.0.png),
[EN large-text action](issue296-cmp-enrollment/register-actions-Dark-English-320-text2.0.png) and
[API connection refusal](issue296-cmp-enrollment/api-down-light-en-390x844.png).
JVM interactions cover input, validation, submit, explicit proof confirmation/cancellation,
one-shot setup and return to Login. Default/large text, keyboard focus and scrolled actions are
checked by the final suite. Captures are synthetic and contain no live proof, setup URL, token,
private manuscript or account identity. Motion timing and real screen readers are not observed.
There is no configured Kotlin formatter/linter; Kotlin compiler, tests and manual style/import
review are the available evidence. Rust formatting/lint remains in xtask.

## Deferred acceptance

| Layer / status | Actual blocker | Next check and acceptance | Owner / residual risk |
| --- | --- | --- | --- |
| API + PostgreSQL + Auth Emulator integration: `UNSUPPORTED` here | The existing #242 lane requires Linux waitid and unprivileged user/network/PID namespaces; this host is macOS | On its isolated Linux prerequisites, run `cargo xtask verify theatre-auth --suite firebase-emulator`; actual SQL/OOB/proof receipt assertions must execute | #242; unassigned — assignment required before acceptance |
| Complete CMP live signup → Kanidm setup → native OIDC/session: `NOT_RUN` | The complete owned #242 Kanidm/PKI journey fixture is unfinished; no live provider/account/mail scope was supplied | Start the owned isolated dependencies, select the documented live preview, obtain emulator OOB, complete real Kanidm setup/login and observe API session; record each step and replay/error/recovery | #296/#242/#174/#191; protocol fixtures cannot establish it |
| iOS compilation: `UNSUPPORTED` | Installed Kotlin/Native 2.4.20 is not the required 2.2.20 | With separately installed pinned compiler, `cargo xtask theatre-host ios-build --kotlin-native-home PATH`; then build/run the Swift host | #191/#184; new Swift bridge remains a compilation/runtime gap |
| Android/iOS native journey: `NOT_RUN` | No installed-device acceptance performed | Separately run system-browser return, process death, secure store, account switch, TalkBack/VoiceOver and large text on named targets | #191/#184; JVM results supply no OS or device acceptance |
| Production activation and abuse/issuer acceptance: `NOT_RUN` | Proposed admission requires owner review and existing operator gates | Review ADR 0044, real ingress/quotas/issuer scopes, recipient policy and exact origins before explicit activation | Operator; flag remains false by default |

No real mail, cloud credential, account, provider deployment or application installation was
created. No whole-issue, device, issuer, mail-delivery or production-ready claim follows from
the development implementation or PR. Residual risk: full isolated enrollment, Swift/native
runtime and production admission remain unaccepted independently of passing core/JVM layers.

The supervised live launcher was executed with `https://127.0.0.1:9` and a synthetic `.invalid`
issuer, then its owned process group was stopped and checked absent. Native automation listed
the JVM `MainKt` process but refused to bind its window by either displayed name or bundle ID;
there is no claimed manual-window interaction/capture from that run. The actual input-event
and connection-refusal observations above come from the named JVM Compose UI test renderer.
