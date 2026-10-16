# CMP architecture and ownership

> **Scope.** Where Cantos Theatre Kotlin code lives, which way dependencies point, how platform
> adapters are isolated and who owns each lifetime. Use before creating a feature, package,
> Gradle module, port, `expect`/`actual` declaration or state holder, and before splitting or
> moving existing CMP code.

The product boundaries are owned by
[mobile.md § Boundaries](../../../../docs/architecture/mobile.md#boundaries) and the
[architecture overview](../../../../docs/architecture/overview.md). The general dependency rule
is the foundation's [decoupling reference](../../cantos-engineering/references/decoupling.md).
This reference maps both onto Kotlin Multiplatform. The isolated mock has real code and Gradle
hosts; native production listening remains planned. Inspect the
[current structure](../../../../apps/cantos-theatre/mobile/README.md#structure) before choosing paths.

## ARC-01 — inward dependencies and domain boundaries

**MUST**, for handwritten application code: keep the conceptual layers below separate in
responsibility, without inventing modules/folders to match a diagram. Domain owns business
invariants; it imports neither application, presentation, infrastructure nor platform frameworks.
Application depends on domain and required ports; adapters implement those ports; presentation
receives values and emits typed intents; the composition root wires implementations/lifetimes.
Feature packages cannot manipulate another feature's internal state. No circular imports.

| Layer | Current `com.cantos.theatre` package / responsibility |
|---|---|
| Domain | shared `domain/playback`: `PlaybackState`, `PlaybackEvent`, `PlaybackEffect`, `reducePlayback`; `domain/publication` and `domain/listening` hold catalog/listening values and rules; synthetic corpus belongs to the sandbox |
| Application | `application/{playback,catalog,listening,preferences}`: independent authorities, event processing and effect interpretation |
| Ports | `ports/{MediaEngine,CatalogPort,ProgressPort}`: required capabilities, no vendor implementations |
| Adapters / infrastructure | sandbox `adapters/mock` and `adapters/mock/sandbox/{database,sessions}`: mock acknowledgements, synthetic fixtures and memory storage; shared HTTP/identity adapters remain separate; native audio/durable storage is absent |
| Presentation | shared `features/shell/presentation/CantosPresentation.kt` renders neutral immutable models/intents from the `presentation/model` leaf consumed by shell/auth/Membership; sandbox `app/CantosDemoApp.kt` and `DemoShellPresentation.kt` bind diagnostic projections/intents/slots to that same UI |
| Composition root | shared `app/AppContainer.kt` requires injected owners/ports/start state; sandbox `mockAppContainer` selects mocks explicitly; Android `CantosDemoApplication`, sandbox iOS `MainViewController`/SwiftUI owner and desktop host supply demo lifetimes |

Good Kotlin: `fun reducePlayback(state: PlaybackState, event: PlaybackEvent): PlaybackTransition`
uses plain domain values. Counterexample: `domain/playback` importing `android.media.*`,
`Composable` or `application.PlaybackController`. The `repository` gate
(`check_theatre_architecture`) rejects import, fully qualified-name and package/directory
violations for `domain`, `ports`, `application`, `features` and `demo`; compile the touched
targets as well. All shared main sources, including `app`/`adapters`, resources and declared
Gradle dependency/source routing are additionally checked for sandbox re-entry. The static
guard traverses supported literal project dependency declarations; it is bounded lexical
parsing, without overload resolution or resolved Gradle/artifact proof. Its actual scope and
negative fixtures are recorded in [#122 evidence](../../../../docs/evidence/issue122-cmp-sandbox-isolation.md).
Exception: framework code belongs in presentation/adapter
source, never in domain; a small feature may colocate declarations while preserving import
direction. DDD boundaries follow capability and invariant owners, not a mandatory class hierarchy.

## ARC-02 — explicit wire, storage and domain mapping

**MUST**, at external boundaries: keep API DTOs, generated wire types, persistence records and
domain values distinct, with validated conversion. Good: `fun RawCheckpoint.toSnapshot():
RestoreOutcome` validates source/position/speed together. Counterexample:
`val session = json.decodeFromString<VerifiedDownload>(raw)` forging verification evidence.
Verify invalid/unknown fields, schema versions, restoration and identity mismatches with boundary
tests. Exception: contract-approved raw inputs may remain unaccepted values until the reducer
rejects them; do not expose them as settled facts. No invented endpoints, DTO fields or auth
policy; the listener API is still reserved and Studio stays Web-only.

## Dependency direction

```text
Android Activity / MediaSessionService host        iOS app delegate / SwiftUI host
                     ↘                                ↙
              composition root (app/): builds adapters, owns process + account lifetimes
                                     ↓
            feature route: collects state, binds effects, maps events to intents
                                     ↓
            feature screen: read-only values + onX callbacks → sections, rows, dialogs

UI intent → state holder / controller → pure reducer → effects as data
          → narrow port → platform adapter or generated HTTP client → OS / listener API
OS or HTTP event → adapter → typed event → reducer → new immutable state → UI
```

An arrow means "may depend on". Data flows both ways; imports flow inward. A screen never
imports an adapter, a reducer never imports Compose. `commonMain` is portable: no `android.*`
or `platform.AVFoundation.*`. A multiplatform HTTP/storage adapter may live there when justified,
but domain/application signatures never expose its engine/driver/DTO types. Native implementations
remain in `androidMain` / `iosMain` or Swift.

## Current layout and future capabilities

The implementation keeps ARC-01's production-safe UI/domain/application/ports in `:shared` and
mock adapters, fixtures, previews, diagnostic composition and workbench in `:preview-sandbox`.
The sandbox depends on shared; shared has no reverse dependency or implicit mock fallback.
`androidApp` and the desktop/iOS hosts are demo hosts. The production host `androidProductionApp`
consumes shared plus production adapters only (none exist yet, so it renders an explicit
unavailable state) and establishes release graph and actual APK/AAB exclusion independently
through `cargo xtask theatre-host android-release`. Build/verify integrate
`:shared:assertProductionSafeDependencies` for resolved shared desktop and opt-in Android
release compile/runtime graphs; its report is `shared/build/reports/production-boundary/resolved-dependencies.txt`.
That library guard is separate from the lexical source guard and production artifact acceptance;
record actual runs through the [testing reference](cmp-testing.md#discover-before-executing).
The following feature/capability sketch is **future guidance only**; keep actual paths authoritative and
add a folder/module only for a concrete consumer:

Start with the fewest Gradle modules the chosen CMP template needs (typically one shared KMP
module plus the Android and iOS hosts). Split a module only for a named reason: the generated API
client regenerated in isolation and excluded from formatting, or a build target that must not
see a dependency. Inside the shared module, organize by feature and capability:

```text
<root package>/
  app/              composition root, navigation graph, process and account-session lifetimes
  features/
    discovery/      ui/ state/      works, episodes, availability
    library/        ui/ state/      in progress, played, downloaded
    player/         ui/ state/      mini and full player; attach to playback/
    downloads/      ui/ state/      download list, storage, retry/remove
    settings/       ui/ state/      autoplay, metered network, account
  playback/         controller, pure reducer, MediaEngine port, checkpoint codec
  downloads/        manifest model, lifecycle reducer, DownloadExecutor port, verifier
  progress/         outbox model, sync scheduler reducer, conflict projection
  catalog/          catalog and library access through the generated client
  session/          account scope, auth/session state, CredentialStore port
  platform/         expect/actual factories and signals: connectivity, lifecycle, motion setting
  design/           CantosTheme, generated tokens, Cantos component wrappers
  localization/     typed string resources
  api/              generated client and DTOs — never hand-edited
```

`api/` speaks JSON. A MessagePack representation is added only through the foundation's
[wire codec procedure](../../cantos-engineering/references/wire-codec.md#5-extending-to-theatre-and-cmp):
decode by response `Content-Type`, named maps, the committed goldens in `commonTest` and a JSON
default switch. No production listener API client exists yet. Generated Kotlin test strings
embed the reviewed mock corpus; they are neither HTTP bindings nor generated expected results.

In that future sketch, `playback/`, `downloads/`, `progress/`, `catalog/` and `session/` are
capabilities of the **shared application layer** from mobile.md, each retaining a pure domain core.
Feature packages consume them; they never import each other's `state/`.
Features communicate through the application layer or through navigation arguments that carry
IDs only (`EpisodeId`, `PublicationId`), so a route can be rebuilt after process death.

Do not create empty folders, a module per feature or widget, a `BaseScreen` with flags, or
`common/`, `utils/`, `helpers/`, `models/` buckets; they become dependency magnets without a
reason to change. A small feature may keep its route, screen and state in two files.

## Shared application layer: owners and lifetimes

| Capability | Lifetime owner | Holds | Decides | Must not |
|---|---|---|---|---|
| playback | process (one instance) | active publication, user intent, engine status, interruption, position, speed, sleep timer | transitions per [playback semantics](../../cantos-listening/references/playback-semantics.md) | be created per screen or per navigation |
| downloads | process | manifest rows, transfer state, verification outcome | lifecycle transitions, cleanup | treat a signed URL as file identity |
| progress | account session, persisted | outbox operations, last observed server revision, conflict candidate | when to flush, what to show on conflict | decide which device wins; the server does ([progress sync](../../cantos-listening/references/progress-sync.md)) |
| catalog | account session | page cursors, cached episode summaries | nothing durable | mirror server authorization |
| session | process, keyed by account | account scope, auth state | when to refresh credentials | hold credentials outside the secure store |

Each row has **one** mutable authority. Everything else is a projection: `isDownloaded`,
`canResume` and the mini-player's visibility are computed from the owner's state, never stored
in a second flow kept in sync by an effect.

## Platform adapters behind ports

mobile.md names the platform seams: media engine, secure credential store, durable local
storage, network and lifecycle signals, download execution. Each gets a narrow port in
`commonMain`, named for the capability Cantos needs, never for the vendor:

```kotlin
// Current mock seam (see ports/MediaEngine.kt); native implementations remain future work.
interface MediaEngine {
    val events: Flow<PlaybackObservation>
    suspend fun prepare(command: PlaybackEngineCommand)
    suspend fun start(command: PlaybackEngineCommand)
    suspend fun pause(command: PlaybackEngineCommand)
    suspend fun seek(command: PlaybackEngineCommand)
    suspend fun setSpeed(command: PlaybackEngineCommand)
    fun close()
}
```

| Mechanism | Use when | Avoid when |
|---|---|---|
| interface in `commonMain` + implementation injected at the composition root | the adapter is stateful, has a test fake, or is implemented in Swift | never; this is the default |
| `expect`/`actual` function or class | a small, stateless platform factory or value (`currentMotionPreference()`, `platformName`) | the declaration needs a fake in tests or holds state; `actual` cannot be swapped per test |
| Swift implementation of a Kotlin interface passed in at startup | the API is awkward from Kotlin/Native (KVO, delegate-heavy AVFoundation, background `URLSession`) | the Kotlin surface exposes `suspend` or `Flow` to Swift without a recorded interop decision |

Shape iOS-facing ports as plain callbacks or listener interfaces; wrap them into `Flow` with
`callbackFlow` on the Kotlin side. Exposing coroutines to Swift needs an explicit interop/
cancellation decision; SKIE and KMP-NativeCoroutines are candidates only if a concrete consumer
needs them, selected in an ADR rather than required for every port.

A port needs a real external capability and consumer; the current mock plus scripted test
implementations exercise its contract while native adapters are pending. Do not claim a production
adapter exists because an interface exists. Avoid interfaces solely to count internal calls.

## SOLID for functional architecture

Each ID below is **MUST** when its responsibility/boundary exists. Composition is preferred to
inheritance; none mandates OOP, a framework, a module per use case or interfaces for pure functions.
The oracle column is a review obligation today, not a configured static gate.

| ID / rationale | Positive Kotlin | Counterexample | Verification | Legitimate exception |
|---|---|---|---|---|
| SOLID-S: one coherent reason to change | `PlaybackController(scope, engine)` owns playback; `CatalogController` owns catalog | `TheatreManager` owns catalog, downloads, progress, navigation and player | Map each writable fact and lifetime to one owner; tests target the capability | Composition root wires several owners, without absorbing their rules |
| SOLID-O: extend appropriate policies/ports through composition | `MediaEngine` gains an adapter; `when(event)` stays exhaustive | `else -> Unit` to avoid editing decisions when a new event arrives | Add variant and inspect compiler decisions; preserve port contract | Closed business ADTs deliberately require edits; no catch-all for OCP |
| SOLID-L: substitutable behavior, not interface shape | `engineContract { scriptedEngine() }` and later `engineContract { nativeEngine() }` assert commands return before acknowledgements and callbacks retain identity | `override suspend fun start(...) { awaitReadyForever() }` blocks later Pause | Shared adapter contract tests for ordering, identity, failure/cancellation and cleanup | Platform extras may differ behind separate optional capabilities; required guarantees cannot weaken |
| SOLID-I: narrow capability ports | `class CatalogController(port: CatalogPort)` | `interface EverythingRepository { fun catalog(); fun play(); fun billing() }` | Review actual caller operations and fake size; remove unrelated dependencies | Coherent related operations can share one port |
| SOLID-D: policies depend inward | `PlaybackController(scope, engine: MediaEngine)` with root injection | `PlaybackController` constructs `ExoPlayer` or imports a vendor SDK | Import/root review plus fake-port tests; compile relevant source sets | Pure functions need no interface; platform roots may name concrete types |

`engineContract` is an illustrative shared-test shape, not an existing test helper. Fake/native/
production implementations should share it when present; device-specific behavior still needs
device tests. A fake alone cannot establish substitutability of an unimplemented native adapter.

## Concurrency guarantees

Apply [FP-04, KT-STATE, KT-LIFETIME and KT-CONCURRENCY](kotlin-conventions.md) rather than a
second set of ownership rules. Current playback serializes events and publishes immutable state;
a **separate ordered effect worker** issues self-contained commands, and observations arrive
through another collector. It never waits for engine readiness in the reducer/event owner.

| Guarantee | What establishes it | What it does not establish |
|---|---|---|
| Thread safety | Confinement, synchronization or an evaluated concurrency algorithm | Correct business ordering or durability |
| Atomic state publication | `MutableStateFlow.value`/pure retryable `update` | Atomicity with a network/media/storage call |
| Serial effect execution | One ordered consumer of accepted effect batches | OS acknowledgement order, exactly-once delivery or multi-process durability |
| Exactly-once delivery | Requires an explicit end-to-end protocol; do not infer it from a channel | An in-memory loop cannot survive crash/replay as a delivery guarantee |
| Durable idempotency | Stable operation identity, persisted receipt and guarded acceptance at the external authority | Never guaranteed by debounce, a generation number or a fake's command list |

Generation/source fencing rejects stale observations; it does not cancel already-issued external
commands. Test rapid inputs, reset barriers, close/cancellation, delayed callbacks and repeated
commands separately. Logout/account-switch handling remains a future boundary requiring the same
stale-result discipline; the memory-only mock has no authenticated session.

## One media session owner

[mobile.md](../../../../docs/architecture/mobile.md#boundaries) requires a single media session
that screens attach to. In Kotlin terms:

- The composition root creates exactly one `PlaybackController` for the process and passes it
  (or a narrow read/intent view of it) down. Screens collect its `StateFlow` and send intents.
- The platform host owns the engine's OS lifetime: on Android a media session service keeps
  playback alive in the background; on iOS the app-level audio session and remote command
  handlers are registered once. The controller attaches to that host; the host does not create a
  second controller.
- The mini-player and full player are two views of the same state. Expanding the full player is
  navigation, not a new session.

```kotlin
// Counterexample — a new engine per navigation; audio doubles or stops on back.
@Composable
fun FullPlayerRoute(episodeId: EpisodeId) {
    val engine = remember { createMediaEngine() }
    // ...
}
```

The oracle is a test that navigates library → full player → back → full player against a
counting fake engine factory and asserts exactly one `load` and one engine instance, plus a
device run that shows one notification and one audio stream.

## What never enters the app

- **WebView audio.** Playback is native; a WebView player fails background, lock-screen and
  interruption requirements and is forbidden by [AGENTS.md](../../../../AGENTS.md).
- **Rust FFI** for business code, without an immediate, recorded need
  ([050 non-goals](../../../../docs/work-plan/050-cmp-native-listening.md#non-goals)). Rust
  rules reach mobile as HTTP behavior and shared contract vectors.
- **Handwritten DTOs that diverge from the contract.** Generate the Kotlin client and models from
  the listener contract owned by [listener-api.md](../../cantos-listening/references/listener-api.md)
  and [http-api-boundary.md § Contracts](../../cantos-engineering/references/http-api-boundary.md#contracts).
  Generated files name their source and generation command. OpenAPI Generator's Kotlin
  multiplatform output and Ktor client engines are candidates for an ADR.
- **Generated DTOs as UI state.** Map them once, in the capability adapter, into refined values
  (`EpisodeId`, `PlaybackPosition`, sealed availability). That is refinement at the edge, not a
  divergent copy; route the mapping through validation as in
  [boundary-hardening.md](../../cantos-engineering/references/boundary-hardening.md).
- **Server policy re-implemented in Kotlin.** Entitlement, offline policy, progress acceptance
  and authorization are server decisions. Kotlin keeps the local candidate, shows the outcome and
  submits choices; it never computes "who wins" on its own.

## Decomposition record

Splitting a god screen or controller, regrouping packages, or extracting a state holder starts
with a record, kept in the PR description:

```text
Current owners:   UI | presentation state | port | adapter | platform host | server policy
Proposed owners:  same columns
Dependency graph: before → after (import edges, not data flow)
Lifetime graph:   which object lives for process / account / route / composition; effect keys
                  and cleanup for each subscription, request and platform callback
Symbol map:       old file/symbol/source set → new file/symbol/source set, with actual callers
Continuity:       stable list keys, remember/rememberSaveable owners, scroll and focus, request
                  and account generations, callback ordering, single engine instance
Evidence:         selected tests per affected source set, with pre/post executed counts
```

A folder move is not a decomposition, and a shorter file is not progress if one controller still
owns every concern. Before moving a top-level declaration, check Kotlin/JVM file-facade names and
Swift-visible entrypoints (the iOS view-controller factory) so Swift and Android callers still
resolve. Mirror moved packages in `commonTest` and platform test source sets.

| Anti-pattern | Why it fails | Instead |
|---|---|---|
| god `TheatreViewModel` holding catalog, player, downloads and settings | every change touches everything; lifetimes conflict | one owner per capability at its lifetime |
| controller per button, use case per getter | ceremony with no rule | a pure function or a direct call |
| module per widget | build cost without independent consumers | packages inside the shared module |
| feature `state/` imported by another feature | hidden coupling, broken restore | application layer or ID-only navigation arguments |

## Adding a dependency

Before adding a KMP library: check the pinned artifact's published targets (Android, iOS device,
iOS simulator, JVM test host), its Kotlin and CMP compatibility, licence and maintenance, and
whether an existing dependency already covers the need. Record the choice in an ADR when it is
one of the decisions in [mobile.md](../../../../docs/architecture/mobile.md#implementation-decisions-to-record).
Never upgrade the toolchain as a side effect of a feature. Read the
[candidate evaluation](cmp-tooling.md) before adding typed-error, validation, collection or
verification libraries; it records the pinned baseline and unresolved compatibility probes.

## Reviewer checklist

1. Which object is the one mutable authority for each fact this change touches, and what is its
   lifetime?
2. Does any `commonMain` import point at a platform, engine, driver or vendor type?
3. Can navigation create a second engine, second download executor or second outbox flusher?
4. Is any server policy now decided in Kotlin?
5. Is the API type generated, and is its mapping into refined values tested with an invalid
   payload?
6. Do TYPE-01–05 close construction/copy/decoding bypasses without inventing authorization?
7. Do FP-01–04 keep decisions pure, values immutable and retryable transforms effect-free?
8. Do SOLID-S/O/L/I/D preserve cohesive owners, exhaustive ADTs and substitutable narrow ports?
9. Are cancellation, rapid commands and stale observations tested with reproducible schedules?
10. Does VER-01–08 evidence name actual targets, bounds/seeds, complete ordered effects and gaps?
