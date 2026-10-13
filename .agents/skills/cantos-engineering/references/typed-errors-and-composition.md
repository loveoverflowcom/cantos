# Typed errors and functional composition

> **Scope.** Canonical error policy for Rust domain/application code, Leptos and Kotlin CMP.
> Use when designing a fallible factory, port, command, asynchronous flow or error projection.
> Platform references apply this policy; they do not define a competing taxonomy.

Compose this with [type barriers](types-as-proofs.md), [boundary hardening](boundary-hardening.md)
and the [functional core](functional-core.md). Those references own construction, refinement
provenance and atomic rejection. This reference owns error meaning and propagation. These are
working rules, not a claim that every existing Cantos boundary already implements them.

## ERR-01 — classify by meaning and owner

| Class | Meaning / example | Owner and representation | Recovery |
|---|---|---|---|
| Expected domain error | a requested transition is refused: stale revision, wrong episode, stale approval | the bounded context's closed error/outcome ADT, with safe structured facts | correct inputs, reconcile or obtain fresh evidence; no automatic retry of the same rejection |
| Validation error | raw fields or relations cannot construct an accepted value | validator/factory result; field/rule paths at a human boundary | preserve candidate input and return independent findings together |
| Infrastructure failure | a dependency cannot establish the requested result: transport timeout, storage unavailable | adapter translates to the port/application's typed failure vocabulary | bounded retry only with operation safety and outcome certainty established |
| Unexpected fault | defect, corrupt persisted invariant, impossible adapter response | contain at the outer execution boundary; safe failure plus restricted diagnostics | fail closed; investigate or reconcile; never fabricate domain success |

The same symptom has different meanings by provenance: invalid caller input is validation;
invalid persisted accepted data is corruption/version skew. A timeout may mean **outcome unknown**,
not that a write failed. An infrastructure failure is still an explicit value above its adapter.
No SQL error, SDK exception, `Throwable`, HTTP response or framework type enters domain APIs.

Use a small closed vocabulary per capability/context; shared wire `ErrorCode` is a public
contract, not a domain mega-enum. A wrapper must add ownership, a distinction, an invariant or
translation value. Keep an existing adequate outcome instead of adding a parallel wrapper.

## ERR-02 — typed propagation and composition

- Rust uses `Result<T, E>` for fallibility and `Option<T>` for meaningful absence. Kotlin uses
  sealed outcomes/rejections, or an adopted typed `Either<E, A>` / `Raise<E>` API. Kotlin
  `Result<T>` fixes failure to `Throwable`; it does not express the domain's error alternatives.
- Compose dependent steps with Rust `?`/`and_then` or Kotlin exhaustive `when`/typed bind.
  `map` transforms success; `map_err`/`mapLeft` translates failure at its owning boundary;
  `fold` projects both alternatives. Monadic composition means dependent steps stop on the
  first typed failure; it does not require a monad library or custom generic framework.
- Accumulate **independent** validation findings when the contract requires all blockers.
  A fail-fast `?`/bind chain cannot implement that promise. Do not run dependent stages on
  invalid inputs merely to accumulate more errors. Keep ordering/path attribution deterministic;
  failure collections represent at least one finding by construction or a checked invariant.
- Preserve the exact error until a boundary deliberately maps it. No `.ok()`,
  `getOrNull`, `unwrap_or_default`, nullable sentinel or string conversion that turns failure
  into absence/success. An intentional lossy projection needs a stated contract and test.
- Recovery is an explicit typed decision, not “log and continue”. Diagnostic logging belongs
  to the shell that owns the failure; pure transformations do not log, sleep or execute effects.

Review every closed ADT's consumers with exhaustive `match`/`when`, including retry and UI
projection. A catch-all for an open foreign protocol belongs only at its adapter, with an
explicit unsupported/protocol-failure outcome. Do not use a catch-all to hide new domain cases.

## ERR-03 — preserve refinements through the chain

```text
raw DTO → validation result → refined value → pure decision → application outcome
         typed findings        invariant-preserving operations     ↓
                                                        shell executes effects
```

Consumers require the refined value, not its raw primitive plus a remembered check. Every
transformation either preserves its predicate or returns a typed refusal and a new validated
value. Related facts need an aggregate check; positive position and duration separately cannot
prove `position <= duration`. Deserialization, copy, restoration and conversions follow
[boundary-hardening](boundary-hardening.md); neither a wrapper nor `map` establishes validity
by itself. Authorization/approval witnesses remain bound to exact actor, resource and revision;
the application revalidates mutable facts at commit.

No expected rejection partially changes settled state or dispatches an external effect. A typed
return does not make prior I/O atomic: transactions, idempotency and reconciliation still belong
to their owning application/adapters. Test the complete unchanged state and ordered effects.

