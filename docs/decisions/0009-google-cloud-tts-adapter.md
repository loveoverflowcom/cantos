# ADR 0009: Google Cloud Text-to-Speech as the first commercial synthesis adapter

- Status: proposed
- Date: 2026-10-11
- Decision owner: Cantos maintainers; provider selection made by the repository owner on 2026-10-11
- Related issue or PR: [#6](https://github.com/loveoverflowcom/cantos/issues/6)
- Supersedes / superseded by: N/A; extends [ADR 0007](0007-casting-production-inputs.md)

## Context

[#5](https://github.com/loveoverflowcom/cantos/issues/5) freezes casting, rights and budget inputs
against a reference-only catalog that can never dispatch. #6 needs one concrete provider path
with accountable capabilities, failure classes and costs. The work plan supersedes the former
local/open-source generation requirement; no local inference is installed or run.

The owner selected Google Cloud Text-to-Speech and asked for mocked evidence only in this slice:
no credential, paid call or generated audio. The following facts were read from Google's
published documentation on 2026-10-11 and are assertions about that documentation, not about an
account or a live response:

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

Add one adapter in the existing `cantos-server` package: a pure core
(`synthesis::google`) and a one-request HTTPS shell (`synthesis::google_http`). No new service,
crate, table, migration, route or wire contract.

1. **Capabilities are a dated declaration.** `VI_VN_VOICES` lists the 40 documented vi-VN voices
   and their family. Cantos names each family's model (`standard`, `wavenet`, `neural2`,
   `chirp3-hd`) because Google selects the model by voice name. `plan_line` returns every
   blocking finding before anything is sent: language other than `vi-VN`, unknown voice, model
   that does not match the voice, rate outside 250–2,000‰ or pitch outside ±20 semitones
   (Chirp3-HD: exactly 1,000‰ and 0), empty, non-NFC or over-5,000-byte text, and any emotion or
   intensity. Google offers no emotion control, so a requested emotion — including `neutral` —
   is refused rather than silently dropped. Lines are never split.
2. **Frozen lines are not reinterpreted.** `plan_resolved` refuses a `ResolvedSynthesisInput`
   whose provider, adapter version or output contract is not this adapter's
   (`google-cloud-tts`, `google-cloud-tts-synthesize-1`,
   `audio/wav;codec=pcm_s16le;rate=24000;channels=1`). Every line frozen by #5 today carries the
   reference catalog and its Script IR delivery emotion, so none can reach Google until casting
   adds Google capabilities and an explicit, reviewed control mapping.
3. **The request is exact.** LINEAR16 at 24 kHz; `speakingRate` and `pitch` are sent only for
   families that support them. Keys are ordered, so equal lines give byte-identical bodies.
   `request_digest` is SHA-256 over a domain label, the adapter version and the body. It
   identifies the bytes sent, not a speech-cache fingerprint (#7).
4. **Send-boundary classification is conservative.** Connect (TCP/TLS) failure is `not_sent`.
   A timeout or reset after connecting is `ambiguous`. 400 → `invalid_input`; 401/403 →
   `account_rejected`; 429 → `rate_limited`; 500/502/503/504 → `provider_error`; any other non-2xx
   → `unexpected_response`; a 2xx whose body is oversized, cut off, not JSON, missing or empty
   `audioContent`, not base64, not a valid WAV, not mono 16-bit integer PCM at 24 kHz, or has no
   frames → `malformed_audio`. Only the canonical `error.status` name is kept; the provider
   message can echo private text and is discarded.
5. **Charge exposure and retry follow from documentation, not hope.** Without a documented
   non-execution guarantee, idempotency key or lookup, every class that reached Google has
   `charge: unknown`, and no class with an unknown charge may repeat automatically:
   `ambiguous`, `rate_limited`, `provider_error` and `unexpected_response` are
   `resolve_before_retry`; `invalid_input` and `account_rejected` are `do_not_retry`. `not_sent`
   (`charge: none`) and `malformed_audio` (`charge: usage_observed`, a known extra charge rather
   than an unknown duplicate) are `retry_allowed`, still bounded by the caller's attempt limit
   and budget. Every class names a recovery action.
6. **Usage and money stay distinct.** Google's response reports no usage. The attempt record
   carries the counted billable characters (Unicode scalar values of NFC text) and a list-price
   estimate in US cents, rounded up per request and before any free allowance. `usage_observed`
   means Google executed the request; no record is a settled charge, which requires billing
   data.
7. **Credentials and transport.** An OAuth access token in a redacting newtype, sent as a
   sensitive `Authorization: Bearer` header, plus an optional validated `x-goog-user-project`.
   API keys are not accepted, because the TTS documentation does not establish them, and a key
   would never go in a URL. `reqwest 0.12.15` (rustls with ring, webpki roots, HTTP/1.1, no
   default features) with a 10 s connect timeout, a 60 s total timeout, no redirects,
   HTTPS-only and a 32 MiB response bound. `0.12.15` resolves against the workspace's existing
   pins; newer 0.12 releases require upgrading the pinned `tower-http`.
8. **Live use is an opt-in operator command.** `google-tts-sample` plans a line from a file and
   sends nothing unless `--live` is given together with a new output path and a cents cap that
   the list-price estimate may not exceed. It sends exactly once, never retries, writes the WAV
   only after validation and prints the attempt record. It is not the production dispatch path:
   no PostgreSQL reservation, attempt intent or approval gate exists yet (#7).

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Google Cloud TTS `text:synthesize` | 40 documented vi-VN voices, synchronous WAV that the existing inspector can probe, per-character pricing | No emotion control, request ID, idempotency or lookup | Selected by the owner |
| Azure AI Speech, ElevenLabs, OpenAI TTS | Richer prosody or expressive controls | Different credential, billing and language-support models | Not selected for the first adapter; each needs its own ADR |
| Add Google voices to the #5 casting catalog now | Studio could cast Google voices | Requires an explicit emotion/intensity mapping in versioned settings, contract, schema and Studio changes | Deferred to a follow-up slice |
| Drop unsupported emotion and send neutral delivery | Every frozen line would plan | Silent performance substitution, forbidden by business rule 8 | Rejected |
| Treat 429 and 5xx as safely retryable | Fewer manual resolutions | Google documents no non-execution or non-billing guarantee | Rejected until documentation or billing evidence shows otherwise |
| `hyper-util` + `hyper-rustls` directly | Fewer transitive crates | More connection, timeout and body code to own and test | Rejected; reqwest maps connect and timeout errors directly |
| Generic multi-provider `SpeechSynthesizer` trait | Future providers plug in | Speculative without a second provider | Rejected; the port sits at the HTTP transport, with a test fake as its second implementation |

## Consequences

- Product and user impact: none visible yet. Studio cannot select Google voices and no frozen
  production line can be dispatched.
- Compatibility and migrations: no schema, migration or wire change. `Cargo.lock` adds reqwest
  and its rustls/ring dependency tree for `cantos-server`; existing pins are unchanged.
- Operations, storage and production cost: the opt-in command is the only path that can spend
  money. It needs an operator-held access token, sends one request and never retries. No audio
  is stored by Cantos; the operator chooses an output path outside Git (`*.wav` is ignored).
- Implementation and recovery requirements: #7 must persist attempt intent and reserve budget
  in PostgreSQL before any send, and surface every `resolve_before_retry` attempt for an
  explicit, recorded resolution. Changing the request mapping, controls, output contract or
  validation requires a new `ADAPTER_VERSION`.
- Deferred questions: whether Google bills rejected or rate-limited requests; whether real
  responses always carry a plain PCM header (format tag 1) that the inspector accepts; voice
  stability across unannounced model updates; whether API keys are supported; casting
  integration and the reviewed emotion mapping; a local/open-source path if it is ever required
  again.

## Validation and revisit conditions

[Evidence](../evidence/google-tts-adapter.md) records the executed mocked checks. Live synthesis
is NOT_RUN by owner decision. Run the opt-in command under a cost cap before treating this
adapter as provider-live-tested, and record real envelopes as fixtures with their capture date.

Revisit when the live run disagrees with any declared fact; when Google publishes request IDs,
idempotency, a model revision or a billing rule for errors; when prices or the vi-VN voice list
change; or when a second provider is added.
