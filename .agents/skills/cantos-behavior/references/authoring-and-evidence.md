# Stateful authoring and evidence

Read [the skill](../SKILL.md) and [catalog](../../../../specs/behavior/catalog.json) first.
This is a method, not product truth or a new DSL.

## One bounded migration

- Select a small reviewed stateful slice with a real production seam. Inventory approved
  requirements, implementation facts, assumptions and unresolved decisions separately.
- Define closed state/action/error variants, legal action guards and rejected outcomes.
  Keep external facts explicit; do not invent database fields to fit a model.
- Express unchanged settled values relationally through a checkpoint (`before`, action,
  outcome, current state), not by introducing another reducer or post-hoc generated oracle.
- Keep bounded actor/object/input/time classes explicit. Do not imply numeric, hash, codec,
  concurrency or fairness completeness from a small sequential simulation.
- Register stable properties, named runs, witnesses and controls together. IDs survive
  file/declaration rename; semantic splits need a reviewed old-to-new mapping.

## AI scenario partitions

Derive these from the chosen capability's actual guards, not from a generic product template:

1. Minimal success reaching a nontrivial settled state
2. Exact duplicate after lost response, preserving identity and effect count
3. Contradictory duplicate and independent payload dimensions
4. Lower/upper time boundary and already-settled history
5. Revoked, stale or wrong-scope authority according to owning requirements
6. Authority/input change between decision and final settlement
   Include recovered authority over an unchanged binding without silently clearing revocation.
7. External rejection and interrupted attempt followed by retry
8. Distinct owner projection versus permission/public visibility
9. Cross-object/actor isolation when the approved model actually includes those objects
10. Unmodeled semantics that require a decision or native-only boundary test

For each included class, identify catalog run/property IDs, antecedent, expected exact outcome,
needed observations, witness and sensitive mutation. Do not claim a class absent from this model.
Wider API/Leptos/CMP flows reuse IDs only where their observations have the same meaning;
keep native async, DOM/semantics, accessibility and device tests for their separate boundaries.

## Non-vacuity and negative controls

The current runner asks Quint to find `not(witness)` invariant violations. That creates an
actual witness trace; failing to find one under the budget fails the gate. It does not prove
unreachability. Deliberate action/state mutations must typecheck and violate exactly their
registered property; a tool/parser/runtime error is not an invariant counterexample.
If a property becomes `true`, its corresponding control should fail to find a violation.
If a transition becomes unreachable, its witness should fail. Both are gate failures to fix,
not reasons to drop the witness/control or increase claims.

Example: the private audio model's owner history stays admitted after upload expiry while
producer replay fails. A control moving expiry ahead of admitted history in the projection
must break `ownerViewPrecedence`. This checks sensitivity to that defect only; it is not a
real SQL/authorization/media observation.

## Counterexample handling

1. Read the failing stage and exact status first. Preserve the existing bundle.
2. Check command/backend/seed, source/input/tool/config identities and complete raw trace.
3. Locate the first violating checkpoint and the owning requirement. Distinguish a model
   error from implementation/observation error or a product ambiguity.
4. For a genuine implementation mismatch, fix the actual consumer, retaining the same
   expected model/run ID; replay the concrete trace and rerun the affected checks.
5. For a real requirement change, request acceptance and update inventory/catalog/model/
   runs together. Do not infer it from UI/code or silently rewrite expected results.
6. Promote only a small sanitized reviewed regression corpus with model/tool/provenance.
   Never commit private content, credentials, full generated campaigns or bulk media.

Report type/effect and named examples, bounded sampled model properties, witnesses and
mutation sensitivity separately. Default model-only execution is not `bounded-model-checked` (no solver),
`differentially-tested` against Rust, `integration-tested` against SQL/storage or `device-tested`.
The optional registered conformance gate adds attributable pure-core `differentially-tested`
observations, not those other tiers. See the [observation protocol](../../../../specs/behavior/observation-protocol.md).
Native constructor/transport laws remain useful independent checks. An accepted `.qnt`
expectation and a real observed implementation must both exist before claiming conformance.
