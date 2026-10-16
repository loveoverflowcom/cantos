# PR2 Studio OIDC evidence

Review boundary: opt-in Studio BFF credential/session integration in the existing development
host. [Architecture/threat model](../architecture/studio-oidc.md),
[ADR0023](../decisions/0024-studio-oidc-bff.md) and
[operator policy](../../services/kanidm/README.md) separate implementation from deployment.
No real provider/client/account/grant/TLS/container was activated or created.

Initial base: fresh Cantos develop `df7d62fecf0023cb557a3051ae75918cedcd2610`. PR92 remains an explicit
unmerged dependency at published `277e28c1ab3a06e3c2fae6093e3278bdece1a0bb`, tree
`58ba323476f2a7ccc2043919acd74eb1c63454ca`; normal merge `efe16cd27581c3f71c8912f48d1622fb774de79b`
starts branch `feat/studio-oidc` in `/private/tmp/cantos-studio-oidc`. No force push, dependency
rewrite, PR90/91/93 integration, remote merge or next identity session is performed here.
Before publication, a second fetch found develop at `389c5cb2760ec31b20f5dc816670e9914455313e`
(merged Web PR94/96). Normal merge `0f1bf6284ab7333d848cffca1a8eff5e7f965c9f` incorporated that
fresh base after own implementation `1ae392d1a9737a2592e17e288baa28fe9f076ee5`, without conflicts.
The integrated workspace test exposed the new Theatre AccountPage exceeding Rust1.87's default
query depth. The only Theatre source fix is the compiler-suggested crate attribute
`#![recursion_limit = "256"]`; UI and authentication behavior are unchanged. The failing
integrated log is retained at `/tmp/cantos-oidc-preflight-integrated.log`; a fresh rerun records
the fix below. These inherited Web changes are now part of the develop base, not PR2's UI work.
Fresh VOT develop `f3fea641a5fd226b883c4e32224ad8c97b8323f0` was read for private proof/verification/
operator boundaries; Gateway/admin/bootstrap is not copied. Migration **0020_studio_oidc.sql**;
0019 stays with Library/Progress, Discovery0017 stays with PR93. Original migration checksums remain.
Before final publication, PR95 Library head `dc45b46f732c19948f01947039ec4b09fa8a7741`, tree
`b53ccafff81d1d98f4df368cbbaf41f9b5338dca`, was fetched/read solely for PR3 documentation.
Its actual bearer-only `ListenerAuthenticator`/sealed principal/default-deny router seam and
official handoff are pinned in the [successor handoff](../archive/handoffs/identity-authentication-handoff.md).
It was not merged into this Studio branch; no Library test/runtime acceptance is claimed here.

## L1–L5 and security evidence

| Layer / claim | Result | Actual evidence / limits |
| --- | --- | --- |
| L1 architecture/threat boundary | PASS | One Studio BFF, private witness, exact client/redirect, development/loopback host, narrow SQL issuance; independent canonical review below. Static gate results recorded below. |
| L2 pure types/errors/rules | PASS | Private non-serde witness, exact typed refusal partitions, bounded parser/P-256 key usage/expiry, proof consumed at DB issuance. |
| L3 core/crypto | PASS | 7 tests: actual ephemeral P-256 signatures, wrong issuer/audience/azp/nonce/sub/alg/kid/key/signature, duplicate keys, missing claims, exact time boundaries; RFC7636 vector and 8000 finite expiry combinations. No Kani claim. |
| L4 HTTP provider orchestration | PASS | Production adapter + actual loopback HTTP signed mock: PKCE/redirect enforcement, code expiry/replay, bounded key rotation/load outage, bad metadata/claims, provider inactive and fresh clock after I/O. `mock-integration-tested`, not Kanidm-live. |
| L5 PostgreSQL/HTTP | PASS | 7 new tests through full disposable PostgreSQL17.4 gate, including independent anonymous logins and explicit shared-lock operator/OIDC registration, final actor-lock wait, competing rotations, callback concurrency/lost acknowledgements, failed exchange no redispatch, account isolation/idempotent registration, disable/expiry/revocation/logout, pending remote outage, exact HTTPS cookie namespace and disabled OIDC configuration. Actual Axum router/DB role/grants, origin/referrer/cache/cookie and reserved route checks. |
| Live Kanidm / browser / production | NOT_RUN | Operator client/grant/secret/TLS/runtime preparation outside authorized actions; production activation stays closed. HTTP cookie assertions are not browser observations. |
| Listener / native / Admin | NOT_RUN | Separate PR3/PR4 and Library seam; no authority supplied by Studio. |

