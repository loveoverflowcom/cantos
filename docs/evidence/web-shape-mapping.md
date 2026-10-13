# Web shape mapping: restoring Leptos corners

## Input, invariant and ownership

Report: after [Theatre CMP shape normalization](theatre-cmp-shapes.md) (PR #86, merge `7f02e65`),
Leptos Theatre buttons, navigation selection, cards and mini-player lost their soft corners.
Keep CMP at its normalized 4/8/12/16/20 scale and restore the Web corners.

Invariant: Web corner geometry does not depend on the CMP shape scale. Owner:
`packages/ui-system/leptos/foundation.css.in` (the Web stylesheet template) declares one set of
Web-only roles; Web CSS reads them. Canonical `tokens.json` (v0.2.2), `tokens.css`, `tokens.rs`,
the preview catalog and every CMP output are unchanged.

## Root cause (confirmed)

PR #86 changed the shared `shape.*` values (control 24→8, container 18→16, overlay 24→20; reading
stayed 12) to normalize CMP. Leptos consumed the same custom properties
(`--cantos-shape-control` and the others), so the CMP-density change reached Web with no Web
decision. Evidence: the same CSS selectors rendered 8px buttons at the base commit (measured), and
the PR changed only token values, not Web CSS.

## Design decision

| Option | Result |
| --- | --- |
| Revert shared values | rejected: breaks the CMP contract the user asked to keep |
| Override `--cantos-shape-*` in Web | rejected: makes computed canonical variables misleading |
| New section in `tokens.json` | rejected: bumps the shared version and regenerates Kotlin for a Web-only fact |
| **Web-only `--cantos-web-shape-*` roles in the Leptos template** | chosen: one documented source, canonical variables stay truthful, CMP untouched |

Roles: `control` 1.5rem (24), `reading` 0.75rem (12), `container` 1.125rem (18), `overlay` 1.5rem
(24); the historical v0.2.0 values, not new design. `shape.field` was an alias of `reading`, so
fields use the Web `reading` role. `shape.pill` stays shared: circles, filter chips, avatars and
choice controls are unchanged. `shape.detail` and `shape.thumbnail` have no Web consumer.
Border widths, spacing, type, color, states and focus width are untouched.

Generator: `scripts/ui_tokens.py` now accepts custom properties declared by the template only if
they start with `--cantos-web-` and do not shadow a canonical role; any other declaration is a
build error. Only `foundation.css` changed among generated files.

## Before / after (computed `border-top-left-radius`, Theatre mock at `?mode=mock`)

"Before" is `origin/develop` `df7d62f` built and measured; "after" is this branch built and measured.
Rows marked † were not measured before; the value is the pre-fix source mapping.

| Element | Role | Before | After |
| --- | --- | --- | --- |
| `.theatre-action` (primary, tonal, text), `.cui-button` | control | 8 | 24 |
| `.theatre-nav-link` (selected and idle) | control | 8 | 24 |
| `.theatre-chip`, `.theatre-text-link` | control | 8 | 24 |
| `.theatre-mini-player` | overlay | 20 | 24 |
| `.theatre-feature` | overlay | 20 | 24 |
| `.cui-dialog` † | overlay | 20 | 24 |
| `.theatre-continue`, `.theatre-cover`, `.theatre-cover-link`, `.theatre-section-empty` | container | 16 | 18 |
| `.cui-surface`, Studio `.session-panel`, `.live-status`, `.authoring-inspector` † | container | 16 | 18 |
| `.theatre-input`, `.theatre-demo-notice`, `.cui-field-control`, `.cui-badge`, `.cui-status` | reading | 12 | 12 |
| avatar, cover moon, `.cui-filter-chip`, choice controls | pill | 999 | 999 |
| border width (input 1px, secondary button 1px) | n/a | 1 | 1 |

Reading did not change in #86, so text fields and notices needed no repair. The feature card's
inner cover keeps `border-radius: 0` so the artwork joins the card edge.

## Verification by layer

| Layer | Result |
| --- | --- |
| L1 architecture | Source trace above; Web consumers (foundation, gallery, Theatre, Studio) read only Web roles and `shape.pill`; no CMP file changed (`git diff` has no `cmp/`, `mobile/`, `tokens.json`, `tokens.css`, `tokens.rs`) |
| L2 domain/types | not applicable: presentation only |
| L3 regression | `scripts/test_ui_tokens.py` `WebShapeIndependence` and template-refusal tests; mutation: pointing one Theatre rule back at `--cantos-shape-control` fails `test_web_components_use_web_shape_roles_never_the_shared_cmp_scale` |
| L4 application/mocked | Theatre Web mock (session-only fixture) and Leptos gallery built and driven in the in-app browser |
| L5 real renderer | Chrome in the Claude browser pane: computed radii above; dark and light; 1280 and 390 wide; keyboard focus ring (3px, follows the 24px corner); no horizontal overflow; Vietnamese text wraps. No native device and no authenticated Studio session |

Same-scenario captures (headless Chrome, light and dark, 1280x800 and 390x844, Discover and
Library, before and after) were opened and compared; the pane additionally drove the after build in
dark and light. The before dark Discover capture reproduces the reported 8px buttons and selection.
The PNGs are ignored local artifacts, not committed.

Not verified: Studio authenticated surfaces (editor, production views) were not driven; Theatre
playback and the live API are out of scope; Safari/Firefox rendering; a CMP rebuild (no CMP file
changed, so its prior evidence in [theatre-cmp-shapes.md](theatre-cmp-shapes.md) stands).
