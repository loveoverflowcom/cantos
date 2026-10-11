# UI system independent review — session 4

## Scope and assessment

Date: 2026-10-11. Starting clean branch: `feat/leptos-ui-system`, HEAD
`974eff5840d349f665ce5a4ceb6566fd79c9637b`. Original requested base:
`dc7c2de311c400a7581347a80b33d9b8b34c25a4`. Review used the original-base UI delta and
two-dot `03ba1416aa0e646b1d6022534f97e44f5b2ea1a4..974eff5840d349f665ce5a4ceb6566fd79c9637b`
after PR29 reconciliation. Local merge `83fbadb40c7655d5721c6c8b346638390ef49c74` has
PR29 develop `03ba1416aa0e646b1d6022534f97e44f5b2ea1a4` as its second parent; ancestry was
rerun successfully. Backend, contracts, production/authoring/editor reducers and API transport
are unchanged relative to that parent. This review does not review or approve the TTS backend.

Coverage: complete-for-declared-scope for the UI source boundary, generator, shared component
anatomy, app integration and scoped fixes; partial for rendered/device accessibility. Relevant
product, architecture, queue and prior evidence documents were read. Prior session evidence is
provenance, not a result rerun here. Large historical backend patches, real provider behavior,
font/art rights, other browsers and mobile implementation are outside this scope.

Assessment after scoped fixes: **no-actionable-findings** in the inspected UI source;
**insufficient-evidence** for complete accessibility/platform acceptance. Four confirmed defects
below were repaired under the requester's explicit fix authorization, separately from the
read-only review lane. Remaining runtime evidence gaps are not a pass or a safety certificate.

## Confirmed defects and regressions

| Finding | Location and trigger → consequence | Fix and evidence actually held |
| --- | --- | --- |
| P2 · compatibility · confirmed defect · confirmed | `view.rs::ValidationControls` was mounted in both revision and adaptation panes with the fixed `revision-validate`/reason IDs. A description could resolve to the hidden pane. | Callers supply `revision-validate` and `adaptation-validate`. The structural caller regression failed before and passed after; mounted Studio DOM reported no duplicate IDs. |
| P2 · compatibility · confirmed defect · confirmed | Safari 18.6 native menulists rendered at 22 px despite the 44/48 px minimum. Inserting an indicator also exposed stretched grid rows and long native option overflow (Studio document 305→311 px at width 320). | Shared scoped select wrapper, licensed outlined indicator, start-aligned fields and a block native select with clipped display text. Authoring/import compose the same wrapper while retaining exact event/option bindings. Native anatomy regression failed before/passed after; gallery five selects 44/48 px, revision seven selects 48 px and 305/305 px document after repair. Full option text stays available in the native popup and unsupported-value explanation. |
| P2 · compatibility · confirmed defect · confirmed | `foundation.css.in` negative dialog-body focus margins caused Safari horizontal scrollable overflow: modal client/scroll widths 560/567, with a visible scrollbar. | Keep focus padding inside the body box. Same-browser captures show no horizontal strip; narrow modal client/scroll widths are 273/273. No page-level overflow hiding was added. |
| P2 · logic · confirmed defect · confirmed | Safari pointer activation did not focus the opener. Dialog captured the body/previous element and later reported `ReturnFocusRejected`. | Opt-in `Button::focus_on_press` focuses its enabled native button before dispatch. Gallery and Studio discard triggers opt in; other buttons keep native pointer behavior. Direct-page pointer → Escape regression failed before and returned to the correct opener afterward, with no error; reopening retained exact Vietnamese text. |

The first two defects have permanent source/native anatomy regressions. Dialog regressions are
manual Safari reproductions/captures in this task's Computer Use transcript, not an automated
browser suite. A scripted or DOM-only fake was not substituted for native focus behavior.

## Assurance ledger

