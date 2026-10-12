# PR43 Android Studio host preview acceptance

On 2026-10-12 the requester changed the runtime selection to Android Studio Compose Preview.
This follow-up to the [desktop acceptance](cmp-pr43-acceptance.md) reviews the actual shared
components through Google's layout renderer. No emulator or connected-device run was performed.
The bounded result is Android host compilation, screenshot inspection and static semantics
assertions. Product mobile apps remain guides and component production qualification stays **0/29**.

## Implementation and freshness

The isolated `packages/ui-system/cmp/android-preview` is a real Android library consumer of
`../src/commonMain/kotlin`. Thin `@Preview` functions call existing `UiRoot`, `ComponentExample`
and registered deterministic Vietnamese fixtures; no screen or component is copied. It produces
no APK and does not add an Android SDK requirement to the desktop project. The diagnostic min
SDK 23 does not choose a product support floor. Repository commands remain:

```sh
cargo xtask ui-preview cmp android-build
cargo xtask compose-preview ui-system-cmp \
  --file packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/preview/AndroidPreviews.kt \
  --composable AndroidFieldsLightPreview --label fields-light-narrow-vi --pid 12787
```

Captures were made from HEAD `d425dad553ecfa11a5e9ece7d5795f416a81811f` with the Android follow-up
changes uncommitted (`dirty: true`). Metadata retains that actual provenance. The wrapper source
SHA256 is `77e25c4ffc08d3af55f5d32706639a0a23cd7ca3fec4ea97538d33d8ce23e5dd`.
The source/token digests after adding the host and excluding only its local outputs are:

| Input | SHA256 |
| --- | --- |
| Canonical tokens | `e9713d5ad276be53bf3a63b6ce72ad04d1dbf0b5918cbb759a39ea7c2c1f39a2` |
| CMP source | `50911d0141d03b464508ac22a0c13a4d4d92e5a63de7b6e445654cf0d6229c3f` |
| Leptos source | `393b0e9b5aeae2a6ea1bd2852f5dbd6981f07d2182390e98cd297bacc442fe18` |

Captured bundles are ignored local artifacts under
`artifacts/ui-evidence/compose-preview/ui-system-cmp/<run-id>/`. Source and output hashes were
asserted again after capture; no metadata flag or historical failed bundle was rewritten.

## Toolchain and actual setup

Kotlin/compiler **2.2.20**, Compose **1.9.0** and stable Material3 **1.8.2** retain the existing
pins. The host adds AGP **8.11.1**, Android UI tooling **1.9.0**, compile SDK **35**, Gradle
**8.14.3** and Microsoft full JDK **17.0.19+10-LTS**. IDE distribution properties pin Gradle and
the official SHA256; no wrapper binary/JAR is committed. Android CLI **1.0.16500706** connected
to Android Studio **Rabbit 1 | 2026.2.1**, PID **12787**, after successful project sync.
The existing SDK was used. No TLS or OS security preference was weakened.

Initial sync used an incompatible IDE default Gradle 9.7; the first capture failed to find the
module (`20261012T053851Z-f2f3d94fd6bd`). The distribution pin corrected that choice. Gradle 8.14.3
also needed JDK 17 instead of the IDE's bundled JDK 25; an ignored project-local setting selected
the existing full JDK. Only IDE instances started for this task were restarted. The IDE remains
open on the diagnostic project. No existing emulator or phone was operated.

## Confirmed issues and fixes

1. **Text semantics rejected after a successful real render.** The original adapter assumed
   `--print-semantics` produced JSON. The actual CLI emits an indented `ComposeViewAdapter` tree,
   a blank line, then `Wrote rendering to <PNG path>`. The retained failed bundle
   `20261012T062504Z-39550877225f` contains the successful PNG and raw output; baseline parser
   execution against that output failed. The adapter now accepts one validated observed tree or
   the existing JSON form, preserves the UTF-8 tree bytes as `semantics.txt`, and records format,
   path and hash in metadata schema v2. Tests reject malformed, ambiguous/mixed output and a
   mismatching trailer path. It invents no accessibility AST. Adapter suite: **25 tests PASS**.
