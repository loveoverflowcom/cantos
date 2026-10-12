# Cantos CMP UI-system preview

This package is a **development-only preview consumer** of the shared UI system. It now has
Kotlin Multiplatform `commonMain` component wrappers and a Compose Desktop JVM host. It is
neither Cantos Theatre nor Cantos Studio mobile, and implements no API, authentication,
navigation shell, downloads, audio session or playback adapter.

## Ownership and invariant

Every shared declaration in [`preview-catalog.json`](../shared/preview-catalog.json) must have
one real native example or an explicit unsupported explanation. The common package exposes
29 wrappers with the same component names as Leptos. Close/retry labels, accessible names and
visible content come from the caller. Caller-owned values and callbacks remain
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
`ComponentModifiers.kt`. Remaining navigation/choice/layout/feedback wrappers stay in
`Components.kt` for the next bounded session. No new module or runtime dependency is added.

String field calls remain compatible. `TextField`, `TextArea` and `NumberField` also accept
caller-owned `TextFieldValue`, retaining text/selection/composition without conversion. Return
received values promptly and do not normalize composing text. Read-only permits selection changes
but rejects text/composition edits; disabled/loading blocks all callbacks. The caller owns a
transition out of active IME composition before locking a field; real platform commit/cancellation
is still unverified. All overloads expose native `KeyboardOptions`/`KeyboardActions`; custom
actions are guarded by editability. TextArea exposes positive ordered min/max line limits.
NumberField keeps raw `0`, empty and incomplete localized decimals; keyboard hints are not numeric
validation. Blank help/error messages have no visible or semantic error state.

`Button(name, onPress, ..., content)` adds a RowScope content slot with a required localized
accessible name that includes its visible label. The existing label overload delegates to it.
For Button/IconButton/FilterChip, `modifier` still applies once to the whole field; new
`controlModifier` attaches focus, geometry and semantics to the actual native control. It does not
silently move existing root modifiers. Decorative Icon now inherits the surrounding content tint,
with an explicit tint override when needed. Focus consumes width/offset and the control's shape;
text fields use `shape.field`. Parents need spacing for an outset focus ring.

The [session 1 evidence](../../../docs/evidence/cmp-ui-system-session1.md) records 41 executed CMP
tests and inspected captures. Both previews declare nested-theme, composed-content, selection,
ime-action and blank-error examples: 29 APIs, 129 scenarios, unchanged 73 tokens. OS Java-window
input is still blocked, native/mobile/screen-reader/full reference evidence is incomplete and all
qualifications remain unapproved. Continue with [session 2](../../../docs/work-plan/016-cmp-ui-system.md).

## Preview contents

- Search and component-family filtering; one selected component and its registered state list
- Real button, icon-action, field, choice, selection, slider, tab and dialog callbacks
- Light/dark and compact/comfortable profiles, 1×/1.3×/2× text-scale probes, width presets
- Named token catalog with color swatches, raw values, aliases, density and reduced-motion overrides
- Official Apache-2.0 Material Outlined SVG paths mapped to decorative Compose vectors
- Vietnamese fixtures such as `Người dẫn chuyện`, `Ánh đèn cuối sân khấu`,
  `Nguyễn Thị Minh Phương`, `Ỗ, Ữ` and `Ngày mai, mình có diễn tiếp không?`
- An event log that changes only local preview state; no preview action sends a network request

Expanded windows use a master/detail layout; narrower windows use a component selector and a
scrollable detail area. Large-text/narrow modes make the page scroll rather than clipping tools.
Nested scroll panes receive bounded height. The navigation rail and desktop-window dimensions
are renderer-local geometry derived from named breakpoints, not new token definitions.

## One token source

[`shared/tokens.json`](../shared/tokens.json) is the sole semantic source. The repository generator
writes `src/commonMain/kotlin/com/cantos/ui/generated/CantosTokens.kt`; the icon and catalog
generators write adjacent files. Do not edit these files by hand or add a per-application palette.
`CantosTheme` maps those generated roles to one Material `ColorScheme`, typography and shapes.
`space.*` and wrapper control sizes map to dp; type sizes map to sp. Material primitives retain
native intrinsic metrics. Exact pixel parity with web intrinsic metrics is not claimed.

The source names Noto Sans and Noto Serif. The preview explicitly resolves these roles to the
platform sans/serif fallback; it does not redistribute embedded design-handoff fonts. Reviewed
font resources and Vietnamese glyph/rendering checks remain necessary before app adoption.

Reduced-motion inspection resolves the three canonical duration roles to zero. The pending
sample is deliberately static. This is not a platform reduced-motion adapter or a claim about
all Material ripple/native motion. Focus uses real Compose focus nodes with token-driven rings;
select the focus preset or use Tab/Shift+Tab. Dialog dismissal requests focus restoration to its
trigger. These behaviors still need the named renderer tests and observation below.

## Toolchain decision

Pinned versions are Kotlin Multiplatform **2.2.20**, Compose compiler **2.2.20**, Compose
Multiplatform Gradle plugin **1.9.0**, and explicitly stable Material3 **1.8.2**. No experimental
M3 Expressive alpha widgets are adopted. Material3's minimum interactive-size composition local
is the one scoped experimental opt-in in `CantosTheme`; it follows the selected desktop density.
No Android Gradle plugin or Xcode target is introduced.

This is a conservative, reproducible desktop foundation rather than a latest-version upgrade.
The official [Compose 1.9.0 release](https://github.com/JetBrains/compose-multiplatform/releases/tag/v1.9.0)
requires Kotlin 2.1+ and states that `compose.material3` points to stable 1.8.2. The
[Kotlin compatibility matrix](https://kotlinlang.org/docs/multiplatform/multiplatform-compatibility-guide.html)
permits Gradle 7.6.3–8.14 for Kotlin 2.2.20; the intended Gradle distribution is **8.14.3** within
that release family. The [Compose compiler guide](https://kotlinlang.org/docs/multiplatform/compose-compiler.html)
requires the compiler plugin to use the same Kotlin version. The [desktop compatibility guide](https://kotlinlang.org/docs/multiplatform/compose-compatibility-and-versioning.html)
requires JDK 11+ for desktop runtime and JDK 17+ for native packaging; this preview emits JVM 17
bytecode and does not package an installer. The available host Java is 21. No auto-downloading
JDK toolchain, unverified wrapper binary, formatter or external plugin is added. A full local
JDK with `javac` is required for the desktop test lifecycle, even though the source is Kotlin.

Repository operations go through [`cargo xtask`](../../../xtask/README.md), including the
renderer-specific UI-system build/run/verification routes. Under that surface the Gradle
primitives are `compileKotlinDesktop`, `desktopTest`, and `run`; use the repository routes rather
than a second root-level script. `MainKt` opens only the local preview window.

## Tests and evidence boundary

`commonTest/PreviewModelTest.kt` checks exact 29-component inventory parity, search/family
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
preserves that historical cloud result separately from changed source. The current suite declares
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
