# Admin authorization evidence

Current resumed status is in the [14 October appendix](#resumed-integrated-review-and-acceptance--14-october-2026); earlier sections
retain their historical source, counts, hashes and blockers.

Evidence for the bounded [Admin policy](../architecture/admin-authorization.md) and
[wire contract](../../packages/contracts/admin-v1.md). Final command results are recorded below
only after execution. Live Kanidm/custom current-group semantics, Admin login/UI, provider/device
acceptance and production activation are NOT_RUN. No paid service, live credentials, rights,
TLS bypass, Podman bootstrap or account security setting is used.

## Provenance and resource recovery

Fresh develop00eab6d; published PR10141c3b75/treebe21bbac was normal-merged at09e496e.
[Handoff](../work-plan/admin-authorization-handoff.md) records full dependency identities and
separates own changes from inherited PR92/98/101. VOT origin/developf3fea641 was fetched and
its Console proof/directory identity source inspected, with no source difference in the inspected
proof/identity files between that ref and the local checkout. No Gateway/code is copied.

Initial free disk616,910,848 bytes (about588MiB). Existing main target28GiB includes24GiB of
revision evidence and was preserved. No Cargo/rustc/Gradle task was active; the unrelated Homebrew
PostgreSQL service was left running and untouched. Only stopped identity-campaign Cargo incremental
cache was removed:7,656,557,017 bytes, from
`/private/tmp/cantos-identity-domain/target/debug/incremental`. Unusual files were copied separately
and SHA256 checked before cleanup. `/private/tmp/cantos-admin-evidence/cleanup.json` retains the
process inventory, exact cache scope, preserved-file manifest and disk observations. Free space
immediately afterward8,386,347,008 bytes (7.8GiB). No PG cluster/WAL, code/ref, unique fixture,
raw evidence, audio, model/runtime or Trash was purged.

Execution uses Rust1.87.0, jobs2, `CARGO_INCREMENTAL=0`, `RUST_TEST_THREADS=1`,
`TMPDIR=/private/tmp`, shared Cargo target at`/private/tmp/cantos-identity-domain/target`, and an
ignored worktree `target/debug` build-only symlink. One heavy lane at a time. PostgreSQL17.4
Homebrew and existing actual FFmpeg6.0 fixtures on PATH. Every PG gate creates/stops its own
fresh cluster through xtask; never an inherited/production DATABASE_URL. Raw logs remain in
`/private/tmp/cantos-admin-evidence` and worktree`target/revision-evidence`; not in Git.

## Invariants and oracles

| Claim / failure mode | Owner / oracle | Evidence boundary |
| --- | --- | --- |
| Verified credential before role; reject forged/wrong-purpose claims | Shared signature primitive + Admin credential; actual fresh P-256 signatures, bad issuer/audience/alg/kid/signature/type/time/claims | example-tested; not live provider |
| Missing/unknown/current removal refuses role; closed role-action policy | Pure Admin domain;80 boolean/enum/time partitions and generated unknown-group property | type-enforced, example-tested, property-tested |
| Exact registry actor; no automatic Admin account or email identity | Privileged narrow point lookup + actual PG fixture | integration-tested only where actual output records pass |
| Current local block/disable/time after lock waits | Canonical actor locks, fresh SQL clock; actual concurrent wait/revoke test | integration-tested; remote removal is observation-only |
| Duplicate delivery and ambiguous response recovery | Same actor/operation receipt, target conflict; concurrent/reopen/server restart regressions | integration-tested; no exactly-once HTTP claim |
| Required redacted append-only audit; no silent audit failure | Actual PG triggers/runtime grants; audit/effect/receipt/final-clock fault injection | fault-injected with real PostgreSQL |
| Expiry during audit wait commits no effect/success | Permit retains lifetime; final SQL-clock recheck rolls back and records denial | real PG blocked-table expiry regression |
| Raw transport rejections keep safe JSON | Actual Router with lazy undialled pool: malformed body/type/field/path/method | example-tested at HTTP seam |
| Role cannot bypass owner/resource rights | Admin touches only block/operation/audit tables; distinct route/session test | statically-checked/source review; no generic ACL grant |

Independent reviewer used current `cantos-code-review` against own changes over09e496e, read-only,
with no DB/build or outward action. Confirmed defects fixed: missing failure audit on receipt lookup;
proof expiry while waiting for required audit; unsafe non-JSON extractor/method rejections; missing
final-clock failure audit/outcome classification. Each gets a named production regression. Parent's
full Ultra campaign review is separate and has not run in this session.

## Pre-Protobuf integration gate ledger

All commands below ran before the fresh Protobuf merge, against the Admin feature snapshot
committed as`4a33f1833f9e1943d254cfe8578190f218131bfe`/tree`7be8a8f9bb4556680af3ea663d08d4a8aee2d2bd`.
They use the environment above, worktree root and unchanged Admin inputs. Input
manifest `/private/tmp/cantos-admin-evidence/tested-admin-inputs.json` SHA256
`a379cfd9e6fa1cbf6af212aa0754f062017201f3d3a4b17f5dca1d62eb58e62a` pins Admin code, crypto,
HTTP, migration, test/contract sources and the PG runner. It was hash-rechecked after execution;
no Admin input changed. Final Git SHA/tree and own/inherited commits are pinned in the PR body.

| Command / scope | Actual status | Evidence / log |
| --- | --- | --- |
| `cargo test --locked --offline -p cantos-api --lib platform::admin` | PASS5 final examples/property;80 policy partitions | `core-final.log`; broader final Rust/DBSP runs rerun these final source inputs |
| `cargo xtask verify rust` | PASS rust-format, rust-lint(workspace), rust-tests(workspace) | `rust-final.log`; includes5 Admin core, Admin wire fixture, actual Router transport/default-unavailable tests; PG ignored here and tested separately |
| `cargo xtask verify contracts` | PASS script-ir-reference, contract-tests, wav-reference, audio-inspection | `contracts.log`; Admin contract registered in existing xtask gate; actual existing FFmpeg Opus fixtures, no new audio quality claim |
| `cargo xtask verify postgres` | PASS full gate, including11 initial Admin tests +1 actual PostgreSQL restart recovery, Theatre7 and Studio7 | `postgres-handoff.log`, worktree `target/revision-evidence/admin-postgres-tests.log` and `admin-restart-tests.log`; signed loopback mock, real PG17.4/runtime role/Axum; not Kanidm-live |
| `cargo xtask verify repository` | FAIL repository overall; static services/architecture/links PASS;317 Python tests with37 Darwin killpg EPERM errors and1 dependent assertion fallout | `repository.log`; unchanged baseline fixture defect from prior campaign, not suppressed or relabeled |
| `cargo xtask preflight --changed-only` | UNSUPPORTED cmp-ui-preview/theatre-cmp-demo due missing compatible Gradle; every other selected gate NOT_RUN | `preflight-full.log`; exact unavailable summary retained |

Available command:
`cargo xtask preflight --changed-only --skip repository --skip cmp-ui-preview --skip theatre-cmp-demo`,
log`preflight-available.log`, completed with the following actual summary:

```text
NOT_RUN repository (explicit skip; prior FAIL retained)
PASS rust-format
PASS script-ir-reference
PASS rust-lint (workspace)
PASS rust-tests (workspace)
PASS dbsp
PASS wav-reference
PASS audio-inspection
PASS web-lint (cantos-studio, cantos-theatre)
PASS web-build (cantos-studio)
PASS web-build (cantos-theatre)
PASS ui-preview
NOT_RUN cmp-ui-preview (explicit skip; missing Gradle)
NOT_RUN theatre-cmp-demo (explicit skip; missing Gradle)
PASS mutation
PASS postgres
preflight: PASS with 3 explicitly skipped (NOT_RUN) step(s)
```

This successful available run does not accept the full unskipped preflight. Prior PR101 disk-failed
Rust, contracts, Web, PostgreSQL and DBSP available gates were now rerun successfully on the combined
tree. Web/UI-preview evidence is compilation only; no UI, accessibility/motion or device acceptance
is claimed. Mutation applies to unchanged Script IR golden kernels, not an Admin/Kani proof.


Initial failed attempts are retained in `core.log`, `pg-compile.log`, `postgres-first.log` through
`postgres-fourth.log`, `postgres-final.log` and `rust.log`: own compiler/test-path/lint errors,
required migration count/list updates and missing worktree build-cache symlink were fixed. A narrow
inherited Theatre export-order lint correction precedes its test module; no Library/domain behavior
or UI change. Historical migrations are untouched. The final successful gates replace none of the
original raw failure logs. No disk failure from a previous PR is claimed resolved until its actual
affected available gate reruns.

## Read baseline and review

Real PG/runtime-role fixture:2000 exact synthetic bindings,20 warm sequential admission point
reads total7ms; EXPLAIN function scan observed0.017ms,10 shared-buffer hits, execution0.019ms.
Raw output is in `admin-postgres-tests.log`. This is a tiny local correctness/budget baseline,
not a latency SLO or load/cold-cache/p95 benchmark. There was no existing Admin query to optimize;
PK point lookups need no further index or maintained aggregate. DBSP fit is rejected for Admin
rights; the inherited experimental engine is tested separately by broad preflight.

Independent review artifact is `/private/tmp/cantos-admin-evidence/independent-review.json`,
source fingerprint22 main files`7470212aa68c47bdb02b6ed6e8fbe99bc8655b1a747a6ecfcacc1f97f172fb0f`:
`no-actionable-findings` after four confirmed P2 fixes. Reviewer execution is NOT_RUN; gate
claims above are this implementation session's actual output. A final manifest hashes raw logs
and reviewer/source records outside Git. No final Ultra, merge, production or live-provider
acceptance is inferred.

Residual risk: full repository/preflight is incomplete due the known Darwin/Gradle blockers;
Kanidm's current-group/rotation contract and live credentials/deployment are NOT_RUN; remote group
removal is a bounded observation, not SQL atomicity; audit operational retention/quotas/backups and
workload/clock budgets remain acceptance tasks. PR101 Web mounting/native resource IO remains
incomplete, explicitly handed to parent Ultra. No Admin UI/client/login or unblock/grant capability
is claimed.


Pre-Protobuf observed free disk: 4393058304 bytes (about4.1GiB).
Raw evidence manifest `/private/tmp/cantos-admin-evidence/evidence-sha256.json` SHA256
`8cd10ab59bf3b1c6579e2a844c7297174b80edf91ebdb954f25698a3112e01f8` hashes original/final gate logs,
review, cleanup and pinned Admin inputs. PostgreSQL clusters/evidence and unique fixtures stay intact.

## Post-Protobuf integration

Fresh develop`b28b87856b30c78c2921aa1134ac657b0584ebd7` is normal-merged at
`fc0fe1869895a68c1668dbb3d48bea077279484c`; Protobuf wire/runner/selector code is inherited
unchanged. Admin still uses plain JSON; no Admin/Studio/Theatre auth semantics are altered.
Studio ADR0024 disambiguates the inherited0023 against Protobuf0023. Prior gate PASS rows above
are pinned to their pre-integration snapshot; fresh integrated results are recorded separately.

Integrated commands on the published25f78ba checkpoint:

| Command | Actual result | Log |
| --- | --- | --- |
| `cargo xtask verify rust` | PASS format/lint/workspace tests | `rust-protobuf.log` |
| `cargo xtask verify contracts` | PASS including protobuf-reference + Admin fixtures | `contracts-protobuf.log` |
| `cargo xtask preflight --changed-only --skip repository --skip cmp-ui-preview --skip theatre-cmp-demo` | PASS with3 explicit NOT_RUN skips; all available gates including DBSP, protobuf-reference, Web/WASM, mutation and PostgreSQL | `preflight-protobuf-available.log` |
| `cargo xtask verify postgres` after outage-regression addition | PASS full gate,12 Admin initial tests +1 real server-restart recovery, Theatre7/Studio7 and Protobuf cases | `postgres-outage-final.log`; mock JWKS503/introspection503 produce503 + unavailable audit and zero effects |
| `cargo fmt --all -- --check` and focused clippy for Admin/Studio/Theatre identity PG test consumers | PASS | `outage-focused-lint.log`; clippy uses `--locked -p cantos-api --test admin_postgres --test theatre_identity_postgres --test studio_oidc_postgres -- -D warnings` |
| Final `cargo xtask verify repository` | FAIL overall; services/architecture/links PASS;322 Python tests,37 Darwin killpg EPERM errors and1 dependent assertion | `repository-integrated-final.log`; same baseline defect, no suppression |
| Final unskipped `cargo xtask preflight --changed-only` | UNSUPPORTED cmp-ui-preview/theatre-cmp-demo; all other selected gates NOT_RUN | `preflight-integrated-full-final.log`; compatible Gradle absent |

The final test-only addition has no production code change. Its focused format/clippy checks cover
only changed mock consumers; unchanged gates are not rerun merely for final documentation.
Raw pre-Protobuf Admin PG logs were copied to`pre-protobuf-target-logs/` and hash-verified against
the original manifest before the integrated runner reused target filenames. The preservation
mapping is`pre-protobuf-preservation.json`; original evidence remains independently recoverable.

Additional cleanup under exclusive Cargo build lock, after no Cargo/rustc/Gradle remained:98
stopped pre-Protobuf PR4 executable build-cache outputs,1,826,221,296 bytes, excluding every fresh
integrated Rust/contracts output. Exact path/size/SHA256 before removal and fresh space observations
are in`stale-native-cleanup.json`. No raw log, PG/WAL/fixture/audio/source/ref is removed. Total
regenerable cache recovery:9,482,778,313 bytes. Free disk after this cleanup4,187,418,624 bytes;
latest observed after integrated gates about1.7GiB. Further unique-data cleanup is not authorized.

Integrated Admin inputs SHA256 manifest`integrated-admin-inputs.json`:
`14590d96246c67f57f1f6ca18f1cab738e54240fd730a9b931fb382d93d43141`.
Changes from the prior manifest are only the inherited Protobuf PG runner and the final test/mock
outage addition; production Admin/verifier/HTTP/SQL/DTO inputs remain identical. Kani/live
Kanidm/device checks remain NOT_RUN; final Ultra and merge authority remain with parent.

The independent reviewer reread the final two-file test/mock addition over the published25f78ba
checkpoint and found no actionable defect; production inputs are unchanged. Reviewer execution
remains NOT_RUN. Follow-up record is`independent-review-integrated.json` outside Git. Final raw
logs and preserved earlier evidence are hashed by`integrated-evidence-sha256.json`; the final
published SHA/tree and manifest digest are pinned in draft [PR103](https://github.com/loveoverflowcom/cantos/pull/103).
Final observed available disk before handoff:1,789,059,072 bytes (about1.67GiB), after all gates
stopped. Further unique-data cleanup is not authorized. No remote CI wait, final Ultra, merge or
production acceptance was performed.

## Independent integrated review and local acceptance

The current tested implementation is `97efec8b358bcc7e7b79249fe55687933af6e67f`, tree
`82fba427a0bd4af02f4a48e78a69dd8bbcb1281f`. Normal merges retain the reviewed registry,
Studio, Theatre and Protobuf history. The current migration union is 1–14,16–22,24,25 (23 entries);
artwork0023 remains reserved. Original 0018/0020/0021/0022 checksums are unchanged. Admin remains
plain JSON; the inherited Studio pilot retains JSON default, explicit Protobuf opt-in and legacy
MessagePack compatibility.

This appendix supersedes earlier provisional current-state descriptions for this snapshot.
The original 38-record `/private/tmp/cantos-admin-evidence/integrated-evidence-sha256.json`
remains intact at SHA256 `723980a1a14cbc180ab08f726fcbc1376a14bd8bf0cd5e232f5df0f512d01acf`.
Its earlier PASS/FAIL/UNSUPPORTED results remain historical evidence, not this run's acceptance.
Earlier failure logs, source snapshots, databases/WAL and unique fixtures remain preserved.

Independent source review confirmed and repaired OAuth Basic encoding for special-character
confidential secrets. Actual signed loopback-provider execution failed before the repair and
passed afterward. The concrete Admin consumer now shares the crate-private RFC6749 component
encoder; configuration still accepts the original valid secret domain. Integration retained one
shared ES256 payload kernel, while each caller checks its own purpose and claims: Admin requires
exact `cantos-admin-api`, issuer/audience/client, `at+jwt`, bounded time and current groups.
Native public-client transport cannot select Admin's confidential path. Studio/Theatre session
and resource modules retain their reviewed behavior; no cookie, native bearer or producer grant
is converted into Admin authority.

The actor-disable regression commits actual `active=false` while admission waits on the actor
lock, then checks denial, zero revocation/receipt effects and one required redacted audit.
The append-only test oracle was also strengthened: `TRUNCATE admin_audit` alone reaches an
earlier foreign-key `0A000`, so a generic is-error assertion did not prove the immutable trigger.
The new source first required exact `P0001` without changing that graph; actual fresh compilation
and PostgreSQL execution reproduced the mismatch. The repaired fixture includes audit and
operation tables, requires exact `P0001` for all 9 protected UPDATE/DELETE/TRUNCATE attempts and
compares every complete ordered audit/operation/revocation row after each refusal. Registry
immutability still requires exact `55000`; no production migration changed.

| Regression / command | Actual result | Raw evidence |
| --- | --- | --- |
| Admin special-secret signed-provider adapter, old source | FAIL 0/1; ProviderUnavailable | `admin-basic-fail-before-fresh.log`; earlier stale-artifact compile failure is not an executed test |
| Same special-secret adapter, repaired source | PASS 1/1 focused | `admin-basic-fixed.log`; final full Rust run covers the integrated source |
| Strong append-only oracle with audit-only TRUNCATE | FAIL 0/1; actual `0A000` versus required `P0001` | `admin-append-only-fail-before-compiled.log`; fresh 14.68s compile, 0.18s test; helper filename preparation failure is separate and not counted |
| Repaired full-graph append-only oracle | PASS 1/1 focused | `admin-append-only-fixed-focused.log`; fresh disposable PostgreSQL 17.4, 0.17s test; retained cluster stopped |
| `cargo xtask preflight --changed-only` on final integrated source | PASS all 17 gates | `admin-preflight-final-combined.log`; no skips or unsupported steps in this invocation |
| PostgreSQL portion of that actual final run | PASS: Admin 13, Admin restart 1, Theatre Web 15, native 6, Studio OIDC 18; all inherited actual HTTP/process-kill/restart/migration/replay/codec suites PASS | `admin-final-combined-pg-raw-evidence/manifest.json` |

The final mutation gate killed 9 mutants with 0 survivors on the inherited Script IR kernels;
this supplies no Admin/identity-specific Kani proof.

Raw campaign evidence lives outside Git at `/private/tmp/cantos-identity-campaign/`.
The final manifest is `/private/tmp/cantos-identity-campaign/pr103-acceptance-manifest.json`,
SHA256 `dd49dbc36279bf64c8c50b98e860977ec1e96e2a3ecc6e076408b44dd074cead`, with 13 records and
a nested 22-record raw PostgreSQL manifest. It pins the exact source, actual command logs,
fail-before/fixed snapshots and independent review evidence. Parent execution owns the command
results; the independent reviewer performed source checks and scoped fixes without starting a
second build lane. All task-owned builders, API/provider processes, browser sessions and disposable
PostgreSQL clusters are stopped; inherited PostgreSQL services were not touched. Free disk after
the final owned processes stop: 1,027,809,280 bytes (recorded snapshot; later activity can change
available space). Unique evidence and databases/WAL remain preserved.

Acceptance remains bounded. `GET /api/v1/admin/capabilities` audits current admission;
`POST /api/v1/admin/actors/{actor}/revoke-access` appends an **Admin-only** local access block,
receipt and required audit. It does not disable/delete an actor, revoke Studio/Theatre sessions,
grant a role, change a resource ACL or implement global/provider SSO logout. Required audit
failure returns unavailable and rolls speculative effects back; replay reauthenticates current
credentials and policy.

Current provider groups are an introspection observation whose witness expires at the earliest
token/access expiry or **at most five seconds after observation**. Local active/block/time facts
are reread after lock waits and before commit, but remote group removal cannot be SQL-atomic.
Live Kanidm `cantos_admin_groups` current-membership/removal semantics remain **NOT_RUN and
unaccepted**. Missing configuration leaves Admin unavailable, and unsupported or missing group
evidence cannot authorize a request. Production activation is closed pending the exact
operator/provider contract, credential delivery, key rotation, at-rest/backup protection and
audit retention/quota/tamper/recovery gates. There is no
role-grant or unblock endpoint and no automatic grant/client/account/security bootstrap.

PR103 remains draft while dependency PR101's Android INTERNET/callback/host activation lacks
the direct human exception after automatic approval review rejected the source patch. Actual
native system-browser/secure-store/crash/device/playback and live-provider gates remain separate.
The inherited Studio/Theatre browser evidence is not Admin login/UI evidence; Admin has no UI.
No identity-specific Kani proof, complete mobile acceptance, remote merge or whole-campaign
completion is inferred from these local/mock results. GitHub owns PR execution status.

## Resumed integrated review and acceptance — 14 October 2026

This appendix supersedes the prior Android dependency blocker/current migration inventory for
the resumed snapshot only. Historical 13 October source, gate counts, fail-before results and
manifests above remain unchanged evidence of their exact inputs. PR101 is merged at develop
`98f247ebf144fe413c1ddcaf1d4f655f08cc1b4d`; its specifically authorized Android source is applied
and compiled. It supplies no new device/system-browser/secure-store/provider acceptance or iOS
framework PASS. Canonical Kotlin/Native 2.2.20 remains UNSUPPORTED and production stays closed.

Admin normally merged that develop snapshot at `eccf31684524572eef3048e75a9abd08d52c6581`, preserving storage PR90,
IDE PR107 and the reviewed Theatre/Studio dependency history. Final tested Admin source:
`eccf31684524572eef3048e75a9abd08d52c6581`, tree `d3f3b31ba7ab28969b47a1008d2200bf8b7bed92`. The combined migration registry is
**24 entries: 1–22,24,25**, including original Storage0015 and Admin0022; artwork0023 is absent.
Historical SQL/checksums are unchanged. Studio keeps JSON-default/opt-in Protobuf/legacy
MessagePack and immutable session fences; Admin remains JSON with a dedicated confidential
resource credential and no Studio/Theatre/producer authority or resource ACL bypass.

Reviewed repairs/assurance in this resumed source: preserved the AdminService Arc in composition, corrected the 24-entry registry integration and aligned inherited test oracles; the valid issued-Admin credential test adds isolation assurance rather than a confirmed production defect or fail-before claim.
The signed-mock-issued valid Admin resource credential returns capability 200 before and after
10 exact Studio rendition `bytes`/`read-access` bearer/cookie/relabel requests. Each refused
request returns 401 with safe JSON and no redirect, Set-Cookie or read-grant headers. The refusal
window changes no authority/media/audit rows and triggers no extra Admin introspection; the
legitimate capability requests retain their own required allowed audits. Test-local SELECT on
empty Studio sessions distinguishes authentication refusal from a missing-grant fixture 503.
No production grant, seeded media/storage IO or fail-before defect is claimed by this assurance.

Named fail-before evidence, if any, remains scoped to the actual executed old kernel/fixture;
new assurance cases are not described as confirmed bugs without reproduced evidence.

| Executed resumed scope | Actual result | Raw artifact |
| --- | --- | --- |
| Final `cargo xtask preflight --changed-only` | **PASS: all 17 gates, no skips; 332 repository Python tests** | `resume-20261014/admin-preflight-frozen-head.log` |
| Actual Admin PostgreSQL/runtime-role and restart suites | **PASS: 14 Admin tests plus 1 actual database-restart recovery** | `resume-20261014/admin-frozen-head-pg-raw/manifest.json` |
| Inherited Theatre/Studio and full HTTP/process-kill/PG-restart/migration/replay/codec/storage/publication suites | **PASS: 16 Theatre Web, 7 native, 18 Studio OIDC tests and all inherited HTTP/process-kill/PostgreSQL-restart/storage/migration/replay/codec/publication suites** | `resume-20261014/admin-frozen-head-pg-raw/manifest.json` |
| Mutation/other relevant exact gates | **PASS: 9 Script IR mutants killed/0 survived and 207 common/JVM tests with 0 failures/errors/skips** | `resume-20261014/admin-preflight-frozen-head.log` |

Final resumed evidence manifest: `/private/tmp/cantos-identity-campaign/resume-20261014/pr103-resumed-acceptance-manifest.json`, SHA256
`1a0336cf7b598ee416fc91dc5f70936bdd6c8a56447fa0cfafb0a3d179e507dc`, 10 top-level records, 12 source pins and 67 raw PostgreSQL proofs. It binds the final source,
actual logs, reviewed regressions and preserved failure evidence. Owned-process stop records:
`resume-20261014/admin-frozen-owned-process-stop.json`. Inherited services, unique fixtures/databases/WAL and
historical evidence remain preserved. The earlier 13 Admin PG+1 restart count is historical;
no earlier PASS is promoted to this integration's result.

Admin capability/revoke routes still affect only current dedicated Admin authority, a local
Admin block/operation receipt and required redacted append-only audit. Replay reauthenticates;
missing evidence/audit/provider availability fails closed. Current membership is an observation
with a witness valid at most five seconds and final local SQL-time fencing, not atomic remote-group/
SQL authorization. Live Kanidm custom current-group semantics, confidential credential delivery,
issuer rotation/TLS, audit retention/quotas/tamper/recovery and bearer/backup protection remain
**NOT_RUN/unaccepted**. Missing configuration keeps Admin unavailable. No role grant/unblock,
global SSO logout, OAuth registration/account mutation or production activation is supplied.
Native OS/store/browser/playback/accessibility acceptance remains separate and unrun; inherited
Web observations are not Admin UI/login evidence. No broader issue or whole-campaign completion
is claimed here. GitHub owns PR execution/merge status.

Later develop PR108 CMP presentation extraction at `648c5b76ca229f2ba84cd724e3202580e22b39c9`
was normally integrated at `980fca66d94a58fbadbf0154e255d3616e891bc0`, tree
`fdb22b5a246ab718b9c7b01e2a04aa7177ecf544`. Actual proportional preflight based on `eccf316`
passed all three selected gates: repository, rust-format and theatre-cmp-demo, with no skips.
Common tests passed207/0failures/0errors/0skips; the canonical mock Android build also passed
(61 executed tasks,21seconds). Independent reviewers found all25 existing presentation functions
unchanged except necessary visibility and edge whitespace; identity/controller/Android-host
source bytes and the prior Admin source pins remain unchanged. This is source/build/mock evidence,
not a new visual, device or live-provider acceptance.

The supplement `/private/tmp/cantos-identity-campaign/resume-20261014/pr103-final-integration-supplement.json`,
SHA256 `527507c28217e36eb05a6cd2e8c839a38827faab2b4c9a71f80da1b678319551`, binds the two new logs
and26 preserved JUnit XML files outside build output. The full17 result remains exactly scoped
to `eccf316`; the later source has the named proportional checks above.
