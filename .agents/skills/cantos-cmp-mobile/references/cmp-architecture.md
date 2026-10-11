# CMP architecture and ownership

> **Scope.** Where Cantos Theatre Kotlin code lives, which way dependencies point, how platform
> adapters are isolated and who owns each lifetime. Use before creating a feature, package,
> Gradle module, port, `expect`/`actual` declaration or state holder, and before splitting or
> moving existing CMP code.

The product boundaries are owned by
[mobile.md § Boundaries](../../../../docs/architecture/mobile.md#boundaries) and the
[architecture overview](../../../../docs/architecture/overview.md). The general dependency rule
is the foundation's [decoupling reference](../../cantos-engineering/references/decoupling.md).
This reference maps both onto Kotlin Multiplatform; it does not add layers. Everything below is
a **proposed** layout: no mobile code exists yet. Recheck it against the real tree before relying
on any path.

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

An arrow means "may depend on". Data flows both ways; imports flow one way. A screen never
imports an adapter, a reducer never imports Compose, and nothing in `commonMain` imports
`android.*`, `platform.AVFoundation.*`, a Ktor engine or a SQL driver.

## Proposed packages

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
default switch. No CMP client or Kotlin binding exists yet.

`playback/`, `downloads/`, `progress/`, `catalog/` and `session/` are the **shared application
layer** from mobile.md. Feature packages consume them; they never import each other's `state/`.
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
// Illustrative, proposed — not existing code.
interface MediaEngine {
    val events: Flow<EngineEvent>
    fun load(source: PlayableSource, startAt: PlaybackPosition)
    fun play()
    fun pause()
    fun seekTo(position: PlaybackPosition)
    fun setSpeed(speed: PlaybackSpeed)
    fun release()
}
```

| Mechanism | Use when | Avoid when |
|---|---|---|
| interface in `commonMain` + implementation injected at the composition root | the adapter is stateful, has a test fake, or is implemented in Swift | never; this is the default |
| `expect`/`actual` function or class | a small, stateless platform factory or value (`currentMotionPreference()`, `platformName`) | the declaration needs a fake in tests or holds state; `actual` cannot be swapped per test |
| Swift implementation of a Kotlin interface passed in at startup | the API is awkward from Kotlin/Native (KVO, delegate-heavy AVFoundation, background `URLSession`) | the Kotlin surface exposes `suspend` or `Flow` to Swift without a recorded interop decision |

Shape iOS-facing ports as plain callbacks or listener interfaces; wrap them into `Flow` with
`callbackFlow` on the Kotlin side. Exposing coroutines to Swift needs an interop tool (SKIE and
KMP-NativeCoroutines are candidates) chosen in an ADR.

A port exists only with two real implementations — the platform adapter and a deterministic test
fake count. A port added only so a test can count calls hides a rule that belongs in a reducer.

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
Never upgrade the toolchain as a side effect of a feature.

## Review questions

1. Which object is the one mutable authority for each fact this change touches, and what is its
   lifetime?
2. Does any `commonMain` import point at a platform, engine, driver or vendor type?
3. Can navigation create a second engine, second download executor or second outbox flusher?
4. Is any server policy now decided in Kotlin?
5. Is the API type generated, and is its mapping into refined values tested with an invalid
   payload?
