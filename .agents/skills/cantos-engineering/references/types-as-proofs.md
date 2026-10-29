# Types as proof barriers

> **Scope.** Encode Cantos invariants as named types, private constructors, evidence values,
> closed enums and restrained typestate so invalid states are unrepresentable or checked once at a
> boundary. Use when modeling IDs, revisions, digests, fingerprints, money, durations, rights
> status, approvals, publication readiness, job states, player states or any value whose validity
> other code relies on.

A private newtype with a fallible constructor is runtime-checked evidence with compiler-enforced
provenance. Call it a proof *barrier*, not a proof. Reserve "proved" for a verifier that actually
ran under a stated model ([`mutation-and-formal.md`](mutation-and-formal.md)).

For Kotlin use the canonical [TYPE-01–05](../../cantos-cmp-mobile/references/kotlin-conventions.md):
distinct identities, relational validation, private factories, copy/serialization bypass audit,
closed ADTs and typed rejection. Kotlin provides neither dependent types nor general refinement
proofs. Public `copy` and deserializers are construction paths, not automatic proof preservation.

## Workflow

1. State the proposition in one sentence: *a `ScriptRevisionId` always refers to an immutable
   accepted revision*; *`MoneyMinor` is never negative*; *a `PublishableRelease` has passed every
   gate for exactly these inputs*.
2. Find every construction path: constructors, `pub` fields, `Default`, `From`/`TryFrom`,
   `Deserialize`, SQL row decoding, fixtures, migrations, test builders, macros.
3. Choose the smallest encoding that removes the invalid states (below).
4. Make the representation private; expose only invariant-preserving operations.
5. Strengthen consumers to require the refined type, so they cannot forget the check.
6. Keep wire and storage DTOs raw; convert at the trust boundary
   ([`boundary-hardening.md`](boundary-hardening.md)).
7. Test constructor partitions, bypass paths, and preservation under every operation.

## Choose the smallest sufficient encoding

1. **Newtype** for one identity, unit or intrinsic invariant: `DialogueId`, `SceneId`,
   `ScriptDigest`, `SpeechFingerprint`, `DurationMs`, `LoudnessLufs`, `MoneyMinor`.
2. **Struct of refined fields** when invariants compose independently.
3. **Enum** when states are mutually exclusive or fields are conditionally valid.
4. **Separate input/output types** for pipelines: `RawScript → ValidatedScript → AcceptedRevision`.
5. **Evidence value** when a check authorizes a later operation: `FrozenProductionInputs`,
   `BudgetReservation`, `PublishableRelease`.
6. **Typestate** only for one strong compile-time lifecycle on one object. Prefer an enum for
   heterogeneous collections and data-driven transitions.

## Identities must not mix

```rust
// Illustrative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DialogueId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScriptRevisionId(Uuid);
```

A function that takes `(DialogueId, ScriptRevisionId)` cannot be called with the arguments
swapped. Do not add `From<Uuid>` blanket conversions or `Deref<Target = Uuid>`; they reopen the
mix-up. IDs come from the shell (generator or database) and are parsed at boundaries.

## Model states, not flag combinations

```rust
// Counterexample: 2^3 combinations, most meaningless.
struct Step { started: bool, finished: bool, error: Option<String> }

// Illustrative: only meaningful states exist, each carrying only its own data.
enum StepState {
    Pending,
    Leased { lease: Lease },
    Succeeded { artifact: ArtifactId },
    Failed { reason: StepFailure, retry: RetryDecision },
    Cancelled { at: ServerTime },
}
```

Exhaustive `match` then forces every consumer to handle a new state. Keep execution status,
editorial status and publication status in separate types; one enum for all three is a defect.

The defect is an *invalid combination*, not the number of flags. Independent input facts whose
every combination is meaningful stay Booleans: `listening::domain::reconcile::classify` takes
`same`, `protected` and `checkpoint` as such facts and already returns the closed `Admission`;
an enum there would exclude nothing. A `Vec` of independent findings (`publication::evaluate`'s
blockers) is a collection, not a state. Name the invalid combination and who can produce it;
when there is none, there is nothing to prevent.

