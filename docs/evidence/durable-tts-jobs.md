# Durable local/fake TTS job foundation

> **Historical (ADR 0018).** This evidence describes the in-process text-to-speech runtime that
> [ADR 0018](../decisions/0018-external-audio-production.md) removed on 2026-10-12. The code, tests
> and contracts it names no longer exist; links to them are pinned to commit `2de8c69`, the last
> commit that contained them. Nothing here describes current behavior.

Status: bounded implementation for [issue #7](https://github.com/loveoverflowcom/cantos/issues/7),
2026-10-11. Branch `feat/durable-tts-jobs`, originally based on merged
[PR #38](https://github.com/loveoverflowcom/cantos/pull/38)
`209e9e905c680e7cecab831f2d8f7894656e14fe`. The sequential review integrated merged
[PR #39](https://github.com/loveoverflowcom/cantos/pull/39)
`0ff16fd7e50b556487bd16c8f36d4a9b6ae754da` through a normal merge, then fixed the owner-session
expiry defect below. Fresh PostgreSQL and actual local eSpeak reruns passed on that fixed source.
The original disk-blocked handoff is preserved as historical evidence below; disk space is now
available. Final no-skip preflight, review, commit/tree identity and merge disposition belong to
[PR #40](https://github.com/loveoverflowcom/cantos/pull/40) and the ignored local review record.

This slice supplies same-package development-only local/fake jobs and immutable result
selection, not complete #7/020 acceptance. Issue #6's live/listening/rights gaps remain open.
Google live, billing and commercial dispatch are **NOT_RUN** and disabled. VieNeu remains
research-only **NO_GO**, with no worker dispatch. Full cache/reuse, partial regeneration,
mixing, object storage/CDN, publication, UI, Theatre and native mobile are outside scope.

## Invariant, ownership and implemented boundary

One current-authorized frozen dialogue scope has one write-once selected result; crashes,
duplicates and stale workers cannot silently repeat possible execution, attach another
revision/dialogue's output or overrun the frozen budget. An observation of possible execution
survives lease loss and is reconciled before another dispatch. This is idempotent selection,
not an exactly-once third-party synthesis or billing claim.

## Sequential review: session expiry during lock waits

Independent defect-first review found a confirmed P1: enqueue, control and owner resolution
authenticated before acquiring script/job locks, but a session could expire during that wait.
New commands could commit and exact retries could return success with an expired token.
Holding the session row against revocation does not stop the database clock advancing.

All five successful commit paths now call the existing `durable_actor` immediately before
commit, including enqueue/control early replay branches and the shared resolution replay path.
The check returns `Unauthenticated` and rolls back the whole transaction when expiry has passed.
Worker observations remain unfenced facts; migration, operation identities, wire contracts,
immutable receipts and valid-session behavior are unchanged.

The integrated repository gate also exposed a transient macOS reap race in inherited PR #39
preview tooling. Its separately fail-first regression and bounded cleanup correction are
recorded in [preview evidence](compose-preview-tooling.md#review-repairs-and-macos-checkpoint).
That correction affects the private tooling process boundary, not job dispatch or mobile UI.

Six real PostgreSQL regressions cover new operations and exact replays for each command.
An independent restricted app-role transaction holds the precise script/job row; the database
oracle witnesses the authenticated caller waiting on that owned blocker while the session is
valid, then witnesses expiry before releasing it. Each test compares all columns across eleven
durable tables and ten input/editorial tables and checks the relational invariants. Polling,
queries and the caller are bounded; the owned blocker is rolled back before assertions.

`target/pr40-review/postgres-expiry-fail-first.log` records all six failing on unchanged
production source: each returned `Ok`; the three new operations changed state. With only the
final authentication checks added, `postgres-expiry-fixed.log` records **PASS**: 27 durable tests
(21 existing plus six regressions), and 84 real PostgreSQL tests overall, followed by the
existing CLI/HTTP, host-kill, database-restart, immutable replay and independent JSON/MessagePack
oracles. No timeout, assertion or durability setting was weakened. The fresh owned cluster
`cluster-1791743854249687000` was retained and stopped; no old database was removed.

`target/pr40-review/local-espeak-final.log` records **PASS**, one actual installed-runtime test
in 1.10 seconds, on the same fixed/integrated source and fresh retained cluster. Four dialogue
WAVs completed with native 22,050 Hz mono 16-bit PCM and the same measured hashes/frame counts
as the earlier table below. Fresh receipts and audio facts are in ignored
`target/durable-job-evidence/f15d2708-4027-4d8a-9fca-10e9bf2a8033/installed-local-audio-facts.json`:
run `f8239cd0-6796-458e-abf5-5da7973dc028`, snapshot
`57c95b2b-f1ca-48f9-a884-da54472c5266`, revision 1, input digest
`production-p1:sha256:51afcbf3056a71368050db9de107d574d9610dfe1642bcf79907f82fcd1ac2f6`.
This is local `provider-live-tested` and structural `audio-measured` evidence. Actual charge
remains unknown; listening, commercial dispatch and production asset acceptance are **NOT_RUN**.

The existing `services/cantos-api` package owns the pure `jobs.rs` state/retry/fingerprint
decisions and the `postgres/durable.rs` transaction shell. `production-jobs` is an authenticated
development CLI; `cantos-worker` is a separate process of that same package. The worker reuses
production-p1 reconstruction/current approval, receipt-v1 linkage and recovery rules, WAV
inspection and the exact eSpeak adapter. There is no new crate, broker, microservice, provider
framework, HTTP route or Studio UI. PostgreSQL remains the authority.

Migration `0007_durable_jobs.sql` adds immutable run scope, jobs, dialogue steps, attempts,
unique dispatch markers, observations, budget/reservation/settlement history, controls and owner
resolution records. Triggers/constraints preserve scope, append-only facts, valid transitions,
monotonic fences and write-once selected attempts. The
[development contract](https://github.com/loveoverflowcom/cantos/blob/2de8c69/packages/contracts/durable-jobs-v1.md) records concrete commands,
states, bounds and the local private filesystem boundary.

The functional core explicitly models `queued`, `running`, `completed`, `failed`, `paused`,
`cancelled`; exhaustive event matching rejects other transitions. Retry and lease configuration
require validated constructors. Failure decisions keep every `may_have_executed` class in
reconciliation, including malformed output, cancellation and exhausted retry history. A bounded
exponential ceiling/equal-jitter delay is seeded by the immutable attempt operation UUID and
enforced using observation/resolution time. Retry does not reset attempt counts.

`durable-input-v1` covers the complete resolved dialogue fields and scope. It protects revision,
dialogue and receipt association; it deliberately includes identity/rights and is **not** the
future position-free speech reuse key. Exact enqueue/control retries reuse existing records;
changing a payload under the same operation conflicts.

## Effects and recovery ledger

Current actor/member, accepted revision/export, settings/required-rights, approval and time are
rechecked in the committing transaction before intent, final send authorization and acceptance.
Each dispatch commits reservation plus conservative attempt intent before any provider call,
then commits a unique send marker under the current fence. No transaction spans synthesis or
filesystem I/O. Private create-only files supplement PostgreSQL; they do not authorize retries.

| Boundary | Restart / concurrent behavior | Implemented oracle; execution status |
| --- | --- | --- |
| After claim, before intent | Lease expiry permits a new fence; no provider execution was recorded | Actual child kill/restart and provider-side fake ledger; final targeted PASS |
| After intent, before send | Missing definitive outcome pauses and holds reservation; no blind redispatch | Actual child kill; final targeted PASS |
| After provider execution, before observation | Possible execution remains ambiguous even with local journal files | Actual child kill and independent execution count; final targeted PASS |
| After observation, before accept | New fence reopens exact output, audits receipt/hash/profile and selects once | Actual child kill/restart; final targeted PASS |
| Pause/cancel before committed send | Control prevents authorization; definitive known non-send can be observed | Actual child control-before-send case; final targeted PASS |
| Pause/cancel after send | Late factual observation/accounting survives; stale selection is rejected | PostgreSQL interleavings; final targeted PASS |
| Duplicate intent/send/observation/accept | Same operation reconciles; unique send marker and write-once selection refuse a second result | PostgreSQL exact retry plus fake execution ledger; final targeted PASS |
| Concurrent reserve/retry | Integer conditional budget update serializes the last available unit | Independent ledger/counter checker and concurrent PostgreSQL connections; final targeted PASS |
| Non-execution attestation then late output | Preserve contradictory fact and quarantine; do not accept/resume it | PostgreSQL contradiction case; final targeted PASS |
| Invalid queued authorization | Pause/fence the invalid job and allow a later current-authorized job to progress | PostgreSQL stale queued job regression; final targeted PASS |
| Missing/invalid recorded output | Quarantine its job, retain original observation and avoid redispatch | Same recurring worker quarantines the faulty job, then accepts a later job; final targeted PASS |

Leases expire and heartbeats require the current owner/token/deadline. Reclaim increments the
token, so stale heartbeat, send and acceptance fail. An accepted attempt is immutable. Late
observations are exact/idempotent unfenced facts; they do not overwrite control pointers.
Unresolved intent and ambiguous failures pause as `uncertain_attempt`, retain reservations and
require owner reconciliation. `resolve-not-executed` records an explicit evidence-backed owner
attestation, not a provider lookup. A separate resume is required. Contradictions remain
quarantined; completed/cancelled runs are terminal.

Preparation now durably pauses invalid queued authorization as `authorization_invalid` rather
than repeatedly failing the first claim and starving later jobs. Recovery quarantines missing
or invalid recorded output as `recorded_output_unavailable` / `recorded_output_invalid`,
preserving observations and requiring explicit repair. The store boundary and new regression
are implemented and exercised on the final targeted source. The recurring worker continues
after quarantine; the process harness waits for both its quarantine and later acceptance events
before killing only that owned child. Original fixture output bytes remain preserved under
`preserved-original-output.wav` while its recorded locator is removed/replaced for the fault.

## Provider, money and audio facts

Only exact approved local eSpeak inputs are admitted, under explicit `local_espeak` or
`synthetic_fake` mode. Synthetic execution is not a catalog provider or voice proof. Its
create-only `.fake-provider/<operation-id>.json` ledger is independent of job acceptance and
counts actual scripted executions. The fake emits original bounded native PCM and labels its
receipts `synthetic-fixture`. A fake result is never reported as a live provider envelope.

Local eSpeak uses the pre-existing exact `espeak-ng-local` / `espeak-ng-1.52.0` /
`espeak-ng-local-v1` adapter with native `wav-pcm-s16le-mono-22050-v1`. Charge remains unknown;
local reservation is zero external provider-spend units and settlement is provisional. CPU and
resource costs remain unknown. Synthetic reported fees exercise reservation/settlement currency
and concurrency, including known zero; they are not account bills. Uncertain outcomes retain
their allocation. Commercial overrun/account settlement needs separate policy and evidence
before commercial dispatch can be enabled.

The worker uses private create-only attempt directories, sync calls and bounded output;
recovery requires the exact configured root/attempt/output name and rechecks WAV/hash/linkage.
Symlinks, hardlinks, nonregular files and wrong-attempt paths fail closed. This is local
filesystem evidence, not an object-storage adapter or an accepted production asset. Process
kill tests do not establish host power-loss/filesystem durability or shared-storage behavior.
Listening, loudness/true-peak acceptance and editorial/publication approval are **NOT_RUN**.

Original implementation's actual local eSpeak job: **PASS**, one selected integration test in
`target/durable-jobs/local-espeak-realpg-final.log`, 1.13 seconds. It completed all four original
`two-scenes.json` fixture dialogues through the real installed eSpeak NG 1.52.0 runtime at
`/opt/homebrew/bin/espeak-ng`, with synthetic private rights assertions and exact current
snapshot approval. This is `provider-live-tested` for local eSpeak only and structural
`audio-measured`, with charge still `unknown`. Native output is 22,050 Hz mono signed 16-bit PCM,
inspector `cantos-wav-inspection-1`, profile `cantos-espeak-tts-pcm-1`:

| Dialogue / voice | Frames / file bytes | Actual WAV SHA-256 |
| --- | --- | --- |
| `dialogue-01` / `vi` | 52,814 / 105,672 | `c7c4db634144aae58bd454f4b7097d111f8e3da14414db0fcb01451ed9f5a14f` |
| `dialogue-02` / `vi` | 42,936 / 85,916 | `592f54268c1ddd72e70eaa029cb92c2226901d46d6321856ac9f0b97d2eefff8` |
| `dialogue-03` / `vi` | 53,631 / 107,306 | `6b249e0cc99abcab706110da89091efc9ab77a61a24d8f325901bc985d756b7c` |
| `dialogue-04` / `vi` | 49,419 / 98,882 | `620a332b8e02e2a288b5e615209091d7bf3b60bf949bdf9ef1190d958459db3b` |

The private facts/receipts/audio remain in ignored
`target/durable-job-evidence/0c334672-2ba6-43fd-b943-43df44051679/`;
`installed-local-audio-facts.json` records run `a71d37c2-9b23-4439-af1e-b1abd196e108`, script
revision 1, snapshot `5b25efca-8d75-4478-bbe4-12d0220b115b` and input digest
`production-p1:sha256:708dcb3272f85ed84d0fe1a90909ae6376b0081c811e34d551c80bcb044f0f8c`.
This confirms the development job/receipt path, not source/voice legal clearance, listening,
resource cost or production asset acceptance.

## Original implementation verification and assurance ledger

This section preserves the pre-review checkpoint at head `9150a8c34c428fc9b1199968f5ee90fd72c2681c`.
Its source hashes, test counts and disk-blocked handoff are historical, not the integrated
review source. The sequential review section and PR #40 handoff record supersede its status.

Actual initial core output in `target/durable-jobs/domain-tests.log` recorded seven pure tests
PASS before subsequent jitter and shell hardening. That result is historical and does not
establish final-source verification. `core-shell-tests-loopback.log` similarly records an
earlier 55-test local/mock run. Actual final checks and the resource-blocked full preflight are
recorded below.
Initial denied socket/resource or failed runs must remain identifiable; only actual final
reruns can be entered as PASS. No GitHub Actions checks are required, inspected or polled.

The pre-final targeted real PostgreSQL rerun in `target/durable-jobs/durable-realpg-third.log`
records **PASS: 20 selected tests, zero failed, 8.29 seconds**, including four process tests
(one covers both after-intent and after-provider kills), migration upgrade/replay, concurrent
budget and exact linkage checks, plus the stale queued-job regression. This establishes
`integration-tested` and `fault-injected` evidence for that source snapshot; it predates the
recorded-output quarantine and shared eSpeak path correction, so final-source retesting remains
necessary; it is superseded by the final targeted rerun below. The actual fake ledger checks
observed four unique executions for four lines after
claim-only recovery; zero/one unchanged executions after after-intent/after-provider crashes;
one unchanged execution after observed-output recovery; and zero dispatch markers/executions
when cancelled after intent before send.

The final targeted runs used all API Rust source/tests/migrations and Python verification
primitives on the named base and dirty implementation branch, source-inventory SHA-256
`f63f83d679b0d6ca9ff41509cf8aaf6b1a2993a0dc7ac4050444c4c5a9454e7e`.
`target/durable-jobs/durable-realpg-final.log` records **PASS: 21 selected tests, zero failed,
10.13 seconds** on that source, including the same recurring worker's missing/invalid recorded
output quarantine followed by later-job acceptance. The prior crash/ledger counts above held
again. The actual local job's separate final PASS and audio facts are recorded above.

The initial Rust lint then rejected `StepWork` for `large_enum_variant`; the recorded failure is
`target/durable-jobs/verify-rust-large-enum-fail.log`. Boxing `RecordedOutput` at the enum and
construction boundary fixed that mechanical storage choice; no lint was suppressed. The
post-boxing source inventory recorded for the Rust gate was SHA-256
`f5dafdb66d61045bc0fbea72a6ca983d9e265f14a16cea9047b004c5dfb56939`.
`verify-rust-final.log` reports **PASS** for rust-format, rust-lint (workspace) and rust-tests
(workspace), including 59 API library tests, 17 `tts_local` tests and 61 xtask tests. The ignored
PostgreSQL/live tests do not count as executed in that workspace result. Mechanical independent
re-review found no actionable issue; the later reviewed source and full PostgreSQL result are
recorded below, together with the completed repository and partial-preflight dispositions.

`target/durable-jobs/verify-contracts-final.log` reports **PASS** for script-ir-reference,
contract-tests and wav-reference. The earlier full PostgreSQL gate retained in
`verify-postgres-http-oracle-fail.log`
passed all five Rust database suites (revision 16, import 9, adaptation 21, production 11 and
durable 21; the durable suite took 10.59 seconds), then **FAIL** at the terminal HTTP production
oracle: `KeyError: 'generation_or_cost_tables'`. The assertion referenced an old report key after
the no-dispatch oracle began reporting `generation_or_cost_rows`. The existing assertion now
checks that both generation attempts and generation/cost row counts are zero; its recovery
semantics were retained. That failed full run is not PASS because its earlier suites passed.
Independent review of the one-line oracle correction found
no actionable issue; its in-memory reversal reconstructed the prior oracle exactly.

The disposable runner now requests `max_wal_size=64MB`, `min_wal_size=32MB` and
`checkpoint_timeout=30s` to reduce obsolete preallocated WAL pressure in these small retained
test clusters. It explicitly keeps `fsync=on`, `synchronous_commit=on` and
`full_page_writes=on`; no commit-durability option was disabled. The WAL maximum is a soft
recycling target, not a hard disk cap. Independent source review found no actionable issue in
this server-options change and confirmed that it alters flush/recycling frequency without
changing authority/lease oracles. The fresh full gate below exercised the resulting runner.
The independent 14-changed-code-file aggregate after these Python fixes is SHA-256
`84901b848132613483a9f9de0197bb31bbbfcd4bab4e283110c9067316fca041`;
the final broad inventory in `target/durable-jobs/final-source-identity.json` is SHA-256
`75de0f2eb77d58cebde47b379322828a0e3355d946f7006d020604438bb2696e`.

The fresh `cargo xtask verify postgres` now reports **PASS** for the entire `postgres` gate in
`target/durable-jobs/verify-postgres-final.log`. It created/stopped its own PostgreSQL 17.4
cluster at `target/revision-evidence/cluster-1791732488566161000/pgdata`, without substituting
the debug or inherited database. All **78 selected database tests passed**. Exact Rust database
suite results were revision **16 PASS /
2.06 s**, import **9 PASS / 2.20 s**, adaptation **21 PASS / 15.02 s**, production **11 PASS /
1.65 s**, durable **21 PASS / 9.65 s**. The terminal independent oracles also report PASS for
real CLI/loopback HTTP, current production settings/rights/approval/eligibility, Studio editor,
preserved TXT/DOCX/failed imports, host kill, PostgreSQL restart, immutable exact replay,
migration replay, zero generation/cost rows in the no-dispatch journeys and MessagePack/JSON
equivalence. No ignored local live test is counted as executed by that full gate.

Earlier failures are retained and distinguished:

- `verify-postgres-first.log`: **FAIL**, revision migration upgrade test expected six migrations
  while migration 0007 made the actual count seven (15 passed, one failed). The fixture's
  expected additive migration count was updated; the fresh full gate above passed.
- `durable-realpg-first.log`: **FAIL**, zero passed / 19 failed at frozen fixture eligibility.
  The synthetic rights fixture was corrected, keeping current approval/rights admission intact.
- `durable-realpg-second.log`: **FAIL**, 11 passed / eight failed because synthetic charge
  references did not meet the existing receipt admission format. The references were corrected;
  no authority, receipt check or accounting constraint was weakened. The third rerun above passed.
- `local-espeak-realpg.log`: **FAIL**, zero passed / one failed, with `Paused` rather than
  `Completed`. The real installed runtime produced malformed/truncated output after receiving a
  long absolute `-w` filename. Original generated bytes remain retained. The shared local
  adapter was corrected to run from its private workspace with a relative output filename.
  `deep_workspace_passes_only_owned_relative_output_basename` covers the filename regression;
  the final actual eSpeak rerun passed as recorded above. This failed initial run remains retained.

Use the existing direct installed Rust/Cargo 1.87.0 binaries, PostgreSQL 17 tools and sequential
heavy lanes; the stable-named toolchain path was confirmed in the PR #38 handoff rather than
installed again. The same environment is:

```sh
PATH="/Users/manhblue/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/opt/homebrew/opt/postgresql@17/bin:$PATH" \
RUSTUP_TOOLCHAIN=1.87.0 CARGO_INCREMENTAL=0 RUST_TEST_THREADS=1 TMPDIR=/private/tmp cargo xtask <command>
```

The targeted debugging runs used this task's first disposable cluster, explicitly restarted at
port 58931 after the migration-count gate failure. It was not an inherited/production URL.
Each fixture owns its database in that isolated cluster; the fresh final xtask gate created
another disposable cluster. The task-owned debug cluster is now stopped and preserved with its
evidence. Exact targeted commands were:

```sh
PATH="/Users/manhblue/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/opt/homebrew/opt/postgresql@17/bin:$PATH" \
RUSTUP_TOOLCHAIN=1.87.0 CARGO_INCREMENTAL=0 TMPDIR=/private/tmp RUST_TEST_THREADS=1 \
CANTOS_TEST_CLUSTER_URL=postgresql://cantos_test_admin@127.0.0.1:58931/postgres \
cargo test -p cantos-api --test durable_postgres --locked -- --ignored --skip installed_local_runtime --test-threads=1 --nocapture

PATH="/Users/manhblue/.rustup/toolchains/stable-aarch64-apple-darwin/bin:/opt/homebrew/opt/postgresql@17/bin:$PATH" \
RUSTUP_TOOLCHAIN=1.87.0 CARGO_INCREMENTAL=0 TMPDIR=/private/tmp RUST_TEST_THREADS=1 \
CANTOS_TEST_CLUSTER_URL=postgresql://cantos_test_admin@127.0.0.1:58931/postgres \
CANTOS_DURABLE_ESPEAK_RUNTIME=/opt/homebrew/bin/espeak-ng \
cargo test -p cantos-api --test durable_postgres --locked installed_local_runtime -- --ignored --test-threads=1 --nocapture
```

| Command / boundary | Final result |
| --- | --- |
| Focused `jobs`, worker/CLI and mocked adapter tests | PASS in final workspace run; API library 59, local adapter 17; separate initial focused logs retained |
| Targeted real PostgreSQL durable suite, pre-lint mechanical boxing | PASS: 21 tests / 10.13 s in `durable-realpg-final.log`; fresh full gate additionally runs current source 21 PASS / 9.65 s |
| `cargo xtask verify rust` | PASS: rust-format, rust-lint (workspace), rust-tests (workspace), `verify-rust-final.log` |
| `cargo xtask verify contracts` | PASS: script-ir-reference, contract-tests, wav-reference, `verify-contracts-final.log` |
| `cargo xtask verify postgres` | PASS: entire fresh gate, `verify-postgres-final.log`; earlier terminal oracle FAIL retained separately |
| Explicit installed eSpeak durable-job case | PASS: one actual test / 1.13 s in `local-espeak-realpg-final.log`; four completed dialogues with audio facts above |
| `cargo xtask verify repository` | PASS: 308 text files, 15 skills, 146 Python tests and both git diff checks; `verify-repository-final.log` |
| `cargo xtask preflight --changed-only` | NOT_RUN; full preflight BLOCKED by disk for another fresh PostgreSQL cluster |
| `cargo xtask preflight --changed-only --skip postgres` | PASS for repository, rust-format, rust-lint (cantos-api), rust-tests (cantos-api) and wav-reference; postgres NOT_RUN, skipped by `--skip`; `preflight-unaffected.log` ends `PASS with 1 explicitly skipped (NOT_RUN) step(s)`; full preflight remains BLOCKED |
| Independent engineering/pipeline review and fixed-defect retests | No actionable findings on reviewed final source, including mechanical boxing, terminal oracle correction and explicit durable WAL options; fresh full PostgreSQL regression gate PASS |
| Google live/billing/commercial dispatch, VieNeu dispatch, listening, publication, UI/mobile | NOT_RUN; outside authorization/scope |

The PostgreSQL suite is ignored by ordinary `cargo test` and run by the disposable xtask gate.
The runner removes inherited database configuration and uses a new local cluster and restricted
app role; it must not substitute a production/inherited URL. `tests/durable_postgres.rs` has
an independent relational invariant checker over reservations/settlements, selected receipt
scope and budget counters, plus a regression proving the checker rejects corrupted facts.
The process harness kills only children it starts, restarts the worker with the same database
and private root, and reads the fake provider's execution ledger. Source presence is not PASS.

## Affected read inventory and DBSP decision

| Path / consumer | Query and bounds | Authority / feeding writes |
| --- | --- | --- |
| `production-jobs status` → `Store::job_status` / `summary` | Run/job point lookup and per-job total/succeeded counts; at most 1,000 steps per run | Current session/script permission on every read; job/step/control writes |
| Worker `claim_step` | Mode-filtered eligible job, one step, ordered bounded `LIMIT 1`, row locks/`SKIP LOCKED` | PostgreSQL job/step lease, deadline and terminal/control state |
| Worker `prepare_step` / recovery | Exact run/snapshot/step plus latest attempt, observation/resolution and budget | Current owner approval/head/settings/rights/time; intent/observation/resolution/budget writes |
| Dispatch / heartbeat / acceptance | Exact scoped lease/token/deadline, attempt/observation and frozen input | Current PostgreSQL authority and fence inside mutation transaction |
| Enqueue/control replay | Unique actor/script/job operation point reads | Current session/member/owner checks; immutable operation receipts |

The new queue index covers eligible step status/deadline/order and a job/status index supports
bounded step counts; existing primary/unique indexes support exact run/snapshot/operation reads.
The four-dialogue fixture can reasonably choose sequential scans; index presence is not proof
of a faster query. There is no new public list API, unbounded N+1 fetch or measured hot repeated
multi-tenant aggregate. Actual queue frequency/cardinality at production scale is unknown.

Baseline protocol: one synthetic owner, one run and four frozen dialogues, restricted app role,
real authorization and isolated disposable PostgreSQL. Nine `job_status` samples record median
and nearest-rank p95; `EXPLAIN (ANALYZE, BUFFERS)` records the actual summary and claim-selection
plans. A local observational target is status p95 below 100 ms at concurrency one; this small
sample is not a production SLO, throughput benchmark or regression gate. Broader p99,
CPU/RAM, scale/skew and write-latency budgets are not measured here and remain a gap before
performance acceptance. The final targeted-source PostgreSQL 17.4 baseline is retained in
`target/durable-job-evidence/7dbbaad8-16a5-4efb-a350-172a7ea2f91c/read-plans.log`:

| Observation | Actual bounded result |
| --- | --- |
| Authorized `job_status`, nine samples | Median 0.856 ms; nearest-rank p95 0.920 ms at concurrency one |
| Summary `EXPLAIN` | Execution 0.032 ms; planning 0.210 ms; one job row; five shared-buffer hits |
| Summary step counts | `durable_steps_by_job` index-only scans: four total rows / zero succeeded; four / zero heap fetches |
| Claim-selection `EXPLAIN` | Execution 0.022 ms; planning 0.105 ms; one selected row; 12 shared-buffer hits |
| Claim plan | One-row run scan, indexed job/step checks, `SKIP LOCKED`, 25 kB quicksort; no reported spill |

These plans ran on the real app role, while the timed `job_status` path applied actual session
and script permissions. `EXPLAIN` query time includes instrumentation and is not application
latency. The observed p95 was below the local 100 ms observation budget; nine warm sequential
samples/four steps cannot establish tail latency, cold behavior, throughput, broad queue scale
or a production performance acceptance. The earlier pre-final baseline remains in
`target/durable-job-evidence/93655cf7-4b30-410b-ac62-ce17ca449ae2/read-plans.log`
(median 0.822 ms / p95 0.954 ms); no performance change is attributed to that small difference.

Simpler optimization comparison: retain PostgreSQL point/unique/eligible-step indexes and
serialized job/budget consistency. This is a newly implemented query workload; no previous
durable-worker baseline exists and no before/after speedup or isolated index gain is claimed.
Future scale work must compare representative cardinality/skew against simpler PostgreSQL
query/index changes before attributing benefit to another engine.

DBSP decision: **DEFER**. There is no measured hot repeated aggregate/large join consumer,
incremental adapter, CDC/checkpoint or reviewed adoption need. Rights, controls, reservations
and acceptance require current transactional authority; an asynchronous derived view would add
freshness/rebuild/operational cost. Direct reads represent their own PostgreSQL transaction, so
engine lag/fallback/rebuild checks are inapplicable to this slice. Feeding mutations preserve
atomic PostgreSQL visibility. Reopen only when a concrete measured consumer meets the existing
[read assessment admission conditions](../../.agents/skills/cantos-engineering/references/postgresql-read-performance.md).

Later update (#58): this summary now has an opt-in, feature-gated experimental DBSP read model
([ADR 0017](../decisions/0017-experimental-dbsp-job-summary.md),
[evidence](dbsp-job-summary.md)). This section's measurements and its conclusion for the default
path are unchanged; production adoption remains unadmitted.

## Completion report and residual risk

- **Invariant:** exact frozen dialogue result selection, conservative possible-execution
  recovery, monotonic fences and bounded integer reservation prevent silent duplicate/stale work.
- **Owner / boundary:** pure `jobs` core; existing production/receipt/audio/local adapter;
  PostgreSQL durable shell; same-package development CLI and separate worker process.
- **Evidence level:** `documented` implementation/read inventory, final workspace
  `example-tested`/`mock-integration-tested` core and adapter output, real PostgreSQL/process
  `integration-tested` / `fault-injected`, and local eSpeak `provider-live-tested` with structural
  `audio-measured`. Named source snapshots and remaining broader gates are distinguished above.
- **Evidence:** named tests/logs and actual gate results above; no ignored test or skipped gate
  counted as executed/pass.
- **Command:** exact targeted commands and xtask environment above; Rust, contracts,
  PostgreSQL, repository and five partial-preflight steps passed. The skipped PostgreSQL step
  is NOT_RUN; the required no-skip preflight and complete handoff remain resource-blocked.
- **Provenance:** merged PR #38 base `209e9e905c680e7cecab831f2d8f7894656e14fe`; dirty
  implementation on `feat/durable-tts-jobs`; original Vietnamese Script IR fixtures, synthetic
  PCM/process ledger and disposable local PostgreSQL. Targeted source SHA-256 `f63f83d6…`,
  post-boxing source `f5dafdb6…` and final broad source `75de0f2e…` are fully recorded above.
  Git/merge/tree identity belongs
  to the subsequent local handoff, not a self-referential committed SHA in this document.
- **Edge classes covered:** test catalog includes legal/illegal transitions, retries/jitter,
  duplicate delivery, intent/send uncertainty, actual child kill/restart, stale leases, current
  authority invalidation, exact receipt scope, pause/cancel races, budget concurrency, immutable
  history, migration replay/upgrade, contradiction and recorded-output quarantine, private-output
  faults and continued recurring-worker progress. Their actual executed dispositions are above.
- **Not covered / residual risk:** Google live/billing/commercial execution and VieNeu production
  are not authorized; listening/rights acceptance remains open. No provider exactly-once
  contract, complete billing ledger, host power-loss proof, object storage/retention/CDN, cache,
  partial regeneration, mixing, QC/publication or UI/mobile proof. Scale/p99/resource/read/write
  benchmarks and fuzzing are NOT_RUN. Issue #7 is not closed by this foundation.
- **Run / step identity:** each run pins production-p1 snapshot and exact accepted revision;
  each dialogue step pins `durable-input-v1`, with unique attempt/operation/reservation and
  receipt-v1 linkage. Actual local run `a71d37c2-9b23-4439-af1e-b1abd196e108` and its exact
  snapshot/input digest are recorded with the four dialogue facts above.
- **Crash points exercised:** actual process kill at after-claim, after-intent, after-provider
  and after-observation; cancellation after intent before send; recurring-worker quarantine
  followed by later accepted work before owned child kill. Counts/outcomes are above.
- **Duplicate-call oracle:** independent create-only fake provider operation ledger; actual
  counts and source snapshot above. No vendor deduplication guarantee.
- **Provider evidence:** fake/mock plus four actual local eSpeak dialogue invocations, exact
  version/profile and native byte facts above. Google/VieNeu worker execution NOT_RUN.
- **Cost accounting:** frozen integer USD/VND policy; reservation, retained uncertainty,
  release, provisional local and reported synthetic settlement. Actual charge/resource cost
  unknown; local/caller usage is not a provider bill. Actual account billing NOT_RUN.
- **Audio facts:** native `wav-pcm-s16le-mono-22050-v1`; receipt/WAV/hash audit, actual local
  frames/sizes/checksums in the table above. Synthetic PCM is not speech quality evidence.
- **Listening:** audio-listened absent; no named reviewer/render acceptance.

The [queue](../work-plan/README.md#recommended-bounded-follow-ups) recommends separate #6
acceptance, remaining #7 adoption criteria and #8 reuse/partial-regeneration work; this report
does not authorize their implementation. The subsequent local handoff will record the draft PR,
Git SHA, remote/tree comparison, working tree and disk after Git actions. Merge is blocked while
the required full preflight is incomplete; issue #7 remains open. Initial handoff had about
930 MiB free. Requester-authorized cleanup removed only inactive reproducible Cantos workspace
build artifacts, recorded in `inactive-cache-cleanup.json` (1,045.7 MiB) and
`inactive-workspace-cache-cleanup.json` (681.6 MiB), plus about 1 GiB of inactive incremental
compiler data. After confirming no Web compiler was active, `inactive-wasm-cache-cleanup.json`
records another 807.0 MiB of inactive WASM dependencies/build/incremental/fingerprint directories.
Free disk was about 2.3 GiB before the fresh PostgreSQL gate and later about 517 MiB. The
metadata-based `inactive-ui-native-cache-cleanup.json` then records 421.8 MiB of inactive native
UI compiler artifacts, preserving the API/xtask/shared dependency closure. No database was
removed to recover that space. `inactive-web-output-cache-cleanup.json` records another
101.3 MiB of inactive top-level Web compiler/bindgen output; preview/dist/screenshot evidence
was retained.

Normal PostgreSQL-owned `pg_switch_wal()` and `CHECKPOINT` maintenance on the two task-created
failed clusters recycled their future WAL pools from approximately 544 / 400 MiB to 64 MiB
each. `owned-cluster-checkpoint.log` and `owned-cluster-wal-cycle.log` retain the actual engine
operations and show fsync/synchronous_commit/full_page_writes still on. Every database/row was
kept; this was WAL recycling, not file deletion. The previous PR #38 cluster was untouched.

Disk was about 836 MiB after the full PostgreSQL PASS and 717 MiB after the partial-preflight
builds, below the approximately 1 GiB additional space needed for another fresh preflight
PostgreSQL cluster. Full preflight remains BLOCKED by resources, not an observed implementation
failure in the passed gate results.
The parent requested new human authorization to remove only `pgdata` from the two stopped
task-owned failed clusters `1791730669920731000` and `1791731826490402000`, while retaining
their logs/audio/evidence and the successful cluster. That authorization is pending and no
database directory has been removed. The unaffected repository gate passed, and the partial
preflight passed five steps with an explicit postgres skip recorded as NOT_RUN; that does not
satisfy full preflight or authorize merge. No whole `target`, database, audio/media/models,
source/evidence, old Tabula cache or
user application was removed/shut down in the recorded cleanup.
