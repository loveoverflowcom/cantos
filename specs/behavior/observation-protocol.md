# Private admission observation protocol v1

Capability: `cantos.media.private-audio-admission`; schema:
`cantos-private-audio-admission-observation-v1`. Lifecycle remains **draft**. The
[model](media/private_audio_admission.qnt) and [catalog](catalog.json) own expectations/IDs;
[production lifecycle](../../services/cantos-api/src/contexts/media/domain/lifecycle.rs)
owns actual decisions. This is a test protocol, not an API, replacement DSL or production dependency.

## Command and boundaries

`cargo xtask verify behavior --conformance [--capability cantos.media.private-audio-admission]`
first executes all existing model stages. It then consumes that exact bundle, decoding the
pinned Quint 0.33.0 ITF representation. Only replay events and stable IDs enter the ignored
Rust unit-test observer; expected states/outcomes never enter it. No handwritten reducer
computes expected values. The Rust observer calls `decide_admission` at Begin and Complete,
`submission_state` at every checkpoint, and real typed submission/rendition constructors.

The observer retains its own facts/pending requests, counts actual inspection decisions and
successful rendition constructions, and reports full actual immutable documents. Receipt
aliases are projected from the constructed document, never copied from expected checkpoints.
No production function, raw/proof constructor, dependency or contract field changes.

## Exact mapping

| Model input | Real typed fixture |
| --- | --- |
| Digest alias 0 / 1 | 64 lowercase `a` / `b` SHA-256 characters |
| Length alias 0 / 1 | 100 / 101 bytes (separate from digest dimension) |
| Clock class n, 0–4 | 2000 + 1000 × n milliseconds |
| createdAt 1 / expiresAt 3 | Real `[3000, 5000)` submission window |
| Settled ID 1 | Fixed synthetic rendition UUID from the native lifecycle fixture |
| Complete(true) media fact | Synthetic valid mono Opus metadata, 48 kHz, 24 kbps; no inspected bytes |

The strictly increasing time map preserves order/equality, inclusive creation/exclusive expiry,
admitted-at checks and rollback observations. Payload maps preserve equality classes only,
not real hash/size/profile completeness. Binding/rights/owner and synthetic Brief are admitted
by existing typed constructors. Fixture changes outside these mappings fail closed.

## Checkpoints, effects and comparison

Decode every `state`, `before`, `last`, `outcome` checkpoint with contiguous exact indices and
previous-state linkage. Nullary sums carry empty `#tup`; numeric values carry canonical decimal
`#bigint`; Complete carries Boolean; Rejected nests a closed error variant. Reject unknown
fields/actions/variants and malformed or out-of-range input facts.

Assertion is `Assertion`/`NoOutcome` and never calls a decision or constructs another rendition.
Initial NoEvent occurs once. RestoreCurrent changes only recovered current authority over the
same binding, never revocation or history. Compare exact state/outcome/error, pending request,
owner view, receipt identity/admitted time/payload, effect counts and cumulative real decision
calls. Retain full submission/rendition JSON and require immutability across all checkpoints.

Complete(false) still calls the real decision but deliberately reports the model's MediaRejected
harness result. Its raw decision result has no independent oracle in this ITF checkpoint because
Quint masks it. Abandon and external fact updates are also harness effects. Neither is a claim
about actual inspector, storage, cancellation or application reauthorization order. Owner
Admitted means history, never producer permission, publication or listener access.

## Positive selection and sensitivity

Replay all 19 catalog named runs, all 11 original-model reachability witnesses and the one
retained sampled-property trace as **positive** expectations. Witness status `violation` means
it reached a witness by violating its negation, not wrong behavior. The other property samples
have model evidence but only the retained sample is replayed against Rust, not all 500 samples.

Seven deliberately mutated-model counterexamples are **sensitivity-only**, never correct
expected behavior. Decode structurally valid mutant counts/IDs even when they violate original
invariants. Replay growing prefixes through the first semantic state/outcome mismatch against
the correct core. Stop that control there: later mutant actions may be illegal after a correct
rejection. A parser/process/guard failure never kills a control. Record exact mismatch index,
expected mutant checkpoint and actual observation.

