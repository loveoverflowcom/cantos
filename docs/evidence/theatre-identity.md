# PR3 Theatre identity evidence

Current resumed status is in the [14 October appendix](#resumed-native-identity-acceptance--14-october-2026); earlier sections
retain the results and blockers of their original snapshots.

Review boundary: separate opt-in Web BFF→actual sealed Library identity, Web/CMP account lifecycle
cores and native OS/protocol groundwork. Native resource IO is **incomplete/fail closed**, not
accepted from device absence. [Architecture](../architecture/theatre-identity.md),
[contract](../../packages/contracts/theatre-identity-v1.md) and
[successor handoff](../work-plan/theatre-identity-handoff.md) own implementation versus deferred scope.
No real provider/client/grant/secret/TLS/network/container/account was provisioned or activated.

## Provenance and invariant ledger (L1–L5)

Mac executor; isolated `/private/tmp/cantos-theatre-identity`, branch `feat/theatre-identity`.
Base/dependency exact heads and normal merges are pinned in the handoff. PR98 is inherited;
PR91/93/95 are now develop base. Native reference code: local VOT Workspace
3de94ae93790550d1edcb4d86fce17d005fbbd29, read only. Migration0021 is new; historical SQL remains
byte-identical. Rust1.87.0, CARGO_BUILD_JOBS=2, RUST_TEST_THREADS=1, TMPDIR=/private/tmp;
existing PostgreSQL17.4/FFmpeg fixtures only. Every PG gate created/stopped a disposable cluster;
no inherited URL, persistent role setup or existing DB/evidence deletion. Logs stay outside Git.

| Claim / layer | Oracle held | Disposition / limit |
| --- | --- | --- |
| L1 separate surfaces/purposes | Source/type review, fixed clients/redirect/scope and separate SQL; actual PR95 sealed seam | held for Web; Admin/native resource IO default deny |
| L2 native code response is not Library authority | Private verified ES256 response; no principal/session conversion or native mount | type-enforced; native trusted transport/durable flow/resource proof missing |
| L2–L3 signature/issuer/audience/azp/nonce/time | Actual ephemeral P-256 signed tests on production shared verifier, independent Theatre Web/native audiences | PASS3 Theatre; inherited Studio7 retain cryptographic refusal/rotation/clock evidence |
| L3 Web generation/account/intent law | Pure production AccountState examples and bounded switch schedules | Original2 PASS; final added stale-session/pending-cookie-cleanup cases rerun in final Rust gate |
| L3 CMP callback/cancel/replay/account law | Common production code compiled with cached Kotlin2.2.20 + JUnit4.13.2; fake entropy/browser only | PASS5 before final stale-callback refinement; fresh rerun recorded below; not device or secure-store IO evidence |
| L4 provider exchange/orchestration | Production ES256/HTTP transport + named signed loopback HTTP mock, code/PKCE/redirect/rotation/revoke | mock-integration-tested; no live Kanidm |
| L5 Web registry/session/Library | Actual disposable PG + Axum + signed HTTP mock; issuance race, actual login/callback/logout, opaque cookie, actor lock disable, cross-surface denial | PASS5 on earlier combined run; final adds real A/B immutable-receipt isolation; final result below |
| L5 combined original migrations/Studio | Actual upgrade/replay/current registry tests | Original bytes preserved; exact membership now1–14 plus16–21; no private storage0015 |
| Native OS adapters | Android Kotlin compilation against SDK35; Swift typecheck against iOS18.4 Simulator SDK | compiled only; no app launch, browser, Keychain/Keystore IO, device, screen reader or native audio claim |
| Account/outbox | Immutable account-owned body/op, receipt-first after dispatch; old generation cannot project/acknowledge asB | memory-only helper; durable outbox/restart and actual UI/transport sync deferred |
| Independent review | Self source review plus actual gates; parent Ultra review planned afterPR4 | gap-required for independent combined campaign acceptance; no independent reviewer claimed |

## Commands and original failures retained

Raw commands use the environment above and shared Cargo build cache only at
`/private/tmp/cantos-identity-domain/target`; this worktree's evidence directories are independent.
No source/evidence from previous worktrees is rewritten.

| Command / log | Actual result |
| --- | --- |
| `cargo test --locked -p cantos-api --lib platform::studio_oidc`, `/tmp/cantos-theatre-shared-studio.log` | PASS7 after extraction; initial sandbox HTTP bind EPERM retained in initial logs |
| `cargo test --locked -p cantos-api --lib platform::theatre_identity`, `/tmp/cantos-theatre-native-verifier-final.log` | PASS3; actual native/Web ES256 refusal tests, no resource admission |
| `cargo test --offline -p cantos-theatre --lib application::account`, `/tmp/cantos-theatre-account-core.log` | PASS2 initial core; final four-case scope rerun below |
| Kotlin2.2.20 compiler + JUnit4.13.2, `/tmp/cantos-theatre-kotlin-adapters.log` | PASS5 common tests; Android source compiled with SDK35 stubs; no Android runtime or Gradle build |
| `xcrun swiftc -typecheck -sdk <iPhoneSimulator18.4.sdk> -target arm64-apple-ios18.4-simulator -module-cache-path /private/tmp/cantos-theatre-swift-cache .../TheatreIdentity.swift` | PASS source typecheck, `/tmp/cantos-theatre-swift-check.log`; unmounted Swift source, not simulator launch |
| `cargo xtask verify postgres`, `/tmp/cantos-theatre-postgres-first.log` | FAIL new BFF direct actor-lock permission; fixed via existing narrow Library actor-lock function, no widened actor UPDATE |
| PostgreSQL second/third/fourth/final/combined logs | Fail-before retained: missing test read-function grant, stale migration membership/count and FK TRUNCATE oracle; final combined reached existing Publication500-versus503 assertion |
| `cargo xtask preflight --changed-only`, `/tmp/cantos-theatre-preflight-full.log` | UNSUPPORTED compatible Gradle missing; all remaining selected gates NOT_RUN |
| Available preflight logs `/tmp/cantos-theatre-preflight-available.log`, `...-final.log` | FAIL own duplicate test module / item order; fixed structurally, no lint suppression |
| Available preflight `/tmp/cantos-theatre-preflight-release.log` | Rust format/lint/tests PASS; DBSP FAIL due disk exhaustion and partially linked nonexecutable fixture; remaining gates NOT_RUN |

The Publication rollback fixture raised a generic SQL exception. Unchanged `StoreError::from`
classifies that as Unavailable and unchanged HTTP maps it to503; its old500 oracle was inconsistent.
Only the test expectation is corrected to503 plus exact `unavailable`, while rollback row equality
and original-operation retry remain asserted. Source comparison against develop confirms those
production conversion/mapping paths are unchanged. This is not a new500→503 API behavior change.

Disk was207MiB after DBSP compilation. Only new incremental compiler directories whose birth time
was after this task's initial compile were removed, with path/size ownership record
`/tmp/cantos-theatre-owned-cache-cleanup.json` (778MiB). No pre-existing cache, DB or evidence was
removed. Remaining runs use CARGO_INCREMENTAL=0 and keep DBSP FAIL visible; optional broad mutation
is explicitly NOT_RUN for resource budget, with no Script IR change.

## Final results and required follow-ups

Final gate records are appended after execution. Full repository's known Mac killpg EPERM fixture
must remain FAIL if reproduced; no assertion or gate is suppressed. Full unskipped preflight stays
UNSUPPORTED until compatible installed Gradle is available. All unknown/missing results are gaps.

Residual risk: native public-client exchange/resource verifier/revocation/registry consumer and
host wiring remain incomplete; Web port not mounted in mock UI; durable outbox/UI account sync,
actual provider/TLS/grants, two-device/two-tab races, secure-store lifecycle, native app/device/audio,
accessibility, performance/retention/encrypted backend bearer/pending recovery and parent independent
Ultra review remain deferred. This draft is implementation evidence, not production acceptance.

### Final publication ledger

These final rows supersede provisional results above.

| Gate / named scope | Final observed result | Retained log |
| --- | --- | --- |
| repository static / services | PASS: corrected inward adapter/persistence placement and narrow composition; pure-core/Table ownership regressions pass | `/tmp/cantos-theatre-repository-gate-final.log` |
| repository overall | FAIL:316 Python tests,37 Mac killpg EPERM errors +1 related fallout; no new architecture test failure | same log |
| Rust format / workspace lint | PASS available preflight and final formatter check | `/tmp/cantos-theatre-preflight-bounded.log` |
| workspace Rust tests | Earlier PASS; final attempt FAIL at link due disk exhaustion | `/tmp/cantos-theatre-preflight-release.log`, `...-bounded.log` |
| final Web account pure source | PASS4 directly compiled from actual production account.rs with Rust1.87 and locked UUID library; not a copied model/UI test | `/tmp/cantos-theatre-account-pure-final2.log` |
| final CMP pure/account/Android source | PASS5 common JUnit tests; Kotlin2.2.20 compilation of actual common/Android adapter sources against SDK35 | `/tmp/cantos-theatre-kotlin-final.log` |
| native/Web verifier core | PASS3 actual signatures, with7 retained shared Studio tests; native response grants no Library authority | `/tmp/cantos-theatre-native-verifier-final.log` |
| full combined PostgreSQL earlier | PASS, including actual Library7 regressions and existing restart/socket/Studio paths after minimal integration repairs | `/tmp/cantos-theatre-postgres-publication.log` |
| final PostgreSQL current Theatre slice | PASS7 actual SQL+HTTP, current cap/lease/clock code, A/B immutable-receipt isolation and expiry after actor wait | `/tmp/cantos-theatre-postgres-final-seven.log`, `target/revision-evidence/theatre-identity-postgres-tests.log` |
| final PostgreSQL overall | FAIL later in existing Catalog/Brief suite: disposable cluster WAL E58P01/recovery failure under exhausted disk and23 follow-on failures | same final-seven and cluster logs |
| Web WASM lint/typecheck | PASS Studio/Theatre, including real same-origin account port | `/tmp/cantos-theatre-web-final.log` |
| Web build | FAIL Studio LLVM ENOSPC; Theatre/UI preview NOT_RUN after failure | same Web log |
| contracts | script-ir-reference PASS; contract-tests FAIL linker ENOSPC; WAV/audio NOT_RUN | `/tmp/cantos-theatre-contracts-final.log` |
| DBSP | FAIL LLVM ENOSPC/partially linked fixture; not waived by later skip | `/tmp/cantos-theatre-preflight-release.log` |
| full unskipped preflight | UNSUPPORTED compatible installed Gradle absent; other gates NOT_RUN | `/tmp/cantos-theatre-preflight-full.log` |
| broad mutation / Kani | NOT_RUN: resource budget; no Script IR changes/new formal claim | explicitly skipped bounded preflight |
| Swift iOS source | PASS typecheck; no app/simulator/device/Keychain IO claim | `/tmp/cantos-theatre-swift-check.log` |
| Kanidm / UI/CUA / secure-store IO / native app/audio / independent review | NOT_RUN; native resource/exchange consumer is incomplete code, not just missing environment | handoff |

Current xtask accepts installed Gradle8.13/8.14.1/8.14.2/8.14.3; none is available. Later cleanup
removed only exact newly generated test binaries attributable to this task's retained logs, with
ownership JSON records/tool output. Source, databases, evidence and pre-existing artifacts were
preserved. Standard xtask reaping concerns only completed test databases inside its own fresh cluster.

Architecture checks caught initial adapter ownership and SQL in HTTP. Adapter now belongs to
Listening behind its auth facade; persistence owns the lease. Shared pure OIDC is checked at its
actual new path. HTTP still cannot execute SQL, import lower adapters or mint an actor principal.
Admission is capped at3 leases in the existing8-connection pool with a5second permit deadline;
expiry is a separate statement after actor wait, exercised by the seventh actual PG regression.

Final scope: Web backend/session/sealed seam implemented; Web runtime mounting/full UI sync deferred.
Native OS/protocol/verifier groundwork implemented and source-compiled; public exchange/resource
revocation/registry consumer and host wiring remain incomplete. Native response is not authority;
PR3 native integration is not complete. Parent Ultra independent review remains required afterPR4.

### Independent campaign native completion — code awaiting fresh gates

The campaign review found missing native exchange/resource/host consumers and a Web issuance
expiry gap after INSERT/trigger waits. Migration 0024 preserves 0021 and adds final rollback,
distinct native flows/sessions and narrow issuance. Native public-client code now supplies
bounded discovery/JWKS/exchange without a secret, actual access-JWT purpose/signature checks,
current introspection, exact registry issuance, a separate opaque session and current Library
admission. Native API DTO/schema and direct production-core/HTTP/PG regressions were added;
their existence is not execution evidence.

Fresh gates are **NOT_RUN in this implementation subtask** until the parent resource lane runs
the current combined source. Named targets: access signature/token confusion; public mock
refusing Authorization/client_secret; actual flow/exchange/session/Library HTTP with disposable
PG; replay/unknown exchange; remote revoke/local disable; stale session fence refusing writes;
Web final-expiry registration rollback. Prior result tables remain historical, not current
integrated acceptance.

Android manifest source activation was rejected twice by automatic approval review as a
network-settings change. The unapplied proposal is outside Git at
`/tmp/cantos-native-android-manifest-proposal.md`; direct human exception is pending. Common
transport/controller, OS adapters and iOS host work continued. No manifest workaround, provider
registration, deployed credentials/TLS/network mutation or container was performed. Native
runtime/device/provider/secure-store evidence remains NOT_RUN.

The response-order regression `delayed_guarded_logout_and_completed_callbacks_never_expire_new_shared_cookies`
uses the actual Axum routers, disposable PostgreSQL and signed provider fixture. A Notify gate
holds A's remote revoke after its local commit while two initially anonymous callbacks complete;
the oracle rejects any late old DELETE cookie expiry and any completed callback global flow
expiry, checks B's current session and verifies known revoked versus arbitrary unknown begin.
Source was added before the corresponding fix; execution/fail-before reproduction is NOT_RUN
in this subtask. The existing stale-only fixture also requires provider outage to leave B
unrevoked with 503 and no cookie effects.
`guarded_inert_a_cookie_does_not_cancel_new_anonymous_b_flow` sends A's inert credential with
a newly installed B binder through both expected-session and stale-only cleanup. It checks the
actual SQL flow remains uncancelled and its HTTP callback/session succeeds. Source-only
NOT_RUN; it covers the distinct cancellation race that cookie-header assertions cannot detect.

The subsequent durable inventory regression
`immutable_guest_inventory_logout_cancels_paused_callback_and_permits_fresh_login` pauses the
second already-consumed guest callback in the real mock token endpoint, completes the first,
adds a third immutable binder, and acknowledges active expected-session logout with all cookies.
It requires the late callback to refuse without a new session/cookie, then admits a fresh login.
Migration 0024 adds source-flow association; the same deterministic flow-before-session lock
order covers pending callbacks and already-issued siblings. Production parser tests reject
malformed/duplicate/mixed/overflow inventories and mutable fallback. These are source-only
NOT_RUN until the parent executes fixed and retained fail-before sources.

`immutable_inventory_revokes_already_issued_sibling_even_when_its_cookie_response_is_held`
covers successful B issuance whose cookie response is withheld across A logout. B's late cookie
remains inert; unguarded HTTP DELETE is refused without effects. The outage/retry fixture
`inert_a_retry_finishes_captured_sibling_revoke_without_cancelling_fresh_b` requires durable
batch attribution to finish old sibling acknowledgements after old cookies are absent, while
fresh B remains admitted. Remote effects have an overall five-second budget.
The scoped prior guarded production-kernel overlay is preserved outside Git at
`/tmp/cantos-theatre-inventory-kernel-revert/manifest.json`; it intentionally retains the current
fixture/native/cookie syntax and is not an original whole-head snapshot. Only the parent may
temporarily apply it for actual fail-before execution and restore the recorded fixed hash.

Android secure storage now maps `ProviderException` (including Keystore backend busy) to typed
Unavailable without broad exception or coroutine cancellation catches. OS fault injection and
native builds remain NOT_RUN in this subtask.

`retired_a_cancels_only_captured_unexpired_rotation_receipts_and_preserves_observed_b` covers
the completed rotation lost-ack gap: root-serialized A→B→C issuance needs exact captured
unexpired flow binders for old A to cancel a matching receipt. Old A/UUID without B's binder,
and a raw expired-binder replay, both preserve observed B. The captured fresh C receipt is
revoked before its held cookie response can regain admission. Roots serialize; they confer no
actor-wide or permanent retired-session cancellation authority. Source execution is assigned
to the parent lane.

## Independent integrated acceptance — 13 October 2026

This ledger supersedes the provisional campaign statuses above for implementation source
`2a1a98546a04420638626ff6d05bbce550fb1235`, tree
`2e8178a6607dd212acd6d657c36d5ade486ad13c`. The historical implementation failures and logs
remain evidence of their own snapshots. Actual Web runtime used clean source
`0a4ee8fad30373a9f51f2117a5c731b066b3f7ea`, tree
`bcde33a85907aa291ed69d38c7ef140c0065f989`; the subsequent commit changes only a PostgreSQL
test oracle, with production/UI bytes unchanged.

Evidence root: `/private/tmp/cantos-identity-campaign/`. Final manifest
`pr101-acceptance-manifest.json` records **16 files**, SHA256
`e507afad65f29c01f9959b36f52de564171cce26ea0de27bda5e80fa2c216591`.
The earlier provisional manifest is preserved and is not the final acceptance record.

| Named scope | Observed result | Retained log in evidence root |
| --- | --- | --- |
| Fourth full local preflight | **FAIL overall: 16 PASS, 1 PostgreSQL FAIL**; the registry immutability oracle omitted the new Theatre flow FK graph | `theatre-preflight-integrated-fourth.log` |
| Corrected current full PostgreSQL verification | **PASS**: 15 Theatre Web, 6 native, 18 Studio OIDC tests, registry and all inherited PostgreSQL/restart/socket suites | `theatre-postgres-integrated-oracle-fixed.log` |
| Final test-only `preflight --changed-only --base 0a4ee8f` | **PASS: 8 selected gates, no skips**: repository, rust-format, cantos-api lint/tests, DBSP, WAV reference, audio inspection and PostgreSQL | `theatre-preflight-final-test-only.log` |
| Integrated Rust workspace | **PASS** lint/tests, including 107 Theatre Web unit tests and the 4 canonical account tests | `theatre-rust-integrated-first.log` |
| Web WASM lint/build | **PASS** all 4 named Web gates: shared Studio/Theatre lint, both app builds and UI preview | `theatre-web-integrated-first.log` |
| Reference/preview/CMP gates | **PASS** in the fourth full run: Script IR reference, Protobuf reference, CMP UI preview and Theatre CMP demo | `theatre-preflight-integrated-fourth.log` |
| Mutation | **PASS**: 9 killed, 0 survived, 0 timeout, 0 unviable; no new native formal-proof claim | `theatre-preflight-integrated-fourth.log` |
| Android SDK compile | **PASS — Theatre mock Android build**: 61 tasks executed, 52 seconds; shared protocol and Android adapters compiled | `theatre-android-integrated-final.log` |
| iOS simulator framework | **UNSUPPORTED**: required Kotlin/Native 2.2.20 lacks executable `bin/konanc`; no alternate version or download substituted | `theatre-ios-build-availability.log` |
| Actual Theatre Web runtime | **14 PASS, 0 FAIL, 2 INCONCLUSIVE, 7 NOT_RUN**; all 12 captured PNGs opened, inspected and SHA-verified | `theatre-ui-runtime/report.md`, `checks.json`, `image-inspections.json` |

This is **not a single 17-green-gate invocation**. Commit `2a1a985` expands only the inherited
TRUNCATE oracle's FK graph and retains its expected immutability error `55000`; it does not
weaken a production constraint or ignore a failure. The corrected full PostgreSQL verification
and final selected preflight establish the current test result separately from the retained
fourth-run failure.

The scoped prior guarded-production-kernel experiment also remains a named **FAIL**:
`theatre-guest-flow-kernel-fail-before.log` observes late callback 303 where 401 is required.
It reverted only the cancellation kernel under the current signed-mock fixture, not an original
whole-head snapshot. Fixed code was restored by hash before acceptance; the current 15-test Web
suite passes the pending callback, held successful receipt, fresh-binder expiry, A/B isolation
and attributed remote-revocation retry regressions.

### Actual browser scope

`theatre-ui-runtime/report.md` SHA256 is
`572c57bd2b156a6b10542918a52aaf24fa1b026f44aa0cdc78668788e11eb9af`.
The real built Web/Axum/disposable-PostgreSQL journey used an ephemeral ES256 provider over
actual discovery/JWKS/token/introspection/revocation HTTP. Trusted provider A/B chooser actions
redirected to the actual callback, exercising nonce/PKCE, exact registry issuance and BFF cookie
admission. Continue/Saved/History reads were actual authenticated empty-Library responses;
there was no synthetic publication/audio/private-entry seed or forged DOM/auth/network body.

Named observations include stable exact issuer/subject mapping, held A bootstrap during B
cookie rotation, the actual 409 private-request fence and bounded B rebootstrap, provider 503
with unavailable UI/recovery, delayed A 401 cleanup preserving B with 409, and local logout 204
with remote revocation **pending** followed by fresh official A login. Remote completion is
not inferred from that pending response. Trusted skip-link/Enter focus, Vietnamese text and
1440×900 light, 390×844 dark and 320×800 light layouts were observed. No localStorage keys or
fixture secrets appeared in the exported sanitized ledger. The run used 27 trusted actions,
12 PNGs and 10 minutes 4 seconds; zero runtime exceptions were recorded.

Native-select driver behavior and effective OS text enlargement remain **INCONCLUSIVE**.
The 7 **NOT_RUN** groups include 200% browser zoom, legitimate nonempty Library/public
audio/playback, a separate late-Library-success-after-logout journey, explicit mock-player
interactions, live-provider production acceptance, native device/system-browser/secure-store/
screen-reader/IME acceptance, and actual local 900-second expiry/SQL actor-disable runtime.
Provider inactivity was exercised and remains distinct from clock expiry or SQL disable.
The initial task-provider CSP fixture error and its exact callback-origin repair are retained
in the report; no browser security or TLS bypass occurred.

### Implemented code and remaining activation gates

The native path now implements fixed public-client discovery/PKCE exchange, real ID/access
signature and purpose witnesses, current introspection, exact registry issuance, separate opaque
sessions, fresh resource admission, local-first durable revocation, typed common consumers and
OS adapters. Studio's latest fences and cookie inventory, the shared OIDC proof boundary,
JSON-default/opt-in Protobuf and legacy MessagePack were preserved during the normal merge.
Migrations 0018/0020/0021 stay byte-identical; the pre-Admin registry has 22 entries, including
0024 Theatre and 0025 Studio final-time changes.

The Android INTERNET/callback manifest and dependent MainActivity/Application source activation
are **still unapplied** after automatic approval review rejected the source proposal as a
network-settings change. Exact records remain at `/tmp/cantos-native-android-manifest-proposal.md`
and `/tmp/cantos-native-auto-review-rejection-record.md`; direct human exception is pending.
Android SDK PASS does not establish an activated auth host, OS browser/store behavior or device
acceptance. iOS source wiring is not an iOS build PASS. PR101 remains draft pending this source
blocker; this handoff does not complete the whole mobile client.

Live Kanidm/client registration, provider/device/secure-store lifecycle, native playback,
accessibility and production activation remain **NOT_RUN/closed**. Durable outbox, guest import,
explicit cross-client linking, backend at-rest bearer protection, backup/retention and controlled
pending-revocation recovery keep their separate owners/gates. Equal client-specific subject
strings never authorize automatic issuer merging. No live account/grant/credential/TLS/network
mutation or provider container launch was performed.

All task-owned API/provider/runtime-PG and retained `--keep` verification-PG processes were
stopped; their databases, WAL and unique fixture/evidence data were preserved. Stop records are
included in the final manifest. Only the inspector's own disposable Chrome profile was removed.

## Resumed native identity acceptance — 14 October 2026

This appendix supersedes the held Android source blocker and provisional native gate status
for the resumed snapshot only. The 13 October hashes, failures, counts and Web runtime ledger
above remain historical evidence of their exact source. No new Web/device runtime result is
inferred from retaining that unchanged frontend.

Fresh `develop` storage commit `edbd7c7f6f0494e0d01dc5b4139134052b07ed07` was normally merged as
`b5ba2c58fb79687fc6dd5b21d01fcd274b9468ed`, tree
`fda7318054cb7f26fe45cc1af270d6ad235c4ce9`. The resumed source is
`1e2adae537d6b94ee824a5eb8f290a9dbdfa170a`, tree `ab556046cc5922d933c8792c6e7a037b2d595ec2`. The pre-final-gate nine-source manifest
is `resume-20261014/source-before-final-gates/manifest.json`, SHA256
`16bb38d01300da4dedb38b65a93b600fefb40eebdf9c6a9545eb247ad08eab4a`.
The current pre-Admin registry has **23** entries: 1–21, 24 and 25. Storage 0015 is now inherited
from develop; all existing SQL bytes/checksums remain unchanged and no migration 22/23 is
invented in this branch.

### Authorized source and confirmed repairs

The user explicitly authorized the held Android INTERNET permission, callback filter and
Application/MainActivity source patch. The same scoped source operation was successfully
retried; the earlier automatic-review rejection/proposal records remain preserved.
`resume-20261014/android-authorization-resolved.json` records the resolution, SHA256
`d47fe7fcb94ed69061050a9a614aed37887cc93c808a4a97419c5f05eb0cc415`.
This is repository source authorization. It registers no live OAuth client or origin, installs
no app, configures no persistent credential and changes no OS/device network or TLS setting.

The Android host now owns the opt-in controller at process lifetime, dispatches the exact
callback through `onCreate`/`onNewIntent` with `singleTask`, and mounts identity only when both
operator-supplied nonsecret HTTPS configuration values validate. Unconfigured/invalid hosts
retain the isolated mock. The raw callback-query guard rejects duplicate, encoded-alias,
unknown, malformed and oversized field names before Android URI value getters; it neither
verifies pending state nor creates account authority.

Two cleanup defects were repaired in the actual `NativeAccountController`: failed OS persistence
must retain the exact exchanged token for a later acknowledged revoke, and a pending or
unavailable denial marker must not be cleared merely because its credential is lost/corrupt.
The fixed controller keeps private account projection closed and forbids restore/new login
through that barrier. A missing/corrupt credential after process death has no manufactured
remote acknowledgement; controlled recovery remains unassigned and is not implemented here.

### Actual execution ledger

All resumed paths below are relative to `/private/tmp/cantos-identity-campaign/`. These are
executed local/common/build results, not live provider or native OS acceptance.

| Named scope | Observed result | Retained artifact |
| --- | --- | --- |
| Same new regressions against the preserved old production controller | **FAIL before repair:** 207 tests, 4 failures, 0 errors, 0 skips | `resume-20261014/native-cleanup-fail-before/command.log`, `execution.json`, XML reports |
| Hash-restored fixed controller and current common/JVM suite | **PASS:** 207 tests, 0 failures/errors/skips; identity subset 28 = NativeAccount11 + NativeLogin3 + CallbackQuery4 + NativeIdentityClient7 + AccountLifecycle3 | `resume-20261014/theatre-cmp-fixed.log`, `native-cleanup-fixed-test-results/manifest.json` |
| Preserved pre-mount Android manifest against the same static contract | **FAIL before repair:** 2 tests, 2 failures, 0 errors | `resume-20261014/android-manifest-fail-before/unit.log`, `execution.json` |
| Mounted Android source manifest contract | **PASS:** 2 tests; static source assertions only | `resume-20261014/android-manifest-fixed-unit.log` |
| Rust format/lint/workspace tests on the resumed tree | **PASS** in the retained executed run | `resume-20261014/theatre-rust-integrated.log` |
| `cargo xtask theatre-demo android-build` | **PASS — Theatre mock Android build:** 61 actionable tasks, 20 executed and 41 up-to-date, 1m52s | `resume-20261014/theatre-android-host-build.log` |
| Built APK manifest inspected with existing `aapt2` | INTERNET, Application/MainActivity, callback VIEW/BROWSABLE scheme and `singleTask` are present in the packaged manifest; no OS dispatch claim | `resume-20261014/android-packaged-manifest.txt` |
| `cargo xtask theatre-demo ios-build --kotlin-native-home <required-2.2.20>` | **UNSUPPORTED:** required installed 2.2.20 path lacks executable `bin/konanc`; existing other-version distributions untouched | `resume-20261014/theatre-ios-availability-explicit.log` |
| Final full `cargo xtask preflight --changed-only` | **PASS: all 17 gates, no skips; repository 331 tests; mutation 9 killed/0 survived** | `resume-20261014/theatre-preflight-frozen-head.log` |
| Final PostgreSQL gate and inherited restart suites | **PASS: 16 Theatre Web, 7 native, 18 Studio OIDC tests and all inherited HTTP/process-kill/PostgreSQL-restart/migration/replay/codec/storage/publication suites** | `resume-20261014/theatre-preflight-frozen-head.log` |

The 16 Web/7 native suites include assurance using two actual signed-mock-issued credentials,
each with its own session GET returning 200 before and after the exercise. All 20 requests to
the exact Studio rendition `bytes`/`read-access` routes (bearer, cookie and cookie relabeling)
return 401 with safe JSON, no redirect/read-grant headers, unchanged authority/media row counts
and zero additional Theatre/native introspection. Test-local SELECT on the empty Studio
`sessions` table distinguishes actual authentication rejection from a missing-grant fixture 503;
no production grant, media fixture or storage/media IO acceptance is claimed. These are added
assurance cases, not a confirmed bug or a fail-before claim.

The four old-controller failures are named
`failed_persistence_retains_exchange_token_until_exact_revoke_is_acknowledged`,
`restart_with_pending_marker_and_lost_token_never_invents_revoke_acknowledgement`,
`pending_marker_with_corrupt_or_invalid_token_stays_closed_without_unknown_revoke`, and
`unavailable_marker_stops_before_store_or_remote_or_browser_effects`. The baseline controller
SHA256 is `eaf7e50375a3dee01bec433622fa4f68bafad967af632718d6a999ef4c9aa02c`; the fixed bytes were
restored by SHA256 `76213f53ae2c07838e1ae035614fc13aa5547923ad181d64db8a9667a9691a0d` before the
passing run. This experiment changes only the selected production controller under the same
new fixture; it is not an original whole-branch acceptance run.

The fixed 26-suite XML manifest is SHA256
`a4301a6f415a6408c0d624c4f501923f6be32120d715df2adcef3ffd232970d4`.
The final manifest contains **27 top-level records**, including the nested **64-record**
PostgreSQL raw-output manifest. Final resumed evidence manifest: `/private/tmp/cantos-identity-campaign/resume-20261014/pr101-resumed-acceptance-manifest.json`, SHA256
`ab3f1270317a1db8b6f7640a6df6db048070c8dc92d03135554392b38bb87f67`. It binds the final source, executed gate outputs,
fail-before/restoration records and packaged-manifest evidence. Owned process/database stop
records are `resume-20261014/theatre-frozen-owned-process-stop.json`; unique fixtures, databases/WAL and evidence remain
preserved. The stop record observes zero task-owned processes and leaves inherited PostgreSQL
PID 741 untouched. Stopped build-cache cleanup retained hashes/proofs; the later free-space
snapshot was 10,381,979,648 bytes.

### Remaining acceptance and recovery boundaries

Device/system-browser callback dispatch, Keystore/Keychain read/write/invalidation, process
recreation/crash/lifecycle, native UI/text scaling/screen reader, live provider and native
playback acceptance remain **NOT_RUN**. Android build and fake-port tests do not establish those
semantics. iOS source inclusion remains distinct from its **UNSUPPORTED** framework build.
The 13 October Web observations keep their original statuses; no new Web runtime was executed
for this source-only native change.

Production activation stays closed. Live client registration/provider/TLS/grants, explicit
cross-client issuer linking, controlled lost-credential recovery, pending-revocation operations,
backend at-rest/backup/retention and durable Library/outbox/guest-import/playback acceptance
retain separate owners and gates. This resumed source handoff closes the held Android source
step, not the whole mobile client or broader issues. GitHub owns PR execution/merge status.
