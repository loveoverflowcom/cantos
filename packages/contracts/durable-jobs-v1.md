# Durable jobs v1 — development foundation

Status: implemented bounded backend contract for [issue #7](https://github.com/loveoverflowcom/cantos/issues/7).
The [evidence report](../../docs/evidence/durable-tts-jobs.md) records actual verification separately.
This is an internal development CLI/store contract, not a new Studio HTTP API or a production
provider guarantee. [Production input v1](production-v1.md) owns frozen documents/current
approval and [TTS receipt v1](tts-receipt-v1.md) owns attempt/audio admission. The
[pipeline](../../docs/product/production-pipeline.md) remains the product source of truth.

## Execution boundary

`production-jobs` and `cantos-worker` are binaries in `services/cantos-api`, sharing the same
PostgreSQL store and pure `jobs` rules as the backend. No new crate, microservice, broker,
generic provider framework, HTTP route or shared wire DTO is introduced. Both CLIs require
`CANTOS_ENV=development` and `CANTOS_DATABASE_URL`; it is rejected unless it names a permitted
local PostgreSQL configuration. User-facing commands additionally require the existing
`CANTOS_SESSION_TOKEN`. Credentials and database URLs are environment values, never reports or
command-line arguments.

Execution mode is exactly `local_espeak` or `synthetic_fake`. Both admit only the existing
eSpeak NG 1.52.0 provider/model/adapter/output profile, the declared Vietnamese voices/languages
and explicit neutral/default controls. The fake is an explicit simulation, never a catalog
provider or real voice result. Google commercial dispatch is disabled; Google live and billing
are **NOT_RUN**. VieNeu remains research-only **NO_GO** and is not reachable from the worker.

## Commands and result

```text
production-jobs enqueue <script-id> <snapshot-id> <operation-id> (--local-espeak|--test-fake)
production-jobs status <run-id>
production-jobs (pause|resume|cancel|retry) <run-id> <operation-id>
production-jobs resolve-not-executed <run-id> <attempt-id> <evidence-reference>

cantos-worker --output-root <absolute-private-directory> --worker-id <id>
  [--once] [--lease-seconds <1..300>]
  (--local-espeak <absolute-runtime> | --test-fake
    [--failpoint <after_claim|after_intent|after_provider|after_observation|before_accept|after_accept>])
```

The worker has no implicit provider. Failpoints are admitted only with the explicit fake.
`--once` claims at most one eligible step; ordinary worker mode polls while owning only its
local process. The output root already exists as a private directory, with no group/other
permissions on Unix. Store it outside Git or in ignored local evidence storage.

Commands print a JSON summary containing `run_id`, `job_id`, `status`, `reason`,
`completed_steps` and `total_steps`. UUID inputs are normalized; unknown command/provider
flags are rejected. A nonzero exit is not an automatic retry instruction. The CLI's initial
retry policy is three attempts, base delay one second and cap 30 seconds. Store callers use a
validated policy with one to eight attempts; persisted delay bounds are at most 86,400 seconds.

Enqueue is keyed by `(script_id, actor_id, operation_id)`. An exact same-payload retry returns
the existing run's current summary; a changed snapshot, mode or retry policy under that key
conflicts. Current authorization is rechecked even for retries. Control operations similarly
bind `(job_id, actor_id, operation_id)` to one action and return the current summary on exact
replay. They do not rewind completed work or reset append-only attempt counts.

## Frozen authority and identity

Enqueue requires the current owner-authorized production snapshot and exactly one selected
dialogue step per resolved line (one to 1,000). The immutable run pins script, snapshot,
creating actor, `production-p1` input digest, mode, request bytes and retry policy. The snapshot
already contains the exact accepted revision, complete export/e1, content/c1, resolved
casting/pronunciation and required-rights/budget versions.

The shell reconstructs/admit-checks the frozen document against the existing production
authority. Current owner/member access, editorial review, exact approval, head/settings/rights
and expiry are rechecked under the transaction's authority locks before intent creation,
committed send authorization and result selection. A draft edit cannot modify an active run's
document; it can invalidate permission to continue. Status applies current script permission
but remains inspectable after production eligibility expires.

Each step binds its dialogue to:

```text
durable-input-v1:sha256:
  SHA256(b"cantos/durable-input/v1\n" + serde_json(ResolvedSynthesisInput))
```

The fixed resolved-input serialization includes dialogue/character identity, effective text,
language, provider/model/voice/adapter/output contract, performance, pronunciation and voice
rights reference. This is a versioned **run linkage identity**, deliberately including scope
fields. It is not a position-free speech cache fingerprint and authorizes no cross-run reuse.
A future encoding/field change requires a new scheme and compatibility handling; old steps
must not be silently rehashed.

## Durable records

Migration `0007_durable_jobs.sql` adds:

| Record | Meaning / mutability |
| --- | --- |
| `durable_runs` | Immutable snapshot scope, enqueue identity and retry policy |
| `durable_jobs` | One execution status/reason per run |
| `durable_steps` | One dialogue/input fingerprint; mutable lease/status; write-once accepted attempt |
| `durable_budgets` | Immutable run/currency/limit; serialized reservation/settlement totals |
| `durable_reservations` | Immutable attempt budget allocation, scoped to run and step |
| `durable_attempts` | Immutable operation, intent, reservation, attempt number and original fence |
| `durable_dispatches` | Immutable unique committed send marker per attempt |
| `durable_observations` | Immutable exact receipt and optional private output path per attempt |
| `durable_settlements` | One immutable release/provisional/synthetic settlement per reservation |
| `durable_resolutions` | Immutable owner non-execution attestation with evidence reference |
| `durable_controls` | Immutable actor/action/operation receipt |

Foreign keys and insert-time scope checks prevent snapshot/script/digest, reservation/run/step
and selected-attempt/step mismatches. Triggers preserve settled records, job/step identity,
monotonic fences, legal status changes and write-once selection. The runtime role has no DDL
or immutable-record update/delete permission. A PostgreSQL superuser remains outside that
application boundary. Migration replay retains the existing checksum contract.

## States, leases and effect order

Execution states are `queued`, `running`, `completed`, `failed`, `paused` and `cancelled`.
Editorial review and publication approval are separate; `completed` establishes only that all
dialogue steps selected a valid result in this run.

| From | Pure event → next |
| --- | --- |
| `queued` | start → `running`; pause → `paused`; cancel → `cancelled` |
| `running` | complete → `completed`; fail → `failed`; pause → `paused`; cancel → `cancelled` |
| `paused` | resume → `queued`; cancel → `cancelled` |
| `failed` | explicit retry → `queued`; cancel → `cancelled` |
| `completed`, `cancelled` | terminal; replacement requires a new run |

Step states are `pending`, `leased`, `succeeded`, `failed` and `cancelled`. Claim selects one
eligible step with row locks/`SKIP LOCKED`, assigns an expiring lease and increments its fence.
The lease duration is one to 300 seconds, default 30. Heartbeats extend only a still-current
lease. The job lock gives controls, reservations and acceptance a consistent order; synthesis
and filesystem I/O run after the transaction commits.

1. Claim under a new fence and reconstruct current-authorized frozen input.
2. Commit integer-money reservation, stable attempt/operation identity and conservative intent.
3. Write supplementary create-only private intent/input files.
4. Recheck current authority and fence; commit one unique dispatch marker.
5. Invoke the selected local/fake adapter once; inspect output with the existing receipt/WAV rules.
6. Persist output/journals and an exact immutable observation. Late observations are unfenced
   facts, so pause/cancel/lease loss does not erase possible execution or accounting.
7. Recheck current authority/fence and accept one exact observed result, without replacing a
   previous selection. Mark the job completed only when all its steps succeeded.

## Recovery, ambiguity and controls

An expired claim with no intent can be reclaimed. An observed output can be recovered under a
new fence only by reopening its exact private attempt path and revalidating checksum, WAV
profile, document, operation, provider and dialogue linkage. A stale worker cannot accept,
heartbeat or authorize a send under its old fence.

If current authorization becomes invalid while a queued job waits, preparation durably pauses
that job as `authorization_invalid`, fences its unfinished steps and allows later eligible jobs
to progress. A missing recorded output is quarantined as `recorded_output_unavailable`; unsafe,
corrupt or receipt-mismatched bytes are quarantined as `recorded_output_invalid`. Neither case
creates a fresh provider attempt or discards the original observation. Recovery of the private
file/authorization needs explicit repair and current checks; it is not proof of non-execution.

An intent with no definitive observation, or any `may_have_executed` failure, pauses with
`uncertain_attempt` and retains the reservation. Intent/dispatch markers and local files cannot
prove non-execution after a crash. There is no automatic redispatch or provider lookup. The
owner can record a bounded nonblank evidence reference with `resolve-not-executed`, releasing
that reservation; a separate authorized resume is required. This is a human assertion, not
vendor-confirmed non-execution. Contradictory late output is preserved and quarantined as
`resolution_contradiction`, and cannot be accepted or silently resumed.

Only timeout/rate-limit failures marked `not_sent` or `confirmed_not_executed` permit bounded
retry. Other definitive failures stop; exhausted history cannot reset itself through restart or
control replay. Backoff uses a bounded exponential ceiling and equal jitter seeded by the
immutable recorded operation UUID, so recovery computes the same delay. The database enforces
the observation/resolution deadline even after explicit resume/retry. Commercial concurrency
policy remains a later dispatch gate.

Pause/cancel acquire the job lock, prevent new claims/send authorization and increment fences
for unfinished steps. Already authorized provider work may finish; its exact observation and
accounting survive, while selection is blocked. Resume recovers existing valid output rather
than invoking the provider again. Cancelled/completed runs remain terminal. This is idempotent
result selection with conservative recovery, not exactly-once external TTS or billing.

## Money, audio and scope limits

Amounts are integer minor units in the frozen `USD` or `VND` currency. Conditional budget
updates serialize concurrent reservations, enforcing reserved plus settled at or below the
run limit. Uncertain attempts keep their reservation. Synthetic reported charges are fixture
facts; local unknown charge uses provisional settlement and is never shown as a real bill.
Local eSpeak reserves zero provider-spend units, which makes no CPU/resource or legal claim.
Commercial overruns, account reconciliation and a complete billing ledger must be designed
and tested before any commercial dispatcher is enabled.

The worker admits the existing native 22,050 Hz mono signed 16-bit PCM profile. Private output
uses create-only files with sync calls and bounds; recovery rejects symlinks, hardlinks,
nonregular files and paths outside the exact configured attempt. Filesystem/host power-loss
durability, distributed/shared storage and asset retention are not established by process
kill tests. No object-storage asset, CDN output, mix, QC approval or release manifest is created.

Full cache/reuse, partial regeneration, Studio progress/preview controls, mixing, object
storage/CDN, publication, Theatre and native mobile are outside this slice. Issue #6's live,
rights and listening gaps remain open. Issue #7 stays open until its full acceptance criteria
are satisfied; the [work queue](../../docs/work-plan/README.md) records recommended follow-ups.
