# Compatible editorial handoff evidence

Date: 2026-10-10. Review boundary: port the local editorial capabilities onto develop's
existing Script IR, immutable revision backend and Studio without introducing another schema
or save protocol. Part of [010](../assurance/script-editor-acceptance.md) / issue #1; no issue
acceptance or production deployment is claimed.

## Invariant, owner and provenance

Every preserved source and editorial review remains attached to immutable owner-scoped facts.
A denied, stale, corrupted or interrupted operation cannot expose another creator's source or
partially record a review. Retries preserve the original accepted revision/review identity.

Base: `origin/develop` at `36ac0a1`; branch `feat/editorial-handoff`, with the listed changes.
The user selected develop's backend and Studio after upstream advanced during the earlier
rebase. The original implementation remains on `backup/script-revisions-pre-rebase` at
`92e046c`, and the historical rebase result on `backup/script-revisions-rebased` at `b4336c4`.
Neither archive is a compatible replacement for the newer develop backend.

`script_ir` stays unchanged and pure. `revisions` owns the exhaustive permission table and pure
review/replay/stale decision. `postgres/editorial.rs` resolves current facts and performs short
transactions through the existing Store. HTTP maps raw DTOs/errors. Operator provisioning is
trusted local I/O with pure bounded-input checks, outside the HTTP role. No additional service,
port trait or domain crate is introduced. Source registration is thin CRUD, so no separate
state machine is extracted. [ADR 0004](../decisions/0004-editorial-handoff.md) records the
compatible decision and the one pinned direct dependency, already present in the lockfile.

## Claim and oracle ledger

| Claim / failure mode | Oracle | Evidence level |
| --- | --- | --- |
| Develop's existing save, Studio and hardening survive | Existing Script IR, Studio reducer, API and PostgreSQL suites | example-tested; differentially-tested; integration-tested |
| Review replays precede stale-head rejection; key reuse cannot rebind a revision | Pure decision table and concurrent real HTTP requests | example-tested; integration-tested |
| Review is an explicit owner fact, not a save/publication side effect | Editor denial, pinned first-review fields, stale unreviewed revision conflict | example-tested; integration-tested |
| History is ascending, bounded and currently authorized | Real HTTP Schema assertions, cursor pages, bounds, revocation and body omission | integration-tested |
| Sources stay byte-exact and private | NFD/CRLF original, independent SHA-256, anonymous/other-owner/unlinked denials | integration-tested |
| Stored corruption never becomes a new review or history result | Tampered revision through history/review; source checksum corruption | integration-tested |
| Review and receipt are atomic; retries recover from failure | Receipt INSERT trigger failure, zero reviews afterwards, same-key retry | fault-injected; integration-tested |
| Settled source/review/receipt cannot change | UPDATE, DELETE, TRUNCATE rejected with SQLSTATE 55000 | integration-tested |
| Migration upgrades preserve old bytes and rollback failed extension | Seed schema 0001, save revision, inject 0002 collision, retry and replay | integration-tested |
| Operator credentials use the existing session contract | Actual CLI actor/issuance/revocation, real authentication, invalid expiry rejection | integration-tested |
| Process/database recovery preserves handoff facts | Kill/reap HTTP, restart PostgreSQL, replay migration, compare exact source/review/history and save replay | fault-injected; integration-tested |

## Executed checks

