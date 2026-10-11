# Two UI-system development previews

Status: local implementation prepared for review. No component has new two-renderer
qualification; rendered runtime/accessibility/reference QA remains blocked or NOT_RUN.
No publication, merge, production adoption or mobile acceptance is granted by this record.

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
Accessibility: real keyboard/focus/semantics observations NOT_RUN; screen-reader walkthrough NOT_RUN.
Visual: supplied static reference pixels/CSS inspected; implemented runtime pixels NOT_RUN; no fidelity approval claimed.
Motion: no new continuous animation; reduced-motion token projection example-tested, OS adapter/recording NOT_RUN.
Renderer / device: pinned Web/desktop compiler checks recorded above; Android/iOS device-tested NOT_RUN.
Async races / input / media: no remote port/media/provider; local controlled state and repeated input only.
Residual risk: actual same-scenario runtime/reference comparison, both runtime target evidence, desktop/native font fallback,
platform-specific browser versus native semantics, mobile/assistive-technology scope and full aggregate gates.