## ERR-04 — exceptions and panics stay at the outer edge

Prefer an external library's supported typed result/mapping API. If foreign I/O or platform
interop only throws, catch around the **smallest foreign call** in its concrete adapter and
translate documented failures immediately. Typed domain/application APIs and abstract ports
must not require callers to catch exceptions. Do not wrap an entire use case in `try/catch`.

No `throw`, `require`/`check`, `!!`, unchecked indexing, `unwrap`/`expect`, `panic!` or
`unreachable!` for caller-controlled expected business flow. Internal assertions may diagnose
programmer defects only where callers already supplied validated values; they cannot replace a
fallible factory or be the only protection for an accepted invariant. Existing assertion-based
mock construction is not evidence of typed production validation.

A final task/platform/HTTP containment boundary may handle an unexpected fault when the runtime
supports it, after preserving cancellation and fatal-runtime behavior. Record its scope,
safe outcome, diagnostic owner and remaining uncertainty. Do not convert every defect into a
retryable “unavailable”, hide corruption as bad user input, or rely on Rust panic catching for
normal recovery (`panic=abort` cannot unwind). Fatal runtime failures are not ordinary values
that the application can safely recover from.

Keep unavoidable effects and unverifiable foreign behavior in concrete adapters and their
outer hosts, as far from pure domain code and abstract interfaces as possible. Name the trusted
assumptions at those seams; a typed wrapper does not verify the external implementation.

## ERR-05 — cancellation has its own semantics

Cooperative cancellation is lifecycle control, not validation or an unavailable dependency.
Kotlin adapters rethrow `CancellationException` before classifying foreign failures; broad
`runCatching`/catch helpers must not swallow it or typed DSL control signals. Rust future
cancellation/drop is not automatically an `Err`; explicitly modeled cancellation applies only
where a protocol needs an observable outcome. Do not invent a cancellation value for every API.

Document the owner, cleanup and commit boundary. Cancellation cannot undo a remote write or
prove a subprocess stopped. Fence late results by account/session, request, resource and revision
identity. A cancelled or disconnected mutation with no receipt needs reconciliation before replay.

## ERR-06 — retry is a pure decision, execution is an effect

Classify failure, operation safety, attempt budget, elapsed deadline and outcome certainty as
inputs. Supply time and jitter as facts. Return stop, retry-after or reconcile as typed data;
the shell schedules and executes it. Retryability is not inferred from a message or HTTP status
alone. Explicit rules:

- validation, denied access, stale evidence and revision conflicts require correction or
  reconciliation, not blind replay;
- transient reads may retry within a bounded policy; writes require the existing operation-ID
  contract and identical intent/payload on replay;
- unknown write outcomes reconcile through receipts/status before a new effect; do not mint a
  fresh key just because the finalize response was lost;
- cancellation stops automatic retry; corruption and unexpected faults fail closed unless a
  capability's recovery protocol explicitly permits a safe action.

The capability owners define concrete retry/idempotency behavior; see
[HTTP writes](http-api-boundary.md#retriable-writes) and the existing production/listening
contracts. No generic retry loop or backend synthesis job is introduced by this policy.

## ERR-07 — translate once per real boundary

```text
foreign failure → concrete adapter → typed port/application failure
domain refusal  → application outcome → HTTP ApiError / renderer error projection
```

Mappings are total over their owned closed variants, preserve correction/reconciliation facts,
and disclose only contract-approved fields. Keep credentials, signed URLs, SQL, stack traces and
private prose out of responses, UI text and routine logs. Diagnostics use safe identity/reason
codes and follow the owning privacy policy. Reuse the existing wire contract rather than
renaming codes to match new internal types.

Applications: [Kotlin TYPE/FP](../../cantos-cmp-mobile/references/kotlin-conventions.md),
[Leptos async](../../cantos-leptos-web/references/leptos-async.md),
[Rust HTTP mapping](http-api-boundary.md#errors).

## Cheapest verification first

Use the [L1–L5 ledger](verification-strategy.md#layered-verification): exact factory/error
partitions, exhaustive projections, composition short-circuit/accumulation and refinement
preservation at L2–L3; typed boundary, cancellation, duplicate and out-of-order tests with named
fakes at L4; real storage/HTTP/native behavior at L5. Apply
[Kani selectively](mutation-and-formal.md#bounded-model-checking), never to every error wrapper
or UI. Missing infrastructure does not block delivery of independently verified layers under
the [handoff policy](local-execution.md#local-handoff-and-pr-completion); list deferred checks,
prerequisites, command, criteria, owner and residual risk separately from actual failures.
