# Leptos preview v3 foundation — issue 141

This is the bounded component-library/workbench slice of
[issue 141](https://github.com/loveoverflowcom/cantos/issues/141), based on
`develop@2d816d9592075f7c3bdeb0a53b8896b2e7178043` on 15 October 2026. GitHub owns execution
status. The full [v3 inventory](ui-design-inventory-v3.md) maps the existing APIs and future
specimens; proposed specimens do not become public APIs or runtime qualifications.

## Outcome and boundary

The existing Leptos gallery now discovers v3 anatomy, states, owners, consumers, reusable
APIs and canonical token roles. Search/status/scope filters retain mounted samples. Reuse links
open real API samples; token links populate the existing token inspector. Native disclosure
controls expose the inventory without placing every proposal ahead of the working samples.
The preview composition root injects Light/Dark/System, density, Vietnamese locale, text scale
and reduced-motion policy. Nested roots retain their own theme/density while honoring the
preview's reduced-motion override. No component reads OS preferences or creates a service owner.

Two preview-owned compositions exercise episode/listening controls and Studio content/transfer/
review presentation using existing Button, RangeField, TextField, Select, Status, Badge, Surface,
Grid and Stack APIs. They carry proposed/planned labels and local presentation intents. Playback
has no audio adapter; publication remains disabled with a visible explanation. They are not
production components or admission of the corresponding proposed APIs. Remaining proposal
batches follow shared controls/layout, listening primitives, then Web-owned Studio/access
compositions; product rollout remains with #129/#131.

[Token reconciliation](../design/ui-token-v3-reconciliation.md) owns canonical v0.6.0 and its
Web/CMP mappings. Body size and minimum control size change; existing roles, editorial size and
the independent Web/CMP shape scales remain intact. The local licensed font bundle supplies
Noto Sans and Noto Serif 400/700. The preview font diagnostic reads each declared
`FontFace.status`; missing/loading faces cannot report loaded and load errors report FAIL.
No host-installed font or remote font service is used. Other requested weights use the documented
nearest supplied static face, not separate medium/semibold masters.

## Claim ledger and provenance

| Invariant / failure mode | Owner / cheapest oracle | Evidence level and result |
| --- | --- | --- |
| Stable APIs/states and exact compatibility sites survive v3 inventory | Shared catalog; independent frozen baseline and 70-ID oracle | example-tested: 32 APIs, 198 baseline pairs, 70 proposals preserved |
| A proposed design never creates an API or qualification | Shared catalog/generator; existing admission tests + gallery regression | statically-checked, example-tested: new adoption still requires both current renderer records |
| Filtering/theme/scale retain values and focus | Preview root; native browser controls and observable fixture outputs | DOM-tested, interaction-tested: retained Vietnamese input and controlled values |
| Actual local fonts are loaded, rather than fallback being mistaken for readiness | Preview shell; all four declared FontFace status observations | DOM-tested: all four report loaded |
| Reduced policy reaches nested roots without delaying intents | Preview root/CSS; computed nested shape duration + immediate callback | DOM-tested, interaction-tested: duration 0ms, callback accepted synchronously |
| Contrast uses actual foreground/background pairs | Existing Badge samples; browser computed colors + WCAG sRGB arithmetic | example-tested on actual computed pairs: all ten exceed 4.5:1 |

Source digests after the final renderer edits:

- Canonical token JSON: `25e99c49faa15a9112cd03e7e92f21360b808bbe68f0a2531017d9a0e21f0741`.
- Leptos source: `119f3dcf8490e7309c4127adb4103116ef2bb6170851bc8107057c293ea2618c`.
- CMP source: see the [fresh CMP report](issue141-cmp-preview.md).
- Raw design HTML: `6b5c42359b8530f443863ead2ac31af86c9f1735e903a5bac4ef117574ff8cfb`,
  fetched through the authenticated Drive connector from the issue's HTML file.
- Manifest/baseline hashes and exact source URLs: [inventory provenance](ui-design-inventory-v3.md).

The existing six PR137 qualification records are historical and remain unchanged. New issue141
records refresh Button, TextField, Surface, Badge, BackButton and Logo on both renderers for the
changed source/token digests. This preserves their existing admission; it admits none of the
70 proposals. Source freshness now also hashes the shared Web font bundle/provenance and design
inventory; changing these inputs invalidates Leptos independently of CMP.

## Actual browser observations

Renderer: Codex in-app browser on this macOS host, real Leptos CSR served by
`RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos run`; no API, database, identity or audio.
The exposed read-only DOM scope does not expose browser version/DPR; those remain unknown.
Locale is `vi-VN`, density Compact unless explicitly switched. Observations used native browser
input through CUA Playwright locators, not authored `dispatchEvent` calls.

Viewport overrides were 1280×900, 390×844 and 320×844 CSS px. Actual document client widths were
1265, 375 and 305 respectively where the vertical scrollbar occupied 15px. At 200% text the
root size was 32px. Narrow samples retained one column and no horizontal document overflow;
single-line input values retain native horizontal editing/scrolling behavior.

| Scenario | Observed result / opened local-only image |
| --- | --- |
| `noto-vietnamese`, `nested-theme` | Noto Sans/Serif 400/700 each loaded; Vietnamese input/focus retained on outer theme change; nested Light distinct from outer Dark; nested shape duration 0ms under reduced policy |
| `v3-inventory` | `mini-player` search returns 1/102; anatomy/state/consumer/reuse links and planned/unqualified reason visible; absent query exposes empty state; `size.control` link filters token inspector; native summaries toggle with visible disclosure markers |
| Button/auth variants | Enter sends exactly one intent; Tab exposes focus on secondary action; loading/disabled block native activation and preserve count; long labels wrap at narrow width; `actions-wide-light-focus.jpg`, `actions-narrow-light-focus.jpg` |
| TextField/auth tonal | Error associated through `aria-invalid`; readonly keeps text; loading exposes busy and disables editing; typed fixture output remains `anh-nguyet@example.test` across theme/scale/state changes; `field-narrow-dark-200.jpg` |
| BackButton | Enter dispatches one local intent; Tab focuses icon-only variant; loading/disabled remain inert; host context changes locally; `back-narrow-light-focus.jpg` |
| Badge/feedback | Five tones have icon and full text; light/dark actual colors measured; empty and recoverable error samples exercised; `badges-narrow-light.jpg`, `badges-narrow-dark.jpg` |
| Surface/reading | Quiet tonal hierarchy, primary-container supporting text and Vietnamese wrapping inspected; nested scopes remain distinct; `surfaces-wide-dark.jpg` |
| Logo | Nine official variant/tone combinations visible, decorative images hidden from accessibility, independent named symbol exposed; local images complete; `logos-wide-dark.jpg` |
| Dialog | Escape returns focus to opener; text survives close/reopen |
| `v3-compositions` | Local play intent and review error projection observed; disabled publication remains inert; no audio/network effect |

All named images were opened with CUA image output by Codex on 15 October 2026. Logs, exact
browser observations/computed colors and screenshots remain ignored under
`artifacts/ui-evidence/issue-141/` (`browser-observations.json`, `artifact-manifest.json`). They
are **evidence-local-only**, not uploaded by `gh pr create`. CMP captures/semantics are attributed
separately in its report. No screen-reader or native-device claim follows from these images.

The supplied v3 HTML was rendered through the existing Trunk host at 1280×900. Its isolated
Button specimen (`?spec=button`, enabled/light) was captured and opened as
`reference-button-light.jpg`; matching library action variants were opened at the same viewport.
The handoff's anatomy/hierarchy guided the comparison. Text fixtures and the explicitly
reconciled shape scales differ, so this is qualitative design inspection, **not a pixel-fidelity
oracle or approved pixel baseline**. No static-export PNG was scored against Chromium. #140 owns
optional attributed pixel comparison, and the separately scoped motion/scenario workbench
remains future work.

Measured Badge text contrast (neutral/info/success/warning/error): light
12.65/10.69/5.92/5.64/5.54; dark 10.79/7.69/7.20/7.59/7.57. The source checker also retains all
canonical paired-role contrast checks. These measurements do not cover artwork/scrims.

## Independent gates and completion report

Invariant: discover and reuse declared components without losing existing APIs/states, mounted
editing state or admission barriers; proposals remain explicitly unqualified.
Owner / boundary: UI-system canonical inputs/generators and the existing Leptos preview; shared
semantic values coordinate CMP. No backend/domain/rights/publication behavior changes.
Evidence level: documented, statically-checked, example-tested, DOM-tested, interaction-tested,
accessibility-checked, screenshot-inspected, cross-viewport-inspected, cross-theme-inspected;
CMP semantics/runtime/visual evidence is separately named in its report.
Evidence: native SSR tests, generator/catalog/font regressions and actual browser observations.
Command: `cargo xtask ui-preview leptos verify`, `cargo xtask ui-preview leptos run`,
`cargo xtask ui-preview cmp verify`; final repository/preflight results below.
Provenance: develop revision/digests above; repository-created Vietnamese fixtures; local browser
and JVM renderer; no live backend/provider or external audio.
Layer gates / verified scope: L1 ownership reviewed; L2 durable domain rules inapplicable;
L3 structural/static/example checks; L4 local presentation callbacks exercised; L5 browser runtime
and JVM semantics observed, DB/API/device scope inapplicable to this slice.
Deferred acceptance: production adoption of proposals, full motion/cancellation recordings,
forced-colors/OS theme-change walkthroughs, real IME and named screen-reader/device acceptance.
Execution blockers / failures: intermediate repository gate correctly rejected stale old
qualifications; fresh attributed evidence resolves freshness without relaxing admission. Default
macOS Python 3.9 is too old for existing repository tests; use the installed Homebrew Python.
Edge classes covered: existing/proposed/absent query; conjunction/case/family/status/scope;
light/dark/nested; default/200% text; long Vietnamese and diacritics; zero values; keyboard/focus;
readonly/loading/disabled/error/empty; local callbacks, dialog return, token lookup.
Not covered / residual risk: no pixel-fidelity approval, full motion recordings, browser version/
DPR, other engines, native device/screen-reader acceptance or 70-proposal product implementation.

All commands used `RUSTUP_TOOLCHAIN=1.98.1`. Repository/preflight use the installed Homebrew
Python via `/opt/homebrew/bin`; CMP uses the already installed Gradle 8.14.3 bin directory
on task-local PATH. `TMPDIR=/private/tmp` is required for the canonical macOS path in the full
Rust gate. No tool or service was installed/provisioned. Remote CI is not checked.

| Exact command | Result | Scope / ignored log |
| --- | --- | --- |
| `cargo xtask verify repository` | PASS | 430 Python tests; five optional pinned-Pillow PNG adapter tests skipped; repository links/catalog/font bytes and whitespace; `repository.log` |
| `cargo xtask ui-preview leptos build` | PASS | Preview WASM/Trunk build; `leptos-build.log` |
| `cargo xtask ui-preview leptos verify` | PASS | 73 native examples + five compile-fail doctests, WASM lint and build; `leptos-verify-final.log` |
| `cargo xtask ui-preview leptos run` | Observed readiness and browser input | Trunk host and owned shutdown observed; `leptos-run.log` |
| `cargo xtask ui-preview cmp verify` | PASS | 115 tests/18 suites, real JVM captures/semantics; [CMP report](issue141-cmp-preview.md) |
| `TMPDIR=/private/tmp cargo xtask verify rust` | PASS | Format, workspace lint and workspace tests; `artifacts/agent-runs/issue-141-media-temp-diagnostics/logs/01-canonical-temp-rust.log` |
| `cargo xtask preflight --changed-only` | PASS | All ten selected gates, no skip flags: repository, Rust format/package lint/package tests, Web lint, both product Web builds, Leptos preview, CMP preview and Theatre CMP mock; `preflight.log` |
| `cargo xtask verify contracts` | UNSUPPORTED | Missing trusted ffmpeg/ffprobe; script-ir-reference, protobuf-reference, contract-tests and wav-reference printed NOT_RUN because prerequisite precheck stopped the suite; `contracts.log` |

The first full Rust run failed three unchanged backend tests before their intended scenarios:
filesystem registry construction rejected the macOS `/var` symlink under `O_NOFOLLOW`.
The exact backend sources match the base revision. With only canonical `TMPDIR=/private/tmp`,
the full owning gate and all three tests passed. The diagnosis/unchanged-source/filesystem probe
is retained at `artifacts/agent-runs/issue-141-media-temp-diagnostics/run.md`; no backend fix was
made. An initial format failure in newly generated design provenance constants was corrected
in the generator; the final formatter and drift checks pass. The earlier failures remain in logs.

Rust formatting is checked by xtask. No Kotlin/Python/CSS formatter/linter is configured for
these package-local files; touched syntax/styles were reviewed, Kotlin compiled, Python tests
executed and whitespace checked. No literal palette, unscoped geometry or `transition: all` was
introduced. Named local inspector geometry stays in preview CSS; generated output comes from
the existing generators.

Deferred acceptance owners are unassigned unless the linked consumer issue assigns them:

| Outstanding check | Prerequisite / next command or scenario | Acceptance boundary |
| --- | --- | --- |
| Contract suite | Trusted installed ffmpeg/ffprobe on PATH; `cargo xtask verify contracts` | All five contract/reference gates PASS; unchanged domain/media contract coverage is incomplete locally |
| Optional PNG adapter | Explicitly prepared pinned Pillow 11.3.0; `cargo xtask visual-compare --self-test` | Optional five real-PNG adapter examples pass; metric success never accepts UX |
| Native/reader/OS input | Ready Android/iOS targets and a named reviewer; target-specific preview/admission scenarios | Native geometry, OS IME/Back/insets and real screen-reader walkthrough; JVM/browser captures cannot substitute |
| Full motion/scenario and remaining proposal batches | Concrete consumer and renderer references; existing component-first/admission loop | Named cancellation/reversal/reduced-policy recordings and fresh two-renderer admission before new product import |

UI/Web report additions: screens/consumers are the existing package-local Leptos preview and
shared CMP preview, with Theatre Web/CMP and Studio Web as future consumers. Surface intensity
is Low–Medium developer chrome; reading specimens stay Low. Tokens are the documented v0.6.0
body/control delta with both mappings and actual contrast above. Localization is Vietnamese
developer diagnostics/fixtures; no product translation resource changed. Accessibility covers
labels, semantic states, keyboard focus and dialog return; no screen reader was walked.
Visual evidence names opened paths, themes, viewports and scales above. Motion evidence covers
computed reduced durations and immediate intent dispatch, not a recording/profile. Same-scenario
rechecks retain the named fixtures and viewports; remaining differences/limits are explicit.
Surface/permission context is an isolated preview with no Studio/Theatre service context.
Rendering is CSR/debug through Trunk. Input methods were browser keyboard/pointer, not real OS
IME or device touch. Backend/media paths are none; asynchronous request races are inapplicable
because these compositions have no remote request owner. Style review is the manual token,
geometry/focus/motion audit plus existing static source/generator checks.
