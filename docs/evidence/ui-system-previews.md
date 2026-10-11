# Two UI-system development previews

Status: published in draft PR41 and reconciled locally with PR39/40; Mac review and partial
runtime verification are recorded below. Required CMP verification remains UNSUPPORTED.
No component has new two-renderer qualification or production/mobile acceptance.

## Request and boundary

The user requested two preview apps **inside UI System**, for Leptos and CMP, to declare/display
components and named tokens, test each against the design before real use, and update repository
skills so new components must be registered/tested in both. The existing Leptos gallery is evolved;
CMP gains a separate common Compose library with a JVM desktop diagnostic consumer. Studio/Theatre
business flows, backend/provider/audio behavior and reserved mobile apps are outside this slice.

Base: develop `209e9e905c680e7cecab831f2d8f7894656e14fe`, separate dot-cloud checkout.
The canonical token source v0.2.0 is unchanged (`e9713d5ad276be53bf3a63b6ce72ad04d1dbf0b5918cbb759a39ea7c2c1f39a2`
canonical JSON SHA256). Generated CSS/Rust remains compatible; native logical units and token/icon
metadata now come from the same validated source, not another palette.

## Invariant and ownership ledger

| Invariant | Failure mode | Oracle / owner |
| --- | --- | --- |
| Every public component has a stable declaration and both actual preview samples or an explicit unsupported reason | Metadata-only/fake parity hides an absent renderer | Shared catalogue inventory + renderer compile/semantics tests |
| Tokens retain semantic names, theme/density/alias values and native logical units | Palette/unit drift | Token generator drift/contrast/unit examples + both runtime inspections |
| A new app import requires both fresh qualification records | Source or static-image evidence silently treated as design acceptance | Exact baseline/new-import checker; runtime/accessibility/visual records remain reviewer-owned |
| Filters do not lose retained Leptos state or hide the focused navigation trigger | Discovery loses user input/focus | Native hidden structure tests + browser interaction scenario |
| Disabled/loading native controls cannot dispatch and retain visible reasons | Duplicate callback or invisible block | Pure projection tests + desktop semantics tests |
| Preview commands start no backend/provider/database | Diagnostic run changes product state | xtask pure command/prerequisite/supervision tests |

No durable domain decision is extracted: these are presentation fixtures, component contracts and
local preview navigation. The pure functional cores are declaration/qualification validation,
state-to-availability projection and deterministic sample/filter selection.

## Implemented

- One JSON component/state catalogue, 29 public APIs, generated Rust/Kotlin registrations.
- Searchable Leptos component/family/state and named-token panels; retained mounted samples,
  existing zero/IME/dialog fixtures, preview-only styles and offline official icons.
- Separate Kotlin Multiplatform/Compose desktop preview, reusable controlled native wrappers,
  token/theme/density/text-scale inspection and explicit browser/native parity notes.
- Generated native token/icon mappings; same semantic colors, dp/sp logical geometry, no WebView.
- `cargo xtask ui-preview leptos|cmp build|verify|run`; Leptos local port8083; installed pinned
  Gradle/Java prerequisites; dedicated CMP gate selected independently of Cargo package metadata.
- Repository drift/inventory/new-adoption guard, qualification skeleton, and skills requiring
  both previews plus honest runtime/accessibility/reference QA before a new real app import.

Exact existing Web import sites form a compatibility baseline only. There are no qualification
records in the new catalogue, so no new native component or app import is called approved.

## Verification, 2026-10-11