Separately replay the original revoke-during-inspection and revoked-replay named scenarios
with a deliberately faulty observer passing revoked=false to the actual admission seam.
Both must semantically mismatch. This detects a concrete authority-fact adapter defect,
not every possible implementation defect or all production mutations.

## Stable scenario coverage

These suffixes belong to the catalog capability prefix above; selection remains catalog-owned,
not this prose table. Every listed run is replayed with its full raw checkpoint sequence.

| Catalog run suffixes | Independently compared observations |
| --- | --- |
| `run.admit`, `run.lost-response-replay` | Actual Inspect/Admitted/Replay; one construction, immutable full documents and no assertion effect |
| `run.wrong-digest`, `run.wrong-length`, `run.contradictory-replay` | Independent digest/length aliases and exact DigestMismatch versus OperationReused, unchanged receipt/effects |
| `run.media-rejection-retry`, `run.abandon-retry` | Pending harness attempt cleared, no rejected settlement, later actual inspection/construction; failed media remains synthetic |
| `run.revoke-during-inspection`, `run.invalidate-during-inspection`, `run.expire-during-inspection` | Actual current decision on completion, exact authority error, no rendition; observer-fault control specifically covers revocation |
| `run.expired-owner-history`, `run.revoked-replay` | Historical receipt retained; owner projection distinct from denied producer replay |
| `run.stale-revocation`, `run.error-precedence` | Exact error precedence and independent owner-view precedence under combined facts |
| `run.before-creation`, `run.before-admission-replay` | Lower creation/admission-time errors with strict increasing alias mapping, including rollback |
| `run.restored-authority-admit`, `run.restored-authority-replay`, `run.revocation-survives-restoration` | Same binding regains current authority, no revoked grant reset, exact admission/replay and unchanged documents |

Original-model witnesses provide additional generated paths for the registered antecedents.
Native wrong-settled-binding, arbitrary numeric/hash/constructor and explicit transition-matrix
regressions remain independently owned; do not fabricate missing model state to label them
conformance. Later API/Leptos/CMP adapters may reuse these IDs only where observations have
these same meanings, retaining their own transport/interaction/device evidence.

## Freshness and evidence

Model `report.json` remains independent and model-only. `conformance.json` records its exact
hash, source revision, model/tool/catalog/config/input identities, observer/comparator/fixture/
compiler-config input hashes, full source snapshots, commands, logs, observations and first
mismatches. All output stays ignored under `target/verification/behavior/`; no bulk/private
content or media is committed. Retained input bytes permit reconstruction of dirty inputs.

Identity, original traces, mutant sensitivity, observer sensitivity and endpoint freshness must
all PASS. Empty/missing/duplicate/reordered traces, zero tests, partial observation, wrong schema
or stale retained/source/tool/config hashes fail closed. Cargo is locked/offline; each owned
observer process has a 180-second bound. Unexpected compiler overrides/ambient replay settings
are refused, including legacy/TOML ancestor/global Cargo configuration, configured tool
selection and target runner/linker overrides. Config absence is also pinned. Exact Rust/Cargo
1.98.1, launchers, toolchain binaries and compiler driver hashes are recorded and rechecked;
Cargo invokes the pinned compiler directly. Sources/HEAD/tools are checked again at completion. Endpoint checks do not isolate
against concurrent mutate-and-restore writes or hostile replacement of both tool and receipt.

## Assurance limits and next boundary

Evidence is bounded sequential `differentially-tested` pure core with synthetic shell effects,
plus explicit sensitivity defects. It is not exhaustive refinement, solver/liveness proof,
production application orchestration, authentication, SQL concurrency/restart, actual bytes,
object storage/provider, public delivery or API/Leptos/CMP/device evidence. Preserve native
constructor, foreign settled binding, clock/numeric/hash and explicit state/event-matrix tests.
Those outside-model claims retain their owners and gates. [Issue 106](https://github.com/loveoverflowcom/cantos/issues/106)
remains open for model review and later shared scenarios. Do not adapt the model to code drift
without reviewed requirement evidence.
