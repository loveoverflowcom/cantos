# Listener Library / Progress verification

2026-10-13. **Domain/use-case/default-auth Router checks pass; actual account-sync, PostgreSQL,
issuer and Web/CMP playback acceptance are pending.** Machine-readable [results](listener-library-progress-results.json)
bind source and raw evidence hashes. [Contract](../../services/cantos-server/contracts/theatre/library/v1/README.md)
and [consumer/recovery handoff](../work-plan/listener-library-progress-handoff.md) own behavior.
No broad issue is closed, no campaign PR is merged, and no CI completion is awaited.

## Scope and publication lineage

Fresh develop `df7d62fecf0023cb557a3051ae75918cedcd2610` plus exact inherited
[PR93](https://github.com/loveoverflowcom/cantos/pull/93) head
`b07ab14e5ec52ebc711ea272dfa2bcb61c09fdfd` / tree
`e3a596a9563fb4538cdb0e7bbf25f2162bc2209b`, including PR91 Publication.
Fresh checks found develop advances to `3623afd3ddcac64752df2fc54811a748c3fa39e9`
(PR94 Web shapes) and `389c5cb2760ec31b20f5dc816670e9914455313e` (PR96 Web Library/Account
visuals), incorporated by normal feature merges. The latter still uses the session-only mock
reducers and has no account transport; no v2 API/navigation expansion is added by this backend. PR91/93 were not merged by this session.
Private Storage PR90 is independent. Identity mapping PR92 is not a dependency and does not authenticate.

The four own logical commits are contracts/refined core; application/named fake; PostgreSQL/Axum;
and this evidence/recovery handoff. Two normal develop integration merges preserve the already-develop UI work. Git-data
publication preserves exact blobs, modes, trees and inherited parents; published commit hashes may
change because connector commit metadata differs. The final PR and fetched head/tree own remote
identity, while the result file retains local source identities and proof hashes.

## Verification order and limits

| Level | Evidence | Result / limit |
| --- | --- | --- |
| L1 static/type | strict private/refined register state, sealed Listener scope; 18 compile-fail doctests; repository dependency policy; workspace rustfmt/clippy | PASS, including default-deny auth barrier; no issuer verifier |
| L2 contracts | Rust DTO/schema fixture round trips, required nullable members and closed errors/events; owning docs/HTTP resources | PASS; OpenAPI/generated Kotlin drift not delivered |
| L3 domain | bounds/revision/rewind/completion/history/Saved tests, independent full-u64 classification property and two-device schedules; real Kani production kernel | PASS, 131 checks and 7 reached covers; no SQL/engine/account claim |
| L4 use cases | 23 focused core/auth/application/cursor tests total; named transactional-memory adapter drives production LibraryService; independent Saved desire/revision schedule oracle | PASS, not real persistence or account-sync acceptance |
| L5 adapters | 3 Library and 4 inherited Theatre default-auth Router regressions; whole workspace Rust tests compile/run API; 7 real Library PostgreSQL/Router sources registered/compiled but ignored | default-auth Router PASS; real PostgreSQL NOT_RUN; native/client acceptance NOT_RUN |

The levels were pursued core-first. Final affected checks re-ran after review corrections. A point-read
schema gap, omitted required-null member, Restart restoration gap, post-fence expiry clock and
filter-scalar corruption barriers were fixed, rather than attributed to baseline. The separate
read-only reviewer found the new PostgreSQL runner's undefined URL variable; it was fixed. The
scalar constraint regression was strengthened so revision/order metadata and digest are coherent
and only the targeted saved/completion scalar is corrupted.

L4 exercises ten identical retries → one register/revision/receipt; immutable key reuse/stale
sequence; Save/Unsave plus old responses; two devices, rewind/delayed checkpoint/explicit resolution;
actual-start-only history; clear preserving progress/Saved/session tombstones; A/B isolation,
service restart, access loss and exact audio identity; rollback/uncertain retry; latest-completed
Work suppression and cursor route/account/limit/frontier/restart schedules. The fake now matches
page ordering/frontiers, but its transaction mechanics remain a double. Physical playback and
SQL equivalence are not established by these tests.

The seven real PostgreSQL source regressions cover ten retries and concurrent duplicate receipts,
first-write and same-base races, Save/Unsave historical responses, real conditional writes,
receipt-trigger rollback, clear/preservation/session dedup and authenticated synthetic Router
account isolation, replacement/withdrawal without substitution, targeted scalar CHECK rejection,
and latest-completed SQL/Router suppression. They reuse the canonical publication fixture inside
`cfg(test)` so production never gains a mock-auth principal constructor. The owned existing
`scripts/test_postgres.py` gate runs these in its disposable cluster; it is not a new runner.

## Final commands

Local raw logs remain in `artifacts/agent-runs/library/`; hashes are committed in results.

- `cargo test -p cantos-api --lib contexts::listening --locked --offline`: **23 PASS, 7 ignored**;
  real PG sources compile, without being executed.
- `cargo test -p cantos-api --doc --locked --offline`: **18 PASS**.
- `cargo test -p cantos-api --test theatre_contract --test listener_library_contract --locked --offline`:
  **7 PASS**, default-deny auth/route/Origin/strict no-store boundaries only.
- `cargo clippy -p cantos-api -p cantos-contracts -p xtask --all-targets --locked --offline -- -D warnings`:
  **PASS** after fixing an own-source bool assertion warning.
- `cargo xtask verify contracts`: **PASS**, including strict Library fixtures, existing contract
  regressions, independent WAV reference and two real Opus format-inspection tests. This is not
  listening QC, signed delivery or account sync.
- `cargo xtask verify repository`: **PASS** before final docs; final docs recheck is recorded in results.
- `cargo xtask verify rust`: **format PASS, workspace lint PASS, workspace tests FAIL**.
  API lib reports 210 passed / 53 ignored; the xtask stage reports 130 passed / 17 failed, all
  matching the previously observed private-input fixture failures (`private input must stay outside
  any Git checkout`). No new source compile failure is mislabeled baseline. Overall Rust stays FAIL.
- `cargo xtask verify kani --harness listener-reconcile`: **PASS**, actual Kani0.68/CBMC6.11,
  exact production `classify`/checked successor, full u64/boolean inputs, unwind2, independent
  partition oracle, 131 checks / 7 satisfied covers, lock unchanged. Proof ran on clean local
  source commit `b6ba15d61291dd7016892c7cb9def14573eea4af`; all model-input hashes remain identical
  after the Web-only develop integrations and documentation changes. No assumptions/stubs,
  unchecked arithmetic or unreachable whitelist weakens this harness. It does not prove SQL,
  identity, unbounded reconciliation schedules or physical playback.
- `cargo xtask preflight --changed-only --base origin/develop`: **UNSUPPORTED** because installed
  Gradle8.14.3 is absent; all other selected steps are **NOT_RUN in this orchestration**. The
  independent commands above retain their own outcomes. Dry-run selection also remains NOT_RUN.
- `cargo xtask verify postgres`: **NOT_RUN this session**. Earlier cluster startup reached `/tmp`
  Unix-socket EPERM and was denied; no retry, alternate socket or bypass was attempted. Inherited
  failed startup evidence is retained. SQL runtime/constraints/plans/throughput are unaccepted.

## Latest develop integration repair

The final PR96 merge recheck found a **new inherited source compile failure**, separate from the
17 known fixtures: Leptos account view layout-query depth130 on Rust1.87. The fail-before
`final-rust-after-pr96.log` is retained and hashed. A fresh develop check found no correction.
The integration adds only two lines in Theatre Web `lib.rs`: a comment and the compiler-suggested
`recursion_limit=256`. No component, behavior, navigation or Library domain was changed.
Independent review confirmed the minimal diff and the generated-layout diagnosis. Repaired full
Rust compiles/runs API210 passed/53 ignored and Theatre Web91 passed, then fails on the exact
original17 xtask fixture names; format/lint remain PASS. `cargo xtask verify web` also passes WASM clippy, Studio/Theatre Trunk builds and the
UI-system preview build; this is compile/build evidence, not browser/account/native acceptance. All194 Kani model inputs and58 Library source hashes remain unchanged.

## Remaining acceptance

See the ordered handoff queue for identity, authorized disposable PostgreSQL, actual two-device
Web/CMP playback/outbox/reconciliation, retained-release delivery, typed client generation and
performance plans. In particular controlled expiry while waiting on the Catalog fence, representative
multi-Work keyset traversal/differential SQL, full host/database restart and real A→logout→B lifecycle
remain required. Source inspection establishes transaction design, not successful transactions.
No benchmark, DBSP speedup, fake server, guest merge, analytics or live-provider claim is made.