2. **Nested host outputs changed the CMP source digest.** The new project generated local build,
   Gradle, IDE and Kotlin files below the renderer root. Exact exclusions now cover only
   `android-preview/{build,.gradle,.idea,.kotlin}` and its `local.properties`. Gradle configuration,
   wrapper properties, manifest and Kotlin sources stay fingerprinted; real source directories
   with those names elsewhere still affect freshness. Failing-first cache/source tests now pass;
   catalogue suite: **31 tests PASS**. This closes the introduced freshness issue without hiding
   tracked inputs or permitting stale app adoption.

These supplement the desktop record's Select equality regression and fix. Shared common component
code is unchanged by the Android host follow-up. Independent source review and PNG review cover
the final implementation; native production qualification remains a separate gate.

## Captures and observations

All four fresh PNGs were opened, then their observed text trees were asserted. The preview
annotations set Vietnamese locale, dimensions, theme and platform font scale. `UiPreferences`
keeps its multiplier at 1 to avoid applying Android font scale twice.

| Run ID | Function / configured scenario | Actual pixels | Result |
| --- | --- | --- | --- |
| `20261012T063430Z-e3675d34cab1` | `AndroidFieldsLightPreview`; 372×840 dp, light, 1× | 977×2205 | PASS; three EditText nodes, raw `1,25`, selection diagnostic, Vietnamese dialogue/helper/error text |
| `20261012T063439Z-96c00a87a1c0` | `AndroidChoicesDarkLargePreview`; 372×1000 dp, dark, 2× | 977×2625 | PASS; CheckBox, Button and SeekBar nodes, complete long label/reason, narrator stable value and `50,0` |
| `20261012T063446Z-b434dc318383` | `AndroidNavigationLightWidePreview`; 840×840 dp, light, 1× | 2205×2205 | PASS; all four long tab labels, eight Button nodes, five action variants/add-scene/recovery and callback count zero |
| `20261012T063453Z-f3eb534bcd17` | `AndroidNestedThemeDarkLargePreview`; 840×840 dp, dark, 2× | 2205×2205 | PASS; nested-theme Vietnamese text, save Button and long reading paragraph |

Fields show diacritics, raw numeric text, focus rings and helper/error text without observed
clipping. The narrow choices labels and reason reflow at 2×. Navigation wraps its fourth long
tab to another row and retains readable action/recovery labels. Nested dark text, serif reading
and focus styling remain visible at 2×. No actionable overlap or clipping was observed in these
four static scenarios. This is visual inspection, not a measured contrast or font-metrics audit.

| Run (short) | PNG SHA256 | Semantics SHA256 |
| --- | --- | --- |
| `063430Z` | `5eeec86a6a4b898f6445800bcbd4910121fe93dabeb8c99f1cb0f20d0ed680f1` | `5642d740f2367738798c86b4f29f91ebd572cb595e5aadad63c9070aa21769ab` |
| `063439Z` | `862d4a61ed6f512f468be7196907402f59e331489f095bba6312130d9ba75689` | `a7b37cf5e41e9bc595cc4330673db02a05dcaba8a3a0b5a9886337b1d74744fe` |
| `063446Z` | `f7cb7869a238aa4597ac6ad45c18ebd94137766c2f35f8c17b235e18cecb4105` | `57a3aafb66468901a737b32446e76bf27b955d805f8e9a84fa64530fc8e5fcdc` |
| `063453Z` | `eccd2cb697f1e4ea0f57429e8e4dcaed0c56e3de9a2c3d3ea4179499c26fc2c3` | `ebc239be8c01e84aa41103de43bcb0d0ab16bb06fed1fc60408dd5f21e1b389b` |

Separate assertions verify capture status/schema, exact source/PNG/semantics SHA256, root bounds
against PNG dimensions, the node counts above, and 27 localized/value strings. The CLI text
format exposes class names, bounds and text. It does not expose the complete accessibility
action/state matrix, so these are bounded **static semantics-tested** results. Capture provenance
flags `image_inspected` and `semantics_asserted` stay false; this report records the later review.

## Verification and remaining gates