| Claim | Owner / target | Required by | Evidence held → level | Disposition |
| --- | --- | --- | --- | --- |
| Canonical v0.2.0 roles produce deterministic scoped CSS and typed Rust | `shared/tokens.json`, generator, `cantos-ui` | doc:docs/design/ui-system.md; policy:AGENTS.md | real output drift, schema/contrast/determinism fixtures, historical-byte preservation → statically-checked, example-tested | held; generated files load before host geometry |
| SVGs are offline, decorative and provenance-bound | shared icons, generated `Icon` | policy:AGENTS.md provenance; doc:packages/ui-system/shared/README.md | nine pinned official hashes/Apache-2.0; allowlisted static anatomy, malicious SVG refusal; Safari renders → statically-checked, example-tested, rendered-observed | held; no icon font/CDN/runtime markup injection |
| Component text/attributes and textarea preserve escaping | fields/buttons/feedback/native HTML, CSR apps | review-profile:requester SSR/CSR escaping | native malicious text/entity/leading-LF examples; actual `< > &` Vietnamese field remains text → example-tested, rendered-observed | held for tested seams; deployed apps are CSR, not an SSR application |
| Controls expose labels, stable descriptions and disabled/loading reasons | shared controls; Studio consumers | policy:AGENTS.md UI verification | native anatomy tests; zero duplicate mounted IDs; native accessible labels and loading reasons → example-tested, rendered-observed | held within sample; VoiceOver remains gap-required |
| Raw zero, buffers, selected choices and provider/model bounds survive integration | app authoring/production and shared fields | review-profile:requester PR29 preservation | unchanged reducer/transport diff, native tests; gallery empty→0, casting provider/model/neutral/inherit and unsupported voice observation → example-tested, rendered-observed | held within observed examples; no provider dispatch or production approval claim |
| Draft/revision authority stays app/backend-owned | Studio shell and private API | policy:AGENTS.md architecture/immutability | pane-switch draft retention, safe discard Escape, Enter login/read, validate/review/save revision 2→3 on a fresh fixture; restart/replay PostgreSQL runner → integration-tested (local PostgreSQL), rendered-observed | held; domain/backend source unchanged |
| Dialog owns modal anatomy, safe initial focus and caller state | shared Dialog; Studio discard | review-profile:requester focus/scroll/Escape | direct Safari initial close focus, typed text, close/reopen, Escape/return target; narrow/200% captures → rendered-observed | held for observed paths; exhaustive Tab traversal including browser chrome and large-body scrolling remains gap-required |
| Responsive layout and Vietnamese text remain usable | gallery and Studio Leptos Web | doc:docs/design/ui-system.md#accessibility-and-motion-acceptance; review-profile:requester | measured iframe widths, direct-page interaction, actual Safari page zoom, screenshots → rendered-observed | held for recorded cases only; 600/840 and every-theme/state cross-product not run |
| Private editing and unavailable delivery gates are preserved | Studio/Theatre | policy:AGENTS.md architecture boundaries | no public-delivery consumer/provider/storage effect in primitives; Theatre unavailable playback controls; actual private local fixture only → source-reviewed, example-tested | held for scope; production/listening acceptance not applicable |
| CMP stays deferred and native playback stays separate | `cmp/README.md`, mobile guides | review-profile:requester Leptos only | documents/dependency boundaries → source-reviewed | not-applicable: no CMP/mobile code or runtime gate in this slice |

Opaque palette pairs and sampled hover/pressed text pairs were checked with independent sRGB
math; the sampled minimum was 4.65:1. This is mathematical example evidence, not pixel sampling
of every rendered state, font or monitor. Native HTML-string tests do not establish interaction.

## Actual browser method and observations

The Mac was initially locked. After the requester unlocked it and authorized Safari interaction,
Computer Use used the native Safari surface, version **18.6 (20621.3.11.11.3)**. No browser
security setting, remote debugging flag or OS accessibility preference was changed.

Builds are debug CSR. Gallery used a task-owned loopback static server; Studio initially used
Trunk's same-origin `/api/` proxy. For stable final viewport checks, a task-owned stdlib static/API
loopback harness served the same build and forwarded only `/api/` to the local API. This avoided
Trunk reloads discarding fixture state. The native before-unload warning was observed and only
synthetic test drafts were discarded. These are local harness observations, not deployed evidence.

