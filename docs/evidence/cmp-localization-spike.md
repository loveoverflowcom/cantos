# CMP localization candidate spike — issue #169

**Status: bounded comparison completed on the JVM; architecture/adoption remains OPEN in
[#167](https://github.com/loveoverflowcom/cantos/issues/167).** This record does not accept the
whole of [#169](https://github.com/loveoverflowcom/cantos/issues/169). In particular, candidate
Android/iOS linking/runtime, persisted locale precedence, native accessibility and full catalog
migration remain unaccepted. The production/mock boundary remains the
[mobile guide](../../apps/cantos-theatre/mobile/README.md)'s boundary.

## Claim, ownership and scope

Invariant: UI-copy resolution must preserve literal content parameters and return the expected
Vietnamese/English copy for the chosen language; evaluating a candidate must not silently adopt
it or change the application's pinned toolchain.

Owner/boundary: localization is application presentation. Resource adapters and Compose are
outside playback/domain state; Script IR, story titles and dialogue are never translated by
this experiment. Shared UI-system primitives continue to receive caller-owned labels. The
renderer-neutral catalog and library decision belong to #167. The three retained
[fixture projects](fixtures/cmp-localization-spike/README.md) are experiment input only and are
not included in any application Gradle settings or shipping graph.

The pre-change baseline is commit `2e3f29a120d2c5ab5962b8c4bf5a2dc8c92c30cb` on 2026-10-16.
Theatre pins Kotlin
**2.2.20**, Compose **1.9.0**, Material3 **1.8.2**, Gradle **8.14.3**, AGP **8.11.1**, Java **17**.
`ShellCopy`, `AuthCopy` and `MembershipCopy` are paired typed catalogs; launch/unavailable XML
resources are a separate resolver. No source-format decision can be inferred from their
coexistence.

## Reproducible experiment and observed outputs

The [retained source](fixtures/cmp-localization-spike/README.md) contains all Gradle descriptors,
XML/Kotlin catalogs and exact expected-output tests. There is no wrapper or repository command
added. The one-off projects were copied under ignored
`artifacts/agent-runs/issue169-candidates/{compose,moko,lyricist}`. Each has one actual JUnit
renderer test, `CandidateTest.renders_both_languages_with_literal_content_and_exact_plural`.
It exercises `common.cancel`, bookmark-add(title), episode quantities 0/1/2, and an exhaustive
typed unavailable error. The title is `Ánh đèn {time} %1$s cuối sân khấu`.

| Row / semantic key | Vietnamese expected result | English expected result |
| --- | --- | --- |
| `common.cancel` | `Hủy` | `Cancel` |
| `theatre.player.bookmark.add(title)` | `Thêm dấu trang cho Ánh đèn {time} %1$s cuối sân khấu` | `Add bookmark for Ánh đèn {time} %1$s cuối sân khấu` |
| Episodes, 0 | `0 tập` | `0 episodes` |
| Episodes, 1 | `1 tập` | `1 episode` |
| Episodes, 2 | `2 tập` | `2 episodes` |
| Typed `Unavailable` | `Không thể phát nội dung này.` | `This content cannot be played.` |

The actual invocation used the already installed executable and JDK below, once per candidate
in sequence (replace `<candidate>` with `compose`, `moko`, or `lyricist`):

```sh
JAVA_HOME=/Users/manh.pd1/Library/Java/JavaVirtualMachines/ms-17.0.19/Contents/Home \
/Users/manh.pd1/.gradle/wrapper/dists/gradle-8.14.3-bin/cv11ve7ro1n3o1j4so8xd9n66/gradle-8.14.3/bin/gradle \
  -p artifacts/agent-runs/issue169-candidates/<candidate> desktopTest --console=plain
```

| Candidate / exact experiment version | Actual final command result | JVM assertions/captures | Android | iOS |
| --- | --- | --- | --- | --- |
| JetBrains Compose Resources 1.9.0 | `BUILD SUCCESSFUL in 7s` | 1 test, 12 exact rows, 2 PNGs + semantics | NOT_RUN | NOT_RUN |
| moko-resources + generator 0.27.1 | `BUILD SUCCESSFUL in 8s` | 1 test, 12 exact rows, 2 PNGs + semantics | NOT_RUN | NOT_RUN |
| Lyricist 1.8.0, manual catalog API | `BUILD SUCCESSFUL in 7s` | 1 test, 12 exact rows, 2 PNGs + semantics | NOT_RUN | NOT_RUN |

The first pass already compiled and passed all three tests. Pixel inspection then found a
transparent root, making the PNGs unreadable against a black viewer background. The fixture now
uses an explicit Material `Surface`; the table above records only the corrected final inputs.
All three final JUnit XML files record `tests="1" skipped="0" failures="0" errors="0"`.
The six final PNGs were opened and **screenshot-inspected**: readable Vietnamese diacritics,
literal `{time}`/`%1$s`, visible six rows and no clipped text in the captured intrinsic Surface.
This is an 800×360 JVM test frame and a cropped Surface capture, not a full-app viewport matrix.
An earlier moko fixture compile failed because generated extension-property imports were
missing; adding those imports corrected the fixture. These were experiment defects, not
reported library incompatibilities. Full logs remain ignored beside the experiment.

## Measured footprint and setup

Measurements enumerate UTF-8 `.kt` output bytes under each copied project's `build/generated`;
they are source-generation inventory, **not** APK/AAB/framework size, memory, startup, or build
performance. The experiments have tiny catalogs and no native target. All use the same skeleton,
which includes Compose Resources; its empty 3-file/3,501-byte scaffolding in non-Compose candidates
is reported separately rather than credited to those candidates.

| Candidate | Localization-generated Kotlin | Incidental empty Compose resource scaffolding | Candidate-specific setup above common Compose/KMP skeleton |
| --- | --- | --- | --- |
| Compose Resources | 5 files / 6,324 bytes | Included in its resource output | Existing Compose plugin + one resource dependency; resource package declaration; two XML qualifier directories |
| moko-resources | 7 files / 5,187 bytes | 3 files / 3,501 bytes | One generator plugin; two runtime/Compose dependencies; `resourcesPackage`; `base` and `en` XML trees with separate plurals files |
| Lyricist manual catalog | 0 generated catalog files / 0 bytes | 3 files / 3,501 bytes | One runtime dependency; typed `Copy`, two Kotlin values, locale map and CompositionLocal provider; no KSP |

Generator size can be reproduced by listing generated `.kt` files and summing their byte lengths;
for moko count `build/generated/moko-resources` separately from `build/generated/compose`.
Lyricist's KSP mode was **NOT_RUN**: its setup/generated footprint is not represented by the
manual mode's zero. No translator participated, so translator workflow assessments below are
source/tool-format observations, not a measured usability study.

## Capability and decision matrix

Primary sources were checked on 2026-10-16. Current documentation can describe newer APIs than
the application's pins; successful JVM execution below is the only executed compatibility
claim. An upstream target declaration is not a Native link or device result.

| Concern | Compose Resources 1.9.0 | moko-resources 0.27.1 | Lyricist 1.8.0/manual mode |
| --- | --- | --- | --- |
| Application fit | Already used for launch/assets; no new runtime needed for those resources | Adds another resource generator/runtime alongside existing Compose assets; duplication needs a migration/rollback decision | Typed Kotlin catalog resembles existing copy, but adds its own locale/state runtime; app-owned state must remain authoritative |
| Typed/named parameters | Generated resource identity; `vararg Any` formatting is not a typed named-parameter contract | Generated identity and `StringDesc`; formatting also uses `vararg Any` | Kotlin function properties carry parameter names/types; the spike uses `(title: String)` and `(count: Int)` |
| Non-Composable use | `suspend getString` / `getPluralString` | `StringDesc` resolved through platform APIs (`localized`, Android context) | `lyricist.strings` / `StateFlow` usable outside Compose |
| Literal interpolation | Both title brace syntax and `%1$s` retained by actual rendered assertions | Same actual rendered assertions | Kotlin insertion retains the same literal title; actual rendered assertions |
| Plurals | Resource quantities supported; this JVM fixture's vi/en 0/1/2 pass | Resource plural quantities and platform selection; this JVM fixture passes | Developer-written function; this fixture's vi/en integers pass, **no automatic CLDR proof** |
| Runtime locale | Pinned resource environment is platform-selected; its environment constructor/provider are internal | Public `StringDesc.localeType = Custom(tag)` is process-global; caller must also cause recomposition; native behavior unrun | Public language tag updates its StateFlow; persistence is caller-owned; the spike mounts each language separately |
| Preview | Actual JVM composable semantics/capture; Android preview requires assets, AGP/IDE | Actual JVM composable semantics/capture; resource generation and native packaging add setup | Actual JVM composable semantics/capture; manual catalog needs no resource bundle |
| Translator source | Familiar XML and qualifier folders; named semantic/parameter schema still needed | Familiar XML with distinct source layout and iOS bundle copy work | Kotlin values/lambdas require code-aware editing; XML processor and its plural constraints need separate evaluation |
| Licence / release observation | Apache-2.0; repository already pins 1.9.0 | Apache-2.0; 0.27.1 release dated 2026-09-18 | MIT; tested 1.8.0 release dated 2025-12-05; newer 1.9.0 dated 2026-07-21 |

Supporting primary references:

- [JetBrains resource usage](https://kotlinlang.org/docs/multiplatform/compose-multiplatform-resources-usage.html)
  documents typed resource IDs, untyped positional format arguments, suspend access and plurals.
  [Pinned v1.9.0 environment source](https://github.com/JetBrains/compose-multiplatform/blob/v1.9.0/components/resources/library/src/commonMain/kotlin/org/jetbrains/compose/resources/ResourceEnvironment.kt)
  is authoritative for visibility at this pin. The
  [current locale-environment recipe](https://kotlinlang.org/docs/multiplatform/compose-resource-environment.html)
  also shows platform locale changes and a `key(locale)` subtree; remounting is not evidence that
  active input/focus/playback survives.
- [moko 0.27.1 source and setup](https://github.com/icerockdev/moko-resources/tree/release/0.27.1)
  declare Android/JVM/Apple support, StringDesc, plural APIs and static-iOS resource copy phases.
  Its [version catalog](https://github.com/icerockdev/moko-resources/blob/release/0.27.1/gradle/libs.versions.toml)
  separates Kotlin 2.1.0 runtime baseline from Kotlin 2.3.20/AGP 8.13.2 generator compile-only tooling.
  The JVM generator/test ran at Cantos's pins; Android/iOS plugin behavior did not.
- [Lyricist 1.8.0 source](https://github.com/adrielcafe/lyricist/tree/1.8.0)
  supplies manual catalogs, state and Compose provider. Its
  [pinned version catalog](https://github.com/adrielcafe/lyricist/blob/1.8.0/gradle/libs.versions.toml)
  uses Kotlin 2.2.21, coroutines 1.10.2 and Compose 1.7.3 (Compose is compile-only in its adapter).
  JVM compilation at Kotlin 2.2.20/Compose 1.9.0 passed, but does not establish Native KLIB
  compatibility. [Latest 1.9.0 pins](https://github.com/adrielcafe/lyricist/blob/1.9.0/gradle/libs.versions.toml)
  include Compose 1.10.3; that release was not tested or substituted into Cantos.
- [Unicode CLDR plural rules](https://unicode.org/cldr/charts/48/supplemental/language_plural_rules.html)
  distinguish the fixture's English one/other and Vietnamese other behavior. The three integer
  examples cannot certify fractional quantities, regional fallback, or other languages.

The experiment resolved each candidate's own transitive graph in isolation. It did not force
Cantos's coroutines 1.8.1 onto a library that requests a newer version. In particular, Lyricist
1.8.0 requests coroutines 1.10.2. Adopting it requires a separate transitive compatibility and
lifetime review; passing this fixture does not authorize an application dependency upgrade.

## Bounded split-locale repair versus adoption

At Compose 1.9.0 there is no public common resource-environment subtree override demonstrated
by this spike. Changing global JVM `Locale` or iOS `AppleLanguages`, suppressing internal
visibility, or using a locale-keyed remount would not satisfy the app-owned locale/state
preservation requirement merely because text changes.

The accompanying XML adapter is intentionally limited to the five existing launch/unavailable
messages: the application locale selects explicit generated resource IDs whose translations
remain in the existing XML owner. It does not alter the process locale or add a localization
runtime. This is a narrow compatibility seam for existing copy, **not** a canonical catalog,
general plural solution or full conversion of Shell/Auth/Membership. The PR separately
implements saved-locale preferences, count repairs and feedback; its resolver tests, native
reopen/isolation and app rendering evidence belong to the [repair ledger](issue169-cmp-locale-acceptance.md); candidate tests above must not be substituted for them.

Before wider conversion, #167 must review catalog shape, semantic/parameter contract, fallback,
CLDR formatting, migration/rollback and the library tradeoff. Retaining the current catalog
until that decision is different from accepting #169's outstanding requirements.

## Evidence ledger and remaining acceptance

| Layer / claim | Evidence level and status | Limits |
| --- | --- | --- |
| L1 architecture/ownership | documented — isolated input projects; no app dependency/adoption | Architecture decision remains OPEN |
| L2 typed error/parameter | type-enforced for closed fixture error and Kotlin lambda signature; resource IDs generated | Resource varargs are not named/typed parameters; no shared canonical schema |
| L3 fixture oracle | example-tested — exact vi/en strings, 0/1/2, literal braces/format text | No property/mutation suite or all-CLDR oracle |
| L4 Compose rendering | semantics-tested on JVM desktop; screenshot-inspected in vi/en for all three candidates | Fresh mounts, one default theme, not locale-switch/focus or native accessibility acceptance |
| L5 Android/iOS | NOT_RUN for all candidate builds, links and runtime | Requires target metadata/resolved graph inspection, pinned Native compiler, SDK/Xcode, native hosts and device scenarios |

Deferred acceptance **from these candidate tests**: implementation/adoption owner in #167 must
choose and review the architecture before migration. See the [repair ledger](issue169-cmp-locale-acceptance.md)
for separately executed application/Android reopen and release-isolation results. The candidate
fixtures alone do not prove locale precedence and saved key/value
preference; same-mounted locale-switch state/caret/focus preservation; app reopen; large-font,
light/dark/pseudo-locale layouts; native resource packaging and TalkBack/VoiceOver; and absence
of preview/mock dependencies from production APK/AAB under #122. Android resources need actual
SDK/AGP build and emulator/device resolution. iOS needs installed Kotlin/Native 2.2.20,
Xcode/framework resource copy setup, simulator/device resolution, and native locale integration.
No SQLDelight preference store or playback refactor is implied.

Execution blockers/failures: the corrected JVM candidate commands are recorded independently;
no Android/iOS candidate command was attempted, so those are **NOT_RUN**, not passing native
compatibility. Initial missing-import and transparent-capture fixture defects are recorded
above. There is no provider, database, network playback or audio-quality experiment.

Edge classes covered: Vietnamese diacritics; English singular versus plural at 0/1/2; literal
content braces and percent placeholders; exhaustive unavailable-error mapping; actual generated
resource access and actual Lyricist Compose provider on the JVM.

Not covered/residual risk: no library is accepted; KSP, fractional/all-language CLDR, parameter
schema/drift, native packaging, dynamic locale lifecycle, persistence, content language semantics,
font scaling/themes, focus/caret, screen readers and production artifact size remain open. The
next concrete acceptance is a #167 decision with target-specific build/link proof and a
same-mounted locale-switch scenario before any catalog migration.

Provenance: source baseline `2e3f29a120d2c5ab5962b8c4bf5a2dc8c92c30cb`; final tests ran
2026-10-16 at 11:50 Asia/Ho_Chi_Minh. Retained fixture bytes initially matched the executed ignored copies by SHA-256. Final
hygiene removed trailing whitespace in one Compose test and a redundant EOF blank line in one
Lyricist descriptor; comparison after normalizing only trailing whitespace still matches every
executed fixture. This formatting-only cleanup changes no tested token or behavior. Final fixture
file counts/UTF-8 bytes are Compose 7/5,616, moko 9/5,914 and Lyricist 5/5,316. All three candidate PNG pairs are byte-identical:
Vietnamese SHA-256 `731e0908fd54b6845feeabfb3012a127e0d1487583149eb381cd160e2f5ca061`,
English `d8af02447fa6b0342a187da2124ecb05c0ebb41cebcf344a7ede8d3ffe410e05`.
Fixtures are retained with this record; logs, generated files and PNG/semantics outputs are in
ignored `artifacts/agent-runs/issue169-candidates/`. Live versus mocked: actual local candidate
libraries/Compose desktop renderer, synthetic public text, no external service/native host.
Performance: generated-source inventory only; no comparative startup, APK/AAB/framework size,
CPU/memory or build-speed claim. Command duration is a single cached-host observation, not a
benchmark. Host: macOS arm64, Microsoft OpenJDK 17.0.19, one Gradle worker, 1,536 MiB heap.
