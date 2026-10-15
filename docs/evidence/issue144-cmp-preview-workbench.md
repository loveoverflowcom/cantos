# CMP preview recipes and scenario workbench

This record covers issue [#144](https://github.com/loveoverflowcom/cantos/issues/144): reopen a
declared component/state or actual Theatre composition with synthetic inputs, replay intents,
and reset/recreate owners without crossing an old mount. It is local preview evidence, not
native listening or production persistence acceptance.

## Boundary and ownership

The shared CMP gallery's Workbench tab uses `PreviewRecipe`, `PreviewSettings`,
`ComponentSampleState` and the existing `ComponentExample`. Catalog, isolated and state-matrix
views share the same components with Android `@Preview` wrappers. An open modal is inspected
in isolated mode: an immutable matrix must not trap its host behind an undismissable dialog.
Interactive focus recipes request real focus; immutable static seeds make no focus claim.
The ordinary gallery retains its filtered-selection contract and preserves edited strings
across availability changes. Workbench filtering preserves the mounted field and selection.

Theatre's `desktopMain/.../app/workbench` binds the actual `CantosDemoApp` to existing mock ports.
`DemoAppSession` extracts the existing route dispatcher and UI inputs; it creates no service
or coroutine scope. Each owned workbench mount owns one child scope, container and session.
Reset selects the recipe's Fresh/Restored seed, rather than `AppContainer`'s Demo reset default.
Replacement closes/cancels the old mount once; callbacks are fenced by mount identity and the
old session is invalidated. Borrowed containers remain externally owned and lifecycle controls
report unavailable. Appearance changes retain the same owner and composition, including pending
writes and text selection.

The workbench and its settings import are confined to desktop source sets. `commonMain` excludes
`com/cantos/ui/preview/**`; a non-vacuous source-set regression checks this bounded seam.
[#122](https://github.com/loveoverflowcom/cantos/issues/122) still owns extraction of the older
mock sandbox and production release APK/AAB dependency/resource exclusion. This change neither
claims that prerequisite accepted nor uses a mock APK as production release evidence.

## Recipes, components and snapshots

No new shared component, dependency, token or design baseline is introduced. Controls reuse
Button (text/tonal/secondary), FilterChip, stable-value Select, TextField, TextArea, Surface,
FieldLabel/Help and existing state fixtures. All registered component/state IDs remain bound
to the canonical catalog; 70 design proposals remain proposals.

The shared snapshot schema `CANTOS-CMP-PREVIEW|1` allows 256 intents and 16,384 characters.
Export encodes allowlisted synthetic text/choice IDs and checks the projection against pure
replay. Import rejects unknown recipes/versions, malformed data, mismatched projections,
out-of-range tabs and off-step slider values before rendering. IME selection/composition and
dialog sample state are values; scopes, ports and controllers cannot be encoded.

Theatre JSON v1 allows 40 scripted intents, a 65,536-character payload and bounded synthetic
navigation/progress collections. Export refuses arbitrary queries, membership information,
storage failures and unsupported transient playback states. Valid restore recreates owners
through the existing injection seam. Invalid input leaves the current mount unchanged and
never selects a live adapter. The six fixture IDs are Fresh, Restored, Empty, Loading, Error
and Offline; Offline simulates unavailable catalog/writes. The replay step interval is an
explicit diagnostic clock. Timer remains a selection, with no scheduled native timer.

## Executed evidence

Source: `develop@f265ae4184be37a4d36fc4dedee195a98141644d` plus the issue-144 diff. Kotlin 2.2.20,
Compose 1.9.0, Material3 1.8.2, installed Gradle 8.14.3, Microsoft JDK 17.0.19 on macOS arm64,
Rust 1.98.1. Commands use the installed Gradle/JDK on PATH and Android SDK at
`$HOME/Library/Android/sdk`; no installation or service provisioning occurred.

| Claim / layer | Evidence | Result and scope |
| --- | --- | --- |
| L1–L3 typed recipe, strict codec and deterministic replay | `cargo xtask ui-preview cmp verify`; `PreviewWorkbenchTest` in common tests | PASS; 10 pure recipe/codec tests, including IME, unsafe-content and render-invalid import rejection |
| L4 shared workbench and existing component interaction | same command, desktop Compose tests | PASS; 205 total tests, zero failures/skips at checkpoint 5; actual callbacks, filtering, restore/reset/recreate, real focus and modal-matrix safety |
| L4 Theatre lifecycle and fake ports | `cargo xtask theatre-demo verify` | PASS; 295 tests, zero failures/skips; virtual-time A→B→A, dispose during write/replay, stale callback, borrowed owner, deterministic script, settings preserving queued writes and rendered search selection, actual multiline paste/restore |
| Android shared recipe wrapper | `ANDROID_HOME=… cargo xtask ui-preview cmp android-build` | PASS; actual Android library compilation, not launch/device evidence |
| Android Theatre mock | `ANDROID_HOME=… cargo xtask theatre-demo android-build` | PASS; actual debug mock APK, not production release acceptance |
| Official Android Studio layout capture | `cargo xtask compose-preview ui-system-cmp --file packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/preview/AndroidPreviews.kt --composable AndroidRecipeFieldPreview --label issue-144-recipe-field --timeout-seconds 30` | UNSUPPORTED; studio-check cannot reach a ready selected Gradle project/IDE |
| iOS framework/runtime, Android runtime, screen reader | not executed | NOT_RUN; no native-runtime claim is needed to accept this preview orchestration slice |

Checkpoint logs remain in ignored `artifacts/agent-runs/issue-144/`. The final preflight summary
and final checked diff identity belong in the PR handoff. The source-boundary test includes
explicit non-vacuity assertions and runs in the Theatre suite.
Kotlin has no configured formatter/linter: compiler, tests and manual review are the available
Kotlin evidence. No Rust/domain/API/schema/migration behavior changed.

Earlier attempts are retained: an invalid test import, a gallery index-picker regression and
the new gallery availability-retention regression failed before their fixes; a Theatre compiler
import omission also failed before correction. The first optional Android attempt failed before
compilation because SDK location was unset; supplying the existing SDK resolved it. A first
capture invocation used a qualified composable name and was rejected; the corrected invocation
reported the separate Studio-readiness UNSUPPORTED result. The repository Python suite initially
selected Xcode Python 3.9.6, which lacks `tomllib` and `os.waitid`; the installed Homebrew
Python 3.14.8 supplies both and is selected explicitly for handoff. No script was changed for
that interpreter mismatch. None of these failures is relabelled as a passing attempt.

## Inspection and qualification continuity

The report-only inspector opened the three recipe frames (390 dp light field selection,
320 dp dark 2× auth button with reduced motion, 840 dp System-dark 1.3× checkbox matrix), the
1140 dp workbench controls, three existing auth/account boards, the focused auth action and
two Theatre workbench frames. Captures are JVM desktop Compose roots at their declared dp
constraints and 1 px/dp. Shared sidecars record host font scale 1 and platform fallback fonts;
Theatre retains its bundled fonts, but its sidecars do not record host font scale/readiness.
Fixture language is Vietnamese unless explicitly selected;
this is no claim of a complete locale translation.

Initial Theatre captures exposed a transparent control background with black text. An intermediate
fix used `UiRoot`; repository admission correctly rejected its new app-consumer use, along with
TextArea. The final host uses the already admitted Surface with its paired semantic roles and
TextField for snapshot transfer, with a selectable text log. The desktop package also follows the
existing `app` layer policy. No checker exception, baseline consumer or qualification scope was
added. The same Fresh-light and Restored-dark/large-text scenarios are recaptured and reviewed
separately; actual UI paste/restore covers multiline snapshot transfer. The initial finding is preserved in
`artifacts/ui-evidence/issue-144/report.md`; post-fix observations are kept under its `recheck/`
directory. Images/logs stay local and ignored; no images or private media are committed/uploaded.

The six existing Button/TextField/Surface/Badge/BackButton/Logo qualifications are rebound to this
CMP source identity, without extending their admitted scope. Their public implementation and
tokens are unchanged. Baseline and checkpoint-5 capture inventories share 67 PNG names: 59 are
byte-identical; seven gallery images changed with the new navigation tab; the focused auth action
can differ in fill tint between captures while preserving label, full outset ring and geometry.
The focus frames were opened, so no bit-identical claim is made for that frame. Existing semantics,
keyboard/focus, contrast and component suites remain passing; reviewed account boards preserve
Vietnamese text, error reasons, light/dark grouping and large-text wrapping. Qualification records
point here for the bounded source rebinding, not to a new pixel oracle or a device acceptance.

## Residual risk

Native audio, OS reduce-motion, timers, background/lock-screen controls, TalkBack/VoiceOver,
device process death, filesystem/SQLite durability and production APK/AAB exclusion remain
unaccepted. Run the documented native acceptance matrix on named devices when those production
consumers exist. Open the selected Android preview project in ready Quail 2+ and rerun the exact
capture command for optional layout-renderer evidence. No provider, network, auth or production
storage boundary was exercised; no cost-bearing provider call was made.
