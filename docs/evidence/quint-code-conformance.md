# Quint pure-core conformance — 14 October 2026

[Issue 106](https://github.com/loveoverflowcom/cantos/issues/106), PR B, stacked on unmerged
[PR 117](https://github.com/loveoverflowcom/cantos/pull/117) at
`d76cdb51258b874ff3f68d987de64d7aa690402e`, tree `74ab82662aad645b907a53f0880b9ecd870f8ae6`.
Target its feature branch, not develop. No merge or moving-base reconciliation is included.
See [ADR 0026](../decisions/0026-quint-executable-behavior.md) and the
[observation protocol](../../specs/behavior/observation-protocol.md).

## Invariant and boundary

Selected original Quint checkpoints must agree with independent production private-admission
decisions and owner projection. The test-only Rust observer receives events/IDs, never expected
state/outcome. It calls `decide_admission` on Begin/Complete and `submission_state` at every
checkpoint. Real typed constructors supply full synthetic submission/rendition documents,
preserved unchanged on rejection/replay/assertion. Assertion repeats no decision/construction.
Media domain owns the rules; Python decodes ITF and compares, never decides admission.
No production function, dependency, transport/Protobuf field or database schema changed.

## Published-code pure-core evidence

`cargo xtask verify behavior --conformance`: PASS on clean published code HEAD
`a1357b8f06634ec2177ccd59c98f54818d5a076b`, tree `435c66f2946057e682882d47823e947f9b82dc28`.
Bundle: `target/verification/behavior/cantos.media.private-audio-admission-75348259c6294166ab593802f5517163/`.
Original bytes, hashes, commands, logs, ITF, actual observations and first mismatches are retained.
This evidence correction changes only this document; final-head checks belong in the PR ledger.

- Model: six stages PASS, 19 named runs, seven bounded sampled properties, 11 witnesses and
  seven typechecked model controls. This is still model-only evidence, not proof of code.
- Conformance: identity, original traces, mutant sensitivity, observer sensitivity and freshness
  all PASS; 31 original traces and 265 checkpoints (19 named + 11 witnesses + retained sample).
- All seven wrong model controls semantically mismatch correct observations. Growing prefixes
  stop at first divergence, never treating later illegal continuation/parser failure as a kill.
- Wrong revoked=false adapter input mismatches original revoke-during-inspection checkpoint 3
  and revoked-replay checkpoint 4. This is one concrete defect, not all production mutations.
- 268 implementation/fixture/compiler-config file hashes; 14 Cargo configuration/absence
  identities; exact Rust/Cargo 1.87.0 launcher/binary/driver hashes and pinned direct compiler.
- Legacy/TOML ancestor/global Cargo config, compiler/tool-selection and runner/linker overrides
  are checked/refused; source/HEAD/tool/config identities are rechecked at completion.

Model report SHA-256: `2f11ff12af2c170ad7f34c91fdc3055456537d6b8efc03d36bd64ddcf5e52355`.
Conformance report SHA-256: `68719a7dec94724411fd7a75564db4d743e7fb76c4fa8760a334d7a868652f71`.
Observer source: `a80afbda012c10b81a38bed4910d1f637f745a23b9308cabb3d3b22ce21a049a`.
Comparator source: `e503463e978ea275ef31f9b8958ef093db7eb0c2e3e0a20270d476d279c2465c`.
Model report stays model-only; `conformance.json` owns the separate pure-core result.

## Native and review ledger

- Five ordinary observer regressions PASS; ignored replay runs explicitly as exactly one test.
- Behavior Python contracts: 36 PASS (21 original + 15 codec/identity/comparison/config tests).
- Independent review rechecked first-run comparisons/hashes, found compiler/config provenance
  gaps, then confirmed their closure. Published-code execution above follows those fixes.
- `cargo xtask verify rust`: PASS (format, workspace lint/tests), including 404 API and 157 xtask
  unit tests. DBSP has 409 unit tests PASS; ignored infrastructure/runtime tests remain ignored.
- Broad `cargo xtask preflight --changed-only --base feat/issue106-quint-behavior-foundation
  --skip mutation --skip postgres`: 16 gates PASS, two explicitly NOT_RUN. Passed repository,
  behavior, format, Script IR/Protobuf references, workspace lint/tests, DBSP, WAV/actual Opus,
  Web lint/both bundles, UI preview and both CMP/JVM gates. Not full acceptance.
- Broad behavior used intermediate bundle `729f8b0e1d1b4560959551d393504607`; later config hardening
  was rerun in retained dirty-input bundle `c60b791a8e7c4d34b31bc69704d8a604` and then the clean
  published-code bundle above, not inherited. Broad log SHA-256:
  `5e3345b62f569b6673cafcee5b0ae7f2ac302e6f1457da6d6fa6d0b11a755daf`.
- Initial explicit `cargo xtask verify contracts`: FAIL at standalone cantos-contracts catalog_wire
  compilation with ENOSPC. Script IR/Protobuf references PASS; following WAV/audio stages
  NOT_RUN in that command. Equivalent workspace suites passed earlier, not a substitute for
  this failed command. After checking process/file use and locking inactive sessions, only seven
  task-generated incremental cache sessions were reclaimed (1,328,618,515 bytes). Executables,
  source/audio and every evidence bundle were preserved. The bounded warm retry then PASSED all
  five contracts stages, including actual Opus inspection; retry log SHA-256:
  `1cb14b576c747ca09ed210ae470fce333aef226aaad51f5ea734eb57926c6e41`.
- Earlier temp-root fixture/selector and enum-name lint failures were corrected; guards retained.

## Deferred acceptance and residual risk

Lifecycle remains draft. Evidence is bounded sequential pure-core differential testing with
synthetic shell effects. Only the retained property sample is replayed against Rust, not all
500 model samples. Time maps n to 2000+1000*n ms, window [3000,5000); digest a/b and size100/101
preserve finite equality/order classes, not numeric/hash/codec completeness.
Complete(false) calls the decision but masks its raw result as MediaRejected in Quint: no
independent raw-decision oracle there. Abandon/authority changes are harness effects.
Owner Admitted is history, never producer permission or public/listener access.

Production application orchestration, authentication, SQL/concurrency/restart, actual bytes,
private object provider, public delivery and API/Leptos/CMP/device conformance remain NOT_RUN
by this observer. Native broader gates retain their separate scope. No exhaustive refinement,
solver/fairness/liveness proof. Endpoint freshness cannot isolate mutate-and-restore writes.
Retain native constructor, foreign binding, clock/hash and explicit event-matrix regressions.

Mutation and PostgreSQL are NOT_RUN here; PR A records cold mutation-baseline and Unix-socket
blockers. Rerun their existing gates on a suitable host. Capacity no longer blocks staging or
final-head checks; report the final published commit/tree and its fresh dispositions in the PR.
No merge, provider call, paid production, persistent credentials or public media access.
Issue 106 stays open.