## Produce evidence instead of booleans

```rust
// Counterexample: the check's result is discarded after the branch.
if gates_pass(&candidate) { publish(&candidate).await?; }

// Illustrative: publish requires a value only the decision function can build.
let release = decide_publication(&facts).map_err(PublishError::Blocked)?;
publish(release).await?;
```

`PublishableRelease` has a private constructor in the module that owns `decide_publication`. It
carries the exact input identities it was decided for, so it cannot be reused for a different
candidate. It is never `Deserialize`, never `Default`, and never stored as a standing right; a
new decision is made at commit time.

## Literals that stand for a closed set

A spelled-out string, number or path is a hidden type. Before keeping one that other code must
spell identically, classify it; a grep hit is only a candidate.

| The literal is | Do |
|---|---|
| a closed set of purposes, states or outcomes (`"s"`/`"t"`/`"n"`, `"session_changed"`) | an enum or sealed type with exhaustive matching; where a wire or platform boundary must stay a string, one `const` owner in the module that defines the vocabulary, referenced by producer **and** consumer |
| a canonical wire name (route, cookie, header, error code, JSON key) | the typed DTO or the owner's constant in production; the value itself never changes without a contract version |
| the expectation in a test of that contract | keep an **independent** literal; importing the production constant makes a typo pass |
| a file or module path | resolve it from its owner; a deleted path is removed or re-pointed, and a gate must fail on a missing input, never skip it to stay green |

- A `match`/`when` on a string across two modules is a defect-in-waiting: a misspelling compiles
  and falls into the `else` arm. Make both sides name the same owner.
- The owner is the module that defines the vocabulary. Do not add a global constants package or a
  shared contract crate for reuse ([ADR 0046](../../../../docs/decisions/0046-service-owned-contracts.md));
  each consumer keeps its own representation and proves it against the canonical fixtures.
- Test diagnostics name a case by label; they never format cookies, bearer values or token bodies.

## Money, time and audio units

- Money: integer minor units plus a currency type (`MoneyMinor`, `Currency`), never `f64`. An
  unavailable estimate is `CostEstimate::Unavailable`, not zero.
- Time: separate `ServerTime` (authoritative), `DeviceTime` (context only) and media positions
  (`PositionMs`). Never compare a device clock with a server clock to order writes.
- Audio: `LoudnessLufs`, `TruePeakDbtp`, `SampleRateHz` as newtypes so a peak limit cannot be
  passed as a loudness target.

## Preserve the proposition

Audit every method on a refined type: `DerefMut`, `AsMut`, `pub` fields, mutable iterators,
unchecked setters and derived `Default` can break the guarantee. Expose semantic operations that
return a refined value or a `Result`.

## Name the proposition each barrier holds

A runtime-validated type is a barrier for the propositions its constructors check, and only for
values built through them. Write those propositions down and audit the paths per proposition.
At `cf3ab8ed`, `identity_auth::decision::SecurityState` documents a decoded response "whose
shape was checked", but its fields are `pub`: `from_response` refuses version `0` and an
unspecified account state, while any crate code or test can write the struct literal. A
consumer relying on `version != 0` relies on convention, not the type; closing it is the
owner's call, and a review records a hardening opportunity, not a verified defect. By
contrast `identity_auth::turso::record` re-admits each `ObservedSecurity` column through
`Subject::from_hex`, `Epoch::from_slice` and `positive`, so the row path keeps their propositions.

## Compiler and lint claims need a sensitive misuse

"The compiler rejects X" is `type-enforced` only with a named misuse that fails for that reason:

1. Write the smallest misuse that would compile if the barrier were removed: supply every other
   field and argument so the barrier is the only error.
2. Check the cause, not just the failure. Stable rustdoc does not compare the code in
   `compile_fail,E0451`: any compile error passes (checked with the pinned 1.98.1, where an
   `E0308` misuse labeled `E0451` passed). Read the diagnostic once and record it.
