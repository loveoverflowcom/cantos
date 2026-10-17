# Prompt-4 conditional DBSP decision

> Later update (#58): an experimental, feature-gated DBSP adapter now exists for the durable job
> summary ([ADR 0017](../decisions/0017-experimental-dbsp-job-summary.md)). This record's DEFER
> for script-revision reads is unchanged, and the observations below describe its own checkout.

- Date: 2026-10-10
- Decision: **DEFER**; admission to an engine experiment is not met.
- Implementation status: DBSP adapter proposed / not integrated; runtime experiment **NOT_RUN**.
- Source inspected: local `develop` at `ff6e3cc5c8f8506c904179b2d54ef1a6ebec1c39`, clean at entry.
- Architecture status: [ADR 0003](../decisions/0003-script-revision-persistence.md) remains
  **proposed** for production adoption. This record accepts no engine or deployment ADR.

## Decision and evidence

Retain the existing PostgreSQL serving path and migration. The
[read inventory](script-revision-reads.md) and current
[Store](../../services/cantos-api/src/platform/postgres/mod.rs), [routes](../../services/cantos-api/src/interfaces/http/mod.rs) and
[schema](https://github.com/loveoverflowcom/cantos/blob/3e6c183d3d616e8ec4a782ba788b4b70d0c6e49a/services/cantos-api/migrations/0001_script_revisions.sql) supply head/history point
reads, authentication, save replay and evidence resolution. The session–actor join is bounded
by a token lookup; no implemented list, catalog, repeated large join or aggregate consumer
exists. Save-side evidence fan-out is a write-path concern, not an incremental read consumer.
Inventing a Theatre endpoint or a join benchmark would not establish demand for this slice.

[Prompt-3 measurements](script-revision-performance.md) cover 303,480 measured HTTP requests
on PostgreSQL 17.4 at 100 and 5,000 scripts. The disposable covering index was **rejected**:
at 5,000 scripts warm member p95 was 15.83 → 15.75 → 15.84 ms across baseline, candidate and
return baseline, about 0.5% improvement against the predeclared 15% threshold. Initial index
storage added 768 KiB and membership-update WAL rose 44.1%. Those observations justify keeping
the current path; they neither reject DBSP generally nor prove production capacity. The
[protocol](script-revision-benchmark-protocol.md) owns the full workload and limitations,
including shared-buffer reset with uncontrolled OS cache, rather than disk-cold execution.

In prompt 4, both retained raw-report checksums, all 144 per-run result records, fixture/phase
metadata, source/query hashes and curated plans were checked against the
[safe result record](script-revision-performance-results.json). Current Store, HTTP, migration
and benchmark hashes match the recorded inputs. This is an audit of historical measurements,
not another benchmark or a fresh PostgreSQL integration run. Prompt-2
[persistence evidence](script-revision-persistence.md) and
[review](script-revision-review.md), and the prompt-3
[review](script-revision-performance-review.md), retain their original execution boundaries.

The missing consumer, hot improved baseline and serving/freshness contract are sufficient
reasons to defer. No engine version, connector, CDC stream, checkpoint, rebuild, fallback or
delivery guarantee has been selected or tested. Engine-specific official version/connector/
serving verification belongs to the admission process below, before any future experiment.

## Admission checklist and measurable reopening criteria

The owning method is the engineering
[PostgreSQL/DBSP workflow](../../.agents/skills/cantos-engineering/references/postgresql-read-performance.md).
Reopen a bounded experiment only when **every row has a linked evidence artifact**. These are
admission gates, not claims of implementation or automatic adoption thresholds. The future
workload owner must choose numerical budgets before changing its path; the prompt-3 index
threshold is not a DBSP threshold.

| Gate | Evidence needed to reopen | Current disposition |
| --- | --- | --- |
| Concrete repeated consumer | Actual handler/consumer, authoritative SQL, result contract, role/tenant filters, ordering/pagination and every feeding write/retraction/access change | Missing: current reads are points; no maintained join/aggregate consumer |
| Measured hot path | Representative cardinality/skew, observed or justified read/write rates and fan-out; at least three baseline repetitions with API p50/p95/p99, throughput at named concurrency, CPU/RAM and plans; a predeclared latency/throughput/resource budget miss attributable to repeated join/aggregate work | Missing for an eligible consumer; current bounded warm runs fit their investigation budgets |
| Improved PostgreSQL comparator | Measured query/index/statistics/batching/pagination alternatives where applicable, equivalent responses/access semantics, write/WAL/storage costs and baseline-return control; select the best justified PostgreSQL path | Prompt 3 rejected one point-read index; it is not a baseline for a future join/aggregate |
| Committed-position and access contract | Implementable snapshot-to-change-position handoff; result version/watermark meaning and atomic visibility; explicit read-after-write, lag, clock/expiry and current authorization rules | Missing: current direct PG reads have no derived serving contract |
| Verified engine boundary | Exact official engine build/version/edition and input/output connectors, supported SQL/time semantics, serving interface, replay/checkpoint guarantees and matching official references; reproduce support in disposable environments | Not selected; no dependency, connector or serving API is assumed |
| Cost and recovery budgets | Numerical API gain and allowed write regression, CPU/RAM/state/storage/WAL/backlog ceilings, freshness/lag bounds, rebuild time/resources, outage duration and fallback concurrency/timeouts; workload duration/sample counts and stop conditions | Missing for a candidate; current local PG budgets do not budget an engine |
| Reviewed adapter proposal | Owning slice and maintainer-reviewed scoped proposal/ADR, domain-independent adapter, authorized PG fallback and a test/benchmark mapping for each relevant edge class below | Missing; ADR 0003 remains proposed and grants no engine adoption |

More scripts alone, the existence of a small authentication join, a vendor benchmark or an
index rejection does not reopen the experiment. Revisit when actual importer/editor or later
Theatre work introduces the qualifying measured consumer. A failed gate keeps **DEFER** and
records which artifact is missing; it does not justify a fabricated workload.

## Future verification protocol — proposed, NOT_RUN

Keep this protocol within the owning slice; extend its concrete runners when a candidate
exists. It defines no executable engine command, production CDC configuration or new schema.

1. Pin PostgreSQL, engine/connector versions, source/query/schema hashes, fixture seed,
   effective non-secret configuration and budgets. Use a fresh disposable PostgreSQL cluster
   and synthetic fixtures representing the admitted consumer. Preserve the authoritative
   transaction, permission and response semantics; keep engine dependencies outside domain
   and Script IR modules.
2. Establish one consistent committed PostgreSQL snapshot and its proven change-position
   handoff. For each seeded change sequence, stop writes at a named committed barrier and
   drain/pin engine output to that **same** position before comparing; a merely later or
   unknown watermark is not an equivalent snapshot. Exercise bootstrap while writes continue
   separately. Compare the real PG query and actual API status/body, bags/counts, sort and
   pages under the same actor/tenant context. Record minimized failures, not just timings.
3. Map each row of the canonical
   [differential/recovery checklist](../../.agents/skills/cantos-engineering/references/postgresql-read-performance.md#differential-and-recovery-checks)
   to real test names, failpoints and commands for the selected adapters. It owns updates/
   retractions, duplicate delivery/retries/ordering, NULL/numeric/time/Vietnamese semantics,
   sort/pagination, transactions, restart, snapshot/rebuild, authorization/tenant isolation
   and freshness/fallback. Record a workload/adapter reason for any inapplicable row. A
   documented protocol or fake connector remains insufficient for engine adoption.

The current READ COMMITTED membership/actor checks do not retroactively revoke an already
authorized transaction. Any stronger revocation barrier is a separate reviewed contract;
a future view must not silently weaken current access. For a revision consumer, compare full
export/base/actor/operation identity, including equal c1 with changed rights/provenance. c1
excludes that metadata, and
neither c1 nor e1 alone is revision identity or a CDC watermark. Prompt-3 WAL deltas are write
cost observations, not an implemented serving checkpoint.

4. Only after correctness passes, compare original PG, improved PG and the engine through the
   entire API/transport/materialization path. Retain cold-state/bootstrap and warm steady-state
   preparation, repeated samples/variance, p50/p95/p99, throughput and component CPU/RAM.
   Include small/skewed/broad/selective cases, read-heavy and low-read/high-write mixes, bursts,
   API write latency/throughput, CDC/maintenance/WAL/backlog/storage, rebuild and outage fallback
   capacity. Separate shared-buffer cold from OS/disk cold. Record raw artifacts outside Git.
   Adopt only if real-boundary equivalence, freshness, recovery and all predeclared benefit/cost
   budgets pass; otherwise reject that candidate or defer with the missing evidence named.

## Completion evidence and limits

**Invariant:** an incremental serving experiment requires a real measured consumer and an
aligned authoritative correctness/freshness contract; missing prerequisites cannot become
an integration claim. **Owner / boundary:** engineering read-performance admission and the
010 queue; PostgreSQL remains authority. **Evidence level:** documented decision/protocol,
source inspection, and statically-checked repository hygiene. No domain rule is implemented,
so type/core extraction and new behavioral regression tests are inapplicable.

Prompt-4 commands and result logs are recorded in the
[bounded pre-commit review](script-revision-dbsp-review.md). Native/WASM builds, Rust/PG
integration, mutations, runtime UI/mobile/audio, a new read benchmark and every engine/CDC/
recovery/serving benchmark are **NOT_RUN** in this documentation-only slice. Existing prompt-3
passing results are historical, not rerun claims. The future edge classes above are documented
obligations with no new runtime coverage.

The [queue](../work-plan/README.md) continues bounded importer/editor work in 010. No production
serving/migration, dependency, credential/remote or GitHub execution status changes; no push/PR
or issue closure. Prior prompt-1/2/3 commits remain intact. This completes conditional prompt 4;
the separate final review of the complete prompt sequence remains a parent-owned follow-up.

**Residual risk:** production traffic/capacity, an eligible maintained workload and engine
semantics/cost/freshness/recovery remain unknown. Production auth/TLS, rights eligibility, full
UI/accessibility and native mobile acceptance remain outside this decision. DEFER is a fit and
admission decision, not a measured engine rejection or production approval.
