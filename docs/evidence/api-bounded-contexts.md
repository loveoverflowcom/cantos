# API bounded-context refactor handoff

Date: 2026-10-12. Scope: finish and review the existing in-progress refactor in one pull request.
The [architecture record](../architecture/api-contexts.md) describes implemented ownership;
this file separates implementation from executed evidence. GitHub owns PR execution status.

## Invariant ledger

| Claim / invariant | Failure mode | Oracle | Evidence level |
| --- | --- | --- | --- |
| Domain and Script IR depend only on pure rules/data | An adapter, clock or runtime changes business decisions | Repository dependency guard and adversarial fixtures | statically-checked, example-tested |
| Contexts use approved upstream owners and private SQL adapters | A context bypasses authority through an adapter or table | Guard fixtures for grouped/alias/relative paths, SQL ownership and visibility | statically-checked, example-tested |
| Existing library paths remain usable | CLI/library callers break after internal moves | `module_compatibility` integration target | example-tested |
| Current owner/approval identity is evaluated together | A transferred script reuses prior owner approval | `production::frozen_approval_cannot_follow_a_transfer_to_a_new_owner` | example-tested |
| Transactions retain authorization, replay, expiry and frozen identity | Read-check-write races or stale authority | Existing real PostgreSQL persistence/production/adaptation tests | integration-tested |
| Durable recovery retains fences, ambiguity and selected output | Duplicate dispatch or output selected by stale worker | Existing durable-worker fault/recovery suites | fault-injected |
| Historical Script IR, wire and speech identities stay compatible | Refactor changes stored bytes/digests or clients | Contract fixtures, reference oracles and mutation gate | differentially-tested, mutation-tested |

## Implementation review

The service stays one crate. The existing work is completed with private application/SQL
boundaries, production command decisions, synthesis enqueue/status/control/preparation/dispatch/
settlement/acceptance/reconciliation use cases,
worker runtime/dispatch/recovery/output modules, pure audio admission in Synthesis and temporary
crate-root compatibility entrypoints. Current backend imports use context owner paths.

The SQL inventory retains queries, row locks, transaction scope, migrations, indexes and DTOs.
Moving a stored frozen-document read behind Production's entrypoint preserves its query and
does not introduce a second transaction. Consequently there is no new performance claim,
benchmark comparison or DBSP consumer; PostgreSQL remains authoritative and the optional
summary view preserves ADR 0017's limits. Pure command tests supplement existing wire,
immutability, retry/recovery and real-database regressions rather than substituting for them.

The architecture guard deliberately permits the two documented read-only SQL joins and the
feature-gated `AttachedView` binding. It admits transport byte validation/diagnostic formatting
and immutable catalog output in HTTP; all approval/rights/retry policy remains inward.

## Executed checks

Focused architecture checker evidence: 50 tests in `test_check_repository.py` pass, including
27 architecture cases covering pure in-memory imports, hidden filesystem/runtime dependencies,
module aliases, clock/console effects, grouped/relative adapter imports, façade/legacy bypass,
upstream direction, private modules, Script IR isolation, HTTP policy/SQL calls, SQL ownership,
the limited legacy read join, the platform view binding, flat `domain.rs`/`script_ir.rs` forms
and nested comment/raw-string handling.
The current repository source scan reports no dependency violations.

Checks ran on 2026-10-12 from the repository root on `refactor/api-bounded-contexts`, continuing
the user's four commits through `91dac4adc972cbb356aa8e70d44ea858735cedda` plus this request's
dirty tree. The comparison base is `develop` at `8eb7f495fd07936295bda328b7392e83ed97ffa5`.
Rust sources were stable for the final preflight; the final repository rerun covers the later
guard fixtures and this evidence record. Session logs remain under ignored
`target/api-bounded-contexts/`, with PostgreSQL subtest logs under `target/revision-evidence/`.
They are local session artifacts, not committed source data.

The environment used Rust 1.87.0, Homebrew Python 3.14.8, PostgreSQL 17.11, Trunk 0.21.14,
Gradle 8.14.3 and Java 17.0.19. The final preflight and repository commands used this prefix:

```sh
PATH="/Users/manh.pd1/.cargo/bin:/opt/homebrew/bin:/Users/manh.pd1/.gradle/wrapper/dists/gradle-8.14.3-bin/cv11ve7ro1n3o1j4so8xd9n66/gradle-8.14.3/bin:$PATH" RUSTUP_TOOLCHAIN=1.87.0 CARGO_BUILD_JOBS=4 TMPDIR=/private/tmp
```

