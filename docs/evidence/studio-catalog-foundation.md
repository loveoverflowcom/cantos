# Studio catalog foundation — session 1 of 2

## Boundary and provenance

The internal Work/ContentUnit domain, PostgreSQL persistence and authenticated application
commands/queries are implemented. Studio catalog HTTP routes, raw/shared wire DTOs,
JSON schemas/fixtures, status/error contracts and HTTP interaction are **NOT_RUN / pending**
at this historical checkpoint. [Session 2](studio-catalog-api.md) records the HTTP implementation
and later verification on the same branch/draft PR. Studio Web remains outside this backend PR;
[#66](https://github.com/loveoverflowcom/cantos/issues/66) stays open.

Base: `23b25a55cbb3b8d03526721835cf0318716b605a`, freshly fetched `origin/develop` before
creating `feat/studio-catalog-api` in `/tmp/cantos-studio-catalog-api`. Full instruction source:
Library `Pasted text(20261012-144439).txt`, 300/300 lines, 7,224 bytes, `has_more=false`,
read on 2026-10-12. Actual issue #66 was read with no comments. ADRs 0001/0015/0018,
current architecture, actual contexts/contracts/migrations and repository skills were inspected.
All test fixtures are synthetic Vietnamese metadata and authorized local development tokens;
no private manuscript, actual media, provider, object store or production service is used.

## Invariants and owners

| Claim / failure prevented | Owner and oracle | Evidence level |
| --- | --- | --- |
| Stable typed identity, immutable Work/ContentUnit snapshots, closed kinds; no forced script/cast | Catalog domain constructors and 26 deterministic unit tests | type-enforced, example-tested |
| Domain decisions import no HTTP/SQL/provider/worker; other contexts expose only owning entrypoints | Repository context/table/dependency checks and seven boundary regressions | statically-checked, example-tested |
| Novel/story → chapter, podcast → episode; bounds and stale/overflow/lifecycle rules cannot bypass constructor admission | Domain tests and raw storage re-admission tests | example-tested |
| Exact script/source owner and digest pinning; legacy bytes/checksums unchanged | Editorial `owned_revision`, ingestion `source_checksum`, real PostgreSQL link/upgrade tests | integration-tested |
| Current owner/reader/editor authorization after lock waits; session expiry before commit and replay | Application use cases, security-definer actor/member SHARE locks, real lock interleavings | integration-tested |
| Exact retries return original outcomes; changed payload/duplicate ID/stale revision reject without rows | Immutable operation receipts and independent row counts | integration-tested |
| Child/membership/reorder mutations advance Work guard atomically; no partial order and archived positions/history retained | Deferrable order/head constraints, guarded heads, concurrent PostgreSQL test | integration-tested |
| Pagination is deterministic and explicitly refuses stale/cross-query/cross-actor cursors | Bounded keyset queries, global catalog epoch, actual page/permission changes | integration-tested |
| Committed snapshots/receipts survive a real PostgreSQL restart | Before/after synthetic fixtures, new Store, migration replay and exact result/row comparison | integration-tested |

Pure transitions take admitted values and return new values or typed errors. The application
owns permissions, link checks and the transaction; PostgreSQL only executes the effects and
revalidates stored snapshots/checksums. No new crate, service, ORM, generic repository or
parallel authentication implementation was introduced. Existing HTTP DTOs and routes are
unchanged. Catalog visibility has no delivery/publication semantics.

## Migration and local role

`0011_catalog.sql` applies independently after 0009. #64 reserves 0010 for separate audio
intake; no unmerged intake code/table is used. The registered versions are 1–9 and 11 (ten
migrations). Applied historical SQL files are unchanged. Upgrade tests seed the original
schema/source/script before 0011 and compare exact migration checksum, source bytes/SHA-256,
Script IR canonical bytes/content digest/export digest after upgrade/replay.

Immutable snapshot/operation tables reject UPDATE/DELETE/TRUNCATE. Head identity/kind/owner
and unit parent are immutable; snapshot heads and positions use guarded updates. The test app
role receives SELECT, append INSERT, only required pointer/session-revocation column updates,
and member edits; it receives no DDL, identity update or settled-record mutation. Actor/member
locks use fixed-query security-definer functions with `pg_catalog` search path and revoked
PUBLIC execution. Deployment role provisioning is separate; narrow grants are reproduced in
`tests/catalog_postgres.rs::grant_and_seed`.

## Affected reads and DBSP assessment

Initial workload bounds: 1,000 retained units per Work, at most 50 items per page,
128 KiB per snapshot, 4 MiB per operation result, existing five-second SQL and three-second
lock deadlines. Frequencies/production cardinality are unknown. The bounded smoke workload
has 100 synthetic works (80 Alice, 20 Bob), first page 20, one warm application request and
safe SELECT `EXPLAIN (ANALYZE, BUFFERS)` under the same application role. Acceptance targets
for this foundation are bounded rows/bytes, identical authorized IDs, a Limit plan with no disk
spill and no inconsistent page; there is no numerical latency/throughput SLO yet.

| Read | Consumer / data / order | Write dependencies and scope | Assessment |
| --- | --- | --- | --- |
| Session, actor and member | All use cases; hashed token and exact actor/work keys | Expiry/revocation/deactivation/member changes, current transaction locks | Existing session path; no cached access |
| Work / unit head | Point reads and every mutation; PK and pinned revision join | Head/lifecycle changes; exact Work permission | Indexed point read, bounded snapshot |
| Work list | Internal Studio consumer; owner/member filter, UUID keyset, limit+1 | Any catalog write bumps transactional epoch; actor/query/filter cursor | Owner/member/PK indexes; bounded first-page smoke baseline |
| Unit list / reorder facts | One authorized Work; `(position,id)` keyset; reorder at most 1,001 facts | Unit edits/create/archive/reorder; global epoch | Work/order index, exact retained slots; no OFFSET |
| Work history | One authorized Work; revision ASC keyset, limit+1 | Append snapshot and head changes; current Work permission | Composite revision PK; immutable historical payloads |
| Idempotency receipt | Actor+operation PK; exact fingerprint/result digest | Append-only command result, reauthorization on retry | Indexed point read, 4 MiB cap |
| Optional source/script/rights | Admission only through owning ingestion/editorial entrypoints | Immutable source/revision/registry; same Work owner | Exact owner keys and revisions; validates original bytes/digests |

The simpler PostgreSQL choice uses existing PK/revision indexes plus owner/member/order indexes,
keysets and bounded materialization. There was no earlier catalog query to optimize or measured
hot aggregate. DBSP decision: **defer engine adoption**; use PostgreSQL for this initial bounded metadata workload, no repeated
join/aggregate benefit has been demonstrated. No incremental engine/cache/outbox is added.
The single catalog clock trades concurrency and broad cursor invalidation for an explicit
transactional consistency model. Per-page member checks are bounded but can add round trips.
Partitioning, N+1 removal and numerical production targets require representative measured
read/write concurrency and permission-revocation evidence before changing this model.
No speedup, percentile, throughput or production-scale acceptance is claimed.

Raw output is retained locally under `target/catalog-evidence/read-baseline.json` and
`target/revision-evidence/`; one request timing is an observation, not a distribution.
The final preflight warm request and plan observations are retained in the baseline JSON;
the Limit plan uses PK index scans, an empty-membership sequential scan, no spill and 21 rows. These are one-sample observations. The EXPLAIN plan executes only
safe synthetic SELECTs. Index write/storage overhead and sustained
contention are not measured; no new engine maintenance overhead exists.

## Independent review and repairs

A separate read-only reviewer used `cantos-code-review` over the complete foundation diff.
The confirmed membership defect required an active target even when removing a role.
The command now requires an active target only when granting/changing a role. Regression:
owner grants editor → actor deactivated → owner removes → actor reactivated → access remains
`NotFound`, with exact revision/operation/member/epoch row counts. A foreign duplicate unit ID
also returns `NotFound` and never reparents the unit. Final independent review assessment: **no-actionable-findings**, with complete coverage
for this internal foundation and explicit HTTP/production exclusions.

Initial runs exposed incomplete app-role grants in the new PostgreSQL harness and sandbox
restrictions on owned loopback/process tests. The harness now includes the existing narrow
session `UPDATE(revoked)` grant required by `FOR SHARE OF s`; shared auth was not modified.
Only required local verification is retried outside that sandbox restriction. No remote CI is
requested, polled or treated as local evidence.

## Execution record

Toolchain: Rust 1.87.0; local PostgreSQL 17.4 (Homebrew), new disposable UTF-8 clusters.
Source: feature branch plus the current dirty implementation; final commit/remote SHA is in
its draft PR and handoff. Commands run from `/tmp/cantos-studio-catalog-api` with
`RUSTUP_TOOLCHAIN=1.87.0`, `TMPDIR=/private/tmp`, PostgreSQL 17 binaries on PATH as needed.

| Command | Result and scope |
| --- | --- |
| `cargo test -p cantos-api --lib catalog --locked --offline` | PASS: 32 new catalog/domain/storage/cursor tests plus 3 existing planning-catalog tests |
| `cargo xtask verify repository` | PASS: repository architecture/link/text checks and 216 Python tests |
| `cargo xtask verify rust` | PASS: rust-format, rust-lint (workspace), rust-tests (workspace); `RUST_TEST_THREADS=1` for existing ephemeral-port/process test scheduling |
| `cargo xtask verify contracts` | PASS: script-ir-reference, contract-tests, wav-reference |
| `cargo xtask verify postgres` | PASS: all existing compatibility/HTTP journeys; 15 Catalog tests plus 1 exact test after PostgreSQL restart |
| `cargo xtask preflight --changed-only` | PASS: repository, rust-format, rust-lint (cantos-api), rust-tests (cantos-api), dbsp, wav-reference, postgres; no skips |

Local logs: `/tmp/cantos-catalog-{focused,repository,rust,contracts,postgres,preflight}.log`;
raw Catalog PostgreSQL and restart logs are under `target/revision-evidence/`, with
`target/catalog-evidence/{read-baseline,restart,server-restart}.json` for safe local observations.
The final preflight repeated the corrected output path and all 16 Catalog PostgreSQL tests.
Unrelated concurrent ephemeral-port scheduling exposed an existing xtask test race during a
parallel workspace run; serialized test scheduling passed without an xtask code change.
Catalog concurrency remains explicitly exercised inside its PostgreSQL tests.

Catalog HTTP/schema/mock integration and Studio Web interaction: **NOT_RUN**, explicitly
owned by session 2 and the later Web slice. Web/mutation/mobile suites were not selected for
this metadata foundation. The selected generic `dbsp` gate passed; no business DBSP consumer
was introduced. GitHub CI: **not checked**; no wait/dispatch/rerun performed.

## Residual risk / next owner

Session 2 owns HTTP/DTO/schema/error/CSRF/body-limit/representation and mock HTTP integration
acceptance plus a final complete-API review on this same draft PR. No catalog HTTP interaction,
Web UX/accessibility, Theatre, mobile, audio intake, QC, rights clearance, publication or
production-scale benchmark is claimed. The initial clock serializes all catalog writes and
invalidates all cursors on any change; high contention requires a measured follow-up. Language
validation checks bounded structural tags, not an IANA registry. Domain restore has no
application command. See [the concrete continuation](../work-plan/011-studio-catalog-api.md).
