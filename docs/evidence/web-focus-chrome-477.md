# Theatre Web focus chrome — #477

Status: implemented in the Theatre Web stylesheet only (shared Leptos source untouched);
**bounded Chromium observations, acceptance pending**. Related:
[#477](https://github.com/loveoverflowcom/cantos/issues/477), [Floating fields evidence](web-floating-fields-423.md).
Base `cec90f7a0b906b61649f5d986e882f288f92463b`. Screenshots stay out of Git (attached to the PR).

## Finding and cause

Painted controls drew a 3-unit focus ring **4 units outside** their shell, leaving a visible dark
gutter between the fill and the ring. Two owners did it:

- the shared `+4` outset offset (`.cui-button`, `.cui-nav-item`, `.cui-filter-chip`) and the Theatre
  generic `.theatre-app :where(a, button, input, …):focus-visible` rule, and per-class copies for
  the brand link, public work card and diagnostic preview;
- the Theatre generic rule has the same specificity as the shared primitive rules and loads later, so
  it **overrode** the compact language Select's inset perimeter with an outward ring (observed
  baseline: Select trigger `outline 3px, offset 4px`; correct owner rule is `2px, offset -6px`).

`:hover` and `:active` never drew a stroke, and the Floating field shell and trailing action were
already correct; they are unchanged. `size.focusOffset` in `tokens.json` is **not** changed (Studio
and CMP consumers keep it).

## Change

| Owner | Change |
| --- | --- |
| `apps/cantos-theatre/web/src/styles/theatre.css` | generic fallback becomes zero-specificity (`:where(...)`) so shared primitives win; one flush (`outline-offset: 0`) rule for painted shells (`.cui-button`, `.cui-nav-item`, `.cui-filter-chip`, action, chip, text link, back, nav link, brand, avatar, account row, auth brand, skip link); auth brand, public work card and diagnostic preview use `0`; the current-page avatar ring yields to focus (`:not(:focus-visible)`) so one ring shows |
| `packages/ui-system/leptos/foundation.css.in` (+ generated `foundation.css`) | **Not changed.** Theatre Web does not set `Expression::Theatre`, and the shared source digest is bound to preview qualification records; editing it would stale all twelve Leptos qualifications without new captures. The same flush rule for the `theatre` expression is a follow-up that needs the preview re-qualification |

## Inventory

| Control | Result |
| --- | --- |
| Primary/tonal/text/auth buttons, Login/Register CTA, dialog and retry actions | CONFIRMED, fixed: ring flush |
| Filter chips, nav links, brand, skip link, avatar link | CONFIRMED, fixed: ring flush; avatar current ring and focus ring never stack |
| Language Select trigger (compact) | CONFIRMED (cascade override), fixed: inset 2 at the 40-unit surface again |
| Floating TextField / search shell, invalid, focused-invalid | NO_GAP: single inset stroke on the shell, control `outline: none` |
| Field trailing action (password eye, search action) | NO_GAP: already `outline-offset: 0`, inside the shell, no shell ring |
| Public work card (`:has` link focus) and auth brand | CONFIRMED, fixed |
| Account menu rows | fixed with the shared list (flush) |
| Choice (checkbox/switch) and range controls, plain inline text links | INTENTIONAL_EXCEPTION: not painted shells; keep the outset ring for recognisable focus |
| `?mode=auth-preview` fixture buttons above the app | INTENTIONAL_EXCEPTION: dev-only diagnostics outside `.theatre-app`, not in a production bundle |

## Observations (Chromium in the Claude browser pane, `cargo xtask app cantos-theatre --web` with `TRUNK_BUILD_FEATURES=preview`, `?mode=auth-preview`)

Computed `outline-offset` on the real focused element after a real Tab keypress (modality), before
→ after:

| Element | Before | After |
| --- | --- | --- |
| Đăng nhập / Tạo tài khoản / Tìm / Cập nhật danh mục (`.cui-button`) | 4px | 0px |
| `.theatre-chip` ×4, `.theatre-nav-link` ×2, `.theatre-brand`, `.theatre-skip`, `.theatre-avatar` | 4px | 0px |
| Language Select trigger | 3px / +4 (outward) | 2px / −6 (inset) |
| Search input (Floating) | shell 2px / −2 | unchanged |

Dark 1024×768 screenshots opened before/after for the Login CTA and a filter chip (halo gutter
visible before, ring touching the fill after) and for the avatar and search field after.
At 375×812 light, the avatar, nav link, chip, button and Select report the same after-values and
`scrollWidth == innerWidth` (no overflow).

## Regression oracles

- `cargo test -p cantos-theatre --features preview painted_controls` — every Theatre
  `:focus-visible` offset except the documented fallback is flush or inset; the avatar current ring
  yields to focus.
- These read stylesheet text; the computed geometry above is the behavioural observation.

## Not run

Forced colors, reduced motion emulation, 200% zoom, vi/en long-label matrix, real screen reader,
Firefox/Safari, light-mode screenshots (computed values only), 1280/1440 widths, the signed-in
account menu and logout dialog (no API session), disabled/pending states beyond computed rules, and
CMP (out of scope). Theatre Web does not yet set `Expression::Theatre` on its `UiRoot`, so its fix
lives in `theatre.css` under `.theatre-app`; the shared `cui-*` Theatre-expression stylesheet keeps
the outset focus offset until a re-qualified follow-up. Studio Web was not rendered and none of its
source changed.