The separate Rust and contract commands used `RUSTUP_TOOLCHAIN=1.87.0 CARGO_BUILD_JOBS=4
TMPDIR=/private/tmp`. Local loopback/process permissions were enabled for mock HTTP and
disposable PostgreSQL checks. Initial sandbox socket failures, system Python 3.9's missing
`tomllib`, a stale documentation link and a DBSP module visibility error were corrected before
the passing runs below.

| Command | Result | Local log |
| --- | --- | --- |
| `cargo xtask verify rust` | PASS: format, workspace lint and workspace tests | `rust.log` |
| `cargo xtask verify contracts` | PASS: Script IR reference, contract tests and WAV reference | `contracts.log` |
| `cargo xtask preflight --changed-only` | PASS: all 14 gates below | `preflight.log` |
| `cargo xtask verify repository` (final rerun) | PASS: scan, 243 Python tests and whitespace checks | `repository.log` |

The changed-only planner conservatively selected every gate because some changed paths had
uncertain impact. Its final summary was:

| Gate | Result | Observation |
| --- | --- | --- |
| repository | PASS | Repository scan and 241 Python tests |
| rust-format | PASS | Workspace formatting |
| script-ir-reference | PASS | Historical reference oracle |
| rust-lint (workspace) | PASS | Clippy |
| rust-tests (workspace) | PASS | Includes four module compatibility tests and the owner-transfer regression |
| dbsp | PASS | Feature-enabled lint/tests |
| wav-reference | PASS | Five PCM fixtures agree with Python `wave`/`hashlib` |
| web-lint (cantos-studio, cantos-theatre) | PASS | Both WASM surfaces |
| web-build (cantos-studio) | PASS | Trunk build |
| web-build (cantos-theatre) | PASS | Trunk build |
| ui-preview | PASS | Shared Leptos preview build |
| cmp-ui-preview | PASS | Gradle build successful; `desktopTest` was `UP-TO-DATE` |
| mutation | PASS | Baseline 15 tests; 9 killed, 0 survived, 0 timeout, 0 unviable |
| postgres | PASS | New disposable local cluster, actual process kills and restart/replay checks |

PostgreSQL executed 17 revision, 9 import, 21 adaptation, 11 production, 50 durable-job and
12 DBSP summary-view tests successfully. The durable gate's two installed-local-runtime cases
were filtered by its existing command; this handoff makes no installed-provider acceptance
claim. The actual HTTP host/CLI journey additionally passed restart, immutable source and
revision replay, production freeze/approval/eligibility, independent row-count/digest oracles
and JSON/MessagePack equivalence after host death and PostgreSQL restart.

Named fault evidence includes `actual_kill_before_or_after_provider_execution_never_blindly_redispatches`,
`actual_kill_after_observation_recovers_exact_output_without_a_second_provider_call`,
`cancellation_after_intent_before_send_blocks_the_actual_provider_process`,
`expired_claim_without_intent_recovers_but_stale_worker_cannot_write` and
`authorization_expiry_and_revocation_after_lock_wait_roll_back_context_submit_and_accept`.
Provider HTTP tests use deterministic loopback responses; neural process tests use executable
test doubles; worker fault tests use the existing synthetic provider and durable ledger. These
exercise real application/adapter code and are `mock-integration-tested`, alongside real
PostgreSQL `integration-tested` and process-kill `fault-injected` evidence. Live-provider tests
remain ignored by their existing opt-in contracts. GitHub CI is not checked.

## Completion report

- Invariant: existing immutable identities, wire behavior and durable authorization/recovery
  survive clearer context ownership; pure rules cannot import adapters through canonical paths.
- Owner / boundary: five business contexts, the shared Script IR kernel, HTTP interfaces and
  technical platform; one concrete PostgreSQL transaction authority.
- Evidence: all 14 final preflight gates and the separate contract verification pass; the final
  repository rerun is recorded in the command table above.
- Evidence level: statically-checked, example-tested, differentially-tested, mutation-tested,
  mock-integration-tested, integration-tested and fault-injected as scoped above.
- Command: the Rust, contract, preflight and repository xtask commands and runtime environment
  are recorded above with attributable logs.
- Provenance: the recorded base/user revision plus this request's dirty-tree continuation,
  original synthetic fixtures, preserved
  historical contract/reference fixtures; external providers mocked where those suites specify.
- Edge classes covered: dependency bypasses, owner transfer, idempotent retries, stale revisions,
  expired sessions/leases, duplicate selection, crash boundaries and ambiguous provider outcomes
  through the named existing suites and new focused regressions.
- Not covered / residual risk: source checks do not expand macros or dynamic SQL and cannot
  prove policy-free arbitrary handwritten code. No new live provider, billing, listening,
  object-storage, publication, browser/mobile or production-readiness claim. Temporary public
  aliases remain supported; no persisted-model or migration change is authorized here.