| Command / check | Actual result |
| --- | --- |
| `cargo xtask ui-preview cmp android-build` | PASS `compileDebugKotlinAndroid`; real shared source compiled for Android |
| Four `cargo xtask compose-preview ui-system-cmp` invocations | PASS; fresh real PNG and text tree per scenario |
| Open four PNGs; assert hashes/source/bounds/text/node classes | PASS; screenshot-inspected and bounded static semantics-tested |
| Adapter and catalogue regression suites | PASS 25 and 31 tests respectively, including failing-first cases |
| `cargo xtask verify repository` | PASS; 214 Python tests on the PR54-integrated tree, links/JSON/skills and diff checks |
| `cargo xtask preflight --changed-only --base 7478236c1a23b045dc93fa690b65a9edc9215fb5` | PASS; all 13 selected gates, no skips; `android-preflight-integrated-pr54.log` |

The complete preflight passed repository, rust-format, script-ir-reference, rust-lint
(workspace), rust-tests (workspace), wav-reference, web-lint (Studio/Theatre), web-build (Studio),
web-build (Theatre), ui-preview, cmp-ui-preview, mutation and postgres. Mutation executed a
15-test baseline and killed all nine mutations, with zero survivors/timeouts/unviable mutations.
PostgreSQL used a newly initialized disposable cluster and stopped it after the integration and
restart checks. It never inherited a production database URL.

The first full preflight passed 12 gates but PostgreSQL failed with SQLSTATE 53100, `No space left
on device`; that failed run remains in `android-preflight.log`. Only this worktree's generated
incremental caches and its already-stopped failed disposable database files were removed; all
source, test logs and images were retained. The full 13-gate retry then passed. No assertion or
expected result was weakened to address the host resource failure.

The prior desktop record supplies the fresh 75-test CMP execution, 26 opened JVM PNGs and bounded
Chrome input/dialog observations for unchanged common UI code. The retry's desktop task was
UP-TO-DATE, not another 75-test execution. CMP source/configuration inputs stayed unchanged
between the captures and both passing preflight runs. The latest-base integration below changed
backend inputs. Final documentation hygiene is checked again through the repository gate before push. Remote CI is NOT_RUN under the local handoff policy.

## Latest develop integration

Before push, direct remote ref discovery found that develop had advanced to
`7478236c1a23b045dc93fa690b65a9edc9215fb5`: audio-quality documentation `260f203` and PR54 speech
clip reuse. Merge commit `ac090f7fd124bd1ae28f5d777acbdda34e44c34d` has parents Android handoff
`81df467ed400e01833a60fa884f555a96cbe2a4b` and that develop SHA. The merge was clean; independent
review confirmed both work-queue updates survived and all CMP/Android/parser/catalogue inputs
were unchanged. The final PR delta remains the bounded preview work.

The new base adds backend contracts, tests and migration 0008, so the full preflight was repeated
against that pinned base on the integrated tree: **13/13 PASS**, no skips. Repository discovery
now executes **214** Python tests, including ten new upstream oracle tests. Workspace tests cover
the new speech fingerprint, clip-reuse and regeneration contract targets; PostgreSQL exercises
the new migration and backend tests in a fresh disposable cluster. Its shutdown was observed.

Canonical tokens and CMP fingerprints remain the capture values above. The **integrated Leptos
digest** is `72cb9022a63a2a55a87f9368b9435156cc88e2f7d177be3cbed9dadc6a8e8338`, because its
fingerprint includes the changed `Cargo.lock`. The earlier Leptos digest and browser observations
remain historical evidence for their recorded inputs. No Android recapture was needed: wrapper,
shared Kotlin, host configuration and all four PNG/semantics hashes were asserted unchanged.
Only results documentation changes after this full run; the repository gate checks that final
documentation again before publication.

Android/iOS product builds remain **UNSUPPORTED**. Device interaction, active Vietnamese IME,
TalkBack/VoiceOver, native audio/lifecycle, motion/reduced-motion, Noto metrics and the complete
reference/state/token matrix remain **NOT_RUN**. Static layout rendering does not close these
gates or grant component adoption. The next work remains named runtime/accessibility/reference
qualification before any production use.
