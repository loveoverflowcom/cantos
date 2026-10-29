# Search Dock refinement: no collapse control, aligned rows, calm states (#604 AC-10..AC-14)

Source checkpoint: 2026-10-29, base `origin/develop` `f8c54495` (includes the #606 overlay).
Development evidence for [#604](https://github.com/loveoverflowcom/cantos/issues/604), not
production acceptance. The owner's two design images (`image(20261029-072153).png` before,
`image(20261029-072201).png` target) are chat attachments that this work did not receive, so no
pixel comparison against them was made; the observations below are of the patched build only.

Browser observations come from the in-app Chromium pane driving the real Theatre Web WASM (live
mode, `CANTOS_API_ORIGIN=http://127.0.0.1:8080`) against a **synthetic** loopback catalogue stub
with twelve fictional Works (one with a 110-character title) and null covers. No Server, Identity,
auth or cover policy was exercised.

## Change

| Item | Result |
| --- | --- |
| Collapse control | `theatre-search-collapse` and its `Thu gọn` copy are removed from Theatre Web, with no stand-in close icon. Closing stays with Escape (focus returns to the input), an outside press and focus leaving the field. The shared Web catalogue key `theatre.shell.search_collapse` and the CMP `search-collapse` control are untouched. |
| Header | `theatre-search-dock-heading` is gone. The context line is one `<p class="theatre-search-dock-context">` rendered only for a narrowed catalogue, so an unfiltered layer starts with its first row. |
| Alignment | The shared scroll box pads `space-sm`; each row and the context add `field-inset − space-sm` through `--theatre-search-row-inset`, so a row title starts exactly under the prompt, also under the narrow-viewport inset override. |
| States | Resting rows have no fill (nothing is preselected). Hover paints the 0.08 state layer for a real pointer only (`@media (hover: hover)`), keyboard focus paints the 0.12 layer and the focus ring, a press paints the pressed layer. There is no selected row because no selection state exists. |
| Scrollbar | A fine pointer gets `scrollbar-width: thin` and a 55 % tonal thumb on a transparent track; touch keeps its overlay bar and forced colors `scrollbar-color: auto`. No `::-webkit-scrollbar` rule, so the standard properties stay in force. |
| Preview | The `theatre-docked-search` specimen no longer shows a collapse button. |

## Verification

| Layer | Result |
| --- | --- |
| Shared tests | `cargo test --locked -p cantos-ui --test theatre_expression`: PASS (20). New: inset, fine-pointer scrollbar, forced-colors fallback, no webkit scrollbar. |
| Theatre tests | `cargo test --locked -p cantos-theatre --features preview --lib public_home`: PASS (28). New: no collapse id or copy in any state and locale; no heading or context element for an unfiltered catalogue; context is a `<p>`. |
| Browser 1280 dark VI | Pill and layer share left/right edges (232/961); row title x equals the prompt x (256 = 232 + 24); no collapse control; no context when unfiltered; hero height 339 px open, closed and with a typed query; no horizontal scroll. ArrowDown: only the first row has a fill (0.12) plus ring; hovering another row gives only it the 0.08 fill. Escape closes and returns focus to the input; a press outside closes; a real click on the input opens; typing `đèn` + Enter shows the compact context line and three rows. A press on a result navigates to the Work. |
| Browser 390 light VI | Geometry only: pill and layer 40–350, row title x = prompt x (64), scroll box clamped to 195 px (bottom 607 of 760), scrolls on its own, no horizontal scroll. |

Captures (opened when taken): [focus and hover](search-dock-refinement-604/after-desktop-dark-vi-focus-and-hover.jpg) ·
[typed query](search-dock-refinement-604/after-desktop-dark-vi-typed-den.jpg).

## Not run / residual

- NOT_RUN: a same-scenario before capture at 1280 (the #606 before image is another build), the
  owner images, 1440, 320, EN with long labels, 200 % text, forced colors, reduced motion, virtual
  keyboard, Vietnamese IME, screen reader, Safari/Firefox, light theme at 1280.
- The 390 px viewport capture was unreliable in the browser pane (screenshots lagged the page), so
  only its measured geometry is recorded, not an image.
- The context copy still reads `… · Tất cả loại` for a text-only query; that is #606's copy and was
  left alone.
- Theatre CMP keeps its own `search-collapse` control: a parity gap for a separate owner decision.
- The UI qualification records of the changed shared files are not rebound here.
