# Work-scoped delegation — first slice implementation evidence

- Decision: [ADR 0028](../decisions/0028-work-scoped-delegation.md); contract:
  [delegation v1](../../services/cantos-server/contracts/studio/delegation/v1/README.md); issue
  [#113](https://github.com/loveoverflowcom/cantos/issues/113) (stays open).
- Source baseline: PR 136 branch `docs/issue113-delegation-rfc` at `3c27aef` (based on
  `develop` `b6066ac`), 15 October 2026. Rust 1.98.1, PostgreSQL 17.11 (Homebrew), macOS arm64.
- Fixtures: synthetic Vietnamese text and generated identifiers only. No real credential,
  manuscript, provider, identity service or object store was used.

## Invariant

A delegation bearer performs only its granted Catalog actions on exactly its granted Work, only
while the grant is unrevoked and within `[issued_at, expires_at)`, both grantor and delegate are
active, the grantor still owns the Work and the Work is not archived; delegated effects are
recorded under the delegate actor and linked to the grant, never attributed to the grantor.
If false, an agent credential would carry owner power, cross-Work access or false attribution.

| Claim | Failure mode | Cheapest oracle | Evidence level |
| --- | --- | --- | --- |
| Closed actions; no review/publish/rights/membership/grant variant | wildcard or unknown label widens scope | exhaustive label table; closed DTO enum; schema/wire test | `type-enforced`, `example-tested` |
| Lifecycle admission precedence and clock boundaries | expired/revoked/inactive admitted, or Work facts leak first | bounded exhaustive loop: 7 instants × 8 fact combinations against an ordered-list oracle, non-vacuity count = 2 | `example-tested` (bounded exhaustive) |
| Exact Work, current owner, unarchived, granted action | Work B or archived Work reachable | table test plus all 7 non-empty action subsets × 3 actions | `example-tested` |
| Secret format and purpose-separated lookup | session/producer token accepted; hash collision across purposes | spelling partitions; golden hash from Python `hashlib` | `differentially-tested` (independent hash oracle) |
| Delegated writes carry no Source/Script link; rights unchanged | delegate binds owner sources or changes rights | domain table; HTTP 403 with no committed rows | `example-tested`, `integration-tested` |
| Bearer-only transport, no mixed credentials, exact Origin exemption | cookie CSRF or credential confusion | HTTP header partitions; route-path partitions | `example-tested`, `integration-tested` |
| Owner-only issuance, idempotent retry without secret, atomic rejection | member issues; lost response mints a second secret | PostgreSQL HTTP tests asserting rows unchanged | `integration-tested` |
| Delegate attribution and grant lineage | grantor recorded as author | row assertions on content, Work revision, receipts, lineage | `integration-tested` |
| Revocation fence on the Catalog clock | write admitted after revocation commits | two lock-interleaving tests with a blocker connection | `integration-tested` |
| Append-only settled rows | update/delete widens or erases history | trigger refusals as the migration owner | `integration-tested` |
| Assertions kill real defects | vacuous tests | eight hand mutations, all killed (below) | `mutation-tested` (hand, not tool) |

## Commands and results

Unit and wire tests (no infrastructure):

```sh
RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-api --lib --locked delegation
RUSTUP_TOOLCHAIN=1.98.1 cargo test -p cantos-contracts --locked --test delegation_wire
```

Results: 14 passed (12 domain, 2 HTTP adapter) and 3 passed.

PostgreSQL/HTTP tests ran against a disposable cluster created exactly as
`scripts/test_postgres.py` does (`initdb --auth=trust --no-locale`, role `cantos_app`), with
`CANTOS_TEST_CLUSTER_URL` set and `LC_ALL=C`:

```sh
cargo test -p cantos-api --locked --test delegation_http_postgres -- --ignored --test-threads=1
```

Result: 4 passed. The test is also registered in `scripts/test_postgres.py`, but the xtask
`postgres` gate itself was not run here (see blockers).

Hand mutations (one at a time, file restored and touched after each), all **killed**:

| Mutation | Killed by |
| --- | --- |
| record the grantor as the writing actor | journey test, revoke-ordering test |
| drop the Source/Script link check | journey test |
| give delegates rights-change authority | journey test |
| revoke a grant through any Work path | revocation test |
| ignore the revocation row | revocation test, revoke-ordering test |
| drop the exact-Work check in the core | journey test |
| accept a cookie alongside the bearer | journey test |
| remove the clock lock from delegated append | revoke-ordering test |

Regression on the same cluster (Catalog append/unit-page code was extracted, not changed):
`catalog_postgres` 20 passed after updating two expectations this migration legitimately
changes (registry now 1–26; a plain `TRUNCATE catalog_operations` now stops at the new lineage
foreign key with `0A000`, while `TRUNCATE … CASCADE` still reaches the immutability trigger);
`catalog_http_postgres` 24 non-audio tests passed; `revisions_postgres` 21, `production_postgres`
11, `admin_postgres` 14, `identity_postgres` 9, `studio_oidc_postgres` 18,
`theatre_identity_postgres` 16, `theatre_native_postgres` 7, `imports_postgres` 9 and
`adaptations_postgres` 21 passed.

### Read inventory and plans

New database-backed reads: grant by token hash (every delegated request), by grantor/operation
(issuance retry), by ID (revoke) and the owner page by Work. Each joins the revocation primary
key. Synthetic baseline (`EXPLAIN (ANALYZE, BUFFERS)`, PostgreSQL 17.11, 200 Works × 100 grants,
10% revoked): token and operation lookups are single unique-index scans (6 and 4 shared
buffers); the 21-row owner page uses `catalog_delegation_grants_work` with a merge join that
walked 361 revocation index entries (359 buffers). The owner page is not a hot path; a nested
loop would bound it by page size if revocations grow large. DBSP: **rejected** — indexed point
lookups, no repeated join or aggregation. No latency target or benchmark was set or run.

## Layer gates

| Layer | Status | Evidence | Scope / gaps |
| --- | --- | --- | --- |
| L1 architecture | PASS | `documented`, `statically-checked` (compiles in Catalog boundaries) | ADR 0028; Catalog owns grant, core has no I/O |
| L2 domain/types | PASS | `type-enforced` | private permit/grant fields, fallible constructors, closed enums |
| L3 unit/property | PASS | `example-tested`, `differentially-tested` | bounded exhaustive loops; no generated property campaign |
| L4 application/adapters | PASS (via L5) | `integration-tested` | no fake-store layer; exercised directly against PostgreSQL |
| L5 DB/API | PASS locally | `integration-tested` | disposable cluster outside the xtask runner; production roles NOT_RUN |

Repository gates on the final diff, as xtask printed them (Rust 1.98.1; `python3` shimmed to
Python 3.14 because the host's Python 3.9 has known unrelated suite failures):

| Command | Result |
| --- | --- |
| `cargo xtask verify repository` | `PASS` (392 Python tests, architecture/link checks, `git diff --check`) |
| `cargo xtask preflight --changed-only --skip postgres --skip audio-inspection --skip dbsp --skip web-lint --skip web-build` | `PASS` repository, rust-format, rust-lint and rust-tests (cantos-api, cantos-contracts, cantos-studio, cantos-theatre), wav-reference; the five skipped steps `NOT_RUN` |
| `cargo xtask verify web` | `PASS` web-lint, web-build (cantos-studio, cantos-theatre), ui-preview |
| `cargo xtask verify dbsp` | `PASS` |
| `postgres`, `audio-inspection` (and therefore `verify contracts`) | `UNSUPPORTED` per `cargo xtask doctor`: missing ffmpeg, ffprobe |
| `behavior` | `UNSUPPORTED` per doctor (no locked Quint tool); no model changed |

The changed-only preflight was split only to stay within free disk space; together the stages
cover every gate it selected except the two `UNSUPPORTED` ones.

## Deferred acceptance and blockers

- `cargo xtask verify postgres` / `verify contracts`: **UNSUPPORTED** on this host because
  `ffmpeg`/`ffprobe` are not installed. Equivalent delegation and regression suites ran manually
  as above. Audio-dependent tests (`audio_lifecycle_http`, listening) fail here only because
  `ffmpeg` is missing.
- Pre-existing, unrelated: the private-artwork PostgreSQL harnesses grant `cantos_app` only
  `SELECT` on `sessions`, but session authentication takes `FOR SHARE OF s`; PostgreSQL logs
  `permission denied for table sessions` and the suites return `unavailable`. Neither file is
  in this change.
- Not covered: delegated reads racing a revocation (only writes are lock-tested), Quint
  capability for this contract, production role provisioning, Studio Web management UI, live
  Kanidm/OIDC sessions as grantors, last-used/denial audit, rate limits, restart recovery.
