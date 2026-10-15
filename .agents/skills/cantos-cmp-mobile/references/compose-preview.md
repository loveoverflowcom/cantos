# Compose preview without an emulator

Use Android Studio's layout renderer for a quick pixel and semantics pass over actual Compose
code. Theatre Mobile's isolated mock has a Gradle Android host and existing wrappers in
`preview-sandbox/src/androidMain`; its capture target still requires the official CLI and ready IDE.
Studio is web-only and has no mobile capture target; never scaffold one for preview evidence. The separate `ui-system-cmp` target selects a real Android library host in
`packages/ui-system/cmp/android-preview`, with thin wrappers around the shared components and
existing deterministic fixtures. The [PR43 host record](../../../../docs/evidence/cmp-pr43-android-preview.md)
records an actual Android build and four inspected captures with asserted static semantics.
This host produces no mobile APK and adds no device
or production qualification; current component admission is recorded in the shared preview catalog.

## Prerequisites and discovery

1. Inspect the selected project's actual Gradle files, Kotlin/CMP versions, Android target and
   preview annotation dependencies. Targets have fixed roots: `cantos-theatre` selects
   `apps/cantos-theatre/mobile` and `ui-system-cmp` selects `packages/ui-system/cmp/android-preview`. Never substitute a Web screen
   or the other product. The UI-system host directly consumes `../src/commonMain/kotlin`; the
   preview source passed to the capture command must be inside the isolated Android project.