| Check | Outcome / evidence level |
| --- | --- |
| `cargo xtask verify repository` | PASS; 162 Python examples, 313 text files / 15 skills, generation/links/JSON/whitespace |
| `cargo xtask verify rust`, pinned Rust1.87 | PASS; whole-workspace rustfmt, warnings-denied Clippy and tests |
| `cargo xtask ui-preview leptos verify` | PASS; 42 native package tests (9 gallery), WASM Clippy and actual Trunk build |
| `cargo xtask verify contracts` | PASS; Script IR oracle, wire/receipt tests and independent WAV reference |
| `cargo xtask verify web` | PASS; three WASM consumers linted and actual Studio/Theatre/gallery Trunk builds; no runtime implication |
| Token generator tests | PASS25; native logical units, alias/density/reduced-motion mapping |
| Registry/adoption tests | PASS15; declarations/samples/aliases, source/build/state freshness, unavailable evidence and unsupported cases |
| Icon generator tests | PASS3; licensed official SVG hashes and native visible paths |
| Supplemental Kotlin2.4.20 core/mapping JUnit | PASS9; compiled/example-tested, distinct from pinned Compose tests |
| `cargo xtask ui-preview cmp build` | PASS; actual final common/JVM compilation with Kotlin2.2.20, Compose1.9.0, stable Material3 1.8.2 and Gradle8.14.3 |
| `cargo xtask ui-preview cmp verify` | FAIL before pinned test source compilation/execution: Temurin17 test dependency resolution rejected the environment proxy certificate (`certificate_unknown` / PKIX). All 17 declared tests NOT_RUN |
| Actual gallery browser inspection | BLOCKED / NOT_RUN: local Chromium launch requires unavailable Unix-socket operations; supported cloud browser cannot reach the isolated loopback server |
| CMP window / Compose semantics/captures | NOT_RUN; no renderer/window pixels or keyboard/accessibility observations |
| `cargo xtask preflight --changed-only` | UNSUPPORTED: missing PostgreSQL17 `initdb`, `pg_ctl`, `psql`; prerequisite check correctly starts no selected gate. Separate scoped PASS results above are not full preflight acceptance |

Rust1.87, Trunk0.21.14, Gradle8.14.3 and a full official Temurin17 JDK were materialized for this
requested slice. Main CMP compilation succeeded under the existing host Java21/default trust;
that directory lacks `javac`. The full JDK fixed compiler capability but could not resolve the
pinned test artifacts under its normal trust configuration. No custom truststore, added CA,
TLS bypass, network/security change or denied-browser workaround was applied. Mobile SDKs,
PostgreSQL, models/audio providers, paid calls and production services were not provisioned.

The exec server changed session keys during the final checks, invalidating old sessions and
scratch `/tmp` logs. Existing PASS output was observed, but all available source gates were
rerun with persistent task-workspace logs. This technical reset does not produce runtime QA.

### Persistent local artifacts

The separate checkout is `cantos-ui-previews`, branch `feat/ui-system-preview-apps`, still based
on develop `209e9e905c680e7cecab831f2d8f7894656e14fe`; final `git fetch origin develop` found the
same SHA. At the pre-publication handoff it contained only owned, uncommitted local changes;
no publication had occurred.

The requester subsequently authorized committing this scoped slice, pushing
`feat/ui-system-preview-apps` and opening a **draft** PR to develop to preserve the work.
GitHub owns the publication status; this authorization does not include merging or component
qualification. The checks above describe the reviewed source before that publication step.

Task folders next to the checkout contain reproducible evidence without generated binaries in Git:

- `cantos-ui-preview-evidence/`: canonical scoped gate logs, preflight prerequisite result and status ledger
- `cantos-cmp-validation/cmp-build-handoff.log`: final pinned source build; `cmp-verify-jdk17-retry.log`: exact blocked pinned verification; `core-tests.log`: supplemental nine examples
- `cantos-reference-inspection/`: three supplied PNG boards, their hashes and inspection report

An independent read-only admission review checked the exact existing import baseline, found
namespace/comment-alias gaps, missing state/build-input freshness and metadata-only sample
false positives. Each was corrected with negative regressions. The checker strips comments and
opaque strings, requires native enum ID/dispatch plus actual wrapper invocation (one explicit
retained Dialog helper), and invalidates source qualifications for declaration or build changes.
It remains a conservative source/evidence checker, not a full language compiler, authenticated
reviewer identity or pixel comparison service. Indirect re-exports remain a manual review gate.

