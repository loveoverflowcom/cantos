# PostgreSQL test-oracle retirement (#333)

The original inventory covered 23 files / 251 tests. [PR #395](https://github.com/loveoverflowcom/cantos/pull/395)
retired six paths / 68 tests; this change ports the final **17 files / 183 tests**. **Remaining: zero
Rust files in `legacy-pg-tests`.** Explicitly retired PG mechanisms are distinguished from translated
business assertions. Native SQL, HTTP, filesystem and restart boundaries are described in the
[native port](remaining-pg-retirement.md) and [183-entry mapping](remaining-pg-oracles.json).

Initial oracle source: `755c857cd90627aebcac398552c4968e5855b9be`. After concurrent #395/#396/#397
merges, the branch was rebased onto `refactor/cantos-native-turso@7057ab41720b9bb341b27bae8a08f0ea7211b6ae`.
Final diff/review/preflight use that same integration base. ADR 0046's service-owned contracts,
fixtures and representations are preserved. Retained PostgreSQL schema/checksum/operator recovery
assets are unchanged; no PG conversion or production cleanup is claimed.

The PostgreSQL runner now contains only its no-effect refusal. Its dead orchestration/helper tests
are removed; independent retained Studio safety assertions remain. The benchmark refusal from #395
remains. No gate, retired command or operator seeding lane is enabled.

## File coverage ledger

Paths in the first column are relative to `services/cantos-server/legacy-pg-tests/tests/`.
The detailed appendices name every test and its actual assertions, helper assertions,
negative cases, native partial mappings and remaining gaps.

| legacy_file (test count) | important_oracle | native_replacement | evidence | decision | remaining_gap / next owner |
| --- | --- | --- | --- | --- | --- |
| `adaptations_postgres.rs` (20) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `admin_postgres.rs` (15) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `audio_intake_postgres.rs` (21) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `catalog_http_postgres.rs` (17) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `catalog_postgres.rs` (20) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `delegation_http_postgres.rs` (4) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `private_artwork_storage_postgres.rs` (4) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `production_grants_postgres.rs` (2) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `production_postgres.rs` (11) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `schema_postgres.rs` (13) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `support/audio_lifecycle_http_postgres.rs` (11) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `support/brief_http_postgres.rs` (8) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `support/private_artwork_postgres.rs` (10) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `support/private_artwork_repository_postgres.rs` (1) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `support/publication_http_postgres.rs` (12) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `support/theatre_catalog_walk_http_postgres.rs` (4) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |
| `support/theatre_dev_seed_http_postgres.rs` (10) | Lasting business oracles ported; PG-specific mechanics retired explicitly | Real native owner/application/HTTP counterparts | [Exact mapping](remaining-pg-oracles.json) | REPLACED / mechanism-specific OBSOLETE | [Native port](remaining-pg-retirement.md); production/recovery acceptance remains separate |


## Native discovery and trust boundary

The source-connected native test roots include `tests/server_turso_barriers.rs`,
`tests/theatre_business_journey.rs`, `tests/theatre_business_process.rs`,
`tests/identity_auth_candidate.rs`, `src/theatre_library_http_tests.rs`,
`src/platform/{identity,identity_auth}/turso/tests.rs`, `src/platform/turso/tests.rs`,
`src/contexts/{media,listening}/turso/tests.rs` and their ordinary domain/contract modules.
The appendices below preserve the original source-only review at its pinned revision; their old NEEDS_PORT conclusions are superseded for the 17 files by the current mapping. The already-retired
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

## Support and remaining acceptance

The former Catalog → brief/audio → publication → Theatre walk/seed chain is now wired from
`tests/catalog_http_turso.rs`. The password suites use the native Identity helpers delivered by the merged PR #395. Existing native Identity helpers remain; the
artwork HTTP helper now supports an awaited graceful shutdown so tests really release the file
owner before reopening it. No support file is removed merely by name association.

Cargo, xtask and CI enroll no archived PG target. The retired CLI refusal remains intentional;
its dead runner body is removed. This port appends native migrations 17–23 and changes persistence
adapters, while preserving public contracts and every registered older migration. See the exact
mapping and native port record for remaining process/browser/device and production acceptance.

The retained PG registry/checksum/prefix inputs protect operator recovery and are not native
migration versions. Preserve registered SQL byte-for-byte. No merge, deployed legacy cleanup,
production migration or deployment is authorized by this source task. Reverting source alone is
not a schema downgrade: an older native binary refuses a file with newer history. An operator
must use the existing verified backup/restore policy for any separately accepted persisted file;
this task migrates only run-owned fixtures and touches no production state.
