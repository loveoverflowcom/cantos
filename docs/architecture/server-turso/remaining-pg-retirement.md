# Remaining PostgreSQL business-oracle retirement

Base: `refactor/cantos-native-turso` at `755c857cd90627aebcac398552c4968e5855b9be`.
This change removes 17 archived Rust files / 183 test functions. The [exact function mapping](remaining-pg-oracles.json)
pins each old assertion family to its native test or explicitly explains a retired PG mechanism.
The six files / 68 tests in the separate [PR #395](https://github.com/loveoverflowcom/cantos/pull/395)
remain on this independent base. The counts must not be reported as zero until both changes land.

## Schema and adapter mapping

Versions 1–16 and registered PostgreSQL SQL/checksums are unchanged. Versions 17–23 append real
STRICT tables; all 69 Server-owned tables now exist, in addition to development sessions/history.
No identity.db table is imported. Media bytes remain outside SQL.

| Source rule / objects | Native mechanism | Actual native evidence roots |
| --- | --- | --- |
| Catalog members, operation receipts, clock, scoped delegation and revocation | v17; same-unit admission, immutable operation/grant/revocation guards; trusted milliseconds; two-phase out-of-range reorder within one writer | `tests/catalog_studio_turso.rs`, `tests/catalog_http_turso.rs`, `tests/delegation_turso.rs` |
| Frozen adaptation source/base/prompt, immutable proposals/submissions/acceptances and cancellation transitions | v18; validated TEXT JSON and receipt digests, append-only guards including alternate unique keys, restricted run transitions; microsecond timestamps | `tests/adaptation_turso.rs`, `src/test_support/port_ack_tests.rs` |
| Casting/settings versions, recorded rights, snapshots pinned to actual receipts, approval invalidation | v19; serialized CAS, parameterized paired JSON lookups, exact input/receipt reconstruction; immutable snapshots/approval and micros | `tests/production_turso.rs`, `src/test_support/port_ack_tests.rs` |
| Original audio intake, exact revision/digest/owner and receipt binding | v20; binding trigger + composite FK, checked bytes/digests; actual inspection/storage outside the writer then admission recheck | `tests/audio_intake_turso.rs`, `src/test_support/media_intake_tests.rs` |
| Image intent/assets, bounded metadata, private target identity, atomic Catalog artwork selection | v20; checked receipt snapshots, FKs, immutable unique-key guards and one shared Catalog CAS unit; milliseconds | `tests/private_artwork_turso.rs`, `src/test_support/artwork_turso_tests.rs`, `src/test_support/image_repository_turso_tests.rs` |
| Brief head, grant scope/hash/deadline and immutable revoke | v21 plus existing v13 brief table; exact source/content pins, monotonic head, real same-unit owner/producer rechecks | `tests/support/brief_http_turso.rs` wired from `catalog_http_turso` |
| Admin admission, operation receipt, append-only audit and revocation | v22; verified role observation before writer, exact active actor facts inside it, effect/audit atomic; fresh final trusted-clock check | `tests/admin_turso.rs`, `src/platform/admin/turso_tests.rs` |
| Rendition/review/candidate, rights/QC/approval, release pointer/events and operation receipt | v23 adds missing review/event/operation tables to v14 chain; checked object identity, immutable receipts, final pointer CAS and current rights/authority | `tests/support/{audio_lifecycle_http,publication_http}_turso.rs` wired from `catalog_http_turso` |
| Owner publish → anonymous Theatre walk and synthetic seed replay | Same real Studio/publication/listener routes and private/public filesystem adapters; synthetic publication origin only | `tests/support/{theatre_catalog_walk_http,theatre_dev_seed_http}_turso.rs` |
| Exact schema/history, damaged registry and safe suffix migration | Native registry refuses empty marked, malformed, gapped or changed history; all-table inventory; retained PG SHA-256 constants | `tests/server_schema_history_turso.rs`, platform Turso tests and `packages/store-turso/tests/owned_database.rs` |

## Trust and concurrency

PostgreSQL roles, SECURITY DEFINER/EXECUTE/search_path checks, SQLSTATE numbers, advisory/row-lock
observers, TRUNCATE and PG query-plan formats have no embedded privilege equivalent. The boundary
is the private single owner, checked parameterized adapters, same-unit authorization, schema guards
and checked receipt bytes/bindings. No simulated PG role or SQL rewriting is introduced. Immutable
rows reject UPDATE, DELETE and replacement/upsert attempts, including every alternate unique key.

All mutations use the existing bounded serialized writer. Reorder stages positions out of range
and saves only changed snapshots before restoring other positions. Competing commands still have
one winner; queued mutations re-admit authority after obtaining the writer. Pure query use cases
use pinned read snapshots. Media inspection, bytes, storage and role verification remain outside
SQL write units. Audio finalize rechecks the fresh clock after inserting a speculative rendition;
expiry rolls metadata back while an already persisted private object may remain for retry.

Unknown acknowledgement tests exercise actual COMMIT and ROLLBACK, then reconcile original operation
keys on the normal owner, including a true close/reopen. Cancellation after object persistence,
dropped transactions, injected SQL failures, stale input, corrupt rows, wrong target identity and
concurrent delivery are distinct cases. Only clock/ack and external role observations are doubles.
Test-case serialization respects the existing two-inspector resource bound; tasks raced within a
case remain concurrent. Synthetic media is generated at runtime and never committed.

## Historical mechanisms and limits

PG epoch-prefix scripts, schema, checksums, production principals, containers and recovery assets
are retained unchanged. Their sequence/catalog fingerprints and PG suffix execution are specific
to operator recovery, not native business persistence. `test_vps_prefix_upgrade.py` uses command
doubles; it does not establish a real PG rehearsal. Native verified backup/restore and exact suffix
migration are tested independently. No PG-to-Turso conversion or production-data parity is claimed.

The retired PostgreSQL runner now contains only a no-effect refusal; its unreachable cluster/HTTP
orchestration and tests of dead helpers are removed. The historical Theatre-auth script keeps its
own loopback fixture utility. Native `seed theatre` remains refused: a test-owned synthetic Studio
journey is not an implemented operator seeding command. No gate or parallel orchestration entrypoint
is added. Provider/Firebase registration-grant and old cookie-exchange mechanisms are obsolete under
Identity auth v1; native authentication/browser qualification remains with its existing owner.

Local native SQL, object and Router evidence is distinct from actual two-process authentication,
Linux isolation/Kani, browser/device, provider/cloud storage, rights verification, human listening,
production activation and cutover. Admin tests supply a synthetic verified role observation; the
production Admin verifier stays fail-closed. Publication tests use a synthetic origin and local
filesystem, not CDN or public delivery acceptance. Warm synthetic performance samples and actual
indexed plans establish bounded local behavior only. Exact commands/results and remaining gates
are recorded in the PR; remote CI is not awaited.
