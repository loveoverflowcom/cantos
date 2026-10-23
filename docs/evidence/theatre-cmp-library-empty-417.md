# Theatre CMP Library empty presentation

This source-bound record accompanies [#417](https://github.com/loveoverflowcom/cantos/issues/417).
Its implementation base is `develop@e4085c9761de7b66c43ca52f5cd04b9a4d8528c6`
(the merged Web #416 slice). It is presentation work for Theatre CMP only. It does not accept
Android/iOS runtime accessibility, backend integration, authentication or production release.
The PR carries the final-diff preflight result and the local capture/review ledger.

## Invariant and ownership

Only a `Complete` section with no retained records establishes emptiness. Saved IDs or listening
points without metadata still count as content; they keep the existing unavailable-metadata
explanation. A completed empty Library gets one primary Discover invitation and compact Continue,
Saved, Bookmarks and History summaries. Partially empty libraries keep contextual explanations and
at most one local Discover invitation. Full History uses the same composition with its own copy.
Empty completed History has neither View all nor Clear; paged/recoverable History retains its path.

`LibraryEmptyView` derives these facts from the current immutable projection, without remembered
emptiness or new state authority. Disabled bookmarks receive an unavailable explanation and are
excluded from the supported-section empty calculation. Guest sign-in guidance remains owned by
the existing Library header. The header's explicit Refresh is retained; `LibraryFooter` remains
unchanged, including `Complete -> Unit`, section retry and load-more intents.

No API, contract, backend, auth, account fence, player, pagination or sync/outbox engine changes.
No settled record is written by the presentation. Dependency/lifetime direction is unchanged:
application owners project facts, the screen renders values, and callbacks dispatch existing
intents to the shell. No new dependency, module, token or ADR is needed for this bounded composition.
Queue: no change, because task order and prerequisites are unchanged.

## Component lookup and design choice

The shared catalog's `empty-state` / CMP `EmptyState` was inspected. It has a fixed `menu_book`
symbol and one stacked anatomy, with CMP qualification still `implemented-unverified`. Extending
its API would affect unrelated consumers and require shared admission. The Library instead owns
one screen-specific composition, reused for sections, the global invitation and full History.
It composes existing stable Material controls over Cantos tokens; it adds no shared component API.

Canonical registry glyphs are selected in one `LibraryEmptyKind` map: play for Continue, book for
Saved/library, the existing player bookmark star for Bookmarks, and audio list for History. The
symbol is decorative; adjacent localized text carries its meaning. No glyph is fetched at runtime.
A trial registry extension was discarded to retain existing shared API and qualification inputs.

Primary goal: find a story to start a new Library or return to existing listening. Primary action:
Discover only where a completed section is empty. Existing content leads in partial libraries;
refresh, account and paging remain secondary tools. Intensity is Medium for Library and
Low–Medium for empty feedback: one tonal container, a restrained symbol, emphasized editorial
title and a filled pill action, with no gradients or added animation. Nonhero rows use the page
background. At enlarged text the symbol stacks above the copy to preserve reading width.

Both presentation directions were rendered on the actual production shell at 390 × 844 dp:

| Direction | Observation | Decision |
| --- | --- | --- |
| Hierarchical per-section empties | Contextual symbols and one action improve the baseline, but repeated empty titles dominate and History remains far below the fold. | Retain this treatment only for partial libraries. |
| Global tonal invitation plus compact summaries | One focal explanation/action, then quieter section meanings; no repeated “nothing here” headings or Discover actions. | Selected for completed empty supported sections. |

Baseline and both options were opened. The first 320dp/200% hero captures exposed too little
reading width beside the symbol; the selected source stacks it above text and removes separate
row surfaces. Same-recipe final recheck and artifacts are recorded in the PR/local ledger.

The authenticated [mobile design overview](https://drive.google.com/file/d/1m-djID3J20t4i26mmKtZSR8lRQl_Evi3/view)
was opened from the supplied [design folder](https://drive.google.com/drive/folders/1GVOA3v-TGRRo23dFdeHW2lg32WI9DPZ4).
It supports lavender containment, calm type/action hierarchy and native density, but supplies no
approved CMP empty-state reference at these exact viewports. This is not a pixel-fidelity claim.
Web #416 informs vocabulary only; no Web markup/layout or runtime is imported.

## Deterministic and rendered evidence

All preview recipes mount `PreviewTheatreApp → CantosTheatreApp → TheatreShell` with the production
controllers and in-memory SQLite Library/outbox. Only external catalog, identity, Library HTTP,
playback access/decoder and locale ports are mocks. `PreviewLibraryRead` holds/refuses actual port
reads; Stale first admits pages then fails a refresh. It never forces a production UI phase.
The workbench recipe schema remains version 2 with additive fixture IDs.

| Claim | Oracle / production seam | Evidence boundary |
| --- | --- | --- |
| Complete versus every other phase | `LibraryEmptyViewTest`, every section × all seven phases; retained IDs/points without metadata | Pure example-tested projection; no I/O |
| Updates and disabled capability | Current snapshot save/play/clear/account replacement tables; unavailable-bookmark UI assertion | Pure projection / JVM semantics |
| One action and usable target | `LibraryEmptyUiTest`, 320dp/200%, minimum 48dp bounds, button role, focus survives theme/locale, one Enter dispatch | JVM semantics/keyboard interaction; no screen reader |
| Retry and paging stay distinct | All seven full-History phases and existing `LibraryPresentationUiTest` section regressions | Existing production footer and exact intent assertions |
| Actual shell navigation | Completed empty CTA test asserts exactly one Discovery root | Production shell over external mocks |
| Save/unsave and clear update UI | Existing `PreviewShellFlowTest` augmented with hero removal/return and full-History empty checks | Real controllers/outbox and in-memory SQLite; no native durability |
| State/theme/locale/scale | `LibraryEmptyCaptureTest`: six recipes × light/dark × VI/EN; additional 320 × 800dp/200% action/last-section scroll captures | JVM Desktop Compose pixels plus exported semantics; opened subset separately named in PR |

New `CopyKey` summaries, unavailable reason and the reused hero copy have VI/EN values. Existing
copy parity/parameter tests remain in the shared suite. No new translation mechanism or fixed
text-container heights. No Kotlin formatter/linter is configured; compilation, tests and manual
style review are the available Kotlin checks. The behavior catalog has no registered Library
empty-presentation capability; existing sync/admission models and their claims are unchanged.

The baseline `cargo xtask theatre-preview verify` passed. The hierarchical option passed. The
hero checkpoint passed after two diagnosed development failures (old all-empty text assertions,
then test-call type/argument errors); these were corrected without suppressing tests. Full output
and captures are retained under ignored `artifacts/agent-runs/issue417/` as `before`, `hierarchical`,
`hero` and final-diff `final`. Capture sidecars name the renderer, recipe, locale/theme, dp/pixel
size, host density, scale and PNG hash. The final-diff status is reported verbatim in the PR.

## Native evidence and deferred acceptance

Mac environment: installed Gradle 8.14.3, Microsoft JDK 21.0.7, Android SDK 35/build-tools 35.0.0,
Xcode. The restricted filesystem sandbox could not load Gradle's native dylib; the same installed
Gradle works with authorized cache access outside the sandbox. No toolchain or SDK was installed,
no data/worktree/cache was deleted, and no production endpoint was called.

| Target / layer | Result | Scope / remaining acceptance |
| --- | --- | --- |
| Android debug build | PASS | `cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:9`; actual APK built and inspected, not installed or run |
| Android runtime | NOT_RUN | `adb devices -l` found no device/emulator; app/device acceptance remains open |
| Android accessibility | NOT_RUN | No TalkBack/font-scale/inset/orientation run; JVM bounds/keyboard evidence is separate |
| iOS framework build | UNSUPPORTED | `cargo xtask theatre-host ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.2.20` refused missing `bin/konanc`; installed 2.2.0/2.4.20 were not substituted |
| iOS runtime | NOT_RUN | `simctl` listed no installed runtime/device |
| iOS accessibility | NOT_RUN | No VoiceOver/Dynamic Type/safe-area/orientation run |
| Android Studio layout capture | UNSUPPORTED | Theatre has no Android preview wrappers after #287; no IDE or wrapper was scaffolded |

Next native owner: unassigned; assignment required before acceptance. Use the existing build
commands and an available target, then walk Library/full History at 200% text in both orientations,
light/dark, with safe areas/insets, scroll/focus, ≥48dp target checks, contrast and OS reduced
motion, and TalkBack/VoiceOver respectively. No new motion was added, but OS preference delivery
and assistive technology behavior remain unaccepted. Issue #417 must remain open for that evidence.
