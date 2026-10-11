# Shared semantic contract v0.2.0

[`tokens.json`](tokens.json) is the sole active source. This is a bounded Cantos JSON schema,
not a claim of DTCG conformance. The generator validates it before any output write.
The immutable [v0.1.0 snapshot](legacy/studio-tokens-0.1.0.json) is historical migration evidence;
it is not a live second theme.

| Field | Contract |
| --- | --- |
| `version` | Semantic version of token values and role/API contract |
| `tokens` | Theme-independent semantic roles, each `{type, value}` |
| `themes` | Exactly light/dark; identical roles and types |
| `aliases` | Legacy semantic role → direct canonical role; no chains or cycles |
| `contrast_pairs` | Named foreground/background color roles with text/nontext floor |
| `density` | Compact/comfortable logical control dimensions; common roles, no per-app palette |
| `reduced_motion` | Motion durations replaced by zero under the browser preference |

Supported types: `color` (sRGB six-digit hex), `dimension` (non-negative logical units),
`number` (line-height or opacity), `fontFamily`, `fontWeight`, `duration` (milliseconds),
`cubicBezier` (bounded effects curve), and structured `shadow` (offsets/blur/color/opacity).
Web maps dimension units to rem at 16 logical units per rem. Typography line height is unitless;
weights are integer values; durations map to ms. CMP will select dp/sp mappings separately.

Roles map mechanically to CSS custom properties and generated Rust `Token` variants.
`Token::css_variable()` is the runtime contract; `css_value(Theme::Light|Dark)` provides static
resolved CSS values. `Theme::System` returns the light default for this diagnostic API; browser
preference resolution belongs to CSS. Static token values do not resolve density or reduced
motion overrides: read runtime computed CSS for those observations.

## Decisions from the reference

The authenticated design package reviewed on 2026-10-11 supplies identical Lavender Ink palettes
for both products, Noto Sans UI / Noto Serif editorial names, 14/1.55 Web body, 14/1.9 editor,
44 logical-unit Web targets, 48 for comfortable/touch intent, 140ms feedback and proposed 260ms
navigation. Paper, lavender and muted gold preserve the quiet historical/fantasy character.
No embedded font or artwork bytes are redistributed; those files still need rights review.
The Web uses local/system fallbacks when the named fonts are unavailable.

The generator keeps the 8-step rhythm without renaming established Studio spacing roles:
`xs=4`, `sm=8`, `inset=12`, `md=16`, `section=20`, `lg=24`, `xl=32`, `xxl=36`.
Reading/container/overlay/action shapes are separate roles (12/18/24/24), rather than a card
around every line. Elevation is reserved for overlap; zero base shadow keeps reading quiet.
600/840 logical-unit medium/expanded breakpoints are foundation decisions, not copied app
layout thresholds. They guide composition, while actual pane constraints belong to app owners.
CSS media queries cannot consume custom properties: the generator substitutes named breakpoint
placeholders in the [stylesheet template](../leptos/foundation.css.in) from the same source.

Effects easing and hover/pressed opacity are bounded foundation choices; they are not claimed
to be pixel-exact reference values. Reduced motion removes transition duration without delaying
dispatch. Status text uses error/warning/success **on their container** roles; outlines are
boundaries, not body text. Disabled Button uses a full-contrast tonal appearance and a visible
reason instead of applying the reference opacity to text. Hover and press overlays use current
foreground color; rendered contrast and browser `color-mix` remain runtime review tasks.

All token pairs are checked in both themes. This does not measure text over art, transparency,
disabled opacity, focus occlusion or forced-colors rendering. Provenance and exact executed
checks are in the [foundation evidence](../../../docs/evidence/ui-system-foundation.md).
