# CMP UI-system session 1

Status: bounded foundation implementation and local handoff; development preview only.
This is session 1 of the requester-authorized Mac mini Ultra → Extra High → Ultra sequence.
Base/fetched develop: `7f416a1e1bac5bbf338d461463770bffd983b5c1`, clean before edits;
base tree `0c6681bab247eb527560481400100af0fdf7d09e`. Local branch:
`feat/cmp-foundation-parity`. No push, PR, merge or next workchat was started.
The [queue](../work-plan/016-cmp-ui-system.md) owns subsequent order.

## Invariant and ownership

Native component state remains caller-controlled; scoped theme/geometry and semantic descriptions
must not silently change input, duplicate dispatch or imply production qualification.

| Claim / failure mode | Owner / boundary | Cheapest oracle / evidence held |
| --- | --- | --- |
| Nested themes replace preview textScale, retain platform fontScale/display density and restore sibling scopes; otherwise 2× becomes 4× | CantosTheme and validated UiPreferences, no product state | Three real Compose composition probes plus three common constructor/copy tests |
| Names/reasons/availability are reusable, not coupled to diagnostic discovery | ComponentState; PreviewModel retains registry/filter/selection only | Source dependency review and existing projection tests |
| Actions retain one named native node with guarded dispatch; caller root geometry stays compatible | Actions; explicit controlModifier and Button RowScope slot | Production wrapper focus/callback/loading semantics and all six composed-preview label-in-name assertions |
| Text/selection/composition are caller-owned; readonly selects but does not edit; blocked fields cannot submit custom IME actions | Fields String and TextFieldValue overloads; no normalization/parser | Common permission/message examples; rendered wrapper selection/edit/IME-action tests |
| Blank errors do not create invisible invalid state | One FieldMessages projection shared by all text overloads | Null/empty/whitespace examples and real Compose error/state-description assertions; existing Web has equivalent behavior |
| Focus geometry uses native field shape and token width/offset; icons use their content foreground | ComponentModifiers / Icon renderer adapters | Requested native focus + opened state captures; actual Noto/platform/OS claims deferred |
| New scenarios have both samples and do not widen app admission | Shared catalogue/generator and diagnostic consumers | Repository inventory/drift/adoption checks; qualification remains empty |

No durable domain decision is extracted. The functional core is the small availability,
readonly-value and visible-message projection; thin renderer adapters use existing Material APIs.
No new crate, dependency, app route, palette or product permission is added.

## Implemented slice and compatibility

Reusable source now has explicit theme/state/actions/text-field/focus owners within the same public
package. Remaining choice/navigation/layout/feedback wrappers stay in Components for session 2.
String signatures and existing Leptos consumers are retained. Button slot content requires a
localized AccessibleName containing the complete visible label; `modifier` still owns the whole
field and `controlModifier` owns its native interactive node. Icon inherits foreground/tint.

All text wrappers expose native KeyboardOptions/KeyboardActions. TextArea also exposes ordered
positive min/max line limits. The rich overload forwards TextFieldValue without flattening
selection/composition; readonly allows selection-only changes, and custom keyboard callbacks run
only while editable. Callers own committing/cancelling an active IME composition before locking
input. Native decimal keyboard hints do not validate numbers; raw zero, empty, minus, incomplete
comma decimals and Vietnamese text remain unchanged.

