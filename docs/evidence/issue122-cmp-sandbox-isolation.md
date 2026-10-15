# CMP preview sandbox module isolation — issue 122

This change separates the preview build graph from the shared application code. It is part of
[#122](https://github.com/loveoverflowcom/cantos/issues/122), based on
`develop@84ebda4f30fbba271ca8bbc68839f69e37afa646`, including merged #163 and #164.
**Production Android release acceptance remains BLOCKED / NOT_RUN:** this repository has a
demo Android host, not a production Android host. Keep the issue open until that acceptance is
completed. A demo APK, library graph or R8 removal cannot establish production APK/AAB exclusion.

## Ownership and compatibility

```text
preview-sandbox (KMP) -> shared (KMP)
Android demo         -> shared + preview-sandbox
desktop/iOS demo     -> preview-sandbox -> shared
future production host -> shared + production adapters (not implemented here)
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

A third `cargo xtask verify postgres` run passes **all 242 Rust PostgreSQL cases**, including
all 21 audio-intake cases and the three previously failing reads. The overall gate still reports
**FAIL**: the first real-socket audio recovery probe stops at the unchanged
`scripts/test_postgres.py:593` cache assertion before the restart cycle. The script still expects
plain `no-store` for private byte reads. Its SHA-256 matches fetched develop:
`28396e37f6bb338ade95d7d467eee3f5cc11df3c1949c03aa13872b9e1874ef1`.
The same Python helper also checks a read-access 303 redirect, whose expected policy correctly
remains plain `no-store`; replacing every expectation with the private-byte policy is incorrect.
Full output is retained as `postgres-approved-cache-helper.log`; affected Rust checks are in
`rust-approved-cache-helper.log`. The runner stopped its own hosts and disposable cluster.

The remaining Python correction is outside the approved Rust patch and is not applied. A
read-only audit covers the gate's remaining cache oracles before proposing the complete bounded
test-only change. Existing 200/206/416 byte-read assertions must keep the exact private policy,
while redirects, JSON, PUT and early-authentication expectations retain their appropriate policy.
No remaining socket/range/restart acceptance is inferred from passing in-process cases.
Disk remains approximately 19 GiB free; no SDK/tool was installed or old evidence/cache deleted.

Independent source inspection still finds only the Android `.demo` app, whose unconditional
sandbox dependency and mock composition root predate this PR. There is no production host or
xtask production release command. Shared library release graphs cannot clear that acceptance.
The exact Kotlin/Native 2.2.20 and official Android preview CLI prerequisites remain unavailable.

## Deferred acceptance and production impact

- **#122 / production host owner:** adopt a real production host under #14, then add its exact
  release command to xtask. Inspect that host's `releaseCompileClasspath` and
  `releaseRuntimeClasspath`, build its actual release APK/AAB, and inspect DEX/classes, resources
  and manifest for sandbox classes, fixtures, test libraries, controls and permissions. The host
  and reproducible command are unavailable today; assignment is required before acceptance.
- **Native owner:** provide already installed Kotlin/Native 2.2.20 and usable Xcode simulator SDK,
  then run `cargo xtask theatre-demo ios-build --kotlin-native-home <installed-2.2.20>`; link success
  still requires separate Android/iOS launch/lifecycle/accessibility acceptance.
- **Preview owner:** provide official Android CLI and ready Quail 2+ project, then repeat the
  recorded capture command and open its PNG/semantics. The Android compile and JVM render do not
  accept Studio layout rendering or device behavior.
- **Verification owner (unassigned):** the initial approximately 3 GiB resource deferral has
  been cleared by the user's cleanup and the resumed campaign above. The PostgreSQL gate now
  passes all 242 in-process PostgreSQL cases after both approved Rust test repairs, but still
  fails at the inherited Python socket cache oracle. Range/restart phases beyond that probe
  remain `NOT_RUN`. Audit the complete remaining same-contract helper scope, obtain separate
  approval for the bounded Python test-only change, then rerun `cargo xtask verify postgres`
  on a new disposable cluster. No successful unchanged heavy gate needs repetition. No gate is
  relaxed or remote CI substituted; assignment is required before accepting the remaining scope.

No schema/migration, Script IR/API DTO, publication/cache/audio revision, rights/approval policy
or provider cost changes. No new persistence, path provider, SQLite engine, filesystem or HTTP
mock consumer is introduced; their traversal/isolation/crash gates remain with #16/#17 at adoption.
[ADR 0026](../decisions/0026-theatre-mobile-local-storage-sandbox.md) remains proposed. Production
native listening stays unsupported, and incomplete behavior remains outside production hosts.
