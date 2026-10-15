# Shared semantic contract v0.6.0

v0.6.0 reconciles [design v3](../../../docs/design/ui-token-v3-reconciliation.md): UI body is
16 logical units and the regular minimum control is 48 at both densities; compact/comfortable
inline padding remains 12/16. Existing roles, editor typography, palettes, motion and the
independent Web/CMP shape scales are preserved. The proposal's duplicate and future geometry
roles are deferred with reasons; its older baseline never removes v0.5.0 additions. Generated
CSS/Rust/Kotlin and both renderer consumers use this single source.

v0.5.0 adds scoped auth/Membership type and symbol roles from the Mobile v2 reference:
`type.authHeading` (32 / 1.22 / 400), `type.membershipTitle` (28 / 1.4 / 400),
`size.authLogo` (128), `size.statusSymbol` (72) and `shape.statusSymbol` (28).
Each type role declares size, line height and weight. `CantosTypography` preserves the injected
editorial font and platform scaling; the Leptos preview maps the same roles. Existing palette,
regular typography and shape mappings are unchanged. Both generated token inventories expose
all nine additions.

[PR137's bounded admission](../../../docs/evidence/pr137-cmp-design-v2.md) records fresh evidence
for the six existing Button/TextField/Surface/Badge/BackButton/Logo APIs consumed by the CMP
Membership mock. This is shared-preview admission, not Android/iOS, screen-reader, live
Membership or release acceptance. The compatibility import baseline is unchanged.

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
| `reduced_motion` | Every shared duration has an explicit zero mapping under reduced motion |

Supported types: `color` (sRGB six-digit hex), `dimension` (non-negative logical units),
`number` (line-height or opacity), `fontFamily`, `fontWeight`, `duration` (milliseconds),
`cubicBezier` (bounded effects curve), and structured `shadow` (offsets/blur/color/opacity).
Web maps dimension units to rem at 16 logical units per rem. Typography line height is unitless;
weights are integer values; durations map to ms. The generated CMP lookup retains logical
numbers, maps geometry to dp and type sizes to sp through its theme adapter, and never imports CSS units.

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
These are the original reference values; current body/target values follow the v3 reconciliation above.
The Theatre CMP mock now supplies the exact Noto Sans/Serif resource families (SIL OFL 1.1)
and user-provided reference paintings; its [asset record](../../../apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/provenance.json)
keeps hashes and the outstanding production artwork-rights boundary. The Leptos component preview
also loads the [same local licensed font bytes](assets/noto-fonts/README.md); product Web hosts
retain their existing local/system fallbacks. Shared theme callers may inject licensed font families
without changing platform font scale. The shared package contains that attributed font bundle and
no paintings; official logo vectors are local path-outlined assets with their own [provenance](assets/cantos-logo-kit-v2/README.md).

v0.2.1 adds `color.playerSurface`, `color.onPlayerSurface` and `color.playerAccent` from the
Mobile v2 handoff. Both themes deliberately retain the separate dark player surface and pale
control accent. Existing roles are unchanged; CSS/Rust/Kotlin and the preview token inventory
are generated together. All three player contrast pairs are part of the canonical checks.

The generator keeps the 8-step rhythm without renaming established Studio spacing roles:
`xs=4`, `sm=8`, `inset=12`, `md=16`, `section=20`, `lg=24`, `xl=32`, `xxl=36`.
v0.2.2 standardizes the existing shape roles instead of adding a mobile-only token set:

| Role | Logical radius | CMP Material slot / use |
| --- | --- | --- |
| `shape.detail` | 4 | extraSmall; small details |
| `shape.control` | 8 | small; compact/text controls |
| `shape.reading` / alias `shape.field` | 12 | medium; fields, regular actions, mini-player |
| `shape.container` | 16 | large; story/continue cards and large covers |
| `shape.overlay` | 20 | extraLarge; dialogs |
| `shape.thumbnail` | 10 | compact artwork, including mini-player; explicit requested exception |
| `shape.pill` (Full) | proportional in CMP; 999 logical units in CSS | circular icon controls, genuine chips/badges/indicators; scoped auth actions in v0.4.0 |

`CantosShapes` is the typed CMP adapter over these roles; `CantosTheme` maps the five Material
slots from it. Values live only in `tokens.json`. Ordinary containers never use Full. Stock
Material buttons opt into small/medium explicitly because their default Full shape bypasses
Material theme slots. The shared scale is CMP's: Web does **not** consume it (see below), so it
changes no Web corner; spacing, fonts, colors and motion values remain unchanged. Elevation is reserved for overlap; zero base shadow keeps reading quiet.
### Web shape mapping

Leptos keeps the soft corners it shipped with in v0.2.0 through five renderer-local roles
declared once, in the [stylesheet template](../leptos/foundation.css.in), not in `tokens.json`:

| Web role | Value | Historical shared role | Use |
| --- | --- | --- | --- |
| `--cantos-web-shape-control` | 24 (1.5rem) | `shape.control` | buttons, navigation selection, chips as actions |
| `--cantos-web-shape-reading` | 12 (0.75rem) | `shape.reading` / `shape.field` | text fields, notices, badges, reading surfaces |
| `--cantos-web-shape-container` | 18 (1.125rem) | `shape.container` | story/continue cards, covers, panels |
| `--cantos-web-shape-overlay` | 24 (1.5rem) | `shape.overlay` | featured container, mini-player, dialogs |
| `--cantos-web-shape-panel` | 24 (1.5rem) | none (Web v2 base panel) | primary page panels that hold nested cards, covers or rows (Theatre continue-listening, account identity); nested items keep `container` |

Regular Web CSS reads only these roles plus the shared `shape.pill`; it never reads the shared
`shape.{control,reading,field,container,overlay}` custom properties, so the CMP scale can change
without moving a Web corner. The generator refuses a template custom property outside
`--cantos-web-*` or one that shadows a canonical role, and `scripts/test_ui_tokens.py`
(`WebShapeIndependence`) pins the values and the no-direct-use rule. CMP outputs never mention
these roles. Values are logical-unit equivalents of the historical v0.2.0 numbers, not new design.

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

v0.3.0 adds avatar identity roles: six `color.avatar<Tone>` / `color.onAvatar<Tone>` pairs
(Lavender, Gold, Sage, Rose, Sky, Clay) and `size.avatarSmall|Medium|Large` (32/40/64). The
hues are muted container tones beside Lavender Ink, not status colors; every pair is a 4.5
text contrast check in both themes (measured 7.7–9.4). Existing roles are unchanged.

## Scoped auth/account mapping (v0.4.0)

[Issue 127](https://github.com/loveoverflowcom/cantos/issues/127)'s supplied
[design v2](https://drive.google.com/drive/folders/1oJ8Y8NzmjfFh_qSAx_qV6tNH9JIKlERx)
introduces additive roles for concrete auth/account consumers; it does not change the regular
CMP scale, Web mapping, player, mini-player, editor, colors, fonts or density roles.

| Role | Logical units | Scope |
| --- | --- | --- |
| `size.authPrimary`, `size.authSecondary` | 60, 56 | Minimum action height; intrinsic text height may grow |
| `size.authIcon` | 28 | Decorative circle in the filled auth action; no independent name |
| `size.authField`, `shape.authField` | 56, 20 | Tonal text entry with external label/help/error |
| `size.backControl` | 48 | Contextual Back hit area; host owns destination/cancellation |
| `size.authOutline` | 1.5 | Functional outlined auth action; regular `size.border=1` unchanged |
| `shape.authPressed` | 16 | Auth press target radius; feedback never delays dispatch |
| `shape.authContainer`, `shape.profileContainer` | 36, 36 | Quiet tone-grouped auth/profile surfaces |
| `shape.membershipContainer`, `shape.accountGroup` | 32, 28 | Membership and account reading groups |
| `motion.shape.duration` | 220 ms | Interruptible shape feedback; reduced motion 0 |

Web maps dimensions exactly to rem (`1.5` becomes `0.09375rem`, 1.5 CSS px at a 16 px root),
including text/browser zoom. Native mappings retain `1.5` as Double before dp conversion, never
round it to an integer. Android Back geometry meets the 48 dp target; the same logical geometry
exceeds iOS's 44 pt project target. Native platform behavior still needs device acceptance.

Capsule is an explicit variant, not a change to every Button or Surface. Profile/member text
uses the surface's paired foreground (`onPrimaryContainer` on `primaryContainer`), never a
muted foreground on that background. All existing contrast pairs remain active. Functional
focus/error/selected boundaries stay intact; only these surfaces omit decorative borders.

Both package-local previews register concrete specimens and state controls. New `BackButton`
and `Logo` remain **implemented-unverified** and unqualified until fresh two-renderer runtime,
accessibility and inspected-reference evidence exists. Variants likewise remain preview-only:
no real app adopts them in this issue. [Evidence and pending admission](../../../docs/evidence/ui-auth-account-v2.md)
separate implemented source from executed gates. Future #128–#131 consumers retain their own
product/backend/auth/navigation acceptance boundaries.

## Avatar

A text avatar shows a person or creator when no picture is available. The rules follow the
Google account / YouTube channel fallback and correct the VOTHub divergence, where CMP showed
one letter and Leptos two. One contract serves both renderers:

| Rule | Contract |
| --- | --- |
| Initial of a word | first BMP letter (Unicode `L*`) or ASCII digit, uppercased, keeping following combining marks (U+0300–036F, 1AB0–1AFF, 1DC0–1DFF, 20D0–20FF, FE20–FE2F); leading punctuation is skipped |
| Single (default) | initial of the first word: `Nguyễn Thị Ánh` → `N`, `@cantos.studio` → `C` |
| Pair | first and last words, keeping the Vietnamese given name: `Nguyễn Thị Ánh` → `NÁ` |
| No initial | empty text and a plain tone circle, never `?` |
| Tone seed | trimmed caller stable ID (account/creator ID) so a rename keeps the color; else the display name with whitespace runs collapsed |
| Tone | 32-bit FNV-1a over UTF-8 of the seed, mod 6, in the order Lavender, Gold, Sage, Rose, Sky, Clay |
| Glyph size | proportional to the diameter (0.44 single, 0.36 pair), not to text scale, so it never escapes the circle |
| Picture | caller-owned and drawn over the tone; while loading or after failure the initials show. CMP takes a host-decoded `ImageBitmap`; Web accepts only HTTPS, same-origin, `blob:` or `data:image/` URLs |
| Semantics | decorative by default; an optional localized accessible name makes it a standalone image |

Golden vectors (both renderer test suites assert these):

| Input | Single | Pair |
| --- | --- | --- |
| `  Nguyễn Thị Ánh  ` | `N` | `NÁ` |
| `  __Đỗ Văn An` | `Đ` | `ĐA` |
| `岑 昕` | `岑` | `岑昕` |
| `a◌́nh Dương` (decomposed) | `A◌́` | `A◌́D` |
| `ß` | `S` | `S` |
| `  😀 ` / empty | empty | empty |

| Seed | Tone |
| --- | --- |
| `user-42` | Rose |
| `user-1` | Sky |
| `alice@example.com` | Sage |
| empty, `creator-7` | Gold |
| `Nguyễn Thị Ánh`, `Người dẫn chuyện` | Lavender |

Supplementary-plane characters are skipped on both renderers because Kotlin common has no
code-point classification. Unicode category tables of the two standard libraries can still
differ for rare letters; the vectors cover realistic Vietnamese, Latin and CJK names. Fetching,
caching, size limits and decoding of pictures stay in the host; the primitive calls no network.

## Preview registration and adoption

[`preview-catalog.json`](preview-catalog.json) is the one component declaration source. Each
component has a stable ID/API name, a family, concrete host-driven sample states and status/reason
for both Leptos and CMP. `registered` and `implemented-unverified` describe code registration;
neither means design qualification. `unsupported` records an honest target exception and cannot
qualify for shared adoption. Both preview apps display these records and named canonical tokens.

`scripts/ui_preview_catalog.py` deterministically generates Rust/Kotlin metadata. The existing
token/icon generators produce both native and Web mappings. The repository gate checks drift,
public component inventories, actual sample/dispatch presence and new explicit app imports.
Current source checks are conservative; they are not compiler, semantics or visual-diff evidence.
Compiler/renderer tests independently check controlled values, callbacks and anatomy.

[`design-inventory-v3.json`](design-inventory-v3.json) maps the 102 specimens of the offline v3
design export onto that registry (32 existing APIs with their baseline states, 70 proposals). The
same generator turns it into display-only discovery data for each renderer: Rust `DESIGN_SPECIMENS`
for the Leptos workbench and Kotlin `DesignInventory` for the CMP gallery's Design v3 tab.
`DesignSpecimen` carries design status and ownership scope, never registration or qualification;
`PreviewCatalog` and `qualifications` above stay the only runtime sources. The Leptos digest hashes
the inventory file directly; the CMP digest covers the generated Kotlin like any other CMP source.

The exact current Leptos import sites at base `209e9e905c680e7cecab831f2d8f7894656e14fe` are a
bounded compatibility baseline, not newly approved components. Do not widen this exemption for
new adoption. A new component/import site requires `qualifications` with `component_id` and
`evidence` paths for **both** renderers. Each JSON evidence record has:

- `component_id`, `renderer`, `status: "PASS"`, named `reviewer` and inspected `reference`
- `tokens_sha256` and `source_sha256`, matching `python3 scripts/ui_preview_catalog.py --digests`
- `checks` entries for `structural`, `runtime`, `accessibility` and `visual`; each with actual
  `status: "PASS"`, reproducible `command` and existing repository-local `artifact`

Use [`templates/ui-component-qualification.json`](../../../templates/ui-component-qualification.json)
as an unqualified record skeleton. `NOT_RUN`, missing/stale artifacts, source/token edits or an
unsupported target refuse qualification. Approval invalidation is conservative across each
renderer package and its build manifests, lock/toolchain inputs and shared component/state
declarations. Evidence/baseline fields are excluded from that digest to avoid recursive hashes.
The gate verifies records/freshness; reviewers still own honest assertions,
pixel inspection, design comparison and appropriate device scope. A desktop/JVM test does not
verify Android/iOS or a screen reader.

Run the canonical `cargo xtask ui-preview leptos build|verify|run` and `cargo xtask ui-preview
cmp build|verify|run`, then `cargo xtask preflight --changed-only`. The
[skill admission sequence](../../../.agents/skills/cantos-ui-design/references/preview-catalog.md)
owns the before-production requirement. No current CMP/new-catalog qualification is granted by
the generated records alone.
