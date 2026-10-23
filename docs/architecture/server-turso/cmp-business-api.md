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


## CMP authorized boundary and recovery follow-up

This follow-up fixes the native consumer gaps on the freshly fetched integration base
`a2777d9833e6ad877dcfbd929df40e04b84969ae` after #387. Backend business semantics, migrations,
Web HttpOnly cookies and native opaque Bearer/session protocols are unchanged.

- The production authorized port accepts the exact origin already approved by the host,
  including Android debug loopback. The route/surface allowlist and credential lease fences
  still run before dispatch; release and iOS do not admit cleartext.
- The client classifies the strict, real pre-admission Library `503` envelope without a stamp as
  retryable unavailable, with no session revalidation, player reset or account body application.
  Local transport/unknown-outcome handling remains earlier in the decision. A real `401` or
  explicit different stamp still fences authority. Success/receipts require an actual matching
  header, bounded JSON and valid schema/binding; unknown statuses/envelopes fail closed.
- Foreground and restored-network hints call the one process/account flusher. An opportunity
  after retry exhaustion opens a new bounded episode. Duplicate in-flight callbacks share the
  drain; persisted sent intents reconcile their original receipt before byte-identical replay.
  Android uses one process ConnectivityManager registration whose decision is the neutral
  `NetworkRecoveryTracker`: from API 24 a current-default registration where a new default replaces
  the old one (the platform stops reporting the replaced network and may never report it lost), and
  on API 23 an all-networks registration. Before API 26 the adapter reads capabilities when a
  network becomes available, as the platform does not guarantee a capabilities callback then.
  iOS retains NWPathMonitor and forwards active-scene/restored-path hints to the main owner.
  These hints grant no authority.

### Regressions and real boundary evidence

Commands below run from the repository root unless `--project-dir` names the Gradle project.
Toolchain: macOS arm64, Rust 1.98.1, Gradle 8.14.3, JDK 21.0.7, Kotlin 2.2.20/CMP 1.9.0,
SQLDelight 2.1.0; the Server/Identity use the pinned Turso 0.8.3. Build output, raw logs and JUnit
reports remain local under ignored `artifacts/agent-runs/cmp-live-api-recovery/` and module `build/`.

| Claim | Oracle / exact focused command | Result and limits |
| --- | --- | --- |
| Debug loopback was refused before dispatch | `NativeAuthorizedHttpPortTest.hostApprovedDebugLoopbackUsesTheProductionAuthorizedBoundary` | Old source failed: expected one dispatch, got zero; fixed source passes, with foreign scheme/host/port and release cleartext refusals |
| Real unstamped 503 was treated as a changed session | `NativeLibraryClientTest.contractedPreAdmission503WithoutStampIsRetryableWithoutAuthorityLoss`, runtime `preAdmissionOutageKeepsAuthorityPlayerAndDurableLibrary` | Old source failed (`session_changed`, authority became null); fixed source preserves authority, player and pending rows; malformed/wrong-stamped/success-without-stamp inputs reject |
| Exhaustion left the next opportunity without retries | runtime `exhaustedEpisodeCanStartAnotherBoundedEpisodeWithoutChangingTheFrozenIntent`, BookmarkSync `exhausted_retries_keep_the_durable_bookmark_and_recovery_reuses_its_exact_intent` | Old scheduling failed the resumed-episode oracle; fixed virtual-time tests exercise foreground and reconnect separately, duplicate callbacks, receipt-first replay, frozen operation/body and A-to-B fencing |
| Real Identity/Turso router journeys | `cargo test --locked -p cantos-server --test theatre_business_journey` with `CANTOS_TEST_IDENTITY_BINARY` pointing to this checkout's built executable | PASS: 2 existing journeys; the explicit Gradle lane is ignored here |
| Library HTTP harness | `cargo test --locked -p cantos-server --lib theatre_library_http_tests` | PASS: 5, with the explicit trusted-host Gradle lane ignored |
| Production CMP authorized path over real TCP HTTP and Identity | `cargo test --locked -p cantos-server --test theatre_business_journey cmp_authorized_library_against_real_identity_and_http -- --ignored --exact --nocapture` | PASS: 1; `LIVE-AUTHORIZED-LIBRARY-OK requests=23`; real login, actual session stamps, Identity stop → genuine 503/no stamp → restart/recovery, lost committed response recovered by receipt, final Server row/anomaly assertions |
| Opt-in live Desktop Preview composition over the real Server and Identity | `cargo test --locked -p cantos-server --test theatre_business_journey cmp_live_desktop_preview_against_real_identity_and_http -- --ignored --exact --nocapture` | PASS: 1; `LIVE-DESKTOP-PREVIEW-OK`. `LiveEnrollmentOwners` (shell, v1 login, authorized Library, real public catalog) saved, bookmarked and logged out; the harness read 1 saved row, 1 bookmark row and no anomalies. In-memory preview store, audio unavailable, no window driven. Record: [Desktop live acceptance](../../evidence/cmp-desktop-live-acceptance.md) |
| Retained trusted-host fixture | `cargo test --locked -p cantos-server --lib theatre_library_http_tests::cmp_production_client_against_the_real_api -- --ignored --exact --nocapture` | PASS: 1; `LIVE-BOOKMARK-OK`, 54 requests. Synthetic trusted-host principal, explicitly not authentication acceptance. The fixture server stamps and the JDK transport reads that actual header |
| A replaced default network was kept as proof of connectivity | `NetworkRecoveryTrackerTest.replacedDefaultNetworkIsNotKeptAsProofOfConnectivity` (default A usable → default B usable without a loss for A → B lost → C usable emits one opportunity) | The prior set-based tracking is the all-networks case in `allNetworksRegistrationWouldHaveKeptTheReplacedNetwork`, which does not emit for C; the current-default tracking does. Also duplicate, stale-loss, unvalidated-default and closed-owner cases. JVM logic only: OS callback delivery is not exercised |
| Whole JVM application/sandbox | `cargo xtask theatre-preview verify` | PASS; production owners plus named external doubles; no native device acceptance |
| iOS build prerequisite | `cargo xtask theatre-host ios-build --kotlin-native-home /Users/manhblue/.konan/kotlin-native-prebuilt-macos-aarch64-2.2.20` | UNSUPPORTED: the pinned compiler is absent; Swift syntax parse passes, Kotlin/Native/framework/app compile and iOS runtime NOT_RUN |

