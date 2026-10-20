# Independent Ultra review of domain assurance

## Disposition and source boundary

Reviewed on 2026-10-13 after the three ordered implementation phases and fresh
develop integration. The initial review head was
`f824a5a2a5237e1d0343fd937cd954c5bc21fc91`, tree
`6a91aae5e1c2a2c36f91055bc5890cc04bca8855`. Fetched develop
`909169b6fca661d5a2108298548e2a6b5616a145` was already an ancestor; no stale-base
review or additional integration was needed. Its upstream PR73, PR77, PR83 and
PR85 changes remain intact. PR75's merged steps 1–4 and PR81's steps 5–11 were
reviewed as cumulative obligations, without undoing upstream changes.

The initial disposition was **changes requested** for one confirmed P2 workflow
defect. That defect is fixed and published in
`3367980818be17caaffe45b30eedfd8dc3ffe76e`, tree
`e32fb3de5347a98dd32ee4ab75665029aa94a6d6`. Independent read-only review of the
four-file repair found no actionable regression. The reviewed domain/client
boundaries have no additional confirmed actionable finding in this campaign.

Implementation review is complete. Acceptance is **not an overall PASS**: the
full Rust gate still fails and real PostgreSQL/browser/device acceptance remains
pending. Source review, ordinary tests, compile-fail barriers, property tests,
mock execution and three local bounded proofs retain their distinct meanings.

## Confirmed finding and minimal repair

**P2 · logic · fixed: empty Cargo home omitted effective global configuration.**

Affected owners: `xtask/src/kani.rs::prepare_configuration` and
`scripts/verify_kani.py::configuration_identity`. With `CARGO_HOME=""`, both
owners previously treated the home as the checkout directory. Actual Cargo
ignores an empty override and resolves `HOME/.cargo`. Its effective global
`config` and `config.toml` could therefore be absent from configuration approval
and freshness checks. If neither usable environment value existed, the workflow
also omitted the platform fallback rather than refusing an unresolved identity.