## Inspected design evidence

Three actual supplied static boards were opened: Cantos Web (`11c_qhgO4kxUCbpUCOo5i36tI8cfWe1t0`),
Studio writing desk (`1twcjWY4wjGrdiFRqxARaJbWOPOYT3SnC`) and screenplay
(`1ZpVOJNbwvTW6G1DdHobFNrxAKC-Kq-Qv`). Corresponding inert HTML CSS/font metadata was inspected.
All 40 semantic color comparisons (20 roles × two themes) match canonical tokens; Noto Sans /
Noto Serif family metadata agrees. Observed paper/lavender surfaces, editorial type, capsule
controls and quiet grouping ground the diagnostic styling. Latest filled-tonal input direction
is retained rather than restoring earlier outlined controls.

These are source/static-reference findings. They are not screenshots of either implemented
preview. No corresponding native, dark/narrow gallery, 200% text or interaction reference exists
in that packet, and no same-scenario pixel-fidelity PASS is claimed. Both previews provide actual
named visual token specimens in source; their runtime appearance still needs inspection. CMP
explicitly uses platform sans/serif fallback rather than bundled Noto font metrics.

## CMP reuse and next acceptance boundary

Coverage is **29 compiled common-source wrapper candidates, 29 declared native examples,
zero two-renderer production qualifications**. Existing Leptos consumers remain a bounded
compatibility baseline; neither their earlier Safari evidence nor native build success approves
new native application use. Tests that have not run cannot be used to promote these candidates.

A subsequently authorized CMP sequence can reuse the canonical Kotlin token/icon mappings,
shared catalogue, controlled wrapper contracts, desktop fixtures and 17 declared tests. It must
review/fix native API and accessibility/font/platform gaps, execute the pinned tests and actual
renderers, inspect corresponding reference scenarios and record fresh two-renderer evidence
before adoption. The later Ultra → xhigh → Ultra work is queued separately; this slice does not
start it, implement either mobile product or duplicate the separately reviewed Compose capture
adapter in draft PR39.

## UI report

Screen / flow / consumers: developer-only component/token catalogue; Leptos Web and CMP JVM.
Surface intensity: low/medium diagnostics, quiet paper/lavender surfaces, no section-border revival.
Tokens: canonical v0.2.0; no semantic role/value edits; both generated mappings and existing contrast checks.
Localization: developer diagnostic strings are the documented exception; realistic vi-VN fixtures,
semantic API/token identifiers and English implementation-status reasons. Production locale resources unchanged.
Accessibility: bounded Mac Safari interaction/focus observations below; CMP and screen-reader walkthrough NOT_RUN.
Visual: supplied static reference pixels/CSS and Mac Safari runtime pixels inspected; no two-renderer fidelity approval claimed.
Motion: no new continuous animation; reduced-motion token projection example-tested, OS adapter/recording NOT_RUN.
Renderer / device: pinned Web/desktop compiler checks recorded above; Android/iOS device-tested NOT_RUN.
Async races / input / media: no remote port/media/provider; local controlled state and repeated input only.
Residual risk: actual same-scenario runtime/reference comparison, both runtime target evidence, desktop/native font fallback,
platform-specific browser versus native semantics, mobile/assistive-technology scope and full aggregate gates.

## Mac reconciliation and review, 2026-10-11