The two explicit Gradle-backed Rust commands additionally require
`CANTOS_TEST_GRADLE=/absolute/path/to/gradle-8.14.3/bin/gradle` and a compatible `JAVA_HOME`.
Identity is built with `cargo build --locked -p cantos-identity --features service --bin cantos-identity`;
its test SMTP sink, files and private Unix socket are run-owned. No external mail or production
account/data is involved. In the new CMP lane the production account controller, authorized
adapter and Library client run over real TCP, the real Identity executable, the Server database and
the desktop SQLite store. The test uses a JDK test transport, replaces the credential journal/
logout barrier, `PlaybackAccessPort` and the audio decoder, and calls `foregrounded()` and
`networkRestored()` directly. This is real HTTP/auth/storage integration, not native media or OS
connectivity-callback acceptance. The request count printed by the test is an observation, not an
invariant.

The behavior catalog's listening-progress entry remains `needs-decision` without a model;
`identity/native_session_admission.qnt` is draft and models outage without revocation, excluding
HTTP and client retry scheduling. Neither model nor Rust/Kani owner changes here. No formal or
Kotlin/Native conformance claim is made.

### Handoff gates and scope blocker

`cargo xtask preflight --changed-only --base a2777d9833e6ad877dcfbd929df40e04b84969ae`
was run without skips on the final diff. It reports repository FAIL and every later selected gate
NOT_RUN because it stops at that failure. The 31 repository findings were re-run on a clean checkout
of that base and are the same set (only the checkout path in one message differs). Independent
checks run on this diff: `cargo fmt --all -- --check`, `cargo clippy --locked -p xtask
--all-targets -D warnings` and `cargo test --locked -p xtask theatre_host::tests` (12) PASS; the
Android script tests PASS; `cargo xtask theatre-preview verify` PASS selects 490 shared tests
(487 executed, 3 opt-in live suites skipped) plus 103 sandbox tests. Server, WAV and audio-inspection
results from the earlier head (Server library 647 passed/5 ignored, the six unchanged
`surface_policy_baseline` failures reproduced on the base, WAV five fixtures, audio inspection two
tests) were not rerun for this change, which touches no Server or audio code. Both Library live
suites were executed explicitly on the earlier head and were not rerun here. The public-catalog
live-origin opt-in remains NOT_RUN.

Android `cargo xtask theatre-host android-debug --api-origin http://127.0.0.1:8080` compiles the
production debug APK (SDK/build-tools 35) and its artifact inspection passes. The only permission
delta is `android.permission.ACCESS_NETWORK_STATE`, required by the process `ConnectivityManager`
registration; it is added to the exact inspector allowlist in `xtask/src/theatre_host.rs` and the
exact manifest expectation in `scripts/test_theatre_android_host.py`. A negative control in
`scripts/test_inspect_android_release.py` shows the delta is accepted only when allowed and that
`CAMERA` stays rejected in both `uses-permission` and `uses-permission-sdk-23`. No inspection rule
is bypassed. Script-suite and handoff counts are recorded in the PR for the final diff, not here.

The read-only `cantos-code-review` pass covers the final production/test changes and callers;
no backend business semantics, schema, credentials, player or second sync owner is introduced.
Acceptance remains incomplete because of the explicit Android inspection/scope blocker and
unrun iOS/device layers. This is draft handoff evidence, not production or whole-issue acceptance.

Android/iOS device lifecycle, OS connectivity, background/locked playback, secure-store and
process-death acceptance remain NOT_RUN. Restore the installed pinned Native compiler, then run
the iOS framework build and named simulator/device scenarios; run the compiled Android host on
a named device/emulator through foreground and network loss/restoration. Studio Catalog/Content
and creator-to-publication remain separate backend follow-ups, not implied by the fixture seed.
Queue order/rationale is unchanged; this bounded repair does not close #335 or #336.