A same-origin iframe fixed the **child CSS viewport** to width 320, 360, 768 or 1440 and height
720. Native screenshots plus child DOM measurements checked document width, visible bounds,
duplicate IDs, focus/outline, select heights and modal width. At 100% Safari's scrollbar used
15 CSS px, hence document client widths 305/345/753/1425. An iframe is not a resized native
window: its modal can allow focus into the parent harness. Native focus/Enter/Space and final
dialog regression checks were therefore also performed on the direct app page. Browser chrome
can participate in keyboard traversal; this review does not certify an exhaustive focus trap.

| Observed case | Actual result |
| --- | --- |
| Gallery light, compact: 320/360/768/1440 | Document client/scroll widths equal; no out-of-bounds content or duplicate IDs; five selects each 44 px after repair. |
| Gallery dark, comfortable: 768 | Theme changed through native Down/Return; five selects each 48 px; focus outline 3 px; visible Vietnamese text and controls. |
| Gallery native buttons | Option-Tab reached the primary action; Space then Enter produced count 2. Loading disabled all seven actions/chips, exposed visible reasons and kept count 2. |
| Gallery number / dialog | Clearing numeric input preserved empty, then pasting `0` preserved zero. Safe close focus, Vietnamese `< > &` text, reopen retention and Escape return observed. Narrow 320 dialog was 273/273 px. |
| Gallery direct native page at 200% | Actual Safari page zoom, modal title/close/body/action remain visible without the previous horizontal scrollbar; Escape works. |
| Studio revision light: 320/360/768/1440 | Seven native selects 48 px; document widths 305/305, 345/345, 753/753 and 1425/1425. Editor changes column structure and wraps controls; captions/diacritics observed in captures. |
| Studio dark: desktop and 320 at 200% | Safari Page Menu explicitly read **200%**; parent viewport 1621→810 CSS px. Child stayed 320 CSS px, document 313/313 due scaled scrollbar. Scene title and wrapped controls observed; zoom restored to 100%. This is page zoom, not text-only zoom or OS font scaling. |
| Studio private fixture flow, direct page | Native Enter login/read; title `Nguyễn Thị Ánh Nguyệt — bản nháp cuối phiên < > &` survived pane change and discard Escape. Validate + explicit review + save created immutable revision 3. Pointer discard returned focus to the opener. |
| Studio casting sample | Correct-grant fixture loaded eight catalog capabilities. Switching reference-only→Google required an explicit model; google-neural2 kept the unsupported synthetic voice with a visible explanation. Neutral and inherit choices and pitch 0 were observed. No settings save/freeze/approval, provider call or audio generation was performed. |

All native captures are in this task's Computer Use transcript, including 320 select before/after,
dialog scrollbar before/after, actual reference HTML, editor widths and page zoom. There is no
fabricated PNG or claim that a static design board is an HTML screenshot. Task-owned diagnostic
files/results and command logs are outside Git.

## Reference and delivery boundary

The already-materialized Studio reference HTML was rendered locally at 320/768/1440, unchanged:
SHA256 `de39cc8d0a032a1a2abd9d021b4703ec7af288c0124abab585c41533bf5b1ae6`.
It has no external `src`/`href` resource hosts. Source CSS supplied the paper/lavender palette,
spacing and editorial hierarchy. The reference's full homepage, artwork, role chooser and
future navigation are not implemented by the current Studio editor. Shared semantics are the
delivery scope; this is not a pixel-identical clone. Embedded font/art distribution rights
remain unresolved, so the apps keep disclosed local/system font fallbacks and no reference art.
No private Drive ID/URL or raw HTML enters Git. Other three design surfaces were not re-rendered
here; their prior source review is linked in the [foundation record](ui-system-foundation.md).

## Local checks rerun in this session

Python 3.13.5; Rust/Cargo 1.87.0; Trunk 0.21.14. Cargo jobs were limited to two and warm targets
reused. No SDK/model download, Docker, remote CI polling or blanket Cargo clean occurred.

