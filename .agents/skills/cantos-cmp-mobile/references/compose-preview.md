# Compose preview without an emulator

Use Android Studio's layout renderer for a quick pixel and semantics pass over a production
Compose screen. The repository command is implemented; **Cantos has no mobile Gradle project or
preview composables yet**, so a real Cantos capture is currently `UNSUPPORTED`.

## Prerequisites and discovery

1. Inspect the selected app's actual Gradle files, Kotlin/CMP versions, Android target and preview
   annotation dependencies. Use `apps/cantos/mobile` for Theatre or
   `apps/cantos-studio/mobile` for Studio; never substitute a Web screen or the other product.
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

The example below is a **future invocation**: replace the file and function with ones discovered
in the app. Neither `PlayerPreview.kt` nor `PlayerDarkVietnameseLargePreview` exists today.

```sh
cargo xtask compose-preview cantos \
  --file apps/cantos/mobile/<source-set>/PlayerPreview.kt \
  --composable PlayerDarkVietnameseLargePreview \
  --label dark-vi-VN-large-text
```

Quote the real path if it contains spaces. Paths resolve from the repository root even when
xtask is called from a subdirectory. The adapter confines the file to the selected mobile app
and passes that app's absolute directory as `--project`; an instance PID can be selected with
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
`artifacts/ui-evidence/compose-preview/<app>/<run-id>/`; no previous image can satisfy a new run.

| Artifact | Use |
| --- | --- |
| `preview.png` | Actual CLI image; the tool checks the PNG header/dimensions, not visual quality |
| `semantics.json` | The CLI's object/array preserved without imposing an invented tree schema |
| `render.stdout.log`, `render.stderr.log` | Raw renderer output, retained even on failure/timeout |
| `version.*.log`, `help.*.log`, `studio-check.*.log` | CLI version, supported options and IDE/project discovery |
| `capture.json` | Source path/hash, Git HEAD/dirty state, command, scenario label, output hashes, renderer and result |

Open every cited PNG using an image-capable tool and record visible observations via the shared
[visual review](../../cantos-ui-design/references/visual-review.md). Inspect `semantics.json` for
the production screen's expected localized text, names, state and actions; make assertions against
the **observed schema**, and state which renderer produced it. A dumped tree alone is not
`semantics-tested`. The generated `image_inspected` and `semantics_asserted` fields remain false:
write the inspection and assertions in a separate report instead of altering capture provenance.

The adapter rejects ambiguous/missing JSON, missing/invalid PNG headers and a preview source
changed during capture. Keep failed bundles for diagnosis. `capture: PASS` means the command
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
mobile test gate to xtask when that project lands. Today all Gradle tasks in
[CMP testing](cmp-testing.md) are discovery examples, and Robolectric dependencies/runners remain
candidates. Label a host run with its actual renderer and selected/executed test counts; it does
not close the [native acceptance matrix](../../../../docs/architecture/mobile.md#future-acceptance-matrix).
