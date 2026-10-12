---
name: cantos-production-pipeline
description: >-
  Rule owner for implementing and verifying Cantos production runs: frozen production inputs,
  durable PostgreSQL jobs with expiring leases and fencing tokens, write-once acceptance and the
  transactional outbox, provider-neutral TTS and adaptation ports with capability checks and
  ambiguous-attempt reconciliation, integer-money reservations and budgets, canonical speech and
  mix fingerprints with partial regeneration and cache reuse, versioned audio profiles, mix plans,
  technical QC records and fault-injection testing. Use for production runs, workers, claims,
  retries, cancellation, TTS or adaptation providers, voice and control mapping, cost, budgets,
  cache keys, regeneration, mixing, loudness or QC code.
---

# Cantos production pipeline

This skill is a **behavioral contract and router** for the path from a pinned script revision to
accepted, QC-measured audio artifacts. It composes [`cantos-engineering`](../cantos-engineering/SKILL.md):
the required order, the [evidence vocabulary](../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
and the [completion report](../cantos-engineering/SKILL.md#9-completion-report) apply unchanged.
This skill adds the production-specific method and never redefines them.

## Who owns what

| Truth | Owner |
|---|---|
| Stage flow, frozen inputs, cache and partial regeneration, jobs and recovery, audio and QC, cost visibility | [pipeline](../../../docs/product/production-pipeline.md) |
| Casting validity, no silent voice substitution, fingerprint contents, regeneration scope, budgets, durability | [business rules § Casting](../../../docs/product/business-rules.md#casting-and-performance), [§ Production, caching and costs](../../../docs/product/business-rules.md#production-caching-and-costs) |
| Lease, fencing and outbox as a proposal to validate | [architecture § Data and delivery](../../../docs/architecture/overview.md#data-and-delivery) |
| Phase scope, acceptance and review boundary | [work plan 020](../../../docs/work-plan/020-cast-and-generate-dialogue.md), [030](../../../docs/work-plan/030-mix-review-and-publish.md) |
| Human production record | [production run](../../../templates/production-run.md) |
| **How** to model, claim, call, reuse, measure and test those rules | this skill |
| Script revisions, `SpokenContent` bytes and digests | [`cantos-script-ir`](../cantos-script-ir/SKILL.md) |
| Rights evaluation, approvals, the publication gate, release and delivery | [`cantos-publication`](../cantos-publication/SKILL.md) |
| Listener contracts that must never start a job | [`cantos-listening`](../cantos-listening/SKILL.md) |

When this skill and a document disagree, the document wins for product behavior: report the
conflict and fix the stale side when in scope.

## Repository reality

The backend implements frozen production candidates, scoped approval, bounded WAV/receipt
inspection and sample-only eSpeak/Google probes. The bounded
[durable worker foundation](../../../docs/evidence/durable-tts-jobs.md) adds same-package
development-only local/fake execution with PostgreSQL runs/jobs/steps, immutable intent and
observations, reservations, leases and fenced result selection. Google commercial dispatch
stays disabled; VieNeu Nano remains research-only and refuses production dispatch. A bounded
[clip reuse slice](../../../packages/contracts/clip-reuse-v1.md) adds the position-free `speech-v1`
fingerprint, a verified local clip registry, attempt-less reuse under the existing fence and a
read-only regeneration plan. Cross-run in-flight de-duplication, unpinned-model refresh epochs,
mixing, object-storage acceptance, transactional outbox and publication remain unimplemented.
The advanced production spine and illustrative types below remain **proposed** unless named
implementation evidence says otherwise. Discover the real manifests,
migrations and modules first. A provider, an audio tool, a numeric loudness or
peak target, a retry schedule and a budget currency each need a recorded decision
([ADR](../../../templates/adr.md), after listening tests for audio) and a pinned, verified version
before they are treated as settled. Libraries (sqlx, proptest, a failpoint crate, cargo-fuzz,
ffmpeg) are candidates, not commitments.

## The production spine

```text
shell   read: frozen inputs · step and attempt history · accepted artifacts · budget · one clock
        instant · a jitter sample · provider capabilities
          ↓ plain typed facts (no ports, no async)
core    freeze_production_inputs · plan_regeneration · resolve_reuse · decide_next_step
        classify_failure · backoff_delay · plan_reconciliation · resolve_attempt
          ↓ StepDecision (data)
shell   claim a step → ONE txn: reservation + attempt intent + operation ID → send → record
        observations (unfenced, idempotent) → probe the bytes → upload and verify →
        ONE txn: fenced write-once accept + outbox rows
          ↓ then, per scene and episode
core    plan_scene_mix → tool shell renders → measure → evaluate_technical_qc → QcResult facts
```

Only decisions computed from facts read **inside** the committing transaction may reserve budget
or accept a result. A worker may be killed between any two shell steps; each step names what a
restart reads and what the next claim does.

## Rules this skill owns

These rules govern later production implementation; frozen-input admission and sample adapters
already implement bounded parts, as recorded in the repository evidence. Do not infer durable
execution from a probe or current approval from an offline receipt. Rule cards with good example,
counterexample, oracle and exception live in the linked reference.

| # | Rule | Failure it prevents | Oracle | Reference |
|---|---|---|---|---|
| PL1 | A run reads only `FrozenProductionInputs`; editing the draft never changes it | in-flight audio mixes text from two revisions | edit-during-run test; digest unchanged | [jobs](references/durable-jobs.md#1-create-a-run-from-frozen-inputs) |
| PL2 | Execution status, editorial review and publication status are separate machines | a completed job reads as approved | exhaustive state × event table | [jobs](references/durable-jobs.md#2-three-state-machines-one-meaning-each) |
| PL3 | Claim with an expiring lease and a monotonic fencing token; never hold a transaction across synthesis | a crashed worker's step never resumes | crash test on real PostgreSQL | [jobs](references/durable-jobs.md#4-claim-heartbeat-accept-the-fenced-protocol) |
| PL4 | Acceptance is write-once and conditional on the token; the record of what happened is unfenced and idempotent | a stale worker overwrites a newer result, or its paid call vanishes | stale-token accept returns zero rows; its observation persists | [jobs](references/durable-jobs.md#5-outcomes-are-facts-acceptance-is-fenced) |
| PL5 | Intent, operation ID, key and reservation are durable before the send; an ambiguous outcome is reconciled before any repeat | duplicate billing and duplicate output | provider-side execution ledger shows one execution | [adapters](references/provider-adapters.md#5-reconcile-before-you-repeat) |
| PL6 | Failures are a closed class set mapped to a retry decision with bounded jittered backoff and no wildcard arm | blind retry loops; unbounded spend | exhaustive decision table; bounds over attempts × jitter | [jobs](references/durable-jobs.md#3-the-pure-core) |
| PL7 | A fingerprint covers every effective input and nothing else; downstream keys hash accepted checksums | stale audio reused, or a reorder rebilled | per-field mutation test; reorder-only metamorphic test | [fingerprints](references/fingerprints-and-invalidation.md) |
| PL8 | Reuse needs fingerprint, scope, checksum and availability; regeneration is a pure plan | another creator's voice or a deleted object reused | one example per `ReuseDecision` variant | [fingerprints](references/fingerprints-and-invalidation.md#5-reuse-needs-four-facts-not-one) |
| PL9 | Money is integer minor units with a currency; estimate, reservation, usage and settlement are different records; unavailable is not zero | a float drift; an estimate shown as a charge | boundary tests; two connections racing for the last budget | [cost](references/cost-and-budget.md) |
| PL10 | Ports are named for capabilities; a required unsupported control blocks; credentials stay server-side and redacted; fakes are unreachable from production | silent voice substitution; leaked keys; a demo reported as support | exact-variant table; sentinel log scan | [adapters](references/provider-adapters.md) |
| PL11 | QC results are append-only facts bound to the artifact checksum, profile and tool; a technical pass is not editorial approval; uncertainty is surfaced | an approval resting on a different render; certified fidelity without evidence | golden synthetic signals; stale-result join test | [audio](references/audio-mix-and-qc.md) |
| PL12 | Untrusted bytes are probed under resource bounds before an artifact exists | a panic, memory blow-up or hang on malformed audio | fuzz target with bounds; named regression inputs | [audio](references/audio-mix-and-qc.md#8-decoders-are-fuzz-targets) |
| PL13 | Crash, duplicate, timeout, stale-worker and budget behavior is proven at named fault points against an independent invariant checker | prose guarantees that fail on the first restart | scenario catalog run on real PostgreSQL | [faults](references/fault-injection-testing.md) |

## Working order for a production change

Apply the foundation's [required order](../cantos-engineering/SKILL.md#the-required-order) with
these pipeline-specific steps:

1. **Read** the owning doc sections above, the 020/030 acceptance criteria, the linked issue and
   the code. The [business-rule invariants](../../../docs/product/business-rules.md#product-invariants-to-verify)
   "one line changes", "worker dies after dispatch", "provider response is lost" and "repeated play
   costs nothing" are in scope for any job, cache or provider change.
2. **Ledger.** One row per claim: the step or attempt affected, the interleaving that breaks it, the
   oracle and the evidence level you will honestly reach.
3. **Facts first.** Write the plain fact structs and the decision enum before the worker loop. If a
   decision reads a port, the core is not extracted yet.
4. **Name the crash points** of every new effect (claim, reserve, send, observe, probe, upload,
   accept, outbox, consume) and write down what a restart does at each.
5. **Close the construction paths** of `FrozenProductionInputs`, the lease token, a reservation and
   an accepted artifact so a handler cannot forge the proof
   ([types-as-proofs](../cantos-engineering/references/types-as-proofs.md)).
6. **Implement the shell** behind narrow capability ports, never a vendor name in the domain
   ([decoupling](../cantos-engineering/references/decoupling.md)).
7. **Test** per the table below, then report with the production fields.

## Verification: what each claim needs

| Claim | Cheapest adequate oracle | Honest label |
|---|---|---|
| decision precedence and failure classification | exhaustive table over the fact dimensions, exact variants | `example-tested` (state the domain) |
| backoff bounds and determinism | exhaustive attempts × jitter loop | `example-tested`; `property-tested` if generated |
| one changed line plans exactly its speech and dependent mixes | hand-written graph, `plan_regeneration` equals the expected set | `example-tested`; `property-tested` over mutations |
| each effective input changes the fingerprint; position does not | per-field mutation plus reorder property | `property-tested` |
| fingerprint encoding is stable | golden literals reviewed separately | `differentially-tested` |
| claim, heartbeat, fencing, write-once accept, reservation race | concurrent connections on real PostgreSQL | `integration-tested` — never an in-memory store |
| no second paid call after a lost response, worker death or duplicate delivery | scripted provider with its own execution ledger plus named failpoints | `fault-injected` |
| the provider mapping matches the vendor | recorded real envelopes, never adapter output | `differentially-tested` |
| a real provider returns playable Vietnamese audio | cost-capped, opt-in live run | `provider-live-tested` |
| loudness, peak, duration and clipping are correct | synthetic signals with analytical answers, then a named tool and profile | `audio-measured` |
| the mix and performance sound right | a named reviewer on the named render | `audio-listened` |
| the probe survives hostile bytes | fuzz target with resource bounds | `fuzz-tested` (robustness only; no correctness claim) |
| assertions kill defects in planner, fingerprint and decision code | mutation run on the pure modules | `mutation-tested` |

A scripted fake provider is `example-tested` or `fault-injected`, never `provider-live-tested`.
Mocked and live evidence are separate lines in every report, as the
[work plan](../../../docs/work-plan/README.md#shared-completion-evidence) requires.

## Smells that fail review

| Smell | Why it is wrong | Do instead |
|---|---|---|
| a worker calls `script_repo.latest(episode)` | the run now mixes revisions | load the frozen input document by run ID |
| `SELECT … FOR UPDATE` held while synthesizing | a dropped connection releases it and a second worker repeats the paid call | lease row plus fencing token, committed per statement |
| `UPDATE step SET status = 'succeeded' WHERE id = $1` | a stale worker overwrites a newer result | `WHERE lease_token = $2 AND accepted_artifact_id IS NULL` |
| `for _ in 0..3 { submit() }` after a timeout | a timeout may already be billed | record the ambiguous attempt, reconcile, then decide |
| insert the attempt row after the provider responds | a crash leaves an unrecorded paid call | intent and reservation first, then send |
| `attempts = 0` in the claim query | crashes bypass the retry bound | count from the append-only attempt history |
| adapter maps an unknown emotion to `neutral` | silent performance substitution | blocking finding `EmotionUnsupported` |
| `cost: f64`; `unwrap_or(0)` for an unknown estimate | drift, and free-looking runs that are not free | `Money` in minor units; `CostEstimate::Unavailable` |
| key = `(dialogue_id, script_revision)` | stale audio after a voice or model change | exhaustive destructuring of every effective input |
| position or revision ID inside the speech key | a reorder rebills paid speech | position-free `SpokenContent` |
| rights status inside a cache key | an expired licence regenerates instead of blocking | `ReuseDecision::Block`; rights are checked, never keyed |
| `qc.passed = true` without an artifact checksum | the result may describe another render | join QC to the exact checksum, profile and tool |
| editing dialogue text to fix a loudness problem | silently changes the story | change the mix profile or plan; the text is untouched |
| decoding provider bytes without bounds | a hostile file hangs or exhausts memory | probe with size, duration and time limits first |
| `TTS_PROVIDER=fake` accepted in production | a demo reported as provider support | registry rejects fake IDs |
| claiming "exactly-once synthesis" | no provider contract proves it | "at-least-once execution, idempotent acceptance" |

## Completion report

Use the [foundation report](../cantos-engineering/SKILL.md#9-completion-report) with every field,
then add:

```text
Run / step identity:      <run ID, step kind, input digest, fingerprint scheme and version>
Crash points exercised:   <named points and outcomes, or "none">
Duplicate-call oracle:    <provider-side execution count per fingerprint, or "not exercised">
Provider evidence:        <fake | recorded fixture | live: provider, model, adapter version, cap>
Cost accounting:          <estimate · reservation · usage · settlement kinds touched; currency>
Audio facts:              <profile revision, tool and version, measured values, or "no audio rendered">
Listening:                <named reviewer and render checksum, or "audio-listened absent">
```

"Pipeline verified" is not an evidence level. Name the label from the vocabulary for each line and
keep fakes, recorded fixtures and live providers distinct.

## References — load the relevant set, one at a time

| The work is about… | Reference |
|---|---|
| frozen inputs, run/step/attempt states, `decide_next_step`, failure classes, backoff, cancellation, claim, heartbeat, fenced accept, recovery sweep, outbox, operation IDs | [`durable-jobs.md`](references/durable-jobs.md) |
| provider ports, capability checks, send boundary, attempts, reconciliation, bytes validation, credentials, fakes, fixtures, live tests, adapter versions | [`provider-adapters.md`](references/provider-adapters.md) |
| speech, mix, master and rendition fingerprints, the dependency graph, partial regeneration, cache reuse, unpinnable models | [`fingerprints-and-invalidation.md`](references/fingerprints-and-invalidation.md) |
| money, estimates, reservations, settlement, budget exhaustion, concurrency limits | [`cost-and-budget.md`](references/cost-and-budget.md) |
| audio profile, mix plan, measurement, technical QC, QC records, synthetic test signals, listening review, fuzzing the decoders | [`audio-mix-and-qc.md`](references/audio-mix-and-qc.md) |
| the fault harness, named crash points, scripted providers, real PostgreSQL, interleavings, the scenario catalog and invariant checker | [`fault-injection-testing.md`](references/fault-injection-testing.md) |

## Compose with

- [`cantos-engineering`](../cantos-engineering/SKILL.md) for the order and report, with
  [`functional-core.md`](../cantos-engineering/references/functional-core.md) (decisions as data),
  [`immutability.md`](../cantos-engineering/references/immutability.md) (append-only attempts and
  artifacts), [`types-as-proofs.md`](../cantos-engineering/references/types-as-proofs.md) (frozen
  inputs, tokens, money), [`persistence.md`](../cantos-engineering/references/persistence.md)
  (transactions, constraints, test databases),
  [`boundary-hardening.md`](../cantos-engineering/references/boundary-hardening.md) (rows and
  provider payloads) and [`http-api-boundary.md`](../cantos-engineering/references/http-api-boundary.md)
  (Studio commands with operation IDs).
- [`cantos-script-ir`](../cantos-script-ir/SKILL.md) for the pinned revision and the per-line
  `SpokenContent` this skill fingerprints.
- [`cantos-publication`](../cantos-publication/SKILL.md), which consumes accepted artifacts and QC
  results and owns rights evaluation, approvals and release; the outbox events there ride on the
  mechanism defined here.
- [`cantos-listening`](../cantos-listening/SKILL.md), whose requests must never create a job,
  attempt or reservation.
- [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) and
  [`cantos-ui-design`](../cantos-ui-design/SKILL.md) for Studio casting, progress and cost
  screens, which render typed states and never decide them.
- Reviewing a production change: [`cantos-code-review`](../cantos-code-review/SKILL.md), whose
  production-durability pass applies this skill's rules.
