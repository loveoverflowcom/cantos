# Retained PostgreSQL test oracles (#333)

This is a **source coverage inventory**, not native behavioral acceptance. It is pinned to
`refactor/cantos-native-turso` at `12fb751ec71e20233fe4caaee35e4544fe4a0d4b`.
The [checkpoint](../server-turso-porting.md#checkpoint-boundary) and
[CMP slice](cmp-business-api.md) still delimit the implemented native business surface.

## Retirement boundary

The inventory covers **all 23 Rust files and 251 test functions** beneath
`services/cantos-server/legacy-pg-tests/`, including nested support suites and their shared
assertions. **Deleted: 0. Ported: 0. Retained: 23.** Every file contains an unmatched oracle
or supports a retained suite. This is an intentional safety result, not a claim that every
PostgreSQL mechanism should be recreated.

The executable cleanup is the direct `scripts/test_postgres.py` entrypoint: it refuses with
exit 2 before parsing old flags, probing tools, choosing a port or creating a cluster. Its
historical body (`_legacy_main`) and helper assertions remain source inputs, exercised only by
the existing mocked runner-safety regressions. No active xtask gate or CI job uses that body
as PostgreSQL verification; the repository gate reaches it only through those mocked tests.
The historical target strings inside it are not current test instructions. `free_port` remains
importable by existing historical tooling. Do not use private functions as a verification lane.

Native Cargo discovery comes from the normal `services/cantos-server/tests/*.rs` roots and
`#[cfg(test)]` module declarations, with `ServerFixture` enabled by the Server self dev-dependency.
There is no manifest target or module link enrolling the archived tree. That explains why the
old runner is invalid; it does **not** make the archived assertions redundant.

## Decision vocabulary

- **REPLACED:** every lasting assertion, including negative cases and the original production
  boundary, has equivalent native evidence that actually ran. No whole file qualifies here.
- **OBSOLETE:** an explicitly retired mechanism has no current obligation. A PG-only assertion
  can be obsolete while its surrounding authorization, privacy or atomicity test still matters.
  No whole file qualifies here.
- **NEEDS_PORT:** a lasting assertion is unmatched or only partially represented. Keep the file
  until its owning #333/#335 boundary is implemented and exercised.
- **BLOCKED:** retain an oracle whose acceptance depends on another named prerequisite. In
  particular, retained PG prefix/data-recovery evidence cannot be replaced by a fresh native file.

All native mappings below and in the appendices are **source-checked partial candidates**,
not a fresh PASS. Cargo, rustc and rustup were unavailable in the inventory environment; no
native test was run or runner-enumerated. Assertions in pure-core tests, fake repositories,
lazy/detached-store HTTP refusals, seeded listener reads and generic owner tests do not prove
successful Studio writes or the original database/HTTP/private-storage boundary.

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
| `identity_password_facts_postgres.rs` (14) | exact/misbound facts, security rechecks, deadlines/peer UID, durable revocation, HTTP no-effects | strong session/schema inner coverage; partial HTTP journey | [Security ledger](legacy-test-oracles-security.md) | NEEDS_PORT; retain | #335 full scripted-RPC → HTTP → stored-row negative matrix; #333 actor/session consumers |
| `identity_password_postgres.rs` (15) | real signup/login, Web cookie/native separation, CSRF, refresh/logout, account switch/outage/disable | real native candidate and partial real-Identity HTTP journeys | [Security ledger](legacy-test-oracles-security.md) | NEEDS_PORT; retain | #335 complete Web/Studio/renewal/disable/late-logout matrix; #336 process qualification |
| `identity_postgres.rs` (9) | exact actor keys, registration/link races, no orphan, rollback/retry, operator no-session, legacy history | strong actor adapter/schema partials | [Security ledger](legacy-test-oracles-security.md) | NEEDS_PORT; retain | #333 exact missing race/retry/CLI partitions; #336 preserved legacy data/history |
| `imports_postgres.rs` (9) | original bytes and failed-extraction history, durable receipts, access, corruption/rollback/restart | partial real extraction/domain/wire contracts | [Content ledger](legacy-test-oracles-content.md) | NEEDS_PORT; retain | #333 source/evidence schema and transaction/HTTP adapters |
| `private_artwork_storage_postgres.rs` (4) | real authorized HTTP + SQL + filesystem, exact private targets, formats/history/CAS/corruption | partial fake metadata/storage integration, real object bridge and unauthenticated router refusals | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 Catalog owner/write and Media image/artwork port, combined real boundary |
| `production_grants_postgres.rs` (2) | Studio create/list succeeds while private Work stays hidden; narrow legacy privileges | PG role/Firebase mechanism obsolete, but business success/isolation unmatched | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain mixed file | #333 real Studio/private-Theatre equivalent plus explicit owner/file trust-shift evidence |
| `production_postgres.rs` (11) | immutable frozen inputs, rights/approval invalidation, concurrent replay/CAS/rollback/reopen | partial pure Production rules only | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 settings/rights/snapshot/approval schema, actual adapters and fault/race evidence |
| `revisions_postgres.rs` (21) | exact bytes/digests, immutable revisions/reviews, authorized negotiated HTTP, replay/rollback/restart | partial decisions/codec/domain and pre-DB HTTP refusals | [Content ledger](legacy-test-oracles-content.md) | NEEDS_PORT; retain | #333 Editorial persistence and successful cross-codec owner/reader journeys |
| `schema_postgres.rs` (13) | no bootstrap/repair, exact checksums and business data, immutable constraints, prefix recovery | partial native registry/file refusal/upgrade/barriers | [Security ledger](legacy-test-oracles-security.md) | NEEDS_PORT; retain; prefix recovery BLOCKED | #333 complete business mapping; #336 PG source conversion/recovery and separately authorized legacy retirement |
| `support/audio_lifecycle_http_postgres.rs` (11) | prepare/PUT/finalize, scoped credentials, exact review/candidate/replay, actual-byte reads | partial fake lifecycle/domain/storage tests; read closure insufficient | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 Media write transaction and full authorized HTTP/private-storage journey |
| `support/brief_http_postgres.rs` (8) | pinned immutable brief/producer grants, stale/revoked/expired authority, rollback/replay | partial domain values; FK closure brief table is not a write adapter | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 brief heads/grants/operations and late rechecks |
| `support/identity_world.rs` (0; helper) | real production router/peer/cookie fixture, no dev sessions, independent DB assertions | ServerFixture + real/scripted Identity helpers are ingredients only | [Security ledger](legacy-test-oracles-security.md) | NEEDS_PORT; retain | Port its two password-suite consumers and preserve the helper assertions |
| `support/private_artwork_postgres.rs` (10) | owner/history, atomic cover/Catalog CAS, immutable rows/FKs, corruption/replay/migration | partial artwork domain/fake adapter and object-target grammar | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 native Catalog/artwork transaction/barrier/decoder cases |
| `support/private_artwork_repository_postgres.rs` (1) | production repo exact prepare/finalize/selection receipts and Catalog race | fake metadata/storage integration is only L4 | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 actual file-backed production repository and one-winner stored-state assertions |
| `support/publication_http_postgres.rs` (12) | rights/QC/approval, commit-time CAS, immutable publication/retraction and origin delivery | genuine listener/manifest/media read partials; no creator publication transaction | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 publication operations/events/expected-pointer transaction and late authority/fault cases |
| `support/theatre_catalog_walk_http_postgres.rs` (4) | owner publish→listener discovery/detail/library, privacy/rights/withdrawal and binding | genuine native listener/Library/read journeys with seeded catalog | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 creator→publication route, not seed-only parity; preserve distinct negatives |
| `support/theatre_dev_seed_http_postgres.rs` (10) | complete synthetic published fixture, exact replay/repair refusal/restart, media identity | native test seed builds domain-valid rows, not Studio acceptance | [Media ledger](legacy-test-oracles-media.md) | NEEDS_PORT; retain | #333 native catalog_http seeder and Studio write chain; xtask seed still refuses |

## Native discovery and trust boundary

The source-connected native test roots include `tests/server_turso_barriers.rs`,
`tests/theatre_business_journey.rs`, `tests/theatre_business_process.rs`,
`tests/identity_auth_candidate.rs`, `src/theatre_library_http_tests.rs`,
`src/platform/{identity,identity_auth}/turso/tests.rs`, `src/platform/turso/tests.rs`,
`src/contexts/{media,listening}/turso/tests.rs` and their ordinary domain/contract modules.
The appendices distinguish exactly which assertions those roots contain. The already-retired
Library oracle removed in [PR #387](https://github.com/loveoverflowcom/cantos/pull/387) is not
among these 23 files and must not be deleted a second time. [PR #388](https://github.com/loveoverflowcom/cantos/pull/388)
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
Theatre walk/seed. The two password suites use `support/identity_world.rs`. Some archived
`#[path]` or old module locations are already broken after relocation; missing reachability is
not proof that their assertions are obsolete. Current `tests/support/{fake_identity,
identity_process,artwork_storage_http}.rs` still serve native consumers and remain untouched.
Do not remove support by filename association or remove a parent suite without its nested ledger.

Active Cargo manifests, xtask gate commands and CI workflows enroll no archived PG target.
Retired CLI parse/refusal tests and conservative preflight selection are intentional references,
not dangling execution. The historical runner and source-bound PG records remain distinguishable
from active instructions. No PostgreSQL gate is reactivated; no schema/migration/runtime/contract
is changed by this inventory.

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
Source rollback is an ordinary Git revert; there are no data/schema changes to reverse.