[PR41](https://github.com/loveoverflowcom/cantos/pull/41) owns execution status. The requester
authorized a fresh Mac review, fixes, push and normal merge after suitable local verification;
the later three CMP sessions remain separate. No force push, branch-protection bypass or
remote-CI substitute is authorized by this evidence.

The clean Mac checkout was inspected before switching branches. The published PR head was
`dc6928d555014f2170f7f77bc672abc635580ac8` (tree
`7415e4aea6c7466928046d5ac422c12057a626bc`). Fresh fetch and remote-ref inspection confirmed
develop `212e2d981bca7bb14843db159979eccbb0a92430` (tree
`3c202cabcb6d7551249d56ce1f0411117720dcd6`). A normal local merge preserves PR39's optional
Compose capture adapter and PR40's durable-job implementation. Only the combined CMP skill
paragraph and Tool inventory required conflict resolution; existing backend/contracts/capture
paths match the fetched develop tree. Checks below ran on this merged source with review fixes
and an uncommitted merge; final Git identity belongs to the PR rather than a self-referential doc.

### Review corrections

- Source admission now normalizes legal Rust/Kotlin namespace syntax, handles Rust character
  literals without erasing lifetimes, rejects hidden nested/grouped namespace imports and Cargo
  dependency renames, and reads executable Kotlin template expressions separately from inert
  strings/comments, including valid triple-string closing quote runs. Conservative unsupported syntax still requires manual review; this is not
  a complete compiler or a defense against arbitrary indirect re-exports/generated source.
- Qualification digests exclude only the exact generated catalogue files and package-root
  build outputs. Handwritten files with a generated basename, nested directories named `build`,
  checkout ancestors with that name and CMP TOML version catalogues remain effective inputs.
- The CMP picker and detail use the same effective filtered selection, including empty results.
  Select puts its label/value and expanded/collapsed state on the actual interactive node and
  discards an open menu when availability becomes disabled/loading. Filled tonal text fields,
  danger colors and readable disabled text use canonical semantic roles.
- CMP `run` supplies the owned POSIX launcher PID through a fixed exec chain, aligns task-local
  JVM options, requests in-process Kotlin compilation and disables configuration caching.
  A configuration guard rejects absent, invalid or detached JVM identity before run-task work.
  Command construction is example-tested; supported Gradle execution and whole-process shutdown
  remain NOT_RUN. `--no-daemon` alone is not proof of ownership or cleanup.

Independent reviewers inspected admission, Web behavior, CMP semantics and preview process
ownership. A subsequent independent source pass found the Select interruption and inert-template
false positive; both have narrow regressions. No component qualification record was added.

### Mac command evidence

Working directory: repository root. Logs and screenshots are ignored task artifacts under
`target/pr41-mac-evidence/`; no generated binary or private data is committed. All Cargo checks
use the existing stable toolchain path, whose measured compiler/Cargo/Clippy/rustfmt are Rust
1.87.0, with `CARGO_BUILD_JOBS=2` and `TMPDIR=/private/tmp`. This avoids downloading a duplicate
named toolchain. Existing Trunk is 0.21.14 and local PostgreSQL is 17.4.

| Command / check | Actual result | Local artifact |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS; 192 Python tests, 319 text files / 15 skills, generation/links/JSON/whitespace | `repository-final.log` |
| `cargo xtask verify rust` after xtask fixes | PASS; format, warnings-denied workspace lint/tests, including 84 xtask examples | `rust-final.log` |
| `cargo xtask verify contracts` | PASS; reference, contract tests and WAV reference | `contracts.log` |
| `cargo xtask verify web` | PASS; WASM lint and Studio/Theatre/gallery builds | `web.log` |
| `cargo xtask ui-preview leptos verify` | PASS; 42 native package tests, WASM lint and actual Trunk build | `leptos-verify-final.log` |
| `cargo xtask verify postgres` | PASS; new disposable PostgreSQL cluster, actual HTTP/CLI/restart/replay checks and durable-job tests | `postgres.log` |
| Focused admission regression suite | PASS; 25 tests, including failing-first parser/digest/alias cases | Included in final repository suite |
| `cargo xtask verify mutation` | PASS; 15-test baseline and 9/9 mutants killed, zero survivors/timeouts/unviable | `mutation-final.log` |
| `cargo xtask preflight --changed-only` | UNSUPPORTED: required Gradle8.14.3 missing; every other selected gate NOT_RUN in this aggregate invocation | `preflight-final.log` |
| CMP final compile / 23 declared tests / desktop window / lifecycle | NOT_RUN; supported Gradle unavailable | No passing artifact |

The Mac has a full existing JDK21 and Android Studio's JBR21, but only Gradle9.7.0 is installed;
that version is outside the reviewed command contract. The pinned8.14.3 prerequisite is kept.
No Gradle/JDK/SDK download, TLS bypass, custom truststore, CA import or global configuration
change was made. The scoped PASS checks do not turn the aggregate UNSUPPORTED result into PASS.
Merge remains blocked until the required CMP gate can run; approval to merge is not permission
to skip it. Remote CI was not awaited or polled.

### Actual Safari observations

`cargo xtask ui-preview leptos run` launched only its owned loopback preview at port8083.
An unlocked Mac Safari window displayed the actual WASM gallery. These are bounded observations,
not a blanket accessibility or component qualification:

After inspection the task's Safari tab was closed and the owned xtask process interrupted.
The supervisor logged stopping Trunk PID88296; xtask PID88243 and that child were absent in a
subsequent process inspection. This establishes the observed Leptos cleanup only.

| Scenario | Observed behavior / evidence |
| --- | --- |
| Component discovery / retained input | Search shows 1/29 TextField and 2/29 Button; filtering retains pasted `Nguyễn Thị Minh Phương · Ánh đèn cuối sân khấu · Ỗ, Ữ` and raw numeric0. `leptos-filter-textfield.ax.txt`, `leptos-filter-button.ax.txt` |
| Availability | One enabled click increments once; disabled click does not increment; disabled/loading reasons and native disabled semantics remain visible. `leptos-disabled-loading.ax.txt`, `leptos-loading-light.png` |
| Theme / density | Actual dark palette and comfortable density selected through native controls. `leptos-wide-light.png`, `leptos-wide-dark.png` |
| Named token discovery | Search `color.primary` shows2/73; expanded specimen exposes canonical CSS name, light `#675080`, dark `#d9bdf1` and density fallback. `leptos-token-primary.ax.txt`, `leptos-token-primary.png` |
| Dialog | Opening focuses close; editing then Escape from the focused input closes and returns focus to opener; reopening retains Vietnamese text. `dialog-escape-from-input.ax.txt`, `dialog-retained-reopen.ax.txt`, `dialog-keyboard-dark.png` |
| Responsive / text scaling | Task-local static iframe harnesses constrain the real gallery to320/600 CSS pixels. Top-level heading/control wrapping was inspected; no full-page overflow claim. Safari Page Menu explicitly reports200% zoom and the visible controls reflow. `leptos-320-light.png`, `leptos-600-light.png`, `leptos-200-percent.ax.txt`, `leptos-200-percent-light.png` |

Full Tab/Shift+Tab containment could not be established reliably from Safari's native AX focus
projection; plain and Option-Tab walks were saved but are not a PASS. VoiceOver, OS Vietnamese IME,
reduced-motion/forced-colors, full-page narrow/dark token inspection and every component/state
remain NOT_RUN or incomplete. Pasting preserves accents; it does not test composition events.
Local reference HTML/CSS/source maps were inspected without live Drive changes. No corresponding
CMP pixels exist here, so neither runtime appearance nor browser observations grant parity.

| Qualification boundary | Status |
| --- | --- |
| Leptos structural/build | PASS |
| Leptos bounded Safari interaction/visual inspection | Observed; full accessibility/reference matrix incomplete |
| CMP final structural/build/semantics/capture/runtime | NOT_RUN |
| Two-renderer production-qualified components | 0 / 29 |
| Studio/Theatre mobile acceptance | NOT_RUN; product Gradle apps remain guides |

Next order: obtain the reviewed local Gradle capability under the requester's download constraint,
run the unchanged CMP and aggregate gates, then perform actual desktop interaction/accessibility/
visual and owned-process shutdown checks. The later CMP Ultra → xhigh → Ultra sequence must start
from its own freshly inspected SHA and cannot inherit qualification from this partial record.
