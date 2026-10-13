# PR3 Theatre identity evidence

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
