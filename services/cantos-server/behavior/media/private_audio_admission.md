# Selected pilot: `cantos.media.private-audio-admission`

Module-owned inventory of [`cantos.media.private-audio-admission`](../../../../specs/behavior/catalog.json). The [root inventory](../../../../specs/behavior/inventory.md) keeps the shared authority framing; the registry owns the stable ID and lifecycle.

| Classification | Authority / fact | Pilot consequence |
| --- | --- | --- |
| Approved, narrow development scope | [ADR 0020](../../../../docs/decisions/0020-studio-private-audio-intake.md#decision) selects exact revision-bound private intake, actual byte inspection, immutable receipts, exact replay and commit-time rechecks. [ADR 0018](../../../../docs/decisions/0018-external-audio-production.md) keeps all audio production external. | No generation, encoding, public delivery, paid provider or new product rule. |
| Approved contract expectation | [Audio Lifecycle v1](../../contracts/studio/audio-lifecycle/v1/README.md#state-reviews-and-candidate-blockers): exact duplicate PUT returns the same rendition; contradictory settled bytes conflict; revoked/expired/stale producer replay fails; owner precedence is stale, revoked, admitted, expired, awaiting_bytes. | Named success/rejection runs and checkpoint invariants cover only this subset. |
| Implemented, independently of this model | [Pure lifecycle](../../src/contexts/media/domain/lifecycle.rs) `decide_admission` and `submission_state`; [application](../../src/contexts/media/lifecycle_application.rs) invokes them before inspection and again before settlement. [Existing native tests](../../src/contexts/media/lifecycle_tests.rs) cover digest/replay, lower clock bounds and foreign settled binding. | These are real PR B observation seams. PR A calls Quint, not these functions. Existing tests stay. |
| Implemented boundary, excluded here | `AudioSubmission::create/restore`, `AudioRendition::create/restore`, ownership, immutable full bindings, profile admission and same-receipt transaction arbitration. | The pilot starts with one valid prepared submission. It cannot certify constructors, authentication, foreign/corrupt database rows, actual bytes or concurrent commits. |
| Implemented external fact | [Brief freshness](../../src/contexts/production/domain/brief.rs) is recomputed from content, rights, latest guidance and archive status; existing [Work](../../src/contexts/catalog/domain/work.rs) and [Unit](../../src/contexts/catalog/domain/unit.rs) archive/restore can restore current authority for the same immutable binding. | `restoreCurrent` supplies that recovered fact; named admission/replay runs show it does not clear revocation or replace the settled receipt. No real archive/API/DB action runs. |
| Model assumption | One synthetic submission; digest/size aliases 0=exact and 1=conflicting; they are equality classes, not literal hashes or byte lengths. Time classes 0–4 use created_at=1, expires_at=3 and include backwards observations. | No numeric-range/hash/profile completeness or monotonic SQL-clock assumption. The real constructor still owns the 24-hour/positive-size bounds. |
| Model assumption | `Inspecting` is bounded simulator/shell history, not a new persisted state. `complete(true)` supplies abstract independently verified media/storage facts and rechecks current authority. `abandon` drops only that harness attempt. | `Inspect` is not admitted bytes. No actual inspection/storage, cancellation, restart, SQL isolation or request-lane claim. |
| Implemented bounded observation | Exact ITF codec and event-only test observer call the existing production admission/projection and typed constructors, with independent full receipts and decision/effect counts. | [`--conformance` protocol](../../../../specs/behavior/observation-protocol.md) maps fixed aliases, rejects stale identity and compares original traces separately from semantic sensitivity controls. Synthetic shell facts do not establish L4 application or L5 IO acceptance. |

## Exact pure decision boundary

The model preserves `decide_admission` precedence: revoked, expiry upper bound, creation
lower bound, stale Brief, payload mismatch (`DigestMismatch` before settlement;
`OperationReused` after), settled admission lower time bound, exact replay or inspection.
Foreign settled identity is outside the valid one-submission instance and remains covered by
native tests. Owner history uses a separate projection: a settled record survives upload
expiry, and stale plus revoked stays visibly stale. Never infer producer access from that view.
The independent explicit `transition(state, event)` matrix is retained in native tests;
observing a later clock is not its explicit `Expire` event.
