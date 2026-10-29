# Search Dock floating overlay (#604)

Source checkpoint: 2026-10-29, base `origin/develop` `aebe690d`. Source-bound development
evidence for [#604](https://github.com/loveoverflowcom/cantos/issues/604), not production
acceptance. Observations come from the in-app Chromium browser pane driving the real Theatre
Web WASM (production feature set, live mode) over a **synthetic** `/api/v1/theatre/works`
in-page fetch stub with ten fictional Works and null covers. No Server, Identity, auth or cover
policy was exercised, and the owner's deployed build/zoom/browser stay UNKNOWN.

## Reproduction and root cause (browser-confirmed)

The same scenario ran on the unmodified base build and on the patch: 1280×800, dark, Vietnamese,
empty query, focus the Search input.

| Observation | Base | Patch |
| --- | --- | --- |
| Label `grid-row` (computed) | `1 / -1`, so centered across header + dock | `1`, in the header row |
| Header shell height | 520 px (grows with the dock) | 72 px, open or closed |
| Hero height / filters top / document height | 711 / 825 / 1972 | 263 / 377 / 1499, identical open and closed |
| Prompt vs first rows | prompt painted over "Ánh đèn cuối sân khấu" | prompt stays in the pill |
| Context copy | "Kết quả đang hiển thị: Tất cả loại · Tất cả loại" | none for an unfiltered catalogue |

Root cause of the overlap: the empty-prompt rule
`.cui-field-control:placeholder-shown ~ .cui-field-label { grid-row: 1 / -1 }` (specificity 8)
beats the docked header rule `grid-row: 1 / 2` (specificity 6), and the base shell has two
explicit rows, so `-1` is the end of the dock row. The reflow is the dock body living in that
second grid row.

## Change

- `foundation.css.in`: label stays on row 1; the dock is a `position: absolute` layer below the
  field (`inset-block-start: calc(100% + xs)`, full width, `elevation.overlay`, own scroll,
  `--cantos-web-search-layer`), closed `visibility: hidden` + `pointer-events: none`; the pill
  keeps its rest radius and focus stroke. Forced-colors adds a border.
- `fields.rs`: the dock is a sibling of the shell, after the messages, so an error message stays
  visible above it. `data-expanded` moves onto the dock.
- Theatre: outside-press and clamp measure `.cui-field`; focus leaving the field (Tab onto a
  chip the layer would otherwise cover) collapses it; unfiltered copy no longer repeats; no empty
  `<p>`; rows show the real title (with content `lang`) and real kind; the tagline heading
  inside the layer is gone; the results region is named by the search label.
- A narrow, large-text relief for the header (below 16rem of viewport).

## Verification

| Layer | Result |
| --- | --- |
| Shared tests | `cargo test --locked -p cantos-ui --test theatre_expression`: PASS (19). New: prompt stays in one row and no Theatre rule spans `1 / -1`; layer out of flow, anchored, layered; DOM test that the layer follows the shell, not inside it. |
| Theatre tests | `cargo test --locked -p cantos-theatre --features preview --lib public_home`: PASS (27), including two new copy tests. |
| Browser, 1280 dark VI | open/close identical layout; Escape restores focus and hides (`inert`, `visibility: hidden`); typed `đèn` + Enter shows one row; press inside the layer keeps it open, outside press closes it; two catalogue requests total (initial + submit), none on open/close. ArrowDown focuses the first result; Tab out onto a chip collapses. |
| Browser, 390 dark VI and 320 light VI | layer under the pill, own scroll, no horizontal scroll, bottom clamped to the viewport. |
| Narrow large-text relief | viewport 250 px (the `max-width: 16rem` branch) wraps the prompt in two lines; **real OS/browser text scaling was not available**, so 200% text is NOT_RUN. An inline root `font-size: 200%` shows the pre-existing one-letter prompt on both base and patch, and does not trigger the rem media query. |

Captures (JPEG, scaled by the browser pane; opened when taken):
[base 1280 dark](search-dock-overlay-604/before-1280-dark-vi-open.jpg) ·
[patch 1280 dark](search-dock-overlay-604/after-1280-dark-vi-open.jpg) ·
[patch typed query](search-dock-overlay-604/after-1280-dark-vi-typed-den.jpg) ·
[patch 390 dark](search-dock-overlay-604/after-390-dark-vi-open.jpg) ·
[patch 320 light](search-dock-overlay-604/after-320-light-vi-open.jpg).

## Not run / residual

- NOT_RUN: Vietnamese IME composition in a real browser (logic unchanged and unit-tested),
  virtual keyboard/`visualViewport`, forced colors, reduced motion, English long labels, 1440,
  Safari/Firefox, screen reader.
- There is no flip above the field: with little room below, the layer keeps a 10 rem floor and
  the page scrolls.
- The empty clear slot still reserves 52 px, so at 390 px the prompt wraps to two lines (unchanged).
- Theatre CMP and Studio do not consume this CSS or `SearchDock`; the gallery specimen
  (`theatre-docked-search`) uses the same shared component and was not opened in a browser.
- The UI qualification records for the changed shared files are not rebound here.
