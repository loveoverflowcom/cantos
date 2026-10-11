# TTS provider prerequisites

Status: primary-source research, **2026-10-10**. The user subsequently authorized local
runtime/library/model installation and actual local TTS. The bounded implementation selects
eSpeak NG 1.52.0 for a nonprivate Vietnamese inspection sample. Commercial/cloud calls,
credentials, account setup and paid spending remain unauthorized. Installation, synthesis and
audio verification are **NOT_RUN** in this research; the implementation owns live evidence.

The local boundary is recorded in [ADR 0008](../decisions/0008-local-tts-inspection.md).
This record supports the unfinished commercial-provider decision for
[#6](https://github.com/loveoverflowcom/cantos/issues/6). It does not complete that issue's live
sample criteria. [Work plan 020](../work-plan/020-cast-and-generate-dialogue.md) supersedes the
former local/open-source generation prerequisite and requires explicit provider, rights and
spend authorization. The new authorization permits this local inspection; it does not establish
production adoption. The issue still requires both local and commercial live samples, so this
partial slice cannot claim full completion. A production cloud execution boundary is unresolved.

The invariant is that documented provider capabilities cannot become billable eligibility or
publication rights without exact input, provider, voice, rights and cost evidence. The owning
rules remain in the [production pipeline](../product/production-pipeline.md),
[business rules](../product/business-rules.md) and
[production input contract](../../packages/contracts/production-v1.md). This record adds no competing
authorization system.

## Selected local inspection engine

eSpeak NG uses compact formant synthesis. Its publisher describes the sound as less natural
than systems based on human speech recordings. It writes WAV and needs no downloaded neural
voice weights for the selected built-in voices. This is a small verification path, not evidence
that its sound is acceptable for Cantos Theatre.
[Versioned publisher README](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/README.md)

| Exact CLI voice ID | Publisher name | Versioned bundled data |
| --- | --- | --- |
| `vi` | Vietnamese (Northern) | [aav/vi](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/espeak-ng-data/lang/aav/vi) |
| `vi-vn-x-central` | Vietnamese (Central) | [aav/vi-VN-x-central](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/espeak-ng-data/lang/aav/vi-VN-x-central) |
| `vi-vn-x-south` | Vietnamese (Southern) | [aav/vi-VN-x-south](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/espeak-ng-data/lang/aav/vi-VN-x-south) |

These are declarations. Record `espeak-ng --version` and the installed `--voices=vi` table before
using an exact voice. Do not infer emotion, role, real-person identity or cloning permissions.
The CLI declares speed in approximate words per minute, pitch and amplitude; Cantos supports
only controls explicitly mapped and tested by its adapter. Unsupported intent must block.
[Versioned CLI manual](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/src/espeak-ng.1.ronn)

The 1.52.0 source writes RIFF PCM format 1, mono, 16-bit samples with the engine's native rate.
It finalizes RIFF/data sizes for a seekable output file; stdout retains provisional lengths.
Use a regular file and inspect bounded actual bytes. Bulk `--stdin` removes its final byte:
terminate complete UTF-8 input with a newline. Voice selection can fall back from a name to
language properties, so preflight exact installed identity and reject arbitrary voice strings.
[Versioned CLI source](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/src/espeak-ng.c)
The engine defaults to 22,050 Hz before loading phoneme data; actual bytes determine receipt
format. [Versioned engine source](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/src/libespeak-ng/speech.c)

Homebrew's inspected formula declares 1.52.0, GPL-3.0-or-later, Apple Silicon bottles and the
`pcaudiolib` runtime dependency. Source builds additionally require build tools. Formula revision
`5de7d5dec0d98dff198409e0205510dcd17f88fb` pins upstream source SHA-256
`bb4338102ff3b49a81423da8a1a158b420124b055b60fa76cfb4b18677130a23` and ARM64 Sequoia bottle
SHA-256 `330873deca13228ec98927f86fb4e18e990e8707f888aaf665b6e12a55efaf47`. Record actual installed
bottle/dependent versions, checksums and data location; packaging is not a measured disk,
RAM or latency bound.
[Pinned Homebrew formula](https://raw.githubusercontent.com/Homebrew/homebrew-core/5de7d5dec0d98dff198409e0205510dcd17f88fb/Formula/e/espeak-ng.rb)

The publisher's project license applies to the engine and bundled source/data. The selected
language-rule files have no separately downloaded neural weights or cloned-person model.
GNU GPL version 3 section 2 permits running the program and covers its output only when that
output itself constitutes a covered work. This is not blanket clearance of input content,
attribution, publicity rights or publication. Retain notices and review redistribution
obligations before bundling engine/data in a shipped product. The selected slice invokes a
separately installed executable.
[Versioned license](https://raw.githubusercontent.com/espeak-ng/espeak-ng/1.52.0/COPYING)

## Deferred documentation-only comparison

Neither Piper nor Azure is selected, installed, configured or called by this research.

| Question | Local/open-source candidate: Piper | Commercial candidate: Azure Speech standard neural voices |
| --- | --- | --- |
| Vietnamese declaration | `vi_VN-vais1000-medium`: one speaker, 22,050 Hz; the model card labels quality as medium, not a Cantos listening result. [Voice model card](https://huggingface.co/rhasspy/piper-voices/raw/v1.0.0/vi/vi_VN/vais1000/medium/MODEL_CARD) | Microsoft lists `vi-VN-HoaiMyNeural` and `vi-VN-NamMinhNeural` as Vietnamese standard voices; their style/role columns are empty. Account/region availability is untested. [Microsoft voice table source](https://raw.githubusercontent.com/MicrosoftDocs/azure-ai-docs/main/articles/ai-services/speech-service/includes/language-support/tts.md) |
| Runtime / host prerequisite | Current engine is GPL-3.0. [Official engine repository](https://github.com/OHF-Voice/piper1-gpl) PyPI lists `piper-tts` 1.8.0 with a CPython 3.9 ABI3 wheel for macOS 11+ ARM64 (34.1 MB). This establishes published packaging, not a successful Mac installation. [Publisher package](https://pypi.org/project/piper-tts/) | Requires an Azure account, Speech resource, resource endpoint and authorized authentication. A credential must stay in the chosen host/backend boundary. [REST prerequisites](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/rest-text-to-speech#prerequisites) |
| Audio declaration | The CLI writes WAV; each voice needs its ONNX model and JSON configuration. [CLI](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/main/docs/CLI.md), [voice files](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/main/docs/VOICES.md) The Vietnamese model listing is 63.2 MB, separate from runtime/dependency storage. [Model listing](https://huggingface.co/rhasspy/piper-voices/tree/v1.0.0/vi/vi_VN/vais1000/medium) | REST documents RIFF 16-bit mono PCM, including `riff-24khz-16bit-mono-pcm`. The real-time response is capped at ten minutes and can be truncated. These declarations do not establish valid returned bytes. [REST audio formats and limit](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/rest-text-to-speech#sample-request) |
| Control declaration | CLI documents volume, sentence silence and raw espeak-ng phoneme input. Model config records length/noise defaults. No drama-emotion or arbitrary-pitch support is established here. [CLI](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/main/docs/CLI.md), [Vietnamese configuration](https://huggingface.co/rhasspy/piper-voices/raw/v1.0.0/vi/vi_VN/vais1000/medium/vi_VN-vais1000-medium.onnx.json) | SSML documents rate, pitch and volume, but values are suggestions and unsupported values can be substituted. Unsupported styles can silently fall back to neutral. Cantos must preflight required controls and reject a missing voice/style instead. [SSML voice and prosody](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/speech-synthesis-markup-voice) |
| Cost declaration | No commercial API tariff was found for this local engine. Local resource consumption is unmeasured; no zero-cost estimate is established. | Billing covers Unicode code points, whitespace, punctuation and SSML markup except `speak`/`voice` tags in successfully processed requests, even when audio is not produced. [Billing definition](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech#pricing-note) The pricing page declares 0.5 million neural characters monthly on F0; parsed paid rates appeared as `$-`, so an exact account/region/currency quote remains unknown. [Pricing](https://azure.microsoft.com/en-us/pricing/details/speech/) |

These are **declared** capabilities from inspected primary sources. An exact version/voice/control
becomes **verified** only after its actual availability and outputs are exercised with recorded
evidence. A capability becomes **supported by Cantos** only after the chosen execution boundary
implements and tests its mapping. No capability in this table has reached either latter state.

## Voice permissions and terms gaps

The Piper Vietnamese model card attributes its VAIS dataset to CC BY 4.0, but also states that
the model was fine-tuned from the U.S. English Lessac voice. The
[Lessac model card](https://huggingface.co/rhasspy/piper-voices/raw/v1.0.0/en/en_US/lessac/medium/MODEL_CARD)
links to a [research license](https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/license.html)
that excludes commercial use of the dataset materials. The effect on this specific trained model
and intended generated output is unresolved; a top-level model-repository license is insufficient
permission evidence. Obtain an exact model rights/provenance determination before production or
choose an explicitly authorized alternative. Piper's own
[voice documentation](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/main/docs/VOICES.md)
also tells users to review each voice's restrictions. No voice cloning or real-person consent is
inferred from a downloadable model.

Piper's smaller `vi_VN-vivos-x_low` card declares 65 speakers, 16,000 Hz, training from scratch
and CC BY-NC-SA 4.0 dataset restrictions. Its ONNX file is listed as 27.8 MB. It remains deferred;
an explicitly restricted private research trial would not clear commercial production rights.
[Model card](https://huggingface.co/rhasspy/piper-voices/blob/main/vi/vi_VN/vivos/x_low/MODEL_CARD),
[model listing](https://huggingface.co/rhasspy/piper-voices/tree/main/vi/vi_VN/vivos/x_low)
Facebook's `mms-tts-vie` checkpoint likewise declares CC BY-NC 4.0. Revision
`b58928d033932a49aa8e3d6cf11625b25fe928d2` lists a 145 MB safetensors file separately from a
duplicate pickle checkpoint; its Torch environment footprint and Mac performance are unmeasured.
It is deferred and does not establish unrestricted production permission.
[Publisher card](https://huggingface.co/facebook/mms-tts-vie),
[pinned files](https://huggingface.co/facebook/mms-tts-vie/tree/b58928d033932a49aa8e3d6cf11625b25fe928d2)

For Azure, Microsoft places responsibility for input-content rights and applicable voice/talent
permissions on the customer. It states that real-time synthesis does not retain input text or
output audio. This is a documented processing policy, not permission to transmit a private
manuscript or proof of redistribution rights. [Data and privacy](https://learn.microsoft.com/en-us/azure/foundry/responsible-ai/speech-service/text-to-speech/data-privacy-security)
The [Product Terms page](https://www.microsoft.com/licensing/terms/productoffering/MicrosoftAzure/all)
requires the applicable licensing program; program-specific terms were unavailable through this
research path. Record the user's actual agreement and permitted publication use before treating
the selected prebuilt voice as rights-cleared. Custom/personal voice, talent recordings and
cloning remain outside this candidate scope.

## Attempts, errors and uncertain billing

Azure real-time REST documents 400/401/415 input or authentication failures, 429 quota/rate
limits and 502/503 service failures. The inspected real-time contract documents neither a
deduplication key guarantee nor an operation-result lookup API. A transport timeout after a
possible send therefore cannot establish non-execution or zero charge. These are findings about
the inspected path, not proof that every Azure product lacks lookup. [Real-time REST contract](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/rest-text-to-speech)

Azure's separate batch API documents caller-chosen unique synthesis IDs, GET status/results and
`billingDetails.neuralCharacters`; history expires after seven days by default and at most
31 days with the configured retention. That is a potential reconciliation path requiring a
separate choice and verification. A GET capability alone does not prove exactly-once synthesis,
safe duplicate creation or an actual settled monetary charge. [Batch synthesis contract](https://learn.microsoft.com/en-us/azure/ai-services/speech-service/batch-synthesis)

For either execution choice, preserve the immutable snapshot authorization and bind an attempt
to the exact dialogue/input digest, effective model/voice/controls, adapter/tool version and
operation identity before dispatch. Record dispatch uncertainty independently of failure class.
Unsupported voice/control blocks; confirmed invalid input or authentication requires correction;
rate limiting permits only the chosen bounded recovery policy when execution is known not to
have occurred. A timeout, malformed result or provider error after possible dispatch must not
erase the attempt or automatically release unknown cost as zero. Reconcile through a proved
lookup/deduplication contract, or stop for an explicit operator decision with uncertainty visible.
These are requirements for the subsequent implementation, not claims of persisted attempt or
reservation behavior in this research.

## Decision inputs and live evidence still required

- The authorized eSpeak NG local path has now run: all three Vietnamese voices and actual WAV
  bytes are recorded in [local evidence](tts-receipt-inspection.md), with installed identity,
  post-run data fingerprints and scoped receipts. Listening remains NOT_RUN. Broader production
  adoption and a commercial execution boundary remain separate decisions.
- For any later model or provider, select exact version/voice, permitted output use and
  input/voice rights records. Piper requires exact model/lineage clearance before production.
- For a commercial path, supply the actual account/region/tier, approved authentication boundary,
  exact price/currency and hard test spending limit. An advertised free allowance is not proof of
  remaining account quota or authorization for a call.
- Pin the declared capability record, required controls, output profile and retry/lookup policy.
  Published packaging is not a measured disk/RAM/latency bound. No minimum RAM or performance
  guarantee was established; record actual installation storage and execution observations.
- A commercial sample requires its missing explicit prerequisites first. Keep
  audio outside Git, inspect the actual bytes with the existing bounded WAV inspector, record
  output checksum/duration/format and provider/tool attempt evidence, then name a listener and
  pronunciation/performance observations. Structural/sample-peak inspection does not establish
  true peak, loudness, editorial quality or publication approval.

## Evidence boundary

| Evidence | Result |
| --- | --- |
| Repository docs/skills and current #6 body | Read; scope mismatch recorded without changing remote acceptance criteria |
| Linked primary provider/package/model/license documentation | **documented**; inspected 2026-10-10; mutable pages must be rechecked when selecting |
| Local installation/synthesis/validation by this researcher | **NOT_RUN**; root implementation owns live evidence |
| Commercial account/credential configuration or live sample | **NOT_RUN**, unauthorized pending selection/terms/budget |
| Rate-limit/timeout observations or idempotency/lookup experiment | **NOT_RUN** |
| Actual billing, usage settlement, local RAM/latency or cost cap enforcement | **NOT_RUN** |
| Audio validation/listening, expressive quality and publication rights clearance | **NOT_RUN** |

No account, provider or application process was started by this research and no model/audio was
downloaded. Residual risk includes the missing commercial decision/budget, incomplete production
rights evidence and unverified quality. The parent implementation owns live results, processes,
repository checks and PR status.
