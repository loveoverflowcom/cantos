# Studio Catalog API — session 2

## Boundary and provenance

Work item: [011](../work-plan/011-studio-catalog-api.md), [#66](https://github.com/loveoverflowcom/cantos/issues/66).
Review boundary: authenticated Catalog backend/HTTP contracts; Studio Web catalog acceptance
remains separate. Same branch `feat/studio-catalog-api`, draft [PR #68](https://github.com/loveoverflowcom/cantos/pull/68),
base `develop` at `23b25a55cbb3b8d03526721835cf0318716b605a`.
The fetched session-1 checkpoint is `76d27356ac003a64b511a66f0d5609795d18f98c`.
Local and remote SHA matched before edits; no reset or branch replacement was used.

Library attachment `Pasted text(20261012-144439).txt`, 7,224 bytes, was read in full
(300/300 lines, `has_more=false`), alongside the live issue, PR metadata, repository skills,
ADRs 0001/0015/0018, handoff, actual domain/application/persistence and wire conventions.
Session-1 [evidence](studio-catalog-foundation.md) is historical, not HTTP verification.
All fixtures are synthetic Vietnamese text and local development identities; there are no
private manuscripts, real media, providers, storage services or deployed databases.

## Invariants, owners and evidence ledger

| Invariant / prevented failure | Owner and oracle | Evidence level |
| --- | --- | --- |
| Only the current Studio mount serves real Catalog routes | HTTP router; refused legacy/Theatre/Admin routes | example-tested, integration-tested |
| Client input cannot set owner; invalid independent fields are reported together | Shared raw DTOs, Catalog input admission and domain error collectors | type-enforced, example-tested |
| Podcast and plain story metadata require no Script IR, cast or generation | Kind matrix and actual create/get HTTP responses | example-tested, integration-tested |
| PATCH omission keeps fields; null clears only optional links; retry preserves original intent | Domain patch values, resolution after transaction locks and operation receipt lookup | example-tested, integration-tested |
| No silent revision overwrite, conflicting operation reuse, duplicate identity or partial reorder | Typed errors; SQL snapshots/receipt/epoch counts and concurrent HTTP requests | integration-tested |
| Current owner/editor/reader permissions and expiry survive lock waits and every replay | Existing Studio sessions, transaction-bound access and expiry checks | integration-tested |
| Archive preserves history without preventing later access revocation | Permission-only Work transition; archived removal/read/history/replay regression | example-tested, integration-tested |
| Pages have deterministic keys, bounded length and refuse changed epochs/scopes | Actual Work/unit/history pages; mutation and cross-query cursor tests | integration-tested |
| Source/script binding pins owner and exact immutable identity/digest | Owning ingestion/editorial APIs; historical-byte SQL assertions | integration-tested |
| JSON-only transport has bounded bodies and stable errors/statuses | Real router, shared schema validators and literal fixtures | example-tested, integration-tested |
| Committed catalog snapshots and original replies survive restart | Fresh router/Store after PostgreSQL restart and loopback server restart oracle | integration-tested |
| Catalog remains provider-free and separate from audio intake | Existing context/table/dependency checker and module compatibility regressions | statically-checked, example-tested |

The adapter maps raw input and outcomes; it adds no durable decision core because those rules
already belong to the Catalog domain/application. Patch application is a pure function; current
permission, link ownership, clocks and I/O remain in the transaction shell. No crate, ORM,
generic repository or separate authentication implementation was added. Shared DTOs do not
admit domain values. Settled snapshots/receipts remain append-only.

## Decisions and compatibility

The complete [wire contract](../../packages/contracts/catalog-v1.md) documents twelve method/path
combinations exclusively under `/api/v1/studio`. Success is `201` for creates including exact
replays, `200` for reads and other mutations. DELETE means archive and carries an explicit
operation/revision JSON body. Empty PATCH and required-field null are rejected. Shared literal
fixtures and real responses are checked against the hand-authored schema; only JSON is offered.

Accepted prompt choices: existing tokio-postgres/Studio sessions, closed Work/unit kind matrix,
soft archive, optional immutable links, thin Axum and bounded keysets. Rejected as unnecessary:
generic repositories/OOP scaffolding, ORM replacement, public access inferred from visibility,
MessagePack advertisement without an implemented Catalog decoder, backend TTS and production jobs.
No new ADR is needed: these choices implement the existing surface/product boundary.

Minimal existing Studio error-status matches accept the additive shared error variants using
existing fallback messages; no Catalog view, navigation or user interaction was implemented.
Applied migrations 0001–0009 and Catalog 0011 are untouched in session 2. Version 0010 remains
absent and reserved by independent #64; the runner supports the gap. Deployment grants remain
separate from the narrowly scoped synthetic application role used by tests.

## Actual local verification

All applicable gates passed on the final implementation tree (session-1 checkpoint plus the
session-2 diff), using Rust 1.87.0, Python 3.13.5 and disposable PostgreSQL 17.4. Commands run
from `/tmp/cantos-studio-catalog-api` with `RUSTUP_TOOLCHAIN=1.87.0`,
`TMPDIR=/private/tmp`, `RUST_TEST_THREADS=1` and PostgreSQL 17 executables on PATH.
Local loopback/process tests were run with the authorized sandbox exception.

| Exact repository command | Actual result |
| --- | --- |
| `cargo xtask verify repository` | PASS; 218 Python tests, links/JSON/context boundaries and whitespace |
| `cargo xtask verify rust` | PASS; workspace format, clippy and tests |
| `cargo xtask verify contracts` | PASS; script-ir-reference, contract-tests, wav-reference |
| `cargo xtask verify postgres` | PASS before the pure facade relocation; final preflight reran it on the final implementation tree |
| `cargo xtask preflight --changed-only --skip dbsp` | PASS with one explicit NOT_RUN, exact summary below |

```text
PASS repository
PASS rust-format
PASS rust-lint (cantos-api, cantos-contracts, cantos-studio)
PASS rust-tests (cantos-api, cantos-contracts, cantos-studio)
NOT_RUN dbsp — skipped by --skip
PASS wav-reference
PASS web-lint (cantos-studio)
PASS web-build (cantos-studio)
PASS postgres
preflight: PASS with 1 explicitly skipped (NOT_RUN) step(s)
```

Focused evidence in these suites: 36 Catalog domain/storage/cursor tests, ten pure raw admission
and response tests, one JSON-only negotiation test, nine shared consumer/DTO tests and 27 literal
schema fixture cases. The real PG gate passed 19 Catalog foundation tests plus one exact recovery
test, and ten real HTTP/application/PG tests plus one exact recovery test. It also killed and
restarted an actual loopback Cantos server and PostgreSQL, comparing exact Work/unit snapshots
and original mutation replies before/after with no duplicate SQL rows. Every successful HTTP
reply and error in the integration helper was checked against its schema.

The HTTP layer/application/persistence modules use real local PostgreSQL and no mocked provider,
object store or fabricated HTTP reply. Protocol and domain checks are `example-tested`; database
constraints/concurrency/recovery are `integration-tested`. A schema-only fixture is not HTTP
proof; a workspace test run that lists ignored PG tests is not counted as their execution.

Raw final gate outputs are retained locally under `target/catalog-evidence/session2/`.
The current disposable-run outputs live under `target/postgres-evidence/`; Catalog restart
fixtures/socket comparisons are in `target/catalog-evidence/http-restart.json` and
`target/catalog-evidence/http-server-restart.json`. These generated logs/fixtures are ignored
by Git. Repository verification was rerun after recording this evidence.

Initial failures are separate from final results: missing exhaustive consumer matches and
helper lint were fixed; sandbox-denied existing xtask listener tests passed on the authorized
loopback rerun; the repository checker failure prompted moving pure admission into the owning
application facade and adding boundary regressions. None is treated as a passing baseline.
Remote CI was not checked, polled, dispatched or awaited. DBSP is explicitly NOT_RUN per the
user's backend-only scope: no incremental engine, DBSP source or engine consumer changed.
Studio WASM verification covers shared error-enum compatibility only, not Catalog UI acceptance.

## Review and residual risk

Independent baseline review found archived Work membership removal blocked by draft-only touch.
It was repaired with a permission-only revision transition and regression coverage. Final review
also caught test expectations conflating operation/revision conflicts and a lock-expiry test that
attempted to update a session already SHARE-locked; the tests were corrected to exercise the
intended interleavings. The final independent review and a fresh read after the facade relocation both reported
`no-actionable-findings` for their declared source scopes. Reviewers did not run Cargo/PG; the
parent-owned final gates above supply runtime evidence. The lexical boundary checker continues
to document its limits rather than claiming semantic proof of every macro or inferred call.

The global catalog clock serializes all tenants and invalidates every outstanding cursor on any
Catalog write. The indexed read baseline remains the bounded session-1 smoke workload; no
production throughput, latency percentile, skew or contention acceptance is claimed. DBSP
adoption is deferred because this slice adds no incremental read model or engine consumer.
No end-to-end Studio Web, browser accessibility, mobile, producer/provider, storage, audio QC,
publication or listener playback test ran. Issue #66 remains open for responsive/keyboard Web
acceptance. Production authentication and deployment grant provisioning remain their existing
separate boundaries. External intake, publication and all media production are excluded.
