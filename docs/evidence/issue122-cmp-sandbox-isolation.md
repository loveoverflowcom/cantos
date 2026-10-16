# CMP preview sandbox module isolation — issue 122

This change separates the preview build graph from the shared application code. It is part of
[#122](https://github.com/loveoverflowcom/cantos/issues/122), based on
`develop@84ebda4f30fbba271ca8bbc68839f69e37afa646`, including merged #163 and #164.
The first slice (PR #165) recorded production Android release acceptance as BLOCKED / NOT_RUN
because only a demo host existed. The
[production host follow-up](#production-android-host-and-release-artifact-acceptance) adds a
production Android host that composes `:shared` only, its explicit release command and actual
release classpath plus APK/AAB inspection. A demo APK, library graph or R8 removal is still not
used as production exclusion evidence.

## Ownership and compatibility

```text
preview-sandbox (KMP) -> shared (KMP)
Android demo         -> shared + preview-sandbox
desktop/iOS demo     -> preview-sandbox -> shared
production Android host -> shared only (follow-up below; no listening adapter exists yet)
```

- `shared` retains domain rules, ports, application controllers, injected `AppContainer`,
  native identity adapters, fonts with font-only provenance, and the actual
  `CantosPresentation`/auth/Membership UI.
  `presentation/model` is a plain Kotlin leaf consumed by those three presentation features.
  Opaque Work/Episode IDs replace fixture-specific presentation enums. No default catalog,
  storage or engine implementation is constructed by shared code.
- `preview-sandbox` owns the existing mock catalog/media/storage adapters, deterministic
  database and sessions, synthetic inventory, mappings, artwork, `mockAppContainer`,
  `MockStorageSessionApp`, desktop workbench, Android preview wrappers and demo composition roots.
  Cantos-owned `FakeStorage*`/`FakePersistentProgressPort` identifiers become `Mock*`; third-party
  Okio `FakeFileSystem` naming is unchanged. No Okio, SQLDelight or Ktor dependency is added.
- One renderer and one copy of each reducer remain. Sample notices, reset/scenario controls and
  manual Membership fixture controls/copy are injected only by sandbox providers; shared
  diagnostic slots default to absent. Membership's existing pure state machine and explicit
  fixture-mode value remain shared, with production submission unavailable; this is no claim
  that Membership receipts or native playback are production-ready.
- The three sample story keys no longer constrain `PlaybackSource`/`PlaybackIdentity` in main
  code. Those constructors reuse the existing opaque `WorkKey` nonblank invariant. Catalog
  lookup still checks the episode, and callback Work/Episode/generation matching remains exact.
  The independent canonical-vector decoder and sandbox mappings retain their fixture allowlist.
  Reducer algorithms and canonical vector bytes are unchanged; no API or wire contract changes.
- #163 recipe/snapshot v1 field names, fixture IDs, replay, reset, privacy checks and owner fencing
  are preserved. #164 keeps the process-owned `NativeAccountController`, typed Back/return
  context, callback cancellation and borrowed-owner cleanup. Static Android wrappers mount the
  same real UI. iOS keeps the `CantosTheatreDemo` framework name and Swift interface; the sandbox
  framework exports shared code, and Xcode paths copy both modules' resources.

The mock storage implementation is moved and renamed, not replaced with a SQL simulator.
`ProgressPortContract` and its two memory-only bindings live in the sandbox to avoid a
`shared` test dependency back to its dependent module. Pure navigation, playback, identity,
codec and font tests remain independently runnable in shared. All 325 original test
annotations are preserved; five boundary regressions are added. Execution counts below include
inherited contract cases and therefore differ from source annotation counts.

## Enforcement and executed checkpoint evidence

The repository guard checks all shared main Kotlin/resource roots, inward layer ownership,
literal Gradle project edges and transitive local-project cycles, dependency coordinates and
simple aliases, and declared source routing. It rejects mock/sandbox/legacy names and sample
resources. Canonical UI preview exclusion must be in the same source set as its inclusion.
This is bounded lexical/DSL inspection, not arbitrary Kotlin/Gradle program analysis.

`:shared:assertProductionSafeDependencies` separately traverses actual resolved components,
rejects unresolved results, and refuses sandbox/mock and test-only dependencies. Main compilation,
assembly and `desktopTest` depend on it. Android opts into both **shared library** release
classpaths; missing expected configurations fail rather than silently skipping inspection.
Its report path is `shared/build/reports/production-boundary/resolved-dependencies.txt`.

Checked implementation input digest:
`607e73eae1e94dc9bfe9ec56653bd2f4bf854dd4792e9ed75b9213f7a9401c26`
(206 Git-inventoried non-document mobile/configuration and xtask source files plus the two
repository-check scripts; binary artwork/font preservation checked separately). Full local
records are under ignored `artifacts/agent-runs/issue122-cmp-isolation/` and
`artifacts/agent-runs/issue122-architecture/`. Handoff preflight and the final Git tree identity
are reported in the PR; the table records the executed checkpoint scope.

| Layer / claim | Status | Executed evidence and scope |
| --- | --- | --- |
| L1 source and build isolation | PASS | 30 focused Python architecture tests, including eight real-tree negative mutations; old checker yields 57 expected failed assertions against the new boundary controls. Existing layer checks retained. |
| L1 resolved shared library graph | PASS | Desktop compile/runtime and Android shared `releaseCompileClasspath`/`releaseRuntimeClasspath` resolved without forbidden components. This is not a production app graph. |
| L1 resolver sensitivity | PASS | An ignored process-local Gradle init script adds cached `org.jetbrains.kotlin:kotlin-test:2.2.20` to shared main. The guard exits 1 with `Forbidden shared main dependencies` for both desktop classpaths. No project file is edited and no compile follows the rejected graph. |
| L2–L3 core / types / errors | PASS | `cargo xtask theatre-demo verify`: shared desktop suite, 123 executed, zero failures/errors/skips. Includes opaque catalog source and exact wrong-source/episode/stale-generation callback fencing, existing numeric bounds, independent canonical corpus and exact bundled-font/license provenance. |
| L3 xtask plans / supervision | PASS | `cargo test -p xtask --locked`: 164 passed. `cargo clippy -p xtask --all-targets --locked -- -D warnings` and scoped Rust format check pass. These are runner tests, not native app evidence. |
| L4 mock application / UI | PASS | Same Theatre verify command: sandbox desktop suite, 213 executed, zero failures/errors/skips. Existing ordering, faults, cancellation/reset epochs, contention, exact port contracts, owner disposal, account flow and workbench snapshot/replay/selection tests execute. Shared renderer test asserts an ordinary injected mount has preferences without sample notices or diagnostics. |
| L5 Android compilation | PASS | `cargo xtask theatre-demo android-build`, existing SDK 35: both libraries, moved preview wrappers and demo debug APK compile/package. No app launch/device acceptance. |
| Android Studio preview capture | UNSUPPORTED | `cargo xtask compose-preview cantos-theatre --file apps/cantos-theatre/mobile/preview-sandbox/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt --composable TheatreFreshSessionPreview --label issue122-fresh --timeout-seconds 30`: official Android CLI unavailable. |
| iOS framework link | UNSUPPORTED | `cargo xtask theatre-demo ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.2.0`: installed version differs from required 2.2.20; no compiler or LLVM installed. Native runtime NOT_RUN. |
| Production Android release graph and APK/AAB | BLOCKED / NOT_RUN | No production host or production release command exists. Demo output is explicitly excluded as evidence for this criterion. |

Tools selected from existing installations: Rust 1.98.1, Gradle 8.14.3, Microsoft JDK 21.0.7,
Python 3.13 and SDK 35/build-tools 35.0.0. `TMPDIR=/private/tmp`; Gradle's existing user home is
`/private/tmp/cantos-gradle-user-home`. One heavy lane ran at a time. This task installed no
SDK/tools, deleted no caches/evidence/worktrees, provisioned no real service and inspected no
GitHub CI status.

Earlier failures are retained: sandboxed xtask had 157 passed / 7 failed at denied loopback or
process primitives; one host-authorized retry passed all 164. First Theatre checkpoint compiled
both modules but failed one workbench path guard (120 shared + 212 sandbox tests). Its assertion
was migrated to inspect the sibling shared UI exclusion and sandbox-only settings import; the
later 122 + 213 run passes. These are separate from missing native/production prerequisites.
The first handoff repository gate also caught eight stale local links and an unstaged JSON
move still present in Git's tracked-file inventory. Links are migrated to current ownership,
and the unchanged provenance bytes are staged at their new path; the checker is not weakened.
A later gate identified that the qualified Web font bundle intentionally hashes its README.
The correct resource split retains meaningful font-only provenance with the four shared fonts
and their OFL, while preserving the original mixed artwork manifest byte-for-byte in the sandbox.
An exact-inventory/hash/license test checks the shipping font record. The original font README
then truthfully retains its original link and exact baseline bytes. The unchanged Leptos digest
is `82e7446300057dc01e60a0a0947255a3b9433256a60d73b4330b7c6d36f46eec`;
no qualification, baseline, digest policy or guard is edited. A new affected Kotlin run passes
123 + 213 tests after adding the resource, and Android packaging is rechecked for that input.
Kotlin emits one nonblocking unreachable-null warning in `PlayerControls.kt`; no Kotlin
formatter/lint gate is configured.

## Artifact and visual observations

The shared desktop JAR has 584 entries and no searched mock/sandbox entrypoints, sample
catalog/storage fixture classes or sample artwork paths (SHA-256
`969e2a04ab6a06f423fdc89b76b92d856c90a87effd67e8434115cbb2329e6a0`). The shared Android **debug
library** classes JAR has 573 entries, no searched mock/sample paths or sample story-key constants;
its merged library manifest declares minSdk 23 and no permission. Inventories and exact hashes
remain local. These checks do not scan a production APK/AAB or assert release resource absence.

Two final JVM captures were opened and their semantics inspected under
`artifacts/ui-evidence/issue122-cmp-isolation/font-provenance/`:

| Recipe | Observed pixels and semantics | Capture SHA-256 |
| --- | --- | --- |
| `theatre-fresh-v1`, seed 1; light, Vietnamese, 390×844 dp frame, 1× text, reduced motion | Real Library UI shows empty Continue/Saved groups, sample notice, zero position/saved count, mini-player and two destinations; workbench controls surround the same app composition. | `8091ba0768bb642c8d8905a0adacb6ecd3d14a7d64e91a492c6561318b39d3d8` |
| `theatre-restored-v1`, seed 1; dark, Vietnamese, 320×800 dp frame, 2× text, reduced motion | Large text wraps in the narrow real Library UI; restored episode 3, position 768000 ms and two saved works appear in projection/semantics, with mini-player and navigation retained. | `e04f870c9b5f93e95124e04eaf523f35aa3cd2403dd34c1b58a5e33f6d62dbbb` |

Both are 1200×1000 dp desktop test windows, renderer density 1. Their inner frames are layout
constraints, not devices. Captures establish inspected static rendering; the passing input tests
provide JVM interaction/focus evidence. No TalkBack/VoiceOver, OS process death, real file/SQLite
persistence, network delivery, native playback or audio quality result is claimed.

## Resumed verification after disk cleanup

After the user freed disk space, the same clean implementation commit
`0eb18b5c267ed55854bbb4339a064e417a3d3eb3` (tree
`ca4e2bf756ef196458271a0e279648ee6fb983a8`) was checked with 35 GiB initially free.
Freshly fetched `origin/develop` remained `84ebda4`; the remote feature branch matched HEAD.
No concurrent Cantos build was running. Existing Homebrew PostgreSQL was left alone; the gate
created and stopped its own new disposable cluster. No additional cleanup or tool installation
was performed. Conventional prior evidence files were copied before suites could overwrite
their latest-record paths; existing revision cluster directories were retained in place.

The resumed command was:

```sh
cargo xtask preflight --changed-only --skip repository --skip rust-format \
  --skip script-ir-reference --skip theatre-cmp-demo
```

The four skips reuse the matching implementation inputs and passing results from the initial
handoff; they are `NOT_RUN` in this invocation. This run's exact summary is **FAIL**, not an
aggregate pass. Full output is retained in
`artifacts/agent-runs/issue122-cmp-isolation/preflight-resumed-gates.log`.

| Resumed gate / claim | Status | Evidence and bounds |
| --- | --- | --- |
| Protobuf reference, Rust workspace lint/tests | PASS | Independent goldens/drift oracle and workspace lint pass. Workspace tests: 1583 passed, zero failed, 300 ignored; ignored I/O tests require their separate gate. |
| DBSP | PASS | Opt-in generic engine lint/tests: 696 passed, zero failed, 300 ignored; no synthesis worker or production adoption. |
| WAV reference and actual media inspection | PASS | Python WAV/hash oracle and two ignored real-media tests run with existing ffmpeg/ffprobe on temporary synthetic signals; no story audio or native playback acceptance. |
| Web lint, both Web builds, shared Leptos preview | PASS | Existing WASM lint/build commands complete; compilation does not establish browser runtime. |
| Shared CMP preview | PASS | Existing Gradle desktop test gate completes; no device or official Android Studio capture claim. |
| Script IR mutation | PASS | Baseline 16 tests passes and all nine bounded mutants are killed in a disposable copy. |
| PostgreSQL, first attempt | FAIL | 208 tests pass before one private-artwork additive-upgrade assertion fails: actual migration versions 1–26 versus expected 1–25. |
| PostgreSQL phases after the first failure | NOT_RUN | At that input, later private-artwork repository/storage, listener/audio-intake and socket/restart phases were not reached. |

The failing assertion at `services/cantos-api/tests/support/private_artwork_postgres.rs:674`
and the registry at `services/cantos-api/src/platform/postgres/migrations.rs` are unchanged from
the fetched base. Both base and HEAD register migration `0026_catalog_delegation.sql`; the test
still expected `(1..=25)`. At that input, the API source/tests, contracts and Cargo lock had no
branch diff. This is a source-confirmed pre-existing stale expectation; no baseline database
execution is claimed. The user then approved exactly one character, `25` to `26`, in the test.
The strict inventory equality and every checksum/snapshot assertion remain unchanged.
The corrected test file SHA-256 is
`1c15a52d1ed22f37411b6526a7ff1a1ee75620901183c6b583f42bd4ab129fb0`;
the 206 original implementation inputs above remain unchanged.

`cargo xtask verify postgres` on a second new cluster passed the private-artwork suite, including
the corrected assertion, and the later repository/storage/listener suites. It reached **239
passed / 3 failed**: all three audio-intake failures share the existing
`owner_audio_request` assertion at `services/cantos-api/tests/audio_intake_postgres.rs:1441`,
which expects plain `no-store` for handler-owned GET/HEAD audio responses. The unchanged production
transport returns `private, no-store, no-transform`; PUT still requires plain `no-store`.
At that input, the helper and transport were unchanged from the fetched base. The proposed
test-only helper patch preserves exact equality per method and rejects unexpected methods.
No production cache policy is relaxed.
The second gate is **FAIL**; socket/API/database restart phases were not reached (`NOT_RUN`).
Its complete output and first failing audio suite are retained as
`postgres-approved-expectation.log` and `postgres-second-failure.log` beside the initial log.
Disk after the campaigns was approximately 20 GiB free; resource deferral is no longer the blocker.

The user subsequently approved that exact Rust helper patch. Its file SHA-256 is
`0bac285d3c6b13d0d7cec18675da4ab200359c6ed25340964237cb6f85d15133`.
GET/HEAD owner bytes must exactly match `private, no-store, no-transform`; PUT must exactly match
`no-store`; unknown methods fail explicitly. Every other header/status/body/storage/integrity
assertion remains unchanged. Independent read-only review found no actionable finding.
`cargo xtask verify rust` passes format, workspace lint and workspace tests (1583 passed, zero
failed, 300 ignored); this is separate from the real PostgreSQL evidence.

A third `cargo xtask verify postgres` run passed **all 242 Rust PostgreSQL cases**, including
all 21 audio-intake cases and the three previously failing reads. That attempt reported
**FAIL**: the first real-socket audio recovery probe stops at the unchanged
`scripts/test_postgres.py:593` cache assertion before the restart cycle. The script still expects
plain `no-store` for private byte reads. Its SHA-256 matches fetched develop:
`28396e37f6bb338ade95d7d467eee3f5cc11df3c1949c03aa13872b9e1874ef1`.
The same Python helper also checks a read-access 303 redirect, whose expected policy correctly
remains plain `no-store`; replacing every expectation with the private-byte policy is incorrect.
Full output is retained as `postgres-approved-cache-helper.log`; affected Rust checks are in
`rust-approved-cache-helper.log`. The runner stopped its own hosts and disposable cluster.

A complete independent read-only audit covered all 14 integration targets, three filtered library
modules, recursive supports and all nine Python cache assertions/dependencies, including restart
paths. Exactly one stale Python oracle remained. JSON, PUT, early-authentication, import and
artwork assertions had appropriate distinct policies and were not changed.

The user approved the complete Python-only correction: a required keyword-only
`expected_cache_control`, exact header equality, and all five explicit call expectations. Four
byte-read call sites (GET/HEAD 200, GET 206/416) require `private, no-store, no-transform`; the
read-access 303 call retains `no-store`. All other status/body/hash/length/ETag/range/encoding,
`nosniff` and redirect assertions remain intact. Independent final diff review found no actionable
finding. The corrected script SHA-256 is
`f01364c584e8af1c6644b0a4fecb9dceff7f23ab6405015ae1308cd665fc7bcf`.

The fourth full `cargo xtask verify postgres` run reports **PASS**: 245 Rust PostgreSQL cases
passed, zero failed or ignored, including the three deferred restart cases. Real socket HTTP,
API process kill, PostgreSQL restart, exact immutable replay, owner GET/HEAD body/hash/length/type/
ETag, first/middle/suffix/final ranges, 416 and local read-access 303 all execute successfully.
The before/after audio recovery projections are equal. This is backend integration with synthetic
fixtures and named mock identities; it does not establish native device or production delivery
acceptance. Full output is retained as `postgres-approved-python-oracle.log`.

The resulting `target/catalog-evidence/audio-server-restart.json` record has SHA-256
`b5e2c5ae4d16d0a883ae4f65ba8b8494dcda29eee614a9afcd161b5f53915733`; a copy is retained with
this run's ignored evidence. The runner stopped only its own hosts and disposable cluster.
All earlier failure logs and old records/clusters remain retained. Disk remains approximately
19 GiB free; no SDK/tool was installed or old evidence/cache deleted. Final repository/Python
hygiene is reported separately in the PR; unchanged Rust/Web/CMP/DBSP/media/mutation results are
reused on their recorded inputs rather than repeated for commit/push packaging.

Independent source inspection still finds only the Android `.demo` app, whose unconditional
sandbox dependency and mock composition root predate this PR. There is no production host or
xtask production release command. Shared library release graphs cannot clear that acceptance.
The exact Kotlin/Native 2.2.20 and official Android preview CLI prerequisites remain unavailable.

## Production Android host and release artifact acceptance

The follow-up is based on `develop@f3619bf`. It closes the remaining #122 criterion with a real
production host instead of treating the demo APK as evidence.

```text
:androidProductionApp (com.cantos.theatre) -> :shared
:androidApp (com.cantos.theatre.demo)      -> :shared + :preview-sandbox
```

- **Host.** `apps/cantos-theatre/mobile/androidProductionApp` is an Android application module
  included only with `-Pcantos.android=true`, like the demo host. Its single launcher
  `TheatreActivity` is the production composition root. No production catalog, progress or media
  adapter exists yet (#14), so it cannot compose an `AppContainer`; it renders the shared
  `CantosListeningUnavailable` state. That state is explicit, localized (Vietnamese default,
  English) and has no catalog, library, player, sample, diagnostic or loading content. There is
  no Application subclass, permission, identity callback, metadata, network adapter or mock
  fallback. The minSdk 23 floor comes from the shared library and is not a support decision.
- **Release type.** The release is unsigned and unshrunk (`isMinifyEnabled = false`,
  `isShrinkResources = false`), so absence comes from the dependency graph and never from R8.
  Signing, shrinking, distribution, launcher artwork and platform support remain separate #14
  decisions. Launcher artwork stays with the demo host; the production host has the system default
  icon, so no launcher acceptance is claimed.
- **Gradle boundary.** `:androidProductionApp:assertReleaseDependencyBoundary` runs before every
  build of the host (`preBuild`). It resolves `releaseCompileClasspath` and
  `releaseRuntimeClasspath`, rejects unresolved results, allows local projects `:shared` and the
  host only, and refuses test/mock/tooling modules. Report:
  `androidProductionApp/build/reports/release-boundary/resolved-dependencies.txt`.
- **Artifact inspection.** `cargo xtask theatre-host android-release` builds
  `:androidProductionApp:assembleRelease` and `:bundleRelease`, compiles the sandbox's Android
  release classes as the forbidden inventory, and runs `scripts/inspect_android_release.py` on the
  actual APK and AAB. The inspector parses `class_defs` of every DEX file, so it reports
  defined classes rather than referenced types. It rejects the exact sandbox class inventory,
  Cantos mock/demo/preview/workbench names, and test or Compose tooling namespaces. It also
  rejects sandbox resource bytes even when renamed, foreign Compose resource packages and
  mock/demo/sandbox/preview paths. Manifests are decoded by installed `aapt2`; the AAB's proto
  manifest is repacked as a proto APK. Each must declare the application ID, one expected launcher,
  no debuggable/test-only flag, no instrumentation and only allowlisted permissions. The APK and
  AAB must define the same class set and three production classes, preventing a vacuous pass.
- **Static guard.** `check_theatre_architecture` now requires the production host as an included
  module whose only local project edge is `:shared`. It scans its plain `main` source set with the
  shared boundary. It also rejects lowercase sandbox factories such as `mockAppContainer`, which
  the previous symbol pattern missed in shared and host code.

| Layer / claim | Status | Executed evidence and scope |
| --- | --- | --- |
| Release classpaths | PASS | Both host release classpaths resolve to local projects `:androidProductionApp` and `:shared` plus external AndroidX, JetBrains Compose and Kotlin libraries and their transitive `org.jspecify` and Guava `listenablefuture`; no sandbox, demo host, test, mock or Compose tooling component. |
| Release APK/AAB bytes | PASS | `cargo xtask theatre-host android-release`: unsigned APK SHA-256 `3f4507f6a6d20a1a48bf9e5a3e1349537a6e06658cbb198661904d41338d19bd` (130 entries) and AAB `984a4062502e33337aae74555d0765445bd4f986275e5e1b9824697dd86a69e0` (136 entries) each define the same 25,984 classes, with zero findings against 69 sandbox classes and three sandbox resource files. The rebuilt APK was byte-identical. |
| Manifest | PASS | Package `com.cantos.theatre`, minSdk 23/targetSdk 35, one launcher `com.cantos.theatre.app.TheatreActivity`, no debuggable/test-only flag. The only permission is the androidx.core `com.cantos.theatre.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION` signature permission; library components are Compose resources, AndroidX startup and profile installer. |
| Gradle boundary sensitivity | PASS | A process-local ignored init script adding `:preview-sandbox` to the host makes `assertReleaseDependencyBoundary` fail for both release classpaths before compilation. No project file was edited. |
| Artifact inspector sensitivity | PASS | With that injection and the boundary task explicitly excluded, the resulting APK/AAB (26,147 classes each) produce 272 findings and exit 1: all 69 inventory classes, sandbox R and resource-accessor classes, transitive `ui-tooling-preview`, the foreign Compose resource package and the three resource files in each artifact. Clean outputs were then rebuilt and reinspected. |
| Inspector and policy unit tests | PASS | `scripts/test_inspect_android_release.py`: 11 tests on synthetic DEX/ZIP bytes and recorded aapt2 output in both formats, under Python 3.9.6 and 3.14. Eleven hand mutants of the inspector (referenced-type listing, ignored inventory/test namespaces/resource bytes/foreign package, any permission, debuggable, launcher, empty inventory, APK/AAB mismatch, required classes) are each killed. |
| Host static contract | PASS | `scripts/test_theatre_android_production_host.py`: four manifest, theme-token, build and import tests. |
| Architecture guard | PASS | 32 Theatre guard tests. The new host and lowercase-factory cases produce 13 failures against the previous checker, so they are not vacuous. |
| xtask command | PASS | Unit tests cover exact Gradle tasks, inspector arguments, missing host/SDK/inventory refusal, missing tools and no inspection after a failed build. |
| Shared presentation | PASS | `cargo xtask theatre-demo verify`: shared 125 and sandbox 213 desktop tests, zero failures/errors/skips. Two new tests check localized heading/body semantics, absence of shell/player/demo nodes, and a 16-capture layout matrix. |
| Demo host | PASS | `cargo xtask theatre-demo android-build` still packages the demo debug APK. |
| Android runtime (emulator) | PASS, bounded | A locally debug-key-signed copy of the inspected APK ran on the `Pixel_7` AVD: Android 17 (API 37) `sdk_gphone16k_arm64`, 1080×2400 at 420 dpi (411×914 dp). Observations covered cold launch without a crash, no `DEBUGGABLE` flag, English light, Vietnamese dark through per-app locale, Vietnamese at font scale 2.0 and landscape (914×411 dp). The app was then uninstalled and the emulator stopped. |
| TalkBack, physical device, process death, launcher icon | NOT_RUN | No screen-reader walk, physical device, OS process kill or Home/drawer observation was performed. A connected physical phone was deliberately not used. |
| Signed/shrunk production release, store distribution | NOT_RUN | No production signing, R8 configuration or distribution exists; these are separate #14 release decisions. |
| iOS production host | NOT_RUN | #122 concerns Android APK/AAB; Kotlin/Native 2.2.20 is still absent (2.4.20 is installed), so iOS linking remains `UNSUPPORTED`. |

The opened JVM captures are under ignored
`artifacts/ui-evidence/issue122-production-host/jvm/` (light/dark, vi/en, 390×844, 320×568,
844×390 and 320×568 at 2× text). Large text scrolls vertically without horizontal clipping;
the logo is decorative and the title is a heading. The opened emulator captures are under
`artifacts/ui-evidence/issue122-production-host/emulator/`:

| Observation | Capture SHA-256 |
| --- | --- |
| English, light, 411×914 dp | `3a3fdfe7d81605a5c0e45833793d2bee33b4f27182d58fc47f27b6482964d7da` |
| Vietnamese, dark, 411×914 dp | `e67f91e39554e84edf2260d459cb1817c9d7ca6f7f8d46dcdfc3a255e4527308` |
| Vietnamese, light, font scale 2.0 | `35b41bdc3fadda96ccb125b8d7da02978b23d6852613f1e0ab1bb1b33cc3931d` |
| Vietnamese, light, landscape 914×411 dp | `4521559c64bfbf68e51301ebe3bd221b9fd6e9b5f9df1e913acf5a19dad005ea` |

UI Automator dumps beside them contain exactly the localized title and body text. Logs are under
ignored `artifacts/agent-runs/issue122-production-host/`. Tools were existing installations:
Gradle 8.14.3, Microsoft JDK 17.0.19, Android SDK platform 35/build-tools 35.0.0, emulator AVD
`Pixel_7`, Rust 1.98.1 and Python 3.9.6. Nothing was installed or downloaded.

## Deferred acceptance and production impact

- **#122 production release isolation:** accepted by the follow-up above for the unsigned,
  unshrunk Android release of the production host. Rerun `cargo xtask theatre-host
  android-release` whenever the host, `:shared` or their dependencies change. When #14 adds
  production adapters, permissions, signing or R8, update the inspector allowlists deliberately.
- **Native owner:** provide already installed Kotlin/Native 2.2.20 and usable Xcode simulator SDK,
  then run `cargo xtask theatre-demo ios-build --kotlin-native-home <installed-2.2.20>`; link success
  still requires separate Android/iOS launch/lifecycle/accessibility acceptance.
- **Preview owner:** provide official Android CLI and ready Quail 2+ project, then repeat the
  recorded capture command and open its PNG/semantics. The Android compile and JVM render do not
  accept Studio layout rendering or device behavior.
- **Verification owner (unassigned):** the initial approximately 3 GiB resource deferral has
  been cleared by the user's cleanup and executed matching-input campaigns. All three approved
  stale test-oracle corrections are exercised; the full PostgreSQL socket/range/restart gate now
  passes. No successful unchanged heavy gate needs repetition. Final hygiene and reused gate
  provenance remain explicit in the PR; a preflight with deliberate reuse skips is not an
  unskipped aggregate PASS. Production/native acceptance above is still incomplete, and combined
  integration with concurrently merged Studio PR #166 has not been verified. No gate is relaxed
  or remote CI substituted.

No schema/migration, Script IR/API DTO, publication/cache/audio revision, rights/approval policy
or provider cost changes. No new persistence, path provider, SQLite engine, filesystem or HTTP
mock consumer is introduced; their traversal/isolation/crash gates remain with #16/#17 at adoption.
[ADR 0026](../decisions/0026-theatre-mobile-local-storage-sandbox.md) remains proposed. Production
native listening stays unsupported: the production host states this explicitly rather than
shipping incomplete behavior.
