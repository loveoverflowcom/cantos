# Theatre CMP mock architecture and evidence

The current source has production-safe `:shared` and a separate JVM-desktop-only `:preview-sandbox`
KMP module (#287: no Android or iOS target, framework or source set). The sandbox depends on shared
UI/domain/application/ports and supplies mock *external ports* only; it mounts the same
`CantosTheatreApp`/`TheatreShell` as the native hosts (see [Desktop Preview composition](#desktop-preview-composition)).
Current checks belong to the [#122 isolation record](../../../../docs/evidence/issue122-cmp-sandbox-isolation.md).
The one Android application `:androidApp` (build types `debug`/`release`, no flavors) composes `:shared` only; its release
compile/runtime graphs and actual unsigned APK/AAB are inspected by `cargo xtask theatre-host
android-release`. A Desktop Preview build establishes neither production exclusion nor native listening
acceptance. The production hosts mount `CantosTheatreApp` over the live catalog, account, durable
Library and native player (see the [README](../README.md#the-application-over-real-owners)); it
claims no download capability; bookmarks are the account Library's release-bound resource ([#299](https://github.com/loveoverflowcom/cantos/issues/299), README). [ADR 0026](../../../../docs/decisions/0026-theatre-mobile-local-storage-sandbox.md)
remains proposed.

The historical architecture/evidence below began with the requested 407-line brief and
[PR63](https://github.com/loveoverflowcom/cantos/pull/63). Its
[phase1 checkpoint](mock-skeleton.md) is commit `2a425a87ab20aba4061d2910bf02df62c32652e9`.
The source extraction adds no service, FFI or generic MVI framework and preserves the existing stack.

The later [#35 library MVP record](../../../../docs/evidence/theatre-cmp-library-mvp.md)
supersedes the navigation/library observations below: two primary destinations, three direct
library groups and a separate accepted-play history collection. Earlier captures remain
historical evidence for their named revisions, not evidence for this changed presentation.

The reviewed refactor commit `a74c084e97f7c175c9d90fd65c189490241fb7b1` was preserved before
integrating concurrent remote head `b0869291078ec9100f49a5dd24d1eb60a9393a01`. The latter retains
the other owner's `b97b6610a097f9edcdb916f3e511924086f8486b` card-shape/endonym fixes and
`develop` after PR62 (`2de8c694ec695f36fd80a34f4b8305580d5edace`). Both card-shape hunks are
transferred into the moved stateless presentation; no remote work is overwritten. Its external
Linux diagnostic record remains attributed in the historical checkpoint, separate from Mac evidence.

## Ownership and execution

> **Status (#287).** The owner table below and the diagram that follows describe the earlier
> `AppContainer` composition (`CatalogController`, `ListeningController`, `ProgressPort`,
> `CatalogPort`). No host composes them any more: the application's owners are the ones in the
> [README](../README.md#the-application-over-real-owners) (`PublicCatalogController`,
> `NativeAccountController`, `NativeLibraryController`, `NativeListeningController`,
> `PreferencesController`) under one `TheatreShell`. The legacy classes remain in `:shared` pending
> a separate removal, and their sandbox mock database and tests were deleted with the parallel
> demo path. Rows that still apply unchanged are `domain/playback`, `application/playback`,
> `application/preferences`, `presentation/model`, the two presentation features and
> `application/identity`.

Shared `AppContainer` requires explicit ports, an `AppStartState` and one media engine, and
creates one playback controller for the app owner. There is no `AppContainer.mock` factory
or default mock fallback in shared main code. Navigation, detail, player and settings do not
construct another engine. Native lifetime behavior still needs device/simulator evidence;
common/JVM regression suites exercise app wiring with mock ports, with current execution
recorded separately from the historical ledger below.

| Owner | Authoritative value | Boundary |
| --- | --- | --- |
| `domain/publication` | Work/Episode keys, immutable catalog inventory and playback-source conversion | no synthetic fixture corpus, approved release, asset URL, rights, API or framework dependency |
| `domain/playback` | generation/session, intent and observed fake-engine phase | pure `reducePlayback -> PlaybackTransition(nextState, effects)` |
| `domain/listening` | separate SavedWork, Bookmark, ProgressRecord and accepted-play history collections | no completion inference, account, outbox or reconciliation policy |
| `application/catalog` | immutable catalog StateFlow and monotonic request tags | CatalogPort replies cannot replace a newer/reset request |
| `application/playback` | one serialized event loop and a separate ordered effect worker | only observation callbacks enter MediaEngine.events; matching source/generation required |
| `application/listening` | one ordered write owner and typed store failure | ProgressPort I/O is outside state assignment; failed writes are not accepted; the sandbox mock database serializes every port it serves ([mock storage](../README.md#mock-storage-simulator)) |
| `application/preferences` / `app/Navigation` | independent preferences and pure navigation state | StateFlow.update functions perform pure transitions only |
| `presentation/model` | neutral immutable `ShellState`, `ShellEvent`, `ShellCatalogView` and keyed `ShellCopy` vi/en text (`CopyKey`, `clockTime`) | pure shared leaf consumed by shell, auth and Membership; synthetic defaults, IDs and fixture mappings stay in the sandbox |
| `features/shell/presentation` | supplied read-only projection/catalog metadata | emits intents; no fixture/reducer/port/I/O imports; `CantosPresentation.kt` is the root and shared helpers, with `PlayerSection.kt`, `LibrarySection.kt` and `SettingsSection.kt` as same-package, state-free section files. Account/auth content arrives as slots; `RouteRestoration.kt` keeps per-entry scroll and restores focus after Back from auth; artwork is injected with a neutral fallback |
| `features/auth/presentation` | `AccountView` (an `application/identity` projection), route and theme from `ShellState` | stateless Login, Register-unavailable and guest/signed-in Account over the #127 variants; emits `AuthIntent` only, with no controller, port or provider; listener copy in `AuthCopy.kt` |
| `application/identity` | `NativeAccountController` (one owner, unchanged) and the pure `AccountView` projection | `AppContainer` observes it, never closes it, and gates `openLogin/login/restoreSession/back/logout` over the pure `Login`/`Register` navigation suffix |

The sandbox root derives `ShellState` through `projectDemoState` for rendering; it does not own a writable aggregate mirroring the
controllers. Query, timer selection, confirmation and scroll remain screen-local. The legacy
library-tab seed is diagnostic compatibility only; no library tabs are rendered. Fixture data
is under sandbox `adapters/mock`, and simulated local storage under its
[`sandbox`](#mock-app-sandbox), separate from presentation metadata and domain rules. The current
six presentation identifiers are diagnostic fixture identifiers, not production publication IDs.
No DownloadedAsset/offline subsystem is added without a consumer.

```mermaid
flowchart LR
  UI[Stateless presentation] -->|intents| App[AppContainer / root]
  App --> Catalog[CatalogController]
  App --> Playback[One PlaybackController]
  App --> Listening[ListeningController]
  App --> Prefs[Preferences / navigation]
  Catalog --> CP[CatalogPort / MockCatalogPort]
  Playback --> Core[Pure playback transition]
  Playback -->|ordered effects| Engine[MediaEngine / MockMediaEngine]
  Engine -->|fenced observations| Playback
  Listening --> PP[ProgressPort / MockPersistentProgressPort]
  PP --> DB[(Sandbox MockStorageDatabase)]
  Catalog --> View[Derived projection]
  Playback --> View
  Listening --> View
  Prefs --> View
  View --> UI
```

Public intents and consumers use the host application dispatcher; tests inject a deterministic
dispatcher. Owned child scopes cancel without cancelling their caller. The pinned coroutines
dependency is 1.8.1; the [StateFlow source](https://github.com/Kotlin/kotlinx.coroutines/blob/1.8.1/kotlinx-coroutines-core/common/src/flow/StateFlow.kt)
documents read-only exposure, equality conflation and potentially repeated update transforms.
No effect or I/O is placed inside those transforms. The MediaEngine seam is mock-scoped;
Media3/AVFoundation, backend-authorized publications and production lifecycle policy are future
reviewed consumers, not implementations implied by these interfaces.

<a id="mock-storage-simulator"></a>
<a id="mock-app-sandbox"></a>

## Desktop Preview composition

`PreviewOwners` (`com.cantos.theatre.app`, sandbox) is the single composition boundary. It builds
the production owners and wires `TheatreShell`; `PreviewTheatreApp` mounts `CantosTheatreApp` over
it. Mock **external ports** live in `com.cantos.theatre.adapters.preview`. Shared real adapters
(`adapters/http`, `adapters/identity`, `adapters/library`) never import them.

```text
preview-sandbox/src/commonMain/kotlin/com/cantos/theatre/
  adapters/preview/
    PreviewFixtures.kt                synthetic Discovery values under canonical UUIDs (PreviewWorkIds/EpisodeIds)
    PreviewCatalogPort.kt             PublicCatalogPort: pages, search, Ready/Loading/Empty/Error
    PreviewPlayback.kt                PlaybackAccessPort + StreamingMediaEngine decoder stand-in
    PreviewIdentity.kt                NativeIdentityV1Api/journal/barrier -> real NativeAccountController
    PreviewLibraryApi.kt              LibraryApi: in-memory remote with idempotent receipts
    PreviewLocalePreferenceStore.kt   LocalePreferenceStore
  app/
    PreviewOwners.kt                  composition boundary + PreviewSeed application
    PreviewSeeds.kt                   previewState(), PreviewSession (Guest / Fresh / Restored)
    PreviewTheatreApp.kt              CantosTheatreApp + sample artwork + Membership fixture slots
preview-sandbox/src/desktopMain/        Main.kt, previewLibraryStore() (production SQLite, in memory), workbench
```

| Consumer | Entry | Data |
| --- | --- | --- |
| Desktop Preview window | `PreviewOwners(scope, PreviewSession.Guest.seed(), previewLibraryStore())` | a guest on Discover; sample catalog |
| Workbench recipes | `PreviewOwners` per `TheatreMount` from `TheatreRecipe.initialSeed` | `Fresh` guest, `Restored` signed-in with remote data, catalog scenarios, `Offline` |
| Owner-level tests | `PreviewHarness` (desktopTest) over `runTest` + `backgroundScope` | explicit `PreviewSeed` |
| UI/capture tests | `PreviewTestRoot` composable | explicit `PreviewSeed` |

Two rules keep the preview honest. First, nothing in `:shared` knows the sandbox: the only
addition is the optional `TheatreShell(initialMembership = …)` argument. Second, a seed describes a
starting point and reaches the owners through their own entry points (the shell's navigation and
events, the remote Library data, the account journal); a guest's selected episode is made the way
a listener makes it (open, then settle paused), because the runtime clears any session when a guest
is established. Cantos-owned mock types use `Preview`; third-party names are unchanged.

The inward layer direction is unchanged: `domain`, `ports` and `application` cannot import
`adapters` (the repository `check_theatre_architecture` guard rejects it), and `features` cannot
import `adapters` or `app`. Tests of shared classes live in `:shared` (`ShellStateProjectionTest`,
`ShellPresentationStateTest`, `ShellLayoutTest`, `PlaybackControllerTest` over the test double
`ScriptedMediaEngine`, the copy and motion tests); sandbox tests mount the preview, so tests
introduce no `:shared` → `:preview-sandbox` cycle. Sample art/provenance stay in sandbox
resources; fonts, brand and production-safe localization remain shared.
[ADR 0026](../../../../docs/decisions/0026-theatre-mobile-local-storage-sandbox.md) (proposed)
keeps its file-system-double decision for #16 download staging; its mock-database listening
double was removed in #287 because the real SQLite store runs in memory instead.

```text
:preview-sandbox --> :shared
:androidApp --> :shared (CantosTheatreApp over live catalog, account, Library and native player)
iosApp/Cantos --> built :shared framework
```

Shared main source and declared Gradle dependency guards are bounded static evidence, not
resolved release classpaths or DEX/resource inspection. The separate
`:shared:assertProductionSafeDependencies` Gradle task inspects resolved shared desktop
compile/runtime graphs and shared Android release compile/runtime graphs when Android is enabled;
main compilation, assemble and `desktopTest` depend on it. Its report is
`shared/build/reports/production-boundary/resolved-dependencies.txt`. Execution and scope are
recorded in [#122 evidence](../../../../docs/evidence/issue122-cmp-sandbox-isolation.md).
The production host proves both release graphs and its actual unshrunk APK/AAB exclude sandbox
code, resources and test dependencies; debug flags and R8 removal alone cannot establish that
boundary.

## Playback and reset invariants

- Play changes intent and emits a complete command. Only matching `EngineStarted` first confirms
  **simulating**, always with no-audio copy. No clock advances, media plays or timer runs.
- Effects contain identity, complete source, point and speed. Command acceptance is ordered and
  returns before acknowledgement; preparation races cannot wait on a discarded ready callback.
  A delayed start acknowledgement cannot restore an older speed.
- Source and generation jointly fence observations. Ready cannot roll running/paused/ended back;
  user pause wins, cold restore emits Prepare plus OfferResume and never Start.
- Rapid toggle and relative-seek intents resolve against the event loop's current state.
  Bookmark point capture follows preceding controls, then crosses to the independent listening owner.
- Reset increments a local epoch to discard pre-reset bookmark captures. Progress is blocked
  until an accepted restore barrier establishes its actual generation floor; older queued collector
  values cannot repopulate the reset snapshot. Controlled tests force both actor interleavings.
  Generations never wrap. A rejected restore does not establish a new progress baseline.
- Catalog source selection refuses missing/unsupported keys before navigation or engine commands.
  Opening a bookmark validates its source and point together; refusal cannot seek another episode.
  Bookmark projections/actions and marked-state comparison retain exact milliseconds; only clock
  labels use whole seconds, so distinct points within one second remain distinct.
  Unknown UI selections show recovery, never another work's substituted metadata.
- End position/engine end creates no completed-listen, rights or publication assertion. All
  stores are memory-only and disappear with process death.

## One shared corpus

The fixture-only commit `56bb39bcd769befd8385e7cb963b7ff4930f3c89` was explicitly approved and
cherry-picked as `1b8e56a980e095bc1d1346b95f0f975344082327`; no Web implementation is imported.
[Contract/data](../../../../packages/contracts/fixtures/theatre/playback/v1/README.md) is
`theatre-mock-playback@1`, vector_format 1. Its seven vectors contain 51 hand-authored steps.

- index SHA-256: `108503bdcceb36fe6351b1d320075d974615f7dc0ea560582bbe19c2da2b0485`
- schema SHA-256: `0c6130a831dd4486b07736888ed0f6df0f43d1faee68ce3a70fb5a9351c02000`

Gradle generates ignored common-test strings from exactly the index-listed bytes. Generation
checks every SHA-256 and rejects unlisted JSON; no language reducer generates expected outputs.
Tests compare the complete expected state, derived status/cause and ordered effects. The decoder
rejects unknown versions, fields, variants, containers, types, bounds and noninteger lexemes,
including `1000.0` and `-0`. Duplicate JSON keys retain the pinned Web parser's last-key behavior;
duplicate-key rejection is not a verified contract promise.

Source alignment used Web head `2990f3d0d4f10bdbb8c2c5a4548f657c2d95c988`; its authored Rust
tests were NOT_RUN in that executor and are not Kotlin/compiler evidence here. Kotlin/JVM,
Android runtime, iOS and Web results are distinct. The requester and Web owner confirmed the
boundary outside the corpus: already-requested Play remains a no-op after seek-to-end while
intent is Play; only an accepted new Play from paused/observed-ended at the end restarts at zero.
Both clients agree, and a separate Kotlin adversarial test covers the five non-ended phases
plus pause/restart without changing vector bytes. The shared README clarification is consumed
from its owner (`b4aa7e36c03c0c8e862fd5e038799a8bc33bad40` and
`7213fca7d043582350d45f02b277b0481a1807e1`, locally `aa9c8cb356d33a840ca0593883919c08c7865625`
and `65c206e0ffac406e3788747250032fc039d21532`); this adds no second corpus.

## Historical executed local evidence

This ledger and its captures describe the phase2 source named below, before #122 extraction.
They are retained without promoting their counts, APK hash, paths or renderer results to the
current module graph. Current extraction evidence is recorded [separately](../../../../docs/evidence/issue122-cmp-sandbox-isolation.md).

Source: checkpoint/cherry-pick above plus the architecture working diff. Installed tools remain
Rust 1.87.0, Kotlin 2.2.20, Compose 1.9.0, Material3 1.8.2, Gradle 8.14.3, JDK21, Android
SDK35/build-tools35.0.0, macOS15.6 arm64. No toolchain, TLS, credentials or permissions changed.

| Command / evidence | Result | Supporting output / limit |
| --- | --- | --- |
| `cargo xtask theatre-demo verify` | **PASS** | integrated109 tests, zero failure/error/skip; XML13:32:05–07Z; `/tmp/cantos-cmp-phase2-verify-integrated.log` |
| Canonical common tests on JVM | **PASS** | all7 vectors/51steps, full state/effects; 15 reducer/factory tests plus16 strict corpus/parser tests |
| Fake-port/application integration | **PASS** | 18 catalog/listening/navigation/preferences tests, six playback-controller tests, nine app-scope/reset/bookmark tests |
| UI projections/resources | **PASS** | 11 immutable projection/catalog tests and nine locale/resource tests |
| Actual app JVM rendering | **PASS** | ten root interaction tests, eight direct presentation tests and seven capture tests/eight PNGs |
| `cargo xtask theatre-demo android-build` | **PASS** | final integrated build:55tasks,13seconds;7 executed/48up-to-date; `/tmp/cantos-cmp-phase2-android-integrated.log` |
| Phase1 full preflight / verify rust / verify contracts | **PASS**, retained baseline | all15 preflight gates, including disposable PostgreSQL and9/9mutation; predates inherited PR62; no fresh verification claim for that backend |
| Phase2 pre-integration changed-only preflight | **PASS**, with7 explicit **NOT_RUN** gates | repository, rust-format and theatre-cmp-demo PASS; `/tmp/cantos-cmp-phase2-preflight.log`; integrated repeat uses the baseline below |
| Integrated uncommitted-merge preflight | **PASS**, with8 explicit **NOT_RUN** gates | inherited staged PR62 paths broadened selection; repository, rust-format, script-ir-reference, ui-preview, cmp-ui-preview, theatre-cmp-demo and9/9mutation PASS; `/tmp/cantos-cmp-phase2-preflight-integrated.log` |
| Final committed-merge preflight | **PASS**, with7 explicit **NOT_RUN** gates | repository (365text files,15skills,243Python tests), rust-format and theatre-cmp-demo PASS; `/tmp/cantos-cmp-phase2-preflight-committed.log` |
| `cargo xtask theatre-demo run` | **PASS**, bounded process observation | refactored root reached `:shared:run`; launcher/guard PID18235 and MainKt PID18251 shared PGID18235; Ctrl-C exit0, no recorded PID/group descendants remained |
| iOS framework / generated header / Swift compilation | **UNSUPPORTED** | exact installed Kotlin/Native2.2.20 unavailable; current2.2.0 rejected before provisioning |
| Android Studio/official CLI render | **UNSUPPORTED** | required current-host renderer unavailable; historicalPR43 images are separate |
| Android/iOS vector/runtime, OS Back/insets/text scaling, TalkBack/VoiceOver | **NOT_RUN** | no native runtime/device test executed |
| Live desktop OS-window interaction, motion and real audio | **NOT_RUN** | Mac locked; JVM composition input/captures are distinct evidence; no audio implementation |

Final ignored debug APK SHA-256:
`a50b35a7b25858502262f375c3a86f4412a30af9f5565e3d64c86b558c65c47d`.
This is build evidence, not an installation/run result.

The integrated phase2 preflight compares against concurrent remote head `b0869291078ec9100f49a5dd24d1eb60a9393a01`.
Its conservative shared-contract selection also includes backend/Studio checks. The CMP refactor
changes none of their inputs against that baseline; the requester explicitly asked to avoid
repeating unrelated DBSP/backend checks. PR62 remains its owner's verification responsibility.
The following prints `NOT_RUN (skipped by --skip)` for rust-lint, rust-tests, dbsp, wav-reference,
web-lint, web-build and postgres; their all15-gate phase1 baseline remains separate evidence.

```sh
cargo xtask preflight --changed-only --base b0869291078ec9100f49a5dd24d1eb60a9393a01 \
  --skip rust-lint --skip rust-tests --skip dbsp --skip wav-reference \
  --skip web-lint --skip web-build --skip postgres \
  --skip script-ir-reference --skip ui-preview --skip cmp-ui-preview --skip mutation
```

The last four explicit exclusions guard against unrelated merge-index broadening; they are not
selected in the final62-path diff. The final output has three PASS and seven NOT_RUN steps.

The actual run/interrupt observation is retained under ignored
`target/theatre-cmp-evidence/phase2-process.md`. Only task-owned identifiers were checked or
signalled. It establishes startup/cleanup, not OS-window readiness or input.

The first phase2 test compile failed on subtype-list assertion inference and was fixed; no
assertion was weakened. Independent reviews also fixed literal-brace resource substitution,
cancelled listening owners accepting late port replies, and the two reset interleavings above.
The final review found whole-second bookmark payload truncation; exact-point application and
Compose regressions now pass. The new Compose regression initially expected `00:30` rather than
the existing `0:30` clock label; its expectation was corrected without changing the formatter.
Fresh reports establish `example-tested`, `mock-integration-tested` with named mock ports,
JVM `interaction-tested`/`semantics-tested`; scoped source reviews now have no actionable findings.
The final independent precision review fingerprint covers nine files:
`a320e84f82d870cc03e0fb58346f0ad5582ad3ecd694ac58f1cf5f9f7d42c0a1`.
The integrated three-file shape/endonym/root review has no actionable findings, fingerprint
`a6b063ae0d029e081453a64e272c6bc959453c2ce00ac02ab6795b5287f2e5e0`;
it read the fresh109 XML and opened all eight regenerated images.
Remote CI is not checked, polled or used as handoff evidence.

## Opened visual evidence and limits

All eight PNGs and accompanying semantics were opened after the 105-test pass. The integrated
109-test regeneration is inspected independently after retaining the remote card-shape/endonym
fixes; the earlier shape images do not certify changed pixels.
Their historical output was `shared/build/reports/theatre-demo/`; current sandbox captures use
`preview-sandbox/build/reports/theatre-demo/`. Capture metadata's inspection-pending
flag is unchanged, and this record supplies the subsequent inspection. Shared qualified
components remain **0/29**; app evidence grants no two-renderer admission.

| Scenario | Pixels / observation | Limit |
| --- | --- | --- |
| light discovery | 390×844 and840×840; warm paper, purple tonal work groups, Vietnamese serif hierarchy; two columns at wide size; continue uses large card shape | lower narrow content scrolls; no native tablet/inset claim |
| dark full player | 390×844; complete episode title,12:48/28:42, transport/seek/bookmark/queue and explicit no-audio status | artwork placeholder and platform-font fallback; no playback clock/motion |
| dark catalog error | 390×844; readable explanation/retry and preserved mini-player point | retry behavior comes from the executed JVM test |
| saved library | 390×844; two saved works, selected tab and nav fit | memory-only; bookmarks/empty variants have test evidence |
| settings | 390×844; Tiếng Việt/English endonyms, theme, canonical five speed choices and wrapping timer/demo-state controls | lower reset control scrolls; timer schedules nothing |
| work detail at2× | 320×800, initial and scrolled episode3; title/description wrap and episode action remains reachable with large card shape | header/footer reduce body; mini title ellipsizes; scroll viewport can partially clip adjacent/current-marker content |

Final integrated capture SHA-256 (same ignored report directory):

| PNG | SHA-256 |
| --- | --- |
| `jvm-light-discovery-390x844.png` | `0ad5e061193323c2a9a89c74da7224a4fb7d66055e122fcdcc1f3206513fbd6f` |
| `jvm-light-discovery-840x840.png` | `39c904443357378484ece99ad62bdcfd3befa97a28be9265ddf923020a526b4c` |
| `jvm-dark-error-390x844.png` | `2159d066873ce4b83580cb43b3a4db034806878fc34ed8be23474e23d831c4bc` |
| `jvm-dark-player-390x844.png` | `f2854f6bd7f3a99b140da72ff9b17bbdf87a93809b496996aa2d109d3f3b8cf3` |
| `jvm-light-library-390x844.png` | `c2d59ec40f509ec8c190edfbf81684a3f7c098c99c98ff719fa663d6feda6188` |
| `jvm-light-settings-390x844.png` | `23334626dafa05993a01bff732c1593a4cab1ba025a16b6f6ac364c15d522781` |
| `jvm-light-work-text2-320x800.png` | `a6ee9726b7285321716d67319d1f2ee2d65fc3304e7837d0b1d7e8b9ee31caf1` |
| `jvm-light-work-text2-episodes-320x800.png` | `7bb98b568a7fa5ccc4f87dbb0fedfdb967fbfb12684ebd0a714b37202e981aac` |

Exact reference art/font fidelity, full contrast/focus-order audit, native text scaling,
screen-reader traversal and Android/iOS rendering are not established by these images.
The inspected Drive board/handoff provenance and unbundled media limits remain in the phase1 record.
Future listener work continues [050](../../../../docs/work-plan/050-cmp-native-listening.md) and
issues14/12/34 prerequisites; no issue or production acceptance is closed by this mock.
