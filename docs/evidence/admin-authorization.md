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

## Actual gate ledger

All commands below use the environment above, worktree root and unchanged Admin inputs. Input
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


Final observed free disk: 4393058304 bytes (about4.1GiB).
Raw evidence manifest `/private/tmp/cantos-admin-evidence/evidence-sha256.json` SHA256
`8cd10ab59bf3b1c6579e2a844c7297174b80edf91ebdb954f25698a3112e01f8` hashes original/final gate logs,
review, cleanup and pinned Admin inputs. PostgreSQL clusters/evidence and unique fixtures stay intact.
