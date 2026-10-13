# Theatre CMP reference UI repair

## Boundary and provenance

Base: `origin/develop` at `909169b6fca661d5a2108298548e2a6b5616a145`, initially clean.
Branch: `codex/theatre-cmp-ui-repair`. The user requested a new fixing commit using five supplied
images and an external review. That review's suggested three commits and PR are diagnostic
proposal text, not the delivery authorization. This repair remains the isolated memory-only
Theatre mock and preserves two primary destinations, Save/History/paused Resume, the one
app-owned player and truthful simulation notices. No backend/API/native playback/account or
durable-data rule changes. Production item 050 remains outside this review boundary.

The exact [Mobile v2 package](https://drive.google.com/file/d/157jbpHUTKZ-tDzNfXD878nvnF4thWmM4/view)
was downloaded through the user's existing Chrome session after raw connector transport failed.
Its HTML/CSS, [handoff](https://drive.google.com/file/d/1U55XW5txGamBYOm_MGW3itBH_BehJtfj/view),
static board and original paintings were read/opened. The board is a design proposal, not runtime
pixels. Bundled PNG/TTF bytes are unchanged extractions; crops are renderer operations.
The [asset manifest](../../apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/provenance.json)
records their hashes and source. Noto Sans/Serif 2.015 regular/bold identify SIL OFL 1.1 in font
metadata; the included license comes from the
[official Noto source](https://github.com/notofonts/latin-greek-cyrillic/blob/main/OFL.txt).
The package supplies no independent artist/redistribution license for its paintings. Their use
here is the user's requested mock-design repair; production artwork rights still need review.
No narrative/audio or private production assets are included.

## Bug contract and repair

Fresh JVM captures reproduced the supplied large-text episode and dark player symptoms:
density 1, base font scale 1, app text scale 2/1, respectively. The images alone had unknown
build identity; reproduction pins the new evidence to this base. No font-scale compounding was
observed or changed. The header notice occupied fixed chrome, the transport FlowRow separated
Forward, source artwork was a placeholder and font families were system fallbacks.

| Invariant / owner | Repair / oracle |
| --- | --- |
| Large text preserves reachable header, episode and shell actions | Put demo notice inside scrolling content; compact episode index, two-line title and full accessible name; short-screen bounds tests |
| Primary transport keeps backward/center/forward order | One icon-control row with explicit 48/80/48 dp targets, visible 15-second captions and unchanged events; 2× short-screen bounds and seek tests |
| Work identity leads the content | Exact paintings in resume thumbnail, work card/detail and player; unknown/missing/undecodable artwork uses tonal fallback; resource decode tests |
| Font identity preserves Vietnamese and host scaling | Theatre supplies licensed resource families to the shared theme; other callers keep existing defaults; glyph and theme-injection tests |
| Settings remain quiet and modal focus returns | Rows with current values; named radio dialogs for theme/language/speed/timer; separate Demo tools dialog for all four catalog states and reset; input/focus/event tests |
| One player and app-owned preferences survive navigation | Mini-player thumbnail and dedicated player token roles; existing Save/History/resume/session tests plus settings/player preference flow |

`color.playerSurface`, `color.onPlayerSurface`, `color.playerAccent` are additive canonical
v0.2.1 roles. Both themes intentionally use the handoff's separate dark player surface and pale
control accent. All old role values are unchanged. CSS/Rust/Kotlin and both token catalogs are
regenerated through existing generators; all player contrast pairs are checked. Theme extension
accepts font families without changing its host-font-scale calculation. No new shared component
is admitted or qualified.

Intent: discovery/library Medium; full player Medium-High; mini-player Medium-Low; settings Low.
Visible/accessibility additions use DemoCopy vi/en, while Vietnamese content stays unchanged.
The third design painting is Mộng Hồ Điệp, absent from this mock catalog: Cánh Buồm keeps a
fallback rather than receiving unrelated artwork. Header/demo disclosure and two-tab navigation
intentionally follow the current mock scope over the older board. Secondary tools may wrap;
the three primary transport controls never reorder. Large list titles can ellipsize after two
lines; their full name remains accessible and appears in the full player.

## Verification record

Local logs/captures: ignored `artifacts/agent-runs/cmp-ui-repair/`. Tools: Rust 1.87.0,
Gradle 8.14.3, Microsoft JDK 17.0.19, Kotlin 2.2.20, Compose 1.9.0, Material3 1.8.2.
Execution uses installed Gradle/JDK/Python on PATH, `TMPDIR=/private/tmp`, and
`CANTOS_THEATRE_DEMO_CAPTURE_DIR` for the named before/after sets. No tool, SDK, IDE,
infrastructure or compiler was installed/provisioned.

Baseline `cargo xtask theatre-demo verify`: PASS, 152 tests after adding only the capture matrix.
Regression-red: 155 tests, two intentional failures (settings chips outside dialog and separated
transport). A fixture typo was corrected before this red run. Intermediate implementation failures
were diagnosed and fixed: resource manifests must be under `files/`; explicit FontFamily local
type; Surface Button role; dialog focus after window attachment; updated reset-dialog traversal.
An import cleanup temporarily removed used types and was corrected. No test expectation or gate
was weakened to bless a failed layout.

Final gates on the repaired tree:

| Exact command | Result / observed scope |
| --- | --- |
| `cargo xtask preflight --changed-only` | PASS: repository, rust-format, selected Rust lint/tests, both Web lint/build consumers, Leptos preview, CMP preview, Theatre CMP |
| `cargo xtask theatre-demo verify` (through preflight) | PASS: 160 tests, 19 suites, zero failures/skipped, including preserved Save/History/resume and new resource/layout/modal flows |
| `cargo xtask ui-preview cmp verify` (through preflight's identical desktopTest gate) | PASS: 78 tests, 11 suites, zero failures/skipped, including font injection and unchanged font-scale nesting |
| `cargo xtask verify rust` | PASS: workspace format/lint/tests; no database or device acceptance inferred |
| `cargo xtask theatre-demo android-build` | PASS: 61 Gradle tasks, 29 executed; mock debug APK assembled; six bundled artwork/font hashes independently checked inside APK |
| `cargo xtask verify contracts` | UNSUPPORTED: installed ffmpeg/ffprobe absent from PATH; audio-inspection unavailable, script-ir-reference/contract-tests/wav-reference NOT_RUN by prerequisite precheck |
| `cargo xtask compose-preview cantos-theatre …` | UNSUPPORTED: official CLI studio-check exited 1; no running synced IDE, no Android image/semantics produced |

The repository gate and standalone `cargo xtask verify repository` both passed 226 Python tests. No remote CI was checked or awaited. Contract/audio tools were
not installed to satisfy this UI repair; rerun the exact contracts command with trusted installed
ffmpeg and ffprobe on PATH. These unchanged media/IR gates remain unaccepted by this run.

The visual follow-up added a failing regression (160 tests, one failure) for the initially hidden
featured/Saved work title, then used compact Saved cover rows and a featured caption band.
Both titles are now visible without scrolling. This band is deliberately opaque for text
contrast, rather than assuming contrast over painting pixels. A generator version-validation
regression also preserves ValueError for malformed non-string inputs after adding v0.2.1.

| Layer | Evidence / boundary |
| --- | --- |
| L1 Architecture | Source-reviewed: presentation → existing application/domain/ports; one mutable authority per existing fact; local dialog/focus state only |
| L2 Domain/types | Unchanged domain invariants; finite local PreferencePanel; cancellation rethrown at resource fallback boundary |
| L3 Unit/static | Common/JVM reducer/controller regressions, exact Vietnamese font coverage, image decode/fallback, compiler and canonical contrast/drift checks |
| L4 Application/UI | Desktop Compose input/semantics: dialog dismissal/return focus, selected controls, speed/timer/localization, Save/History/resume/reset and one session |
| L5 Renderer/native | Opened JVM pixels described below; native runtime/assistive technology remains deferred |

No configured Kotlin formatter/linter exists. Touched Kotlin imports/style are manually reviewed;
compiler and applicable host suites are the executable checks. Rust formatting is checked through
preflight. The unchanged playback corpus runs on Kotlin/JVM as part of the mock suite.

## Visual observations

Agent-opened fresh baseline: `before/jvm-dark-player-390x844.png` and
`before/jvm-light-work-text2-episodes-320x800.png`. These reproduce the supplied layouts.
Before/after scenarios retain fixture, viewport, theme, locale and text scale; capture JSON records
density/base font scale. Files named with `text2` use app scale 2; native OS scaling is unobserved.

The final run produced 26 PNGs plus JSON/semantics siblings; `capture-index.json` records hashes.
Agent-opened final captures (all names below under `after/`):

| Opened scenario | Observed pixels / limits |
| --- | --- |
| `jvm-light-discovery-390x844`, `jvm-dark-error-390x844` | Real resume thumbnail, primary two-tab shell, readable work title in featured caption; Error keeps point and Retry without pretending catalog is Ready |
| `jvm-light-library-mvp-390x844` | Two resume entries, compact Saved cover/title/metadata and independent Save button visible above mini-player; later history still scrolls |
| `jvm-light-work-390x844`, `jvm-light-work-text2-episodes-320x800` | Exact cover/crop, full work title, independent actions; selected scrolled episode and its play action clear of shell; two-line row ellipsis has full accessible name |
| `jvm-dark-player-390x844`, `jvm-light-player-text2-320x800`, `jvm-dark-player-short-320x568` | Original painting, full Vietnamese heading, elapsed/total and ordered transport; at 2×/short height, scrolling keeps controls reachable, with secondary tools below fold |
| `jvm-light-settings-390x844`, `jvm-dark-settings-390x844` | Quiet rows show current values; memory-only note and Demo tools remain separate from choices |
| `jvm-light-theme-dialog-390x844`, `jvm-dark-timer-dialog-text2-320x800` | Named choices and current-state radio, readable Vietnamese marks; 2× timer explanation plus Off/15/30/end and Done retained in scrollable dialog |
| `jvm-dark-library-text2-320x800`, `jvm-light-shell-short-320x568` | English chrome and Vietnamese content reflow; fixed shell retains named mini actions and two destinations; lower groups require scrolling |

Some of the first opened final paths were inspected before the last featured-card refinement;
the changed Discovery/Library/short-shell paths were then reopened after it, with identical
scenario inputs. Unopened remaining captures are only screenshot-captured. The confirmed
observations above, rather than the existence of all 26 images, define visual acceptance.

The Leptos token gallery was driven at `http://127.0.0.1:8083/#gallery-tokens` in the Codex
in-app browser, 1265×712 viewport. Filtering `player`, expanding all three roles and switching
light/dark showed pale on-surface/accent and a dark player swatch, with legible Vietnamese
labels. DOM-backed computed CSS values were read/checked against `#34283f`, `#f7eef9`,
`#dec6f0` (dark assertion; light readback); screenshots were opened inline in this conversation.
This is bounded token-gallery evidence, not a fresh acceptance of all Studio/Theatre journeys.
The temporary tab and owned preview server were closed.
Dialog captures use the actual dialog node on Desktop, since it is a separate native host window;
the JSON distinguishes requested viewport from actual dialog PNG size. A captured image alone
never establishes focus, semantics, motion, native rendering or screen-reader acceptance.

## Native resource boundary and deferred acceptance

The manually linked iOS host now copies common resources through its Xcode build phase to
`compose-resources/composeResources/com.cantos.theatre.resources`. The location follows the
[pinned 1.9.0 iOS reader](https://github.com/JetBrains/compose-multiplatform/blob/v1.9.0/components/resources/library/src/iosMain/kotlin/org/jetbrains/compose/resources/ResourceReader.ios.kt)
and generated Res paths. `plutil -lint` passed; the copy phase was exercised against a disposable
local bundle and exact resource hashes were compared. This is packaging evidence, not an iOS
compile or launch. Xcode still neither invokes nor downloads Gradle.

Required native acceptance: rerun `cargo xtask compose-preview cantos-theatre --file
apps/cantos-theatre/mobile/shared/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt
--composable TheatrePlayerDarkPreview --label ui-repair-player-dark --timeout-seconds 30` with
official CLI and a synced Quail 2+ Theatre project; open the PNG and assert observed semantics.
Then run Android/iOS hosts on named devices/simulators: 390×844 light/dark Vietnamese 1×,
320×800 2×, 320×568 short shell/player, English/long titles, native safe areas/IME/back,
TalkBack/VoiceOver, dialog focus and motion. Native/accessibility owner: unassigned — assignment
required before acceptance. iOS additionally needs installed Kotlin/Native 2.2.20 and compatible
Xcode for `cargo xtask theatre-demo ios-build --kotlin-native-home /absolute/installed/native-2.2.20`.

Residual risk: Desktop Compose is not Android/iOS; no native insets, OS text scaling, IME,
screen reader, motion, lifecycle, real media or production account/sync acceptance is claimed.
Artwork production rights are unverified. JVM fallback/font tests do not establish Native font
loading. The mock remains memory-only and its simulated timer runs no countdown.


## Delivery

The work queue links this bounded repair without changing production ordering or GitHub status.
Only the requested new local commit is authorized; no push, PR, issue update, provider call or
publication is part of this delivery. No skill or shared agent configuration was edited.
