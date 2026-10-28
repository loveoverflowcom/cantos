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
