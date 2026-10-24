# Native signup acceptance and iOS link intake (#435)

Source-bound local evidence, not device, association or production acceptance. Part of
[#422](https://github.com/loveoverflowcom/cantos/issues/422) and
[#435](https://github.com/loveoverflowcom/cantos/issues/435), following merged #428, #431 and #434.
The requested order was Android first, then the remaining independent iOS source seam.

## Provenance and review boundary

- Base: `origin/develop@660dc9d82f4e552afc6c6987230cc4dca557497d`, fetched on 24 October 2026.
  The original clean `develop` checkout was fast-forwarded without reset/stash. Other worktrees,
  branches and existing data were preserved.
- Source checkout: `/Users/manhblue/Documents/Codex/2026-10-23/task-6/cantos-firebase`.
  Task checkout: `/Users/manhblue/Documents/Codex/2026-10-24/task-4/cantos-native-verification`,
  branch `feat/435-native-verification-acceptance`. The PR identifies its final source SHA.
- macOS 15.6 / arm64, Rust 1.98.1, Gradle 8.14.3, Microsoft JDK 21.0.7, Android SDK 35 /
  build-tools 35.0.0. Xcode 16.3 is installed; the required Apple platform and Kotlin/Native
  2.2.20 are not.
- The source is an iOS platform adapter plus a verification-only retry-clock correction on
  Android/iOS: no completion-flow rewrite, HTTP/Identity contract, session engine, migration,
  provider selection or production write.
  No new ADR or queue reordering is needed; ADR 0048 already selects the behavior.
- Full local logs and the transient HTTP driver were retained under
  `/tmp/cantos435-20261024` and copied to the task's ignored
  `artifacts/agent-runs/native-signup-435/`. No codes, passwords or grants were printed or saved
  by that driver; process logs were checked against those in-memory values.

The task reused the existing Rust build directory to bound disk use. `xtask` embeds its root at
compile time; commands after building it in the task checkout resolve that checkout even when
launched from the original directory. The clean-baseline repository reproduction below therefore
also uses the gate's Python primitive directly in the original clean checkout. The final
preflight is invoked from the task checkout. All changed Rust manifests/source are absent from
this diff; the three focused xtask failures concern identical base inputs.

## Provider reconciliation

Identity supports the existing Firebase verification bridge and `lettre` SMTP. The supported
`cargo xtask run cantos-theatre --web` composition selects the installed Firebase Auth Emulator
**14.2.2**, a unique `demo-cantos-*` project, outbound-denial controls, private fresh files and the
loopback `/verify-email` handler. Its real native proof/credential flow is not a mock success.
The provider itself is local simulation, not a production Firebase or mailbox-delivery result.

The [23 October test-host record](native-test-vps-resend-mail-20261023.md) selected **Resend SMTP**
through the existing adapter. That is historical host configuration, not a fresh live observation.
There is no `resend-rs` dependency in current Cargo manifests/lockfile, and password reset remains
deferred in the [auth contract](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md).
The native-first parser from #434 accepts Firebase action links. SMTP fragment proof links keep
their existing Web completion path; this change does not claim Resend mail opens native completion.

Research checked Resend's [official testing guide](https://resend.com/docs/dashboard/emails/send-test-emails)
and [Rust SDK](https://github.com/resend/resend-rust). Its test addresses simulate delivery events
through the hosted API and consume quota; they are not a local Identity proof emulator.
[y-hiraoka/resend-local](https://github.com/y-hiraoka/resend-local) is a third-party MIT package
(source package version 0.0.7), advertising a local mail-send API and browser preview backed by
SQLite. That does not implement Cantos's action-consumption/proof protocol or establish Resend
delivery fidelity. It was neither installed nor executed; no real mail, API keys or credentials
were used.

## Android-first checks

Use these installed-tool settings for the following xtask commands:

```sh
export RUSTUP_TOOLCHAIN=1.98.1
export JAVA_HOME=/Users/manhblue/Library/Java/JavaVirtualMachines/ms-21.0.7/Contents/Home
export ANDROID_HOME=/Users/manhblue/Library/Android/sdk
export PATH=/Users/manhblue/.local/share/gradle/gradle-8.14.3/bin:$PATH
export TMPDIR=/private/tmp
```

| Check | Actual result | Boundary |
| --- | --- | --- |
| `cargo xtask theatre-preview verify` on base and final task source | PASS: 523 shared + 129 sandbox selected, 5 existing skips, no failures | JVM common/desktop, production controller/client/navigation/UI with explicit fake external ports; not Android runtime |
| `cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:8080` in each checkout | PASS: build and APK inspection | Production host, local API override, debug package `com.cantos.theatre.dev`; no association |
| `cargo xtask theatre-host android-release` in the task checkout, held stable | PASS: unshrunk audit, optimized APK/AAB, graph/byte inspection and matching input record | Unsigned release; no installation/distribution. APK SHA-256 `1c71613a12f0f217a848355ae5bbd5bc176e126afebb0686f391eac3886759c7` |
| First release campaign | FAIL at the final source snapshot guard; both builds passed | A documentation edit during the audit/release window invalidated that run. The stable rerun above is separate evidence |
| `adb devices -l` | No connected device | No physical-device acceptance |
| Official API 35 ARM64 emulator setup | Installed and checksum-checked, but boot refused | No Android runtime or explicit-intent test executed |

The installed official system image is `system-images;android-35;default;arm64-v8a`, revision 2,
archive size 769099654, SHA-1 `2026a06409db630b56711afdbffb457c1dbaed49`; Google command-line
tools 19 archive SHA-1 `c3e06a1959762e89167d1cbaa988605f6f7c1d24`. Both came from
`dl.google.com/android/repository`, with metadata checked before extraction. Only these new
installer ZIPs were removed after installation/provenance capture; existing caches/data stayed.
The task AVD is `cantos435-api35` under `/tmp/cantos435-20261024/avd`, with no accounts or app data.

Emulator 35.4.9 recognizes the image and passes its initial compatibility check, then refuses:
`Not enough space to create userdata partition ... need 7372.800000 MB`; the last attempt had
2180.605469 MB available. An explicit 1024 MB partition override produced the same refusal.
No further partition/resource bypass was attempted. Retry needs adequate operator-provided
free space or a connected device; this task does not delete other work or DB/WAL/media to obtain it.

The bounded attempted command was:

```sh
ANDROID_AVD_HOME=/tmp/cantos435-20261024/avd \
  "$ANDROID_HOME/emulator/emulator" -avd cantos435-api35 -port 5556 \
  -partition-size 1024 -no-window -no-snapshot -no-audio -no-boot-anim \
  -gpu swiftshader_indirect -memory 2048 -cores 2 -netdelay none -netspeed full
```

Release OS association remains NOT_RUN: no reviewed release certificate fingerprint or hosted
`assetlinks.json` was supplied. Debug must remain unassociated. A future explicit `adb` intent
tests routing only, not OS verification or a mail-client tap.

## Real local Identity and Server

`cargo xtask run cantos-theatre --web` ran in the task checkout with fresh private native files,
the authenticated Identity Unix socket, Server HTTP and the supported Auth Emulator. A local
Python HTTP driver retrieved the action inventory into RAM, used only the approved loopback
handler/continuation, and emitted 17 sanitized PASS observations:

- generic signup; pending proof cannot log in; explicit confirmation and password-length
  rejection before consumption;
- real action completion with no cookie/session; identical completion returns `replayed`;
  another password cannot replay; explicit native login earns a real grant and Server admission;
- browser `Origin` is refused on native mutation (`401`, as the actual native guard specifies);
- anonymous completion of another signup preserves the first account's exact admitted session;
- `/`, `/verify-email`, `/verify-email.html` serve local HTML; codes/passwords/grants are absent
  from process logs.

An initial HTTP-harness expectation of `403` for the native browser header was wrong; the
production guard deliberately returns `401`. The corrected assertion passed; no source was changed.
These checks do not exercise the CMP host, Library/outbox/player runtime, a browser interaction,
the production provider, a real mailbox, or expiration after a real 900-second wait.

The retained SMTP capability was checked separately:

```sh
CANTOS_TEST_IDENTITY_BINARY="$PWD/target/debug/cantos-identity" \
  cargo test --locked -p cantos-server --test identity_auth_candidate
```

PASS: `real_identity_actor_session_journey` (1 selected/executed, no skips), production Identity
process/private RPC, actual loopback SMTP sink, Argon2id and real Identity/Server Turso files.
This existing test exercises Server application/session adapters, not its HTTP router or Resend.
The task-owned supervised stack was stopped cleanly; native files and evidence were retained.

## iOS source and available evidence

The App's one `@StateObject` owner exists above `WindowGroup`, so scenes use the same native
shell and verification controller. Browsing-web activities yield `webpageURL`, then clear their
URL/user-info and Handoff/search eligibility before any app callback. SwiftUI direct Universal
Links use the same synchronous forwarding path, as Apple's
[delivery contract](https://developer.apple.com/documentation/swiftui/view/onopenurl(perform:))
requires. There is no link queue, saved scene/navigation value, custom scheme or new auth client.
The shared parser remains the trust boundary for the exact HTTPS `cantos.cc/verify-email` URL.
The existing controller owns duplicate/busy/cancel/late-result behavior and anonymous completion;
`IosNativeIdentityOwner.close` clears it. Refused origins compose no verification controller.

Only Release has `CODE_SIGN_ENTITLEMENTS = Cantos/Release.entitlements`, containing exactly
`applinks:cantos.cc`. Debug has no entitlement; both keep the existing bundle ID. No Team ID,
AASA, provisioning, signing or hosted association was added. `plutil` parsed both the real
OpenStep Xcode project and entitlement; this is source configuration, not a signed entitlement.

| Command/check | Actual result | Limit |
| --- | --- | --- |
| `TMPDIR=/private/tmp python3 -m unittest scripts/test_theatre_ios_verification.py scripts/test_theatre_ios_identity.py scripts/test_theatre_ios_production_host.py` | PASS: 11 selected/executed | New tests first failed on the missing adapter/entitlement. The Foundation test compiles and executes the actual production intake, including scrub-before-callback, duplicate same activity, missing URL and foreign activity |
| `cargo xtask theatre-host ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.2.0` | UNSUPPORTED | Requires installed 2.2.20; installed 2.2.0/2.4.20 are not substituted |
| `xcodebuild -project apps/cantos-theatre/mobile/iosApp/Cantos.xcodeproj -scheme Cantos -configuration Debug -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath /tmp/cantos435-20261024/xcode-build CODE_SIGNING_ALLOWED=NO build` | FAIL (environment) | No matching destination; iOS 18.4 platform is absent. `-showBuildSettings` for Debug and Release also cannot configure a destination |
| Signed host, simulator/device Universal Links, Mail/Gmail, VoiceOver, multi-scene/cold/warm lifecycle, OS restoration/logging | NOT_RUN | Foundation/macOS and source checks establish none of these |

No UI layout, copy or motion implementation changed. Existing JVM production-root captures and
semantics remain host evidence; no Android/iOS screenshot, TalkBack/VoiceOver or native motion
acceptance is claimed.
Opened the fresh light 390×844 / 1× ready capture and dark 320×568 / 2× completed capture:
the first shows masked passwords, Vietnamese confirmation, its focus ring and create action;
the second shows the scrolled completion copy and Login CTA at enlarged text. It does not show
the entire heading at once. These are desktop renders with fake completion/credential ports,
not observations of an Android or iOS screen. Captures, hashes and semantics are retained in
the ignored evidence directory.

## Gate diagnosis and remaining acceptance

The final no-skip `cargo xtask preflight --changed-only --base
660dc9d82f4e552afc6c6987230cc4dca557497d` summary and the author's separate read-only review pass
are recorded in the PR. That earlier pass was not an independent reviewer. The parent later
reported the retry-clock defect below; independent re-review of the repaired delta remains
pending until the parent checks it.
No qualification digest is refreshed merely to obtain a pass.

Clean `develop` reproduction in the original checkout:
`python3 scripts/check_repository.py --base 660dc9d82f4e552afc6c6987230cc4dca557497d` FAILs on all
11 stale Leptos preview qualifications and their existing adopters. Three focused xtask failures
are also reproduced on unchanged base inputs: the two `preflight::graph` assertions detect the
existing Server-to-Identity dev dependency; `identity_environment_names_exist_in_the_identity_sources`
does not find `CANTOS_IDENTITY_FIREBASE_PROJECT_BINDING_APPROVED` with its current source scanner.
The focused graph run selected 7 tests (5 pass, 2 fail); the environment-name run selected 1 (fail).

The final `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` ran 1074
tests: 1057 passed, 2 failed, 15 skipped. Both failures reproduce in the original clean checkout
with `PYTHONPATH=scripts python3 -m unittest
test_vps_epoch2_cutover.Epoch2CutoverTests.test_journal_metadata_must_also_bind_the_exact_attempt
test_vps_oidc_reconcile.VpsOidcReconcileTests.test_nested_operations_reuse_the_same_lock_descriptor`.
The first assumes GNU `sed -i` syntax and receives BSD sed's undefined-label error (exit 1 vs
expected 65); the second cannot find `flock` (exit 75 vs expected 0). These platform failures are
separate from the new iOS tests, which all pass. No unrelated UI/legacy deployment tooling repair
or compatibility-tool install was folded into the PR. Python fixture output naming Kani is
runner-unit-test output, not an executed Kani proof or native qualification.

| Remaining layer | Prerequisite / next action | Acceptance boundary / owner |
| --- | --- | --- |
| Android runtime | Adequate disk for the official AVD or a connected device; local debug APK + `adb reverse tcp:8080 tcp:8080` | Cold/warm/duplicate/foreign/malformed/expired links, Back/recreation/process death, pending/late responses, explicit login and existing account/Library/outbox/player; developer/operator |
| Android verified link | Reviewed exact release fingerprint, release-signed install, operator-hosted exact `assetlinks.json`; `adb shell pm get-app-links com.cantos.theatre` | Verified OS/mail-client behavior; debug remains unassociated; operator |
| iOS compile and host | Installed pinned Kotlin/Native 2.2.20 and supported Xcode iOS platform; rerun framework lane and real Xcode host build | Swift/Kotlin interop and full host compilation before signed-device acceptance; developer/operator |
| iOS association/device | Verified Apple Team ID, reviewed exact AASA, signed Release, iPhone and Mail/Gmail | Cold/warm/multi-scene, no-app fallback, session and privacy behavior; operator |
| Live provider/mail/handler | Separate provider/handler decision and authorized operator rollout, associations and Web fallback first | Existing Resend SMTP is not Firebase action acceptance; no configuration/template or live-mail action is authorized here |
| Password reset | Capability remains deferred, requires its own work item | No invented reset feature or test success |

Residual risk: source and local HTTP/SMTP evidence do not accept Android runtime, Swift/Kotlin
interop, signed association, actual mail delivery, provider cutover or #422/#435 completion.

## Retry-clock review correction

The parent's independent review of `0ef10642` found that the new iOS composition injected
`NSDate` epoch milliseconds into the relative 429 `Retry-After` hold. It also confirmed the
same defect in Android's existing `System::currentTimeMillis` verification factory at the base.
The iOS defect was introduced by this PR; Android's was pre-existing. A forward wall-clock
adjustment could allow an early retry, and a backward adjustment could extend the wait.

Both verification factories now use the small shared `verificationElapsedClock`, based on a
process-local `TimeSource.Monotonic` mark. Only verification retry timing changes: the existing
epoch-seconds clocks still feed `NativeIdentityV1Client` and `NativeAccountController` for token
and server-deadline validation. No other clocks, session policy or rate-limit semantics change.
This follows [KT-CONCURRENCY](../../.agents/skills/cantos-cmp-mobile/references/kotlin-conventions.md#kt-concurrency--structured-injected-controllable).

The focused regression executes the production clock factory, controller and HTTP client with
a controllable `TestTimeSource` and an explicit HTTP double returning 429 then `replayed`.
Moving the independent epoch clock forward/backward by 24 hours leaves the five-second hold
unchanged; retry is still blocked at 4,999 ms and allowed at 5,000 ms with the original body.
The same client rejects token facts at the shifted epoch values and accepts them at the original
epoch, guarding against replacing token validation with elapsed time. Source guards bind both
native factories to the monotonic default; those guards failed before the fix.

The PR records refreshed scoped test/build/preflight results for this correction. The earlier
APK digests and test totals above remain attributed to their original source; they do not qualify
the repaired Android artifact. iOS full framework/host/device acceptance and Android runtime remain
blocked as described above. No cache cleanup is authorized or executed; the old exact cache
manifest must be treated as stale after new compilation and re-inventoried before any approval.
