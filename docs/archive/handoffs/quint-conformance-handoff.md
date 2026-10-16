# Quint PR B: independent implementation observations

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/quint-conformance-handoff.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

[Issue 106](https://github.com/loveoverflowcom/cantos/issues/106) originally separated the
[PR A foundation](../../../specs/behavior/README.md) and a subsequent implementation-conformance
session/PR. [PR 123](https://github.com/loveoverflowcom/cantos/pull/123) now supplies that pure-core
observer inside [PR 117](https://github.com/loveoverflowcom/cantos/pull/117), rebased onto develop
`1c1d2107f1ca1aacac8c66b7b418c1070b35160c`. The criteria below retain their original scope;
wider API/Leptos/CMP scenarios stay deferred. GitHub owns execution status.

## Real seam and one outcome

Consume stable capability `cantos.media.private-audio-admission` and its catalog property/run/
witness IDs. Add a **test-only observation adapter** calling the existing production functions
in [Media lifecycle](../../../services/cantos-api/src/contexts/media/domain/lifecycle.rs):

- `decide_admission(&AudioSubmission, now, revoked, current, sha256, length, Option<&AudioRendition>)`
  returns exact `AdmissionDecision::{Inspect, Replay}` or `AudioError`.
- `submission_state(now, expires, revoked, current, admitted)` returns the real owner projection.
- `AudioSubmission::create`, `Binding::from_brief` and crate-visible `AudioRendition::create`
  construct real typed synthetic fixtures; no new production raw/proof constructor is needed.
- The separate `transition(state, SubmissionEvent)` matrix stays native and may receive an
  explicitly scoped conformance extension only after its distinct event semantics are modeled.

Put tests beside [lifecycle_tests.rs](../../../services/cantos-api/src/contexts/media/lifecycle_tests.rs)
so the crate-visible constructor remains private. Call actual production decisions at every
Begin/Complete event checkpoint. Named test assertions are marked `Assertion`/`NoOutcome`:
preserve and compare those no-effect checkpoints without reapplying the preceding operation
or constructing another rendition. `RestoreCurrent` only supplies recovered authority for the
same immutable binding and never resets revocation. `complete(true)` may construct valid synthetic rendition metadata
only after the actual decision authorizes it; describe this as pure-core conformance, never
actual inspector/storage/SQL acceptance. `complete(false)` and Abandon are harness effects.
No hand-maintained Rust/Kotlin/JS reference reducer may become the expected-value oracle.
Expected outcomes/checkpoints come from Quint traces, with model identity pinned to the run.

## Observation contract to implement

Use catalog schema ID `cantos-private-audio-admission-observation-v1`. PR A retains raw ITF
variables `state`, `before`, `last`, `outcome` with closed sum variants and bigint values.
A PR B codec must deliberately handle the exact registered representation; PR A only checks
its envelope. Preserve every checkpoint, exact outcome/error, current authority/time facts,
settled identity/admission time, inspection/admission counts and owner state.

Map synthetic digest/size aliases 0/1 to two explicit fixed nonzero bounded byte declarations,
and time classes to a valid real `[created_at, expires_at)` window with admitted_at inside it.
Document the scaling and whether it preserves the modeled order/equality relations. Do not
add API/database fields to fit a model. Keep full submission/rendition documents as the
immutable observed receipt, not a guessed bool or count. An owner-view `Admitted` never
implies producer authority or public/listener access.

Reject stale model/catalog/tool/config/input identities, unsupported observation schema,
unknown actions/variants, malformed bigints, missing/reordered checkpoints, insufficient
state/effect observations and empty test selection before claiming conformance. Trace seed
supplements concrete raw ITF and provenance; it cannot replace them. Replay named runs and
non-vacuity witness traces whose source identity is the **original model** as positive
conformance expectations. A negative-control trace names a deliberately **mutated source**:
it must not become expected accepted behavior. Use such traces only as separately labeled
negative sensitivity inputs that must mismatch a correct implementation observer. Add a deliberately faulty implementation
observer/production mutation control to demonstrate mismatches are detectable.

## Evidence and limits

Native regression tests remain. In particular retain exact clock lower-bound, wrong settled
submission binding, state/event matrix and digest/replay tests; they cover things outside the
valid one-submission model. No current API/Protobuf/database contract changes are necessary.

Gate/report model PASS separately from Rust conformance PASS, then list DB/API/authorization/
actual-media/storage/client/device layers independently. Do not change the model to match a
mismatch without reviewed requirement evidence. Real-provider private policy, byte inspection,
concurrent transactions, restart and public delivery remain their existing acceptance owners.

## Historical dependency strategy

The original split delivery recommended PR B from latest develop after PR A merged. If PR A remained unmerged and continued
work is authorized, branch PR B from PR A's exact published head and target the PR A branch
while developing; label it stacked/dependent. Do not copy/cherry-pick the foundation into a
second unrelated develop PR or merge PR A without permission. After PR A merges, rebase the
conformance branch onto current develop, retarget and rerun all affected evidence on final
inputs before a clean handoff. This split-PR strategy is historical now that PR 123 is included
in PR 117; current integrated results belong in the PR ledger.

## PR B implementation checkpoint

The [observation protocol](../../../specs/behavior/observation-protocol.md), registered test-only
[observer](../../../services/cantos-api/src/contexts/media/quint_conformance_tests.rs) and
[`--conformance` primitive](../../../scripts/verify_behavior_conformance.py) implement the bounded
pure-core handoff. [Evidence](../../evidence/quint-code-conformance.md) records executed scope
and residual acceptance. No production semantics or API/Protobuf/database fields changed.
PR B's pure-core changes are included in rebased PR 117; their earlier results retain their
original source identities and do not establish fresh acceptance of the rebased inputs.
The original handoff is retained as review criteria, not authority to widen to application/IO.