The [official TextFieldValue source](https://github.com/androidx/androidx/blob/androidx-main/compose/ui/ui-text/src/commonMain/kotlin/androidx/compose/ui/text/input/TextFieldValue.kt)
describes selection/composition snapshots and copying rather than inadvertently clearing composition.
The [official KeyboardActions source](https://github.com/androidx/androidx/blob/androidx-main/compose/foundation/foundation/src/commonMain/kotlin/androidx/compose/foundation/text/KeyboardActions.kt)
describes custom/default keyboard actions. These references inform the adapter; actual compatibility
was compiled/tested against the unchanged pinned artifacts, not inferred from current Android docs.

Registry: unchanged 29 API names, 118 → 129 concrete scenarios. Added `nested-theme`,
`composed-content`, and `selection` / `ime-action` / `blank-error` for each of the three text wrappers.
The Leptos gallery uses existing APIs plus app-composed native-control specimens; HTML number
selection restrictions and min/max/step are explicitly different from native raw decimal input.
Canonical v0.2.0 tokens remain 73 roles and retain JSON digest
`e9713d5ad276be53bf3a63b6ce72ad04d1dbf0b5918cbb759a39ea7c2c1f39a2`.
No qualification or compatibility-baseline import is added.

## Actual local evidence

All checks run from the repository root with Rust 1.87.0, Cargo 2 jobs and `TMPDIR=/private/tmp`.
CMP uses task-local PATH with existing official Gradle8.14.3 and full Microsoft JDK21.0.7+6,
Kotlin/compiler2.2.20, Compose1.9.0 and stable Material3 1.8.2. Rust1.87 was absent from the default
rustup installation in this execution environment and was installed with rustfmt/clippy/WASM
through official rustup. No existing toolchain/Gradle9.7/cache, TLS trust or OS setting was changed.
Measured before heavy work: 43GiB free disk and memory_pressure 86% system-wide free.

Ignored evidence is under `target/cmp-session1-evidence/`; no binaries/captures enter Git.

| Command / check | Actual result | Artifact |
| --- | --- | --- |
| `cargo xtask ui-preview cmp verify` final | PASS, 41 executed; 0 failures/errors/skips: 15 common, 19 desktop semantics/composition, 7 capture test methods | `cmp-verify-final.log`, `cmp-tests-final.json` |
| `cargo xtask ui-preview leptos verify` final | PASS: 46 native package tests (13 gallery), warnings-denied WASM lint and actual Trunk build | `leptos-verify-final.log` |
| `cargo fmt --all -- --check` pinned1.87 | PASS; Kotlin manually reviewed, no Kotlin formatter/lint gate configured | Tool output in session |
| `cargo xtask ui-preview leptos run` | Actual Safari native input and opened light/dark pixels below | `leptos-runtime.log`; computer-use tool observations in session |
| `cargo xtask ui-preview cmp run` | Launch reaches unchanged foreground PID guard/run; OS input BLOCKED by app binding | `cmp-runtime.log`, `runtime-observations.json` |
| Repository, Rust/contracts/Web and changed-only preflight | Final outcomes recorded in completion below | Named final logs |

The first Leptos run failed a new test's brittle attribute-order assertion. The test now bounds
the exact root tag and checks each attribute independently. Its next run passed tests but failed
WASM lint for a browser-only missing Get import; that cfg-scoped import was fixed. Both failed
logs are preserved; final verify passes. The repository scanner initially matched rich-field
initializer enum branches before the actual sample dispatch. An equivalent if/else initializer
keeps the existing admission scanner unambiguous; no checker or gate was weakened. Its recheck
passed192 Python tests. Initial CMP37 then40 runs passed before the independent
review regression brought the final suite to41. Only executed reports are counted; later
UP-TO-DATE results are not another execution. Theme/error regression assertions were added with
the implementation rather than running a separate BASE renderer build; no failing-first runtime
claim is made for those bugs.

## Preview observations and review

The primary reviewer opened all eleven current JVM PNGs in `captures/`; the independent reviewer
opened all seven new state PNGs. Renderer roots are explicitly 1140×840 or 372×840, density1;
Vietnamese fixture copy is developer-only. New captures show all five action foreground/icon pairs
in light/dark, rich text/textarea/number focus, blank-error comfortable dark and nested-theme 2×.
The narrow single-line field scrolls text horizontally by native design; no clipped visible stacked
marks were seen. Nested 2× child text stays at 2× rather than compounding.

| Finding | Responsible fix | Same-scenario recheck |
| --- | --- | --- |
| Outset focus ring touched specimen caption/selection help | Preview fixture spacing (md caption, sm field group), no input/hit-area shrink | Same dimensions/theme/text/field states rerendered and opened; clear gap |
| P2 composed sample name omitted complete visible label | Six accessible names begin with visible labels; new actual ComponentExample semantics test | Final41 tests PASS; rendered appearance retained |

Independent `cantos-code-review` read the full core/source/test diff against BASE and seven new
captures. It reported `no-actionable-findings` after the label correction. Its pre-final source
snapshot was `e498e9b986341ef902835f5ffc7c4ce35927f3dd8392ac134555abbc9d49c8ee`;
the final gate executes the added regression. Documentation/aggregate completion is checked separately.

Safari native computer-use observations on the live local gallery:

- Composed Button pointer callback incremented0→1. Disabled reason remained visible, named button
  was disabled and attempted activation left count1.
- Select-all on the text specimen focused its native input. Vietnamese paste preserved
  `Người dẫn chuyện · Ỗ, Ữ`; Return incremented the diagnostic Enter counter. Textarea paste preserved
  accents and Return appended a newline without preventing native behavior.
- Readonly still selected UTF-16 range0–23; attempted paste and Return retained text and Enter count2.
- Light/dark switching retained the input and counters. Opened Safari window pixels were1621×862;
  CSS viewport/browser zoom was not instrumented. Safari18.6 (20621.3.11.11.3) is recorded from its actual Info.plist in the manual runtime record.
- Normal Tab skipped buttons in the current Safari configuration. No OS setting was changed and
  no full keyboard accessibility pass is claimed. Tool key typing dropped non-ASCII characters;
  subsequent native paste preserved them. Paste is not Telex/VNI or OS IME evidence.

CMP inventory listed MainKt / `net.java.openjdk.java`, but supported getApp calls by both returned
`Invalid app`. No Java binding workaround, security change or synthesized OS input was used.
Before interruption, xtask14333 owned launcher14376/MainKt14393 in group14376. After Ctrl-C to
that task, all three recorded PIDs were absent. This proves that cleanup run, not every crash or
window-close behavior. Leptos xtask14122/Trunk14175 were also absent after their owned Ctrl-C; the manual runtime record distinguishes each observation.

Surface/consumers: both package-local diagnostic previews only, low-intensity core specimens with
quiet tonal grouping. Localization: no production resource keys changed; synthetic vi-VN preview
labels/accessibility text remain caller-provided. Tokens: no source/value changes. Accessibility:
JVM semantics/focus and bounded Safari observations; screen-reader not walked. Motion/reduced-motion
runtime not observed. Actual complete design-reference comparison/Noto metrics remains incomplete;
canonical token agreement and inspected preview pixels are not reference qualification.

## Completion and residual risk

| Final suite | Outcome / scope | Log under ignored evidence root |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS:321 text files/15 skills,192 Python tests, generator/admission/link/whitespace checks | `repository-recheck.log` |
| `cargo xtask verify rust` | PASS:rust-format, workspace rust-lint/rust-tests; PostgreSQL-specific tests remain ordinarily ignored outside their separate gate | `rust-final.log` |
| `cargo xtask verify contracts` | PASS:script-ir-reference, contract-tests, wav-reference | `contracts-final.log` |
| `cargo xtask verify web` | PASS:web-lint, Studio/Theatre web-build and ui-preview | `web-final.log` |
| `cargo xtask preflight --changed-only` | PASS:all nine selected gates, no skip/FAIL/UNSUPPORTED; repository, format, scoped Rust tests/lint, Web lint/builds, gallery and CMP | `preflight-code-final.log`, `preflight-final.log` and local handoff record |

Current generator-renderer digests are CMP
`40c34af8757ac44b373c57789c80d43f3ea7eeba3bac3681f5cb13c9859ff001` and Leptos
`a0e3de91f83aee25eb0c183c61e8f6885de272c0e1f5d6cf266b30c3594c4d61`.
The qualification list remains empty. Local commit/branch/status and final gate output live in
the ignored handoff record and final response; no remote CI is polled or substituted.

Residual: OS CMP pointer/keyboard/focus/resize BLOCKED; native Vietnamese IME, active composition
locking, Noto resources/metrics, full responsive/reference/state/accessibility/motion matrix and
screen-reader evidence NOT_RUN. Android/iOS product targets remain UNSUPPORTED guides and mobile
runtime NOT_RUN. These gaps leave qualification at **0/29**; source/compile/JVM/Safari evidence
does not authorize new app adoption. Session 2 scope and session 3 matrix are in the queue.
