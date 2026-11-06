# Theatre CMP Library mobile empty composition — #453

Source-bound increment, 6 November 2026. Part of [#453](https://github.com/loveoverflowcom/cantos/issues/453),
following the [mobile addendum](https://github.com/loveoverflowcom/cantos/issues/453#issuecomment-6681052564).
Base: `origin/develop@6752f90226d096d73a033f7961e31ed7dbf161a9`.
The branch's three production composition files and focused tests define this record's inputs;
final commit identity and handoff-gate summary are recorded in the PR and issue comment.
This increment does not accept the whole Web/native issue or alter historical #581 evidence.

## Bug contract and owner

The completed-empty Library still mounted four empty section summaries below its global
invitation. At 390dp the first summaries occupied the bottom of the viewport; at larger text,
the decorative 240dp illustration delayed the readable invitation. These are composition
observations, not evidence of an obstructed or unscrollable page.

Invariant: after the existing supported personal Library reads have completed without retained
rows, show one coherent invitation and one real Discover action; preserve every nonterminal,
retained, shared-content and unavailable explanation. Text must keep its requested scale and
remain scrollable.

Owner: the app's `LibraryScreen` / `LibraryEmpty` and the existing shell scroll viewport.
`CantosTheatreApp -> CantosPresentation -> LibraryScreen` remains the production path for
Android, iOS and the JVM test host. No domain, controller, persistence or authorization decision
changes. The existing `isLibraryCompletelyEmpty`, section phases, capabilities and shared-row
guard remain authoritative. Behavior SSOT impact: none, presentation layout only; no state,
guard, event, transition or accepted model changes. The path-derived guest-navigation behavior
obligation is still selected by handoff preflight and reported separately there.

## Design and implementation

Read/opened inputs: [v4 handoff](https://drive.google.com/file/d/1rvDU68QnWcorgRMlTsQbWalVzIV7kSTf/view),
[v4 board 3](https://drive.google.com/file/d/1GBHHKVsTWAvV3QXFVmNC3Ysxi9qMF222/view),
[v4 board 6](https://drive.google.com/file/d/13C6Xbq944gkxV4n8o5ahLcHXL1btytV_/view),
[Library mobile concept](https://drive.google.com/file/d/1sB40xMwBwGC7gfypzziQCJwYhm8amJDV/view),
and the addendum's three current captures. The concept is Web-mobile inspiration; its error
copy, gradient, image and chrome are not native requirements. The linked dark 320dp text2
Library image is populated, not a global-empty specimen.

- Completed-empty omits the four redundant personal section containers. Partial/retained,
  loading, failed, stale and paginated Library sections keep their original content and actions.
- Disabled bookmarks still explain unavailability. The header/sync and shared Library slots
  remain mounted independently; retained shared rows still block the empty invitation.
- The shell passes its measured available content height, after header/player/navigation and
  safe-area/IME space, to Library. Only decorative artwork uses that budget: at most one third
  of the viewport divided by font scale, capped by the existing 224dp wide artwork size and
  available width. This app-owned geometry reserves space for the heading, help and action;
  it introduces no shared token, fixed text/container height or text-size reduction.
- The 1.2 artwork aspect ratio, untinted theme-aware resource owner, copy keys, flat theme,
  shared `button` primary action and `ShellEvent.Navigate(Discovery)` remain unchanged.
  Shared `button`/`text-field` catalog APIs and existing app compositions were looked up;
  no new primitive or component admission is introduced. Full History keeps its prior artwork.

Discovery hero/search/card/Save composition from #581 was inspected and retained. Opened JVM
390dp light VI and 320dp dark EN text2 top/card captures show the same hierarchy, wrapping title
beside a decorative arrow and separate Save action. The two 320dp captures are byte-identical
before/after. The 390dp capture differs in asynchronously decoded preview cover pixels, not
composition; this is not evidence of a resource fix. Search/query/clear/back, saved-state and
catalog-phase tests continue to run. Long-title and pending/pressed behavior has source/test
coverage, not a new native interaction or motion acceptance claim.

## Local evidence

Runner: macOS arm64, Gradle 8.14.3, JBR 21.0.11, repository Rust 1.99.1; pinned Kotlin 2.2.20 /
Compose configuration unchanged. All rendering below is **JVM Desktop Compose/Skia** at density
1, comfortable UI density and reduced motion. The workbench recipe is `theatre-library-empty-v1`,
seed 1: production shell/controllers/store with mocked external ports and an in-memory Library.
No fixture is included in a native host or promoted to production catalog data.

Evidence is local-only under ignored `artifacts/agent-runs/issue453-mobile/`:

| Check | Actual result / scope |
| --- | --- |
| Corrected empty regression against the three pinned BASE production files | FAIL as intended: `library-continue` still exists; `regression-baseline.log`. Current files were restored automatically. |
| `cargo xtask theatre-preview verify` after repair | PASS: shared 676 tests, 3 skipped; sandbox 188 tests, 4 skipped; zero failures/errors. `checkpoint-final.log`. |
| `PreviewArtworkResourceTest` | 1 executed, PASS in this run, unchanged. This does not diagnose or close #762. |
| State boundaries | Existing `LibraryEmptyViewTest`, `LibraryPresentationUiTest`, shared catalog/bookmark and real shell tests cover complete/nonterminal, retained/missing metadata, paging, stale/retry, unavailable and populated states. |
| New production semantics tests | One invitation without repeated sections, header/shared slot retention, art shrinking at 200% and a shorter viewport, text growing, scrollable 48dp action. |
| Existing focus/action test | Discover retains focus on theme/locale change and dispatches exactly once by keyboard. |
| Library mobile capture matrix | 16 combinations per before/after: 320x800 / 390x844dp, light/dark, VI/EN, font 1x/2x; top plus scrolled-to-CTA PNG/JSON/semantics, 32 captures per phase. |
| Android availability | `adb devices -l`: no devices. No APK build, installation, emulator, TalkBack, native IME/Back or Android runtime result claimed. |
| iOS / Web | NOT_RUN; no Web source changed, no iOS runtime acceptance claimed. |

The corrected baseline uses this existing focused primitive with the three production files
from BASE, without changing branch history or weakening a test:

```text
gradle --project-dir apps/cantos-theatre/mobile --no-daemon \
  -Dorg.gradle.java.installations.auto-download=false --max-workers 2 \
  -Pkotlin.compiler.execution.strategy=in-process --no-configuration-cache \
  :shared:desktopTest --tests \
  com.cantos.theatre.app.LibraryEmptyUiTest.completed_empty_library_has_one_invitation_without_redundant_empty_sections
```

Both module suites were then run by the supported `cargo xtask theatre-preview verify` lane.
Use the installed Gradle directory on `PATH` and JBR as `JAVA_HOME`; capture exports use
`CANTOS_SOURCE_REVISION` and `CANTOS_THEATRE_DEMO_CAPTURE_DIR`. The final-diff preflight command
is `TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.99.1 cargo xtask preflight --changed-only --base
6752f90226d096d73a033f7961e31ed7dbf161a9`. Its final output/summary is `preflight-final.log` and the PR. The first run had PASS repository,
Rust-format and CMP rows but an UNSUPPORTED Quint row; the repository-locked tool was then
installed explicitly with the existing Node 24.19.0/npm 11.9.0 binaries. The selected guest-navigation
model passed (`behavior-setup.log`); this is model-only, not implementation conformance.
There is no configured Kotlin formatter/linter; imports/style were reviewed manually and the
Kotlin compiler and real JVM tests ran. Rust-format/repository/model statuses come from preflight.

### Opened pixels and limits

The assistant opened the original Drive PNGs and these matched local before/after top pairs:
390 light VI 1x, 390 dark VI 1x, 320 dark EN 1x, 320 light VI 2x, 320 dark EN 2x, 390 dark EN 2x.
Also opened matched 320 light VI 2x and 390 dark EN 2x scrolled-CTA pairs. Captures use the
`issue417-LibraryEmpty-{theme}-{locale}-{width}x{height}-text{scale}-{top|library-empty-discover}`
filename family. Normal-text views retain a balanced invitation without redundant headings;
large text wraps at full scale and the real CTA is reachable by scrolling. The remaining matrix
PNGs are captured/tested, not individually visually approved. The three Discovery images above
use `issue453-discovery-*` filenames. Source/file hashes, capture hashes and opened-image names
are retained in local `manifest.json`; per-capture JSON also contains provenance and PNG SHA-256.
Screenshots establish no TalkBack/VoiceOver, native insets, OS text-scale mapping, motion or
physical touch result. Asynchronous cover decoding means the 390dp Discovery pair is not a
pixel-difference oracle for artwork.

## Review, acceptance and residual risk

Read-only self-review through `cantos-code-review`: inspected the complete diff, shell viewport
callers, Library projection guards, shared/unavailable slots, action/copy/artwork owners and
both modules' relevant tests. No actionable source defect found in this bounded increment.
L1/L2 source review preserves ownership and state authority; L3/L4 regression/semantics and
mocked shell flows pass. L5 JVM pixels are qualified as above; Android/iOS/Web acceptance remains
separate. No API, schema, persisted data, audio, provider cost, rights, release or deployment change.

The issue remains open. AC-CMP-D02/D04 receive this JVM composition/state evidence only;
AC-CMP-D01/D05/D06/D07 and existing whole-issue criteria remain unchecked where their native,
artifact or wider Web scope is unverified. Header/artwork/player owners #532/#417/#155 and #762
are preserved. Recheck on attributable Android/iOS builds with the same widths, locales/themes,
large text and screen readers before native acceptance. User-requested merge does not deploy.