| Security edge | Held oracle |
| --- | --- |
| Wrong issuer/audience/azp/signature/alg/kid/key/nonce, exp/nbf/iat | Signed crypto tests call production verifier with injected time and exact error variants |
| State/browser mismatch/replayed callback | SQL matching hashed flow/binder and atomic consume; race one winner; rejection before provider call |
| Code expiry/replay/redirect/PKCE mismatch | Actual HTTP mock enforces provider-clock expiry/one-time code/verifier/redirect; invalid_grant creates no session and callback retry never redispatches |
| Lost/failed callback acknowledgement | Consume persists before HTTP; discarded successful response replays State refusal and no second session |
| JWKS rotation/outage | Unknown-kid refresh/expiry/failure limit exercised by actual HTTP signed mock; reused-kid Signature refresh is source-reviewed |
| Clock after network | Regression clock100→101 for newly minted token; expiry read again after introspection |
| Identity account separation/registration | Exact PR1 key/lock, SQL PK/FK, repeat actor stable, different subject different actor; no email linking |
| Local disable at lock wait | Actual `studio_oidc_finish` blocked on actor row, concurrent disable commit, callback refused |
| Rotation/logout races | Two rotations from one old token yield one winner; old local session invalid, preexisting logout blocks finish |
| Remote inactive/outage/logout | Per-request introspection refusal; local logout committed before failed remote revoke, retained pending row and controlled retry |
| Session expiry/mode/source | min proof/access/900s; SQL expiry checks; OIDC mode rejects development login, disabled config rejects OIDC sessions |
| Cookie/CSRF/surface isolation | HTTPS Secure/HttpOnly/Strict/host-only/Studio Path, Lax flow; reject namespace fallback/duplicates; exact Origin; no Theatre/Admin/alias OIDC routes |
| SQL privilege scope | Actual HTTP role cannot select registry or insert actor/arbitrary session; narrow function only, no DDL/table rewrite |

## Commands and provenance

Environment: Rust1.87.0, `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 TMPDIR=/private/tmp`, existing
real FFmpeg fixtures under `/private/tmp/cantos-audio-tools` on PATH. PostgreSQL17.4 Homebrew;
every xtask postgres invocation created/stopped a new disposable cluster, never used an inherited
DATABASE_URL. Checkout/main and PR1 evidence were retained. Build cache only is shared through
`CARGO_TARGET_DIR=/private/tmp/cantos-identity-domain/target` and this worktree's `target/debug`
link; all new evidence/cluster roots are private to this worktree. PR1 evidence was not overwritten; raw logs for every PR2 attempt are retained separately.

