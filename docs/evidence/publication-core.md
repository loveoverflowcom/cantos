# Publication core: verification and acceptance ledger

Boundary: [Publication v1](../../packages/contracts/publication-v1.md),
[external production ADR0018](../decisions/0018-external-audio-production.md), and
[successor handoff](../archive/handoffs/publication-discovery-handoff.md). This is PR1 of
Publication → Discovery/Playback → Library/Progress. Related #9/#10/#11 remain open.

**CORE_VERIFIED / IO_ACCEPTANCE_PENDING.** Actual typed/pure/model/fake-port/Router evidence
supports the metadata core. It is not production-ready: PostgreSQL startup failed and no real
public delivery adapter exists. Default delivery is unavailable, so the real host cannot activate
a public release. No backend synthesis, generation, mixing, encoding worker or provider call
was added. Synthetic integration audio is produced only by existing test fixture tools.

## Architecture and invariant ledger

| Level | Boundary / actual oracle | Result and limits |
| --- | --- | --- |
| L1 | Current Catalog/Production/Media ownership, shared clock/Work fence, immutable snapshots, Studio-only surface; two independent read-only source reviews | PASS source review; runtime authority still needs PG acceptance |
| L2 | Private ReleaseId/Sha256/evidence/witness/release values, checked restore, typed Result and exhaustive blocker/error mapping; compile-fail proof-type doctests | PASS type/example evidence; raw DTOs remain intentionally deserializable |
| L3 |19 publication domain tests plus existing lifecycle truth table; fixed256-case independent readiness oracle and proptest pointer/clock laws; actual bounded production Kani | PASS: no blocker shortcut, exact binding/rights/evidence admission, stale QC, wrong Work, missing bytes, expiry and digest tampering |
| L4 |5 application tests compose actual delivery observation/preparation with a named port fake, default unavailable, typed failure, second publisher CAS and failed replacement/no planned pointer effect | PASS against deterministic doubles; not SQL concurrency |
| L5 transport |1 HTTP error-mapping unit test plus2 actual Router/schema fixture tests: six authenticated routes, aliases absent, no-store and producer `passed` rejected | PASS without DB; compilation alone is not authenticated real PG execution |
| L5 PG/storage |8 compiled ignored real Router/app-role/private-object tests; additive0016, immutable tables, atomic pointer/event/receipt, duplicate/racing publish, restart/replay, withdrawal and SQL rollback | NOT_RUN after genuine PG startup FAIL; named public-delivery fake only |

Both reviewers reported no actionable findings after confirmed fixes. Domain review bound the
final source SHA-256 below; shell review inspected authorization, stale re-read, idempotent
receipts, CAS/transaction, SQL guards, pinned restoration and route ownership. Neither reviewer
executed tests; execution results below are from this implementation session.

Resolved findings included fully rehashed malformed nested evidence, Content language/media
binding, omitted rendition rights, future evidence/witness clock drift, duplicate review expansion,
new approval accepting invalid current rights, and false empty active response for superseded
withdrawal. Regressions call production admission/evaluation/prepare functions, not copied mocks.
Approval time is sampled after private byte verification. A repository gate also caught platform
Store owning the media port: the dependency moved into media-owned `PublicationService`, with
trusted HTTP host composition. Typed error/composition checker allowances are exact and65 guard
tests include negative evaluator/platform cases. Initial architecture/link FAIL remains in
`final-verify-repository.log`; corrected repository PASS is separate. No guard or verifier parser
was weakened.

## Actual command ledger

Commands run from the isolated worktree after loading its separate task tooling environment.
All logs, including failed attempts, are preserved under ignored `artifacts/agent-runs/publication-core`.
The final PR body binds the published SHA/tree; the executable source checkpoint is local
`e63afd4602d083c67b81f97fa63bba558259070a`, published with identical tree as
`534660cad31dba3f9e3fd9ef797f317c2f6918b5`, tree
`979b4d7d5a3a41bdf56f895045264b0c47d51060`. The fourth campaign commit adds evidence,
contract/handoff navigation and accurate xtask help wording, and safely merges freshly fetched
develop `bc1dbd655598b2df2526144cbb4fc735bc2f3695` (UI Avatar PR87/89).
Publication domain/lock inputs are unchanged; the fourth commit also makes the pointer-ID
comparison explicitly `&str`, fixing opt-in DBSP-feature type inference without changing policy; no unpublished storage branch was merged.
Git shell push lacked authentication; connected Git-data publication preserved exact blobs,
modes and trees with expected-head fast-forward updates. Local/remote commit IDs differ because
the connector authors commits; matching trees are explicitly fetched and verified.

