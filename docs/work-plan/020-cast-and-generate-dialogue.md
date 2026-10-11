# Cast characters and generate dialogue durably

## Why

Audio drama needs consistent character voices and predictable production costs. A provider timeout or worker crash must not lose progress, generate uncontrolled duplicate work, or replace audio for the wrong script version.

## Scope

Add Studio casting and freeze the selected voice, versioned performance/pronunciation settings,
model and supported controls against one accepted script revision. Production authorization
binds that exact input scope, applicable rights records and budget policy; draft edits cannot
change an existing snapshot. A frozen candidate and eligibility for billable work are separate
facts. The [production input contract](../../contracts/production-v1.md) and
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

For the commercial path the owner selected Google Cloud TTS with mocked evidence first. Following
the eSpeak pattern, the casting catalog now offers its dated vi-VN voices; a Google binding freezes
only with an explicit `neutral` delivery at intensity 0, and frozen lines pin the Google adapter
and 24 kHz output contract. `probe-google-tts` plans or, only with `--live`, a cents cap and an
operator-held token, sends exactly one request for one frozen line, recording receipt-v1 and
private observation files. The [adapter evidence](../evidence/google-tts-adapter.md) and proposed
[ADR 0009](../decisions/0009-google-cloud-tts-adapter.md) separate these mocked checks from live
synthesis, which is NOT_RUN. #6 still needs an operator-paid live sample with recorded envelopes;
Studio cannot yet enter the explicit intensity 0 for capabilities without intensity (eSpeak and
Google alike).

Later slices use PostgreSQL-backed background job states, bounded retries, recovery after
interruption, cancellation and per-dialogue result caching. Reuse results only when all
synthesis-affecting inputs match. Account for production cost and make uncertain provider
outcomes visible. A reference capability fixture is not an available voice or provider.

Regenerate only changed dialogue or dialogue affected by casting/pronunciation changes. Record affected scenes for the subsequent mixing stage while preserving successful unchanged clips and previous versions.

## Non-goals

Final episode mastering/publication, a distributed job platform, automatic provider marketplaces, voice cloning, or automated expressive-quality scoring.

## Dependencies

[010 — Import and edit a versioned script](010-import-and-edit-script.md). Choose concrete job, provider, and cache structures after inspecting the persisted Script IR and immediate Studio needs. Verify actual provider terms, supported languages, configuration, and availability before selecting defaults.

## Suggested sequence

1. Establish casting and generation inputs for one validated episode version.
2. Integrate explicitly authorized provider adapters with useful errors and cost records. The
   bounded local eSpeak sample path is implemented; choose the commercial provider, applicable
   terms/rights and test budget before any cloud/paid call. A neural local-model path is not required.
3. Implement durable execution, restart recovery, and stable result association.
4. Add casting controls, queue progress, clip preview, and partial regeneration to Studio.

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
or duplicate-result association is a correctness issue required before mixing. Continue the
unfinished #6 commercial and production criteria after their explicit decisions; preserve the
local probe's sample-only boundary and historical reference snapshots. The current requested
execution stops at #6 and does not start #7's durable job slice.

## GitHub execution

[#5](https://github.com/loveoverflowcom/cantos/issues/5), [#6](https://github.com/loveoverflowcom/cantos/issues/6), [#7](https://github.com/loveoverflowcom/cantos/issues/7), [#8](https://github.com/loveoverflowcom/cantos/issues/8). See the [issue/dependency map](project-planning.md) and [Kanban](https://github.com/users/loveoverflowcom/projects/6/views/1) for the recommended sequence and live execution state.
