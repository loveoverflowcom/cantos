# Cast characters and generate dialogue durably

## Why

Audio drama needs consistent character voices and predictable production costs. A provider timeout or worker crash must not lose progress, generate uncontrolled duplicate work, or replace audio for the wrong script version.

## Scope

Add Studio casting and freeze the selected voice, versioned performance/pronunciation settings,
model and supported controls against one accepted script revision. Production authorization
binds that exact input scope, applicable rights records and budget policy; draft edits cannot
change an existing snapshot. A frozen candidate and eligibility for billable work are separate
facts. The [production input contract](../../packages/contracts/production-v1.md) and
[casting evidence](../evidence/casting-production-inputs.md) describe the implemented and
locally verified #5 slice, including real PostgreSQL/HTTP recovery and observed Studio
freeze/approval/stale-input behavior. GitHub owns acceptance and execution status.

The user controls Gemini / ChatGPT / Codex and requests tool calls; Cantos owns the data pipeline.
The host-owned adaptation choice superseded the former mandatory local-model prerequisite.
#5 supplies only settings, rights, budget and immutable authorization inputs: no TTS call,
audio preview, reservation, charge, worker or provider credential is implied by its acceptance.

For the current #6 slice, the user subsequently authorized local installation and TTS. The
implemented eSpeak NG 1.52.0 probe generated and validated actual Vietnamese samples for `vi`,
`vi-vn-x-central` and `vi-vn-x-south`, preserving native 22,050 Hz mono signed 16-bit PCM.
The adapter admits explicit neutral/default controls and rejects unsupported intent; it saves
private frozen-input, intent, receipt and output-hash records. This is sample-only execution,
with no current production authorization, durable jobs, reservation or asset acceptance.
Listening quality is **NOT_RUN**. [ADR 0008](../decisions/0008-local-tts-inspection.md) records
the resolved local selection; [execution evidence](../evidence/tts-receipt-inspection.md)
separates actual synthesis from synthetic tests. Commercial selection, terms, applicable rights,
authentication boundary, spending cap and live sample remain deferred. This is part of #6 and
does not satisfy its complete acceptance criteria.

The user's additional local-neural request is bounded by
[ADR 0010](../decisions/0010-local-neural-tts-research.md). The
[candidate comparison](../evidence/local-neural-tts-research.md) separates engine licenses,
weights/preset grants and training-data provenance. The concrete VieNeu Nano adapter is
research-only: production is refused at invocation regardless of a frozen planning approval.
[Neural evidence](../evidence/local-neural-tts.md) records the exact profile, original fixtures,
resource coordination, actual versus mocked results and remaining listening/rights gaps.
Local installation and all six original fixture samples are now verified across the admitted
Đức Trí and Mỹ Duyên presets, with native 24,000 Hz mono 16-bit PCM and checked hashes/receipts.
The cold CLI runs took 6.38–7.32 seconds for 7.66–18.84 seconds of audio, with real-time factors
of 0.389–0.838. Listening remains **NOT_RUN**, and production eligibility remains **NO_GO**
pending training/voice provenance. Actual sample generation does not establish pronunciation,
naturalness or emotion quality.
This adds no commercial authorization, durable queue, retries, clip cache or asset acceptance.

For the commercial path the owner selected Google Cloud TTS with mocked evidence first. Following
the eSpeak pattern, the casting catalog now offers its dated vi-VN voices; a Google binding freezes
only with an explicit `neutral` delivery at intensity 0, and frozen lines pin the Google adapter
and 24 kHz output contract. `probe-google-tts` plans or, only with `--live`, a cents cap and an
operator-held token, sends exactly one request for one frozen line, recording receipt-v1 and
private observation files. The [adapter evidence](../evidence/google-tts-adapter.md) and proposed
[ADR 0011](../decisions/0011-google-cloud-tts-adapter.md) separate these mocked checks from live
synthesis, which is NOT_RUN. #6 still needs an operator-paid live sample with recorded envelopes.
Studio casting now follows each selected provider/model profile: for eSpeak and Google it offers
"inherit" or an explicit intensity 0, a provider switch changes only the provider and model, and
kept values the profile cannot honor are flagged rather than replaced
([Studio casting evidence](../evidence/studio-provider-casting.md)).

The subsequent requester-authorized [durable local/fake foundation](../evidence/durable-tts-jobs.md)
starts from merged PR #38 and implements PostgreSQL jobs/steps/attempts/reservations, a separate
worker in the same backend package, leases/fences, cancellation/pause and conservative restart
recovery. It admits only exact approved eSpeak inputs or explicit synthetic execution. Google
commercial dispatch remains disabled and VieNeu remains research-only NO_GO. The
[development contract](../../packages/contracts/durable-jobs-v1.md) binds result selection to
the frozen revision, dialogue, input fingerprint and existing receipt audit; completion grants
no publication/asset approval. Issue #7 remains open until all its criteria are met.

Later slices add cross-run per-dialogue result caching. Reuse results only when all
synthesis-affecting inputs match. Account for production cost and make uncertain provider
outcomes visible. A reference capability fixture is not an available voice or provider.

Regenerate only changed dialogue or dialogue affected by casting/pronunciation changes. Record affected scenes for the subsequent mixing stage while preserving successful unchanged clips and previous versions.

