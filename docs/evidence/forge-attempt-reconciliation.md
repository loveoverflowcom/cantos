# Forge attempt reconciliation and executable gate follow-up (#203)

## Scope and invariant

Follow-up to [the first-slice record](forge-attempt-verification.md), on `develop`
`738bceb2de3ec79308a3268caa3b73a98ffa969d` plus PR #213's preserved infrastructure relocation
`898f7c5020630428a8d8bac2239aac299294c45a`. This strengthens the existing library-only
producer core, deterministic mock integration and draft Quint SSOT; it does not implement
the broader Studio, Drive, Gemini or Functions product.

The loser of a first-reservation race must compare the winner's request fingerprint before
reporting a replay. A changed request conflicts even while the winning attempt is open.
After a lost outcome response, one matching event inside an unrelated or corrupt readback
does not prove success: the full history must replay, its prefix must reconstruct the original
operation and its event at the exact index must match. A valid later regeneration does not
invalidate proof that the earlier outcome was recorded.

## Regressions and boundaries

- A different-request first-reservation race originally returned `Recorded` rather than the
  required `Conflict`. The new named regression failed on the original source, then passed
  after the fingerprint recheck. Same-request races still replay without dispatch.
- A matching settlement inside foreign history originally returned `Executed` success. Its
  focused regression failed first. Fault-injected cases now reject foreign key, changed
  fingerprint/cost, malformed prefix, corrupt suffix and missing/unreadable history, preserving
  the original `OutcomeNotRecorded` error and never making another provider call.
- Reservation-conflict readback covers unavailable, empty, malformed and foreign histories:
  no retry of the append, provider dispatch or blob write. Six corrupt/foreign histories,
  including full-width reserved-spend overflow, are checked across all three use cases with
  exact errors, unchanged logs and zero appends/calls. The durable-before-provider probe now
  checks the first and regenerated attempts, preserving the earlier exact result.
- Thirteen named Quint runs have thirteen lexical native mirrors. The new run observes a
  changed request against an already-open operation. It does not model a concurrent writer.
- The pinned gate initially failed with `wrong ITF observation variables`: its checker assumed
  the older pilot's four variables, while Forge declares five. Expected fields now come from
  the selected model's successful Quint typecheck output and must agree in scenario/control
  typechecks. Every ITF trace is bound to those exact fields; missing, extra, duplicate,
  foreign or untyped declarations/envelopes fail closed. The private-admission conformance
  codec remains restricted to its registered schema. This is envelope checking, not a second
  state-machine implementation or a Forge conformance observer.

## Current execution evidence

Rust 1.98.1; Node 24.19.0; npm 11.9.0; locked Quint 0.33.0. The working source was based on
`898f7c5`; production `produce.rs` SHA-256 is
`1b99ac1b4c3cb3fdf3c203529a4b171f5ce637d4209dab1ebb68a13d410ff2dd` and
`scripts/verify_behavior.py` is
`a906b2353e2fc7e536f1f16d758038b0c089eea813f0d165ddfef1f78e361404`.

| Layer | Command / claim | Result |
| --- | --- | --- |
| L1 dependency boundary | Existing architecture tests in `cargo test --locked -p cantos-forge`; no dependency, runtime or wire-format change | PASS: 6 architecture tests |
| L2/L3 core and contracts | `cargo test --locked -p cantos-forge` | PASS: 199 library, 3 profile-contract, 2 lexical binding, 4 CLI and 1 compile-fail doctest |
| L4 use cases | Same suite, production `Producer` with deterministic mock provider and fault-injecting attempt/blob doubles | PASS: 13 named mirrors and 17 native-only tests, including the new race/readback boundaries |
| Targeted assertion sensitivity | Delete request-fingerprint recheck, full-history replay check or original-prefix comparison separately; run the exact affected regression; restore the source after each | 3/3 semantic test failures; none counted from compile/process failure. Final original-source suite rerun PASS. This is targeted manual mutation, not a refreshed broad mutation score |
| Static Rust | `cargo clippy --locked -p cantos-forge --all-targets -- -D warnings`; `cargo fmt --all -- --check` | PASS |
| Gate contracts | `python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS: 532 tests, 5 skips. Simulated Kani/provider/error output belongs to test fixtures, not live proofs or calls |
| Pinned model gate | `cargo xtask verify behavior` | PASS for all 7 executable capabilities: 77 runs, 36 sampled properties, 49 reachability witnesses, 36 sensitive controls, all freshness stages PASS |
| Forge model scope | Same command; `cantos.production.forge-attempt` | PASS: 13 runs, 7 properties at seed 203 / 300 samples x 20 steps, 9 witnesses and 7 controls at 2,000 samples; exact 5-variable ITF envelope |
| Existing observer compatibility | `cargo xtask verify behavior --conformance --capability cantos.media.private-audio-admission` | PASS independently for model and existing `pure-core-synthetic-shell` observer, including original traces, mutant/observer sensitivity and freshness. This is not a Forge observer or DB/API acceptance |
| Pinned Kani | `cargo xtask verify kani --harness forge-attempt-ledger` | UNSUPPORTED: Kani 0.68.0 / CBMC 6.11.0 absent; proof NOT_RUN, nothing installed |
| L5 real I/O | Provider, Drive, Functions, live API, deployed/runtime/browser acceptance | NOT_RUN; no implementation or authority for these effects in this slice |

Forge's model bundle is
`target/verification/behavior/cantos.production.forge-attempt-564b16e7236b4fc7ac4d092bb2297201/`.
The initial failed registered run is retained separately as
`cantos.production.forge-attempt-01d3c742f85944c9ae586b79cd0e7586`; it was not relabeled PASS.
The final PR evidence records aggregate handoff limitations separately. Raw local outputs live in
ignored `artifacts/agent-runs/forge-session2-20261016/`; behavior bundles under
`target/verification/behavior/` retain raw traces, original input bytes, tool/source hashes,
commands and status. Historical qualification hashes and prior Kani measurements are unchanged.

## Deferred acceptance and residual risk

- No persistent `AttemptStore` exists. Mocks deliberately inject impossible/corrupt readbacks
  to establish fail-closed application behavior; they do not prove real medium durability,
  serialization, multi-process/local-cloud fencing or provider exactly-once effects.
- The draft Quint model is sampled and has no Forge implementation observer. Its aggregate
  call bound is not an independent per-attempt proof. Native call-by-attempt tests and the
  model dispatch guard remain separate evidence; neither is a real provider call.
- Kani's registered pin requires Kani 0.68.0 / CBMC 6.11.0. A missing bundle remains
  `UNSUPPORTED`/`NOT_RUN`; no unpinned run or historical source hash qualifies the current tree.
- #203 remains open: source relocation, supported-runtime packaging/worker contract, owner
  invocation grant, appData/OAuth, bounded Gemini/encoder, Studio flow, handoff and independent
  live acceptance are unimplemented or unrun. No credentials, deployment, billable generation,
  grants, security settings, GitHub CI or publication were used.
- Later owner: unassigned, assignment required before acceptance. Provision only separately
  authorized tools/identities, run the registered Kani gate, then implement/review each narrow
  I/O boundary with its own failure fixtures and real-medium acceptance. Missing work does not
  turn this safety-only slice into full #203 completion.
