# Retained PostgreSQL test oracles (#333)

This ledger distinguishes the retained source audit from native replacements on base
`refactor/cantos-native-turso@84eea4c3843908f4f21da9760381c129e21d5263` plus this change.
The [checkpoint](../server-turso-porting.md#checkpoint-boundary) still limits the incomplete
business port. Native tests exercise Turso =0.8.3; they do not convert a PostgreSQL database.

## Retirement boundary

The original inventory covered **23 Rust files / 251 tests**. This change removes **six old
paths / 68 original tests**: Revision (21), Import (9), Actor Registry (9), real Identity HTTP
(15), scripted Identity HTTP (14), and their PG world helper (0). The translated business
assertions run in native files; database-role/SQLSTATE/backend-lock syntax is explicitly retired.
**Remaining: 17 files / 183 tests**, all with unmatched production obligations listed below.
The content and security appendices map every removed original test to its native evidence
or the obsolete mechanism; shared native tests consolidate duplicate assertions.

The executable cleanup is the direct `scripts/test_postgres.py` entrypoint: it refuses with
exit 2 before parsing old flags, probing tools, choosing a port or creating a cluster. Its
historical body (`_legacy_main`) and helper assertions remain source inputs, exercised only by
the existing mocked runner-safety regressions. The direct `benchmark_revision_reads.py` entrypoint is also a refusal-only stub: its old
PG setup/query extraction and obsolete helper tests are removed, while source-bound measurement
results and the historical protocol retain their provenance. No active xtask gate or CI job uses that body
as PostgreSQL verification; the repository gate reaches it only through those mocked tests.
Migrated targets are removed from its historical selection and invocations; remaining strings are not current test instructions. `free_port` remains
importable by existing historical tooling. Do not use private functions as a verification lane.

Native Cargo discovery comes from the normal `services/cantos-server/tests/*.rs` roots and
`#[cfg(test)]` module declarations, with `ServerFixture` enabled by the Server self dev-dependency.
There is no manifest target or module link enrolling the archived tree. That explains why the
old runner is invalid; it does **not** make the archived assertions redundant.

## Decision vocabulary

- **REPLACED:** every lasting assertion, including negative cases and the original production
  boundary, has equivalent native evidence that actually ran. The removed files qualify only for translated business behavior; PG mechanism retirements are separately stated.
- **OBSOLETE:** an explicitly retired mechanism has no current obligation. A PG-only assertion
  can be obsolete while its surrounding authorization, privacy or atomicity test still matters.
  No retained whole file qualifies here.
- **NEEDS_PORT:** a lasting assertion is unmatched or only partially represented. Keep the file
  until its owning #333/#335 boundary is implemented and exercised.
- **BLOCKED:** retain an oracle whose acceptance depends on another named prerequisite. In
  particular, retained PG prefix/data-recovery evidence cannot be replaced by a fresh native file.

For the 17 retained sources, native candidates remain **source-checked partial mappings**.
Their original inventory environment lacked Cargo; its NOT_RUN result is historical. The new
native suites and focused real-file/HTTP campaign are recorded with exact outcomes in the PR.
Pure-core tests, fake repositories, seeded listener reads and generic owner tests still cannot
replace unmatched Studio/Production/Media/Admin writes.

## File coverage ledger

Paths in the first column are relative to `services/cantos-server/legacy-pg-tests/tests/`.
The detailed appendices name every test and its actual assertions, helper assertions,
negative cases, native partial mappings and remaining gaps.

| legacy_file (test count) | important_oracle | native_replacement | evidence | decision | remaining_gap / next owner |
| --- | --- | --- | --- | --- | --- |
| `adaptations_postgres.rs` (20) | frozen context, immutable submissions, explicit acceptance/cancellation, rollback/replay/auth/restart | partial domain decisions/parser only | [Content ledger](legacy-test-oracles-content.md) | NEEDS_PORT; retain | #333 Adaptation + Ingestion/Editorial persistence, receipt and successful HTTP port |
| `admin_postgres.rs` (15) | separate capability, atomic audit/revocation/receipt, late recheck, immutable history/restart | partial pure role matrix and wire fixtures | [Security ledger](legacy-test-oracles-security.md) | NEEDS_PORT; retain | #333 native Admin verifier/admission and durable effects; no fallback to Studio authority |
| `audio_intake_postgres.rs` (21) | authenticated brief/upload scope, byte integrity, immutable receipts, expiry/revocation/replay/rollback | partial admission/inspection core; release read closure is not intake | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 native brief/grant/intake adapters and actual-file/HTTP negative cases |
| `catalog_http_postgres.rs` (17) | successful Studio Catalog/Content, private membership, CAS/reorder, exact receipts and restart | partial Catalog raw barriers, domain/wire tests | [Content ledger](legacy-test-oracles-content.md) | NEEDS_PORT; retain | #333 Studio adapter/authority paths and nested support-suite ports |
| `catalog_postgres.rs` (20) | ownership, links, archival, reorder, exact receipts, late authority, rollback/restart | partial Catalog barriers and listener reads | [Content ledger](legacy-test-oracles-content.md) | NEEDS_PORT; retain | #333 Studio writes/members/operations/clock and two-phase reorder |
| `delegation_http_postgres.rs` (4) | scoped grant/revocation, no secret replay, delegate attribution, write/revoke ordering | partial pure delegation rules | [Content ledger](legacy-test-oracles-content.md) | NEEDS_PORT; retain | #333 native delegation tables/adapter and admitted writer orderings |
| `private_artwork_storage_postgres.rs` (4) | real authorized HTTP + SQL + filesystem, exact private targets, formats/history/CAS/corruption | partial fake metadata/storage integration, real object bridge and unauthenticated router refusals | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 Catalog owner/write and Media image/artwork port, combined real boundary |
| `production_grants_postgres.rs` (2) | Studio create/list succeeds while private Work stays hidden; narrow legacy privileges | PG role/Firebase mechanism obsolete, but business success/isolation unmatched | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain mixed file | #333 real Studio/private-Theatre equivalent plus explicit owner/file trust-shift evidence |
| `production_postgres.rs` (11) | immutable frozen inputs, rights/approval invalidation, concurrent replay/CAS/rollback/reopen | partial pure Production rules only | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 settings/rights/snapshot/approval schema, actual adapters and fault/race evidence |
| `schema_postgres.rs` (13) | no bootstrap/repair, exact checksums and business data, immutable constraints, prefix recovery | partial native registry/file refusal/upgrade/barriers | [Security ledger](legacy-test-oracles-security.md) | NEEDS_PORT; retain; prefix recovery BLOCKED | #333 complete business mapping; #336 PG source conversion/recovery and separately authorized legacy retirement |
| `support/audio_lifecycle_http_postgres.rs` (11) | prepare/PUT/finalize, scoped credentials, exact review/candidate/replay, actual-byte reads | partial fake lifecycle/domain/storage tests; read closure insufficient | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 Media write transaction and full authorized HTTP/private-storage journey |
| `support/brief_http_postgres.rs` (8) | pinned immutable brief/producer grants, stale/revoked/expired authority, rollback/replay | partial domain values; FK closure brief table is not a write adapter | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 brief heads/grants/operations and late rechecks |
| `support/private_artwork_postgres.rs` (10) | owner/history, atomic cover/Catalog CAS, immutable rows/FKs, corruption/replay/migration | partial artwork domain/fake adapter and object-target grammar | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 native Catalog/artwork transaction/barrier/decoder cases |
| `support/private_artwork_repository_postgres.rs` (1) | production repo exact prepare/finalize/selection receipts and Catalog race | fake metadata/storage integration is only L4 | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 actual file-backed production repository and one-winner stored-state assertions |
| `support/publication_http_postgres.rs` (12) | rights/QC/approval, commit-time CAS, immutable publication/retraction and origin delivery | genuine listener/manifest/media read partials; no creator publication transaction | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 publication operations/events/expected-pointer transaction and late authority/fault cases |
| `support/theatre_catalog_walk_http_postgres.rs` (4) | owner publish→listener discovery/detail/library, privacy/rights/withdrawal and binding | genuine native listener/Library/read journeys with seeded catalog | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 creator→publication route, not seed-only parity; preserve distinct negatives |
| `support/theatre_dev_seed_http_postgres.rs` (10) | complete synthetic published fixture, exact replay/repair refusal/restart, media identity | native test seed builds domain-valid rows, not Studio acceptance | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 native catalog_http seeder and Studio write chain; xtask seed still refuses |

## Native discovery and trust boundary

The source-connected native test roots include `tests/server_turso_barriers.rs`,
`tests/theatre_business_journey.rs`, `tests/theatre_business_process.rs`,
`tests/identity_auth_candidate.rs`, `tests/identity_password{,_facts}_turso.rs`,
`tests/editorial{,_recovery}_turso.rs`, `tests/ingestion_turso.rs`, `tests/operator_turso.rs`, `src/theatre_library_http_tests.rs`,
`src/platform/{identity,identity_auth}/turso/tests.rs`, `src/platform/turso/tests.rs`,
`src/contexts/{media,listening}/turso/tests.rs` and their ordinary domain/contract modules.
The appendices distinguish exactly which assertions those roots contain. The already-retired
Library oracle removed in [PR #387](https://github.com/loveoverflowcom/cantos/pull/387) is not
among the original 23 files and must not be deleted a second time. [PR #388](https://github.com/loveoverflowcom/cantos/pull/388)
repairs its CMP consumer, not the remaining Studio/Production writes.

PostgreSQL `GRANT`, roles, `SECURITY DEFINER`, SQLSTATE and lock-observer syntax have **no
SQL privilege equivalent** in embedded Turso. Do not implement fake SQLite grants. The selected
boundary is one owner process, private files, checked constructors/admission, parameterized
adapters, schema guards and digest/binding checks. Source-connected foundation tests
`packages/store-turso/tests/owned_database.rs::{one_owner_per_file_and_a_private_directory,
every_connection_enforces_foreign_keys_and_readers_cannot_write}` exercise owner/file/reader
boundaries; per-context barriers still need their own actual table tests. Session/actor native
suites cover particular admission and immutable-row rules, not the entire retained Admin,
Studio, media or legacy-recovery trust boundary. A privileged file owner is not restricted like
an old PG table role. Compensating application tests cannot be described as DB role enforcement.

## Support and reference safety

The nested legacy chain is `catalog_http_postgres` → brief/audio lifecycle → publication →
Theatre walk/seed. The two native password suites now use `tests/support/identity_world.rs`; their PG helper is deleted. Some archived
`#[path]` or old module locations are already broken after relocation; missing reachability is
not proof that their assertions are obsolete. Current `tests/support/{fake_identity,
identity_process,artwork_storage_http}.rs` still serve native consumers and remain untouched.
Do not remove support by filename association or remove a parent suite without its nested ledger.

Active Cargo manifests, xtask gate commands and CI workflows enroll no archived PG target.
Retired CLI parse/refusal tests and conservative preflight selection are intentional references,
not dangling execution. The historical runner and source-bound PG records remain distinguishable
from active instructions. No PostgreSQL gate is reactivated and no registered schema or migration is changed. The two
production fixes preserve source-record time/evidence and indexed import-replay semantics.

## Next safe deletion boundary

For each NEEDS_PORT row, #333 must supply the real owning schema/adapter and current-authority
transaction before a scoped port is practical. #335 owns the remaining first-party auth HTTP
matrix; #336 owns actual conversion/recovery and process/platform acceptance. Port one coherent
oracle with `ServerFixture` and actual private storage where required, keeping its exact errors,
rows, negative cases, controlled writer orderings, immutable receipts, retry/unknown-commit,
rollback and restart observations. Run the Cargo-discovered replacement, then review the mapping
before removing its legacy copy. A generic owner test or successful compilation is insufficient.

The retained PG registry/checksum/prefix evidence also protects migration inputs and recovery;
native versions have different meanings. Preserve registered SQL byte-for-byte. No production
migration, deployed legacy retirement, merge or deployment follows from this source cleanup.
Source rollback is an ordinary Git revert; no migration or production data conversion occurs.