An independent observation used the installed bundled Cargo, a disposable empty
workspace and disposable `HOME/.cargo/config.toml` containing the harmless flag
`build.rustflags=["--cfg=ultra_empty_home_probe"]`. With an empty `CARGO_HOME`,
`cargo -Z unstable-options config get build.rustflags` read that flag while the
old Python identity omitted its file. This agrees with Cargo's
[home resolution implementation](https://github.com/rust-lang/cargo/blob/master/crates/home/src/env.rs).
The reproduced defect is unapproved/unbound effective configuration; this report
does not claim that a false production proof was actually obtained.

The repair aligns Rust approval and Python identity:

- A nonempty `CARGO_HOME` wins, including supported checkout-relative values.
- An empty or unset override uses nonempty `HOME/.cargo`.
- If neither is available, the fixed-model workflow refuses before probing.
- Existing ancestor candidates, both filenames, absent-file identities, bounded
  reads, exact approved bytes and pre/post outcome checks remain intact.

Only those two implementation files, their Python regression file and the xtask
environment contract changed. No production dependency/toolchain, kernel,
authorization rule, browser flow, service guard or CMP implementation changed.

### Regression evidence

Before the repair, two new Rust tests failed semantically: the empty override
resolved to `/example/checkout/` instead of `/example/home/.cargo`, and an
unresolvable home was silently omitted instead of rejected. The initial new
Python cases produced five failing subcases in a 75-test run. An earlier helper
extraction compile error is retained in the logs but is not the semantic repro.

The final three added Rust cases and four added Python cases check empty/unset
fallbacks, explicit absolute/relative override precedence, unresolved-home
refusal and a stale global config after approval. The latter adapter regression
asserts that no version or driver probe starts. All 10 Rust Kani unit tests and
77 Python verifier tests pass. Real compiled xtask also refuses the disposable
global rustflags and unresolved home before proof; those expected FAIL/NOT_RUN
observations are regression checks, not successful model-checker executions.

The proof rerun used clean local commit
`3791364cc6802642238997aa1db2551784bd051e`. GitHub's published commit above has
the exact same tree, independently verified after explicit feature-ref fetch.
Publication used the previous remote head as a fast-forward lease. Equal-tree
binding is explicit; no separate proof execution at the GitHub commit is claimed.

## Cumulative invariant review

The [ledger](domain-invariants.md) remains the complete inventory. Source review
traced admission, reconstruction, consumers and shell rechecks rather than
assuming a private type or a test name establishes end-to-end authority.

| Scope | Reviewed production boundary and independent evidence | Disposition / retained limit |
| --- | --- | --- |
| W1–W4, U1–U5 | Raw serde DTOs remain raw; strict wire admission, PATCH states, nonblank control witnesses and exhaustive projections retain consumers. Contract and UI suites, independent codec/audio gates and compile-fail barriers ran. | No actionable finding. Browser interpretation, IME, focus and assistive technology are not established by native rendering/builds. |
| C1–C5, I1–I2, S1–S3 | Private positive storage-bounded revisions, immutable validated import and Script IR have no public unchecked serde/default construction. Stored content/import/script re-admits facts and checks identities/canonical bytes. | No actionable finding. Real SQL types/constraints/transactions remain unrun; digests do not establish extraction authenticity or rights. |
| E1, A1, P1 | Editorial reconstruction rejects zero/overflow; save/replay shells recheck exact revision and current access. Bound adaptation source consumes the admitted pair; frozen raw context is re-admitted. Production planning binds current owner; actual approval reloads pinned facts under locks and current clock. | No actionable finding. Planning witness has no live dispatcher consumer. The new owner witness parity test is ordinary parity; the existing four-cell owner-transfer rejection table is an independent oracle. |
| M1, M2 | Submission window reaches create/restore/rendition/replay; actual shells retain transaction-time grant, currentness, digest/bytes and replay bindings. Verified audio owns inspected immutable bytes; checked payload ranges are consumed by the real parser. | No actionable finding. Window and range proofs are narrow numeric propositions, not digest/authentication/whole-parser/SQL or listening evidence. |
| D1 | Count application stages updates atomically and uses checked counts; the real experimental DBSP circuit shares the admission predicate and recount oracle. Page/frontier admission is numeric and monotone at its implemented boundary. | No actionable finding. Optional in-memory experiment only; no live domain consumer or feed/bootstrap completeness claim. |
| F1–F2 | Private exchange/intent/ticket and original dispatch/reconciliation identity reject stale or mismatched acknowledgements. Prepare replies check declared bindings and exact server-preserved expiry; ambiguous production outcome survives later denial. Save findings bind their actual snapshot. | No actionable introduced finding. Current backend authorization remains authoritative; actual browser lifetime/cookies/async/IME acceptance is pending. |
| T1–T2 | Admitted resume reaches player-session/progress state; raw input is checked before generation/effects. Current catalog/source binding precedes resume mutation, and checked probe advance precedes transport abort. Root player and PR83 navigation/progress ownership remain intact. | No actionable finding. Synthetic session-only mock engine; actual abort/unmount/audio/native/publication/persistence is not established. |
| X1–X2 | Typed gate selection, fixed direct bundle invocation, configuration approval, bounded resources, exact harness/check/cover parsing, lock/model provenance and owned child cleanup were traced. The empty-home defect above was reproduced and repaired. | No remaining actionable finding in the repair. Endpoint hashing assumes frozen inputs; mutate-and-restore races and universal transitive tool/cache authenticity remain outside the contract. |

The three production proof harnesses call their actual functions, with independent
widened integer specifications, no assumptions/stubs/input exclusions, unwind 2
and default safety/unwinding checks. All named covers succeed; expected catch-all
unreachable assertions and fixed formatting checks do not exclude meaningful
business classes. The exact evidence and residuals are in [the proof record](kani.md).

Two nonblocking evidence/hardening opportunities remain explicit:

1. Theatre's `rejected_resume_admission_keeps_positions_and_route_unchanged` test
   checks constructor rejection followed by untouched context assertions; it
   does not itself execute the rejected context command. Actual player-session
   raw-restore rejection and valid-but-rebound catalog command tests supply the
   relevant stronger effect coverage. Do not upgrade this one test's claim.
2. The pre-existing Studio editor saved-reply projection accepts matching IDs
   without independently admitting impossible revision jumps or canonical
   export/digest facts under a corrupt/misbound upstream assumption. Normal
   backend save/replay admits those facts. This campaign adds no new defect in
   that boundary; future client hardening must preserve server normalization.

Neither observation justifies unrelated feature changes or a fabricated fix.
PR73's Safari/default-kind/owner-producer/chip changes, PR83's Khám phá/Của tôi
destinations and root player ownership, and PR85's CMP mock/docs are preserved.

## Fresh executed verification

Initial API/client/contracts/DBSP/doc checks ran on the clean integrated review
head. The four-file fix changes only workflow ownership; those production sources
are identical afterward. Focused workflow, full Rust and Web checks ran on the
clean published repair. The actual proof rerun ran on the equal-tree local commit
identified above. Counts below overlap and must not be summed as an all-green run.

| Executed command / scope | Actual result |
| --- | --- |
| `cargo test --locked -p cantos-server` | PASS: 311 executable + 10 doc barriers; 136 ignored executable cases. |
| `cargo test --locked -p cantos-studio -p cantos-theatre -p cantos-ui` | PASS: Studio 166 + 1 doc, Theatre 86 + 3 doc, UI 51 + 2 doc; zero ignored. |
| `cargo xtask verify contracts` | PASS: all four gates, including 61 contract cases and actual synthetic-byte WAV/Opus inspection. |
| `cargo xtask verify dbsp` | PASS: 326 executed, 136 ignored; real experimental circuit and API doc barriers, not a live integration. |
| `cargo test --workspace --doc --locked` | PASS: all 16 compile-fail construction barriers. |
| Eight focused xtask module test selections | PASS: 59 tests (Kani 10, gate 11, CLI 7, selection 15, pipeline 3, report 1, tools 8, CI 4). |
| Python verifier/configuration/process suite | PASS: 77 tests; mocked outputs are not proofs. |
| `cargo xtask verify repository` after repair | PASS: 302 Python tests, 475 text files, 18 skills. |
| `cargo xtask verify kani` after repair | PASS: Catalog 100 checks / 6 covers; WAV 20 / 7; submission window 10 / 8. |
| `cargo xtask verify rust`, serial, published repair | FAIL: format and workspace warnings-denied Clippy PASS; tests 802 passed / 17 failed / 136 ignored; later doctests not reached. |
| `cargo xtask verify web`, published repair | PASS: all four WASM lint/build/UI-preview gates. No browser/pixel/runtime acceptance. |
| Repair-only changed preflight, PostgreSQL and CMP skipped | FAIL at the same 17 service fixtures; repository 302, format, Script IR reference and workspace lint PASS; later gates NOT_RUN. |
| Final report-only `cargo xtask verify repository` | PASS: 302 Python tests, 476 text files, 18 skills; documentation changes only. |
| Final changed preflight against `origin/develop`, with `--kani` and PostgreSQL/CMP skipped | FAIL: 802 passed / 17 failed / 136 ignored; repository 302, format, Script IR reference and workspace lint PASS; later proof/Web/audio/DBSP/mutation rows NOT_RUN. |

The 17 full-Rust failures occur in unchanged upstream service fixtures at the
private-input/Git-ancestor guard previously reproduced against clean develop.
The three added Rust tests explain the increase from the prior 799 passed to
802. No guard, assertion or private-input policy was weakened. The earlier
phase2 18-failure run remains historical FAIL; its additional port post-drop
assertion passed focused repeats and serial runs, but its cause is unconfirmed.

Final report-only repository/preflight results are also recorded in the ledger's
Ultra checkpoint. A stopped preflight never inherits independent
Kani/contracts/DBSP/Web PASS rows. Explicit PostgreSQL/CMP skips remain NOT_RUN.
No remote CI was inspected, dispatched or polled; mutation was not rerun.

## Evidence retention and remaining acceptance

Ignored local evidence lives under
`artifacts/agent-runs/2026-10-13T0806Z-ultra-review/`: raw logs, exact command/head/
dirty-status/exit records and `evidence-index.json` with SHA-256 log identities.
Fail-before logs, expected refusal runs, failed Rust/preflight runs and historical
evidence are retained. The new [proof record](kani.md#ultra-review-reexecution)
binds raw proof output, metadata, equal source tree, config/model and lock hashes.
This report is committed; ignored logs are local execution evidence, not a claim
that every reader can download them from GitHub.

Blocked acceptance owners remain unassigned:

1. Run real disposable `cargo xtask verify postgres` where the documented socket
   startup is permitted. The earlier EPERM denial, including reviewed escalation,
   remains NOT_RUN; no alternate route or policy bypass was attempted.
2. Resolve the upstream service-fixture environment without weakening the guard,
   then run full Rust and required preflight to their actual terminal outcomes.
3. Run actual Studio async/cookie/conflict/IME/accessibility and Theatre probe
   abort/unmount acceptance. Real audio/native/publication/persistent listener
   behavior remains unimplemented or unrun, beyond this mock hardening.

Only the named core scopes are `CORE_VERIFIED`; required IO acceptance remains
`IO_ACCEPTANCE_PENDING`. The three 64-bit kernels are not whole-application,
32-bit/WASM, parser, DBSP, rights, database or UI formal verification. PR81 remains
Draft/open and is not merged into develop by this review.
