# Leptos UI library

`cantos-ui` pins Leptos **0.7.8**, matching the apps. Enable `csr` in browser consumers.
The unpublished v0.2.0 foundation API is preserved; the component slice adds variants and exports.
No HTTP, backend, provider, domain rule, browser storage or playback dependency enters this crate.

## Components and ownership

| API | Contract |
| --- | --- |
| `UiRoot` | Reactive `Theme` and `Density`; one scoped boundary, optional class and children |
| `Button` | Native `type="button"`; primary/tonal/secondary/text/danger; reactive `ButtonState`, `on_press`, children |
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

The [development gallery](preview.html) shows every component with long Vietnamese text. State,
error, theme and density controls exercise reactive contracts; event counts and values remain visible.
Its native string-render tests cover anatomy/escaping/ID uniqueness, not browser accessibility.
The preview feature is diagnostic CSR, not a production route or service:

```sh
cd packages/ui-system/leptos
CARGO_BUILD_JOBS=2 NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview
CARGO_BUILD_JOBS=2 NO_COLOR=true trunk serve preview.html --locked --dist ../../../target/ui-foundation-preview --port 8083
```

Focused checks from repository root:

```sh
python3 scripts/ui_tokens.py --check
python3 scripts/ui_icons.py --check
CARGO_BUILD_JOBS=2 cargo test -p cantos-ui --locked
CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui --all-targets --locked -- -D warnings
CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui --all-targets --target wasm32-unknown-unknown --features preview --locked -- -D warnings
CARGO_BUILD_JOBS=2 cargo clippy -p cantos-ui -p cantos-studio --target wasm32-unknown-unknown --features cantos-ui/csr --locked -- -D warnings
cargo fmt --all -- --check
```

Runtime focus/dialog/keyboard, exact responsive widths, text scaling, forced colors, motion and
screen-reader observations have separate evidence in the [component record](../../../docs/evidence/ui-system-components.md).
Continue with the [app integration sequence](../../../docs/work-plan/015-leptos-ui-system.md).
