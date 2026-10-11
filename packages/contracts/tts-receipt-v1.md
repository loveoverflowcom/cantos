# Local TTS sample and offline receipt inspection v1

Status: implemented local sample probe and inspection contract, not a production synthesis or
audio-ingestion API.
Rust decoding and [tests](../../services/cantos-api/tests/tts_receipt.rs) own this experimental file shape;
no HTTP route, shared DTO, database receipt or JSON Schema is added. The user selected local
TTS for [#6](https://github.com/loveoverflowcom/cantos/issues/6). The concrete eSpeak NG 1.52.0
probe retains private file records; commercial selection/terms/budget and its live sample are
deferred. Real synthesis and byte validation ran for all three local Vietnamese voices;
listening quality is **NOT_RUN**. This completes only part of #6.

## Input authority

```sh
cargo run --locked --bin inspect-tts-receipt -- frozen-document.json receipt.json [output.wav]
```

The first input is the `document` member of an existing [production snapshot](production-v1.md).
Reconstruction through `prepare_inputs` must equal the complete export, resolved controls,
rights versions, budget and production-p1 digest. A recomputed hash alone cannot admit forged
resolved text. Receipt scope is that digest plus dialogue ID; reported provider/model/voice/
adapter must equal that line. The snapshot digest binds the declared/resolved production inputs;
the adapter's model ID is not a complete immutable runtime/data pin. No cache scheme exists here.
Offline identity inspection establishes no current permission, rights, approval
or dispatch eligibility. Future production generation must consume `AuthorizedProductionInputs`
and a durable reservation. The explicitly invoked local sample probe has no production
authorization or artifact acceptance. Snapshots/rights/approvals stay immutable.

## Receipt shape

Every object rejects unknown fields; versions and enum variants are exact. Receipt/document
files are bounded to 64 KiB/4 MiB. Receipt IDs are 1–128 ASCII alphanumeric characters or
`-_.:`; provider request IDs and idempotency keys may be null/absent. No manuscript or provider
error body is admitted into the receipt/report. Provider-specific ID formats require an
explicit future version/mapping.

The fields are `version` (`cantos-tts-receipt-1`), `evidence`, `attempt_id`, `operation_id`,
`provider_request_id`, `idempotency_key`, `input_digest`, `dialogue_id`, `provider`,
`reconciliation`, `usage`, `charge` and `outcome`. `provider` has exact `provider_id`, `model_id`,
`voice_id`, `adapter_version`. `evidence` is `synthetic_fixture` or `caller_reported`, both
declarations. No live-evidence flag exists. Reference-only inputs reject caller-reported output.
Exact `espeak-ng-local` / `espeak-ng-1.52.0` / `espeak-ng-local-v1` identity and
`wav-pcm-s16le-mono-22050-v1` contract select local output inspection; exact `google-cloud-tts` /
`google-cloud-tts-synthesize-3` identity and `wav-pcm-s16le-mono-24000-v1` contract select
Google output inspection. Neither authenticates a caller or accepts a production asset.

The optional neural research adapter admits exact `vieneu-nano-local` /
`vieneu-nano-aba295eb96a6fa6003ebe417cc1f2802a7adc1dc` / `vieneu-nano-local-v1` identity
and `wav-pcm-s16le-mono-24000-vieneu-v1` output with one of its exact two presets. It uses the
distinct `cantos-vieneu-nano-tts-pcm-1` profile at 24,000 Hz; it does not gain output admission
through the historical synthetic profile. Receipt shape/version and scope checks are unchanged.
This is research inspection only; production purpose is refused at the adapter dispatch
boundary before any model/runtime I/O even if a planning document/approval is eligible.
See [neural evidence and exact limits](../../docs/evidence/local-neural-tts.md).

`usage` is `{ "status": "unknown" }` or `reported` with identifier `unit` and integer `count`.
`charge` is independently `unknown` or `reported` with USD/VND `currency`, integer `amount_minor`
and identifier `reference`. Counts/money fit the existing nonnegative signed-bigint bound.
Usage never turns unknown charge into zero; reported money remains a caller assertion, not a
signed invoice, settlement, reservation or budget authorization.

`reconciliation` has `lookup_by_operation_id`, `lookup_by_provider_request_id` and
`same_key_idempotency`. Each is `{ "state": "unavailable" }` or `declared`/`verified` with
bounded nonempty `evidence`. Levels are caller assertions; no evidence is fetched/authenticated.
`declared` is insufficient for lookup advice, and idempotency alone never permits resubmission.

`outcome` is `output` with `audio_sha256`, or `failure` with `class` and `delivery`.
Failures: `timeout`, `rate_limit`, `unsupported_voice`, `unsupported_control`, `permanent`,
`malformed_audio`, `cancelled`. Delivery: `not_sent`, `confirmed_not_executed`,
`may_have_executed`. Status codes alone cannot establish non-execution.

Every ambiguous class requires lookup with a relevant ID and recorded verified capability,
otherwise `manual_reconciliation`. No lookup executes and `not found` is not interpreted here.
Known non-execution maps timeout/rate limit to `bounded_retry_needs_fresh_authorization`,
unsupported voice to `select_voice_explicitly`, unsupported control/permanent failure to
`fix_input`, malformed output to `inspect_output`, cancellation to `stop`. Advice executes no
retry, counter, backoff, reservation or dispatch.

## Actual bytes and report

`output` requires 64 lowercase hex `audio_sha256` plus the third input; failure rejects an
unexpected WAV. Metadata and actual reads enforce bounds before inspection. The diagnostic
profiles admit ordinary RIFF/WAVE PCM tag 1, mono signed 16-bit: historical/synthetic
`cantos-offline-tts-pcm-1` requires 24,000 Hz, `cantos-espeak-tts-pcm-1` requires the local
adapter's native 22,050 Hz and `cantos-google-tts-pcm-1` requires the 24,000 Hz Google requests.
All require nonempty complete decoded frames, at most 120 seconds
and 16 MiB actual bytes. The existing
WAV inspector validates structure; SHA-256 covers the whole file. Facts include checksum,
integer frames, actual format, inspector/profile versions. No conversion or provider selection
occurs. Silence passes structure without proving useful speech. This is no mastering/QC target.

Serialize-only reports label `offline-inspection-only-no-dispatch-or-acceptance`, preserve
attempt/cost facts and bind script revision/export without echoing the manuscript. Exit 0 means
inspection passed, 1 is a JSON domain rejection, 2 is a usage/read/write failure with content-free
diagnostics. On Unix, nonblocking open and descriptor type checks prevent a FIFO swapped into
a checked path from hanging the open. Local regular files are the supported input; no general
network-filesystem deadline is claimed. Reports remain private; nothing is uploaded or written.

No report certifies true peak, loudness, fidelity, listening quality, rights or publication.
Keep generated audio/private evidence outside Git. Production reservations, durable jobs,
commercial mapping/live sample and listening acceptance remain deferred.
See [evidence](../../docs/evidence/tts-receipt-inspection.md) and
[provider prerequisites](../../docs/evidence/tts-provider-prerequisites.md).

## Explicit local sample probe

```sh
cargo run --locked --bin probe-local-tts -- frozen-document.json dialogue-id /absolute/path/to/espeak-ng /absolute/new-private-output-directory
```

The probe accepts only exact local identity/output contract, `vi` or `vi-VN` language and one
of `vi`, `vi-vn-x-central`, `vi-vn-x-south`. Current controls are rate 1000 permille, pitch zero,
emotion `neutral`, intensity zero; unsupported values fail before invoking the runtime.
Existing pronunciation resolution supplies effective spoken text, bounded to 2,000 NFC UTF-8
bytes without controls, U+2028/U+2029 or `[[` native phoneme syntax. The same pure predicate
checks text after pronunciation resolution and at adapter admission. Installed runtime version
and exact voice availability are checked;
the adapter uses a 30-second total process deadline, bounded I/O and private regular-file output.
It makes no cloud call, installs no package and converts no sample rate.

Before synthesis, a newly created private destination receives create-only `intent.json` and
`frozen-document.json`. Intent conservatively records possible execution; setup or synthesis
failures can append `receipt.json` without erasing it. Success saves `output.wav`, `receipt.json`
and `audit.json`, with the output hash bound to the exact frozen snapshot/dialogue/provider.
The same destination is refused rather than overwritten or rerendered. These records support
manual inspection after interruption; they are not database jobs, automatic restart recovery,
an external idempotency guarantee, settled billing or accepted production assets. Reported
Unicode-scalar usage remains distinct from unknown charge and unmeasured local resource cost.

## Explicit Google sample probe

```sh
cargo run --locked --bin probe-google-tts -- frozen-document.json dialogue-id
cargo run --locked --bin probe-google-tts -- frozen-document.json dialogue-id --live /absolute/new-private-output-directory --max-estimate-cents 1
```

The probe accepts only the exact Google identity and output contract frozen by casting, re-admits
the snapshot through the same authority, and re-checks voice, family controls, explicit
`neutral`/0 delivery and text before planning. Without `--live` it prints the plan, request
digest and list-price estimate, reads no credential and writes nothing. With `--live`, an OAuth
access token from `CANTOS_GOOGLE_TTS_ACCESS_TOKEN` (and optional `CANTOS_GOOGLE_TTS_QUOTA_PROJECT`)
authorizes exactly one HTTPS request after the estimate passes the cents cap. The new private
directory receives create-only `intent.json` and `frozen-document.json` first, then
`observation.json` (adapter version, request digest, estimate, limitations and the exact
failure evidence or audio facts) and `receipt.json`, plus `output.wav` and `audit.json` on
success. A connection that never opened is `timeout`/`not_sent`; every outcome after it is
`may_have_executed`, because Google documents no lookup, idempotency key or non-execution
guarantee. Successful usage is `effective-unicode-scalars`; charge stays `unknown`. There is no
retry, reservation, current approval check or accepted asset. See
[ADR 0011](../../docs/decisions/0011-google-cloud-tts-adapter.md) and the
[adapter evidence](../../docs/evidence/google-tts-adapter.md).

Historical Google `google-cloud-tts-synthesize-2` receipts retain their scoped inspection profile
and original digest. Historical admission is inspection only; current planning refuses that
adapter and requires a new frozen candidate.
