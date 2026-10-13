# Kotlin and Compose conventions

> **Scope.** Handwritten Theatre Kotlin: domain types, pure transformations, state ownership,
> effects, coroutines and Compose contracts. Generated code follows its generator; its mapping
> into domain values still follows these rules.

These are Kotlin applications of the foundation's
[type barriers](../../cantos-engineering/references/types-as-proofs.md),
[functional core](../../cantos-engineering/references/functional-core.md) and
[immutability](../../cantos-engineering/references/immutability.md).
**MUST** is a review requirement; **SHOULD** permits a reasoned local alternative; **MAY** is
optional. Record an exception's scope, invariant, reason, compensating evidence and owner in the
PR. An exception never silently weakens a contract or changes production scope.

Enforcement today: Kotlin 2.2.20 compiles the isolated mock; compiler exhaustiveness/visibility
and existing tests cover specific claims. Import restrictions and broader rules are
review-enforced. No Detekt/ktlint, compile-negative harness or Kotlin property/mutation runner
is configured. See [testing](cmp-testing.md) and [dependency evaluation](cmp-tooling.md).
Examples are illustrative unless explicitly identified as current code.

## TYPE-01 — strong domain identities

| Field | Contract |
|---|---|
| Level / scope | MUST where confusing identities or units can cause a defect |
| Rule / rationale | Distinguish `WorkId`, `EpisodeId` and `PublicationId`; consider `PlaybackPosition`, `PlaybackDuration`, `PlaybackSpeed` and `Generation` when they carry a real invariant. Prefer `@JvmInline value class` where appropriate; common declarations can target Native despite the annotation name. |
| Good / counterexample | `fun open(id: EpisodeId)` vs. `fun open(id: String)` accepting a work key; `typealias EpisodeId = String` provides no barrier |
| Oracle / enforcement | Compiler rejects swapped identity types; review conversions and target/interop constraints; VER-01 |
| Exception | Raw DTO fields and diagnostic fixture keys may remain primitives until boundary mapping. No wrappers without a meaningful distinction or invariant. |

```kotlin
@JvmInline value class EpisodeId(val value: String)
@JvmInline value class PublicationId(val value: String)
fun openEpisode(id: EpisodeId) = id.value
// Forbidden: openEpisode(PublicationId("release-1"))
```

This proves identity separation only, not syntax or authorization. Validate from an owning contract.

## TYPE-02 — refined values and relations

| Field | Contract |
|---|---|
| Level / scope | MUST when consumers rely on an invariant |
| Rule / rationale | Choose the smallest value class, private class, validated aggregate, sealed ADT or evidence value. Validate related fields together: nonnegative position alone cannot prove `position <= source.duration`. Only a verifier constructs `VerifiedDownload`, bound to the exact bytes/checksum and account/publication facts checked. |
| Good / counterexample | Current `PlaybackSession` checks position against its source in `init` vs. independent positive position/duration wrappers assumed to prove their relation |
| Oracle / enforcement | Factory partitions, boundary/overflow and preservation tests, including decoding/restoration; VER-02 |
| Exception | Explicit raw requests may contain invalid values for transactional reducer rejection. No exception for a value advertised as verified. |

```kotlin
// Illustrative factory using the current mock's exact permille set.
sealed interface SpeedResult {
    data class Accepted(val speed: PlaybackSpeed) : SpeedResult
    data class Unsupported(val permille: Int) : SpeedResult
}
@JvmInline
value class PlaybackSpeed private constructor(val permille: Int) {
    companion object {
        fun of(raw: Int): SpeedResult = when (raw) {
            750, 1000, 1250, 1500, 2000 -> SpeedResult.Accepted(PlaybackSpeed(raw))
            else -> SpeedResult.Unsupported(raw)
        }
    }
}
// Counterexample: data class PlaybackSpeed(val factor: Double) accepts NaN and any speed.
```

The set belongs to the [mock corpus](../../../../packages/contracts/fixtures/theatre/playback/v1/README.md),
not a selected production policy. Current generation is Uint32-bounded with checked exhaustion,
never wraparound. Kotlin has no native dependent types or general refinement proofs; factories
establish runtime predicates, and visibility protects their provenance.

## TYPE-03 — construction and visibility

