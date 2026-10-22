# ADR 0017: Experimental DBSP job-summary read model

- Status: historical; backend consumer superseded by [ADR 0018](0018-external-audio-production.md).
  Incremental work is DEFER under [#384](https://github.com/loveoverflowcom/cantos/issues/384).
- Original decision: opt-in local experiment for issue #58, never production adoption.
- Source snapshot: `b2611c3db3896ce9733280a319008924ee8bf974`; [original decision](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/decisions/0017-experimental-dbsp-job-summary.md).

## Decision and retained invariants

The retired TTS `JobSummary` consumer used DBSP 0.198.0, a PostgreSQL committed feed,
whole-transaction publication and authorized SQL fallback. Its `xid8`/`xmin` snapshot frontier
was engine-specific; row IDs do not prove commit order on Turso. Derived state never grants
identity or authorization, and unknown freshness must not serve a result as current.

The worker/consumer, incremental module, feature and `verify dbsp` gate are removed.
No mandatory DBSP experiment or committed-feed implementation remains on the native roadmap.
Use the [Turso read method](../../.agents/skills/cantos-engineering/references/turso-read-performance.md):
SQL/index first, selective cache only after measurement.

## Historical evidence

[Original tests and benchmark](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/evidence/dbsp-job-summary.md),
[raw result](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/evidence/dbsp-job-summary-results.json) and
[platform retirement audit](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/evidence/dbsp-platform-review.md) preserve unique
source-bound measurements and limits in Git history. They establish no current native acceptance
or performance improvement. Registered SQL/checksum history and migration inputs stay immutable.