| Command | Result | Raw local output |
| --- | --- | --- |
| `cargo test --locked -p cantos-api --lib platform::studio_oidc` | 7 PASS (native-local) | `/tmp/cantos-oidc-core-final.log` |
| `cargo xtask verify postgres` | PASS (initial six-test slice); seven-test slice also PASS via preflight before the fresh develop merge | `/tmp/cantos-oidc-postgres-final.log`, `/tmp/cantos-oidc-preflight-final.log`, `target/revision-evidence/studio-oidc-postgres-tests.log` |
| `cargo xtask kanidm reconcile ... --dry-run` | NOT_RUN mutation, plan printed | `/tmp/cantos-oidc-kanidm-plan.log` |
| `cargo xtask verify rust` | format/lint/tests PASS before the fresh develop merge; integrated rerun recorded below | `/tmp/cantos-oidc-rust.log`, `/tmp/cantos-oidc-preflight-final.log` |
| `cargo xtask verify contracts` | script-ir-reference/contract-tests/wav-reference/audio-inspection PASS | `/tmp/cantos-oidc-contracts.log` |
| `cargo xtask verify repository` | FAIL; static checks PASS, 305 Python tests retain37 Darwin errors+1fallout | `/tmp/cantos-oidc-repository-final.log` |
| `cargo xtask preflight --changed-only` | UNSUPPORTED; compatible Gradle missing, no gate reported PASS | `/tmp/cantos-oidc-preflight.log` |
| `cargo xtask preflight --changed-only --skip cmp-ui-preview --skip theatre-cmp-demo` | FAIL at repository; remaining non-skipped gates NOT_RUN | `/tmp/cantos-oidc-preflight-local.log` |
| `cargo xtask preflight --changed-only --skip repository --skip cmp-ui-preview --skip theatre-cmp-demo` | PASS with3 explicitly skipped NOT_RUN gates before the fresh develop merge | `/tmp/cantos-oidc-preflight-final.log` |
| `cargo xtask kanidm verify ... --dry-run` | NOT_RUN network; explicit read-only plan printed | `/tmp/cantos-oidc-kanidm-verify-plan.log` |

Shared Cargo/xtask inputs conservatively select every gate. Only Gradle9.7 was found; the required
installed8.14.3 is absent. No wrapper download/tool installation or gate-policy relaxation was
performed. Full preflight is incomplete. The available-gates run before the fresh develop merge includes fresh retirement/
exhaustive-mapping fixes and real cross-adapter registration locks. Earlier six-test PG/initial
available logs remain; only the final log proves this final seven-test scope.

Available-gates summary before the fresh develop merge, copied from xtask:

```text
NOT_RUN      repository                                skipped by --skip
PASS         rust-format
PASS         script-ir-reference
PASS         rust-lint (workspace)
PASS         rust-tests (workspace)
PASS         dbsp
PASS         wav-reference
PASS         audio-inspection
PASS         web-lint (cantos-studio, cantos-theatre)
PASS         web-build (cantos-studio)
PASS         web-build (cantos-theatre)
PASS         ui-preview
NOT_RUN      cmp-ui-preview                            skipped by --skip
NOT_RUN      theatre-cmp-demo                          skipped by --skip
PASS         mutation
PASS         postgres
preflight: PASS with 3 explicitly skipped (NOT_RUN) step(s)
```

The four Script IR input-file hashes and nine killed mutants are retained in the final log. This is Script IR
mutation evidence selected conservatively, not OIDC mutation or formal proof. Web builds are
compiled evidence only; no OIDC browser/runtime/accessibility claim is made. Kani not added:
crypto verification and SQL races need the actual library/database oracles; finite expiry
partitions have exhaustive examples. This makes no formal or process-crash identity claim.

Initial failures retained: sandbox HTTP bind EPERM (native authorized rerun passes); compile
errors fixed before checks; first PG run exposed old-schema `auth_kind` admission and new FK
TRUNCATE oracle ordering. Old-schema reading now admits unchanged development rows before upgrade;
TRUNCATE tests include referencing table and still assert immutable-trigger55000. The changed session projection also exposed a benchmark SQL extractor drift; the exact extractor
and its projection regression were updated, eliminating those four introduced errors. No SQL
checksum, credential gate, assertion or baseline process fixture was suppressed. Broader existing PG tests
also pass source/history/restart/HTTP journals; they are not OIDC provider-process crash evidence.

## Independent canonical review

