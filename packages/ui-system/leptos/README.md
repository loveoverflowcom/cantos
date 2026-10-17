# Leptos UI library

`cantos-ui` pins Leptos **0.7.8**, matching the apps. Enable `csr` in browser consumers.
The unpublished v0.2.0 foundation API is preserved; the component slice adds variants and exports.
No HTTP, backend, provider, domain rule, browser storage or playback dependency enters this crate.

## Components and ownership

| API | Contract |
| --- | --- |
| `UiRoot` | Reactive `Theme` and `Density`; one scoped boundary, optional class and children |
| `Button` | Native `type="button"`; primary/tonal/secondary/text/danger; reactive `ButtonState`, `on_press`, label children and optional decorative `icon` |
| `BackButton` | Typed localized name + required host callback; native non-submit contextual return, visible or icon-only label; no route/history ownership |
| `IconButton` | Same action state with an official `IconName` and nonempty `AccessibleName`; label remains in accessible text |
| `Link`, `NavItem`, `NavTabs` | Native links and labelled navigation; reactive safe `LinkHref`, `aria-current="page"`, optional icon; NavTabs is ordinary link navigation, not an ARIA tab widget |
| `FilterChip` | Controlled `selected`, native `aria-pressed`, `on_change(bool)`; blocked/loading reason |
| `TextField`, `TextArea`, `NumberField` | Required stable ID, visible label, controlled raw string and `on_input(String)`; editable/readonly/disabled/loading |
| `Select` | Native option children by default; opt-in `options: Signal<Vec<SelectOption>>` Web menu candidate in preview; controlled string, enabled/disabled/loading |
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