2. Install the **official Android CLI** following [Google's overview](https://developer.android.com/tools/agents/android-cli).
   The legacy Android SDK `android` executable is a different tool. `cargo xtask doctor` lists
   the executable/version; the capture adapter additionally probes the preview command's help.
3. Open that Gradle project in **Android Studio Quail 2 or newer**, finish sync/build readiness,
   and inspect `android studio check` for its project path, IDE version and PID. The CLI connects
   to an already running IDE. It neither launches Studio nor provisions a headless CI service.
4. Select an existing `@Preview` function compiled for the Android target. For a new screen, add
   a thin preview wrapper around the **production composable**, its real theme and deterministic
   presentation state. Never copy the screen into a demo. Keep network, authentication, real
   playback, private drafts and providers out of the fixture.

The official [render command](https://developer.android.com/tools/agents/android-cli/commands/studio_render-compose-preview)
supports `--project`, `--pid`, `--output-image-file` and `--print-semantics`. The
[Studio check reference](https://developer.android.com/tools/agents/android-cli/commands/studio_check)
documents instance and project discovery. Recheck these sources when CLI/IDE versions change.

## Capture through xtask

Compile the existing isolated Android library through the repository command, then open that
project in Android Studio and complete Gradle sync before capture. The compilation command uses
the pinned Gradle 8.14.3/JDK 17 checks and requires Android SDK 35; it does not start an IDE,
emulator or device. It remains separate from the JVM desktop build/test gate.

```sh
cargo xtask ui-preview cmp android-build
cargo xtask compose-preview ui-system-cmp \
  --file packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/preview/AndroidPreviews.kt \
  --composable AndroidFieldsLightPreview \
  --label fields-light-narrow-vi
```

Existing specimens declare their display inputs in `@Preview`:

| Function | Static scenario |
| --- | --- |
| `AndroidFieldsLightPreview` | 372 × 840 dp, light, Vietnamese, font scale 1× |
| `AndroidChoicesDarkLargePreview` | 372 × 1000 dp, dark, Vietnamese, font scale 2× |
| `AndroidNavigationLightWidePreview` | 840 × 840 dp, light, Vietnamese, font scale 1× |
| `AndroidNestedThemeDarkLargePreview` | 840 × 840 dp, dark, Vietnamese, font scale 2× |
| `AndroidDesignInventoryTypicalLightPreview` | 390 × 1800 dp, light, Vietnamese, 1×; the gallery's Design v3 tab on the `mini-player` proposal |
| `AndroidDesignInventorySmallDarkLargePreview` | 320 × 5200 dp, dark, Vietnamese, 2×; the Web-only `studio-workspace` proposal |
| `AndroidDesignInventoryExpandedPreview` | 840 × 1000 dp, light, Vietnamese, 1×; the registered `button` specimen in the two-pane layout |

The three Design v3 wrappers are composition roots: each reads `isSystemInDarkTheme()` once and
injects it with `ThemeMode.System`, so the preview follows the annotation's `uiMode` without any
component reading the OS. They show a read-only map of the design export; proposals have no sample.

The Theatre mock has real wrappers around the same shared app. Compile with
`cargo xtask theatre-demo android-build`, open its Gradle root and sync with
`-Pcantos.android=true` before capture:

```sh
cargo xtask compose-preview cantos-theatre \
  --file apps/cantos-theatre/mobile/preview-sandbox/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt \
  --composable TheatrePlayerDarkPreview \
  --label player-dark-390-vi
```

Quote the real path if it contains spaces. Paths resolve from the repository root even when
xtask is called from a subdirectory. The adapter confines the file to the selected fixed project
and passes that project's absolute directory as `--project`; an instance PID can be selected with
`--pid 123`. `--android-cli /path/to/android` selects an official CLI outside `PATH`.
`--timeout-seconds 60` bounds **each** CLI invocation (allowed range 1–300 seconds).

`--dry-run` validates the project/source and prints the argument vector without contacting the
CLI/IDE or writing evidence. It prints `NOT_RUN`, and proves no preview or semantics claim.
The adapter never runs Gradle, starts an emulator, installs an app or invokes a device command.

Use the existing preview annotations/state to set size, theme, locale, font scale and loading,
empty, error, disabled or long-text variants. `--label` only identifies the scenario; it does
**not** configure the display. Prefer realistic Vietnamese strings such as `Người dẫn chuyện`
and `Ánh đèn cuối sân khấu`. Keep the same fixture and preview configuration for before/after
comparisons, with light/dark and large-text variants when affected.

## Inspect the evidence

Each invocation uses a fresh directory under ignored
`artifacts/ui-evidence/compose-preview/<target>/<run-id>/`; no previous image can satisfy a new run.

| Artifact | Use |
| --- | --- |
| `preview.png` | Actual CLI image; the tool checks the PNG header/dimensions, not visual quality |
| `semantics.txt` or `semantics.json` | The observed text tree or legacy JSON object/array preserved without inventing a schema |
| `render.stdout.log`, `render.stderr.log` | Raw renderer output, retained even on failure/timeout |
| `version.*.log`, `help.*.log`, `studio-check.*.log` | CLI version, supported options and IDE/project discovery |
| `capture.json` | Schema v2: source/hash, Git HEAD/dirty state, command, label, output hashes, semantics format/path, renderer and result |

Open every cited PNG using an image-capable tool and record visible observations via the shared
[visual review](../../cantos-ui-design/references/visual-review.md). Inspect the semantics file named
in `capture.json` for the production screen's expected localized text, names, state and actions
that the observed format actually exposes. The current CLI text tree contains class names,
bounds and text; it does not expose a complete accessibility action/state matrix. Make assertions
against the **observed format**, and state which renderer produced it. A dumped tree alone is not
`semantics-tested`. The generated `image_inspected` and `semantics_asserted` fields remain false:
write the inspection and assertions in a separate report instead of altering capture provenance.

The adapter accepts one JSON object/array or one bounded, indented `ComposeViewAdapter` tree
followed by the matching `Wrote rendering to` path. It rejects missing, malformed or ambiguous
semantics, missing/invalid PNG headers and a preview source
changed during capture. Capture execution requires a POSIX host for cleanup of the CLI process
group it owns, including descendants, on return, failure, timeout or interruption; it never
signals Android Studio. Each command stream is capped at 4 MiB, the source at 4 MiB and the PNG
at 32 MiB. Special files, linked outputs and symlinked project roots are rejected. Capture
bundles and adapter-written files are private, and existing artifact paths are never overwritten.
Keep failed bundles for diagnosis. `capture: PASS` means the command
captured files successfully; unopened images remain `screenshot-captured`. Only an actual visual
inspection earns `screenshot-inspected` and actual assertions earn `semantics-tested`.

| Claim | Evidence still needed |
| --- | --- |
| interaction, navigation, focus or animation | Production Compose UI tests or a driven runtime flow |
| Android/iOS rendering, insets, input, OS audio/lifecycle | Named device/emulator/simulator checks |
| TalkBack/VoiceOver usability | Named screen-reader walkthrough |
| app builds or passes mobile tests | Actual configured Gradle/Xcode builds and test reports |

This is Android **host-preview** evidence for shared CMP UI, never iOS rendering or
`device-tested`. IDE readiness output is retained for inspection; the wrapper does not infer a
fully synchronized app build from the CLI exit status.

## Missing prerequisites and the JVM alternative

A missing Gradle project, official CLI/capability or IDE connection produces `UNSUPPORTED` and
a failing xtask exit. Render/output errors produce `FAIL`; invalid arguments exit 2. Report the
blocker and the checks that actually ran. Do not create a stub app, change an expected result or
silently launch an emulator to obtain a green capture.

For automated interaction/semantics and optional screenshots without an IDE, use **Robolectric
with Compose UI Test on the JVM** only when the real Gradle build configures it. Discover its
source sets, test runner, SDK, graphics mode and tasks; pinned AGP versions may expose
`testDebugUnitTest` or `testAndroidHostTest`, with module-specific names. Do not copy VOTHub's
`:sharedUi:testAndroidHostTest` into Cantos: no such module or task exists here. Add an actual
native test gate to xtask only when its runner is implemented. The current
`cargo xtask theatre-demo verify` runs common/desktop Compose tests, as
[CMP testing](cmp-testing.md) records; Robolectric dependencies/runners remain candidates.
Label a host run with its actual renderer and selected/executed test counts; it does
not close the [native acceptance matrix](../../../../docs/architecture/mobile.md#future-acceptance-matrix).

## Pair an existing capture with a design PNG

Use the [shared pairing/comparison method](../../cantos-ui-design/references/visual-review.md#10-reference-pairing-and-local-comparison)
and `cargo xtask visual-compare`; this does not rerun capture or require a new device harness.
Theatre sandbox `DemoCaptureTest` and shared `AuthAccountCaptureTest` already export JVM PNGs and
metadata/semantics. The shared `PreviewCaptureTest` also writes a `<name>.json` sidecar
(`cantos-ui-capture-v1`) beside each PNG: scenario, section/specimen, theme mode and resolved
theme, UI density, preview text scale, the dp viewport, bitmap pixels, host density and font
scale, token version, the digest of `tokens.json` bytes, `CANTOS_SOURCE_REVISION` (or
`unrecorded`) and the PNG's own SHA-256. Its renderer is always `jvm-desktop-compose-test-root`
and it names what it is not evidence of (devices, an OS window, a screen reader, design-pixel
match). Preserve their observed width/height, density, app/base font scales, theme,
locale, fixture/state and source/build SHA; supplement missing capture timestamps/font versions
without inventing them. The official preview `capture.json` is a different host's record.

Record physical pixels and logical dp/pt separately. Pair an approved reference for that target;
never resize Web/JVM images to simulate native parity. Regions map to a testTag/semantics bound
only when the inspected tree supplies it. Open overlay/crops and assert concrete semantics
separately. JVM, Android Studio layout rendering, Android devices and iOS devices retain distinct
acceptance; a comparator result cannot promote evidence between them.
