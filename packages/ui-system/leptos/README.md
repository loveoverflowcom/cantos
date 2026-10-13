# Leptos UI library

`cantos-ui` pins Leptos **0.7.8**, matching the apps. Enable `csr` in browser consumers.
The unpublished v0.2.0 foundation API is preserved; the component slice adds variants and exports.
No HTTP, backend, provider, domain rule, browser storage or playback dependency enters this crate.

## Components and ownership

| API | Contract |
| --- | --- |
| `UiRoot` | Reactive `Theme` and `Density`; one scoped boundary, optional class and children |
| `Button` | Native `type="button"`; primary/tonal/secondary/text/danger; reactive `ButtonState`, `on_press`, label children and optional decorative `icon` |
| `IconButton` | Same action state with an official `IconName` and nonempty `AccessibleName`; label remains in accessible text |
| `Link`, `NavItem`, `NavTabs` | Native links and labelled navigation; reactive safe `LinkHref`, `aria-current="page"`, optional icon; NavTabs is ordinary link navigation, not an ARIA tab widget |
| `FilterChip` | Controlled `selected`, native `aria-pressed`, `on_change(bool)`; blocked/loading reason |
| `TextField`, `TextArea`, `NumberField` | Required stable ID, visible label, controlled raw string and `on_input(String)`; editable/readonly/disabled/loading |
| `Select` | Native option children, controlled string, `on_input(String)` dispatched on native change; enabled/disabled/loading |
| `Checkbox`, `Radio`, `Switch` | Native checked inputs, controlled bool, `on_change(bool)`; Radio requires group name/value; Switch exposes switch semantics |
| `RangeField` | Native range and keyboard steps; raw string callback, native min/max/step; optional localized `value_text` |
| `FieldLabel`, `FieldHelp`, `FieldError` | Composable label/description primitives for caller-owned native controls |
| `Badge`, `Status` | Visible children + decorative icon + semantic tone; Status announcement is opt-in |
| `LoadingState`, `EmptyState`, `ErrorState` | Localized visible text/children; empty/error recovery actions are caller-composed children |
| `Surface`, `Stack`, `Grid`, `Container` | Quiet tonal containment, wrapping stacks, maximum 1/2/3 columns by container width, reading/wide/fluid measure; typed spacing and children |
| `Dialog` | Controlled native modal, localized title/close label and children; close requests/errors dispatched to caller; safe initial focus |
| `Icon`, `tokens::Token` | Offline decorative official outlined subset, generated closed token roles/static lookup |
| `Avatar` | [Shared avatar contract](../shared/README.md#avatar): Google-style single initial (or first+last pair), FNV-1a identity tone from a caller stable ID, caller-authorized picture over initials with failure fallback; decorative unless given `AccessibleName` |
| `TOKEN_CSS`, `FOUNDATION_CSS` | Compile-time CSS inclusion; Trunk hosts can link the same files directly |

Apps own localized strings, unique stable IDs, values, routes, async effects, validation and permission
explanations. No primitive computes approval, revision, production, QC or publication eligibility.
Use a caller-owned `fieldset`/`legend` for radio groups, headings for regions and routes for navigation.
Children are one-time structural slots; reactive closures inside them keep content current. Avoid
putting a second interactive element inside Link/NavItem/FilterChip/Button.

## State and event contract

`ButtonState::{Enabled, Disabled(DisabledReason), Loading(DisabledReason)}` keeps the action label
and visible reason. Both native disabled and the dispatch guard block repeated activation. A loading
reason uses a polite status region. `DisabledReason::new` and `AccessibleName::new` reject blank text.
The same state is used by IconButton and FilterChip; selected state stays caller-owned.

`FieldState::{Editable, ReadOnly, Disabled(reason), Loading(reason)}` applies to text, textarea and
number controls. `ChoiceState::{Enabled, Disabled(reason), Loading(reason)}` applies to select,
checked inputs and range, which have no native readonly state. Render a readable value separately,
or supply a disabled explanation, when a choice must be read-only. Error and help are reactive
`Option<String>`; blank messages are hidden and omitted from their ID associations.

Text and numeric callbacks pass native strings unchanged: **`"0"` stays `"0"`**, and clearing a number
field yields an empty string. The library does not parse, default, clamp, validate or normalize.
The browser still applies native number/range behavior; a range cannot represent arbitrary empty
or out-of-range DOM values. Apps own parsing and valid numeric bounds, including intensity 0.
Composition hooks on TextField/TextArea let apps defer validation/autosave during Vietnamese IME;
raw input events still dispatch. Updating the controlled signal acknowledges an edit. A caller that
rejects an edit should deliberately restore the DOM value; there is no hidden application reducer.
Select applies its controlled value after native options mount; options must have stable values.
Its non-focusable outlined indicator decorates the native control. Scoped `appearance: none`
lets Safari honor the shared minimum control size while native options, keyboard and form
behavior remain in charge.

Dialog triggers must have focus before setting `open`. Set `Button`'s opt-in
`focus_on_press=true` for a pointer trigger (as the gallery and Studio discard action do):
Safari otherwise leaves focus on the body or a previous control, which cannot reliably
serve as the return target. Other buttons retain native pointer focus behavior.
For SSR selection or async option insertion without a value change, bind native option `selected`
to the same caller value (or remount after reconciling the options). Opaque children are not observed
by a hidden mutation observer.

`class` hooks retain component classes and are escaped as attributes. Fields preserve external
`described_by` IDs and append only visible help/error/reason IDs. Action hooks include description
IDs and typed expanded/controls state. The library does not offer raw HTML, arbitrary spread
attributes, or aria-label overrides that contradict visible text. Callers must keep each ID and its
`-help`, `-error`, `-reason`, `-title`, `-description` namespace unique within the mounted root.
`LinkHref` rejects blank, control-character and unsafe-scheme destinations; route decisions stay in apps.

Status/feedback use `Announcement::{Off, Polite, Assertive}`; Off is the default. Hosts keep significant
async regions mounted and own deduplication/throttling. Loading keeps visible content and uses a
static indicator; no continuous animation or delay is needed. Keep the announcement outside any
caller-owned busy content region so assistive technology can announce it. Tone is supplementary
to text/icons.

## Styling and gallery

Load [`tokens.css`](tokens.css), then [`foundation.css`](foundation.css), then scoped app styles.
Edit [`foundation.css.in`](foundation.css.in) and run `python3 scripts/ui_tokens.py`; generated files
are committed and must not be hand-edited. All selectors are scoped by `.cantos-ui`; semantic values
come from canonical v0.2.0 roles. No per-component dark palette, font download or artwork is bundled.
System/local font fallbacks remain explicit. Layout measures (75ch/76rem and dialog maximum) are
renderer geometry; spacing, colors, type, shape, targets, density and focus use shared tokens.

Button and FilterChip labels use the shared `cui-control-label` anatomy hook. Supporting browsers
center the font's cap-to-baseline box with `text-box-trim`, keeping icons in their own flex item;
other browsers retain the token line-height. Token padding and visible overflow preserve Vietnamese
accents and descenders, including wrapped labels. App-owned native actions can use the same label
span without adopting a component. Use Button's `icon` prop for a leading icon and keep composed
label markup inside its children. This Web-only geometry change leaves CMP typography unchanged.

The [development gallery](preview.html) shows every component with long Vietnamese text. Its
discovery catalogue consumes the [shared component registrations](../shared/preview-catalog.json)
and generated [token metadata](src/preview_catalog.rs), including stable IDs, sample states and
separate Leptos/CMP status and reasons. Registration is not runtime qualification.

Search matches all whitespace-separated words, case-insensitively, against component API names,
IDs, family names, sample states and renderer status. Select a family or follow a catalogue link
to its actual examples. Theme/density settings and native family navigation stay visible; other
sample regions use `hidden` without remounting their controls. The controlled Dialog remains
outside these filters, and its trigger keeps `focus_on_press=true`. The reset action reveals all
samples. A separate read-only token search exposes every semantic name, its generated CSS variable,
both theme values, density overrides and aliases. Open a token to inspect its specimen: active-theme
color swatches; Vietnamese font-family/size/weight/line-height samples; actual spacing, shape,
control/icon dimensions and stroke geometry; static shadow/opacity samples. Every specimen binds
only its generated CSS role, so theme/density changes update it without copying values. Breakpoints,
motion and unsupported dimensions remain labelled value-only; no animation or viewport simulation
is implied. Native disclosure controls keep this detailed inventory compact.

State, error, theme and density controls exercise reactive contracts; event counts and values
remain visible. [`preview/gallery.css`](preview/gallery.css) scopes discovery layout to this
diagnostic app and consumes shared token roles; it does not alter production component styles.
Developer-only catalogue copy stays local to the preview. Add new APIs to the shared registration
source and provide actual examples/tests in both renderer previews before seeking production
qualification; do not copy the gallery into an app route.

Its native string-render tests cover anatomy/escaping/ID uniqueness, generated catalogue coverage,
pure discovery matching, hidden-only filtering, rendered token values and specimen anatomy. They
do not establish browser focus retention, modal behavior, accessibility or visual fidelity.
The preview feature is diagnostic CSR, not a production route or service:

```sh
CARGO_BUILD_JOBS=2 cargo xtask verify web
```

This canonical gate lints all three WASM consumers (including the preview feature), builds both
apps and builds the gallery into `target/ui-foundation-preview`. For changed-path checks use
`cargo xtask preflight --changed-only`; `cargo xtask verify repository` checks generator drift,
icons and Python examples, while `cargo xtask verify rust` checks native workspace examples.
No gallery production route or service is added; the separate CMP preview owns its own runtime
evidence and never inherits browser or native HTML-string test results.

Runtime focus/dialog/keyboard, exact responsive widths, text scaling, forced colors, motion and
screen-reader observations have separate evidence in the [component record](../../../docs/evidence/ui-system-components.md).
Continue with the [app integration sequence](../../../docs/work-plan/015-leptos-ui-system.md).

Filled text, search, number, textarea and native select controls use `surfaceContainerHigh` with
persistent labels, help and error descriptions. Hover adds the shared state layer; keyboard focus
retains the full contrast outline. Invalid controls use an error surface and transient bottom cue
plus text. Forced colors retains a functional boundary. Grouping uses spacing and tonal surfaces,
not permanent field outlines or section separators.

## Consumer map

The CMP foundation sequence adds companion development specimens in
`preview/foundation_samples.rs`: nested theme/density, composed Button children, blank errors,
native selection/composition/Enter observers. They use existing public APIs and app-composed native
control anatomy; real application imports and behavior are unchanged. HTML number controls retain
their selection limitation and min/max/step behavior. These diagnostics do not claim OS IME or
native parity; see [session 1 evidence](../../../docs/evidence/cmp-ui-system-session1.md).

| Primitive / contract | Studio Web | Theatre Web |
| --- | --- | --- |
| UiRoot + Theme + Density | app root, appearance selector; comfortable targets and compact pane spacing | themed skeleton, appearance selector; comfortable targets |
| Container / Surface / Stack / EmptyState | fluid shell; app owns editorial panes | reading measure and honest unavailable-listener state |
| Button / FilterChip | language action, pane navigation, guarded revision and casting actions | no listener actions implemented |
| TextField / Select | development token; theme; ContextSelect for casting/catalog choices | native theme selector only |
| FieldLabel / FieldError + native control classes | buffered editor/casting inputs, retaining NodeRef/IME/undo; app-owned submit buttons | none |
| Status / Announcement | stable revision/validation/production feedback, visible dialog errors | empty state has no automatic announcement |
| Dialog | explicit browser-draft discard; caller owns approval, open/error state and callback | none |

`cui-button`, `cui-button-field`, `cui-button-reason` and `cui-field-control` are also the native
anatomy style hooks used with FieldLabel/FieldError when an app must retain buffered input or a
real `type="submit"` form. State/guards/description IDs then remain caller-owned. Ordinary actions
should use Button; a native form submit is intentionally distinct from its non-submit contract.

Studio's ContextSelect memoizes value/options as in PR29, binds option selected attributes for
initial/late choices, and uses the shared error/reason associations. App unsupported descriptions
move from `<id>-unsupported` to `<id>-error` for migrated selects; accessible labels stay separate
from the complete retained-value note. Buffered numeric inputs keep their existing unsupported ID.
No library API, tokens or icon geometry changed during integration. [Evidence](../../../docs/evidence/ui-system-integration.md)
records app regressions, builds and runtime gaps; component-gallery evidence alone never verifies
an app. That historical integration record does not qualify the later
[CMP development preview](../cmp/README.md) or its native runtime.

## Session 2 interaction counterparts

The development gallery adds stable-value native Select fixtures for reordered, empty, missing
and unsupported options plus interruption; a stepped native range records input/change; layout
and recovery slots exercise semantic gap/insets and explicit announcements. Choice label and
retained Dialog input specimens remain native HTML. CMP in-place Tab selection/native disabled
navigation and HTML anchors are explicitly different. No existing Web library API or app import
changes. See [session 2 evidence](../../../docs/evidence/cmp-ui-system-session2.md) for the 48
executed package tests, real Safari observations and incomplete acceptance matrix.
