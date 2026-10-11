# ADR 0011: Google Cloud Text-to-Speech as the first commercial synthesis adapter

- Status: proposed
- Date: 2026-10-11
- Decision owner: Cantos maintainers; provider selection made by the repository owner on 2026-10-11
- Related issue or PR: [#6](https://github.com/loveoverflowcom/cantos/issues/6)
- Supersedes / superseded by: N/A; extends [ADR 0007](0007-casting-production-inputs.md) and
  follows the provider pattern of [ADR 0008](0008-local-tts-inspection.md)

## Context

[#5](https://github.com/loveoverflowcom/cantos/issues/5) freezes casting, rights and budget inputs.
[ADR 0008](0008-local-tts-inspection.md) added the local eSpeak NG path and the shared provider
pattern: a capability in the casting catalog, provider-specific resolution in `production.rs`,
an exact adapter/output contract on each frozen line, a probe that consumes the frozen snapshot,
and the offline `cantos-tts-receipt-1` audit. #6 still needs a commercial path.

The owner selected Google Cloud Text-to-Speech and asked for mocked evidence only: no
credential, paid call or generated audio. The first slice built the adapter as a standalone
`synthesis` module with its own vocabulary and a line-file sample command. That duplicated the
shared receipt vocabulary and could not reach a frozen line, so the second slice moved it onto
the shared pattern before any consumer existed. No record from the first version
(`google-cloud-tts-synthesize-1`) can exist in a snapshot: Google was never in the catalog.

The following facts were read from Google's published documentation on 2026-10-11. They are
assertions about that documentation, not about an account or a live response:

| Fact | Source |
| --- | --- |
| `POST https://texttospeech.googleapis.com/v1/text:synthesize` is synchronous and returns base64 `audioContent`; LINEAR16 output includes the WAV header | [REST reference](https://docs.cloud.google.com/text-to-speech/docs/reference/rest/v1/text/synthesize) |
| `speakingRate` is [0.25, 2.0]; `pitch` is [-20.0, 20.0] semitones; an unsupported sample rate fails with `INVALID_ARGUMENT` | [AudioConfig](https://docs.cloud.google.com/text-to-speech/docs/reference/rest/v1/AudioConfig) |
| 40 vi-VN voices: 30 Chirp3-HD, 2 Neural2, 4 Standard, 4 WaveNet. Chirp 3: HD supports neither SSML nor the speaking-rate or pitch parameters | [Voices](https://docs.cloud.google.com/text-to-speech/docs/list-voices-and-types) |
| 5,000 total bytes per request, which cannot be increased; Chirp3 200 and Neural2 1,000 requests per minute per project | [Quotas](https://docs.cloud.google.com/text-to-speech/quotas) |
| US$30 (Chirp 3: HD), US$16 (Neural2), US$4 (WaveNet, Standard) per million characters after the free allowance; every character of the input, including spaces and newlines, is billed | [Pricing](https://cloud.google.com/text-to-speech/pricing) |
| Errors use `{"error":{"code","message","status","details"}}` with an HTTP status; `UNAVAILABLE` is retryable only for calls that repeat without unintended effects, `RESOURCE_EXHAUSTED` usually signals exhausted quota | [AIP-193](https://google.aip.dev/193), [AIP-194](https://google.aip.dev/194) |
| The TTS authentication guide documents OAuth access tokens (`gcloud auth print-access-token`) and shows `x-goog-user-project`; it does not mention API keys | [Authentication](https://docs.cloud.google.com/text-to-speech/docs/authentication) |

No source documents a request identifier, idempotency key, request lookup, model revision or
whether a rejected or failed request is billed.

## Decision

Implement Google inside the existing `tts` boundary of the `cantos-server` package:
`tts::google_contract` (pure identity and capabilities, used by casting and the audit),
`tts::google` (pure planning, classification and audio admission) and `tts::google_http`
(the one-request HTTPS shell). No new service, crate, table, migration or route.

1. **Capabilities are a dated declaration in the casting catalog.** One capability per voice
   family: `google-chirp3-hd` (30 voices), `google-neural2` (2), `google-wavenet` (4) and
   `google-standard` (4), provider `google-cloud-tts`, languages `vi` and `vi-VN`, emotion
   `neutral` only, no intensity, documented rate 250–2,000‰ and pitch ±20 semitones (Chirp3-HD:
   exactly 1,000‰ and 0), respelling pronunciation, `reference_only: false` and
   `billable_dispatch_available: false`. Google selects the model by voice name, so the
   family names are Cantos' model IDs. The catalog version is unchanged: the reference and
   eSpeak entries keep their meaning, so existing snapshots still re-admit.
2. **Casting requires an explicit neutral delivery.** Like eSpeak, a Google binding resolves
   only when each line's effective delivery is `neutral` at intensity 0; inherited Script IR
   emotion fails with `unsupported_control`. The creator records the override in versioned
   settings, so it is frozen, digested and reviewable. Nothing maps or drops an emotion
   implicitly. Resolved text must be nonblank NFC without control or line/paragraph separators
   and at most 5,000 UTF-8 bytes (`invalid_google_text`, `google_text_too_large`). Lines are
   never split.
3. **Frozen lines carry the exact adapter.** Resolution writes `google-cloud-tts-synthesize-2`
   and `wav-pcm-s16le-mono-24000-v1`. `tts::google::plan` accepts only that identity and
   re-checks every capability, returning every finding; a line frozen for another adapter or
   adapter version is refused rather than reinterpreted. `vi` scripts are sent as Google's only
   Vietnamese locale, `vi-VN`.
4. **The request is exact.** LINEAR16 at 24 kHz; `speakingRate` and `pitch` are sent only for
   families that support them. Keys are ordered, so equal lines give byte-identical bodies.
   `request_digest` is SHA-256 over a domain label, the adapter version and the body. It
   identifies the bytes sent, not a speech-cache fingerprint (#7).
5. **Outcomes use the shared receipt vocabulary.** Connect (TCP/TLS) failure is
   `timeout`/`not_sent`: receipt v1 has no network class, and its not-sent advice (a bounded
   retry needing fresh authorization) is the right one. Once a connection opened, Google
   documents no non-execution guarantee, so every outcome is `may_have_executed`: a lost
   response after a timeout or 504 is `timeout`, 429 is `rate_limit`, a 2xx whose body is
   oversized, cut off, not JSON, missing or empty `audioContent`, not base64 or refused by the
   audio profile is `malformed_audio`, and every other status (including 503) or reset is
   `permanent`. The adapter keeps the exact HTTP status, canonical `error.status` name,
   lost-response cause or audio defect in a private observation; provider messages, which can
   echo text, are discarded. The shared planner then advises manual reconciliation for every
   `may_have_executed` attempt and never a resend.
6. **Audio is admitted by the shared inspector.** A new `cantos-google-tts-pcm-1` profile
   requires mono signed 16-bit PCM (format tag 1) at 24 kHz, nonempty complete frames, at most
   120 seconds and 16 MiB; the response is bounded at 24 MiB. The receipt audit selects this
   profile only for the exact Google identity and output contract.
7. **Usage and money stay distinct.** Google's response reports no usage. A successful receipt
   reports `effective-unicode-scalars` usage and `charge: unknown`. The list-price estimate in
   US cents, rounded up per request and before any free allowance, lives in the private
   observation and is never a reported charge or settlement.
8. **Credentials and transport.** An OAuth access token in a redacting newtype, sent as a
   sensitive `Authorization: Bearer` header, plus an optional validated `x-goog-user-project`.
   API keys are not accepted: the TTS documentation does not establish them, and a key would
   never go in a URL. `reqwest 0.12.15` (rustls with ring, webpki roots, HTTP/1.1, no default
   features) with a 10 s connect timeout, a 60 s total timeout, no redirects and HTTPS only.
   `0.12.15` resolves against the workspace's existing pins; newer 0.12 releases require
   upgrading the pinned `tower-http`.
9. **Live use is an opt-in operator probe.** `probe-google-tts` consumes a frozen document and
   a dialogue ID, re-admits them through the existing production authority and sends nothing
   unless `--live` names a new private directory and a cents cap the estimate may not exceed.
   It writes create-only `intent.json` and `frozen-document.json` before sending, sends exactly
   once, never retries, and records `observation.json`, `receipt.json` and, on success,
   `output.wav` and `audit.json`. It is not the production dispatch path: no reservation,
   durable attempt, current approval check or accepted artifact exists yet (#7).

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Google Cloud TTS `text:synthesize` | 40 documented vi-VN voices, synchronous WAV the existing inspector can admit, per-character pricing | No emotion control, request ID, idempotency or lookup | Selected by the owner |
| Azure AI Speech, ElevenLabs, OpenAI TTS | Richer prosody or expressive controls | Different credential, billing and language-support models | Not selected; each needs its own ADR |
| Keep the standalone `synthesis` vocabulary and line-file command (first slice) | Finer HTTP failure classes | A second attempt/audio vocabulary beside receipt v1; cannot consume a frozen line | Replaced before any consumer |
| Map Script IR emotions to Google's default delivery implicitly | Every frozen line would plan | Silent performance substitution, forbidden by business rule 8 | Rejected; the creator chooses `neutral` explicitly |
| Add a network or `unavailable` class to receipt v1 | Exact receipt classes | A receipt contract version change for a label; advice is unchanged | Deferred; the exact status lives in the observation |
| Treat 429 and 5xx as safely retryable | Fewer manual resolutions | Google documents no non-execution or non-billing guarantee | Rejected until documentation or billing evidence shows otherwise |
| `hyper-util` + `hyper-rustls` directly | Fewer transitive crates | More connection, timeout and body code to own and test | Rejected; reqwest maps connect and timeout errors directly |

## Consequences

- Product and user impact: a creator can cast Google vi-VN voices and freeze them through the
  existing API and Studio. Studio offers "inherit" or the explicit `0` for capabilities without
  intensity, for eSpeak equally ([Studio casting evidence](../evidence/studio-provider-casting.md)).
- Compatibility and migrations: no migration. The production v1 JSON Schema now lists the
  eSpeak and Google identities, adapter versions and output contracts that the server already
  or newly emits; reference and eSpeak snapshots and digests are unchanged. `Cargo.lock` adds
  reqwest and its rustls/ring tree for `cantos-server`.
- Operations, storage and production cost: the opt-in probe is the only path that can spend
  money. It needs an operator-held access token, sends one request and never retries. Records
  and audio stay in a private directory outside Git.
- Implementation and recovery requirements: #7 must persist attempt intent and reserve budget in
  PostgreSQL before any send, require current approval, and surface every reconciliation for an
  explicit, recorded resolution. Any change to the request mapping, controls, output contract
  or admission requires a new `ADAPTER_VERSION`.
- Deferred questions: whether Google bills rejected or rate-limited requests; whether real
  responses always carry a plain PCM header the inspector admits; voice stability across
  unannounced model updates; API-key support; Studio entry of the explicit neutral delivery; a
  receipt class for network and unavailable failures.

## Validation and revisit conditions

[Evidence](../evidence/google-tts-adapter.md) records the executed mocked checks. Live synthesis
is NOT_RUN by owner decision. Run the opt-in probe under a cost cap before treating this adapter
as provider-live-tested, and record real envelopes as fixtures with their capture date.

Revisit when the live run disagrees with any declared fact; when Google publishes request IDs,
idempotency, a model revision or a billing rule for errors; when prices or the vi-VN voice list
change; or when a second commercial provider is added.
