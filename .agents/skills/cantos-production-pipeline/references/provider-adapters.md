# Provider adapters

> **Scope.** Put TTS and adaptation-AI providers behind provider-neutral ports: a typed
> capability model checked before any paid call, honest classification of the send boundary,
> append-only attempts kept separate from artifacts, reconciliation of ambiguous outcomes before
> any repeat, validation of returned audio, server-side credentials with redaction, clearly
> labeled fakes, recorded-fixture contract tests, cost-capped live tests and adapter versions.
> Use when adding or changing a provider, a voice/model mapping, emotion or pronunciation
> controls, timeouts, request IDs, idempotency keys, provider fixtures or a live provider test.

The product rules live in [business rules § Casting and performance](../../../../docs/product/business-rules.md#casting-and-performance)
(no silent substitution), [§ Rights and access](../../../../docs/product/business-rules.md#rights-and-access)
(credentials stay server-side), [architecture § Interfaces](../../../../docs/architecture/overview.md#interfaces)
(provider-neutral request and capability model) and [pipeline § Jobs and failure recovery](../../../../docs/product/production-pipeline.md#jobs-and-failure-recovery)
(timeouts may already be billed). [Work plan 020](../../../../docs/work-plan/020-cast-and-generate-dialogue.md#risks--unknowns)
records that idempotency, billing, language support and emotion controls differ per provider.
No provider is selected; every type below is a **proposal**, and a vendor SDK is a candidate
needing an [ADR](../../../../templates/adr.md) and a verified, pinned version.

## 1. Ports are named for the capability

```rust
// Illustrative and proposed. The application never names a vendor.
pub trait SpeechSynthesizer: Send + Sync {
    fn descriptor(&self) -> &ProviderDescriptor;

    async fn submit(&self, request: &SynthesisRequest) -> Result<SynthesisReceipt, AttemptFailure>;

    async fn lookup(&self, reference: &ProviderReference) -> Result<LookupOutcome, AttemptFailure>;
}

pub struct ProviderDescriptor {
    pub provider: ProviderId,
    pub adapter: AdapterVersion,
    pub capabilities: SynthesisCapabilities,
}

pub struct SynthesisRequest {
    pub operation: OperationId,
    pub idempotency_key: Option<IdempotencyKey>,
    pub text: SpeechText,              // from cantos-script-ir SpokenContent, NFC
    pub language: LanguageTag,
    pub voice: ProviderVoiceRef,
    pub model: ModelIdentity,
    pub controls: ResolvedControls,    // never the raw requested delivery
    pub output: OutputFormat,
}
```

- The domain and application import none of this module's vendor types; the adapter maps
  vendor payloads into `SynthesisReceipt`, `LookupOutcome` and `AttemptFailure`
  ([`decoupling.md`](../../cantos-engineering/references/decoupling.md)).
- `async fn` in a trait is not object-safe. Choose generics, a closed enum of configured adapters
  or boxed futures deliberately and record the choice; do not let the decision leak into the core.
- Local/open-source and commercial providers (020 scope) implement the same port. Their failure
  and cost models differ (out-of-memory and missing model files versus billing and rate limits),
  so each adapter owns its own mapping into the neutral classes.
- An adaptation drafter is a second port with the same attempt, reservation and reconciliation
  machinery. Its output is untrusted and is validated by
  [`import-and-adaptation.md`](../../cantos-script-ir/references/import-and-adaptation.md); this
  skill owns only the execution record and the cost.

## 2. Capabilities are data, checked before any paid call

```rust
pub struct SynthesisCapabilities {
    pub languages: BTreeSet<LanguageTag>,
    pub emotion: EmotionSupport,
    pub pronunciation: PronunciationSupport,
    pub pacing: PacingSupport,
    pub output_formats: BTreeSet<OutputFormat>,
    pub max_text_len: TextLimit,
    pub model_pinning: ModelPinning,
    pub reconciliation: ReconciliationSupport,
}

pub enum EmotionSupport {
    None,
    Labels(BTreeSet<ProviderEmotion>),
    LabelsWithIntensity { labels: BTreeSet<ProviderEmotion>, steps: u16 },
}

pub enum ReconciliationSupport {
    None,
    IdempotencyKey { retention: Duration },
    RequestLookup { not_found_is_definitive: bool },
}

pub fn resolve_controls(
    line: &LineDelivery,               // requested emotion, intensity (permille), pacing, overrides
    capabilities: &SynthesisCapabilities,
    mapping: &ControlMappingRevision,  // explicit fallbacks accepted during casting review
) -> Result<ResolvedControls, Vec<CapabilityFinding>>;
```

Capabilities come from the adapter's versioned declaration, checked against the provider's
current documentation — never inferred from one successful call. `resolve_controls` is pure and
runs during freezing, so an unsupported required control blocks the line before money moves.

Worked example from the sample episode: `dialogue-02` (An) says
`Ngày mai, mình có diễn tiếp không?` with `hopeful` at 600‰. A provider declaring
`Labels({neutral, happy, sad})` yields
`[EmotionUnsupported { dialogue: dialogue-02, requested: hopeful }, IntensityUnsupported { … }]`.
Only a reviewed `ControlMappingRevision` saying `hopeful → happy, intensity dropped` lets the line
proceed; that revision then enters the speech fingerprint and the production record shows the
degradation. Sending `neutral` because nothing matched is the defect the rule exists to prevent.

The same discipline covers pronunciation: if a provider has no phoneme support, a respelling
fallback for a name such as `Vọng Đài` changes the text sent to the provider. It is an explicit,
recorded fallback whose effective text is fingerprinted — never an adapter's quiet rewrite.
Splitting narration longer than `max_text_len` is likewise a deterministic, versioned adapter
transformation, never an ad-hoc truncation.

## 3. Classify the send boundary honestly

The question is not "did it fail?" but "could the provider have executed and billed it?"

| Observation | Class | Reason |
|---|---|---|
| DNS failure, connection refused, TLS handshake failure | `NotSent` | no request reached the provider |
| request rejected by local validation | `InvalidInput` | nothing was sent |
| HTTP 429, with or without `Retry-After` | `RateLimited` | only if the provider documents no execution; otherwise `Ambiguous` |
| HTTP 400/422 with a validation body | `InvalidInput` | fix the line or the casting |
| HTTP 401/403 | `Permanent` | credentials or account configuration; operator action |
| HTTP 5xx | `Transient` only when documented as not executed and not billed; else `Ambiguous` | — |
| timeout or reset after the request was written | `Ambiguous` | it may have run and charged |
| 2xx with empty, truncated or undecodable audio | `MalformedAudio` | cost probably incurred |
| asynchronous job accepted, then polling timed out | not a failure | persist the job ID; keep looking it up |

Configure a connect timeout separately from the request timeout so the adapter can tell
`NotSent` from `Ambiguous`; a single `timeout(…)` around the whole call cannot. Contract tests
with recorded fixtures (§ 7) pin this mapping per provider.

## 4. Attempts are append-only and separate from artifacts

| Record | Written | Holds | Never |
|---|---|---|---|
| attempt intent | before send, with its reservation, in one transaction | step, lease token, operation ID, idempotency key, provider, adapter and requested model, fingerprint | updated |
| attempt observation | after each thing learned | outcome kind, provider request/job ID, reported usage, artifact ID, observed model version | overwritten |
| attempt resolution | when a human resolves an uncertain attempt | actor, decision, time | inferred |
| artifact | after bytes are uploaded and checksum-verified | content checksum, format facts, fingerprint, provenance | created from an unverified upload |
| accepted pointer | once per step, under the fence | the one selected artifact | moved |

Cost attaches to attempts and QC attaches to artifacts. Several successful attempts can exist for
one step; one is accepted and the rest remain as unaccepted artifacts, reusable only through the
cache rules in [`fingerprints-and-invalidation.md`](fingerprints-and-invalidation.md). Store raw
provider output, normalized dialogue artifacts and later mixes as different artifact kinds,
honoring provider retention restrictions; key layout and private namespaces follow
[`storage-and-delivery.md`](../../cantos-publication/references/storage-and-delivery.md).

## 5. Reconcile before you repeat

An attempt intent without a definitive observation is uncertain by definition
([`durable-jobs.md`](durable-jobs.md#6-recovery-sweep)). The pure planner chooses the cheapest
honest way out:

```rust
pub fn plan_reconciliation(
    attempt: &UncertainAttempt,
    support: &ReconciliationSupport,
    now: Timestamp,
) -> ReconciliationPlan {
    match (support, &attempt.provider_reference) {
        // Retention is measured from the intent, which precedes the send: conservative.
        (ReconciliationSupport::IdempotencyKey { retention }, _)
            if now < attempt.intent_at + *retention =>
        {
            ReconciliationPlan::ResubmitWithStoredKey(attempt.idempotency_key.clone())
        }
        (ReconciliationSupport::RequestLookup { .. }, Some(reference)) => {
            ReconciliationPlan::Lookup(reference.clone())
        }
        _ => ReconciliationPlan::SurfaceForResolution,
    }
}
```

- `LookupOutcome::NotFound` proves non-execution only when the capability says
  `not_found_is_definitive`; otherwise the attempt stays uncertain.
- `SurfaceForResolution` pauses the affected work and shows Studio the attempt, its held
  reservation and the choices: wait for billing data, retry accepting a possible duplicate
  charge, or abandon the line. The choice is an appended resolution with its actor.
- Automatic repetition without one of these paths is the "blind retry" failure in the routing
  cases; "retry three times" is never an acceptable translation of a timeout.

## 6. Validate the bytes before acceptance

`probe_audio(bytes, expected: &OutputFormat) -> Result<AudioFacts, AudioDefect>` checks the
container, codec, sample rate, channels, non-zero duration and truncation before an artifact row
exists. A defect becomes `AttemptFailure::MalformedAudio`: retried within the bound, with its cost
still settled. Provider bytes are untrusted input, so the probe is a fuzzing target for
robustness (no panic, bounded allocation) seeded with synthetic headers built in code, never
with provider output ([`audio-mix-and-qc.md`](audio-mix-and-qc.md#8-decoders-are-fuzz-targets)).

## 7. Credentials, logs, fakes, fixtures and live tests

**Credentials and redaction.** Keys come from the server's secret store; the names in
[`.env.example`](../../../../.env.example) are proposals. A `ProviderSecret` newtype has a
redacting `Debug`, no `Display` and no `Serialize`. Log IDs and fingerprints, never dialogue text
— manuscripts are private — and sanitize provider error bodies before persisting them, because
they can echo input or keys. Oracle: run a scripted production with a sentinel key and a sentinel
line, capture logs and persisted observations, assert neither sentinel appears.

**Fakes.** `ScriptedSynthesizer` lives in test support, is named as a fake, and cannot be
selected by production configuration (the provider registry rejects it). It keeps its own ledger
of provider-side executions, honors idempotency keys and lookups as configured, and can execute a
request and then lose the response. Evidence from it is `example-tested` or `fault-injected`,
never `provider-live-tested`; the [server guide](../../../../services/cantos-api/README.md) already says
fakes are never presented as production support.

**Recorded fixtures.** Capture real envelopes once, during a cost-capped live session using
original sample text such as `Ánh đèn cuối cùng còn sáng trên sân khấu.`: success, rate limit,
validation error, auth error, server error, asynchronous job lifecycle and a malformed response.
Strip auth headers. Do not commit provider audio: replace each payload with a marker the test
fills with synthetic audio generated in code. Record provider, model, adapter version and capture
date beside the fixture, and never regenerate a fixture from the adapter's own output — that turns
an external oracle into a round trip. Evidence: `differentially-tested` against recorded data.

**Live tests.** Opt-in only (an ignored test plus an explicit flag, proposed), credentials from
the secret store, a hard cost cap enforced through the same reservation path production uses,
the sample scene as content, and a record of provider, model, date, request IDs and cost. The
label is `provider-live-tested`; it never runs in default CI and its audio is never committed.
Report it apart from mocked failure cases, as 020's acceptance criteria require.

## 8. Adapter versions are inputs

`AdapterVersion` is a constant in the adapter module and part of every speech fingerprint. Bump
it for any change to request mapping, text chunking, control mapping or post-processing. A golden
request-mapping test turns `dialogue-02`'s `SynthesisRequest` into the expected vendor request; the
golden file name carries the adapter version, so changing the mapping without a bump is visible
in review. Record the provider-reported model version on each observation; a change while the
model is unpinned raises a drift finding that suggests an explicit refresh.

## 9. Rule cards

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| Required unsupported control is a blocking finding | silent voice or emotion substitution | `EmotionUnsupported { dialogue-02, hopeful }` | adapter maps unknown emotions to `neutral` | exact-variant test per control | proposed · an explicit, recorded, fingerprinted mapping |
| Persist intent and key before send | crash after send leaves an unrecorded paid call | intent + reservation transaction, then send | send, then insert on success | crash point `dispatch.after_send_before_outcome` ([catalog](fault-injection-testing.md#8-the-scenario-catalog)) | proposed · none |
| Ambiguous outcomes reconcile before any repeat | duplicate billing and duplicate output | `plan_reconciliation` | `for _ in 0..3 { submit() }` | fake ledger shows one execution | proposed · provider-documented definitive non-execution |
| Attempts and artifacts are different records | failed or stale calls masquerade as results | intent, observations, artifact, pointer | `tts_job.audio_url` updated per retry | schema review; one-accepted invariant | proposed · none |
| Credentials server-side, redacted | leaked keys in logs or fixtures | `ProviderSecret` with redacting `Debug` | `tracing::debug!(?request)` with headers | sentinel scan | proposed · none |
| Fakes are labeled and unreachable from production | a demo reported as provider support | registry rejects fake IDs | `TTS_PROVIDER=fake` accepted in production | configuration test | proposed · none |
| Fixtures come from the provider, never the adapter | the oracle shares the adapter's bug | captured envelope with date | fixture regenerated from adapter output | review; fixture README | manual · none |

## 10. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| capability resolution and findings | table tests with exact variants | `example-tested` |
| vendor error mapping | recorded envelopes per class | `differentially-tested` |
| no duplicate paid call after an ambiguous timeout | scripted fake that executes then drops the response | `fault-injected` |
| probe robustness | fuzz target with resource bounds | `fuzz-tested` (robustness only; no correctness claim) |
| a real provider synthesizes playable Vietnamese audio | cost-capped live run | `provider-live-tested` |
| the returned audio sounds right | a named reviewer | `audio-listened` |