Separate read-only reviewer applied `cantos-code-review` to the own working diff and inherited
registry contract, reading core/config/provider/application/SQL20, HTTP parsers/mounts/callers,
PostgreSQL locks, signed mock/PG sources, xtask plan/metadata primitive. Source review caught the
pre-network clock error (valid newly minted token refused), and exact HTTPS namespace fallback.
The follow-up also caught legacy-cookie retirement recovery and exhaustive error-map policy;
those fixes/HTTP assertions and accurate migration/timing docs were reread. Final assessment `no-actionable-findings` within that
source scope. Reviewer ran `git diff --check` PASS, did not duplicate gates or claim provider/browser
acceptance. That source/doc review preceded the selected-gates rerun before the fresh develop merge;
the raw log contains the promised fresh format/lint/unit/PG evidence for the own implementation.

## Fresh develop integration

The compiler-suggested Theatre recursion attribute was independently reviewed as build-only,
without a new UI, identity or permission branch. The normal merge of develop did not alter
`services/cantos-api` or contracts; Account/Library Web remains its separate session mock and
Theatre HTTP remains reserved. The integrated rerun after the attribute passed all available
gates, including both Web builds and all seven OIDC PostgreSQL tests:
`/tmp/cantos-oidc-preflight-integrated-fixed.log` reports the same PASS/three explicit NOT_RUN
summary above. No UI runtime/accessibility observation is claimed for a compiler attribute.
The earlier failure remains in `/tmp/cantos-oidc-preflight-integrated.log`.

Final requirement audit added a distinct expired-code oracle, rather than treating a generic
exchange failure as code-expiry evidence. An issued code is expired at the injected provider
clock while the local flow is valid. Actual HTTP returns `invalid_grant`; production callback
creates no session and makes exactly one exchange, then retry returns State without network.
The read-only reviewer found no actionable issue in this final fixture/test change. No production
code changed. Final publication run `/tmp/cantos-oidc-preflight-publication.log` passed the full
available-gates selection again with the three explicit NOT_RUN steps above, including seven
actual OIDC PostgreSQL tests and the core/crypto/provider suite. This is the evidence for that
last expiry oracle; earlier logs do not prove it. Source compatibility fix is isolated in commit
`11fd71027d6e026bcacd778def3f3df8a5d46f92`.

Publication-tree gate records (these supersede the earlier command rows for final handoff):

| Command | Actual result | Retained log |
| --- | --- | --- |
| `cargo xtask preflight --changed-only --skip repository --skip cmp-ui-preview --skip theatre-cmp-demo` | PASS with3 explicitly skipped NOT_RUN steps; Rust/core/provider/Web/DBSP/contracts/audio/mutation/PostgreSQL PASS | `/tmp/cantos-oidc-preflight-publication.log` |
| `cargo xtask verify repository` | FAIL: services/static checks PASS (490 text files,18 skills); 311 Python tests,37 Darwin killpg EPERM errors+1fallout failure; repeated after the exact PR95 handoff documentation with the same result | `/tmp/cantos-oidc-repository-publication.log`, `/tmp/cantos-oidc-repository-final-handoff.log` |
| `cargo xtask preflight --changed-only` | UNSUPPORTED: compatible Gradle8.14.3 absent; no gate PASS claimed | `/tmp/cantos-oidc-preflight-publication-full.log` |

The final docs record the observed results after the commands; they do not change runtime behavior.
No failure was suppressed or substituted with the selected-gates PASS.

## Independent identity campaign repairs (verification in progress)

Fresh source review at PR98 head `9794ed9ddcd376680a01b37938eabdde564290e1` found three concrete
defects: the browser lacked the OIDC/current-cookie consumer; OAuth Basic sent unencoded
special-character secrets; issuance checked time before final lock waits. The campaign first
added the real signed HTTP regression and the actual PostgreSQL actor-lock regression. Root
execution reproduced both failures on unchanged provider/0020 production code; raw records are
`/private/tmp/cantos-identity-campaign/studio-fail-before-tests.patch`,
`/private/tmp/cantos-identity-campaign/studio-basic-fail-before.log`,
`/private/tmp/cantos-identity-campaign/studio-expiry-fail-before.log` and
`/private/tmp/cantos-identity-campaign/studio-expiry-fail-before-pg.log`.

