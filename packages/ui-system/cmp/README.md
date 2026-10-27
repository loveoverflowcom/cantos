# Cantos CMP UI-system preview

This package is a **development-only preview consumer** of the shared UI system. It now has
Kotlin Multiplatform `commonMain` component wrappers, a Compose Desktop JVM host and an isolated
Android library for Android Studio host previews. It is
neither Cantos Theatre nor Cantos Studio mobile, and implements no API, authentication,
navigation shell, downloads, audio session or playback adapter.

## Ownership and invariant

Every shared declaration in [`preview-catalog.json`](../shared/preview-catalog.json) must have
one real native example or an explicit unsupported explanation. The common package exposes
one wrapper per registered API (32 at present) with the same component names as Leptos.
Close/retry labels, accessible names and visible content come from the caller. Caller-owned
values and callbacks remain
separate from rendering. Disabled/loading actions require a nonblank visible reason;
read-only text stays selectable and does not dispatch edits. Native choice controls have no
read-only mode.

The catalog is for inspecting an individual design and its states **before** adopting it into
an application. An `implemented-unverified` label is not evidence that a state passed.

### Foundation API ownership

`ComponentState.kt` owns reusable names/reasons, controlled availability and renderer enums;
`PreviewModel.kt` owns diagnostic discovery/selection. `CantosTheme.kt` owns scoped token mapping,
validated preferences and native minimum interactive size. Nested themes replace the preview
text multiplier against the inherited platform font scale; they do not multiply it again.
`Actions.kt` and `Fields.kt` own their native wrappers, sharing focus/state decoration through
`ComponentModifiers.kt`. `Choices.kt`, `Navigation.kt`, `Layout.kt` and `Feedback.kt` now own their native contracts;
`Components.kt` retains the root, field messages and decorative icons. The isolated Android diagnostic host shares these sources directly and adds tooling only.

`Avatar.kt` owns the pure [avatar contract](../shared/README.md#avatar) (`avatarInitials`,
`avatarSeed`, `avatarTone`) and a renderer for a caller-decoded `ImageBitmap` or the initials
fallback. It is registered in both preview catalogues (`avatar`, seven states) with the Leptos
`Avatar`; no application may adopt it before both renderers qualify.

### Theatre expression and Floating fields ([#452](https://github.com/loveoverflowcom/cantos/issues/452))

`UiPreferences(expression = CantosExpression.Theatre)` opts a subtree into the token v0.7.0 remap
(`tokenColor`, the Material scheme and `typeStyle` resolve through `CantosTokens.role`); the default
`Standard` resolves every role to itself, and a nested `CantosTheme` that names none leaves the scope.
Everything below is additive and local to the control that opts in:

