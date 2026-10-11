# Semantic tokens: one source, two renderers

> **Scope.** Create, rename, change or consume a Cantos design token: the versioned source, its
> tiers and names, generated CSS custom properties for Leptos and the generated Kotlin theme for
> CMP, platform units, versioning, contrast records, the rule of three and the proposed tests.
> Use before adding a color, type, space, shape, elevation or motion value anywhere.

Token families, roles and their rules are owned by
[UI system § Semantic tokens](../../../../docs/design/ui-system.md#semantic-tokens). This
reference owns the pipeline that keeps both renderers faithful to them. Implemented so far: the
Studio token source in `packages/design-tokens/`, `scripts/studio_tokens.py` and the committed CSS
it generates, with a drift check. There is no Theatre token set and no Kotlin output; every other
path, format and command below is **proposed** until a UI slice implements it.

## 1. The pipeline

```text
reviewed design decision (design issue or ADR, with contrast record)
        ↓
one versioned semantic token source          packages/design-tokens/ (outside apps/)
        ↓ deterministic generator             pinned, tested, run by one documented command
   ┌────┴─────────────────────────┐
CSS custom properties          Kotlin theme mapping
(Leptos Studio + Theatre)      (CMP: ColorScheme, extended roles, type, shapes, spacing, motion)
        ↓                                ↓
component styles               CantosTheme + component wrappers
```

| Decision for the first slice | Constraint | Candidates |
|---|---|---|
| source location | owned by neither app; reviewed by both renderer owners; versioned with the code | **decided and implemented:** `packages/design-tokens/` (Studio set `studio-tokens-0.1.0.json`); interchange contracts stay in `packages/contracts/` |
| source format | names, types, theme variants and references expressible without code | Design Tokens Community Group JSON (check its current status), or a small project schema |
| generator | deterministic, dependency-light, runnable on Linux and macOS, covered by tests | a repository script, a Rust `xtask`, or a Node tool such as Style Dictionary |
| generated outputs | committed with a drift test, or built on demand by both toolchains | choose one; committing keeps Trunk and Gradle builds free of the generator |

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

| Semantic role | CSS (proposed) | CMP (proposed) |
|---|---|---|
| `color.onPrimaryContainer` | `--cantos-color-on-primary-container` | generated `ColorScheme.onPrimaryContainer` |
| `color.surfaceContainer` | `--cantos-color-surface-container` | generated `ColorScheme.surfaceContainer` |
| `color.success` / `color.onSuccess` | `--cantos-color-success` / `--cantos-color-on-success` | `CantosTheme.extendedColors.success` / `onSuccess` |
| `color.focus` | `--cantos-color-focus` | `CantosTheme.extendedColors.focus` |
| `type.metadata` | `--cantos-type-metadata-size`, `-line-height`, `-weight`, `-tracking` | `CantosTheme.typography.metadata` |
| `shape.control` | `--cantos-shape-control` | `CantosTheme.shapes.control` |
| `elevation.overlay` | `--cantos-elevation-overlay` | `CantosTheme.elevation.overlay` |
| `motion.navigation` | `--cantos-motion-navigation-duration`, `-easing` | `CantosTheme.motion.navigation` (spring spec) |

Where a Cantos role means the same thing as a Material 3 `ColorScheme` or `Typography` role, map
to it so stock Material components pick it up. Roles Material lacks — success, warning, focus,
metadata type — go into an extension object; never smuggle them into an unrelated Material role
(`tertiary` as "warning" breaks the day a component uses `tertiary` for its own purpose).

Known gap: the UI system lists `space.xs` through `space.xl` with six rhythm values. Five names
cannot hold six values; the first slice must name the sixth step before generating anything.

## 4. Units per platform

Share semantics and relationships, never raw pixels ([UI system](../../../../docs/design/ui-system.md#semantic-tokens)).

| Family | Web | CMP | Note |
|---|---|---|---|
| space, shape | `rem` (or `px` for hairlines) | `dp` | one logical unit in the source; the generator converts |
| type size, line height | `rem`, unitless line-height ratio | `sp`, line height in `sp` or `em` | must scale with browser zoom and OS font scale |
| elevation | tonal step plus optional shadow | tonal elevation plus optional shadow | prefer tone ([UI system](../../../../docs/design/ui-system.md#semantic-tokens)) |
| motion | duration + easing, or spring parameters for a JS animator | spring spec, or duration + easing | CSS `linear()` can approximate a spring curve but not preserve velocity on interruption; verify browser support |

## 5. Source sketch

Illustrative only; no values are decided. Placeholders make missing decisions visible instead of
inventing them.

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
| Oracle | drift test: regenerate into a temporary directory and diff with the committed files (proposed) |
| Enforcement / exception | manual until the drift test exists · none |

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

## 8. Rule of three

| Field | Contract |
|---|---|
| Scope | spacing, radius, size, duration or color values written in component code |
| Why / failure | copied literals diverge under the next theme or brand change; premature tokens bloat the contract |
| Good | a third use of the same value **with the same meaning** becomes a semantic or component token; until then a named local constant in the component |
| Counterexample | a fourth copy of `12px` for "row gap"; or a global token for a one-off illustration offset |
| Oracle | literal scan of component styles and Kotlin outside generated files (proposed); review |
| Enforcement / exception | manual · renderer-local geometry for a platform convention, with the reason written beside it |

## 9. Contrast records

The floors are the UI system's
([§ Accessibility and motion acceptance](../../../../docs/design/ui-system.md#accessibility-and-motion-acceptance));
encode them once in the checker and link the document. The source lists the pairs; a pure
function evaluates every pair in every theme:

```rust
// Illustrative, proposed — no token checker exists yet.
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

## 10. Proposed tests

| Test | Oracle | Level when it exists |
|---|---|---|
| generator drift | regenerate and diff against committed outputs | `statically-checked` |
| contrast pairs | every pair × theme meets its floor | `example-tested` |
| role completeness | every source role appears in both outputs; every `var(--cantos-*)` used in CSS is defined | `statically-checked` |
| no raw literals | no hex, `rgb()`, `hsl()` or repeated radius literals in component CSS; no `Color(0x…)` in Kotlin outside generated files | `statically-checked` |
| reference tier unused | components reference only semantic or component roles | `statically-checked` |
| placeholder refusal | a `<pending …>` value fails generation with an exact error | `example-tested` |

All are **proposed**. Until they exist, say so in the report instead of implying a gate.

## 11. Common mistakes

- Copying CSS pixel values into `dp`, or `rem` into `sp`, without the generator.
- Defining a "temporary" palette in Kotlin or a stylesheet.
- Assuming `brand.primary` equals `color.primary`.
- Consuming a reference tone (`lavender.tone40`) from a component.
- Separate Studio and Theatre palettes for what is a density difference.
- Changing a shared role and inspecting only the renderer you were working in.