Button and IconButton replace a decorative action glyph with a stationary indeterminate ring
while loading, including a Button without an icon. The original label and localized reason stay
visible; progress is neither a disclosure chevron nor a success marker. IconButton also exposes
its reactive accessible name as a native title tooltip. Glyphs remain 24 logical units with an
8-unit label gap through existing tokens; there is one native action target. The
[shared action map](../shared/icons/material-outlined/README.md#action-semantics-and-consumers)
owns registry meaning; new app imports still require preview admission.

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

## Web Select menu candidate — preview only

For the `select` recipe, pass `options` instead of native option children. `SelectOption`
carries an opaque stable unique value, localized label, optional content language and disabled
fact. `placeholder` describes a missing value; it never selects or dispatches the first option.
Existing callers with native option children retain their current implementation. Do not pass
both option children and `options`: the latter selects the menu presentation.

The menu uses a native non-submit button with select-only combobox semantics, a nonmodal
Popover listbox and `aria-activedescendant`. Arrow keys/Home/End and localized typeahead browse;
Enter/Space/Tab or leaving focus acknowledges the active enabled option. Escape or toggling
the trigger discards browsing. Pointer selection acknowledges once. The caller must update
`value` to accept a change. A changed option list/value/availability cancels an open browse.
No async/domain decision, preference storage or OS-theme lookup enters the component.

The browser adapter constrains the top-layer popup to the visual viewport, matches the trigger
width, aligns its leading edge (including RTL), flips above when necessary, and repositions on
scroll/resize/trigger reflow. Inset and offset use the computed token padding. Listener/observer
cleanup belongs to the mounted owner. Selected, active, disabled and error are separate facts;
the selected check remains visible in forced colors. There is no menu animation or delayed intent.

Unsupported Popover engines, SSR and an unsuccessful enhancement retain the native select;
its OS popup is **not** claimed to match M3. The native control retains form name/required/value,
and native invalid submission focuses the enhanced trigger when present. Keyboard/focus,
touch, form validity, viewport positioning and assistive technology still require browser
acceptance; native string rendering does not prove them.

This is an unqualified Web presentation of the existing `select` API. The shared baseline and
qualification gate remain intact; CMP layout is unchanged. **Account/Settings/topbar migration
is deferred** until preview admission under the
[existing policy](../../../.agents/skills/cantos-ui-design/references/preview-catalog.md).
See [#221 evidence and continuation](../../../docs/evidence/web-select-221.md).

## Styling and gallery

Load [`tokens.css`](tokens.css), then [`foundation.css`](foundation.css), then scoped app styles.
Edit [`foundation.css.in`](foundation.css.in) and run `python3 scripts/ui_tokens.py`; generated files
are committed and must not be hand-edited. All selectors are scoped by `.cantos-ui`; semantic values
come from the [canonical source](../shared/tokens.json). No per-component dark palette or remote
font/artwork download is required. Official Logo Kit SVG documents are bundled locally and their
letterforms are paths. The preview loads [licensed local Noto fonts](../shared/assets/noto-fonts/README.md)
without host-installed font lookup; product hosts retain explicit font-delivery ownership.
Font readiness before capture is separate from bundle/source checks. Layout measures
(75ch/76rem and dialog maximum) are
renderer geometry; spacing, colors, type, targets, density, focus and the pill shape use shared tokens.
Regular corner radii use the Web-only `--cantos-web-shape-{control,reading,container,overlay,panel}`
roles declared in `foundation.css.in` ([mapping](../shared/README.md#web-shape-mapping)); they never
read the regular shared CMP shape scale directly. Opt-in auth/account variants use their separate
canonical semantic roles without modifying that established Web mapping.

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
Developer-only tool copy is owned by [`preview/messages.rs`](preview/messages.rs). The tool shell uses fixed English, matching the CMP preview, including the boot document and accessibility names. Recipe locale selects bilingual component specimens only; it does not introduce an app locale preference. Legacy Vietnamese typography, form and content fixtures remain verbatim inside `lang="vi-VN"` fixture regions. Registry identifiers and diagnostic output remain developer evidence rather than app resources. Add new APIs to the shared registration
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

## Scenario recipes and replay

The existing preview includes a local scenario workbench. Its bounded pure
[`Recipe`](preview/recipe.rs) selects a production component, typed state, public fixture and
presentation environment. The [host](preview/workbench.rs) owns values, callbacks and reset;
the [browser adapter](preview/platform.rs) owns URL replacement and observed metadata. The
recipe is diagnostic data, never a route, credential, media URL or service request.
The full API/token catalog stays available; exact recipe support is intentionally bounded:

| Recipe component | Catalog API / variant | Supported states |
| --- | --- | --- |
| `button` | `button` / regular `Button` | `enabled`, `disabled`, `loading` |
| `auth-primary` | `button` / `ButtonVariant::AuthPrimary` | `enabled`, `disabled`, `loading` |
| `filter-chip` | `filter-chip` / `FilterChip` | `unselected`, `selected`, `disabled`, `loading` |
| `switch` | `switch` / native `Switch` | `off`, `on`, `disabled`, `loading` |
| `dialog` | `dialog` / native controlled `Dialog` | `closed`, `open` |
| `select` | `select` / opt-in Web menu candidate, Theme + Language examples | `enabled`, `disabled`, `loading`, `error`, `missing-value`, `empty-options` |
| `back-button` | `back-button` / opt-in `BackButtonVariant::Unified`, Account/Sign in/icon-only plus unchanged IconTonal baseline | `enabled`, `disabled`, `loading` |

The canonical query is versioned. For example, append this query to the local preview URL:

```text
?v=1&component=auth-primary&state=enabled&fixture=long-vietnamese&theme=dark&density=compact&locale=vi&text=200&frame=390&motion=system&mode=isolated&playback=live
```

| Field | Accepted values |
| --- | --- |
| `v` | `1`; required for every nonempty query |
| `component`, `state` | Table above; an omitted state uses the component's first listed state |
| `fixture` | `standard`, `long-vietnamese`; repository-created text only |
| `theme`, `density` | `system`, `light`, `dark`; `compact`, `comfortable` |
| `locale`, `text` | `vi`, `en`; `100`, `200` percent text scale |
| `frame` | `320`, `390`, `840`, `1440` CSS px specimen width |
| `motion` | `system`, `reduced`, `force-full` |
| `mode` | `catalog`, `isolated`, `matrix` |
| `playback` | `live`, `settled`, `replay` |

No query defaults to Button/enabled, standard fixture, Light/Compact, Vietnamese, 100% text,
390px frame, System motion, Catalog and Live. With version 1, omitted fields use those defaults,
apart from state, which uses the selected component's valid default. Unknown versions, fields,
values, duplicate fields, invalid encoding and incompatible component/state pairs are rejected
atomically. The host shows a reason and the complete default recipe rather than partially
applying input. Limits are 1024 query bytes, 12 fields, 16 bytes per key and 32 bytes per value;
recipes cannot inject arbitrary text, URLs or CSS.

Catalog retains the existing discovery gallery beside the selected specimen. Isolated shows one
selected component/state; Matrix mounts each supported state for that component. All modes use
the production APIs above, with unique IDs and persistent preview-owned values. Filtering,
environment controls and hiding/revealing samples retain edits. Reset restores fixture values,
state and event history to the currently selected recipe. Changing theme or density is not reset.
An intent log records callbacks; it never simulates authentication, audio or a backend result.

Frame width is local specimen geometry; narrow browser windows may need to scroll the diagnostic
frame. It does not resize the browser viewport or alter DPR. Text 200% changes injected font
scale and does not claim native browser page zoom. Record both the selected frame/scale and
actual browser viewport/DPR/zoom in evidence. Long Vietnamese deliberately exercises wrapping.
Locale applies to workbench specimen copy through typed preview-owned resources; the fixed English tool shell and legacy Vietnamese samples keep their own language tags. Changing specimen locale does not reset the mounted controls or their host-owned values.

System motion preserves `prefers-reduced-motion`; Reduced further suppresses nonessential
transitions. Force full is a labelled opt-in diagnostic override and must be attributed in the
result. Settled suppresses transitions for final-state capture. Replay restores the selected
fixture and event log, then waits for real input; it does not synthesize a press or guarantee a
recording. Press/release/cancel and selection follow native controls. Dialog uses actual focus
containment/return and Escape; NavTabs remains ordinary links. No drag-sheet or mini/full-player
gesture consumer is introduced, so its slop/axis/velocity/snap policy remains unsupported.

Use the existing command surface:

```sh
cargo xtask ui-preview leptos build
cargo xtask ui-preview leptos verify
cargo xtask ui-preview leptos run
```

Discover the component contract, select a recipe, copy its URL and reload. Drive real pointer and
keyboard input, check semantic state and callback history, capture and open the affected states,
then reset/repeat the same recipe and input sequence for a fix. Wait for the local font diagnostic
to confirm the declared faces are loaded. Motion claims need normal/reduced recordings or frame
profiles with labelled early/mid/final states and interruption; SSR and still PNGs cannot establish
timing or cancellation. Browser metadata refresh/export records observed facts separately from
selected settings. The preview-only `motion_probe.rs` observes trusted Button/FilterChip input,
records computed styles/bounds at start/middle/end (or an explicit unavailable midpoint), and
cancels its bounded frame sampler on new input, hidden/offscreen targets, recipe/reset changes
and unmount. Recipe changes clear old samples so export cannot attribute them to another
fixture. This sampled profile is not a video, controlled-clock proof or performance benchmark.
The capture label is local diagnostic text; it never enters a recipe URL. Build-source provenance must identify the actual served source/dirty state;
an unrecorded build field is not a known SHA. Keep artifacts in the existing ignored evidence
directory and attribute token/catalog digests, component/variant/state, fixture, fonts, renderer
and browser along with the recipe. See the
[replay method](../../../.agents/skills/cantos-ui-design/references/preview-catalog.md#replay-a-preview-scenario)
and [actual evidence](../../../docs/evidence/leptos-preview-scenarios.md).

Recipe parsing/string-render checks, browser DOM, keyboard/focus, opened pixels and motion
recordings are separate evidence. This diagnostics feature is absent from product routes and
does not qualify new APIs, expand shared admission or replace Studio/Theatre acceptance.

Filled text, search, number, textarea and native select controls use `surfaceContainerHigh` with
persistent labels, help and error descriptions. Hover adds the shared state layer; keyboard focus
retains the full contrast outline. Invalid controls use an error surface and transient bottom cue
plus text. Forced colors retains a functional boundary. Grouping uses spacing and tonal surfaces,
not permanent field outlines or section separators.

## Auth/account design-v2 specimens

The opt-in `ButtonVariant::AuthPrimary`, `AuthSecondary` and `AuthOutlined` reuse the native
non-submit Button and its controlled loading/disabled reasons and synthetic-event guard.
Only these variants use the canonical auth CTA dimensions, capsule shape and interruptible
pressed radius. The leading primary icon is decorative, with one visible text label. The
outlined variant uses the fractional auth outline role; regular secondary boundaries and the
existing focus width remain unchanged. The CSS transition is not a native Material spring.
Reduced motion removes that transition without delaying intent delivery.

`TextFieldVariant::AuthTonal` changes only the controlled TextField's minimum height and radius.
Native type/autocomplete, raw values, persistent labels, composition callbacks, read-only
selection, help/error/reason associations and caller-owned validation stay the existing contract.
`SurfaceVariant::{Auth, Profile, Membership, AccountGroup}` selects scoped geometry independently
from tone. Pair a profile's `SurfaceTone::PrimaryContainer` with its inherited
`onPrimaryContainer` foreground, including explanatory text, rather than muted body ink.
No variant changes regular listening/editor controls, reading regions or mini-player geometry.

`BackButton` requires a nonblank localized `AccessibleName` and an `on_back` callback. Its native
button is never a submit/link, and blocked states preserve a visible reason. The default shows
the same name next to the circular arrow; `icon_only=true` retains that name as visually hidden
text. On Web, the outer action remains transparent in enabled, disabled and loading states;
blocked text keeps the full-contrast disabled role. The neutral Back specimen makes unintended
filled label capsules visible in both themes. The host alone resolves contextual return
destinations, cancellation and draft/scroll retention. This primitive does not call browser history, choose a fallback route, authenticate,
log out or implement native system Back.

`BackButtonVariant::Unified` is a Web-only, preview-only candidate for
[#222](https://github.com/loveoverflowcom/cantos/issues/222). The native button owns one
continuous hover/pressed layer, token inline/block padding and the 48-unit minimum target;
its decorative arrow has no separately painted circle. The outer pressed shape uses
`shape.authPressed`; keyboard focus retains the shared focus roles. Long labels wrap, and RTL
mirrors the arrow while keeping logical start alignment. Reduced motion removes the shape
transition. This uses existing tokens without changing their canonical values or CMP layout.
`IconTonal` remains the default so existing product callers retain their presentation until
required admission. The recipe workbench includes VI/EN, Account/Sign in, long text and
icon-only specimens with host intent logs; actual hover/focus/press requires native input.

The actual [auth specimens](preview/auth_samples.rs) live in the existing actions, fields,
navigation and layout gallery families. They expose host-driven enabled/loading/disabled and
field error/read-only states, long Vietnamese labels, retained values and intent counters.
Press/keyboard-focus states use real native interaction, not a fake screenshot state flag.
Ordinary controls sit alongside the scoped variants as regression specimens. The existing
theme controls apply light/dark to all examples. Check actual 320px reflow and browser 200%
text scale; no fixed label height or text clipping is introduced by the variant.

The [semantic tests](tests/auth_account.rs) assert native action names/blocked descriptions,
retained escaped input values, field error associations and observable package fixtures.
String rendering does not execute browser events or establish focus, clipping, motion or
screen-reader behavior. These APIs remain **implemented-unverified and unqualified for app
adoption** until both real Leptos/CMP previews satisfy the shared admission contract. They add
no application route, auth/payment integration or backend authority.

## Web AuthTonal spacing candidate — preview only

`TextFieldVariant::AuthTonalSpacious` keeps the existing AuthTonal 56-unit minimum and
20-unit corner while mapping label/control/message gap to `space.sm`. Its visible keyboard
focus has width and offset `2 × size.border`, using `color.focus`; forced colors use `Highlight`.
This reduces the outward focus footprint from seven to four units without changing layout,
native email/autocomplete/required facts, controlled values or external error ownership.
Regular and existing AuthTonal callers retain their geometry. No shared token changed.

Recipe `component=auth-tonal` maps to catalog `text-field`; states are
`enabled|error|disabled|loading|readonly`. VI/EN and standard/long fixtures render the actual
candidate, help/error/reason, a local-only AuthPrimary CTA with `space.lg` gap and the existing
AuthTonal/`space.md` baseline. The CTA records a host intent, never submits a form. The gallery
adds the same candidate in its fluid field section for actual narrow-viewport reflow; workbench
frame widths remain fixed diagnostic frames and may scroll when wider than their host.

Example: `?v=1&component=auth-tonal&state=error&fixture=long-vietnamese&theme=dark&density=comfortable&locale=en&text=100&frame=320&motion=reduced&mode=matrix&playback=live`.
Hover/focus/autofill/forced colors need real browser input or explicitly recorded emulation.
The three added catalog diagnostics do not qualify either renderer. CMP metadata lists the
Web diagnostic names only; it implements no new CMP geometry. Both-renderer/design admission
and actual signup-form observations remain pending. Do not migrate product imports or change
the app's form gap until that gate is met. See [#223 evidence](../../../docs/evidence/web-auth-tonal-223.md).

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
