# CMP business API on `server.db` — slice record (#333)

**Status: implemented and file-backed tested for the routes below; not a completed Server port and
not production acceptance.** This record distinguishes what is *implemented*, what is *tested and how*,
and what is *not established*. It completes one slice of
[#333](https://github.com/loveoverflowcom/cantos/issues/333); the issue stays open.

## Review boundary

| Delivers | Does not deliver |
| --- | --- |
| The Theatre routes the CMP client calls — discovery, manifests, playback authorization, origin media, and the whole Listener Library — served by the production handlers from the embedded Turso `server.db`, with the catalog/publication rows they read | Studio, Editorial, Adaptation, Ingestion, Production and Admin write paths; the publication transaction; artwork/images; delegation; Studio session routes; audio downloads; DBSP |
| The dependency closure of those reads as appended migrations 12–15 (the placeholders 4–11 stay byte-identical) | PostgreSQL data conversion, deployment, a live provider, a device, a TLS/CDN front |
| Re-admission of the Identity session *inside* every Library business unit | A creator → publication end-to-end journey (the catalog is seeded with valid domain values) |
| Evidence: schema/barrier tests, adapter tests, a production-composition journey with the real Identity executable, a two-process loopback test of the `cantos-server` binary, and the production CMP desktop client against the router | Android/iOS device acceptance; the CMP reconnect scheduler |

Base: `refactor/cantos-native-turso` at `b2611c3d` (the snapshot named by the task).

## Migrations and dependency closure

Registered after the reserved placeholders, in foreign-key order. Existing bytes, names, versions and
checksums are unchanged; a file at version 11 upgrades by the missing suffix, a file at 15 is refused
by a binary of 11 (newer) and a diverged version 12 is refused unchanged.

| Version | Name | Tables | Why it is in the closure |
| --- | --- | --- | --- |
| 12 | `server_catalog_publication_read` | `catalog_works`, `catalog_work_revisions`, `catalog_units`, `catalog_unit_revisions`, `catalog_content_revisions`, `catalog_content_heads` | the pinned metadata a release names, the current heads, and the content revision a brief/release binds |
| 13 | `server_production_briefs` | `production_briefs` | referenced by `audio_submissions` |
| 14 | `server_audio_release_chain` | `audio_submissions`, `audio_renditions`, `audio_release_candidates`, `audio_distribution_clearances`, `audio_publication_approvals`, `audio_releases`, `audio_publication_pointers`, `audio_release_withdrawals`, `audio_submission_revocations`, `audio_listener_works`, `audio_listener_units`, `audio_listener_releases` | eligibility, manifest, grant, media identity and discovery ordering |
| 15 | `server_listener_library` | `listener_state`, `listener_progress`, `listener_saved`, `listener_history`, `listener_operations`, `listener_device_sequences`, `listener_playback_sessions`, `listener_bookmarks` | the account-owned Library |

No foreign key was dropped and no row is fabricated to avoid one. Left out because no CMP route reads
or writes them: catalog members/delegation/operations/clock, brief heads/grants/settings/snapshots,
upload intents/receipts, reviews, publication events/operations, artwork/images, editorial, ingestion,
adaptation and admin tables. Per-table mappings: [Catalog](catalog.md), [Media audio](media_audio.md),
[Listening](listening.md).

## Route matrix

`implemented` = served from Turso by the production handler. `tested` names the evidence and its kind.
Router = in-process Axum `Router` over a real file; process = the `cantos-server` binary over
loopback HTTP; CMP = the production Kotlin desktop client.

| Route | Implemented | Tested | Blocked / note |
| --- | --- | --- | --- |
| `GET /works`, `/search` (default v1 and `presentation=1`) | yes | adapter `contexts/media/turso/tests.rs`; router + process journeys | — |
| `GET /works/{work}`, `/works/{work}/units` | yes | same; cursor binding and restart | — |
| `GET /publications/{publication}` (manifest) | yes | same | — |
| `POST /publications/{publication}/playback-authorizations` | yes | adapter; router; process | grant is the origin-mode HMAC grant (ADR 0032, proposed) |
| `GET`/`HEAD /media/{grant}` incl. Range, `ETag`/304, 416 | yes (exact bytes of the publication root) | router + process journeys with real bytes | bytes are transport bytes, not inspected Opus |
| `GET /library/{continue,saved,history}` (`presentation=1`), `GET /library/bookmarks` | yes | adapter; `theatre_library_http_tests.rs`; router journey; process; CMP (bookmarks) | — |
| `GET`/`PUT /library/saved/{work}` | yes | adapter races; router; process | — |
| `GET`/`PUT /progress/{publication}` | yes | adapter (conflict, rewind, duplicate); router; process | — |
| `POST /library/history/playback-starts`, `/clear` | yes | adapter (epoch fence); router; process | — |
| `PUT /library/bookmarks/{id}`, `POST …/delete` | yes | adapter; HTTP suite; router; CMP | — |
| `GET /operations/{operation}` | yes | adapter; router; process | account-scoped |
| `/identity/*`, `/native/identity/*` | unchanged | exercised by the journeys | owned by #335 |
| Studio, Admin, publication writes | **no** | — | still PostgreSQL SQL; answer a typed unavailable, never fabricated success |

## What changed in the runtime

- **Reads**: `contexts/media/turso/listener.rs` and `contexts/catalog/turso/listener.rs` (renamed from
  `postgres/`). One pinned reader snapshot per request; one batched metadata join; eligibility is
  decided by the domain from decoded, digest-checked values (the SQL only selects candidates).
- **Library**: `contexts/listening/turso.rs` (renamed from `postgres.rs`). One `BEGIN IMMEDIATE` unit
  per write: re-admit the session, decide from facts read in the unit, write the effect, device
  sequence, account order and receipt, commit.
- **Admission**: `PasswordListenerAuthenticator` seals a `ListenerAdmission` (redacted) with the
  principal; the adapter calls `identity_auth::turso::admit_in` first in the unit and requires the
  re-admitted actor *and* session to match. The HTTP lease still performs only a pre-dispatch check and
  holds no writer. `X-Cantos-Theatre-Session` is unchanged: the request value is an expectation, the
  stamp is the admitted session, a pre-admission outage is a 503 with no stamp.
- **Indexer**: `publication_application` writes the discovery index through the Turso indexer in the
  same unit as the release row and pointer (previously the PostgreSQL release adapter called it).
- **Composition**: `ListenerService::new` and `LibraryService::new` already compose the Turso adapters
  (`Assembly::new` is unchanged); `server_router` serves them.

## Evidence

All commands run on one Apple-silicon macOS host, Rust 1.98.1, pinned `turso =0.8.3`, with
`CARGO_INCREMENTAL=0`, `TMPDIR=/private/tmp`, `LC_ALL=C`, on a clean worktree of the PR head. Real
means a run-owned file opened by the pinned engine; there are no in-memory databases.

| Claim | Command | Result |
| --- | --- | --- |
| formatting | `cargo fmt --all -- --check` | PASS |
| lint | `cargo clippy -p cantos-server --all-targets --locked -- -D warnings` | PASS |
| unit + integration | `cargo test --locked --no-fail-fast -p cantos-server` (with `CANTOS_TEST_IDENTITY_BINARY` = the locally built `cantos-identity`) | lib 647 passed / 0 failed / 5 ignored (base 618 / 4); **one** target fails: `surface_policy_baseline` 6 of 10, identical to the unmodified base (it asserts the retired `/auth/configuration` route and gets 404) |
| fresh schema and upgrade from the exact registry of 11 | `platform::turso::tests::business::*`, `…tests::an_actor_registry_file_upgrades…` | PASS: versions 1–11 keep name and checksum; actors, sessions, a revocation and a development session survive |
| raw-SQL barriers incl. `REPLACE`/upsert, pointer forward-only, snapshot/column agreement | `tests/server_turso_barriers.rs` | PASS (3) |
| discovery, eligibility matrix, pinning, corruption, concurrent readers, grant + exact bytes | `contexts/media/turso/tests.rs` | PASS (11, one is the pre-existing presentation-stats unit test) |
| Library: races, receipts, rollback, lost COMMIT, admission orderings, bookmarks | `contexts/listening/turso/tests.rs` | PASS (12) |
| Library through the production router; account isolation | `theatre_library_http_tests.rs` | PASS (5) |
| production composition, real Identity process, native + Web credentials, Identity outage | `tests/theatre_business_journey.rs` | PASS (2) |
| `cantos-server` binary over loopback HTTP, graceful stop, second process reads the same outcomes; unmigrated file refused unchanged | `tests/theatre_business_process.rs` | PASS (1) |
| production CMP desktop client against the router and real file | `cmp_production_client_against_the_real_api -- --ignored` with Gradle 8.14.3 | PASS (`LIVE-BOOKMARK-OK`, 54 requests, then database assertions) |
| mutation checks (hand mutants, each restored afterwards) | audio-release update guard removed; admission actor/session check removed; admission removed from the write unit; pointer, withdrawal, revocation and archived-Work predicates removed; representative `pick=1` removed | every one killed by a named test. Two tests were added because the first run left the withdrawal/revocation-only and representative mutants alive. One mutant is **equivalent**: removing the SQL `rights_end` prefilter changes nothing, because the domain (`ListenerPublication::resolve`) refuses an expired window itself; the prefilter is kept as defence in depth |

The router journey and the process test are different evidence: the first drives the composed `Router`
in-process with one Identity process; the second adds the real `main`, environment parsing, host
settings, a TCP listener and a process restart. Neither establishes the creator pipeline, TLS, a CDN or
a device.

### Baseline

Compared with `b2611c3d` in a detached worktree: `surface_policy_baseline` 6 failures (identical),
`identity_auth_candidate` needs `CANTOS_TEST_IDENTITY_BINARY` (identical), the repository architecture
guard's four `media/postgres … ConstraintKind` findings (identical). `theatre_contract::
unavailable_database_never_returns_mock_catalog_or_a_signed_url` asserted 503 only because the tables
were missing; it now asserts an honest empty catalog on a healthy file and a typed 503 when the index
table is damaged.

## Fixture boundary

`test_support::world` builds *valid domain values* — identities, digests, canonical snapshots, the
production brief/submission/rendition/candidate/clearance/approval/release constructors and the
production discovery indexer — and stores them in the real tables. Serving uses the production adapters,
eligibility and media handlers; no router, repository or eligibility is swapped. It does **not** prove
that a creator can reach these rows through Studio, that a review or rights decision happened, or that
the bytes are valid Opus. The explicit `cargo xtask seed theatre` still refuses before effects (its
`catalog_http` seeder is not ported); this slice does not fake its readiness.

## Shared tooling changed

`scripts/check_repository.py` (and its tests): the architecture guard now (1) treats the Turso schema
registry's DDL (`platform/turso/*.sql` and its inclusion by `platform/turso/mod.rs`) like the PostgreSQL
schema registry — one database declares several contexts' tables and the foreign keys between them;
`ATTACH` is still refused; (2) exempts `test_support*` fixture modules from the table-ownership rule (a
fixture seeds rows across contexts; it is never served); (3) lets the relocated Library transport tests
keep the listener-test import exemption. Queries over a context's tables remain confined to its
persistence adapter.

## Deferred ledger

| Kind | Affected claim / path | Blocker | Done independently | Current runtime behavior | Resume prerequisite | Next check | Owner |
| --- | --- | --- | --- | --- | --- | --- | --- |
| source (remaining #333) | publication write transaction (preflight → observation → final unit with the expected-pointer CAS), reviews, intake, lifecycle | Media write adapters still carry PostgreSQL SQL | the read side, the indexer, the release/pointer barriers | Studio publication routes answer a typed unavailable | port `media/postgres/{lifecycle,publication}.rs` to Turso at their own versions | publish through Studio HTTP and read it through `theatre_business_journey` instead of `seed_work` | #333 Media write port |
| source (remaining #333) | Catalog Studio writes, delegation, members, reorder; Production, Editorial, Adaptation, Ingestion, Admin, artwork | not required by a CMP route | the closure tables | typed unavailable | per-context ports (two-phase unit reorder) | the context's legacy PostgreSQL oracle ported | #333 |
| acceptance (not run) | `cargo xtask seed theatre`, a creator → publication journey on Turso | the `catalog_http` seeder and the Studio write path are unported | the fixture builds the same rows from domain values | `seed theatre` refuses before effects | the two rows above | `cargo xtask seed theatre` then the journey | #333/#335 |
| acceptance (not run) | Android/iOS device, TLS front, CDN, live provider | out of scope | CMP desktop client against the router | — | an operator environment | device runbook | #336/#348 |
| investigation | the "existing CMP error-classification bug" named in the task brief | not identified from the task text or the code read here | none; CMP untouched | — | the reporter's repro | add a Kotlin regression where it is found | CMP owner |
| observation | `NativeAuthorizedHttpPort` maps an identity `ServiceUnavailable` to HTTP 503 but `Transport`/`UnknownOutcome` to a status-0 refusal; not claimed to be the bug above | — | — | — | review | Kotlin test of both paths | CMP owner |