The repairs use the already pinned `url` form serializer at token/introspection/revocation,
additive migration 0025 (original 0020 unchanged), closed public mode DTO/route, startup/refresh
actual-cookie confirmation, OIDC navigation/local logout and pure epoch fencing. Unit sources
exercise delayed success/failure/configuration, duplicate/empty replies and preservation of the
original author/ambiguous intent. HTTP sources assert exact mode-only responses, no cookie,
no-store and missing Theatre/Admin/alias mounts. No test, build, browser or live-provider success
is claimed by this in-progress entry; the campaign root owns fresh integrated execution and
rendered/interaction/accessibility evidence before final handoff.

The independent combined-boundary review also reproduced a later receipt-lock expiry defect
after the first repair (`studio-late-effect-fail-before.log` and
`studio-late-effect-fail-before-pg.log` in that same campaign folder). The new 0025 final clock
fence runs after all effects and raises private SQLSTATE P0E01 to roll back the function's entire
subtransaction before returning inactive. The original 0020 checksum is preserved.

Further sources cover dirty-owner retention (including acknowledged-import cleanup), shared
cookie-jar A/B request and logout fences, immutable per-attempt UUID/observation epoch, centralized
private projection invalidation, successful echo-mismatch ambiguity, guarded logout without
global cookie expiry, known inert-cookie relogin and foreign-issuer bearer revocation isolation.
Signed-mock HTTP/PostgreSQL tests use actual handlers and disposable roles. All new execution
and browser observations remain **NOT_RUN in this subagent ledger** until the campaign root
runs the exact integrated tree. Formatting and whitespace checks passed locally; no subagent
build, browser/provider acceptance, commit, push or remote write is claimed.

The campaign root also reproduced the consumed guest-callback resurrection with actual signed
mock HTTP and disposable PostgreSQL: `studio-guest-flow-fail-before-compiled.log` reports 303
after acknowledged logout where 401 was required. The exact pre-repair files, including the
untracked additive migration, are preserved with SHA256 manifest at
`/private/tmp/cantos-identity-campaign/studio-before-browser-inventory/manifest.json`.
The additive repair is now **0025** because 0023 is reserved for the separate artwork campaign;
no published SQL checksum was changed. Sources add immutable max-eight per-flow cookie inventory,
source-session association, lineage serialization with no cancellation authority, fresh private
binder proof for lost-ack rotation, no-proof/expired-proof observed-B preservation, inert-A/new-B
separation, overflow refusal and safe guarded legacy/development-cookie recovery. Unsigned
cleanup preserves pending guest flows; explicit logout is disabled until cookie confirmation.
These new protocol regressions remain NOT_RUN here until root executes the final tree.

The root then executed the actual late-effect cancellation regression: a SHARE ROW EXCLUSIVE
table lock allowed the prior SELECT FOR UPDATE checks but delayed the cancellation UPDATE until
the private binder expired. `studio-cancel-late-effect-fail-before-wired.log` recorded FAIL
(204 instead of required 401). The full pre-final-clock source/test SHA256 manifest is preserved
under `studio-cancellation-before-final-clock/` in the campaign directory. The repair rechecks
supplied binder and active-session expiry after every local write and awaits transaction rollback
before refusal; provider I/O remains after that commit boundary. Fresh reruns belong to root's
integrated ledger; this subagent still claims no new execution PASS.