| Exact command | Actual result / retained log |
| --- | --- |
| `cargo test --locked -p cantos-api --lib publication` | PASS26 (19 pure publication,1 lifecycle,5 application,1 HTTP mapping); `l3-l4-final-merged-tests.log` |
| `cargo test --locked -p cantos-api --test publication_contract` | PASS2 actual Router/raw-contract tests; `l5-composition-http.log` |
| `cargo clippy --locked -p cantos-api -p cantos-contracts -p xtask --all-targets -- -D warnings` | PASS, compiles the ignored new integration targets; `l5-composition-clippy.log` |
| `cargo test --locked -p cantos-api --doc` | PASS12 compile-fail/type docs; `l2-doctests-composition.log` |
| `cargo test --locked -p xtask kani::tests` | PASS10 registry/evidence guards; `l3-kani-registry-composition.log` |
| `cargo xtask verify kani --harness publication-commit` | PASS exact1harness,19checks,6covers, unchanged lock; `l3-kani-final-code.log` |
| `cargo xtask verify rust` | FAIL overall: format/lint PASS; workspace tests reach xtask127PASS/17FAIL baseline private-input fixtures; `final-verify-rust-last.log` (fresh develop included) |
| `cargo xtask verify contracts` | PASS script-ir-reference/contract-tests/wav-reference/actual audio-inspection; `final-verify-contracts-last.log` (fresh develop included) |
| `cargo xtask verify repository` | PASS repository306 Python tests/architecture/links/JSON/skills/whitespace; `final-verify-repository-merged.log` (fresh develop and final docs included) |
| `cargo xtask verify dbsp` | PASS opt-in API feature Clippy/tests/docs after explicit pointer string typing; `final-verify-dbsp-fixed.log`; no view adopted |
| `cargo xtask verify postgres` | FAIL `pg_ctl` startup after successful `initdb`; schema/test commands NOT_RUN; `l5-postgres-attempt.log` |
| `cargo xtask preflight --changed-only --base bc1dbd655598b2df2526144cbb4fc735bc2f3695` | UNSUPPORTED missing Gradle8.14.3 for cmp-ui-preview/theatre-cmp-demo; all other selected steps NOT_RUN; `final-preflight-merged.log`; no skip/bypass requested |

The17 xtask failures report private inputs inside a Git checkout: the managed `/tmp/.git`
causes the existing safety guard to reject synthetic fixture roots. They are not publication
assertion failures and are not suppressed. PostgreSQL17.11 started loopback TCP but its log
reported `could not create Unix socket ... Operation not permitted`, then fatal startup failure.
The owned cluster log is retained at
`target/revision-evidence/cluster-1791887422171545691/postgres.log`.
No denied-access workaround, socket configuration change, fixture guard weakening or unique-data
cleanup was attempted. An initial executor process-creation disconnect is a separate NOT_RUN
attempt, not evidence of a DB result; executor reconnect succeeded. Mac/Python killpg baseline
reports are separate evidence, not the cause inferred for this dot environment.

An initial Kani raw proof succeeded but its evidence adapter rejected malformed multiline
property-description blocks. The harness-local expected boolean was made explicit; the adapter
was unchanged. The original evidence-admission FAIL and subsequent source-bound PASS are retained. A final
CLI attempt with unsupported `--kernel` failed before proof execution and is retained as
`l3-kani-final-invocation-fail.log`; the documented actual command uses `--harness`.
The optional DBSP-feature build exposed an ambiguous Option<&str>/Option<&String> comparison;
the exact pointer-ID projection is now explicit `as_str()`. Original `final-verify-dbsp.log`
FAIL is retained separately from the corrected rerun. This does not enable an incremental view.

## Formal scope and exact inputs

Kani0.68.0/CBMC6.11.0/CaDiCaL3.0.0, bundled rustc1.100.0-nightly
`8925ea358` (nightly2026-08-21), x86_64-unknown-linux-gnu, API library/no default features.
Production Rust remains1.87.0 (`17067e9ac`), Cargo1.87.0, Python3.12.14, PostgreSQL17.11,
FFmpeg/FFprobe7.1.5, Linux6.18.44, Intel Xeon Platinum8573C. No paid/live provider was used.

Harness `contexts::media::domain::publication::kani_proofs::publication_pointer_and_rights_fail_closed`
calls actual `advance_pointer` and `rights_window_valid`. Independent fullu64 pointer/version and
fulli64 start/end/now values have no assumptions, stubs or exclusions; unwind2, default safety,
overflow and unwinding assertions remain enabled. It establishes exact expected-version conflict,
SQL-bigint exhaustion without wrapping and half-open nonnegative validity. It does not model all
readiness combinations, authorization, SQL schedules, storage, crash recovery or listening.
All6 named cover classes are satisfied. The one explicitly approved unreachable assertion is the
closed-enum catch-all unexpected pointer error; no meaningful acceptance/rejection class is excluded.

