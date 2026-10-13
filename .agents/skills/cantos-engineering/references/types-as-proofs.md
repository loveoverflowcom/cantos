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
Still runtime-only: <what remains checked at runtime, and where>
```
