# Optional DBSP platform and legacy capture retirement

Source-bound audit and local implementation record, 17 October 2026. Baseline:
`451d3a068ebfd0e0ce54d7d3bb7f61057e7a5884` on `develop`, checked against the live remote.
This is not production activation or a decision to add a business projection. PostgreSQL
remains authoritative under [ADR 0018](../decisions/0018-external-audio-production.md).

## Current source versus historical claims

| Component / claim | Source at baseline | Assessment |
| --- | --- | --- |
| Real Rust DBSP 0.198.0, opt-in | `services/cantos-api/Cargo.toml:8–23`, `Cargo.lock` | Actual optional `dbsp-experimental`; default builds do not compile it |
| Signed Z-set deltas, grouped counts | `src/platform/incremental/dbsp.rs:57–79`, `mod.rs:28–82` within the API package | Actual circuit: members and two Boolean flag counts per group; not a SQL optimizer, PostgreSQL extension or general projection host |
| Duplicate / unmatched retraction / conflicting member detection | `incremental/dbsp.rs:67–79,102–108` | Separate row/member integrity operators; an errored engine must be discarded |
| Checked output and atomic count-view application | `incremental/dbsp.rs:130–150`, `mod.rs:129–187` | Pure/core behavior with native tests; not SQL transaction conformance |
| Snapshot / whole-transaction paging / frontier | `incremental/mod.rs:226–395` | Actual pure values/decisions; caller must supply complete committed transactions |
| Freshness and shadow decisions | `incremental/mod.rs:398–451` | Actual mapping of caller-provided facts, not a PostgreSQL fence executor |
| PostgreSQL maintainer, JobSummary consumer and development CLI | No callers outside the incremental module; former `postgres::durable::summary_view` and `production-jobs` were deleted | Retired, not a current integration gap to fill by restoring TTS |
| Migration0009 capture/outbox | `migrations/0009_durable_step_changes.sql`, checked migration registry | Actual enabled trigger at baseline, irrespective of Cargo feature; unused by current source |
| Runtime privileges | `infra/vps/postgres/runtime-grants.sql:3–19` | Current deployment grant policy revokes legacy table access from `cantos_app`; not evidence that every deployment applied it |
| Checkpoint / monitoring / retention | In-memory `DbspCounts`; no current adapter/consumer implementation | Not implemented; no durable engine state, feed consumer registry, pruning policy or production observability |
| Tests and gates | `incremental/{mod,dbsp}.rs`, `xtask/src/gate.rs` | 20 native example/property tests plus a compile-fail doctest at baseline; `verify dbsp` runs feature lint/API tests. Former PostgreSQL-to-DBSP tests are retired |