Working directory: repository root. Rust 1.87.0 (develop's compiler) and PostgreSQL 17.11,
provisioned in the local `postgres:17-alpine` disposable container `cantos-rebase-test-pg` on
loopback port 55871. Tests create isolated `cantos_test_*` databases; runtime uses the
non-owner `cantos_app` role. All content is original synthetic fixture text; no provider,
external identity service, private manuscript or media store is involved.

```sh
cargo +1.87.0 fmt --all -- --check
cargo +1.87.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.87.0 test --workspace --locked --offline
CANTOS_TEST_CLUSTER_URL=postgresql://cantos_test_admin:cantos_test_only@127.0.0.1:55871/postgres cargo test -p cantos-server --test revisions_postgres --locked --offline -- --ignored --test-threads=1
python3 scripts/reference_script_ir.py
RUSTUP_TOOLCHAIN=1.87.0 python3 scripts/mutate_script_ir.py
python3 scripts/check_repository.py
python3 -m unittest discover -s scripts -p 'test_*.py'
git diff --check
```

Workspace checks: 30 non-database tests (2 wire, 4 server unit, 15 Script IR, 1 Studio contract,
8 Studio reducer tests); 12 ignored database tests run explicitly and all pass. Python suite:
60 tests pass. Independent reference agrees for both unchanged accepted goldens. Formatter and
Clippy pass on Rust 1.87.0. The bounded mutation run killed all 9 substitutions (0 survivors, timeouts or unviable cases);
repository checks pass for 211 text files and 15 skills, and `git diff --check` passes.
The ordinary workspace run is not database evidence. CI now runs the ignored PostgreSQL suite
in a dedicated service job; no remote result is claimed here.

Local raw output is under ignored `artifacts/script-revisions/`: `handoff-tests-msrv.log`,
`handoff-clippy-msrv.log`, `handoff-postgres.log`, `handoff-mutations.log`, `host-before.log`,
`host-after.log` and `recovery-snapshot.json`. The local HTTP recovery driver is
`/tmp/cantos-handoff-recovery.py`; commands were `python3 /tmp/cantos-handoff-recovery.py before`,
`podman restart cantos-rebase-test-pg`, then `python3 /tmp/cantos-handoff-recovery.py after`.
It uses the existing restart fixture, kills/reaps each HTTP process and compares complete JSON
responses after database restart and migration replay. No live browser/device result is claimed.

## Affected read inventory and PostgreSQL baseline

| Consumer / query | Scope, order and dependencies | Decision |
| --- | --- | --- |
| History endpoint | Current script access, revision PK range after cursor, max 50; one review range query, no per-row SQL; all exports re-admitted | Keep authoritative PostgreSQL |
| Review response/retry | Locked owner head, actor/key receipt PK, review PK joined to immutable revision | Transactional PostgreSQL; no asynchronous view |
| Source endpoint | Current script access, owner/source PK and EXISTS over immutable revision links; checksum verification | Keep authoritative PostgreSQL |
| Credential/source operator commands | Trusted operator account; source registry and preserved text commit together; session revocation affects later authentication | No read cache or engine |

Read frequency and production cardinality are unknown. A small warm baseline used the real HTTP
app role, 100 synthetic revisions, 20-row pages, five warmups and 30 timed reads at concurrency
one. A scoped p95 target of 250 ms was set before timing; observed history p50/p95/p99 were
24.146/36.012/36.370 ms. The 100 sequential saves observed 15.077/21.071/35.387 ms. These are
local observations, not a speedup, capacity claim or a representative workload acceptance.
`read-baseline.json` retains the workload/sample counts and timestamps are not used for ordering.

Actual query texts were extracted from `postgres.rs` and `postgres/editorial.rs` for
`EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` under `SET LOCAL ROLE cantos_app`. Plans are retained
as `history-plan.json`, `source-plan.json`, `reviews-plan.json`; observed execution times were
0.199, 0.167, 0.103 ms respectively, with 20, 1 and 0 output rows. Profiling times are not API
latency. The review plan's miss does not establish matched-review scaling. Primary/composite
keys already support the bounded ranges and owner lookup. No additional covering index or
materialized cache was justified by this small sample; large evidence fan-out remains unmeasured.

DBSP decision: defer, consistent with the [existing admission decision](script-revision-dbsp-decision.md).
These are scoped immutable ranges/point lookups and current access decisions, with no measured
hot repeated aggregate. No engine or CDC adapter is introduced. PostgreSQL commits remain the
freshness boundary; authorization changes are checked per page and retry. Pages can include
new commits between requests and do not pretend to be a frozen full-history snapshot. A future
hot list consumer must earn representative cold/warm, skew, concurrency and write-overhead
comparisons before changing the serving boundary.

## Bounded self-review

Assessment: `no-actionable-findings`. Scope: local tracked/untracked change against BASE
`36ac0a1`, snapshot SHA-256
`89c8c8bb0eff17e42e279be74edee2642f4589df99eead968bea8b98f980d4e5` (34 paths before this report
update). This is an author self-review using `cantos-code-review`, not independent assurance.
Coverage: complete-for-declared-scope: new server/operator/migration paths, existing authorization
and save consumers, added shared DTO/Schema/fixtures, tests, CI, documentation and skill changes.
Workers, Theatre, CMP and rendered Studio were not reviewed because their code is unchanged.
The source fingerprint remains applicable to implementation; this report update changes docs only.

| Claim | Owner / target | Required by | Evidence held / level | Disposition |
| --- | --- | --- | --- | --- |
| Preserve existing Script IR/Studio compatibility | script_ir / shared API / server | user-selected develop authority; contracts/README.md | unchanged module and goldens; 30 workspace tests / differentially-tested, example-tested | held |
| Private source integrity and current access | operator / Store / HTTP | AGENTS.md architecture boundaries; contracts/studio-v1.md | real source/permission/corruption tests / integration-tested | held |
| Pinned owner review, retry identity and atomic rejection | revisions / Store / PostgreSQL | contracts/studio-v1.md; engineering immutability | pure table, duplicate HTTP requests, receipt-insert failure / example-tested, fault-injected, integration-tested | held |
| Existing rows survive migration upgrade and process loss | PostgreSQL / host | AGENTS.md verification; revision persistence rules | old-schema upgrade, failed DDL, real host/DB restart / integration-tested, fault-injected | held |
| History bounds and authority survive paging | Store / HTTP | engineering read-performance workflow | Schema/bounds/access assertions; small real HTTP/plan baseline / integration-tested | held |
| Credentials are provisioned outside runtime privileges | operator / sessions | existing development identity boundary | real CLI issuance/revocation/authentication / integration-tested | held |
| Skills/docs distinguish implemented and planned behavior | documentation / skills | AGENTS.md scope and source of truth | local link/JSON/skill checks / statically-checked | held |

No test oracle or existing golden was weakened. Raw DTOs are not trusted witnesses: permission,
number/UUID bounds, stored canonical integrity and source hashes are checked before durable effects.
SQL rows, serde construction, administrative commands and the migration upgrade path were traced.
Additional maximum-size/concurrent-read measurements remain a non-blocking assurance opportunity;
no production performance or eligibility claim depends on the small baseline above.

## Script IR compatibility and residual risk

Read/write schema remains `{0.1.0}`; golden c1 digests and all content/export/speech bytes are
unchanged. Fixture set: two accepted documents and 24 rejected cases. Migration 0001 is unchanged;
0002 never rewrites accepted revisions. No changed SpokenContent or audio regeneration scope.
The new v1 API definitions are additive; existing Studio wire fields are unchanged.

No importer, full text editor, new browser handoff controls, mobile, audio, provider or publication
flow ran. Explicit review grants no legal or production permission. Registry-only legacy source
references are not preserved source text until an operator records them. No large-source/page,
concurrent load, cold benchmark, database failover/backup restore or sustained resource claim is
made. Maximum-size history pages still decode bounded full exports for integrity; production
sizing and rate limits need measurement. Production identity/TLS and durable browser recovery
remain in the existing queue. These checks do not complete issue #1/010.
