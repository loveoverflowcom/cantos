# Studio Web focus consumer review (#735)

This is a source and bounded browser-CSS record. It does not qualify an authenticated
Studio workflow, a screen reader, CMP or production. Base: freshly fetched
`origin/develop@06ab3bb3a1321c3e093019a5cd4d399f536c9588`. No token, generated output,
shared component API, permission or domain transition changed.

## Inventory and consumer decisions

The canonical `size.focusWidth` / `size.focusOffset` remain 3 / 4 logical units
(3px / 4px at a 16px root; 6px / 8px at a 32px root). The source is
[`tokens.json`](../../packages/ui-system/shared/tokens.json); shared anatomy belongs to
[`foundation.css.in`](../../packages/ui-system/leptos/foundation.css.in).

| Consumer / focus-visible state | Before | Decision and final owner |
| --- | --- | --- |
| Studio native `a, summary, button, input, textarea, select` fallback, excluding `.cui-field-control` | Canonical width, negative width offset; selector specificity overrode shared buttons, choices, links and artwork disclosure | Keep the flush inset for native controls at pane edges; put the **entire** selector inside `:where()` (specificity 0). Shared controls and named app compositions win with their own anatomy. `studio.css` owns this fallback. |
| `.delegation-panel [tabindex="-1"]` non-heading target | Canonical width, negative width offset | Obsolete duplicate removed; the general non-heading programmatic-target rule owns the same inset, keeping content focus visible within a panel. |
| Studio `[tabindex="-1"]` non-heading target | Canonical width, negative width offset | Retain this renderer geometry, derived from width rather than an unrelated spacing alias. It contains the indicator at content/scroll boundaries. |
| Studio heading `[tabindex="-1"]` | General `outline: none`, overridden on three panel titles | Retain the existing heading policy: programmatic focus and announcement survive without framing a non-operable title. Remove the three conflicting rules below. |
| `.cw-audio-stage summary` | Explicit 2px width / 2px offset | Reuse canonical width / offset; padded audio stages provide perimeter room. Open and closed native disclosure states keep the same indicator. |
| `#live-audio-stages-title` | Explicit 2px / 2px | Obsolete override removed; heading policy above. |
| `.cw-preview-zone h5` | Explicit 2px / 2px | Obsolete override removed; actual preview heading has `tabindex="-1"`. |
| `#metadata-review-title` | Explicit 2px / 2px | Obsolete override removed; actual review heading has `tabindex="-1"`. |
| `.artwork-file-input:focus-visible + .artwork-file-label` | Canonical width / offset | Already correct; unchanged. Native input retains keyboard focus, associated label paints its visible ring. |
| `.artwork-metadata summary` | Canonical width / `space.xs`; global fallback actually won with negative offset | Replace spacing alias with `size.focusOffset`; low-specificity fallback now yields. `artwork.css` owns this operable disclosure. |
| Shared `.cui-button`, `.cui-choice-control`, `.cui-link` | Shared canonical rule was overridden by Studio's fallback | Restore shared width / offset by lowering the app fallback; no shared source edit. |
| Shared field control / Floating shell | Dedicated inset 2-unit active stroke; Floating control itself has no outline | Already correct; unchanged. This is the shared field state perimeter, not a second 3/4 ring. |

Lookup reused registered `button`, `link`, `text-field` and choice-control anatomy; no new
component, API, import or preview admission was needed. Artwork and content panels remain
Web-owned compositions. Surface intensity remains Low / Low-Medium. No localization or
motion changed. Behavior SSOT impact: presentation-only focus paint; no state, guard,
authority, outcome, keyboard order or focus dispatch changed, so no model delta.
Queue: no change to dependencies or delivery order.

## Local evidence

Working directory:
`/Users/manhblue/Documents/Codex/2026-11-06/task-3/cantos`.
Full local records are retained in ignored `artifacts/agent-runs/issue735/`.

- `python3 -m unittest discover -s scripts -p test_ui_component_states.py`: the two new
  regressions first failed against the original consumers (legacy selector/heading overrides
  and artwork spacing alias). All eight component-state tests then passed after the repair.
  Assertions name exact selectors, state declarations and owner exceptions, rather than
  checking whether token names occur somewhere in a file.
- `node artifacts/agent-runs/issue735/focus-browser.mjs`: PASS, Chrome
  `156.0.8078.80`, headless, CDP keyboard events and computed styles. The fixture embeds the
  unchanged token/foundation CSS and before/after app CSS in production load order over
  source-matched panel/control DOM. No API, account, media or network fixture was used.
  `computed-focus.json` contains 32 before/after cases: 320/390 CSS px, Light/Dark,
  16/32px root text (100/200%) and forced-colors none/active, DPR 1.
  This is root text scaling, not browser zoom or an OS accessibility setting.
- Keyboard Tab reached the native picker, both disclosures, shared/native buttons, normal
  and Floating fields, checkbox and link. Enter opened and closed both disclosures with
  focus retained. Programmatic focus after keyboard input reached all three actual heading
  selectors and the non-heading content target. The assertions checked `activeElement` and
  `:focus-visible`; screenshots alone were not used as keyboard evidence.
- After: visible picker/disclosure/shared-control rings computed as 3/4 at 100% and 6/8 at
  200%; native/content inset remained negative width. Fields kept one 2-unit inset perimeter,
  including Floating shell focus with no control outline. No horizontal overflow or clipping
  by a scroll/clip ancestor was observed for the tested perimeter targets in the fixture.
- Opened `after-light-320-text200.png` and `after-dark-320-text200.png`: long Vietnamese
  disclosure text wrapped, with a visible focus perimeter in both themes. Their fixture
  layout is not a screenshot of the mounted Studio app.
- Input hashes are retained in `before-inputs.json` / `after-inputs.json`; browser script,
  HTML, computed observations, log and before/after PNGs remain local evidence, outside Git.

Final changed-only preflight and self-review results are recorded in the pull request.
No registered assurance obligation was derived by the preflight plan for these paths;
that path mapping is not proof of no semantic impact.

## Remaining acceptance

AC-04 remains open for the authenticated, mounted Studio picker/disclosure/audio/preview/review
flow and actual 200% browser text zoom. The bounded browser fixture verifies the CSS repair,
not handler reach, focus restoration after async work, screen-reader announcements or private
API integration. Resume with a local qualified Studio session and synthetic Work/unit/audio
facts, launch via `cargo xtask run cantos-studio --web`, and repeat the same Light/Dark,
320–390px, 200% text and forced-colors keyboard matrix on the real panels. Next owner is
unassigned; those product-flow observations are required before whole-issue UI acceptance.
No deploy, CMP qualification or production acceptance was performed.
