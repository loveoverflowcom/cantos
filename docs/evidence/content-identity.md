# Content Identity evidence and boundary

Baseline: `develop` at `b620109d93f7ced11628ce380c282ccf7a9c351c`, after Catalog PR68.
One outcome: immutable content beneath Catalog identity, including non-scripted prose/recording
units. [ADR 0019](../decisions/0019-content-identity.md) and
[Content Identity v1](../../packages/contracts/content-v1.md) own semantics.

## Claim / oracle ledger

| Claim | Failure mode | Evidence |
| --- | --- | --- |
| Stable IDs, metadata and content sequences are distinct | title/reorder changes accepted input | typed constructors; real HTTP metadata/content separation |
| Normalization/digests are deterministic | Vietnamese NFC/NFD causes false identity | literal Python hashlib golden; exact NFC/LF tests |
| Full revision pins rights and attribution | equal body silently renews rights | pure full-vs-content digest sensitivity |
| Novel/story/podcast requires no Script IR | recording needs fake dialogue | all categories through real HTTP/PostgreSQL |
| Retry binds complete normalized command and both guards | lost response duplicates or false conflict | original receipt after head advances; shared Catalog operation reuse |
| Current owner/member/session gates read/replay | revoked member reads private content | app-role HTTP access/replay; inherited Catalog wait/expiry tests |
| Source/script retain exact owning evidence | forged/cross-owner links or moving script head | real ingestion/editorial entrypoints; old rows/bytes/digests asserted |
| Head/history/receipt/Work fence commit atomically | half append or concurrent winners | two connections at clock barrier; fault after content insert; independent row counts |
| Facts stay append-only; loads re-admit them | modified or corrupt snapshot trusted | admin UPDATE/DELETE/TRUNCATE refusal; corrupt DTO → 500 |
| Migration/restart preserve history | exports or applied checksums rewritten | additive upgrade/replay; real PG/HTTP process restart with exact content and retry oracle |

## Execution record

Worktree: `/tmp/cantos-content-identity`; Rust 1.87.0; PostgreSQL 17.4 Homebrew.
Synthetic Vietnamese text/metadata only. No provider, external object store, private manuscript,
audio, browser/device, rights clearance or publication acceptance was exercised.

- Focused Catalog/domain tests and the Content Identity literal contract test: passed locally.
- `TMPDIR=/private/tmp RUSTUP_TOOLCHAIN=1.87.0 cargo xtask verify postgres`: **PASS**.
  Real disposable PostgreSQL, app role and actual Router/server code; no mocked SQL authority.
- Remaining Rust/contracts/repository/preflight checks and review are pending at the remote
  checkpoint; final summaries will replace this statement before handoff.

The first DB start was sandbox-blocked at loopback bind; authorized runs use only fresh disposable
clusters. Test-oracle errors in TRUNCATE SQLSTATE and EXPLAIN decoding were corrected before the
passing gate. The literal rejection fixture also caught Serde's empty recording-variant field
bypass; the production DTO now refuses the field and the same fixture passes.

Raw logs are outside Git at `/tmp/content-identity-*.log`. Existing artifacts under
`target/revision-evidence` and `target/catalog-evidence` preserve suite output/restart oracles.
`target/catalog-evidence/content-read-baseline.json` records actual app-role SELECT plans and
warm Router/API timings. No story/audio is committed.

## Affected read inventory and DBSP decision

| Read / consumer | Query and bound | Write/access dependencies |
| --- | --- | --- |
| Studio content head; append facts | unit PK → head composite FK → revision PK; one ≤128 KiB snapshot | append; Catalog archive/member/session changes |
| Pinned content; replay integrity | exact `(unit_id, revision_id)` immutable row | revisions append only; current Work access required |
| History / future brief selection | `(unit_id, revision > after)`, ascending limit+1 ≤51; response omits bodies | appended sequences; actor/access checked per page |
| Append linked evidence | existing ingestion checksum/receipt and editorial owned export/rights entrypoints | evidence immutable; exact owner/active session |
| Existing Catalog Work/pages | same schema/DTO; content append advances Work/epoch | new content mutations share global clock |

Local workload target: 100 synthetic revisions under one unit, 30 warm head/history requests,
history limit20; p95 below 100 ms as a development diagnostic, not production acceptance.
API timings include auth/row locks/materialization. EXPLAIN uses the actual safe SELECT under
the application role, and is not API latency. Cold-cache, multi-owner skew, production frequency,
p99/throughput and contended write latency remain unmeasured. The unique `(unit_id,revision)`
index is the simple PostgreSQL baseline; there is no new cache or aggregate projection.

DBSP: **defer**. Point and bounded ordered history reads have no repeated aggregate to maintain.
A future measured hot projection needs its own scale/freshness/access and PostgreSQL comparison.
No incremental engine, stale cached permission, index benchmark gain or production latency is
claimed. The global serial fence is an inherited initial-scale limitation.

## Residual risk and next owners

Production Brief, producer credentials, accepted audio and publication are absent. Studio
content UI/interaction/accessibility is unclaimed. Catalog link fields remain compatibility
metadata and may diverge from managed content; downstream code must use ContentRevision.
No source/script permission grant, content backfill or rights clearance is implicit.
Intake PR69/migration0010 remain independent. [Session 2 handoff](../work-plan/content-identity-production-brief-handoff.md)
specifies immutable pinning, registry reconciliation and next acceptance.
