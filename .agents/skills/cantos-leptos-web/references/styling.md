# Styling Leptos Web with tokens

> **Scope.** CSS architecture for Studio and Theatre Web: CSS custom properties as the runtime
> token API, component-owned SCSS or plain CSS, semantic class names, light/dark themes, reduced
> motion, focus visibility, container queries for panes, Vietnamese text in layout and the proposed
> static style gate. Use when creating or changing a stylesheet, a token consumer, a responsive
> rule, an animation or the class structure of a `view!`.

Design intent is owned by [`cantos-ui-design`](../../cantos-ui-design/SKILL.md):
[`material3-expressive.md`](../../cantos-ui-design/references/material3-expressive.md) decides
hierarchy and intensity, [`tokens.md`](../../cantos-ui-design/references/tokens.md) owns the token
source and naming, [`typography.md`](../../cantos-ui-design/references/typography.md) fonts and scale,
[`motion.md`](../../cantos-ui-design/references/motion.md) motion intent. Product token families are
in the [UI system](../../../../docs/design/ui-system.md#semantic-tokens). This reference says how a
Web stylesheet expresses those decisions; it does not choose colors, radii or curves.

## Ownership model

```text
token source (versioned, owned by cantos-ui-design)
    ↓ generated, never hand-edited, header names the source version
CSS custom properties   runtime token API: color, type, space, shape, elevation, motion roles
    ↓
component stylesheet    SCSS (or plain CSS) beside the component that owns it
    ↓
Leptos view!            structure, state and semantic class names
```

Generated `Token::css_variable()` and `tokens.css` define the implemented name mapping.
Regular Web shapes use the separate renderer roles documented by the
[shared contract](../../../../packages/ui-system/shared/README.md#web-shape-mapping); never
replace them with CMP radii. The examples below illustrate component structure.

## Rules

| Rule | Failure mode | Good | Counterexample | Oracle | Status / exception |
|---|---|---|---|---|---|
| No color literals in component styles | dark theme or a palette change misses a screen; contrast drifts | `color: var(--cantos-color-on-surface)` | `color: #4b3f72` | style gate (proposed); review | exception: the generated token file; `transparent`, `currentColor` |
| No radius literals; use shape roles | rounded-everything; pills on reading surfaces | `border-radius: var(--cantos-shape-pill)` on the speed chip | `border-radius: 14px` | style gate (proposed) | exception: `0` |
| Spacing/inset/gap/padding/margin, stroke, focus, elevation and motion consume semantic roles | inconsistent rhythm and boundaries between Studio panes | `gap: var(--cantos-space-sm)` | copied `gap: 10px` | review | local geometry has a named constant with units, scope and reason |
| Semantic class names; readable `view!` | utility walls hide structure and state | `class="dialogue-row"` | `class="flex gap-2 px-3 py-1 rounded-lg bg-..."` | review; gate counts classes per element (proposed) | small shared utilities such as `visually-hidden` |
| Style state from semantic attributes | visual state and accessible state diverge | `.dialogue-row[aria-invalid="true"]`, `[aria-current="page"]`, `[data-draft-status="conflict"]` | `class:red-border=...` with no `aria-invalid` | DOM test asserts the attribute; capture shows the style | — |
| Themes switch at the root through tokens | per-component `.dark` overrides drift | generated `:root` / `[data-theme="dark"]` blocks | `.dark .mini-player { background: ... }` | cross-theme inspection | — |
| Every transition names its properties and has a reduced-motion path | unintended layout animation; vestibular harm; delayed action | motion tokens collapse under `prefers-reduced-motion`; feedback remains immediate | `transition: all`; a slide-in with no reduced variant | review, runtime reversal/reduced-motion observations | — |
| Visible focus is never removed | keyboard users lose their place | `:focus-visible` outline using the focus role | `outline: none` with no replacement | gate (proposed); focus-tour capture | — |
| Panes adapt to their container, not the viewport | the Studio inspector overflows on a wide screen with both panes open | `@container` on the pane | `@media (max-width: …)` for a pane that is narrow on desktop | capture with both side panes open | page-level layout may use media queries |

## Themes

Generated token CSS, not component CSS, carries light/dark. A pattern that honors the system
preference until the listener chooses, without a flash before WASM loads:

```css
/* Illustrative shape of generated output. */
:root { --cantos-color-surface: /* light value */; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --cantos-color-surface: /* dark value */; }
}
:root[data-theme="dark"] { --cantos-color-surface: /* dark value */; }
```

The shell sets `data-theme` only after an explicit choice, and reads a stored choice through the
fallible storage helper. Light and dark are checked separately for contrast; one passing theme
says nothing about the other.

That snippet is illustrative: the current `UiRoot` uses a scoped `.cantos-ui` boundary with
reactive `Theme::{Light, Dark, System}` and density values. Keep the mounted tree and controlled
values stable while changing those attributes, including nested roots; verify entered text,
selection and focus survive. Locale, font readiness/scale, motion/gesture policy and immutable
projections/intents come from the host context or value injection appropriate to their owner.
A reusable component must not read OS preferences or construct services, ports, mock owners,
network/audio/auth effects. Scripted motion consumes the injected policy; CSS resolves media
preferences through the styling boundary. No universal new DI API is implied.

## Component stylesheets

```scss
// Illustrative, proposed. dialogue_row.scss beside dialogue_row.rs.
.dialogue-row {
  display: grid;
  grid-template-columns: minmax(8rem, 14rem) 1fr auto;
  gap: var(--cantos-space-sm);
  padding-block: var(--cantos-space-xs);
  background: var(--cantos-color-surface);
  color: var(--cantos-color-on-surface);

  &__speaker { font: var(--cantos-type-label); }
  &__text { font: var(--cantos-type-body); field-sizing: content; }   // candidate; verify support
  &__findings { color: var(--cantos-color-on-surface-variant); }

  &[aria-invalid="true"] &__findings { color: var(--cantos-color-error); }
}

.script-pane { container: script-pane / inline-size; }

@container script-pane (inline-size < 36rem) {
  .dialogue-row { grid-template-columns: 1fr; }
}
```

- Use `@use`/`@forward`, never the deprecated Sass `@import`; keep nesting to two or three levels.
- SCSS variables are compile-time helpers only; anything that changes with theme stays a custom property.
- Mixins only for meaningful repetition (focus ring, a container breakpoint), never `@mixin flex`.
- Follow the stylesheet pipeline the Web package actually adopts; do not invent a second one.

## Material 3 Expressive, expressed in CSS

- **Containment by tone, not lines.** Use surface-container roles and spacing for grouping;
  outlines only for input boundaries and functional separation; no card around every utterance
  ([UI system § Studio](../../../../docs/design/ui-system.md#studio-text-first-production)).
- **Shape by role.** The script reading area stays a stable rectangle; pills belong to filters,
  speed and compact actions; containers and overlays use their own shape roles.
- **Intensity by surface.** The editor is quiet chrome around text; the Theatre player and
  discovery may carry more expressive shape, color and motion, at the level `material3-expressive.md`
  assigns. Do not import Theatre's expressiveness into the editor.
- **Elevation only for overlap** (mini-player over content, dialogs), via elevation roles.
- **Status never by color alone.** QC, production and download states pair color with an icon
  and text.

## Focus, forced colors and the mini-player

```scss
:where(a, button, input, select, textarea, [tabindex]):focus-visible {
  outline: var(--cantos-focus-width) solid var(--cantos-color-focus);
  outline-offset: var(--cantos-focus-offset);
}

html { scroll-padding-block-end: var(--cantos-mini-player-block-size); }
```

Use `outline` for focus because forced-colors modes drop `box-shadow`; check
`@media (forced-colors: active)` for state indicators that rely on background color. The
`scroll-padding` keeps a focused control from scrolling under the fixed mini-player.

## Vietnamese text in layout

- No fixed heights on text containers and no `overflow: hidden` on tight line boxes: stacked marks
  in "Ỗ", "Ẫ", "ặ" clip first. Body line height comes from the type roles; verify with uppercase and
  mixed-diacritic strings, not only with "Minh".
- Long names and titles wrap: "Người dẫn chuyện", "Ánh đèn cuối sân khấu — Tập 12: Một lời hẹn
  chưa bao giờ nói ra". Truncation needs the full text available to assistive technology.
- Comparable figures (time, cost, duration) use `font-variant-numeric: tabular-nums`.
- Hit areas follow touch guidance through padding or `min-block-size` even when visuals are compact.
- Fonts must cover Vietnamese; the choice and subsetting belong to `typography.md`.

## Tailwind

Not by default. Consider it only for an isolated prototype or if the Web package standardizes on
it through an ADR; its configuration must consume the generated token variables, never define a
parallel palette.

## The static style gate (proposed)

No style gate exists. A proposed check over changed `.css`/`.scss` and `view!` sources would flag
candidates for human judgement, not verdicts:

| Candidate finding | Why |
|---|---|
| hex, `rgb()`, `hsl()`, `oklch()` literals outside the generated token file | token bypass |
| `border-radius` literals other than `0` | shape bypass |
| `outline: none` or `outline: 0` without a `:focus-visible` replacement in the same rule set | removed focus |
| `transition` or `animation` in a file with no `prefers-reduced-motion` handling and no motion tokens | missing reduced path |
| Sass `@import` | deprecated module system |
| `!important` outside the utility layer | cascade fights |
| fixed `height` on text-bearing selectors | diacritic and zoom clipping |
| more than a small number of classes on one element in `view!` | utility wall |
| `set_inner_html` or `inner_html` in Rust UI code | injection surface |

Until it is implemented, run these searches by hand and report them as a manual review with the
exact patterns and paths searched, never as a passing gate. Once implemented, a gate pass is
`statically-checked` for those patterns only; it says nothing about rendered contrast or layout.

## Completion checklist

- [ ] Every value that varies with theme or brand is a token variable; no hex or radius literals.
- [ ] Stylesheets are component-owned, `@use`-based and shallow; `view!` uses semantic classes.
- [ ] State styles hang off semantic attributes that tests also assert.
- [ ] Light and dark, reduced motion, forced colors and focus visibility were checked where touched.
- [ ] Panes use container queries; Vietnamese strings wrap without clipping at 200% zoom.