Final evidence directory `artifacts/agent-runs/kani-17cdd526b37244ea8a05e3e8f41c4133`
contains raw output and metadata with transitive/config/input identities.
Publication source SHA-256:
`8dbb31fc61eedfe7893ffb63a137c46b8abe6c978abc93f9fce17ab775d60393`.
Cargo.lock before/after:
`8cf5d58a6be8481148dae8daf9abec112d6e4ce72740eea56bd87f23108a8e6c`.
Kani lacks Cargo `--locked`; unchanged hashes are observed evidence, not that switch.

## Read inventory, PostgreSQL baseline and DBSP fit

Actual consumers are readiness/active GETs plus clearance/approval/publish/withdraw subject,
replay and pointer reads in `contexts/media/publication_application.rs`. Queries are in
`contexts/media/postgres/publication.rs`, existing lifecycle queries, Catalog production-content
and Production audio-brief; schema/index authority is0011–0014 plus0016. The owning actor/unit
is resolved before scoped release reads. No public list/page/read model exists in this slice.

| Read | Scope/cardinality/order | Consistency and index shape |
| --- | --- | --- |
| Candidate subject | exact candidate→submission→rendition plus one owner Work/unit/pinned Content/Brief | Existing primary/unique keys; shared Catalog clock and Work lock |
| Current review/rights/approval | latest3 review kinds and1 clearance/approval per exact candidate; server sequence DESC | Existing lifecycle indexes;0016 unique(candidate_id,sequence) supports backward limit1 |
| Active pointer/release |1 unit pointer,1 exact release, exact pinned Content/Brief and withdrawal existence | unitPK/releasePK/withdrawalPK; final current-owner/rights check and pointer consistency recheck after I/O |
| Operation replay | at most1 actor+operation+kind; rights/approval actor+operation | Unique receipts; exact request/subject comparison; no reactivation |

Read frequency, representative tenant scale and hot-path timings are UNKNOWN. Before an accepted
baseline, use synthetic100Work×20units,30candidate revisions/unit with3reviews and10rights/
approval decisions, skewed active reads and distinct owners, at concurrency1/8; cap the disposable
experiment at5minutes/1GiB and1000warm samples. Proposed local targets (not measured SLOs):
metadata-only p95≤50ms/p99≤100ms, full32MiB readiness p95≤500ms, mutation p95≤750ms,
current access/expiry freshness at request time, and≤20% metadata/write regression. Revisit
these budgets against actual hardware/production workloads; no API latency or throughput claim
is inferred from proof/unit duration.

The first real PG baseline prerequisite failed startup. Authorized timings and
`EXPLAIN (ANALYZE, BUFFERS)`, row estimates/actuals, buffer/spill/WAL costs, cold/warm/concurrent
runs and improved-index comparison are therefore NOT_RUN. Queries already use bounded point
lookups and matching unique keys; there is no repeated aggregate/N+1 unit list to maintain.
**DBSP decision: reject for this point-lookup metadata slice**, on shape/consistency/operational
cost grounds, not a measured speedup. Defer reconsideration to the actual Discovery aggregation
consumer after a PG baseline. No cache/engine/CDC/checkpoint was introduced.

Feeding changes: pointer activation/withdrawal, clearance/approval insertions, owner/archival,
Content/Brief/metadata/review changes, private/delivery integrity and clock expiry. Current
PostgreSQL access stays authoritative; no-store and second-phase rechecks avoid an asynchronous
projection. The audit feed is not global commit order or an incremental watermark. Actual
isolation/query latency, fallbacks under provider failure and live serving remain acceptance gaps.

## Safe machine-readable provenance

[Results and artifact hashes](publication-core-results.json) bind the final production source
files, exact source checkpoint/tree, command outcomes, Kani raw/metadata and retained logs.
Historical failures remain evidence, not passed assertions. Final commit SHA/tree lives in the
PR body because a document cannot embed its own Git commit identity.

## Required acceptance before public enablement

- Run migrations/replay and least-privilege application grants on real PostgreSQL, including
  the independent0015/0016 registry reconciliation. Preserve historical SQL/checksums.
- Execute the compiled eight PG tests: duplicate/two publishers, exact restart replay,
  immutable republish, stale QC/revoked rights/missing bytes, default blocked delivery, active
  and superseded withdrawal, failed replacement preserving prior valid release, and injected
  SQL rollback/retry. Add actual process/cluster crash/acknowledgement and expiry-during-I/O cases.
- Integrate real publication-class immutable delivery/integrity/retention, then prove missing
  or revoked/expired rights never expose staging/public bytes; validate signed lifetime and ranges.
  [Private storage PR90](https://github.com/loveoverflowcom/cantos/pull/90) is an independent
  dependency, not included here, and supplies no public promotion authority.
- Capture real authorized PG baseline/plans and current-access race evidence. Resolve existing
  fixture startup restrictions without weakening guards; rerun full repository/rust/preflight.
- Establish production identity/grants, legal distribution policy and named listening review.
  Studio Web/browser/a11y and Theatre discovery/playback/library/progress remain separate slices.