| Actual command | Result and boundary |
| --- | --- |
| `CARGO_BUILD_JOBS=2 cargo test -p cantos-ui -p cantos-studio -p cantos-theatre --locked` | PASS after fixes: 35 UI + 127 Studio + 1 Theatre; native logic/rendering examples only. |
| Focused native all-targets Clippy | PASS on initial reviewed head; final reconciliation checks below replace this as final-source evidence. |
| `cargo clippy -p cantos-ui -p cantos-studio -p cantos-theatre --all-targets --target wasm32-unknown-unknown --features cantos-ui/preview --locked -- -D warnings` | PASS after fixes, jobs 2. |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS after fixes: 129 tests, including mounted validation ID regression. |
| `python3 scripts/check_repository.py` | PASS before the report: 290 text files and 15 skills. Final docs/reconciliation check is recorded below. |
| `python3 scripts/ui_tokens.py --check`; `python3 scripts/ui_icons.py --check`; compatibility `studio_tokens.py --check` | PASS. Compatibility command initial rerun; canonical token/icon checks rerun after fixes. |
| `cargo fmt --all -- --check`; `git diff --check` | PASS initial check; source formatted after fixes, final staged/unstaged checks required below. |
| `NO_COLOR=true trunk build --locked` in both app Web roots | PASS after fixes, jobs 2. All three built consumers were independently checked for byte-identical canonical token/foundation CSS; asset fingerprints are saved outside Git. |
| `NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview` in `packages/ui-system/leptos` | PASS after fixes. Gallery is an actual separate CSR consumer. |
| `CARGO_BUILD_JOBS=2 TMPDIR=/private/tmp python3 scripts/test_postgres.py --keep` | PASS on a new disposable cluster: application-role PostgreSQL/HTTP, restart/kill/replay/immutable source/revision/production-input oracles, zero generation attempts. Ran before browser fixture edits; not a provider or UI oracle. |

Logs use `/tmp/cantos-ui-session4-*.log`. Prior target receipt files (158 files, 2.7 MB) were backed
up before running the cluster oracle; existing clusters, source, media, model/runtime and Git
data were untouched. Browser editing used the new editor test database; casting used the new
restart/production-input test database with the correct grants. The first editor database had
no production grants and returned 503 on production reads; its error was not treated as a
casting UI pass. No inherited or production database URL was used.

## PR30 integration checkpoint

The independent remote read and authorized fetch pinned develop to
`44886f1a7f22e2b40c68362cb2b47a0e781bf70a` (PR30, two commits after PR29). PR30 changes the
canonical developer command surface and documentation, not app/domain Rust or CSS. The requester
then authorized a local merge after the separate UI review-fix commit. At this report snapshot
the merge and final xtask compatibility checks are pending; their concrete results must replace
this checkpoint before handoff. The incoming command selector still names `packages/design-tokens`
and its fake graph omits `cantos-ui`; its ADR 0012 number also collides with the UI decision.

## Remaining evidence and stopping boundary

- **NOT_RUN**: VoiceOver names/announcements and reading order; a real Vietnamese OS composition
  session; text-only zoom/OS font scaling; OS reduced motion (current media query was false),
  forced colors, touch input and other browsers/platforms.
- **NOT_RUN**: complete Tab/Shift-Tab traversal including browser chrome, optional custom
  initial-focus error paths, long-body dialog scrolling at every narrow/zoom combination;
  every theme/density/state cross-product and dedicated 600/840 observations.
- Source/native tests cover composition hooks, provider/model bounds, zero/unsupported values,
  async state guards and escaping; missing device checks remain **required evidence gaps** under
  the requester's profile and UI policy. The owner should run those concrete scenarios before
  claiming comprehensive accessibility or production acceptance.
- CMP remains documentation only; Theatre remains a skeleton. No mobile runtime, native audio,
  paid provider sample, publication or end-to-end production/listening result is claimed.
- No push, PR creation, remote merge, Drive mutation or public Site publication is authorized
  by this sequence. The requested four sessions stop after local handoff.

Residual risk: bounded Safari examples and build/source evidence leave the listed accessibility,
other-browser, font-rights and production/listening gates open; they are not waived by this report.
