# Theatre CMP library MVP mock

## Review boundary

Part of [#35](https://github.com/loveoverflowcom/cantos/issues/35), following merged
[Web PR #83](https://github.com/loveoverflowcom/cantos/pull/83). Base: `ef5307225abee8ee0c3e014a5bb29f5f8fa7f82e`.
This slice changes the existing isolated CMP mock: **Khám phá / Của tôi** are the only
primary destinations; Của tôi displays Continue, Saved and History directly. Search,
account, full history and settings are secondary views. Account opens through the avatar.
One app-owned player/engine survives navigation.

No production listener resource, account, native audio, storage, download or sync adapter is
introduced. Saved/history/progress remain in memory and disappear on restart. The account
screen states that sign-in is unavailable. The mock cannot satisfy #35's account isolation,
restart, immutable-publication or real-data acceptance. #11–#17 and #34 retain their gates.
No new ADR is needed: this slice selects no dependency, production policy, API or schema.

## Ownership and invariants

Dependencies remain presentation → root/application → pure domain and narrow existing ports.
`ListeningSnapshot` defensively owns separate saved works, timestamp bookmarks, episode
progress and per-work history. Desired `SetSaved` writes are idempotent; unsaving preserves
every other collection. History is ordered by accepted simulated start, newest first, with
one latest episode per work. A progress update can update an existing history point but cannot
admit a new history entry or restore a cleared entry. No completed-listen policy is inferred
from the engine's end position.

The playback actor reports committed transitions directly to the root. A StateFlow collector
could conflate an accepted start followed immediately by pause; the ordered commit seam keeps
that observation. History admission requires a committed transition into simulated playback,
never a page open, restore, seek, play intent or stale callback. Clear-history confirmation
dispatches only `ClearHistory`, retaining progress, Saved, bookmarks, navigation and playback.

Resume validates the selected catalog episode and exact position, then restores paused.
Playback and listening barriers resolve the point after earlier queued controls/writes,
preserving subsecond positions even across a source change. Full history additionally checks
current membership. Later playback/reset/close changes fence delayed resume resolution.
No release binding is invented: these are synthetic episode identities, not immutable
production publication IDs.

Memory-write failure never becomes accepted UI success. Retry retains one failed action;
newer successful desired writes supersede an older failed write for that same fact. Unrelated
progress writes do not discard a failed user Save. Newer accepted playback points supersede
obsolete failed background points, avoiding a retry that rewinds progress or changes Continue
to an older chapter. A failed history admission can remain absent until another acknowledged
start; this mock promises neither durable retries nor lossless history during store failure.

| Claim / failure mode | Oracle | Evidence scope |
| --- | --- | --- |
| Retry/double desired Save must not toggle or duplicate | reducer/controller exact snapshots and memory-write checks | Kotlin/JVM examples, memory-only |
| Prepared/paused/stale callbacks must not create History | ordered fake-engine application scenarios | mock integration |
| Accepted start then rapid Pause must retain History | committed-transition hook test | deterministic virtual dispatcher |
| Replaying an older chapter at the same point must become latest Continue | pure progress/history ordering regression | Kotlin/JVM examples |
| Clear-history must not erase progress or recreate itself from later seeks | reducer/application and UI cancel/confirm scenarios | memory-only |
| Resume must use correct episode/ms and stay paused | actor/application tests and UI flow | fake media only |
| Exactly two primary destinations and one root mini-player | navigation tests, Compose semantics and driven route flow | JVM Desktop Compose |
| Vietnamese/English and large text must remain usable | localized-key parity, captures, named bounds/semantics | host evidence only |

## Verification provenance

Pinned source: Kotlin 2.2.20, Compose 1.9.0, Material3 1.8.2, Gradle 8.14.3 and Rust 1.87.0.
Installed JDK: Microsoft 17.0.19. Commands use `TMPDIR=/private/tmp`, installed Homebrew Python,
the installed Gradle distribution on PATH and the installed JDK through `JAVA_HOME`.
No tools, SDK, compiler or infrastructure were installed or provisioned.

Full local output and captures stay outside Git in `artifacts/agent-runs/cmp-library-mvp/`.
The baseline `cargo xtask theatre-demo verify` passed **116 tests, 17 suites, zero skipped**.
An isolated clean-base snapshot with only the changed listening domain and its tests then
passed **120 tests, 17 suites, zero skipped**, before the application integration was written.
Its output is `pure-snapshot.log`; test XML is retained under `pure-test-results/`.

The initial sandboxed baseline was `UNSUPPORTED`: Gradle could not initialize its native
library/cache. The same installed tools passed with cache access. An intermediate full-tree
core attempt was `FAIL` because concurrent UI edits referenced not-yet-written application
methods and new route variants. The isolated pure snapshot separated that compilation state
from domain evidence; no expectation or gate was weakened.

Final `cargo xtask preflight --changed-only` passed on the changed working tree:

```text
PASS repository
PASS rust-format
PASS theatre-cmp-demo
preflight: PASS
```

The CMP gate executed **151 tests in 17 suites, zero failures and zero skipped**. The
preceding integrated run passed 150 tests; the final run adds the cross-work failed-history
retry regression and includes keyboard Escape dismissal in the existing confirmation flow.
The repository gate passed **225 Python tests**; it also passed as a separate
`cargo xtask verify repository` run. No Kotlin formatter/linter is configured: Kotlin style
and inward imports were manually reviewed, and the compiler/test gate ran. Rust formatting
passed through preflight. The unchanged seven-file/51-step mock playback corpus ran on
Kotlin/JVM; Rust, WASM, Android and Kotlin/Native vector execution were not selected here.

| Layer / claim | Status and evidence | Remaining boundary |
| --- | --- | --- |
| L1 Architecture | PASS — independent read-only source review, no actionable findings | Inward dependencies; one app-owned player; TYPE/FP/ARC review |
| L2 Domain/types | PASS — type-enforced and example-tested | Independent immutable collections; exact desired Save/history transitions |
| L3 Unit/static | PASS — 151 common/JVM tests and compiler; VER regression cases | No configured Kotlin lint, property or mutation runner |
| L4 Application | PASS — mock-integration-tested with MockCatalogPort, MemoryProgressPort, MockMediaEngine and scripted ports | Ordered commit/read barriers, failed writes, stale/reset/close fencing |
| L5 JVM UI | PASS — semantics-tested, interaction-tested, screenshot-inspected | Desktop renderer only; no native lifecycle or screen-reader acceptance |
| L5 real API/database/media/account | NOT_RUN — outside this mock slice | #11–#17/#34 contracts and real runtime required |

The separate source review found and rechecked two sequencing defects before handoff:
obsolete failed background events rewinding/reordering progress, and resume reading a point
before preceding queued controls. Exact regressions cover both, including a cross-work retry.
No temporary instrumentation was added. Full logs, XML and capture hashes are retained locally;
GitHub CI was not checked or awaited.

`cargo xtask theatre-demo android-build` also **PASS**: 55 Gradle tasks executed and the
mock debug APK was assembled at
`apps/cantos-theatre/mobile/androidApp/build/outputs/apk/debug/androidApp-debug.apk`.
This compiles common/Android code and the native host; no app installation or device test ran.
The iOS build attempt with the installed Kotlin/Native 2.4.20 was **UNSUPPORTED**:
the command requires 2.2.20. Nothing was compiled or downloaded.

`cargo xtask compose-preview cantos-theatre --file
apps/cantos-theatre/mobile/shared/src/androidMain/kotlin/com/cantos/theatre/app/TheatrePreviews.kt
--composable TheatreLibraryLightPreview --label library-mvp-light-390 --timeout-seconds 30`
was **UNSUPPORTED**: the official CLI's `studio-check` exited 1. Its raw bundle is under
`artifacts/ui-evidence/compose-preview/cantos-theatre/20261013T071736Z-1d4f4c54d3b7/`.
No Android host image or semantics was captured; the JVM images above supply only host evidence.

## UI intent and limitations

Discovery/library intensity is Medium; mini-player Medium-Low; account Low. Titles and
listening points lead, with quiet secondary controls. The existing generated theme supplies
warm paper/lavender and dark roles. No palette, token, shared component, typography or motion
contract changes; shared component qualification remains unchanged. Visible and accessibility
copy uses the existing keyed vi/en resources. Content remains synthetic Vietnamese.

Baseline discovery/library PNGs were opened: three primary navigation items and three library
filter chips were visible. Those baseline images establish only the previous JVM presentation.
The new capture matrix covers discovery, library groups, empty library, work, history,
confirmation, account, narrow/wide sizes, both themes and 2× app text. The final preflight
generated **17 PNGs** under `artifacts/agent-runs/cmp-library-mvp/handoff-captures/` with
metadata/semantics siblings. Their bytes match the integrated capture set; 12 integrated
images were opened and the other five were opened from the final set. The local
`capture-index.json` records dimensions, theme, locale, scale and SHA-256 for every PNG.

| Inspected images / scenario | Observed pixels | Limits |
| --- | --- | --- |
| `jvm-light-discovery-390x844`, `jvm-light-discovery-840x840` | Exactly two primary destinations, avatar, secondary search, desired Save on cards and root mini-player; wide shelf uses two columns | Lower rows scroll; synthetic content |
| `jvm-light-library-mvp-390x844`, `jvm-light-library-history-390x844`, `jvm-light-library-390x844` | Direct Continue/Saved/History groups, no filter tabs; Vietnamese points and Saved actions readable; mini-player separated from navigation | Groups below fold require scrolling |
| `jvm-light-history-390x844`, `jvm-light-history-confirm-390x844` | Newest fixture first; clear action opens a readable confirmation explaining preserved data | Confirmation PNG is the actual 320×280 dialog node, not the 390×844 viewport |
| `jvm-light-library-empty-390x844`, `jvm-light-account-390x844` | Independent empty groups/Discovery actions; unavailable sign-in has a visible reason; persistent mini-player | Initial restored mock media can coexist with an empty listening collection |
| `jvm-dark-library-text2-320x800`, `jvm-dark-library-saved-text2-320x800`, `jvm-dark-library-history-text2-320x800` | English at 2× app text wraps; Vietnamese content retains diacritics; history action and point fit after scrolling; both nav labels remain readable | Large Saved cards exceed one viewport but remain scrollable; mini title truncates with full accessible name |
| `jvm-light-work-text2-320x800`, `jvm-light-work-text2-episodes-320x800` | Vietnamese title and episode actions wrap, scrolled third episode remains above mini-player/nav | Initial primary action may be partly below fold; scroll interaction tested |
| `jvm-dark-player-390x844`, `jvm-dark-error-390x844`, `jvm-light-settings-390x844` | Transport remains explicitly simulated, discovery Retry visible, settings are secondary; colors and text legible | No motion/native/audio quality claim |

Production Compose app interaction tests exercised Save/Unsave, avatar/account, library/history,
clear cancel/confirm, paused resume, unavailable recovery and persistent mini-player. Keyboard
tests focused the settings choice then pressed Escape to return with search preserved, and
focused Cancel then pressed Escape to dismiss history confirmation without clearing it.
This is bounded host accessibility evidence; automatic dialog focus placement/restoration and
TalkBack/VoiceOver were not observed. Semantic copy/parity tests cover vi/en. Token values and
contrast records are unchanged; no new contrast measurement is claimed. Reduced motion remains
the existing no-travel mock configuration; motion was not observed.

Android/iOS runtime, native insets/back gestures, TalkBack/VoiceOver, OS text scaling,
background/lock-screen audio, motion and cross-device/account acceptance remain unobserved.
JVM Compose pixels and semantics never count as device evidence. Production data/media/account
checks require the actual listener/publication contracts and real runtime; their issue owners
retain acceptance. Native accessibility owner is unassigned — assignment required before
acceptance.

## Deferred acceptance and compatibility

| Check / status | Reason and exact next command | Acceptance / next owner |
| --- | --- | --- |
| iOS compilation: UNSUPPORTED | Already installed Kotlin/Native 2.2.20 and compatible Xcode; `cargo xtask theatre-demo ios-build --kotlin-native-home /absolute/installed/kotlin-native-2.2.20` | Framework links with pinned toolchain; CMP owner unassigned |
| Android static preview: UNSUPPORTED | Official CLI and synced Theatre project in Quail 2+; rerun the exact capture command above | Open PNG and assert observed semantics; CMP owner unassigned |
| Native runtime/accessibility: NOT_RUN | Named Android/iOS device or simulator, then launch the built hosts using the app README | Repeat navigation/save/history/resume and text-scale flows; TalkBack/VoiceOver, insets/back and lifecycle; CMP/accessibility owner unassigned |
| Real listener/account sync: NOT_RUN | #11–#17/#34 contracts and runtime, then the 050 real-release/account flow | Same approved release/chapter/position across restart/devices; logout/switch isolation; respective issue owners |

No migrations, persisted data, wire DTO/API/version, mock playback vectors, cache or audio
revision, provider calls/costs, rights or approval gates changed. Existing app hosts mount
`AppContainer.mock`; the unimplemented listener boundary remains fail-closed. Rollback is
the previous mock source. Nothing here accepts production listening or closes #35/050.
