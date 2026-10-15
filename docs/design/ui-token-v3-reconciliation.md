# Design v3 token reconciliation

[Issue 141](https://github.com/loveoverflowcom/cantos/issues/141) supplies a
[token proposal](https://drive.google.com/file/d/1vMxSeIx4eqvzAYmUMlDXtiieqmtBuCe1/view)
dated 2026-10-15. Its bytes have SHA-256
`2b5952b4db0f79a73a17ff5aa037ed39cc1ddd60c27ae5a35c72edb14754be05` and version
`0.5.0-design-proposal`, derived from the older canonical v0.4.0. It is design input, not a
second active token source. The sole canonical input remains
[`packages/ui-system/shared/tokens.json`](../../packages/ui-system/shared/tokens.json).

## Implemented value delta: canonical v0.6.0

| Canonical role | Previous | Adopted | Consumer and reason |
| --- | --- | --- | --- |
| `type.body.size` | 14 logical units | 16 logical units | General UI explanations, lists and preview anatomy gain readable body text; editor retains its dedicated 14-unit role and 1.9 line height |
| `size.control`, compact override | 44 logical units | 48 logical units | Regular shared controls now retain the same minimum target at both densities; compact density still uses 12-unit inline padding versus comfortable 16 |

These are semantic values with existing renderer mappings, not a CSS-pixel-to-native-unit
conversion. The generator emits body `1rem` / control `3rem` for Web, and logical 16/48 for
CMP's existing sp/dp adapters. Browser zoom and injected CMP platform density/font scale stay
in charge. Native theme tests cover nested scale and minimum target restoration; generated
inventory checks are separate from those runtime composition tests.

All light/dark colors, contrast pairs, eight spacing steps, type weights/line heights, editor
size, motion/reduced-motion, outlines, auth geometry, and existing aliases remain unchanged.
No role is removed or renamed. In particular, the proposal's older baseline omits nine v0.5.0
additions: `size.authLogo`, `size.statusSymbol`, `shape.statusSymbol`, and all three members of
`type.authHeading` and `type.membershipTitle`. Canonical v0.6.0 retains all nine.

## Proposed roles and constants

| Proposal input | Resolution in this change | Reason / future owner |
| --- | --- | --- |
| `size.minimumTouch=48` | Reuse `size.control` and density mapping | Duplicate target meaning; no new global token |
| `type.editorialHeading.size=28` | Reuse existing `type.headline` | An incomplete size-only type role cannot replace family, line height and weight; add a separate role only for a concrete distinct consumer |
| `shape.componentField=20`, `size.componentField=56` | Reuse scoped `shape.authField` / `size.authField` for auth specimens; regular fields keep their existing mapping | Do not silently turn every editor/regular field into an auth field |
| `shape.componentChip=12` | Defer; existing Web control and CMP action mappings remain | A proposed chip subtype needs a bounded variant/state contract and two-renderer admission |
| `shape.componentIconButton=16` | Defer; existing genuine circle geometry remains | A radius does not establish a new icon-button variant or hit target |
| `shape.componentReading=16` | Defer; retain existing quiet reading mapping | Do not globally alter editor/reading containment for one board |
| `shape.componentThumbnail=16` | Defer; preserve canonical CMP thumbnail 10 and existing Web artwork mapping | Existing exception is intentional; a new cover consumer must declare its anatomy first |
| Web `shape.componentControl/Card/Overlay=24/28/32` | Existing Web control 24 is retained; proposed card/overlay replacement remains deferred | Preserve shipped Web shape scale 24/12/18/24 plus panel 24; no unreviewed global corner change |
| CMP `shape.componentControl/Card/Overlay=20/24/28` | Deferred | Preserve existing CMP 4/8/12/16/20 scale, thumbnail 10 and explicit auth variants; Web geometry cannot override native geometry |
| Web/CMP `size.primaryAction=56/52` | Deferred; existing regular and scoped auth actions remain | Requires a concrete primary-action variant and independent renderer acceptance |

The proposal's `cssAliases.theme`, `cssAliases.platform` and `cssAliases.global` are short-name
presentation conveniences for its HTML export. None becomes a canonical alias or app-local
palette. Every theme short name maps to its existing paired canonical color; all platform shape
and action short names retain the deferred status above; global type, target, stroke, focus and
motion short names resolve through existing canonical roles. The only existing semantic alias,
`shape.field → shape.reading`, remains a direct compatibility alias. Preview-local layout
geometry belongs to the preview owner and must carry a name, unit, scope and reason.

## Font delivery and evidence boundary

The existing Leptos preview now loads the [licensed local Noto bundle](../../packages/ui-system/shared/assets/noto-fonts/README.md)
using exact existing Theatre CMP font bytes. The family names and canonical type weights are
unchanged. Full font files retain Vietnamese letters and combining marks; source tests pin
hashes, license, face declarations and glyph coverage. Browser font readiness, requested
weight selection and opened captures remain separate observations.

CSS, typed Rust, foundation CSS and generated Kotlin inventory are regenerated together.
Token changes invalidate previous source-bound qualification; registration does not grant
adoption. `cargo xtask verify repository`, `cargo xtask ui-preview leptos verify`,
`cargo xtask ui-preview cmp verify` and `cargo xtask preflight --changed-only` retain independent
results. This reconciliation records implemented values and deferred design decisions; it
does not establish Android/iOS, screen-reader, product rollout or pixel qualification.