| Field | Contract |
|---|---|
| Level / scope | MUST for refined types/evidence; SHOULD use narrow visibility elsewhere |
| Rule / rationale | Prefer `private` details, `internal` module APIs, public APIs for actual consumers. Audit constructors, `copy()`, deserialization, API mapping, persistence restoration, defaults, fixture builders and unsafe/unchecked casts. Immutable does not mean valid. |
| Good / counterexample | Current `@ConsistentCopyVisibility data class PlaybackSource private constructor(...)` with validating `of` vs. private construction bypassed by public `copy(durationMs = -1)` |
| Oracle / enforcement | Compile attempts outside the allowed scope; inspect copy visibility on pinned Kotlin 2.2.20; test every runtime entry path |
| Exception | Narrow trusted restoration needs documented producer obligations and paired tests; it cannot forge authorization or verification evidence. |

[Kotlin's copy-visibility documentation](https://kotlinlang.org/api/core/kotlin-stdlib/kotlin/-consistent-copy-visibility/)
describes a phased migration: check the pinned compiler, not an approximate current-doc timeline.
The annotation controls visibility, **not validation**; allowed copies still need `init` checks
or invariant-preserving operations. A plain private-constructor class avoids generated copy
where simpler. Serialization into an evidence value must not bypass its factory.

## TYPE-04 — closed algebraic data types

| Field | Contract |
|---|---|
| Level / scope | MUST for closed event/state/result/error vocabularies |
| Rule / rationale | Use `sealed interface` / `sealed class`, `data object` singleton and `data class` payload cases; finite payload-free enums are appropriate too. Exhaustive `when` expressions have no catch-all `else`. New variants force decisions, UI projections and tests to be reconsidered. |
| Good / counterexample | `when (intent) { PlaybackIntent.Play -> ...; is PlaybackIntent.Pause -> ... }` vs. `else -> showPlaying()` hiding a new state |
| Oracle / enforcement | Compiler exhaustiveness plus review of reducers, labels, accessibility projections and generators |
| Exception | Map external open protocol values at the adapter, including an explicit `Unknown(raw)` when compatibility requires it. Do not invent domain variants to silence the compiler. |

## TYPE-05 — typed expected failures

| Field | Contract |
|---|---|
| Level / scope | MUST for expected business failures in reducers/controllers/adapters |
| Rule / rationale | Use existing sealed outcomes, typed `Either<Error, Value>`, justified Arrow `Raise` or command rejection. No nullable error sentinel, `!!`, unchecked cast or throwable-driven domain flow. `kotlin.Result` alone erases the domain error to `Throwable`. |
| Good / counterexample | `PlaybackTransition(state, listOf(PlaybackEffect.Rejected(PlaybackRejection.OutOfBounds)))` vs. `throw IllegalArgumentException("bad seek")` for caller input |
| Oracle / enforcement | Assert unchanged complete state and exact rejection/effects; boundary error/cancellation tests; VER-02/05/06 |
| Exception | Programmer defects/unexpected platform failures may throw; map at the boundary. Fixtures may fail fast with `requireNotNull`. Legacy nullable factories are refusal-only APIs, not nullable error protocols for new APIs. |

Preserve `PlaybackTransition(nextState, effects)` and transactional rejection. Existing
`PlaybackSource.of`/`PlaybackIdentity.of` return nullable validated values; improving them needs
targeted consumer/corpus review, not an Arrow migration. No broad `catch(Exception)` throughout
application logic. A final platform containment boundary may catch unexpected failures only
after rethrowing `CancellationException` and mapping to a safe typed outcome. The current mock
controller maps to `MockEngineUnavailable`; that establishes no production recovery policy.
Never display raw exception messages or signed URLs.

## FP-01 — pure functional core

| Field | Contract |
|---|---|
| Level / scope | MUST for durable domain decisions |
| Rule / rationale | `State × Event → Transition<NextState, Effects>` is deterministic. No Compose, Android/iOS frameworks, network, database, clocks, randomness, global mutable state, media calls, logging or blocking work. Supply facts as inputs; application code interprets effects. |
| Good / counterexample | Current `reducePlayback(state, event): PlaybackTransition` vs. `fun reduce(...) { engine.start(...); Clock.System.now() }` |
| Oracle / enforcement | Import/signature review and equal-input/exact-transition tests; VER-01/02 |
| Exception | Thin CRUD/adapters need no artificial reducer. Encapsulated local mutation is pure if it escapes only as a fresh value. |

## FP-02 — immutable state

| Field | Contract |
|---|---|
| Level / scope | MUST for reducer inputs/outputs, exposed UI state and accepted records |
| Rule / rationale | Prefer `val` and invariant-preserving replacement. Read-only `List` does not guarantee deep immutability: snapshot inputs, retain no writable aliases, use immutable elements. Evaluate persistent collections for concrete correctness/structural-sharing benefits. |
| Good / counterexample | `state.copy(episodes = input.toList())` with immutable elements/no writable alias vs. `State(inputMutableList)` then `inputMutableList.clear()` |
| Oracle / enforcement | Mutate original inputs after construction and assert state unchanged; audit nested elements/copies/casts; VER-02 |
| Exception | Local builders returning detached snapshots are fine. Persistent collections need [target/compiler checks](cmp-tooling.md) and do not make mutable elements immutable. |

The current reducer emits fresh `emptyList()`/`listOf()` effect snapshots; no library is needed
just to replace these small lists.

## FP-03 — named functional composition

| Field | Contract |
|---|---|
| Level / scope | SHOULD for transformations/validation composition |
| Rule / rationale | Small named functions preserve invariant → implementation → test traceability. Use `map`, `fold`, `flatMap`, `zip` when clearer; direct `when` is often better. |
| Good / counterexample | `episodes.map(::toRow)` or `outcome.fold(::showFailure, ::showValue)` vs. nested `let/run/apply` concealing rejection or a custom monad for one branch |
| Oracle / enforcement | Review can locate decisions and independent oracles without mentally executing a chain |
| Exception | Prefer a loop/branch for clearer order or early rejection. No mandatory optics, higher-kinded abstraction or generic MVI/effect framework. |

## FP-04 — separate publication of state from effect execution

| Field | Contract |
|---|---|
| Level / scope | MUST for state transforms/asynchronous effects |
| Rule / rationale | `StateFlow.update` may retry: no network/storage/media command, ID generation, logging or enqueue inside. Complex state machines should use one serialized event owner with ordered effect interpretation. Every effect needs owner, cancellation policy, identity and stale-result handling. |
| Good / counterexample | Pure `_state.update { it.copy(speed = speed) }`; current event loop publishes state then queues effects to a separate worker vs. `_state.update { engine.start(command); it }` |
| Oracle / enforcement | Contention/deterministic controller tests assert ordered commands; delayed acknowledgements cannot block later Pause/Seek; VER-05 |
| Exception | None for effects in retryable lambdas. Single writers may assign `.value = next`. Simple UI state needs no event-loop framework. |

Current playback has **two loops**, events/effects, plus an observation collector. Commands return
without waiting for readiness; fenced observations acknowledge them separately. Reduction
blocked on acknowledgement can prevent a later command from progressing. Atomic publication
does not make external delivery exactly once.

## KT-API — composable contract

| Field | Contract |
|---|---|
| Level / scope | SHOULD for layout composables; MUST preserve geometry/focus/semantics on changes |
| Rule / rationale | Required parameters first; `modifier: Modifier = Modifier` first optional parameter, applied once at root. Values in, typed `onX` intents out; named slots. |
| Good / counterexample | `Row(modifier = modifier.fillMaxWidth())` vs. ignoring modifier or passing a controller into a row |
| Oracle / enforcement | Call-site review and production-composable semantics/bounds tests; lint remains a candidate |
| Exception | Private helpers may follow their parent's explicit layout contract |

## KT-STATE — one mutable authority, projections for the rest

| Field | Contract |
|---|---|
| Level / scope | MUST for application/screen state; SHOULD for reusable components |
| Rule / rationale | One writable owner at its lifetime; expose `StateFlow` via `asStateFlow()`. Derive cheap facts instead of mirroring mutable flows. Coexisting facts remain independent: cached rows, refresh and a refresh failure may coexist. |
| Good / counterexample | `val canResume get() = state.session != null` vs. mirrored `MutableStateFlow(canResume)`; `CatalogState(lastKnownInventory, refresh)` vs. error erasing usable rows |
| Oracle / enforcement | Ownership inventory, observable-combination and reset/account-switch tests |
| Exception | Menu/scroll/focus/pressed/text-field state may remain local; no reducer for trivial booleans |

## KT-VALUES — immutable values, honestly

Compatibility anchor: [FP-02](#fp-02--immutable-state) owns this rule; construction is
[TYPE-02/03](#type-02--refined-values-and-relations). No separate copy-based validity rule.

## KT-UPDATE — retry-safe state transforms

Compatibility anchor: [FP-04](#fp-04--separate-publication-of-state-from-effect-execution) owns this rule.

## KT-LIFETIME — effects have an owner, keys and cleanup

| Field | Contract |
|---|---|
| Level / scope | MUST for requests/listeners/timers/subscriptions spanning a lifetime |
| Rule / rationale | Name process/account/route/composition owner, restart keys and cleanup. Results carry account/request/source/generation identity checked on arrival. Reset/logout/account switch fences outstanding work; cancellation cannot undo external writes. |
| Good / counterexample | `LaunchedEffect(episodeId)` plus current-identity check and `DisposableEffect` cleanup vs. listener registration in a composable body |
| Oracle / enforcement | Delayed-response, disposal, rapid-input/reset tests; OS callbacks need native evidence too |
| Exception | Transient focus/snackbar effects need no persistence; important operations need durable acknowledgement at their owner |

Use `rememberUpdatedState` for the latest callback in a long-lived effect. Lifecycle-aware
collection and Swift coroutine interop require pinned target checks.

## KT-CLOSED — sealed types and exhaustive `when`

Compatibility anchor: [TYPE-04](#type-04--closed-algebraic-data-types) owns this rule.

## KT-ERRORS — expected failures are values

Compatibility anchor: [TYPE-05](#type-05--typed-expected-failures) owns this rule.

## KT-CONCURRENCY — structured, injected, controllable

| Field | Contract |
|---|---|
| Level / scope | MUST for application/adapter coroutines |
| Rule / rationale | Every coroutine has a named lifetime owner; cancellation propagates. No `GlobalScope`, main-thread `runBlocking` or swallowed cancellation. Inject dispatchers/time. Monotonic time drives waits; wall-clock time is contextual. Supervision isolates siblings, not failure handling. |
| Good / counterexample | `scope.launch(CoroutineName("theatre-playback-events")) { ... }` owned by `AppContainer` vs. `GlobalScope.launch { ... }` surviving logout |
| Oracle / enforcement | `runTest`, injected scheduler, cancel/close/reset/late-callback assertions; single-thread virtual tests alone do not establish multi-writer thread safety |
| Exception | Composition roots choose platform jobs/dispatchers; blocking platform calls use an appropriate injected dispatcher, never pure functions |

Rapid inputs preserve specified effect ordering/idempotence. See
[architecture guarantees](cmp-architecture.md#concurrency-guarantees).

## KT-SECRETS — values that must not print

| Field | Contract |
|---|---|
| Level / scope | MUST for tokens/signed URLs/cookies/credentials |
| Rule / rationale | Generated `data class.toString()` exposes fields; redact, exclude secrets from navigation/saved state/analytics, log IDs and reason codes. |
| Good / counterexample | `override fun toString() = "SignedMediaUrl(redacted)"` vs. `println("loading $url")` |
| Oracle / enforcement | Sentinel-secret tests over logs/saved state; review generated representations |
| Exception | None in shipped output; debug with fake credentials |

## KT-STYLE — readable, mechanically formatted

| Field | Contract |
|---|---|
| Level / scope | SHOULD for touched handwritten Kotlin |
| Rule / rationale | Official Kotlin style, explicit imports, trailing commas, named same-type/boolean arguments, readable wrapped calls. |
| Good / counterexample | `EpisodeRow(episode = row, onPlay = onPlay)` vs. `EpisodeRow(row, {}, {}, true)` |
| Oracle / enforcement | Manual review today; [tooling evaluation](cmp-tooling.md) precedes any formatter/linter gate |
| Exception | No mass reformat in a behavior change; report unrelated formatter touches |

## KT-STABILITY — measure before annotating

| Field | Contract |
|---|---|
| Level / scope | MUST for `@Immutable`/`@Stable` promises; SHOULD measure performance changes |
| Rule / rationale | Annotations promise stability to Compose; they cannot make mutable graphs immutable or prove speedups. Check pinned compiler behavior. |
| Good / counterexample | `@Immutable data class Title(val text: String)` vs. `@Immutable data class Rows(val items: MutableList<Row>)` |
| Oracle / enforcement | Audit complete object graph; device/build-specific before/after measurements for performance claims |
| Exception | Omit annotation when the promise is unestablished; no knowingly false promises |