[#58](https://github.com/loveoverflowcom/cantos/issues/58) is closed and
[#60](https://github.com/loveoverflowcom/cantos/pull/60) is merged, as read from GitHub during
this audit. Their original working development consumer is historical under accepted ADR0018;
neither reinstates its serving configuration. [ADR0017](../decisions/0017-experimental-dbsp-job-summary.md)
and [the old experiment](dbsp-job-summary.md) retain their original evidence.

The old benchmark's small-job regression, fallback under continuous writes and unmet read
benefit budget apply to JobSummary only. They establish no performance conclusion for Catalog,
Discovery or any other future projection. No new SQL/DBSP performance measurement ran here.

## Forward-only retirement boundary

`0029_retire_durable_step_capture.sql` disables the exact
`public.durable_steps.durable_steps_change_capture` trigger. It performs no data update,
delete, truncate, table/function drop, privilege change or historical SQL rewrite. Existing
captured rows, sequence, indexes, source rows and other legacy integrity triggers remain.
The migration registry records it in the existing advisory-locked transaction; rerunning
`migrate` checks its checksum and does not reapply it.

This stops new maintenance cost for the orphaned projection, without claiming a measured
latency or storage gain. Existing outbox storage is not reclaimed. The remaining source-row
identity, transition, deletion and truncation protections are still required.

The retained outbox is incomplete after this boundary. Re-enabling its trigger does not
restore missed changes. Never attach the retired maintainer to it or roll back to the old
TTS runtime against this schema. A future consumer must define a fresh authoritative snapshot,
its own committed-change protocol, continuity/replay and serving/fallback contract. There is
no automatic production rollback/re-enable operation in this change.

The command is transactional, but can wait for a conflicting table lock. PostgreSQL17's
[ALTER TABLE documentation](https://www.postgresql.org/docs/17/sql-altertable.html) describes
trigger enable/disable and its locking. The migration actor owns DDL; `cantos_app` cannot
toggle the trigger. No live installation, row count, external subscriber or scheduled/manual
writer was inspected, and no production migration ran. Before a separately authorized rollout,
the operator must inventory those deployment facts, stop any historical consumer/writer,
retain the normal backup and choose an appropriate maintenance/lock budget. Discovery of a
consumer of this feed blocks rollout until its retirement or replacement is reviewed.

Repository dependency search covered the current API source, tests, migrations, scripts,
xtask and infra. Only historical migration0009 writes `durable_step_changes`; current
runtime source has no caller. Migration0008 still refers to durable sources for historical
clip constraints, so those sources and their safety triggers are intentionally retained.
Repository search cannot establish external database dependencies or authorize data removal.

## Architecture and follow-up review boundary

The retained platform is a two-flag grouped-count sample. Do not introduce a generic
projection trait, schema or service without a concrete consumer. A future adapter owns the
business projection and typed mapping; domain code stays independent of DBSP. The API must
continue to authorize using current PostgreSQL facts before consulting a read model.

The following source findings are bounded follow-up inputs, not implemented fixes in this
retirement slice:

| Finding | Proposed next step / scope |
| --- | --- |
| Dead `ReadModel::VARIABLE = CANTOS_JOB_SUMMARY_READ_MODEL` and parser remain in `incremental/mod.rs:454–490` | Remove obsolete configuration after confirming no callers; do not create a general environment knob without a consumer |
| `Member` documentation still uses job/step terminology, while its payload fixes `[bool; 2]` | Describe the actual grouped-count sample and keep projection-specific semantics out of its platform contract |
| `DbspCounts::step` mutates its runtime before checking errors; discard is a caller precondition | Reproduce reuse after an integrity failure, then make the instance refuse every subsequent step with an exact typed outcome; do not claim a production defect with no current consumer |
| `Snapshot::parse` discards empty xip tokens, accepting malformed comma lists | Add exact malformed-position regressions before tightening construction; verify PostgreSQL's actual text representation and preserve valid empty lists |
| Freshness docs omit request-snapshot ordering relative to bootstrap publication | State the old READ COMMITTED/view-before-statement precondition or require independently established snapshot visibility; frontier comparison alone does not prove bootstrap rows visible to an older snapshot |

No DBSP capability is registered in the [Quint catalog](../../specs/behavior/catalog.json).
The private-admission model covers a different owner; no model-only test is used to certify
this SQL migration. Native deterministic/core tests and real PostgreSQL remain the oracles.

A new consumer remains **defer** until a current query, real frequency/scale and performance
budget are identified. Measure the authorized SQL query and its plan on disposable synthetic
data, compare simpler query/index improvements including write cost, and only then compare
DBSP under the same commit position. The comparison must include read distribution, write
amplification, lag, memory, fallback capacity, replay/restart and authorization/revocation.
Neither Catalog nor Discovery is selected by this audit.

## Verification scope

`tests/legacy_dbsp_postgres.rs` seeds the original0001–0009 schema with SQL-valid synthetic
legacy bytes, then invokes the actual migration registry. Those retired records have no
current business constructors and are never submitted as content or media. Tests cover:

- Retained source/outbox bytes and all original migration checksums across upgrade and rerun.
- Only capture disabled; subsequent inserts and succeeded transitions produce no new outbox rows.
- Legacy identity/transition/deletion/truncation and captured-row immutability still enforced.
- Application role cannot forge, update, delete, truncate the feed or re-enable capture.
- Transaction rollback restores trigger state and rows; a clean installation retains the
  table/function with capture disabled and an empty outbox.

`cargo xtask verify postgres --suite legacy-dbsp` owns a new disposable PostgreSQL cluster,
refuses inherited database configuration and `--keep`, and requires nonzero passing tests.
The full PostgreSQL gate also runs this target. Its focused PASS, when observed, accepts only
these migration semantics; it does not accept the complete PostgreSQL/API/media/restart gate,
engine/feed conformance, a business projection or production rollout. Run results and exact
source identity belong to the sequential review handoff and eventual PR.