The root's fresh integrated `cargo xtask verify postgres --keep` rerun now reports **PASS**:
all 18 Studio OIDC PostgreSQL/HTTP cases and inherited HTTP/restart/Protobuf/MessagePack fixtures
passed. Raw log: `/private/tmp/cantos-identity-campaign/studio-postgres-integrated-fixed.log`.
The first integrated run (17/18 Studio) remains preserved: its receipt-lock observer read
`pg_stat_activity` through the same transaction holding the lock, so the initial current-query
snapshot stayed cached. Both lock regressions now use a separate autocommit observer while
retaining the exact production query/lock-stage assertions; this follows the documented
[PostgreSQL17 statistics behavior](https://www.postgresql.org/docs/17/monitoring-stats.html).
No production failure was ignored. WASM lint/build, final canonical preflight and actual browser
interaction remain pending root's separate gates; live-provider evidence remains NOT_RUN.

## Residual risk / deferred acceptance

Live Kanidm/TLS/client/grants/credentials and browser behavior remain NOT_RUN. Provider disable
cannot be atomic with the local transaction; active introspection is the remote linearization
observation and already accepted work may finish. Provider SSO/global logout is not implemented;
local logout/access revocation are explicit. Failed remote revoke and rotated old bearers retain
pending records; operator recovery for abandoned records is not automated. Before production,
review bearer at-rest/backup protection, secret delivery, rate/retention budgets, pending recovery
and actual provider/session lifecycle matrix. Known expired/revoked OIDC cookies may begin a
fresh login; unknown/legacy recovery needs explicit scoped logout. A shared cookie change is
detected on the next fenced request or actual-cookie observation, not a live cross-tab push.
Full repository/preflight acceptance must report the known baseline Darwin killpg fixture honestly.
Combined migration tree and separate Listener/native/Admin acceptance belong to their owners.
No GitHub Actions wait or production-ready claim follows from this draft handoff.


## Independent integrated acceptance (13 October 2026)

The final reviewed implementation is commit `850a820e7db224c41e2f4fd534301e83c0e1d5bf`,
tree `51964d75c85a11d083af6fe55774c40d3b90ecdb`, incorporating merged develop
`007ae8f0a2d82ead84c5c0b98eecc3b5dece36b3`. Original migration0020 is unchanged;
additive migration0025 preserves the separate artwork reservation0023. JSON remains the
negotiated default, explicit Protobuf opt-in and the legacy MessagePack decoder remain available.

Fresh canonical `repository`, `rust`, `contracts`, `web` and disposable `postgres` verification
all report **PASS**. Full `cargo xtask preflight --changed-only` reports **PASS for all17 gates,
with no skips**: repository, rust-format, script-ir-reference, protobuf-reference, rust-lint,
rust-tests, dbsp, wav-reference, audio-inspection, web-lint, both web-builds, ui-preview,
cmp-ui-preview, theatre-cmp-demo, mutation and postgres. Mutation killed9/9 mutants.
PostgreSQL includes18 Studio OIDC cases plus inherited HTTP/process-kill/restart/codec cases.
Earlier failure logs and exact fail-before snapshots are retained; none is excluded from a gate.

Actual Chrome155 observations on frontend commit `288b89f72248ccaf41b8072b7698aca447ca1207`
record **11 PASS, 0 FAIL, 1 INCONCLUSIVE and8 NOT_RUN**. All11 screenshots were opened and
checked. Real development HTTP sessions exercise dirty Alice draft retention across Bob,
private projection hiding/inertness while pending or unavailable, Alice's return, acknowledged
logout, keyboard focus, accessibility-tree absence of hidden private controls, Vietnamese
content and1440/390/320px layouts. The only subsequent code change replaces equivalent HTTP URI
query access; frontend source is unchanged and the complete final preflight recompiled it.
OS text scaling was inconclusive because rendered font metrics did not change. Screen-reader
speech,200% browser zoom, native IME, live OIDC/TLS/provider disable, and the unobserved browser
race scenarios remain **NOT_RUN**; signed-mock/backend tests do not upgrade these runtime gates.

Raw evidence lives outside Git at `/private/tmp/cantos-identity-campaign/`.
`pr98-acceptance-manifest.json` pins12 logs/reports by SHA256; its SHA256 is
`0a49f171f19596a151f1297e1d8a4907ad1a0418fcc3d28bfdc7a78dde10f626`.
The final complete command log is `studio-preflight-integrated-full.log`; actual browser evidence
is under `studio-ui-runtime/`. This supersedes the pending root gates in earlier ledger entries,
while preserving their historical scope. Production activation remains disabled pending the
operator/provider and client acceptance gates above. No identity-specific Kani, native device,
live provider or end-to-end production acceptance is claimed.
