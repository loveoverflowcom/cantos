# Firebase distribution of real-only Theatre APKs

Implementation boundary: #288. Source/policy tests, unsigned local builds, installed native
runtime and live Firebase distribution are independent evidence. This lane never installs an
app, creates credentials/registrations, or accepts production listening.

| Channel | Module / build type | Application ID | Debuggable | Signer / optimization |
| --- | --- | --- | --- | --- |
| `development` | `:androidApp` / `debug` | `com.cantos.theatre.dev` | true | pinned machine debug certificate; unshrunk |
| `release` | `:androidApp` / `release` | `com.cantos.theatre` | false | operator release certificate; R8 + resource shrinking |

Both compose the real shared catalog/account/Library/native player. The sandbox is JVM Desktop
Preview/tests only (#287). A build type selects packaging, never the server environment.
`--api-origin` is mandatory: release requires HTTPS; development additionally admits the existing
loopback HTTP policy. `--native-issuer` is optional and must match the HTTPS native issuer contract.
Missing identity configuration leaves identity unavailable, without a mock fallback.

## Plan and operator prerequisites

```sh
cargo xtask distribute firebase cantos-theatre --channel development \
  --api-origin https://api.example.org --groups cantos-internal --dry-run
cargo xtask distribute firebase cantos-theatre --channel release \
  --api-origin https://api.example.org --groups cantos-internal --dry-run
```

A dry run starts no subprocess, probes no tools, makes no network request and writes no file.
It displays the module, build type, package, effective policy and unresolved prerequisites.
A real command is a remote write that can notify testers. An agent executes it only after the
user names the Firebase Android App ID and recipients. Build, tests, preflight and CI never upload.

| Nonsecret selector | Development | Release |
| --- | --- | --- |
| App ID (overridden by `--app-id`) | `CANTOS_FIREBASE_DEVELOPMENT_APP_ID` | `CANTOS_FIREBASE_RELEASE_APP_ID` |
| Required certificate SHA-256 | `CANTOS_FIREBASE_DEVELOPMENT_CERT_SHA256` | `CANTOS_FIREBASE_RELEASE_CERT_SHA256` |

The operator must register each exact package with Firebase. A `.demo`, `.dev` or release app
registration cannot stand in for another; package names cannot be changed on an existing Firebase
Android registration. The tool checks `apps:sdkconfig` before building, keeps only the requested
App ID/package facts, and never prints or saves the returned SDK configuration. An absent `.dev`
registration is an operator prerequisite; no registration is created automatically. No Firebase
SDK, Analytics or google-services plugin is needed for CLI App Distribution.

Release additionally requires `CANTOS_RELEASE_KEYSTORE`, `CANTOS_RELEASE_KEYSTORE_PASSWORD`,
`CANTOS_RELEASE_KEY_ALIAS`, `CANTOS_RELEASE_KEY_PASSWORD`. These go through Gradle's environment;
no password is a CLI property or receipt value. The operator supplies the existing key and pins
its certificate. There is no debug/QA signer fallback, key creation or credential rotation.
The installed Firebase CLI owns login/ADC; xtask never logs in. Installed Gradle/JDK/Android SDK
and firebase-tools 15.x >= 15.34.0 remain prerequisites, with no toolchain provisioning.

## Same-release audit and exact artifact validation

`cargo xtask theatre-host android-release --api-origin HTTPS_ORIGIN` builds an unsigned optimized
APK/AAB and an unsigned **unshrunk release** audit. `--native-issuer ISSUER` pins both builds;
`--sign` explicitly enables the operator key for the optimized build only. Debug never substitutes
for the release audit. There are only `debug` and `release` build types and no flavors.

The xtask-owned audit property selects `androidApp/build-release-audit/`, while default release
outputs remain in `androidApp/build/`. Audit mode disables only optimization and signing and sets
`CantosReleaseAudit=true` in the actual manifest. It never edits tracked Gradle sources. The
inspector rejects missing/true audit markers for distributable artifacts, regardless of filename,
package, version or signature. Unsigned artifacts also fail signature validation.

Both variants check resolved compile/runtime graphs, including transitive projects and test/mock
libraries. A source/resource guard rejects copied sandbox declarations and identical fixture bytes;
actual R8 class inputs are scanned immediately before shrinking. Targeted Android tasks never
compile the sandbox. DEX/resources/assets/manifest inspection runs on debug, audit APK/AAB and
optimized APK/AAB. Obfuscated names alone are not an absence oracle: the unshrunk audit and
pre-shrink guards carry that claim. AGP's optimized defaults plus existing dependency consumer
rules are used; no blanket app keep or global warning suppression is added.

`CantosBuildInputs` binds manifests to a digest of current tracked and non-ignored untracked
source/config bytes. The inspector checks it against the current checkout, plus exact origin,
issuer, callback scheme, package, permissions and production entrypoints. This rejects stale
outputs even when package/version are unchanged. AGP `output-metadata.json` must declare exactly
one unsplit APK of the selected variant/package; zero/multiple/split/foreign outputs fail.
Staged APK bytes are private/read-only, hashed while copying, inspected and rehashed before upload.
Release maps are staged first and their `pg_map_id` must match the actual APK DEX R8 marker; the
same check applies to the local AAB. A missing or foreign mapping blocks distribution.

`release-evidence.json` records source SHA/content digest, both equal dependency graphs, audit
status, exact hashes/sizes, mapping and pre-shrink report. Each inspected phase is snapshotted
before the next build under ignored `artifacts/android-release/<run>/`; later optimized outputs
cannot overwrite the audit. A source/config or graph change rejects the pair. APK file bytes are
compared on the same source/features/toolchain; estimated download and installed footprint remain
separate metrics and require their own measurement (`cargo xtask theatre-size`, named device).

`cargo xtask release-build android` packages those exact inspected snapshots, audit and optimized
APK/AAB, mapping, reports and hashes. CircleCI's build-only Android job uses that command via the
existing release preparation lane; it has no distribution credentials or upload step. AAB is
local evidence only: Firebase AAB distribution is refused before side effects until separate
Play linkage/signing/permissions/output policy is accepted.

## Receipt, recovery and upgrades

Distribution receipt schema 3 records module, variant, audit=false, optimization, source identity,
effective App ID/package, exact APK/mapping hash, version, signature/certificate, notes digest,
explicit groups/tester count and independent stage outcomes. Tester addresses, credentials, SDK
configurations and signed download URLs are excluded. The only remote write runs once after the
intent receipt. A timeout, lost/unrecognized response or partial outcome keeps a recovery pointer;
reconcile the Firebase console before retrying. No automatic replay is safe.

Legacy `demo`, `demo-qa` and `production` channel spellings are typed `UNSUPPORTED` refusals,
including dry runs. Old `CANTOS_FIREBASE_DEMO_*`, `CANTOS_FIREBASE_PRODUCTION_*` and `CANTOS_QA_*`
variables are not fallback authorities. Migrate selectors explicitly and verify the new package.
Historical evidence and old binary observations remain historical.

`.dev` has separate app data and callback registration under #285; it inherits no token/SQLite
state from `.demo` or release. Old `.demo` installs may still claim the production callback scheme;
the operator resolves that collision without an automatic uninstall or data deletion. Updating
`com.cantos.theatre` requires the installed certificate (or a separately accepted signing lineage)
and a higher versionCode. This implementation preserves versionCode 1 and creates no upgrade
entitlement: the operator must choose the next version before distributing an update. A different
previously used signer prevents an in-place update; do not replace keys or uninstall to hide it.

## Acceptance still requiring operators and native hardware

Run the optimized artifact on a named Android device: launch/theme/VI-EN/icons/fonts, real catalog,
#285 auth callback/co-install, Library SQLite reopen/account switch, native player/background and
Back. Record source/build/package/version/certificate/hash and inspect captures. Then, with
explicit named app/recipients and credentials, run distribution once and verify tester install/open
against the receipt. Unit/desktop/build PASS does not close these runtime/live acceptance rows.

Primary documentation: [Android R8 configuration](https://developer.android.com/topic/performance/app-optimization/enable-app-optimization),
[Firebase package registration](https://firebase.google.com/docs/android/setup#register-app),
[APK CLI distribution](https://firebase.google.com/docs/app-distribution/android/distribute-cli),
[AAB prerequisites](https://firebase.google.com/docs/app-distribution/android/distribute-cli?apptype=aab).
