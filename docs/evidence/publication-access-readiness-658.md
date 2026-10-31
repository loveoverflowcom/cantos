# Publication/access integration readiness — S12 / #658

**Readiness: NO_GO.** This source-bound qualification contributes to
[#658](https://github.com/loveoverflowcom/cantos/issues/658) and
[#634](https://github.com/loveoverflowcom/cantos/issues/634); it does not close either issue.
A stale Server/Identity pair resurrects a session that was subsequently logged out and an
account that was subsequently disabled. Default restricted serving stays closed, but that
fence is not complete application-session recovery. No production activation, deployment,
security protocol or live data change is delivered.

## Provenance and review boundary

Base: `origin/develop@1c2c867114f181207950e02989dde113da587309` after #657/PR #681.
Target: macOS 15.6 (24G84), arm64; Rust 1.98.1; exact Turso 0.8.3, default features off,
MVCC/FTS off. Dev/test debug information and incremental compilation off; two Cargo jobs;
`TMPDIR=/private/tmp`. The PR pins its final head/merge. [Source hashes](publication-access-658/source-pins.json)
identify the tested dirty Rust inputs independently of packaging commits. The preserved
Identity executable SHA-256 is `ffe5720c23042467c0112d297b4806a3cd734055aec3fd35c675e065a6a150f2`.

The production domain, handlers, schema, contract, clients and security protocol are unchanged.
Two feature-gated test-support methods call the actual verified backup owner and copy a closed,
hash-checked backup into a new private disposable file. The new router/process recovery test
and old-registry refusal test run in the existing Theatre business journey. One bounded
regression repair updates the independent schema-table oracle with the three existing v35
tables. Two S06 registry index drifts are also corrected: the existing viewer-access capability
is added to the explicit test counts and the root inventory links its existing module document.
Registered SQL/history, model semantics/lifecycle and qualification baselines are unchanged.

Owner/catalog/audio/rights/QC are synthetic seeded fixtures. The viewer signs up with a
run-owned loopback SMTP inbox and earns a real Argon2id/Bearer session through the real
Identity executable/private RPC. Server requests drive the production Axum router in-process;
they are not Server-binary/TCP or browser requests. The exact admitted media bytes are
transport fixtures, not inspected/decoded audio or named human listening. No mail leaves the
machine. Databases and closed recovery copies are disposable; no credential/database is
committed. Only sanitized hashes/state/status receipts are retained here.

## Recovery observations

Test: `restricted::recovery::paired_restore_keeps_private_delivery_closed_and_exposes_stale_security_recovery_gap`.
The test first admits exact Range bytes, snapshots both quiesced owners, revokes the viewer
grant, logs out the session and disables its Identity subject. It makes another verified pair,
then reopens new copies of three cuts. Every restored composition uses the ordinary closed
Server router; **none is opted into restricted delivery**.

| Cut | Restored Identity state/version | Same epoch? | Session GET | Shared metadata and old GET/HEAD/Range/conditional media |
| --- | --- | --- | --- | --- |
| After logout/revoke/disable | Disabled / 2 | Yes | 401 | 401; no audio |
| Old Server / current Identity | Disabled / 2 | Yes | 401 | 401; no audio |
| Old Server / old Identity | Active / 1 | Yes | **200 — security recovery FAIL** | 404; default restricted fence remains closed |

The [actual sanitized receipt](publication-access-658/recovery.json) records four individual
backup SHA-256/length receipts, Server v35/Identity v4, the Identity epoch/security versions,
grant-revocation counts, exact metadata generation/order/audio pins and immutable
release/clearance/approval payload hashes. The copies preserve these values. Public list/detail
also disclose no restricted Work. Original snapshots stay unchanged during the drill.

The regression's assertions PASS because they reproduce those exact observations. Its own
receipt and stdout explicitly report **stale-pair safety FAIL / NO_GO**. This is not a test
that accepts HTTP 200 as safe recovery. `IdentityStore::open` documents the absent restore
rotation; Server has no independent retained revocation floor when both files roll back.
Implementing/accepting rotation, resync and post-cut policy reconciliation needs the security
owners; that work is outside this task's credential/security-change grant. No activation
override, new security design, NOT_APPLICABLE file or waiver is supplied.

The second new test,
`restricted::recovery::current_backup_refuses_pre_epic_binary_and_wrong_service_without_rewriting_bytes`,
uses the actual Server v30 registry prefix against a verified v35 snapshot. Serve and Migrate
both refuse `NewerThanBinary { applied: 35, known: 30 }`; the Identity registry refuses a foreign
application. Every refusal preserves snapshot digest/length. This is registry behavior, not
an old executable or production rollback rehearsal.

The [operator runbook](../deployment/publication-access-recovery.md) keeps stale pairs offline,
states the unsupported coordination/rotation/media-restore steps, and proposes RPO/RTO/retention
for owner review. No proposed value is accepted or used as a gate.

## Predecessor attribution

Latest issue bodies/comments for #647–#658 and #634/#676 were read in this session. Existing
checked items are preserved. The [merge/head map](publication-access-658/children.json) comes
from actual fetched develop history; merge status is not acceptance. Prior results below are
their authors' source-bound evidence, not S12 reruns unless the commands below say so.

| Slice / PR | Delivered head → merge on develop | Evidence retained / remaining boundary |
| --- | --- | --- |
| #647 / [#661](https://github.com/loveoverflowcom/cantos/pull/661) | cbe93cbc → 557de1a9 | Accepted ADR0054 D01–D08 and handoff; performance and recovery values still pending owner. |
| #648 / [#664](https://github.com/loveoverflowcom/cantos/pull/664) | 77b9d649 → 9f79067f | Generation/CAS/read authority; actual current Catalog/native tests include this mechanism. No complete client/recovery acceptance follows. |
| #649 / [#668](https://github.com/loveoverflowcom/cantos/pull/668) | 7b19a480 → 4721c3c6 | Exact/display-only association and same-transaction audio activation; original comment separates Turso/HTTP from later bytes and recovery. |
| #650 / [#672](https://github.com/loveoverflowcom/cantos/pull/672) | 8aa5476d → e09bd769 | Narrow saved Studio preview; its browser ran against a synthetic API. Current eight real-Turso preview tests are separate. |
| #651 / [#674](https://github.com/loveoverflowcom/cantos/pull/674) | 75e2890a → 10e9825f | Studio publication and synchronized generic Discovery readers; AC01/03/04 already checked. The evidence-only 96c00670 commit postdated that merge; do not claim it was the merged head. Confirmation/recovery and native acceptance remain open. |
| #652 / [#675](https://github.com/loveoverflowcom/cantos/pull/675) | 612d823f → 989d2c17 | Viewer mailbox claims/groups; ten Kani obligations and denied N/A/checklist actions remain unresolved. |
| #653 / [#677](https://github.com/loveoverflowcom/cantos/pull/677) | 8fc73fc → 28eb4d1d | Real Identity/HTTP/bytes serving with per-fetch admission; restricted default OFF. Private cover/Library restore unavailable. |
| #654 / [#678](https://github.com/loveoverflowcom/cantos/pull/678) | eeacdc52 → 8b76e2bf | Sharing/Shared Web source; private Web Play locked after teardown panic. Group impact/share-history contract and denied browser actions remain open. |
| #655 / [#679](https://github.com/loveoverflowcom/cantos/pull/679) | 5ac74ad5 → a1534cff | JVM/Android packaging evidence; private native Play locked without per-fetch Bearer/redirect-safe transport. iOS unsupported at the recorded toolchain. |
| #656 / [#680](https://github.com/loveoverflowcom/cantos/pull/680) | ff4c1853 → 027f9b72 | [Unit/HIDE source and bounded browser record](unit-policy-hide-656.md); private transport/device/formal/UI qualification still owed. Three v35 tables were missing from the schema-test expected set; S12 corrects that oracle only. |
| #657 / [#681](https://github.com/loveoverflowcom/cantos/pull/681) | b3dda1d5 → 1c2c8671 | [Actual plans/measurements](publication-access-performance-657.md); no runtime index/cache adopted, budget pending. 20/200 Works; Shared-200 p95 54.185 ms, 296 statements/21 candidates; not a production SLA. |

## Epic AC-01..26 ledger

`MET` below applies to the criterion's named native/source mechanism only. Partial, failed,
unrun and unsupported layers retain their gaps. In particular, current backend tests do not
qualify browser/native transport, stale-pair recovery or production. ADR0054/0055 replace the
old AC19/20 wording requiring compatibility/backfill; no legacy lane is restored for a tick.

| Epic AC | Disposition | Actual source / check / target | Remaining acceptance / next owner |
| --- | --- | --- | --- |
| 01 Design authority | PARTIAL | ADR0054/0055; publication-access-handoff and this runbook; #647/661 | D08a performance and D08d recovery values need owner acceptance; implemented/verified limits remain explicit. |
| 02 Revision ownership | MET | Catalog heads + immutable metadata-generation/audio identities; metadata_publication_tests and current Server lib on real Turso | No second draft head or save-triggered publication; other criteria still govern client/runtime acceptance. |
| 03 Draft isolation | PARTIAL | Metadata generation tests and Catalog HTTP 62; prior #651 real loopback r3/r4 | Complete lifecycle/privacy/browser and restored stale-state matrix not accepted. |
| 04 Coherent publication | PARTIAL | Generation/Unit CAS, pinned selection/order and exact association in Server lib/Catalog HTTP; #656 | Full selected-row UI/concurrent interaction and 1,000-entry operational qualification outstanding. |
| 05 Transaction/retry | MET | metadata_publication_tests: lost acknowledgement/reopen, duplicate/changed payload and competing publishers; current Server lib + Catalog HTTP | Real-file native mechanism only; no client timeout/production acceptance inferred. |
| 06 Metadata/audio separation | PARTIAL | Current publication/router and native generation tests; strict Web/CMP readers in #651/655/656 | Full Android/iOS runtime and current browser lifecycle pending; private Play stays closed. |
| 07 Audio compatibility | PARTIAL | metadata_audio_tests, actual publication HTTP and exact byte journeys; immutable hashes in new recovery receipt | Complete restore, media decoding/human listening and deployed acceptance pending. |
| 08 Lifecycle actions | PARTIAL | Audience/revoke/withdraw/replay native tests in current lib/journey; Studio request/core evidence #651/654/656 | Full browser confirmation/conflict/unknown-result campaign and group impact remain incomplete. |
| 09 Secure preview | PARTIAL | studio_preview_turso 8; narrow DTO/no-store/referrer and exact-revision authority; #650 | Member asset previews unavailable; real-stack browser account cleanup and deployed headers pending. |
| 10 Studio UX | PARTIAL | Source/reducer/JVM/browser records #651/654/656 | Historical field-text diff contract, group impact, large paging, complete keyboard/touch/AT/motion matrix pending. |
| 11 Verified email | PARTIAL | Viewer-access core/Turso and real Identity claim tests in predecessors; current Identity/HTTP regression | Restore/epoch gap prevents whole-lifecycle acceptance; pending/history UI and browser-denied repeat remain open. |
| 12 Viewer groups | PARTIAL | Owner-scoped typed grants/membership/current gates; current native suites and #652/656 | Affected-Work impact contract/UI and full account/device matrix pending. |
| 13 Work AND Unit / HIDE | PARTIAL | Current Unit policy truth table, Turso HIDE, actual old-public/private fetch refusal; Web/CMP source tests #656 | Full browser mutation/paging/native/a11y observations pending. |
| 14 Restricted bytes | PARTIAL | Current Identity/router GET/HEAD/Range/conditional and post-open negative controls; exact transport bytes | Trusted HTTPS browser media, safe Android/iOS fetch/seek/refresh and full restoration unqualified. |
| 15 Revocation/recovery | FAIL | New three-cut real-file/process/RPC/router drill: stale pair session 200 after logout/disable | Identity + Server recovery owners, unassigned: rotation/resync and trustworthy post-cut policy/revocation reconciliation before exposure. |
| 16 Public search | PARTIAL | Catalog HTTP/native HIDE, literal Vietnamese query cases and #657 actual plans/equality oracles | Full concurrent performance and deployed/browser matrix pending; no O(1)/FTS/cache claim. |
| 17 Shared discovery | PARTIAL | Current shared router + #654/655 session-scoped readers/cursors/login return | Native/browser account matrix and private durable Library unsupported; no OS-link/claim parity inferred. |
| 18 Cache/privacy | FAIL for stale-pair sessions; otherwise PARTIAL | #654–656 RAM/cursor/late-result purge; default closed private restore; new session rollback observation | Recovery fix plus named browser/native logout/account-switch/OS cleanup qualification. |
| 19 Contract evolution | PARTIAL | ADR0055 synchronized one representation; strict current fixture tests and prior Web/CMP build evidence | iOS compile/runtime and complete every-current-client delivery qualification outstanding; unsupported old selectors fail closed. |
| 20 Migration/rollback | FAIL for security recovery; otherwise PARTIAL | Historical native migrations/current schema guard tests; new v30/v35 and wrong-service refusal preserves bytes | No backfill required by accepted D07. Actual old executable, live coordinated pair/media recovery and rotation/resync unsupported/unrun. |
| 21 Behavior SSOT | PARTIAL | Draft viewer-access/playback/session manifests, named runs/witnesses/controls; predecessor model-only results | Metadata-generation model:null; listening needs-decision; native client observer and actual complete implementation conformance absent. Ten inherited Kani obligations remain owed. |
| 22 Backend qualification | PARTIAL | Current Server lib + 8 integration targets; actual Identity RPC/HTTP/bytes and disposable backup receipts separated | Failed stale recovery; full Server binary/TCP, crash/pair consistency, remaining formal/aggregate and production qualification pending. |
| 23 Browser qualification | PARTIAL | Source-bound #650/651/654/656 observations, with real/synthetic distinctions below | Full trusted HTTPS/media/confirmation/account-switch/touch/keyboard/locale/theme/text/motion/AT matrix; honor existing denials. |
| 24 CMP qualification | PARTIAL | #655/656 JVM/Android build evidence retained separately | Android runtime NOT_RUN; iOS recorded UNSUPPORTED; each platform's media/seek/revoke/cleanup/AT needs named qualification. |
| 25 Performance | PARTIAL | #657 exact pinned engine/source/seed/20 cases/19 plans, comparable experiment with no adoption | Owner budgets; populated Library, CPU/scanned rows/per-index/write contention, 1,000-size and concurrent performance still pending. |
| 26 Handoff/rollout | MET as dossier, NO_GO as product | This source-bound ledger, sanitized receipts, operator runbook, exact local commands and #658/#634 evidence sync | Production effects excluded; #676 is the single residual-review index, not a replacement acceptance owner. |

## Browser and native pending matrix

No S12 browser/device run is claimed. UI/runtime source is unchanged by S12. Existing evidence is
reused only at its original target and source; it is never promoted from screenshots/JVM to
interaction/device acceptance.

| Target / journey | Held evidence | Precise pending criterion and resume |
| --- | --- | --- |
| Studio save → exact preview → selected publication | #650 real WASM/synthetic API; #651 bounded real loopback r3/r4/order; #656 real owner-cookie matrix GET/draft retention at 1280×720 and 390×844, light VI | Current admitted disposable stack: named save failure/unknown/replay/CAS/confirm/withdraw/archive and >20-row matrix; assert buffer/pins and observable public state. Full VI/EN/light/dark/narrow/wide/real 200% text/keyboard/touch/reduced-motion/AT still pending. |
| Studio sharing / Theatre Shared | #654 bounded real Identity owner→pending→claim→metadata and focus/draft observations | Complete group impact/history contract, current-denial/account route cleanup and late responses. **Public-audience mutation, final pending repeat and different-account browser sign-in were denied; no retry or substitute fixture/API was attempted.** New materially relevant authorization is required for those exact actions. |
| Theatre private HTML media | Source retains credentialed CORS selection; visible/imperative Play locked | Fix/review teardown before enable; qualify actual trusted HTTPS host-only cookie and HTMLAudioElement GET/Range/seek/refresh/revoke/withdraw/wrong-session, late results and stopped-source cleanup. No public or bearer-URL fallback. |
| Android | #655/656 actual debug packaging/APK inspection; JVM shared/controller/semantics tests with disclosed doubles | Implement safe native per-fetch Bearer/exact-origin/redirect-refusing transport, then `cargo xtask theatre-host android-debug --api-origin URL`; separately install/run a named authorized target and observe seek/revoke/account-switch/Back/background/late callback/large text/TalkBack. Build/JVM are not runtime acceptance. |
| iOS | #655/656 recorded UNSUPPORTED Kotlin/Native 2.2.20; Swift parse only | Provide the exact supported prepared toolchain, run `cargo xtask theatre-host ios-build --kotlin-native-home PATH`, compile Swift host, then the same named AVPlayer/seek/credential/cleanup/Dynamic Type/VoiceOver matrix. No SDK/toolchain installation or substitute version was performed here. |
| Private covers and durable Library restore | Unavailable/default-deny | Service-owned contracts and current admission before source implementation; qualify every restored private row/asset per session and never infer rights from disk JSON/history. |

## Commands and local results

All commands ran at the S12 worktree root. Full logs, earlier failures and manifests live in
ignored `artifacts/agent-runs/658/`. The exact environment is stated above; explicit
`CANTOS_TEST_IDENTITY_BINARY` selected the preserved compiled artifact. Socket/process tests
ran with the granted local sandbox escalation after an initial loopback bind EPERM.

```sh
export RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
cargo build --locked -p cantos-identity --features service --bin cantos-identity
export CANTOS_TEST_IDENTITY_BINARY="$PWD/target/debug/cantos-identity"
cargo test --locked -p cantos-server --lib
cargo test --locked -p cantos-server --test theatre_business_journey --test server_schema_history_turso --test server_turso_barriers --test studio_preview_turso --test identity_password_turso --test editorial_recovery_turso --test catalog_http_turso --test publication_contract
cargo xtask verify turso
cargo xtask preflight --changed-only --base origin/develop
```

| Run / independent layer | Actual status and scope |
| --- | --- |
| Server library | **847 PASS, 13 ignored.** Pure/domain and real-file library cases retain their named boundaries. |
| Selected Server integration targets | **108 PASS, 3 ignored**, attributable across the initial and continuation runs, not a single green aggregate. Catalog62, Editorial recovery2, Identity password19, Publication2, schema4, barriers3, Studio preview8, Theatre business8. |
| Initial integration run | **88 PASS / 1 FAIL** before stopping at schema. The expected-set oracle omitted `catalog_unit_viewer_policies`, `catalog_unit_viewer_grants`, `catalog_viewer_live_clock`. Exact failure reproduced on clean develop; original log retained. Three explicit names added to expected tables, no SQL/registry change. Affected schema4 and remaining targets then PASS. |
| Recovery drill | Observation assertions PASS; **stale-pair session safety FAIL / NO_GO**. Current/mixed revocation denial and default restricted fence PASS only within their stated cuts. |
| Turso gate / Identity RPC | **PASS: 376 tests, 2 ignored**, counting distinct persistence/service feature builds. Includes real Identity process/RPC authorization/wire cases, store ownership/backup tests and capability report. Three raw engine hazards report their expected FAIL; zero expectation deviations and the actual gate summary is PASS. The first attempt passed 324 Identity tests but failed to link the store capability target with ENOSPC/errno28; retained, then the complete gate PASS after task-cache cleanup. |
| Server lint / format | Final all-Server-target Clippy PASS after both fixture path corrections; format PASS. |
| Native isolation guard | **UNSUPPORTED**, accurately: final exact selected test 1 PASS, executable/credential controls PASS, macOS network isolation UNSUPPORTED. [Sanitized receipt](publication-access-658/isolation.json) pins both executables and the dirty input snapshot. No Linux egress or whole-candidate acceptance. |
| WAV differential reference | **PASS**, five actual PCM cases against Python wave/hashlib plus ffprobe. This resolves the previously uncompleted local run at this source; older failures remain retained. It is structural/hash/sample-peak evidence, not listening/loudness/QC. |
| Tool-backed audio inspection | **2 PASS**, actual mono Ogg/Opus profiles and wrong-container/codec/channels/corruption negatives through local FFmpeg/ffprobe. Structural byte admission only; no human listening or production rights/QC. |
| Final preflight | **FAIL**: repository has the same **66 UI qualification findings** in unchanged UI paths; later planned gates NOT_RUN. Assurance-decisions also FAIL for the two selected model verifiers left NOT_RUN by that aggregate, despite their separate actual model-only PASS. No decision/waiver/hash edit is supplied. The initial configured-runtime attempt was UNSUPPORTED before repository ran; exact tooling then reached this final repository failure. |
| Model/Kani | Viewer-access and public-playback-delivery **PASS model-only** with actual named runs/witnesses/controls and Node24.19.0/npm11.9.0/Quint0.33.0; [report hashes](publication-access-658/models.json), full traces retained locally. No implementation conformance or exhaustive proof. Initial configured Node22 and bundled npm11.17 were UNSUPPORTED; exact task-local tooling then ran the models. Ten Kani obligations and earlier denied N/A/checklist actions remain open. |
| Repository Python fixtures | Canonical full run: 1,337 tests, 3 FAIL, 18 skipped. All three failures reproduce on clean develop. Two #652 index/count drifts are corrected; **114 behavior/registry tests PASS** afterward. The unrelated legacy VPS fixture still fails because `flock` is absent. No current full-Python aggregate PASS, fake lock, skip or installer is claimed. An earlier default-TMPDIR run had 16 FAIL/87 errors from symlink paths and is retained. Python Kani fixtures are synthetic, not solver proofs. |
| Browser/Android/iOS/production | NOT_RUN in S12; prior source-bound browser/JVM/build/UNSUPPORTED results remain separately attributed above. No CI wait, remote workflow or production operation. |

Earlier failed/zero runs are retained: initial build used an incorrect test-only AccountState
import (corrected to its existing admission owner); the first exact short-name filter selected
zero tests (not acceptance); the next local test failed before effects at sandbox listener bind.
The permitted isolated rerun actually executed one test and the final suite executed both new
tests. The initial native-isolation run also FAILS at the fixture's noncanonical media path:
the guard deliberately uses `/tmp`, which aliases `/private/tmp` on macOS. A separate owned
path probe reproduces `InvalidIntegrity` before viewer enrollment. The bounded correction
canonicalizes only the new test's already-owned media parent and reuses that closed host
instead of a second noncanonical constructor; production storage validation is unchanged.
The final journey rerun is 8 PASS/3 ignored and the guard reaches 1 passing selected test;
overall guard status remains UNSUPPORTED as recorded above. Both earlier guard FAILs remain.
No automatic approval rejection occurred in these runs; the inherited denials remain
unchanged and were not retried.

## Behavior, assurance and completion boundary

Behavior lookup: catalog → viewer-access/public-playback/native-session manifests, inventories
and draft models → actual callers. Classification: missing real recovery scenario/coverage;
test-only orchestration and evidence correction, no changed production semantic rule. Existing
bounded models do not represent a two-file cut, missing post-cut revocation history or a restore
protocol. No model, lifecycle, witness, negative control or expected security policy is edited
to bless the observed hazard. No new verifier, cross-service file read or external dependency.

L1/L2: existing owner/authority/immutability boundaries preserved; no new domain rule or data
shape. L3: native library/targeted regression and static checks as reported. L4/L5: new actual
file/process/RPC/router/bytes observations; source/client/model/browser/device/production remain
independent. Retained release, rights and approval payload hashes and generation pins remain
unchanged through the disposable cuts. No release is produced by S12 and no live rights/QC or
human-listened quality is asserted. No runtime performance behavior changed; reuse S11's exact
report and its unaccepted budgets.

The ordered residual list and prerequisites are in the
[runbook](../deployment/publication-access-recovery.md#ordered-review-and-handoff). Security
rotation/resync, native/Web transport and required formal/UI qualification remain blockers;
their owners are unassigned where the prior handoff did not name one. Keep the epic open and
every unmet criterion unchecked. #658 owns this integration record; the parent serializes the
pending update to the one #676 ledger.
