# ADR 0048: Signup email link opens or returns to Cantos Theatre CMP

- Status: Accepted direction: option C, staged (owner selection, 24 October 2026, in the #422
  work chat). Stage 0 is implemented. The owner settled stage 1's handler path, signed-in policy
  and debug association the same day (see [Stage 1 owner choices](#stage-1-owner-choices)).
  Stage 1 is partly implemented in source: Theatre Web serves `/verify-email` and the deploy
  check admits the two association files ([#431](https://github.com/loveoverflowcom/cantos/pull/431));
  Android declares the App Link and CMP completes the signup (stage 1 PR 2). The association
  files, the operator's handler switch and signed-device association remain. The iOS source
  adapter now shares CMP completion; its full Apple framework/host build and device acceptance
  remain separate (see the [#435 record](../evidence/native-signup-435-20261024.md)). Optional
  stage 2 is not implemented
- Date: 2026-10-24
- Decision owner: Cantos owner (product, Identity/auth review for the security contract)
- Related issue or PR: [#422](https://github.com/loveoverflowcom/cantos/issues/422); context #296 (closed), #130, #224
- Supersedes / superseded by: N/A. Would amend the CMP
  [Signup Web dependency](../../apps/cantos-theatre/mobile/README.md#signup-web-dependency-and-acceptance-boundary)
  only after selection; keeps [ADR 0045](0045-v1-email-password-sign-in.md) and the
  [Firebase verification boundary](../architecture/firebase-verification.md) unchanged

## Context

Owner feedback (24 October 2026, screenshot supplied in chat, not attached to the issue): on
Android, tapping the signup verification email leads to the `https://cantos.cc` password page in
a browser, with no natural way back to Cantos Theatre CMP. The wanted outcome is app-first when
Cantos is installed and the next step can be handled safely, Web otherwise, and never a dead end.

### Source facts at `develop@97d192de`

| Area | Fact | Where |
| --- | --- | --- |
| CMP signup | Collects only email, posts `/native/identity/signup`; proof and password complete on Web; explicit CMP login afterwards. No link import, polling or resume login | [CMP README](../../apps/cantos-theatre/mobile/README.md#signup-web-dependency-and-acceptance-boundary), `NativeIdentityV1Api.kt` (no completion method) |
| Android host | `TheatreActivity` is `singleTask`, exported, with only the launcher filter. No `VIEW`/`BROWSABLE` filter, no `autoVerify`, no intent data read. Release `com.cantos.theatre`, debug `com.cantos.theatre.dev` with its own signing | `androidApp/src/main/AndroidManifest.xml`, `TheatreActivity.kt`, `androidApp/build.gradle.kts` |
| iOS host | Bundle `com.cantos.theatre`; no entitlements file, no Associated Domains, no `onOpenURL`/`NSUserActivity` handling, no URL types | `iosApp/Cantos/Info.plist`, `Cantos.xcodeproj/project.pbxproj` |
| Email link | Firebase `VERIFY_EMAIL` to the operator-approved `CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER` (exact HTTPS URL, no query). The link carries `mode`, `oobCode`, `apiKey`, `lang` and `continueUrl` with only the public `operation`/`proof` locators | [Firebase boundary](../architecture/firebase-verification.md#production-configuration-images-and-vps), [auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md#email-link-and-signup) |
| Web capture | Captures the query into RAM, replaces history before other I/O, refuses unknown/duplicate keys, requires `continueUrl` origin **and path** to equal the current page, then needs a chosen password plus explicit confirmation. Completed shows only a Web login CTA | `web/src/platform/signup_browser.rs`, `web/src/application/signup.rs`, `web/src/features/signup.rs` |
| Server | `/native/identity/signup/complete` already exists and shares the Web handler, including the `firebase_oob_code` alternative. The native channel refuses `Origin`/`Cookie`/`Sec-Fetch-*`. No session is issued | `services/cantos-server/src/adapters/inbound/http/identity_auth.rs` |
| Hosting | No `.well-known/assetlinks.json` or `apple-app-site-association` anywhere. The deploy bundle validator refuses every dot-file (`deploy/bundle.rs` `HiddenFile`) and admits only allow-listed static types; Firebase Hosting ignores `**/.*`. Cloudflare Pages serves a mandatory `404.html` for unknown paths because routing is by fragment | `xtask/src/deploy/bundle.rs`, `infra/firebase/hosting/firebase.json`, `infra/cloudflare/README.md` |

### Not established (UNKNOWN)

The production action-handler path actually configured; Gmail/Outlook/Samsung Mail behavior with
verified links on the owner's devices; Chrome/Custom Tab handling of same-host verified links;
iOS Gmail in-app browser handling of Universal Links; whether Firebase or the mail path rewrites
the link through click tracking. None of these were observed for this record.

### Constraints any direction must keep

From ADR 0045, the Firebase boundary and auth v1: opening a link (GET, prefetch, scanner,
App Link launch) never consumes the action or creates an account; completion needs a password the
mailbox owner chose and explicit confirmation; completion is not a session, and a native session
starts only after a separate native login; the oobCode, native proof secret, password and tokens
never enter a return URL, logs, analytics, navigation/saved state or storage; expiry, one-shot
consumption, identical-request retry and uncertain-outcome handling stay Identity's; Web keeps the
cookie, native keeps the opaque Bearer; foreign origins, open redirects and arbitrary intent
extras are refused. A custom URL scheme or hand-built `intent://` is not a default substitute for
verified HTTPS association.

## Decision

The owner selected **C, native-first completion, staged behind an A0 copy fix**. The Web flow
stays the fallback with identical semantics. Selecting the direction does not authorize any
stage-1 association, Firebase template, handler or deploy-validator change: each of those needs
its own reviewed PR, and production configuration stays operator-owned.

1. **Stage 0 (A0, no contract change).** The Web Completed state and the CMP check-mail guidance
   say plainly that a signup started in the app finishes by returning to Cantos and logging in.
   Copy and presentation only. No deep link, association or new route. **Implemented:** the
   Web `AccountCreatedNotice` (signup Completed and the Login notice) adds "if you signed up in
   the Cantos app, go back to the app and sign in there", and the CMP `RegisterWaiting` guidance
   says that the verification email opens Cantos in the browser and that the listener returns
   to this app to sign in.
2. **Stage 1 (C).** The action handler moves to the dedicated path
   `https://cantos.cc/verify-email` (owner choice). That path is claimed by Android App
   Links (`autoVerify`, exact host, exact path) and iOS Universal Links (exact `components` path),
   and Web serves the same SPA there as the no-app fallback. CMP parses the link with the same
   rules as Web: exact keys, `mode=verifyEmail`, bounded oobCode, `continueUrl` whose origin and
   path equal the **pinned build-configured handler** (never "whatever URL arrived"), lower-hex
   locators. It keeps the result only in RAM, then runs a CMP completion state machine
   (password, explicit confirm, identical retry, `replayed`, expiry, unknown outcome) against
   the existing `/native/identity/signup/complete`. On success a signed-out listener goes to
   CMP Login; a signed-in listener keeps the current session (see below).
3. **Optional stage 2 (A-return).** Only if devices show users still land on Web with the app
   installed (user chose the browser, iOS in-app browser): a Web "Mở Cantos" CTA after Completed
   pointing to a separately claimed, secret-free path. iOS does not open Universal Links for a
   same-domain navigation, so that path needs a host other than the page's own.

### Stage 1 owner choices

Selected by the owner on 24 October 2026 in the #422 work chat:

| Question | Choice | Binding consequence |
| --- | --- | --- |
| Handler path | `https://cantos.cc/verify-email` | Exact host and path. Android `<data android:scheme="https" android:host="cantos.cc" android:path="/verify-email"/>` with `autoVerify`, never a prefix or the site root. iOS AASA `components` names exactly `/verify-email`. Web serves the SPA at that path (Cloudflare `_redirects` 200 rewrite, Firebase Hosting rewrite) and still accepts nothing on other paths. The pinned build constant and the operator's `CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER` and Firebase template action URL all use this exact URL. Alias hosts and `www` are not claimed |
| Completion while signed in | Allowed | The completion screen is reachable while a listener is signed in. The request stays anonymous: no `Authorization`, no session fence, no change to the stored grant, Library or outbox. On success the app keeps the current session, says the new account was created, and offers only the existing explicit sign-out followed by Login. It never signs out, switches or logs in automatically. A late completion response after the listener changed account changes nothing but this screen |
| Debug association | None | `assetlinks.json` lists only `com.cantos.theatre` with the release signing certificate fingerprint. `com.cantos.theatre.dev` gets no entry, so its filter cannot verify and Android shows it, at most, as an unverified chooser option. Debug runs check the parser and state machine through an explicit `VIEW` intent to the pinned URL. On iOS the debug and release builds share bundle ID `com.cantos.theatre`, so the Associated Domains entitlement is attached to the Release configuration only. Verified-link opening is accepted only on release-signed builds |

Open for stage 1 review: the CMP Login pre-fill (none, recommended, since the link carries no
email) and the iOS minimum version for `components` matching.

Option B is not recommended. With the native completion route already present, it adds an
app-to-browser-to-app round trip and re-exposes the action in a browser URL, with no security gain.

## Alternatives

| Option | Tap path (app installed) | No app / desktop | Password owner | Contract/API change | Main risks | Relative effort |
| --- | --- | --- | --- | --- | --- | --- |
| **A0. Copy only** | Browser, completes on Web, copy tells the user to return to Cantos and log in | Unchanged Web | Web | None | Still a manual app switch; does not meet app-first | S (Web + CMP strings, existing tests) |
| **A. Return-to-app after Web** | Browser for the first tap; after Completed a CTA opens CMP via a verified secret-free link | Unchanged Web; the CTA falls back to a store/info page | Web | Association assets, one claimed return path; no Identity change | Return link must carry nothing secret or account-identifying; iOS needs another host; Android same-host handoff UNKNOWN; first tap still leaves the app | M |
| **B. Hybrid app-first + system browser** | Verified link opens CMP, which keeps a pending context, then opens the system browser/Custom Tab for the Web step, then returns via a second handoff | Web | Web | Association, two handoff paths, browser-to-app return | Action re-exposed in a browser URL/history; lifecycle across two app switches, cold start and process death; most moving parts and still no native password | L |
| **C. Native-first end to end (recommended, staged)** | Verified link opens CMP; user chooses password and confirms in CMP; CMP Login follows | Same URL serves the existing Web flow | CMP (no-app: Web) | No new endpoint (native completion exists); action-handler path, Firebase template URL, association assets, deploy validator exception, CMP completion port and state machine | Explicit intents bypass verification, so pin the host/path; the intent must be scrubbed and history relaunch ignored; account confusion when already signed in; debug cert association | M–L |

### Threat model notes for C

| Threat | Mitigation to require |
| --- | --- |
| Exported Activity receives an explicit intent with a crafted URL (bypasses App Link verification) | Same parser as Web against the pinned handler origin/path; accept only `ACTION_VIEW` with `https`; ignore extras. A crafted but valid link is equivalent to a phishing link on Web: Identity still verifies the consumed action, and the device gains nothing without the chosen password |
| oobCode kept in `Intent`, recents or saved state | Read once, then `setIntent` with the data removed. Ignore launches flagged `FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY`. Never put it in `SavedStateHandle`, navigation arguments, diagnostics or the credential journal. Process death loses it and the user taps the link again |
| System/OEM logging of intent data | Device-check what `ActivityTaskManager` prints for the launch. No app logging of the URI |
| Duplicate or late taps (`onNewIntent` during a pending completion) | One pending completion. A new link while pending is refused until it ends; late responses are fenced like Login |
| Already signed in as another listener | Completion is allowed (owner choice). Never touch the active session. Completion is anonymous on the wire; after success, say the new account needs an explicit sign-out and Login |
| Lost response or 503 | Identical retry only, with the same password re-entered. `replayed` counts as success. Never "nothing happened" |
| Association file abuse or drift | Association bytes reviewed in Git. Exact package and fingerprint or team ID. Served `application/json` over HTTPS with no redirect. Production publishes the release certificate only; debug has no association (owner choice) |

## Consequences

- Product and user impact: with C, installed Android/iOS users finish in the app and land on
  Login. Others finish on Web, as today. Email clients that open links in their own browser may
  still show Web; that remains a fallback, not a failure.
- Compatibility and migrations: no Identity, Server or wire change. The Firebase template's
  action URL and `CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER` must change **together** with a Web
  release that serves the new path. Links already mailed to the old path stay valid on Web
  until their 900-second expiry. Rollback reverts the handler/template and keeps the association.
- Operations, storage, and production cost: operator-owned Firebase template, handler
  configuration, Play/Apple signing identities and association assets. No new storage or paid
  service.
- Implementation and recovery requirements: a reviewed exception in the deploy bundle validator
  for exactly the two association paths, with exact content type and no other dot-files;
  Firebase `ignore` and Cloudflare `_headers`/`_redirects` handling of those paths and of the
  handler path; the Web parser's current-page path check moves to the new path; CMP parser and
  state-machine tests with JSON vectors shared with Web where practical; Android and iOS host
  adapters for cold start, `onNewIntent`, scene/continuation and process death.
- Deferred questions: the host of the optional stage-2 return link; whether CMP Login
  pre-fills nothing (current recommendation, since the link carries no email) or the email the
  same process just submitted; iOS minimum version for `components` matching. Handler path,
  signed-in completion and debug association are settled in
  [Stage 1 owner choices](#stage-1-owner-choices).

## Validation and revisit conditions

Selection evidence is source inspection at `develop@97d192de`. Stage 0 adds copy-only
regressions; the PR records their exact commands and results.

| Layer | Status |
| --- | --- |
| Source/owner inspection (paths above) | inspected |
| Stage 0 copy regressions (Web `auth_copy`, CMP `AuthCopyTest`) | see the PR |
| Stage 1 CMP parser, state machine, controller, HTTP client, navigation and JVM UI flow (`EmailVerificationTest`, `EmailVerificationControllerTest`, `NativeIdentityV1ClientTest`, `NavigationTest`, `VerifyEmailFlowTest`); manifest and inspector controls (`test_theatre_android_host`, `test_inspect_android_release`) | example-tested on JVM; see the PR. No CMP Login pre-fill: the link carries no email |
| iOS transient intake and Release-only entitlement | source-checked; real Foundation `NSUserActivity` intake is example-tested on macOS. SwiftUI direct URLs and browsing-web activities both use the process-owned shared parser/controller; full framework/host build, scene lifecycle and signed-device association are separate |
| Android device (Gmail, another mail client, installed/absent, cold/warm; verified opening on release-signed builds only, debug through an explicit intent) | NOT_RUN |
| iOS real device (Mail, Gmail, installed/absent); a simulator is not equivalent | NOT_RUN |
| Desktop browser no-app flow | NOT_RUN for this record; the existing Web journey evidence lives in the [Firebase boundary](../architecture/firebase-verification.md#evidence-and-remaining-acceptance) |
| Production template, handler, association hosting, live email | NOT_RUN; operator-owned |

Selecting C needs, before handoff: parser/state-machine regressions (replay, expired,
wrong-purpose, malformed, foreign-origin, duplicate keys, repeated tap, cancel, 503/unknown, late
response, signed-in isolation); `cargo xtask verify web`, Rust/deploy-validator tests and CMP
tests; separate Android and iOS device rows with the exact build, signing and mail client.
Revisit this record if Identity adopts a non-Firebase mail capability, if a native-only action
handler becomes available, if Android/iOS association semantics change, or if device evidence
shows verified links do not open the app from the mail clients listeners actually use.

The iOS adapter follows Apple's [SwiftUI delivery contract](https://developer.apple.com/documentation/swiftui/view/onopenurl(perform:)):
SwiftUI may deliver a Universal Link directly as a URL, while UIKit/Handoff uses
`NSUserActivityTypeBrowsingWeb.webpageURL`. Both entries are transient; the shared pinned parser
owns acceptance. The Release entitlement names the domain only; the operator's separately
reviewed AASA must restrict `components` to `/verify-email` and use the verified Apple app identity
([associated domains](https://developer.apple.com/documentation/xcode/supporting-associated-domains)).