## Non-goals

Final episode mastering/publication, a distributed job platform, automatic provider marketplaces, voice cloning, or automated expressive-quality scoring.

## Dependencies

[010 — Import and edit a versioned script](010-import-and-edit-script.md). Choose concrete job, provider, and cache structures after inspecting the persisted Script IR and immediate Studio needs. Verify actual provider terms, supported languages, configuration, and availability before selecting defaults.

## Suggested sequence

1. Establish casting and generation inputs for one validated episode version.
2. Assess the completed local samples with a named listener and resolve the neural model's
   training/voice provenance. Preserve the implemented eSpeak probe and the VieNeu Nano
   research-only dispatch boundary; installation and sample generation are no longer pending.
   Use the [review fixes and fresh gates](../evidence/local-neural-tts-review.md) and
   [ten-sample listening handoff](../evidence/local-neural-tts-listening.md), including both
   voices' <5-word questions/exclamations whose pinned normalization ends in a period.
   Resolve listening and exact-pin rights before enabling any production connection;
   a planning approval or a valid WAV cannot clear either dependency.
3. Complete #6's separate commercial provider selection, applicable terms/rights, authentication
   boundary and test budget before an explicitly authorized cloud/paid sample. Keep #6 open
   until its complete criteria are met or the user explicitly approves a scope change.
4. Use the requester-authorized [#7 local/fake foundation](../evidence/durable-tts-jobs.md),
   following merged [provider verification PR #38](../evidence/tts-provider-verification.md).
   Complete its remaining production-adoption/reconciliation/storage criteria only within
   separately authorized work; preserve open #6 live/rights/listening gates before production use.
5. Implement #8 scoped cache/reuse and partial regeneration, then add Studio queue progress and
   clip preview. Keep each boundary separately reviewable; mixing/storage/CDN/publication and
   UI/mobile are outside the worker foundation.

## Acceptance criteria

- A character retains the selected voice and pronunciation across dialogue; deliberately changed settings produce new generation inputs.
- Each selected, explicitly authorized provider path generates playable audio for a real sample
  in its owning adapter slice. Demonstrate mocked failure cases separately from live synthesis
  evidence; #5 reference capabilities do not satisfy this later gate.
- Repeating a completed identical request reuses audio; changing one dialogue reuses other valid clips and marks the affected scene for remixing.
- Worker restart recovers incomplete jobs. Timeout, rate limit, malformed audio, permanent failure, and cancellation produce bounded, understandable states.
- Concurrent requests cannot incorrectly attach a result to another dialogue or revision. Recovered workers do not overwrite a newer successful result.
- An ambiguous commercial-provider timeout does not automatically trigger unlimited rebilling; retry policy and any provider idempotency limitations are explicit.
- A configured production budget prevents unauthorized new billable work; usage, estimates, and uncertain charges remain distinguishable.
- The creator can fix a failed input/provider choice and resume the remaining work without regenerating successful unaffected clips.

## Review boundary

For #5, demonstrate accepted revision → versioned casting/settings → rights/budget findings →
immutable frozen candidate → exact owner approval → reopened snapshot and current eligibility.
Verify unsupported controls, every scoped-change invalidation, rights expiry/revocation, actor
isolation, concurrent freeze, exact retry and restart recovery on real PostgreSQL. Rights claims
are recorded assertions, not legal verification; missing/pending claims block when required.
Unknown cost is neither zero nor unlimited. Estimates, reservations and actual charges are
distinct; only a provider receipt can establish actual billing.

The complete 020 boundary later demonstrates casting → generation → clip preview → single-line
edit → partial regeneration. Verify real provider adapters, persistent states, cost visibility,
and crash/retry behavior in those owning slices. Review the narrow boundary needed by generation
rather than speculative provider abstractions. Product approval never authorizes an agent to
make a real payment or provider call.

## Risks / unknowns

Provider idempotency, billing, language support and voice stability differ. Reference settings
provide no evidence of provider availability, voice identity or audio quality. Emotion hints
may not map directly across providers; expose actual supported controls and preserve their
versioned meaning. Real-person voices/cloning require specific recorded consent and rights.

## Follow-ups

Defer provider auto-routing, expanded catalogs and batching for throughput. A provider mismatch
or duplicate-result association is a correctness issue required before mixing. Prioritize actual
listening assessment and training/voice provenance for the completed local neural samples, then
continue the unfinished #6 commercial and production criteria after their explicit decisions.
Preserve the local probes' sample-only boundaries and historical reference/Google snapshots.
The requester-authorized sequence produced the bounded #7 foundation only after verification
PR #38 merged. The [verification handoff](../evidence/tts-provider-verification.md) and
[worker evidence](../evidence/durable-tts-jobs.md) preserve the remaining commercial, rights,
listening, full reuse and production-adoption requirements. Do not close #7 from its foundation
alone or continue those follow-ups without the owning authorization.

## GitHub execution

[#5](https://github.com/loveoverflowcom/cantos/issues/5), [#6](https://github.com/loveoverflowcom/cantos/issues/6), [#7](https://github.com/loveoverflowcom/cantos/issues/7), [#8](https://github.com/loveoverflowcom/cantos/issues/8). See the [issue/dependency map](project-planning.md) and [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for the recommended sequence and live execution state.
