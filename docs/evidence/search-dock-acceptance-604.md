# Search Dock acceptance and chip/dock decoupling (#604)

Source checkpoint: 2026-10-29, base `origin/develop` `223f1c22` (includes the #606 overlay and the
#622 refinement). Development evidence for [#604](https://github.com/loveoverflowcom/cantos/issues/604),
not production acceptance. Nothing here ticks an acceptance criterion by itself; the issue owner
reconciles those against this record.

Browser observations: Google Chrome 156 (headless, macOS) driven over CDP, real Theatre Web WASM
(`trunk serve`, debug build, `CANTOS_API_ORIGIN=http://127.0.0.1:8080`) against a **synthetic**
loopback catalogue of twelve fictional Works (one 110-character Vietnamese title, one long English
title, null covers) that also answers the session probe with a guest `401`. No Server, Identity,
auth or cover policy was exercised. The IME is **CDP-simulated** (`Input.imeSetComposition`), not an
OS input method. Captures are cropped viewport PNGs converted to JPEG; the harness and stub are
scratch tooling, not committed.

## Findings and fixes

| # | Finding (observed on the base build unless noted) | Fix |
| --- | --- | --- |
| 1 | The overlay clearance `--cantos-web-search-available` was measured only on mount, resize and scroll. When the field moved for any other reason, the layer hung below the fold: 390×844 dark VI, dock bottom 858 vs viewport 844; 320×640, dock bottom 772 vs 640. | `platform/search_dock.rs`: a `ResizeObserver` on the panel and `body` re-measures; opening and a visual-viewport resize call one `make_room` that scrolls the page by the overshoot, never past keeping the field 8 px below the top. A plain page scroll never triggers it, so it cannot fight the reader. A second pass after 250 ms covers the browser's own scroll-to-focus (at 200 % text, 1440×900 the first pass alone left the dock 29 px below the fold). |
| 2 | A press on the layer's own padding moved focus to `<main tabindex=-1>` (`relatedTarget` non-null), so the layer closed and the caret left the input. | `press_on_layer_background` cancels `mousedown` default on non-interactive layer background; focus stays on the input or the pressed result. |
| 3 | `ArrowDown` on a result did nothing (Arrow keys only entered the list from the input). | `ArrowDown`/`ArrowUp` step through result buttons; above the first returns to the input; ends do not wrap; IME guard unchanged. |
| 4 | Selecting a kind chip opened the dock (`FilterChip` → `submit()` → `expanded.set(true)`): the layer then covered the other chips, and the next chip press landed on a result (base run: pressing Story navigated to a Work). | `apply(ApplyTrigger)`; only `Search` (Enter, magnifier) opens the layer, `KindChip` never does (`opens_layer`). |
| 5 | The `Xóa tìm kiếm và bộ lọc` button (`theatre-discovery-clear`). | Removed with its `filtered` state and the now-unused `restore_disappearing_action_focus`. The text is cleared by the field's Clear, the kind by the *All types* chip. The catalog key `theatre.shell.discovery_clear` is **kept** (owner: localization catalog; generated into every consumer). |

## Verification

| Layer | Result |
| --- | --- |
| Theatre unit/SSR | `cargo test --locked -p cantos-theatre --features preview --lib public_home`: PASS (30). New: `only_searching_opens_the_layer_never_a_kind_chip`, `arrows_enter_the_list_from_the_field_and_step_through_it_from_a_result`; updated: the form offers no clear-all action in vi-VN and en, Clear stays, one pressed chip. SSR cannot prove events; the decision is a pure function and the events are in the browser rows below. |
| Behavior SSOT | `specs/behavior/catalog.json` registers no Discovery/Search/filter capability (only `discovery-retired-language`, whose query contract is unchanged: language stays `""`). Presentation-only; no SSOT change. |
| Geometry matrix (before → after) | 1280 dark/light × VI/EN, 1440, 390 dark VI / light EN, 320 dark VI / light EN: after, all invariants hold (layout identical open vs closed, overlay absolute and 4 px under the field with equal left/right edges, dock bottom ≤ viewport, field in view, label on grid row 1, no collapse control, no empty `<p>`, no horizontal scroll, no row overflow, resting row unfilled, Escape closes with focus kept and `aria-expanded=false`/`inert`). Before: 390 dark VI, 320 dark VI and 320 light EN fail "dock in viewport". |
| Kind chip matrix (1280 dark VI, 1280 light EN, 390 dark VI) | Dock closed, click Novel/Story/Podcast/All: dock stays closed, one `works?kind=…` request each, matching cards, one `aria-pressed`. Keyboard Enter and Space on a focused chip: dock stays closed, filter applied. Base: Novel opened the dock. On 1280 dark VI only: focus/click opens, typing opens, Enter and magnifier search (one request each, query kept), Escape and outside press close with the query intact, clear-input + Enter drops the text filter, text + kind sends both and replaces rows (no stale rows), pressing a chip with the layer open closes it. |
| Interaction | Open/close add no `works` request. CDP IME: Enter while composing → 0 requests; after commit Enter → 1. Padding and scrollbar-edge presses keep the layer open and keep focus. Tab out of the field closes it. A result press navigates to the Work. |
| States (1280 dark VI) | error (message + Retry, after dismissing the app's own error dialog), empty, no-match, partial page: each renders in the layer without overlap. |
| Media | forced-colors: layer gets a 1 px border; reduced-motion: transition durations 0 s; 200 % text (Chrome default font 32 px): at 1280 and 1440 the layer stays under the field and in the viewport, at 390 the page scrolls it into view. |
| Simulated keyboard | 390 wide, viewport height 844 → 600 → 420 → 320: the page scrolls so the layer bottom equals the viewport bottom and the field stays visible. This is a viewport resize, **not** a real virtual keyboard. |

Captures: [1280 dark VI](search-dock-acceptance-604/d1280-dark-vi-open.jpg) ·
[1280 light EN](search-dock-acceptance-604/d1280-light-en-open.jpg) ·
[forced colors](search-dock-acceptance-604/d1280-forced-colors-open.jpg) ·
[ArrowDown focus](search-dock-acceptance-604/d1280-dark-vi-arrowdown-focus.jpg) ·
[chip Podcast, layer closed](search-dock-acceptance-604/d1280-dark-vi-chip-podcast-dock-closed.jpg) ·
[390 light EN](search-dock-acceptance-604/m390-light-en-open.jpg) ·
[320 light EN](search-dock-acceptance-604/m320-light-en-open.jpg) ·
[320 dark VI](search-dock-acceptance-604/m320-dark-vi-open.jpg).

## Not run / residual

- NOT_RUN: a real OS Vietnamese IME, a real virtual keyboard/`visualViewport`, screen reader,
  Safari/Firefox, a coarse-pointer scrollbar (CDP could not emulate `pointer: coarse`; the
  `(pointer: fine)` gate is covered by the shared CSS test), the owner's two chat images, the
  deployed site, and any comparison against production data.
- 200 % text on a ≤ 390 px viewport: operable, no horizontal scroll, but the prompt wraps inside a
  field narrowed by rem-sized chrome (one to two letters per line at 320). Pre-existing; not changed.
- At 320×640 two of nine first synthetic clicks on the field missed it (focus landed on `<main>`) and a
  retry opened it; the cause (harness timing against a late layout settle vs. a product race) is not
  isolated.
- The context line still reads `… · Tất cả loại` for a text-only query: a shared localization
  message (`discovery_displayed_context`), left for the catalog owner.
- The `discovery_clear` catalog key is now unused by Theatre Web; removing it touches the generated
  catalog for every consumer and is a separate catalog change.
- Theatre CMP keeps its own `search-collapse` control and was not inspected for the chip/clear-all change (parity gap, not touched).
- The shared UI qualification records are not rebound here (the `repository` gate is red on
  `origin/develop` for that reason).