3. Confirm a gate runs it. Doctests run under `cargo test` without `--lib`, `--tests`, `--test`
   or `--all-targets`; `cfg(kani)` code compiles only in Kani runs.

At `cf3ab8ed` the `publication::PublishableRelease` doctest `PublishableRelease { manifest: todo!() }`
omits `evaluated_at`, so it would still fail, with `E0063`, if the fields became public. The
privacy barrier holds by source inspection; that doctest is not sensitive to its removal.

A lint claim is `statically-checked` only when the lint is enabled at deny level and the command
that ran covers the code's target, features and `cfg`. `clippy -D warnings` denies only enabled
lints. At `cf3ab8ed` no `[lints]` table or `deny(clippy::…)` attribute exists, so restriction
lints such as `clippy::unwrap_used` are not enforced; Wasm-only code needs the Wasm lint run and
`cfg(kani)` code is never linted.

## Choose a barrier for a concrete consumer

Use these questions with the [admission pilot](../../../../docs/evidence/identity-session-admission-pilot.md):

| Technique | Use when | Decline or qualify when |
|---|---|---|
| private construction / closed result / refined input | a named caller can forge an invalid state and the consumer can require the checked representation | no consumer gains a restriction, or the claimed invariant is durable I/O: a consumed revoke token cannot make an aborted write commit |
| constructor-coupled proof | the production path constructs the refined input; separate raw rejection, meaningful accepted-value covers and accepted-value postconditions ([harness obligations](mutation-and-formal.md#bounded-model-checking)) | public literals, decoding or fixtures bypass the constructor; prove only the admitted domain and keep the bypass/entrypoint obligation open |
| deterministic replay | event order, failed effects and recovery determine the result; production decisions/shell can be driven with named external failures | the claim concerns unrun restart/concurrency or independent model authority; reuse the registered observer instead of writing a second reducer |

In the pilot `SecurityState` still has public fields; the real RPC path calls `from_response`,
but a struct literal can reach the pure subject-mismatch branch. No privacy guarantee or new
type barrier is claimed. The real-store abort and subsequent admissions answer the effect
question a type cannot answer. Neither a different HTTP return code nor a type-level token
would preserve a failed revocation obligation across the next read or a restart by itself.

## What a type does not prove

- `#[must_use]` warns, where the lint runs, that a value was ignored; it does not make the caller
  act on it.
- A witness or token consumed by value proves the callee received a decision, not that the
  durable effect it authorizes committed. Durability needs the real store
  ([production reach](verification-strategy.md#production-reach-entrypoint-core-and-effect)).
- `Drop` does not run on abort, `mem::forget`, a reference cycle or a killed process
  ([crash, not error](../../cantos-production-pipeline/references/fault-injection-testing.md#4-simulate-a-crash-not-an-error)).
- A lifetime bounds how long a borrow lives; it does not say which transaction, session or
  request a value belongs to. Two values of one type and lifetime from different transactions
  are interchangeable to the compiler. Bind an instance with a value (ID, generation, fence)
  checked where it is used.

## Escape hatches

When trusted recovery or a hot path must skip validation, keep it narrow and loud:
`from_trusted_row_unchecked`, documented proof obligation, a cheap `debug_assert!`, and a test
that exercises the trusted producer with the consumer. `unsafe` is for memory safety only, never
for a logical invariant.

## Verification obligations

For each new or changed refined type:

- valid representatives at every boundary construct;
- values just below, at and above each limit behave as specified;
- empty, maximal, duplicate and malformed inputs return the exact error variant;
- deserialization, row decoding and fixtures cannot forge an invalid value;
- every public operation preserves the invariant;
- equality, ordering, hashing and canonical encoding match domain meaning.

## Report

```text
Proposition:        <what is now impossible to represent>
Evidence level:     type-enforced (+ example-tested | property-tested …)
Barrier:            private field / validated TryFrom / serde try_from / evidence value
Entry paths closed: new · parse · TryFrom · Deserialize · row decode · Default · fixtures
Misuse checked:     <the misuse, its diagnostic and the command that ran it> | none claimed
Still runtime-only: <what remains checked at runtime, and where>
```
