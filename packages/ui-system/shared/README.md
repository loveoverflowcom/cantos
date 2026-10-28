# Shared semantic contract v0.9.0

v0.9.0 adds the contextual guest-profile and account-tile roles of Account v4.3
([#526](https://github.com/loveoverflowcom/cantos/issues/526)): seven themed `color.theatre*` roles,
`ProfileAction` / `OnProfileAction` / `ProfileActionHover` / `ProfileActionPressed` and
`AccountControl` / `OnAccountControl` / `AccountControlHover`. Nothing existing changes value, and the
global `theatrePink`, `theatrePrimary` and every standard role stay as they were. Dark profile action is
raspberry `#a10f65` with cream `#fff8ef` (hover `#900854`, pressed `#790447`); Light reuses the pink pair
`#e8a2da` / `#291332` with hover `#df94d0` and pressed `#d17bbf`. The inverse secondary has no role of its
own: it reads the card's `onPrimaryContainer` as fill and `primaryContainer` as ink. The account control
is a lavender circle with plum ink in Dark (`#bfa0eb`, hover `#d5b7f2`) and the mirrored plum circle with
cream ink in Light (`#291332`, hover `#4a2d5e`); the Light values are a derived mirror, not a design table
row. Measured solid pairs (asserted by `scripts/test_ui_tokens.py`): Dark action text 7.19, Dark
action/card boundary 3.41 (hover 4.02, pressed 4.91), Light action text 8.60, account control/tile 7.55 Dark.
These are consumed only under `[data-expression="theatre"]`; the Web mapping and its scope are in the
[Leptos guest recipe](../leptos/README.md#guest-profile-card-recipe-v43). The CMP renderer has not adopted
the roles: its profile-card mapping is still the superseded v4.2b one, tracked separately.

v0.8.0 adds the docked-search geometry of Search v4.3 ([#452](https://github.com/loveoverflowcom/cantos/issues/452)):
`size.searchHeader` 72 (minimum header, grows with text), `shape.searchRest` 36 and `shape.searchOpen` 28
(resting and open dock radius). Nothing existing changes value. The 280/200 ms open/close motion is
deliberately **not** a canonical role: it is a Web design seed (`--cantos-web-search-*` in
`foundation.css.in`) and native renderers keep their own motion scheme.

v0.7.0 adds the **Theatre expression** ([#452](https://github.com/loveoverflowcom/cantos/issues/452),
owner direction of 2026-10-25): an opt-in role remap for one subtree, 17 themed `color.theatre*`
roles, `type.theatreHeading.weight` / `type.theatreControl.weight` (700), `size.avatarControl` 48,
`size.brandMark` 44 and `size.searchAction` 52. No existing role changes value; a consumer that names
no expression resolves exactly the v0.6.0 roles. See [Theatre expression](#theatre-expression-v070).

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
| `expressions` | Optional. Name → `{standard role: canonical target}`; same type and theme scope, color or font weight only, no chains; contrast pairs the remap touches are re-measured |
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

## Theatre expression (v0.7.0)

The owner chose a more distinctive Theatre treatment (lilac hero, deep-plum selection, cream reading
surface, peach and pink accents, bold editorial headings). It supersedes the quiet first draft for
Theatre only, and it is not a recolor: the 17 `color.theatre<Role>` palettes sit beside the existing
roles, and `expressions.theatre` maps eleven standard color roles
(`background`, `surface`, `surfaceContainer`, `surfaceContainerHigh`, `primary`, `onPrimary`,
`primaryContainer`, `onPrimaryContainer`, `onSurface`, `onSurfaceVariant`, `focus`) and five weights
(`type.{headline,title,authHeading,membershipTitle}.weight` → `type.theatreHeading.weight`,
`type.label.weight` → `type.theatreControl.weight`) onto them. Player, status, outline, accent and
geometry roles are untouched.

| Renderer | Opt in | Opt out (default) |
| --- | --- | --- |
| Web | `UiRoot expression=Expression::Theatre` → `data-expression="theatre"`; the generated rule is declared after every theme block and again for the system-dark selector, so it holds in light, dark and system themes | no attribute, no rule applies |
| CMP | `UiPreferences(expression = CantosExpression.Theatre)`; `tokenColor`, the Material scheme and `typeStyle` resolve through the generated `CantosTokens.role(name, expression)` | `CantosExpression.Standard` resolves every role to itself |

Theatre Web and, since [#559](https://github.com/loveoverflowcom/cantos/issues/559), every Studio Web
root select the expression (`STUDIO_EXPRESSION` in `apps/cantos-studio/web/src/ui.rs`): one palette and
weight remap, no second named profile and no forked hex. Studio keeps its own density, layout and reading
intensity. The expression's contextual selectors (docked search, filter chips, the guest profile card and
account tile) have no Studio consumer and match nothing there; the floating field, compact select and
tonal action rules apply to both. A consumer that names no expression (the component preview default, any
future app) still resolves exactly the standard roles. The superseded Studio Quiet Surfaces v1 choice of
Standard is replaced for the Web renderer only; CMP has no Studio consumer. The generator refuses
an expression that changes a role's type or theme scope, chains remaps, names a non-canonical role
or weakens any contrast pair its swap touches (every pair is re-measured in both themes). The
exact solid-pair measurements for the roles that matter, computed by `scripts/ui_tokens.py` and
asserted by `scripts/test_ui_tokens.py`:

| Pair | Light | Dark | Floor |
| --- | --- | --- | --- |
| `theatreOnHero` on `theatreHero` (`#1d0b28` on `#c6a0f5`) | 8.62 | 13.76 | 4.5 |
| `theatreHeroAccent` on `theatreHero` (`#5e3906` on `#c6a0f5` / `#d9b57d` on `#352142`) | 4.72 | 7.50 | 4.5 |
| `theatreOnPeach` on `theatrePeach` | 9.74 | 6.62 | 4.5 |
| `theatreOnPink` on `theatrePink` | 8.60 | 8.28 | 4.5 |
| `theatreOnPrimary` on `theatrePrimary` | 11.36 | 8.28 | 4.5 |
| `theatreOnPrimaryContainer` on `theatrePrimaryContainer` | 16.15 | 7.66 | 4.5 |
| `theatreOnSurfaceVariant` on `theatreSurfaceContainer` | 5.47 | 7.98 | 4.5 |
| `theatreFocus` on `theatreSurfaceContainer` | 6.55 | 7.89 | 3 |
| `color.error` on `theatreSurfaceContainer` | 5.22 | 7.79 | 3 |

These are solid-color arithmetic only: nothing here measures text over artwork, scrims,
transparency, disabled opacity or forced colors. Light `color.error` on `theatreSurfaceContainerHigh`
is 4.42, so error *text* is never placed on that fill; the field stroke is an indicator (floor 3).
The expressive proposal's hexes are Theatre-only provenance in
[#452](https://github.com/loveoverflowcom/cantos/issues/452); `tokens.json` remains the sole source.

Renderer-local geometry stays out of the canonical source (design v4 calls the numbers starting
points, not shared pixels): Web declares 68 minimum shell and 16 radius (the search header uses the canonical roles) in
`foundation.css.in` under `[data-expression="theatre"]`; CMP names `FloatingFieldMinHeight` 68,
`FloatingFieldRadius` 16 in `Fields.kt`; the search header minimum is the canonical `size.searchHeader`. The shared sizes are the
ones both renderers and every consumer must agree on: a 48-unit header control (`size.avatarControl`,
also the minimum hit area), a 44-unit official mark 12 units before the wordmark (`size.brandMark`,
`space.inset`) and a 52-unit docked search action (`size.searchAction`).

Noto Serif Bold 700 and Noto Sans Bold 700 are the committed static faces of the
[font bundle](assets/noto-fonts/README.md); no font asset is added. The two eye glyphs
(`visibility`, `visibility_off`) are the filled Material Icons originals the design selected, at the
same pinned revision and recorded in the [icon manifest](icons/material-outlined/manifest.json).

## Decisions from the reference

The authenticated design package reviewed on 2026-10-11 supplies identical Lavender Ink palettes
for both products, Noto Sans UI / Noto Serif editorial names, 14/1.55 Web body, 14/1.9 editor,
44 logical-unit Web targets, 48 for comfortable/touch intent, 140ms feedback and proposed 260ms
navigation. Paper, lavender and muted gold preserve the quiet historical/fantasy character.
These are the original reference values; current body/target values follow the v3 reconciliation above.
The Theatre CMP mock now supplies the exact Noto Sans/Serif resource families (SIL OFL 1.1)
and user-provided reference paintings; its [asset record](../../../apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/composeResources/files/provenance.json)
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
| `shape.pill` (Full) | proportional in CMP; 999 logical units in CSS | circular icon controls, genuine chips/indicators; scoped auth actions in v0.4.0 |

Textual CMP `Badge` uses the bounded `shape.reading` radius: a proportional capsule clips
first/last-line glyphs when Vietnamese labels wrap at large text. Chip/icon pill geometry is
unchanged; no token value changes. See the [qualification repair](../../../docs/evidence/ui-qualification-20261025.md#resumed-qualification--confirmed-cmp-paint-defect).

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
| `size.authIcon` | 28 | Retained historical role; the 23 October action anatomy uses `size.icon` without a circle |
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

Both package-local previews register concrete specimens and state controls. The original
[implementation-only checkpoint](../../../docs/evidence/ui-auth-account-v2.md) is historical;
current adoption is governed by fresh qualification records, not a permanent preview-only label.
The [current recheck](../../../docs/evidence/ui-qualification-20261025.md) distinguishes implemented
and adopted variants from those still outside admission. Current CMP also implements Theatre Floating/FloatingSearch with trailing actions (#457);
its native counterpart requires independent JVM runtime and visual evidence. Product/backend/auth/navigation and device acceptance remain separate.

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
