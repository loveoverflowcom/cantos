# Shared Web Back presentation — v3 comparison

Review boundary: one Web presentation refinement requested against the
[shared component preview v3](leptos-preview-v3.md), using the existing
[local comparator](visual-compare.md). Source base is
`1854b3497a149654d3ac246e882f09fc81cd960a`; captures used a dirty working tree on
15 October 2026. No GitHub issue status is changed by this record.

Delivery boundary: this Shared UI refinement contributes to
[issue #129](https://github.com/loveoverflowcom/cantos/issues/129) as a prerequisite slice.
It does not implement Theatre Home, auth/account composition, return-context navigation or
provider/session integration, and does not complete the issue's acceptance criteria.

## Scope and cause

The disabled/loading Web Back action acquired a filled capsule behind its entire contextual
label. The generic `.cui-button:disabled` selector has specificity `(0,2,0)` and outranked
the later `.cui-back-button` `(0,1,0)` transparent background. The tonal icon circle therefore
merged visually with the long label background. The native disabled state and reason were correct.

The supplied v3 HTML's `.back` uses the global transparent button background. This supports
a quiet outer text action, but its Back specimen supplies no disabled-state pixel reference.
The [token reconciliation](../design/ui-token-v3-reconciliation.md) and
[canonical disabled treatment](../../packages/ui-system/shared/README.md#decisions-from-the-reference)
deliberately retain readable tonal disabled controls rather than the design export's text opacity.
This change is a bounded presentation choice, not proof that the former accepted contract regressed
or that the new pixels match an approved design baseline.

The [stylesheet template](../../packages/ui-system/leptos/foundation.css.in) adds only
`.cui-back-button:disabled { background: transparent; }`; generated CSS is regenerated.
The existing disabled foreground, visible reason, native blocked activation, icon circle,
48-unit minimum target, focus and motion rules remain. The
[Back specimen](../../packages/ui-system/leptos/preview/auth_samples.rs) uses a Plain surface
so accidental label fills are visible. That neutral fixture was already present in **both**
before/after captures; only the disabled Back CSS changed between those captures.

Owner / boundary: `cantos-ui` Leptos presentation and its preview. Existing `BackButton`,
`ButtonVariant::Text`, `ButtonState` and canonical v0.6.0 roles are reused. No public API, token,
localization string, domain decision or service changes. There is no product Web Back consumer
in this source revision; CMP Back remains unchanged and gains no qualification from this work.

## Attributed comparison

Real Leptos CSR was served by `RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos run`
on `http://127.0.0.1:8083`. Renderer: Codex in-app browser on macOS, engine/version unobserved,
DPR 1. `document.fonts` reported loaded for the bundled local Noto fonts. No backend, identity,
database, audio, mocked port or external request is required for this specimen.
The immutable pair metadata uses an unverified Chromium label; it supplies no engine acceptance.

Fixture: `neutral-back-canvas-v1`, locale `vi-VN`, Compact density, disabled visible-label and
icon-only Back, long existing Vietnamese contextual name. Native gallery controls select
`#auth-back-state=disabled`, `#gallery-theme` and `#gallery-large-text` for the cases below.
The recipe URL alone does not persist the gallery's 200% toggle or Back state; replay those controls.

| Scenario | Requested viewport, text | Captured content | Changed pixels / total | Advisory result / regions |
| --- | --- | --- | --- | --- |
| `back-disabled-light-v1` | 1280×720 CSS px, Light, 100% | 1265×712 px | 22,298 / 900,680 (2.4757%) | `different`; 1 at `(48,143,385,146)` |
| `back-disabled-dark-v1` | 1280×720 CSS px, Dark, 100% | 1265×712 px | 20,927 / 900,680 (2.3235%) | `different`; 1 at `(56,143,376,137)` |
| `back-disabled-dark-390-text200-v1` | 390×844 CSS px, Dark, 200% | 375×812 px | 30,879 / 304,500 (10.1409%) | `different`; 1 at `(24,303,312,114)`, 2 at `(24,480,56,56)` |

These are unreviewed `v1` regression snapshots, not approved design-state baselines. Pixel
dimensions are the screenshot content dimensions, distinct from the requested viewport.
Threshold remains 0, tile size 16, no masks or anti-alias filtering. The tool completed all
three reruns with `PASS` while each report correctly says `different`; tool completion is
not visual approval or a design-fidelity score.

Before/after DOM facts retain identical Back rectangles, foregrounds, disabled attributes and
reason text. Wide named Back remains 368.15625×48 CSS px; narrow 200% named Back wraps to
311×103.984375 CSS px; icon-only remains 48×48 CSS px. Background alone changes from
`surfaceContainer` to transparent. The evidence collector's source identities distinguish
base + neutral fixture from base + neutral fixture + disabled Back CSS.

Ignored local evidence is under `artifacts/ui-evidence/shared-back-v3/`:
`baseline-facts.json`, `after-facts.json`, `pair-*.json`, original `before-*.jpg` /
`after-*.jpg`, decoded PNGs and `compare-png-*/report.json` / `report.md` / context/detail images.
The reports retain image hashes, comparator hashes, baseline approval and pairing metadata.
Their `captured_at` fields remain unknown; the dated session does not repair those missing fields.
Recheck policy SHA-256 for Light, Dark and narrow Dark respectively:

- `a99b15191f20b53e51b0cda1fb42156dfc955c410cdadaf412f78d73fa96cbdc`
- `f5fb661ebf5c7bb6dcd4270bf739c3c1acf7291a406f6dbe45204ed72b8404c9`
- `710a3ca79614978596c37bde699561b429373982c27b0fc2a13bfbc84da47239`

The initial CUA screenshot bytes were JPEG despite their initial `.png` filenames. The comparator
reported `FAIL` on the invalid PNG signature. Original JPEG bytes were preserved, decoded once
and encoded losslessly as PNG without resizing, cropping, filtering or changing decoded pixels;
the three comparisons then completed. JPEG compression remains an input limitation: region
edges can include compression noise, and the diff is not a lossless browser raster oracle.

Visual inspection ledger: Codex root opened the Light before capture, live after captures for
all three scenarios, then every comparator overlay/diff/mask/heatmap and each reported region's
reference/actual/overlay on 15 October 2026. Observed changes are localized to the Back controls:
the long capsule fill is removed while the circle, foreground and line wrapping remain. Exact
threshold also catches JPEG block noise around region edges. The generated reports retain
`images_inspected=false`; this separate attributed ledger records the human-readable findings
without rewriting tool output. No inspection or design percentage is inferred from tool PASS.

Additional root observations: loading at Dark 390×844 / 200% retains transparent background,
native disabled state, `aria-busy=true`, reason and geometry (`loading-dark-390-text200.jpg`).
In enabled Light, Tab from the preceding select focuses `auth-back-visible` with a 3px solid
outline; Space dispatches exactly one local Back intent (`focus-light.jpg`, `keyboard-facts.json`).
These are local component observations, not navigation, screen-reader or motion acceptance.

## Existing qualification compatibility

The conservative Leptos digest covers the entire renderer, so the small Back change invalidates
six existing records. Codex root reran their bounded browser compatibility checks before refreshing
those records; no new component is admitted. Non-Back component code, styles, assets and canonical
tokens are unchanged. This repair changes no CMP source or qualification record; the statement
is scoped to its diff, independently of concurrent CMP preview work on `develop`.

The review agent reopened the historical issue141 Button design/render, narrow focused auth,
narrow Dark 200% field, both narrow Badge themes, Dark Surface and focused Back captures.
`logos-wide-dark.jpg` contains Back/Badge rather than Logos and is excluded as Logo reference
evidence. The real issue142 Logo top/bottom captures were reopened against the unchanged local
SVG/anatomy contract. None is a new design-state pixel baseline.

| Existing component | Fresh browser compatibility / opened captures by Codex root |
| --- | --- |
| Button | Enter dispatches one intent; native Tab focus has a 3px solid outline. Forced clicks while loading/disabled leave the count at one; eight regular controls remain natively blocked with reason associations. Wide Light and corrected narrow auth captures retain hierarchy and Vietnamese wrapping: `compat-button-light.jpg`, `compat-button-disabled-light.jpg`, `compat-auth-narrow-light.jpg` |
| TextField | Vietnamese value survives readonly, error, theme and 200% text changes; native focus is 3px and help/error IDs resolve to real descriptions. Loading retains the value while disabled/busy. Opened narrow Dark field and full label/help/error: `compat-field-dark-390-text200.jpg`, `compat-field-help-dark-390-text200.jpg` |
| Badge | All five tones retain an icon and full text at 390×844 in both themes. Actual computed text contrast, neutral/info/success/warning/error: Light 12.65/10.69/5.92/5.64/5.54; Dark 10.79/7.69/7.20/7.59/7.57. Opened `compat-badge-light-390.jpg`, `compat-badge-dark-390.jpg` |
| Surface | At 1280×900 Dark, auth/profile/member/account and quiet reading/nested groups retain Vietnamese text, paired foregrounds and zero decorative borders. Opened `compat-surfaces-dark-top.jpg`, `compat-surfaces-dark-reading.jpg` |
| Logo | All 12 specimen images load: nine variant/tone combinations plus decorative/named/140px aspect-fit samples. Wordmarks retain natural 283:96, symbols are square; exactly one image has alt `Biểu tượng Cantos`, others remain decorative/hidden. Light fixtures retain their nested Light canvas in the Dark host. Opened `compat-logos-dark-top.jpg`, `compat-logos-dark-bottom.jpg` |
| BackButton | Enter dispatches one intent; forced clicks on both disabled variants and loading keep the count at one. The attributed comparison, loading and keyboard observations above cover the presentation refinement |

Artifacts remain local under `artifacts/ui-evidence/shared-back-v3/`; `compatibility-facts.json`
retains DOM facts and actual Badge color pairs. Root opened each current capture through CUA.
These are bounded compatibility, DOM, interaction, accessibility and visual observations;
passive components have no interaction to drive. They preserve the existing admitted set without
claiming complete state/motion, screen-reader, OS IME, native-device or design-pixel acceptance.

## Reproduction and independent gates

Use Rust 1.98.1 and the existing system Python 3.9 interpreter with Pillow 11.3.0 for PNG I/O;
repository tests use the installed Homebrew Python 3.14.8 (prepend `/opt/homebrew/bin` to `PATH`).
The commands install no tools. Serve the preview, select catalog/navigation, choose the exact fixture controls
above and wait for fonts before capturing. Keep the neutral canvas, viewport, scroll, state,
theme, locale, density and text scale identical. Reuse the saved metadata and policy. The recorded
comparison output directories are immutable; choose a new output directory for a later recheck:

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos run
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask visual-compare --self-test
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask visual-compare --reference artifacts/ui-evidence/shared-back-v3/before-light.png --actual artifacts/ui-evidence/shared-back-v3/after-light.png --scenario back-disabled-light-v1 --metadata artifacts/ui-evidence/shared-back-v3/pair-light.json --threshold 0 --tile-size 16 --output artifacts/ui-evidence/shared-back-v3/compare-png-light
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask visual-compare --reference artifacts/ui-evidence/shared-back-v3/before-dark.png --actual artifacts/ui-evidence/shared-back-v3/after-dark.png --scenario back-disabled-dark-v1 --metadata artifacts/ui-evidence/shared-back-v3/pair-dark.json --threshold 0 --tile-size 16 --output artifacts/ui-evidence/shared-back-v3/compare-png-dark
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask visual-compare --reference artifacts/ui-evidence/shared-back-v3/before-dark-390-text200.png --actual artifacts/ui-evidence/shared-back-v3/after-dark-390-text200.png --scenario back-disabled-dark-390-text200-v1 --metadata artifacts/ui-evidence/shared-back-v3/pair-dark-390-text200.json --threshold 0 --tile-size 16 --output artifacts/ui-evidence/shared-back-v3/compare-png-dark-390-text200
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask ui-preview leptos verify
RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify rust
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify contracts
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify web
PATH="/opt/homebrew/bin:$PATH" RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask verify repository
PATH="/opt/homebrew/bin:$PATH" RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask preflight --changed-only
```

| Gate / layer | Result and precise scope |
| --- | --- |
| L1 — architecture | Reviewed existing Web owner, selector cascade, preview consumers and minimal scope; documented/source-reviewed, no new boundary |
| L2 — domain/types | Inapplicable to the CSS patch; existing availability/name/intent ownership remains unchanged |
| L3 — examples/static | `cargo xtask ui-preview leptos verify`: `PASS` for native examples, five compile-fail doctests, WASM lint and build; log `preview-verify.log`. Existing `auth_account.rs::back_is_a_named_native_action_not_a_link_submit_or_implicit_navigation` covers native loading semantics. CSS polish uses the foundation's inspected-visual exemption; no redundant source-mirroring CSS test |
| L4 — component/DOM | DOM facts match for geometry, foreground, disabled attributes and reasons across all three cases; loading retains busy/blocked semantics and enabled Tab/Space dispatches once. DOM-tested and interaction-tested locally; no changed application adapter or mocked integration |
| L5 — real renderer | Local CSR rendered and captured in the three named scenarios; root opened live images and all comparator context/detail images as attributed above. No DB/API/native-device or screen-reader acceptance claim |
| Comparator self-test | `cargo xtask visual-compare --self-test`: `PASS`, 12 tests; `comparator-self-test.log` |
| Three PNG comparisons | `PASS` tool completion, each report `different`; initial mislabeled JPEG input failure retained above |
| Rust / contracts / Web | `cargo xtask verify rust`: `PASS` format/workspace lint/tests; `cargo xtask verify contracts`: `PASS` all five selected checks; `cargo xtask verify web`: `PASS` product WASM lint/builds and preview build. Logs: `rust-verify.log`, `contracts-verify.log`, `web-verify.log` |
| Repository | Final `PASS` (`repository-python314.log`): 440 tests, 5 optional Pillow adapter cases skipped under Python 3.14; those cases ran in the separate 12-test comparator self-test with system Python/Pillow 11.3.0. Initial `FAIL` exposed six stale Leptos qualifications; all six were actually rechecked and refreshed, with CMP records unchanged. A second `FAIL` used system Python 3.9 without `tomllib` / `os.waitid`; rerunning with installed Python 3.14.8 passed the gate |
| Final preflight | `PASS`: repository, Rust format/lint/tests for all three affected Web packages, Web lint, both product builds and preview build; `preflight.log`. No selected gate skipped |

Invariant / expected presentation: keep a quiet Web Back outer action in blocked states while
preserving its readable disabled text, tonal icon, target and explicit reason. Confirmed cause
is the selector cascade; the presentation choice is distinct from a historical-contract claim.
Evidence levels: documented, example-tested, statically-checked, DOM-tested, interaction-tested
and screenshot-inspected across the named viewports/themes; no direct design-pixel qualification. Surface intensity remains Medium
auth/account, with quiet Back hierarchy. Localization and token impact: none; no new literal or
key. Accessibility: native disabled/name/reason semantics retained by existing tests and DOM facts;
Tab focus and Space activation observed; named screen-reader walkthrough not performed. Motion: unchanged source; no new
recording or reduced-motion runtime observation. Performance: unaffected and unmeasured.

Deferred acceptance / residual risk: approve an exact Web disabled-state design baseline and
recapture with lossless renderer PNGs and complete browser/timestamp provenance before claiming
design fidelity. Source-bound Leptos compatibility qualification is refreshed after the actual
six-component checks above and final preflight passed. The task-owned preview processes were
stopped, temporary browser tabs closed and the viewport override reset after observation.
Loading Back pixels are observed only in the named narrow Dark case; enabled hover, forced-colors,
real IME, motion, named screen-reader and native-device acceptance are not added
by this slice. CMP and product rollout retain their independent gates. The original inspection
stopped before commit/push/PR; the subsequent delivery request packages this existing change
for review against `develop`. Issue #129's later Home/Auth/Account integration and its separately
scoped browser/API evidence are recorded in [the product integration ledger](theatre-web-auth-account.md);
they do not broaden this inherited Shared Back comparison's conclusions.
