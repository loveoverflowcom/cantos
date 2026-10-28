# Studio Expressive parity — #559

Source-bound observation, 28 October 2026. Part of [#559](https://github.com/loveoverflowcom/cantos/issues/559).
Base `develop@6cacdb4749871215bbc6cedd578cb70fa4776867`. It records what ran; it does not accept the
workspace, editor, production or account surfaces, which need an authorized Studio session.

## Decision

Studio selects the existing `theatre` expression on every `UiRoot` (`STUDIO_EXPRESSION` in
`apps/cantos-studio/web/src/ui.rs`). No second profile, hex or token was added.

Audit of the `[data-expression="theatre"]` rules in `foundation.css.in` against Studio consumers:

| Rule family | Studio consumer | Effect |
| --- | --- | --- |
| palette and weight remap (`tokens.css`) | every semantic role | applies: lilac container, plum selection ink, cream/dark background, 700 headings and labels |
| floating field geometry and tonal fill | sign-in and account fields | applies: 68 px minimum, 16 px radius |
| compact select pill | topbar language select | applies, same pill Studio already set locally |
| tonal button and tonal icon button | tonal actions, theme cycle | applies |
| docked search, filter chip, guest profile card, account tile | none | no match in Studio |

`primaryContainer` is selection ink under this expression, so the entry art panel changed from
`primaryContainer` to the hero pair (`theatreHero` / `theatreOnHero`), as Theatre's auth art does.
Other Studio rules read semantic roles only and were left alone.

## Observations (actual Studio Leptos build, `trunk serve`, built-in browser)

Unauthenticated entry (session unavailable state), Dark system scheme at 1280 wide and Light at 390 wide.
The "before" state is the same build with `data-expression` removed and the art background set back
to `primaryContainer`; it is an emulation of Standard, not a build of the base commit.

| Measure | Standard (emulated) | Expressive (built) |
| --- | --- | --- |
| `data-expression` | none | `theatre` |
| Entry art fill (Dark) | `rgb(81,59,99)` | `rgb(53,33,66)` (`theatreHero`) |
| Page background (Dark) | neutral near-black | `rgb(33,17,45)` |
| Field minimum height / fill | 58 px / `rgb(60,39,72)` | 68 px / `rgb(60,39,72)` |
| Heading weight | 400 | 700 |
| Sign-in action fill (Dark) | `rgb(237,153,222)` | unchanged: the auth action keeps its own role |
| Light 390: page / CTA | not captured | cream `rgb(255,242,233)` / plum, no horizontal overflow |

## Checks

| Layer | Command (cwd) | Result |
| --- | --- | --- |
| Studio unit | `cargo test --locked -p cantos-studio expression_tests` (`apps/cantos-studio/web`) | PASS, 2 tests |
| Token contract | `python3 -m unittest test_ui_tokens` (`scripts`, 3.14 shim) | PASS, 46 tests |
| Wasm build | `trunk serve` (`apps/cantos-studio/web`) | PASS |

## Not covered

- Authorized workspace: dashboard, metadata/editor, authoring, production, inspector, account popover.
- 320 px, 200 % text, forced colors, reduced motion, English copy, keyboard walk and measured contrast
  of Studio-specific compositions beyond the generator's pair floors.
- Hover/pressed/disabled/loading states in the browser; the Light/Dark theme-cycle button path.
- Deployed or production browser acceptance; the owner screenshot's build and computed CSS remain unknown.

## Compact Floating field density (AC-07 slice)

Source-bound observation, 28 October 2026, base `develop@89c5964c` plus this change (uncommitted at
record time). Part of [#559](https://github.com/loveoverflowcom/cantos/issues/559).

Finding: under the `theatre` expression every Floating field has a 68-unit minimum, and the authenticated
work, unit, rights, publication and import forms use ~20 of them. The existing Select density is
select-only and the expression remap is root-scoped, so no shared way to ask for the denser, still
accessible field existed.

Decision: a shared `FieldDensity` on `TextField` (`Compact` renders `data-field-density="compact"`; the
default adds no attribute, so every existing consumer's markup is byte-identical). One foundation rule
restores the standard Floating geometry already declared at the top of `foundation.css.in` (58 minimum,
above the 48-unit target) and leaves palette, weights, radius and state strokes to the expression. The docked
search is excluded. Studio selects it once (`EDITOR_FIELD_DENSITY`) for its authenticated Floating fields;
sign-in and the account menu stay comfortable. No hex, token or profile was added and `studio.css` gained
no field override. Catalog state `web-compact-fields` (Web-only, like Select's `web-*` states) registers the
specimen.

| Layer | Command (cwd) | Result |
| --- | --- | --- |
| Shared renderer tests | `cargo test --locked -p cantos-ui` (root) | PASS (new: markup identity, one scoped rule) |
| Studio tests | `cargo test --locked -p cantos-studio` (root) | PASS, 405 + 5 (new: every authenticated Floating field names the constant; sign-in has none; no local override) |
| Format / lint | `cargo fmt --all -- --check`; `cargo clippy --locked -p cantos-ui -p cantos-studio --all-targets -D warnings`, native and `wasm32-unknown-unknown` | PASS |
| Generators | `python3.14 scripts/ui_tokens.py --check`; `ui_preview_catalog.py` (generate) | PASS |
| Python UI suites | `python3.14 -m unittest test_ui_tokens test_ui_preview_catalog test_ui_component_states test_ui_design_inventory` (`scripts`) | PASS, 92 |

Observation (actual `ui-preview leptos` gallery via `trunk serve`, built-in browser, light, 1280 wide,
Vietnamese fixtures): with a short label the Theatre Floating field computes `min-block-size` 68 px and the
compact one 58 px; radius 16 px and fill `rgb(234,221,247)` are identical, the error stroke, floated label and
read-only/disabled fills were visible. With the long Vietnamese fixture label both grow with their content
(144 vs 136 px at the narrow matrix column), nothing clips. The label text was shortened in the page DOM for
the minimum measurement only.

Pre-existing red, not introduced here: `ui_preview_catalog.py --check` already reports every Leptos
qualification stale and the dependent Studio/Theatre admission errors on clean `develop@89c5964c`. Editing
renderer sources keeps it red; renewing it needs the full gallery runtime inspection
([procedure](pr304-leptos-requalification.md)) and is not claimed here.

Not covered: authenticated Studio runtime (the work form was not rendered in the Studio build), dark mode,
320 px, 200 % text, forced colors, keyboard walk, measured contrast of the compact specimen, the other AC-07
recipe items (Select/action density in the editor, hierarchy of surfaces), peer composition from #131/#571/#570,
and deployed acceptance.