- `TextFieldVariant.Floating` / `FloatingSearch` keep the label inside one tonal shell, draw one
  inset state perimeter (`cantosInsetPerimeter`: idle none, invalid 1, focus 2, focused-invalid 2 in
  the error colour, always inside the node's bounds) and keep help outside it. Minimums are 68 and
  72 logical units (`FloatingFieldMinHeight`, and `size.searchHeader` for the search); text scale and a
  long label grow the shell. `AuthTonal` paints the same perimeter on `shape.authField` ([#478](https://github.com/loveoverflowcom/cantos/issues/478));
  `Regular` is the Standard field and keeps the outset `cantosFocus` ring. `FloatingSearch` (Search v4.3)
  has no floating label: its label is the empty prompt and the field's accessible name, the radius is
  `shape.searchRest` (36) and a pointer hover paints one inset `size.border` (touch has none). Opening,
  closing and the dock body belong to the Search composition; native keeps its motion scheme, with no
  copy of the Web 280/200 ms seed.
- `TextField(..., trailing = { IconButton(...) })` on both the `String` and `TextFieldValue`
  overloads places one action inside the shell. The layout reserves its width; the field owns no
  action or visibility state, and the caller's selection and composition are untouched by a press.
- `IconButton(variant, circular)`: `Text` (default) is the existing plain muted glyph; `Tonal` and
  `Primary` fill a 48-unit container (a reading-radius square, or a circle when `circular`). A docked
  search action passes `controlModifier = Modifier.size(tokenDp("size.searchAction"))`.
- `FilterChip` under the Theatre expression is a tonal 12-radius chip that becomes an ink capsule
  with its check when selected; it keeps its start edge and 48-unit height, so only its width grows.
  The shape change uses the interruptible feedback motion that reduced motion removes.
- Focus perimeter ([#478](https://github.com/loveoverflowcom/cantos/issues/478)): a painted action keeps
  one focus perimeter on its own edge, never outside it. `Button` (and `BackButton`) draw
  `cantosInsetPerimeter` for every auth variant and for every variant under the Theatre expression;
  `IconButton` and `FilterChip` do under the Theatre expression. The stroke is `2 × size.border`, its
  corner follows the measured shape (so the pressed auth capsule keeps it on the edge), and it paints
  over the content with no layout, size or hit-area change. `Primary` and `AuthPrimary` fills are the
  focus hue itself (about 1.1:1), so their ring is `color.onPrimary`, drawn one stroke (2 dp) inside the
  fill so it cannot merge with the page; a selected Theatre chip's ring is `color.onPrimaryContainer`,
  inset the same way; every other fill uses `color.focus` (at least 3:1, contract-tested). The
  Standard expression, unpainted controls (`Link`, `NavTabs`, `Checkbox`/`Radio`/`Switch` rows,
  `RangeField`) and menu items keep the outset `cantosFocus` ring, which needs parent spacing. The
  `AuthTonal` label/help gap still includes that clearance, so form rhythm is unchanged.
- `Select` under the Theatre expression is one tonal capsule whose value and chevron are one
  centered group, with a single 2-unit inset cue while open or focused. A nonblank error uses
  one error-coloured inset unit at rest and two while focused or open; blank errors add no cue.
- `Avatar(size = AvatarSize.Control, accent = true)` is a 48-unit accent avatar; `accent` pins
  `color.theatrePink` and leaves initials and semantics unchanged.

The preview adds Standard/Theatre expression chips and the 11 registered `theatre-*` specimens,
and `TheatreCaptureTest` writes each in both themes at phone width, 2x text and desktop width.

String field calls remain compatible. `TextField`, `TextArea` and `NumberField` also accept
caller-owned `TextFieldValue`, retaining text/selection/composition without conversion. Return
received values promptly and do not normalize composing text. Read-only permits selection changes
but rejects text/composition edits; disabled/loading blocks all callbacks. The caller owns a
transition out of active IME composition before locking a field; real platform commit/cancellation
is still unverified. All overloads expose native `KeyboardOptions`/`KeyboardActions` and an optional
`visualTransformation` (for example `PasswordVisualTransformation`, whose visibility is caller-owned); custom
actions are guarded by editability. TextArea exposes positive ordered min/max line limits.
NumberField keeps raw `0`, empty and incomplete localized decimals; keyboard hints are not numeric
validation. Blank help/error messages have no visible or semantic error state.

`Button(name, onPress, ..., content)` adds a RowScope content slot with a required localized
accessible name that includes its visible label. The existing label overload delegates to it.
For Button/IconButton/FilterChip, `modifier` still applies once to the whole field; new
`controlModifier` attaches focus, geometry and semantics to the actual native control. It does not
silently move existing root modifiers. Decorative Icon now inherits the surrounding content tint,
with an explicit tint override when needed. Focus consumes width/offset and the control's shape;
text fields use `shape.field`. Parents need spacing for an outset focus ring; inset perimeters need none.

The [session 1 evidence](../../../docs/evidence/cmp-ui-system-session1.md) records 41 executed CMP
tests and inspected captures. Both previews declare nested-theme, composed-content, selection,
ime-action and blank-error examples: 29 APIs, 129 scenarios, unchanged 73 tokens. OS Java-window
input is still blocked, native/mobile/screen-reader/full reference evidence is incomplete and all
qualifications remain unapproved. Its continuation is recorded below; [session 3](../../../docs/work-plan/016-cmp-ui-system.md#exact-next-task-for-session-3) owns final review/acceptance.

### Controlled choice, navigation, layout and feedback APIs

Choice labels are one full-row native target with guarded callbacks, a control modifier and
associated help/error/reason. Material's visual checkbox/radio/switch has no second handler.
Stable-value SelectOption lists support null/missing/empty selection and unavailable options;
localized placeholder/empty/menu descriptions are required, and external input changes close
open menus. The fixed-list index overload remains compatible.
The trigger reserves a trailing disclosure slot and options reserve a trailing selected-check
slot; loading uses a separate pending cue. Popup minimum width follows its measured anchor.
The [#229 evidence](../../../docs/evidence/theatre-cmp-action-icons-229.md) separates JVM
semantics/captures from the outstanding reference, native-window, device and AT acceptance.

RangeField accepts finite in-range values, 0–1000 interior ticks, localized valueText, completion
and keyboard alternatives. Stepped configurations need finite distinct native Float ticks;
controlled values match a tick within one ULP. The bounded core uses the actual pinned native
Float interpolation and rejects collapsed precision before dispatch. Continuous arrow keys move
one percent of the range, Page keys ten percent; a completed key interaction dispatches completion.
NumberField still preserves raw text and leaves numeric validation to its caller.

NavTabs is named native in-place selection with keyboard focus and adaptive full-label reflow,
while Leptos NavTabs retains anchor navigation. Dialog accepts initial/return FocusRequesters
and native properties; keep it mounted across close, keep targets valid and retain input in the
caller. Default outside-click dismissal is off. Link/NavItem expose native availability.
Space controls semantic gap/insets; Reading Surface maps reading shape/type intent; horizontal
Stack wraps. Loading/Error announcements default Off and support explicit urgency. ErrorState
has a composable recovery slot and retains its retry overload. Callers own announcement policy.

The [session 2 evidence](../../../docs/evidence/cmp-ui-system-session2.md) records 64 executed JVM
tests, 13 new opened state captures, bounded Safari counterpart observations and independent
review. Both catalogues now declare 151 states for the same 29 APIs and unchanged 73 tokens.
CMP OS-window binding remains BLOCKED; native/mobile/IME/screen-reader/motion/reference acceptance
remains incomplete. All qualifications stay unapproved before app adoption.

## Preview contents

- Search and component-family filtering; one selected component and its registered state list
- Real button, icon-action, field, choice, selection, slider, tab and dialog callbacks
- Light/dark/System and compact/comfortable profiles, 1×/1.3×/2× text-scale probes and named dp
  viewport constraints (320, 390, 600, 840, 1140)
- A read-only Design v3 tab: every design specimen with its design status, ownership scope, owner,
  platforms, reused registered components, anatomy and state matrix (see below)
- A live evidence line with the viewport constraint (dp), host density (px per dp), host font scale
  × preview multiplier = effective font scale, and the fixture locale
- Named token catalog with color swatches, raw values, aliases, density and reduced-motion overrides
- Official Apache-2.0 Material Outlined SVG paths mapped to decorative Compose vectors
- Vietnamese fixtures such as `Người dẫn chuyện`, `Ánh đèn cuối sân khấu`,
  `Nguyễn Thị Minh Phương`, `Ỗ, Ữ` and `Ngày mai, mình có diễn tiếp không?`
- An event log that changes only local preview state; no preview action sends a network request

Expanded windows (the window width reaches the canonical expanded breakpoint) use a master/detail
layout; narrower windows use a component selector and a scrollable detail area. Large-text/narrow
modes make the page scroll rather than clipping tools. Nested scroll panes receive bounded height.
Preview-local geometry (master pane, nested sample roots, narrow-sample cap, token bar bound) is
declared once in `PreviewGeometry` with its unit (dp), scope and reason; it is derived from
canonical breakpoints and is not a token definition. Shared components read tokens only.

### Preview tool language and specimen language

`preview/PreviewToolCopy.kt` owns the developer tool's fixed English labels, instructions,
status framing and accessible names. This bounded policy matches the Leptos preview; it adds
no app catalog import, locale preference or production localization library. Tool copy is
separate from Vietnamese typography/accessibility specimen labels and synthetic story data in
`ComponentExample`, `AuthAccountSamples` and `LogoSamples`. Those fixtures deliberately retain
long names, tone marks and wrapping probes. Generated component/inventory names, states, token
roles and diagnostic error identifiers are displayed verbatim for inspection.

`PreviewSettings.locale` remains the **synthetic specimen content language**, independent of
the fixed tool language. It selects a fresh fixture as documented below; it is not an app UI
locale control. This preview policy does not settle the production catalog decision in #167.

## Scenario recipes and replay

The **Workbench** tab renders the same `ComponentExample` used by the catalog and Android preview
wrappers. It adds recipe orchestration; it declares no new shared component or qualification.
Start it through `cargo xtask ui-preview cmp run`, then select the component ID, its registered
state and Catalog, Isolated or StateMatrix mode. A matrix mounts each registered state through
that same sample function with immutable values. It explicitly excludes the open Dialog modal
to keep matrix controls reachable; operate modal and keyboard-focus journeys in Isolated mode.
Interactive `RecipeFrame(interactive = true)` binds the actual focus node for a `focus` recipe;
static Android roots keep that flag false and never claim a keyboard-focus walkthrough.
Workbench filtering, theme, density, text scale and reduced-motion
changes preserve sample values and selection. **Fixture ViVn/En** selects a new localized seed;
**Reset to selected** explicitly restores the selected seed. The ordinary gallery retains its
existing filtered-detail selection contract.

`preview/workbench/PreviewRecipe.kt` owns `PreviewRecipe.of`: a catalog-bound component/state,
mode, nonnegative deterministic seed and injected `PreviewSettings`. Unknown components and
states return typed rejections. `PreviewSettings` carries Light/Dark/System, compact/comfortable
density, vi-VN/en fixture language, 1×/1.3×/2× preview font scale, reduced motion and a bounded
dp constraint frame. The host supplies the system-dark fact; components never query the OS.
The frame constrains the specimen, independently of the actual window, platform density and
platform font scale; record each separately in evidence.

`ComponentSampleState` holds immutable synthetic widget values, including text selection and
composition. `ComponentSampleIntent`, `reduceComponentSample` and `replayComponentSample` own
pure value replacement and ordered intent replay. Selecting a new recipe starts its seed;
reset and replay restore the selected recipe rather than an unrelated catalog default.
Replay is an action/state diagnostic, separate from the manual animation-frame recipes below;
it supplies no pause or seek-to-frame controls and demonstrates no native timer or background
behavior. Fixture controls do not send network, auth or media commands.

`PreviewSnapshot(recipe, sample, events)` is detached data. `PreviewSnapshotCodec.encode` returns
`Exported(payload)` or a typed rejection; `decode` returns `Restored(snapshot)` or a typed rejection.
The strict `CANTOS-CMP-PREVIEW|1` format accepts at most 256 intents and 16,384 characters.
It serializes only fixed original fixture text IDs and allowlisted choices, then checks that the
projection equals replay of the encoded recipe and intents. Free-form edited text stays visible
in the live specimen but export refuses it, avoiding private text, credentials, paths or URLs in
the snapshot. Malformed/unsupported versions, oversized content, invalid recipes/intents and
projection mismatches are rejected before mounting; restore never constructs a live adapter.
No coroutine scope, controller, port, service or native object is serialized.

Keep component ID/API/variant/state and the reason for any extension in the PR, followed by the
recipe/schema/seed, ordered intents, theme, locale, density, font readiness/scales, specimen
constraint and actual renderer/window metadata. Open each attributed capture and rerun the same
recipe after a repair. Follow [preview admission](../../../.agents/skills/cantos-ui-design/references/preview-catalog.md)
before product adoption; source registration, replay and a screenshot do not complete it.

## Design v3 discovery, injected appearance and capture records

The [design inventory](../shared/design-inventory-v3.json) maps all 102 specimens of the offline
v3 design export onto the one preview registry: 32 existing ones are the registered APIs with
their baseline states, and 70 proposals are not public APIs. `scripts/ui_preview_catalog.py`
generates `preview/generated/DesignInventory.kt` from it (display data only; `PreviewCatalog`
stays the runtime registry, and the Theatre mock's build already excludes
`com/cantos/ui/preview/**`, so no product app ships the inventory). The gallery's **Design v3**
tab searches and filters that data by design status, ownership scope and CMP applicability and
shows each specimen's reason, consumer, reused registered components (a link opens the real
example), anatomy, canonical token roles and the design's state matrix verbatim. A proposal has
no sample and offers nothing to adopt; Studio and Admin specimens state that they are Web-only.
Design status, registration and qualification stay separate facts.

Appearance is injected. `ThemeMode { Light, Dark, System }.resolve(systemIsDark)` is pure; the
desktop host (`Main.kt`) and the three Android Studio Design v3 previews are the composition roots
that read the OS once and pass the fact to `PreviewApp(systemIsDark = …)`. Components, nested themes
and `UiPreferences` only see the resolved `Theme`. `ViewportPreset` names the dp constraints used
for evidence (320 small, 390 typical, 600 medium, 840 expanded, 1140 wide); they are layout
constraints, not device models. The gallery prints the actual constraint, host density, the host
font scale times the preview multiplier (and their product) and the fixture locale in its header, so
a capture states what it was rendered with.

`PreviewCaptureTest` writes a JSON sidecar (`cantos-ui-capture-v1`) beside each PNG in
`CANTOS_UI_PREVIEW_CAPTURE_DIR` (default `build/reports/cantos-preview`): scenario, specimen,
theme mode and resolved theme, density, preview text multiplier, dp viewport, bitmap pixels, host
density and host font scale (`host_font_scale` is the host value; the effective scale is that times
the preview multiplier), token version, `tokens.json` byte digest, `CANTOS_SOURCE_REVISION` (or
`unrecorded`) and the PNG SHA-256. It always names its renderer `jvm-desktop-compose-test-root` and lists what it
is **not** evidence of: Android/iOS devices, an OS window, a screen reader or a design-pixel match.
Open the PNG before calling it `screenshot-inspected`. New sidecars also name `font_source`;
`unrecorded` stays explicit for older helpers. The qualification capture roots inject the same
`PreviewFonts.ui` / `PreviewFonts.editorial` bundle as the canonical desktop host, and an
executed nested-text-layout test verifies those families. Platform-fallback/type diagnostics
remain labeled separately; a token family name alone is not proof of delivered glyphs.

## One token source

[`shared/tokens.json`](../shared/tokens.json) is the sole semantic source. The repository generator
writes `src/commonMain/kotlin/com/cantos/ui/generated/CantosTokens.kt`; the icon and catalog
generators write adjacent files. Do not edit these files by hand or add a per-application palette.
`CantosTheme` maps those generated roles to one Material `ColorScheme`, typography and shapes.
`space.*` and wrapper control sizes map to dp; type sizes map to sp. Material primitives retain
native intrinsic metrics. Exact pixel parity with web intrinsic metrics is not claimed.

The source names Noto Sans and Noto Serif. `CantosTheme` takes licensed families as parameters and
a nested theme that names none inherits the host's, so an injected family reaches every nested
specimen. The **desktop** preview host (`Main.kt`) injects the shared bundle through `PreviewFonts`:
Gradle copies the four committed faces and their provenance from
[`../shared/assets/noto-fonts`](../shared/assets/noto-fonts/README.md) onto the classpath, and nothing is
duplicated in Git or read from installed fonts. Its static masters are 400 and 700, so a requested 500
or 600 resolves to the nearest of them. The test captures, the Android Studio wrappers and any
host that injects nothing keep the reviewed platform sans/serif fallback; the Theatre mock injects
its own copies of the same bytes. Glyph placement still needs an opened capture, and reviewed font
resources for each product host remain necessary before app adoption.

Reduced-motion inspection resolves the three canonical duration roles to zero. The pending
ring turns once per `PendingRingTurnMillis` under full motion and holds a visible stationary arc
under reduced motion (`pendingRingTurnMillis`); no canonical continuous-motion token exists yet.
This is not a platform reduced-motion adapter or a claim about
all Material ripple/native motion. Focus uses real Compose focus nodes with token-driven rings;
select the focus preset or use Tab/Shift+Tab. Dialog dismissal requests focus restoration to its
trigger. These behaviors still need the named renderer tests and observation below.

## Motion policy and virtual-clock replay (issue #145)

`MotionPolicy.kt` maps a `MotionIntent` (Feedback, Navigation, Emphasis), a `MotionProperty`
(Spatial, Effects) and a resolved `MotionPreference` (Full, Reduced) to a `MotionSpec`: `Instant`
or `Timed(durationMillis, easing)`. Durations come from the canonical `motion.*.duration` roles
and effects use the canonical `motion.effects.easing`; reduced motion is `Instant` for every
combination. There is no spring variant: the pinned Material3 1.8.2 has no `MotionScheme`, and the
canonical tokens define none, so spatial changes use the named `SpatialStandardEasing`
(Compose's `FastOutSlowIn`) as a stable fallback. A host injects the preference through
`UiPreferences` (`reducedMotion`, projected as `UiPreferences.motion`); components call
`rememberMotionSpec`, never the OS. No Android or iOS adapter reads the OS setting yet.

Consumers: the Auth action press (`AuthActionShape`, spatial feedback) and the Theatre launch
entrance (opacity as effects, travel/scale/rotation as spatial). Tests, all JVM Desktop Compose:

- `MotionPolicyTest` (common): the bounded exhaustive mapping and token drift guards.
- `AuthShapeTimelineTest`: the real shape outline against an independent model of the declared
  duration and easing, on a manual clock, with press, release, reverse mid-flight, press again
  before settled, five fast taps, reduced motion, determinism and leaving composition mid-animation.
- `AuthButtonReplayTest`: real touch dispatch; asserts the intent before the clock advances a
  frame, the hit box on every sampled frame and writes labelled `cmp-motion-auth-action-*`
  captures with a `frame` sidecar (recipe, mark, time, timebase).
- `PendingRingAndFieldIndicatorTest`: the pending ring moves over a manual clock under full motion
  and stays stationary but painted under reduced motion; an `AuthTonal` field paints one inset
  perimeter (invalid 1 dp, focus 2 dp, focused-invalid 2 dp in the error colour) instead of
  stacking a ring on the error outline (#421, #478).
- `FocusPerimeterContractTest` / `FocusPerimeterPixelTest`: the pure stroke geometry, variant
  decision and ring-versus-fill contrast, and production pixels showing no focus pixel outside the
  bounds, the ring on the edge (including the pressed auth corner), no size change and the
  documented outset exceptions (#478).
- `MotionReplay.kt`: the animation recipe, driver and model helpers. Motion frames are owned by
  tests; production UI never advances the clock. The Workbench tab's intent replay does not
  control this animation clock or offer pause/seek-to-frame controls.

A manual-clock frame proves ordering, retargeting and final state. It is not a recording, frame
pacing, the OS reduce-motion signal, Android or iOS. See the
[evidence record](../../../docs/evidence/issue145-cmp-motion-policy.md) and
[motion.md](../../../.agents/skills/cantos-ui-design/references/motion.md).

## Toolchain decision

Pinned versions are Kotlin Multiplatform **2.2.20**, Compose compiler **2.2.20**, Compose
Multiplatform Gradle plugin **1.9.0**, and explicitly stable Material3 **1.8.2**. No experimental
M3 Expressive alpha widgets are adopted. Material3's minimum interactive-size composition local
is the one scoped experimental opt-in in `CantosTheme`; it follows the selected desktop density.
The isolated Android preview uses Android Gradle plugin 8.11.1; no Xcode target is introduced.

This is a conservative, reproducible desktop foundation rather than a latest-version upgrade.
The official [Compose 1.9.0 release](https://github.com/JetBrains/compose-multiplatform/releases/tag/v1.9.0)
requires Kotlin 2.1+ and states that `compose.material3` points to stable 1.8.2. The
[Kotlin compatibility matrix](https://kotlinlang.org/docs/multiplatform/multiplatform-compatibility-guide.html)
permits Gradle 7.6.3–8.14 for Kotlin 2.2.20; the intended Gradle distribution is **8.14.3** within
that release family. The [Compose compiler guide](https://kotlinlang.org/docs/multiplatform/compose-compiler.html)
requires the compiler plugin to use the same Kotlin version. The [desktop compatibility guide](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)
requires JDK 11+ for desktop runtime and JDK 17+ for native packaging; this preview emits JVM 17
bytecode and does not package an installer. The integrated acceptance used a full JDK 17.
The desktop project adds no Android plugin or SDK requirement; the isolated Android host below
pins its own tooling. No auto-downloading JDK or wrapper executable/JAR is added. A full local
JDK with `javac` is required for the desktop test lifecycle, even though the source is Kotlin.

Repository operations go through [`cargo xtask`](../../../xtask/README.md), including the
renderer-specific UI-system build/run/verification routes. Under that surface the Gradle
primitives are `compileKotlinDesktop`, `desktopTest`, and `run`; use the repository routes rather
than a second root-level script. `MainKt` opens only the local preview window.

## Android Studio host previews

[`android-preview`](android-preview/build.gradle.kts) is a real Android library diagnostic consumer,
not a Theatre/Studio app. It compiles the existing `commonMain` source directly, keeping the desktop
project independent of Android SDK setup. Kotlin/Compose/Material3 pins remain the same; AGP
**8.11.1**, Android tooling **1.9.0**, Gradle **8.14.3**, JDK 17 and compile SDK 35 define this host.
The diagnostic min SDK 23 does not select a production mobile support floor. See the official
[AGP compatibility](https://developer.android.com/build/releases/agp-8-11-0-release-notes) and
[Kotlin compatibility](https://kotlinlang.org/docs/multiplatform/multiplatform-compatibility-guide.html).

```sh
cargo xtask ui-preview cmp android-build
cargo xtask compose-preview ui-system-cmp \
  --file packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/preview/AndroidPreviews.kt \
  --composable AndroidFieldsLightPreview --label fields-light-narrow-vi
```

Open `android-preview` in Android Studio Quail 2+ and complete Gradle sync first. The IDE distribution
properties pin Gradle 8.14.3 with its published SHA256; repository builds use the installed pinned
Gradle through xtask. No wrapper executable/JAR, device, emulator or app install is provided.
Other specimens are `AndroidChoicesDarkLargePreview`, `AndroidNavigationLightWidePreview` and
`AndroidNestedThemeDarkLargePreview`. Each wraps actual shared components and registered fixtures
with explicit Vietnamese locale, width, theme and platform font scale. Open captured PNGs and
assert the observed semantics separately; neither capture nor compilation proves interaction,
IME, TalkBack, Android/iOS runtime or production qualification. Theatre's isolated mobile mock
and the catalog's current qualification records retain their separate scopes. The
[PR43 Android host record](../../../docs/evidence/cmp-pr43-android-preview.md) records the actual
build, four inspected PNGs, static semantics assertions and remaining native gates.

## Tests and evidence boundary

The three-session Mac sequence has attributed historical
[foundation](../../../docs/evidence/cmp-ui-system-session1.md),
[interaction](../../../docs/evidence/cmp-ui-system-session2.md) and
[final review/readiness](../../../docs/evidence/cmp-ui-system-session3.md) records. Its final
74 executed JVM tests and 26 opened captures establish bounded preview behavior for that source.
That registry snapshot had 29 APIs and 152 scenarios with 0/29 qualified; the current
[catalog](../shared/preview-catalog.json) owns registration and qualification freshness. Range requests are
finite/bounded and snapped to native ticks; unchanged physical ticks preserve caller decimal
aliases without callback/completion. Continuous directional steps fall back to the adjacent
Float if one percent cannot be represented. OS binding, real IME, screen reader, motion,
Noto/reference and mobile acceptance remain separate. Historical attempts below retain their
own source/toolchain scope.

`commonTest/PreviewModelTest.kt` checks exact 30-component inventory parity, search/family
filtering, validated names/reasons, blocked dispatch projections, read-only versus disabled,
generated token density/alias/motion lookups and nine official icon path mappings.
`desktopTest/ComponentSemanticsTest.kt` declares JVM Compose tests for callbacks, visible
blocking reasons, accessible icon names, checked state and Vietnamese controlled text.
The [official desktop test guide](https://kotlinlang.org/docs/multiplatform/compose-desktop-ui-testing.html)
documents the JUnit renderer dependency used here.

On the 2026-10-11 implementation host, Gradle was initially unavailable; the earlier
`gradle-9.7.0-bin` cache contained only zero-byte partial/lock files. The task subsequently
provisioned official Gradle **8.14.3**, verified its distribution SHA-256, and resolved the
pinned official Kotlin/Compose artifacts through the existing environment proxy with TLS
verification retained. The canonical `cargo xtask ui-preview cmp build` then **PASS**ed,
including actual compilation of the final common/JVM source with Kotlin 2.2.20, Compose 1.9.0
and stable Material3 1.8.2. This proves compilation, not runtime appearance or interaction.

A supplemental run of generated plain Kotlin, `PreviewModel.kt` and `PreviewModelTest.kt`
using the already-installed compiler **2.4.20** and JUnit **4.13.2** also passed **9 tests**.
The pinned `cargo xtask ui-preview cmp verify` was attempted. Initially the Java 21 runtime
lacked a `javac` executable, and Gradle's `compileDesktopTestJava` capability check blocked it.
An official, checksum-verified Temurin **17.0.20.1+1** JDK then satisfied that requirement and
built main classes/JAR. The supported verify retry **FAIL**ed while resolving test dependencies:
its default truststore rejected the environment HTTPS proxy certificate with `certificate_unknown`
and `PKIX path building failed`. The unresolved official artifacts were `kotlin-test:2.2.20`,
`kotlin-test-junit:2.2.20` and `ui-test-junit4:1.9.0`. Test-source compilation and execution were
**NOT_RUN**. No truststore, certificate, TLS verification or system security setting was changed.
These are recorded tooling blockers, not passing semantic evidence.

`PreviewCaptureTest.kt` declares four actual JVM Compose root captures: wide light/dark
components, narrow light 2× text and wide dark named tokens. Its PNGs go to the task-selected
`CANTOS_UI_PREVIEW_CAPTURE_DIR` or ignored `build/reports/cantos-preview`. A JVM test-root
capture is distinct from an OS desktop-window screenshot and from Android/iOS device evidence.
The desktop semantic/capture tests, window run, inspected pixels, keyboard walk, native font
coverage and motion checks remain **NOT_RUN** until the supported JDK can resolve dependencies
under its normal trusted configuration and the renderer executes them.
No passing Android/iOS or production runtime claim follows from this package.

The initial [Mac PR41 review](../../../docs/evidence/ui-system-previews.md#mac-reconciliation-and-review-2026-10-11)
preserves that historical cloud result separately from changed source. That initial suite declared
23 tests (10 common, 9 desktop semantics and 4 captures), including filtered selection, named
Select state and interruption behavior. Select callers provide localized expanded/collapsed
descriptions; losing availability clears its open menu. Final Mac compilation/tests/window
execution remain NOT_RUN because the installed Gradle9.7.0 is outside the reviewed8.14.3 gate.
No component is qualified for new app adoption by this partial evidence.

The [supported Mac continuation](../../../docs/evidence/ui-system-previews.md#supported-mac-continuation-2026-10-12)
now records actual Gradle 8.14.3/JDK 21 compilation, all 23 executed tests and inspected captures
at 1140×840 or 372×840 using an explicitly sized JVM renderer. The runtime command aligns
immutable JVM options, passed the unchanged PID guard and left no recorded owned PID after
two starts/interruptions. OS-window input is blocked by the UI tool's Java-app binding,
separate from passing JVM semantics/captures. Full preflight is PASS; two-renderer qualifications
remain 0/29 and mobile acceptance is NOT_RUN.

## Explicit platform differences

### Auth/account foundation v0.4.0 (issue #127)

The new roles are scoped variants over the existing controlled primitives, not an app auth
implementation or a replacement for ordinary listening/editor shapes:

- `ButtonVariant.AuthPrimary`, `AuthSecondary` and `AuthOutlined` use intrinsic capsule
  geometry with 60/56 dp minimums, paired filled/tonal colors and a fractional 1.5 dp functional
  outline only on `AuthOutlined`. Their stable-CMP pressed-shape fallback interpolates the
  measured capsule to the 16 dp role with the interruptible 220 ms token tween. Reduced motion
  makes the tween immediate; this is not a native M3 spring claim. An optional native
  `interactionSource` supports actual press specimens without adding a synthetic state prop.
  Both overloads accept a decorative registry `icon` name. The 23 October anatomy centers the
  icon and label together without a circle; loading uses the separate progress cue. The
  text/control supplies the sole accessible name.
- Guest CTA pair (design v4.2b, #517; superseded for Web by design v4.3, [#526](https://github.com/loveoverflowcom/cantos/issues/526), CMP parity not yet adopted): `Surface(PrimaryContainer, Profile)` alone scopes
  `AuthPrimary` and `AuthOutlined` to the card's own roles through an internal local, with no new
  public API. `AuthPrimary` fills with `color.onPrimaryContainer` and reads `color.primaryContainer`
  as text/icon; `AuthOutlined` is transparent with `color.onPrimaryContainer` text and a 1.5 dp
  outline. Both take the 60 dp minimum and the outline weight, so the stacked pair is one size.
  Any other or nested surface, including Membership, keeps the global roles; callbacks,
  eligibility and the disabled state are unchanged. JVM pixel and contrast tests cover it; Android,
  iOS and screen-reader behavior are not established.
- `TextFieldVariant.AuthTonal` is available on both raw-string and `TextFieldValue` overloads.
  The 56 dp minimum and 20 dp corner do not impose a fixed text height. Functional error/focus
  boundaries surround the native field only; help/error/blocking text remains outside their
  paint. `controlModifier` targets the native focus/edit node. Selection/IME composition stays
  host-controlled, and unavailable fields keep the supplied value.
- `SurfaceVariant.Auth`, `Profile`, `Membership` and `AccountGroup` select only geometry
  (36/36/32/28 dp). Tone is independent. `SurfaceTone.PrimaryContainer` inherits
  `color.onPrimaryContainer`, not muted text; callers must preserve that pair in child content.
  There is no decorative card border.
- `BackButton` requires a nonblank `AccessibleName` and a host `onBack` callback. Its 48 dp
  minimum is separate from regular controls. CMP defaults to an icon-only specimen; `iconOnly =
  false` exposes the same localized text and grows intrinsically. Loading/disabled reasons stay
  visible and block callbacks. The primitive does not own routes, history, logout, Android Back
  or iOS navigation.

`AuthAccountSamples.kt` is package-local anatomy only. Its display-name field is illustrative,
never an embedded native password or login form. The actual shared gallery declares auth,
profile/member/group, real pressed/focus, blocked/error and long Vietnamese/2× specimens.
`AuthAccountComponentTest` and `AuthAccountCaptureTest` exercise/capture those wrappers through
the JVM renderer when the supported verify command can execute; test declarations alone do not
prove execution or inspected pixels.

The Android host has `AndroidAuthAccountLightPreview`, `AndroidAuthAccountDarkLargePreview` and
`AndroidAuthStatesDarkPreview`. Each uses the real common components. Capture one only through
`cargo xtask compose-preview ui-system-cmp` with the existing `AndroidPreviews.kt` file; missing
SDK/CLI/ready Android Studio is `UNSUPPORTED`, not mobile evidence. The shared variants remain
unqualified for new app adoption until both renderer records meet the existing admission gate.

- `Link` invokes a caller URI callback. HTML anchor/target/download/history are web-only; preview
  callbacks do not launch a browser or navigate the production apps.
- `NumberField` keeps raw decimal text and supplies the native decimal-keyboard hint. HTML
  `min`/`max`/`step` constraint validation has no native equivalent here; callers validate numbers.
- Field labels are composed into control semantics. DOM `for`/ID associations do not exist.
- The dialog uses the stable native Material modal; DOM `showModal`/inertness and Android/iOS
  Back, safe areas, platform text input and real assistive technologies need separate evidence.
- Stable native semantics and shapes do not imply exact web pixels, font resources, mobile
  touch-target acceptance or M3 Expressive motion parity.

Before application adoption, run the pinned Compose build and desktop tests; inspect light/dark,
compact/comfortable, wide/narrow, Vietnamese 2× text; drive callbacks, disabled reasons,
read-only selection, menu close, dialog Escape/return-focus and keyboard order. Mobile callers
still need Android/iOS-specific evidence and app-owned native adapters.
