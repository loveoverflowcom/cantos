# Admin authorization evidence

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
