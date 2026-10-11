# PostgreSQL/DBSP skills — instruction evidence

Date: 2026-10-10. Scope: existing engineering, listening and review skills, one canonical
read-performance reference, routing cases and queue/documentation links. No engine deployment,
dependency, migration, query implementation or production configuration is included.

## Repository and technical provenance

The initial tree was clean on `develop`, tracking `origin/develop`, at
`ca4c9501b36e11dfbbc508a3394f71ded3211072`. Origin was verified as
`https://github.com/loveoverflowcom/cantos.git`; a fetch confirmed the starting branch.
Inspection of tracked files, the [server guide](../../services/cantos-api/README.md),
[architecture](../architecture/overview.md) and
[ADR 0001](../decisions/0001-modular-monolith.md) found no application manifest/schema or
DBSP/Feldera dependency, adapter, integration decision or benchmark. DBSP remains a
**proposed candidate / not integrated** in this inspected bootstrap.

Primary sources consulted on 2026-10-10:

- [DBSP: Automatic Incremental View Maintenance for Rich Query Languages](https://docs.feldera.com/vldb23.pdf),
  PVLDB 16(7), 1601–1614 (2023), DOI `10.14778/3587136.3587137`: computation over changes
  and incremental view maintenance.
- [Feldera documentation](https://docs.feldera.com/) and
  [the foundation's publications](https://docs.feldera.com/literature/papers/): a concrete
  incremental SQL engine built on DBSP, with tables/views and pipelines.
- [Feldera connectors](https://docs.feldera.com/connectors/) and
  [fault tolerance](https://docs.feldera.com/pipelines/fault-tolerance-overview/): delivery and
  recovery depend on actual adapters, edition and configured mode; they are not PostgreSQL
  transaction or end-to-end guarantees obtained merely by naming DBSP.
- [PostgreSQL EXPLAIN](https://www.postgresql.org/docs/current/sql-explain.html): ANALYZE
  executes the statement, BUFFERS exposes buffer use and profiling adds overhead. This
  documentation consultation does not select a PostgreSQL version for Cantos.

The working-method conclusion is conditional: evaluate incremental views for repeated
read workloads after PostgreSQL query/index improvements. It is not a decision to replace
PostgreSQL's authority or proof that DBSP optimizes every PostgreSQL query.

## Completion report

**Invariant:** Database-backed GET/read/fetch work inventories and measures the actual read
path before claiming an optimization; an incremental path preserves authoritative results,
access and consistency through feeding writes, retractions and recovery.

**Owner / boundary:** [cantos-engineering](../../.agents/skills/cantos-engineering/SKILL.md)
owns [the method](../../.agents/skills/cantos-engineering/references/postgresql-read-performance.md).
Persistence/HTTP references and `cantos-listening` route to it; `cantos-code-review` routes
to its acceptance checklist. Existing product/architecture documents retain authority.
No functional core was extracted: this change edits instructions only.

**Evidence level:** `documented`, `statically-checked` (skill/link/text structure) and
`example-tested` (the existing Python regression suite only). No database or engine
performance/correctness evidence is claimed.

**Evidence / command:** Commands run from the repository root on the source revision above
plus this documentation diff. Local tools: Python 3.13.5 and Git 2.49.0.

| Check | Command / operation | Result |
| --- | --- | --- |
| Repository links, text and skill structure | `python3 scripts/check_repository.py` | passed; 150 text files, 15 skills |
| Python regression suite, default macOS temp path | `python3 -m unittest discover -s scripts -p 'test_*.py'` | failed; 44 tests, two pre-existing assertions compare `/var` paths with canonical `/private/var` paths |
| Python regression suite, canonical macOS temp path | `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | passed; all 44 tests, unchanged code and assertions |
| Patch whitespace | `git diff --check`; `git diff --cached --check` after staging | passed |
| System skill validator, default Python | `python3 ~/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/<name>` | unavailable; PyYAML missing |
| System skill validator, isolated tooling environment | `/tmp/cantos-dbsp-validator-ixt6wr48/bin/python ~/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/<name>` for `cantos-engineering`, `cantos-listening`, `cantos-code-review` | passed; all three, Python 3.13.5 / PyYAML 6.0.3 |
| Routing R1–R30 | Author walkthrough of [routing cases](../../.agents/skills/routing-cases.md), existing ownership plus five new PostgreSQL/DBSP cases | completed as a document walkthrough only; independent evaluator not-run |
| PostgreSQL plans/oracle and DBSP fault/rebuild checks | Actual backend/schema/engine harness required | not-run; absent in this bootstrap |
| Cold/warm workload benchmarks | Actual queries, fixture generator, serving path and engine required | not-run; no measured performance targets/results |

The regression suite checks the existing repository validator and story initializer. It does
not execute database queries. Documentation changes use the foundation's comments/metadata
test exemption; the existing hygiene suite remains required. No tests matching new prose were
added. The system validator invocation is host tooling, not a new repository command.

Python checks used `PYTHONDONTWRITEBYTECODE=1`. The temporary validator environment was
created with `python3 -m venv /tmp/cantos-dbsp-validator-ixt6wr48`; its Python ran
`-m pip --isolated install --no-cache-dir --disable-pip-version-check --index-url https://pypi.org/simple PyYAML`.
No repository or system dependency was added. The canonical temp-path rerun addresses macOS
path aliases without relaxing assertions; the initial default-environment failure is retained
above. The scripts are unchanged by this commit.

### Author routing walkthrough

Snapshot: base SHA above plus this documentation diff. Evaluator: the authoring agent in this
work session. Method: trace the old/new owner links and expected decisions; no second agent,
forward task execution or automated routing score. R1–R25 retain their original owner/rubric:
the new method is conditional on actual database-backed reads or dependent writes, and does
not authorize builds, provider calls, inspection fixes or engine deployment.

| New case | Decision traced in the updated instructions | Remaining gap |
| --- | --- | --- |
| R26 | Engineering inventory/fit workflow plus listening; absent schema/runtime yields a proposed experiment and explicit unrun gates | no implemented query or runtime to execute |
| R27 | PostgreSQL baseline and fit rejection for a point lookup when maintenance has no benefit | no actual lookup workload measured |
| R28 | Feeding retractions and permission changes; same-position PostgreSQL oracle; replay and fallback acceptance | no live CDC/fault injection |
| R29 | PostgreSQL conditional progress/idempotency remains authoritative; derived reads and write overhead assessed separately | no concurrency or overhead run |
| R30 | Limited warm compute timing cannot establish API p50/p95/p99 or a 4× gain; complete comparable benchmark evidence required | no performance run |

**Provenance / artifacts:** Original fictional Vietnamese titles in the examples; no manuscript,
production dataset, private media or provider calls. The commit contains only safe instructions
and this evidence record; check output is attributable to this work session.

**Edge classes covered:** Documented requirements cover inserts/updates/deletes/retractions,
delivery duplicates/order/retries/idempotency, NULL/time/collation, sort/pagination, commit
boundaries, reconnect/restart, snapshot/rebuild, tenant/auth filters, freshness and fallback.
Mutation assessment includes write amplification, CDC/maintenance overhead and unchanged
authoritative write semantics. These are future verification obligations, not exercised
database cases in this change.

**Not covered / residual risk:** No real PostgreSQL EXPLAIN, differential query run, DBSP engine,
CDC connector, recovery experiment or benchmark ran. No API latency or throughput gain is
established; p50/p95/p99, CPU/RAM, freshness and rebuild budgets must be selected and measured
for the actual workload. Runtime skill adherence remains manual; passing structure checks does
not prove agent behavior. Independent routing evaluation and application builds are `not-run`.
The [queue](../work-plan/README.md) retains its existing order; any future integration must earn
a scoped reviewed decision and adoption evidence.
