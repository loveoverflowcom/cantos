# Semantic tokens: one source, two renderers

> **Scope.** Create, rename, change or consume a Cantos design token: the versioned source, its
> tiers and names, generated CSS custom properties for Leptos and the generated Kotlin theme for
> CMP, platform units, versioning, contrast records, the rule of three and the proposed tests.
> Use before adding a color, type, space, shape, elevation or motion value anywhere.

Token families, roles and their rules are owned by
[UI system § Semantic tokens](../../../../docs/design/ui-system.md#semantic-tokens). This
reference owns the pipeline that keeps both renderers faithful to them. Implemented so far: the
canonical source `packages/ui-system/shared/tokens.json`, `scripts/ui_tokens.py`, generated scoped
CSS, typed Rust and Kotlin lookup metadata, plus a compatibility `scripts/studio_tokens.py` CLI.
Studio, Theatre Web and the CMP foundation consume these roles; `CantosTheme` maps the native
palette, typography, shapes and injected preferences. The
[shared schema](../../../../packages/ui-system/shared/README.md) and
[migration](../../../../docs/architecture/ui-system-migration.md) describe implemented ownership;
illustrations below remain proposed unless that contract implements them.

## 1. The pipeline

```text
reviewed design decision (design issue or ADR, with contrast record)
        ↓
one versioned semantic token source          packages/ui-system/shared/ (outside apps/)
        ↓ deterministic generator             pinned, tested, run by one documented command
   ┌────┴─────────────────────────┐
CSS custom properties          Kotlin theme mapping
(Leptos Studio + Theatre)      (CMP: ColorScheme, extended roles, type, shapes, spacing, motion)
        ↓                                ↓
component styles               CantosTheme + component wrappers
```

| Implemented decision | Constraint | Owner |
|---|---|---|
| source location | owned by neither app; reviewed by both renderer owners; versioned with the code | `packages/ui-system/shared/tokens.json`; the legacy snapshot is immutable migration evidence |
| source format | names, types, paired themes, aliases and contrast pairs validated before generation | [bounded shared schema](../../../../packages/ui-system/shared/README.md), not a claim of DTCG conformance |
| generator | deterministic, dependency-light, covered by tests; repository checks run through `cargo xtask` | `scripts/ui_tokens.py` is the implementation primitive |
| generated outputs | committed with drift checks so Trunk/Gradle builds need no generator | scoped CSS, Rust token API, Kotlin lookup and preview metadata |

Record the choices with [`templates/adr.md`](../../../../templates/adr.md). Mechanics for wiring the
outputs into each build belong to
[`cantos-leptos-web` styling](../../cantos-leptos-web/references/styling.md) and
[`cantos-cmp-mobile` design system](../../cantos-cmp-mobile/references/cmp-design-system.md).

## 2. Three tiers

| Tier | Holds | Consumed by | Rule |
|---|---|---|---|
| reference | raw palette tones, font files, base durations | the source only | never referenced by a component; values are open decisions |
| semantic (system) | the doc-owned roles: `brand.*`, `color.*`, `type.*`, `space.*`, `shape.*`, `elevation.*`, `motion.*` | every component on both renderers | the stable contract; names follow the UI system |
| component | a role bound to one component, e.g. `player.primaryControl.size` | that component on both renderers | only after the rule of three (§ 8) and only when both renderers need it |

Decoupling follows from the tiers: a palette change edits reference values; a renderer change
edits a generator template; neither touches component code.

## 3. Names and mapping

A semantic name says what the value **does**, never what it looks like: `color.primary`, not
`color.lavender`; `space.sm`, not `space.8`. Renderer names are derived mechanically:

| Semantic role | CSS | CMP mapping |
|---|---|---|
| `color.onPrimaryContainer` | `--cantos-color-on-primary-container` | `CantosTheme` → `ColorScheme.onPrimaryContainer` |
| `color.surfaceContainer` | `--cantos-color-surface-container` | `CantosTheme` → `ColorScheme.surfaceContainer` |
| `color.success` / `color.successContainer` | `--cantos-color-success` / `--cantos-color-success-container` | `tokenColor` lookup; paired foreground/container |
| `color.focus` | `--cantos-color-focus` | generated named lookup |
| `type.metadata.*` | `--cantos-type-metadata-size`, `-line-height`, `-weight` | `typeStyle("metadata")` → Material body-small mapping |
| `shape.control` | regular Web uses the separate Web shape role | `CantosShapes.control` → Material small mapping |
| `elevation.overlay` | `--cantos-elevation-overlay` | generated structured shadow lookup |
| `motion.navigation.*` | duration/easing custom properties | generated duration/easing lookup; no native spring claim |

Where a Cantos role means the same thing as a Material 3 `ColorScheme` or `Typography` role, map
to it so stock Material components pick it up. Roles Material lacks — success, warning, focus,
metadata type — use named lookup/extension mappings; never smuggle them into an unrelated Material role
(`tertiary` as "warning" breaks the day a component uses `tertiary` for its own purpose).

The shared contract owns the spacing rhythm and distinct Web/CMP shape scales. Inspect its
current mappings rather than copying a numeric snapshot into a component or skill. Semantic
roles come before platform mappings; CSS px must never be copied directly into dp/sp.

## 4. Units per platform

Share semantics and relationships, never raw pixels ([UI system](../../../../docs/design/ui-system.md#semantic-tokens)).

| Family | Web | CMP | Note |
|---|---|---|---|
| space, shape | `rem` (or `px` for hairlines) | `dp` | one logical unit in the source; the generator converts |
| type size, line height | `rem`, unitless line-height ratio | `sp`, line height in `sp` or `em` | must scale with browser zoom and OS font scale |
| elevation | tonal step plus optional shadow | tonal elevation plus optional shadow | prefer tone ([UI system](../../../../docs/design/ui-system.md#semantic-tokens)) |
| motion | duration + easing, or spring parameters for a JS animator | spring spec, or duration + easing | CSS `linear()` can approximate a spring curve but not preserve velocity on interruption; verify browser support |

## 5. Source sketch

Illustrative only, not the implemented schema or a second source. The active source and exact
version live in `shared/tokens.json`; proposed deltas must reconcile there. Placeholders in a
design proposal make missing decisions visible instead of inventing them.

```json
{
  "version": "0.1.0",
  "reference": {
    "lavender": { "tone40": "<pending palette decision>", "tone80": "<pending palette decision>" },
    "neutral": { "tone98": "<pending palette decision>", "tone10": "<pending palette decision>" }
  },
  "themes": {
    "light": {
      "color.primary": "{reference.lavender.tone40}",
      "color.surface": "{reference.neutral.tone98}"
    },
    "dark": {
      "color.primary": "{reference.lavender.tone80}",
      "color.surface": "{reference.neutral.tone10}"
    }
  },
  "contrast_pairs": [
    { "foreground": "color.onPrimary", "background": "color.primary", "floor": "text" },
    { "foreground": "color.onSurfaceVariant", "background": "color.surfaceContainer", "floor": "text" },
    { "foreground": "color.focus", "background": "color.surface", "floor": "non_text" }
  ]
}
```

A generator must refuse a placeholder: an unresolved value is a build error, not a silent default.

## 6. Generated outputs are immutable artifacts

Every generated file starts with a header naming its source version, source digest and the command
that produced it, and is never edited by hand:

```text
/* Generated from cantos tokens v0.1.0 (sha256:<digest>) by <documented command>. Do not edit. */
```

| Field | Contract |
|---|---|
| Scope | every generated CSS or Kotlin token file |
| Why / failure | a hand patch makes one renderer drift silently and is overwritten on the next run |
| Good | edit the source or the generator template, regenerate, commit source and outputs together |
| Counterexample | changing a value inside the generated Kotlin "just for iOS" |
| Oracle | implemented generator `--check` and drift tests via `cargo xtask verify repository` |
| Enforcement / exception | source/output drift is statically checked · none |

Released token versions are immutable values: a change produces a new version, never a rewrite of
an old one. In code the generated Kotlin is `val`s and immutable data classes; CSS roles are
defined at the root and theme scopes and never reassigned by component selectors.

## 7. Change protocol

1. Find the role and **every consumer** on both renderers (custom property uses, Kotlin property
   references, component tokens built on it).
2. Edit the source; bump its version — removing or renaming a role is breaking and migrates both
   consumers in the same change, adding a role is additive, changing a value still needs evidence.
3. Regenerate both outputs with the documented command; never one renderer alone.
4. Recompute the contrast record for every pair the change touches, per theme.
5. Render and open the affected states on **both** renderers in light and dark
   ([`visual-review.md`](visual-review.md)). CSS evidence proves nothing about Kotlin.
6. Report the version, the roles and both consumers in the
   [completion report](../SKILL.md#completion-report).

Theme switching changes role values per theme scope; it never adds `.dark .button` overrides.
Studio and Theatre share one brand and palette; they differ in density and layout, not in roles.
Resolve Light/Dark/System at the host's appropriate boundary. Inject density, locale, licensed
font families/font scale, motion/gesture preferences and immutable projections/intents through
context or values; a reusable component never reads the OS or creates a service/port/mock owner.
Current Leptos `UiRoot` scopes theme/density; native `CantosTheme` receives concrete preferences
and optional font families. Do not claim a universal preference API exists where the renderer
only has a narrower contract. Observe nested themes without resetting entered values or focus.

## 8. Rule of three

| Field | Contract |
|---|---|
| Scope | spacing, radius, size, duration or color values written in component code |
| Why / failure | copied literals diverge under the next theme or brand change; premature tokens bloat the contract |
| Good | reuse an existing semantic role; a repeated new meaning earns a reviewed role; local geometry has a named constant with units, scope and reason |
| Counterexample | a fourth copy of `12px` for "row gap"; or a global token for a one-off illustration offset |
| Oracle | literal scan of component styles and Kotlin outside generated files (proposed); review |
| Enforcement / exception | manual · renderer-local geometry for a platform convention, with the reason written beside it |

## 9. Contrast records

The floors are the UI system's
([§ Accessibility and motion acceptance](../../../../docs/design/ui-system.md#accessibility-and-motion-acceptance));
encode them once in the checker and link the document. The source lists the pairs; a pure
function evaluates every pair in every theme:

```rust
// Illustrative Rust shape; the implemented checker and known-value tests are Python.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContrastFloor {
    Text,
    LargeText,
    NonText,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct ContrastRatio(f64);

impl ContrastFloor {
    /// Encoded once from docs/design/ui-system.md § Accessibility and motion acceptance.
    pub const fn minimum(self) -> ContrastRatio {
        match self {
            Self::Text => ContrastRatio(4.5),
            Self::LargeText | Self::NonText => ContrastRatio(3.0),
        }
    }
}

pub fn contrast_ratio(a: Srgb, b: Srgb) -> ContrastRatio {
    let (lighter, darker) = ordered(relative_luminance(a), relative_luminance(b));
    ContrastRatio((lighter + 0.05) / (darker + 0.05))
}

pub fn pair_failures(theme: &ResolvedTheme, pairs: &[ContrastPair]) -> Vec<PairFailure> {
    pairs
        .iter()
        .filter_map(|pair| pair.evaluate(theme).err())
        .collect()
}
```

Test it like a theorem: `every_contrast_pair_meets_its_floor_in_light_and_dark`. Anchor the
formula with independent known values (black on white is 21:1; a color on itself is 1:1) so the
oracle is not the implementation. Record the resulting table in the design issue or ADR. A
passing token check does not prove rendered contrast over cover art, scrims, gradients or
disabled text; those need inspected captures.

## 10. Test evidence

| Test | Oracle | Level when it exists |
|---|---|---|
| generator drift | regenerate and diff against committed outputs | `statically-checked` |
| contrast pairs | every pair × theme meets its floor | `example-tested` |
| role completeness | every source role appears in both outputs; every `var(--cantos-*)` used in CSS is defined | `statically-checked` |
| no raw literals | no hex, `rgb()`, `hsl()` or repeated radius literals in component CSS; no `Color(0x…)` in Kotlin outside generated files | `statically-checked` |
| reference tier unused | components reference only semantic or component roles | `statically-checked` |
| placeholder refusal | a `<pending …>` value fails generation with an exact error | `example-tested` |

Generator drift, both-theme contrast, CSS-role completeness and placeholder/type refusal now
have implemented checks in `scripts/test_ui_tokens.py`; typed Rust mapping has native smoke.
No-raw-literal/reference-tier gates remain **proposed**; native theme/shape tests independently
check their mappings, and generator drift includes Kotlin output. Runtime
contrast, interaction and native evidence remain separate; cite the exact executed result.

## 11. Common mistakes

- Copying CSS pixel values into `dp`, or `rem` into `sp`, without the generator.
- Defining a "temporary" palette in Kotlin or a stylesheet.
- Assuming `brand.primary` equals `color.primary`.
- Consuming a reference tone (`lavender.tone40`) from a component.
- Separate Studio and Theatre palettes for what is a density difference.
- Changing a shared role and inspecting only the renderer you were working in.
