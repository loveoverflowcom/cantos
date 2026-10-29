# Android release audit and explicit Firebase distribution

Implemented source policy for [#288](https://github.com/loveoverflowcom/cantos/issues/288).
Build, source/unit, audit, device and live distribution evidence are independent. This document
is not device/signing/Firebase acceptance. Native identity is first-party Identity v1
(email/password JSON on the API origin, opaque Bearer session); Web cookie migration in #295 does
not change these Android policies.

| Channel | Module/task | Application ID | Debuggable | Composition |
| --- | --- | --- | --- | --- |
| `development` | `:androidApp:assembleDebug` | `com.cantos.theatre.dev` | true | real only |
| `production` | `:androidApp:assembleRelease` | `com.cantos.theatre` | false | real only, R8/resource shrinking |

These are distribution policies, not server environments or Gradle flavors. Only `debug` and
`release` build types exist. API origin is an explicit HTTPS input for distribution, independent
of channel, and is the only app configuration: the catalog and Identity v1 use that one origin.
No issuer, browser callback or custom URL scheme is configured, registered or built in; the one
verified signup App Link (ADR 0048) is the only `BROWSABLE` filter. Sandbox adapters
belong only to JVM Desktop Preview/tests (#287).

## Local build and audit

```sh
cargo xtask release-build --self-test
cargo xtask theatre-host android-boundary-tests # cached deps/SDK; no APK, offline init-script probes
cargo xtask theatre-host android-debug --api-origin https://api.example.org
cargo xtask theatre-host android-release --api-origin https://api.example.org
# Clean committed source, credential-free packaging (also used by CircleCI):
cargo xtask release-build android
```

Theatre-host inspects debug bytes or, for release, performs this ordered sequence:

1. Snapshot source SHA and source content hashes without recording environment/secrets.
2. Build the **release variant** with `-Pcantos.releaseAudit=true`, optimization disabled and
   signing disabled, into `androidApp/build-audit`. Its separate project cache at
   `.gradle/release-audit` prevents Gradle's changed-output cleanup from deleting the audit APK
   during the optimized build. No tracked Gradle source is mutated.
3. Inspect audit APK/AAB manifest, DEX definitions, assets and resources before building optimized
   output. Require the production composition classes; compare against sandbox source inventory
   and fixture byte hashes without compiling any sandbox target.
4. Build ordinary optimized release into `androidApp/build`, inspect APK/AAB separately and
   retain manifest entrypoint non-vacuity checks. Obfuscated class-name absence alone is not
   exclusion evidence: both compile/runtime resolved graphs and **AGP ALL scoped class inputs
   before R8** are independently inspected. Source/resource checks catch copied fixture bytes.
5. Bind identical release pre-shrink class/source fingerprints and resolved graphs. Reject any
   disagreement or source change. Record exact APK/AAB/report/mapping SHA-256 and byte size in
   `androidApp/build/reports/release-boundary/release-evidence.json`.

The release coordinator holds a checkout-local POSIX lock for the whole audit/build/inspection
sequence; overlapping audits must use independent checkouts. Binding also checks the R8 mapping
ID against the optimized APK and AAB DEX markers. Successful binding retains an immutable,
hash-validated snapshot under `artifacts/android-release/<attempt>/`; CI packaging reads those
bytes rather than mutable Gradle output paths and revalidates the copied package. The evidence
record names its `snapshot_directory`. Failed or partial attempts never become accepted packages.
An explicit packaging API origin is passed through to the build and recorded configuration.

Only declared optimization flags, the audit manifest marker, app output root, isolated task history
and audit signing policy differ. Audit artifacts remain unsigned and carry `CantosReleaseAudit=true`. Distribution
inspects this marker in the actual staged manifest and requires false; renaming an audit APK or
passing its bytes under another filename does not admit it. Default release remains optimized.
AGP 8.11.1 keeps its existing separate resource shrinker; no AGP/CMP upgrade is included.
Generated serializers and Media3 retain their library consumer rules; AGP handles manifest
entrypoints. No blanket keep or global dontwarn is used. Optimized runtime smoke is mandatory.

`release-evidence.json` reports comparable **APK archive bytes**. Estimated download and installed
footprint stay `NOT_RUN` until measured separately on a named device/tool. No MB or percentage
target is selected. Local AAB inspection is not Firebase AAB support.

## Operator prerequisites and reviewable plan

```sh
cargo xtask distribute firebase cantos-theatre --channel development \
  --api-origin https://api.example.org --groups cantos-internal --dry-run
cargo xtask distribute firebase cantos-theatre --channel production \
  --api-origin https://api.example.org --groups cantos-internal --dry-run
```

Dry-run starts no subprocess, network, credential read, build or file write. It lists module,
build type, package, signer policy, configuration and unreached stages. Explicit recipients are
mandatory. Real execution omits `--dry-run` and is a remote write that may notify testers;
agents require explicit authorization naming the Firebase app and recipients. Ordinary
build/test/preflight/CI never upload.

| Input | Requirement |
| --- | --- |
| `--app-id` / `CANTOS_FIREBASE_DEVELOPMENT_APP_ID` | Firebase Android App ID registered for **com.cantos.theatre.dev** |
| `--app-id` / `CANTOS_FIREBASE_PRODUCTION_APP_ID` | separate App ID registered for **com.cantos.theatre** |
| `CANTOS_FIREBASE_DEVELOPMENT_CERT_SHA256` | exact intended debug signing certificate, required for real development distribution |
| `CANTOS_FIREBASE_PRODUCTION_CERT_SHA256` | exact operator release certificate, required for real production distribution |
| `CANTOS_RELEASE_KEYSTORE`, `CANTOS_RELEASE_KEYSTORE_PASSWORD`, `CANTOS_RELEASE_KEY_ALIAS`, `CANTOS_RELEASE_KEY_PASSWORD` | operator-provided release key; presence only is checked; no generation or fallback to debug/QA |
| `--api-origin` | exact public HTTPS build configuration, verified from staged manifest |
| `--groups` / `--testers` | explicit recipients; addresses stay out of logs/receipts |

Missing `.dev` registration or signer is an operator prerequisite, never
permission to reuse a `.demo`/production App ID. Firebase Android registrations cannot change
package. No Firebase SDK, Analytics or google-services plugin is needed by App Distribution CLI.

The real flow validates destination registration, builds through theatre-host primitives, resolves
exactly one AGP-declared APK (no glob, splits, zero or multiple outputs), copies exact bytes into
private `artifacts/distributions/<run-id>`, then inspects package/version/debuggable/signature,
configuration, audit marker and that the manifest declares no browser callback or URL scheme
beyond the exact signup App Link. Release mapping must match the same optimized APK and
mapping hashes in the audit evidence. APK and mapping are rehashed before the intent receipt.
A single upload uses those staged bytes; uncertain remote results are reconciled in the Firebase
console before any replay. Receipts are sanitized and preserve the observed outcome.

AAB distribution remains refused **before side effects** pending Play linkage, signing,
permissions and output policy. Unsigned local/CI APK/AAB builds are inspection artifacts, not
installable/distributable acceptance.

## Migration and acceptance still required

`demo`/`demo-qa`, `.demo` environment variables and the old QA key variables are retired; commands
are refused rather than silently uploaded to a new app. `production` now deliberately means
optimized release, never production-package debug. Historical receipts/recordings retain their
original topology in Git history. Synthetic policy fixtures are labeled as such.

`.dev` owns a new app-data sandbox; do not copy DB/tokens from `.demo` or production. A previous
`com.cantos.theatre` APK signed with another certificate cannot be upgraded in place. Preserve the
intended signing key and increase versionCode for updates. No tool uninstalls an old app or
deletes user data.

### Development build identity (#644)

Local, CI and audit builds keep the committed `versionCode 1` / `0.1.0`, which cannot say which
source an installed APK contains and is never evidence of "latest". A real **development**
distribution instead passes two explicit Gradle inputs, derived in `xtask/src/distribute/identity.rs`:

| Input | Value | Result in the APK |
| --- | --- | --- |
| `-Pcantos.versionCode` | Unix seconds at the run (strictly newer on every distribution) | `versionCode` |
| `-Pcantos.sourceRevision` | first 12 hex digits of `HEAD`, plus `-dirty` when the tree has uncommitted changes | `versionName` `0.1.0+<revision>-dev` |

After inspection, and before any upload, the staged APK's `aapt2` badging must equal both values.
A reused or stale output (for example the old `0.1.0-dev (1)`) fails the Inspect stage and nothing
is distributed. The receipt records the same versionCode/versionName and `source.commit`/`dirty`,
so on a device compare `adb shell dumpsys package com.cantos.theatre.dev | grep version` with the
receipt's `artifact` fields. Production keeps its existing policy; this lineage adds no
installed-device, Firebase console or tester acceptance, and a clock change must not move
backwards past a previously distributed versionCode.

Before release acceptance, record a named optimized-release device smoke: launch, VI/EN,
light/dark, icons/fonts, real catalog, Identity v1 email/password login/logout, Library/SQLite
reopen, account switch, player/background/Back. Record device/OS/package/version/certificate/APK
hash. Co-install, live Identity v1 sign-in and tester install/open are separate gates; missing
device/registration/key
means `BLOCKED`/`NOT_RUN`, not a host-test PASS. No live distribution or runtime acceptance is
implied by source/unit/build/audit results.

Primary references: [Android optimization](https://developer.android.com/topic/performance/app-optimization/enable-app-optimization),
[Firebase package registration](https://firebase.google.com/docs/android/setup#register-app),
[APK CLI distribution](https://firebase.google.com/docs/app-distribution/android/distribute-cli).
