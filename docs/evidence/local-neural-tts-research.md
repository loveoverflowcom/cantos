# Local neural Vietnamese TTS research

Status: **documented** primary-source comparison, 2026-10-11. VieNeu-TTS v3 Nano is selected
for a bounded private research sample; production adoption is **NO_GO** pending provenance
review. Installation identity, installed model/preset bytes, disk footprint and the host
profile are **verified** below. All six live original sample cases **PASS** with inspected WAVs,
measured cold latency and observed RSS. Listening remains **NOT_RUN**; successful synthesis
does not resolve production rights.
Related issue: [#6](https://github.com/loveoverflowcom/cantos/issues/6).

This extends [local provider prerequisites](tts-provider-prerequisites.md) without changing
the [eSpeak inspection selection](../decisions/0008-local-tts-inspection.md), existing samples or
receipt evidence. The owning rules remain the [production pipeline](../product/production-pipeline.md),
[020 work item](../work-plan/020-cast-and-generate-dialogue.md) and
[production-input decision](../decisions/0007-casting-production-inputs.md).
This research does not complete the commercial provider criteria or authorize closing #6.

## Selection and scope

Two concrete CPU options were compared: Piper with its published Vietnamese models, and
VieNeu-TTS v3 Nano with ONNX Runtime. Published platform artifacts and the recorded install
establish packaging feasibility. The measured local run below separately establishes successful
CPU synthesis; Vietnamese speech quality still requires a listening assessment.

| Candidate | Declared Vietnamese voices / native sample rate | Required model bytes | Decision |
| --- | --- | --- | --- |
| Piper `vi_VN-vivos-x_low` | 65 speaker IDs; 16,000 Hz | 27,789,413 | Small CPU option; defer all downloads because noncommercial restrictions and provenance need an intended-use determination |
| Piper `vi_VN-vais1000-medium` | One speaker; 22,050 Hz | 63.2 MB, publisher rounded listing | Defer: Lessac training lineage does not establish production rights |
| Piper `vi_VN-25hours_single-low` | One speaker; 16,000 Hz | 63.1 MB, publisher rounded listing | Defer: unknown InfoRe dataset license and noncommercial Ryan lineage |
| VieNeu-TTS v3 Nano | 11 SDK presets; 24,000 Hz; Cantos admits only Đức Trí and Mỹ Duyên | 281,756,486 across six preset-inference files | Select for private research under the publisher's explicit public weights/preset grant; production dispatch remains disabled |

Selection is based on a named public grant for the exact released model and preset assets,
CPU execution, and a bounded footprint. It is not a naturalness, pronunciation, emotional
performance or publication recommendation. Only original short, nonprivate Vietnamese fixtures
are admitted. No paid API, cloud synthesis, account, API key or new access grant is needed.
No dataset agreement has been accepted, and no gated dataset access is part of this slice.

## Piper: engine and voice rights are separate

The current [OHF engine](https://github.com/OHF-Voice/piper1-gpl) is GPL-3.0-or-later.
The [1.8.0 package](https://pypi.org/project/piper-tts/) publishes a 34.1 MB
`cp39-abi3-macosx_11_0_arm64` wheel, requiring Python 3.9 or newer. Its attested publishing
commit is [`639388b6317fc4731e91d53da42aea68fd4166ff`](https://github.com/OHF-Voice/piper1-gpl/tree/639388b6317fc4731e91d53da42aea68fd4166ff);
wheel SHA-256 is `33e7425933e9290fe651ae127916ed1ca6104cfa3d94e9049295dd3a5c449382`.
This is engine packaging evidence, not voice clearance. Piper's
[voice documentation](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/639388b6317fc4731e91d53da42aea68fd4166ff/docs/VOICES.md)
directs users to each model card and imposes no additional model restrictions of its own.

The official voice repository inspected at
[`c10ece1aade47bb51c153c893d14e5bf8e5b7117`](https://huggingface.co/rhasspy/piper-voices/tree/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN)
contains three Vietnamese families. Its top-level MIT tag cannot replace their specific rights:

- [Vivos model card](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN/vivos/x_low/MODEL_CARD):
  trained from scratch; declares CC BY-NC-SA 4.0 for its dataset. The card inconsistently names
  InfoRe Technology 1 while linking Vivos. Its
  [configuration](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN/vivos/x_low/vi_VN-vivos-x_low.onnx.json)
  records 65 `VIVOSSPK…` / `VIVOSDEV…` IDs; no supported male/female labels are inferred.
  The original [university dataset page](https://ailab.hcmus.edu.vn/vivos/) timed out during
  inspection. Original speaker consent and output permissions remain unconfirmed here.
- [VAIS model card](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN/vais1000/medium/MODEL_CARD):
  cites a CC BY 4.0 dataset but fine-tunes the English Lessac model. The
  [Lessac card](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/en/en_US/lessac/medium/MODEL_CARD)
  links an [individually granted research agreement](https://www.cstr.ed.ac.uk/projects/blizzard/2013/lessac_blizzard2013/license.html)
  excluding commercial uses of its materials. Its application to these weights and output is
  unresolved. No agreement was submitted or accepted.
- [25hours_single model card](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN/25hours_single/low/MODEL_CARD):
  explicitly marks InfoRe dataset licensing unknown and fine-tunes English Ryan. The
  [Ryan card](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/en/en_US/ryan/low/MODEL_CARD)
  adds CC BY-NC-SA 4.0 dataset lineage.

[CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/legalcode.en) permits the
covered uses only within its noncommercial scope and carries attribution/share-alike duties.
Calling a trial private or research does not establish that commercial product development
qualifies. No Piper model is downloaded or selected for dispatch in this slice.

| Piper model file | Publisher SHA-256 |
| --- | --- |
| [Vivos ONNX](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN/vivos/x_low/vi_VN-vivos-x_low.onnx) | `6ab13374eb0862021a545befe7727aef59e16117f1c075aa9e0362237ecc98ae` |
| [VAIS ONNX](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN/vais1000/medium/vi_VN-vais1000-medium.onnx) | `ec7c89e2c85f4d1edc24b6120c18aaf1bda614f06b511567eb9c7c0de15e2dab` |
| [25hours_single ONNX](https://huggingface.co/rhasspy/piper-voices/blob/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN/25hours_single/low/vi_VN-25hours_single-low.onnx) | `266945c4a80cb3301450c8ba6fcf6ba3542c56cf952a08a8ff3d2d4ff0ab5609` |

## Nano: public grant and unresolved training provenance

The [publisher's Nano FAQ](https://huggingface.co/pnnbao-ump/VieNeu-TTS-v3-Nano#-usage-rights--licensing-faq)
explicitly applies Apache-2.0 to released graphs, configuration, constants and SDK preset assets.
It grants commercial use of preset-generated audio without an additional fee or license, and
states speaker/rightsholder consent for training and synthetic speech. These are publisher
statements, not independently inspected consent records. The internal corpus pipeline is
undisclosed. Its linked
[1000-hour corpus](https://huggingface.co/datasets/pnnbao-ump/VieNeu-TTS-1000h) is CC BY-NC 4.0
with institutional research access conditions. No corpus was requested or downloaded.

The public artifact grant supports this authorized research trial; it does not resolve the
relationship between the linked restricted corpus, the full training inputs and production
distribution. Cantos therefore imposes **research-only** eligibility and **NO_GO for production**
until a recorded review resolves that provenance gap. This restriction is Cantos's decision,
not a claim that the publisher forbids commercial output. Preserve the model/package license
and attribution notices. A frozen snapshot, a valid WAV or a receipt cannot authorize production
or turn research output into an accepted production asset.

The [SDK 3.8.3 package](https://pypi.org/project/vieneu/3.8.3/) declares Apache licensing and
Python 3.10+. Its wheel is `vieneu-3.8.3-py3-none-any.whl`, 2,643,059 compressed bytes
(5,111,563 uncompressed bytes), with SHA-256
`7388d166e65746f5bb075bf8094d324f8131a82393680b712260d6f2f983ef06`.
It was not published with a PyPI trusted-publishing attestation. The separately pinned
ONNX Runtime version below requires Python 3.11+, so the complete selected runtime requires
Python 3.11 or newer.

GitHub source was inspected at
[`85344322b7258b4e25479b692e8e3396baf9db34`](https://github.com/pnnbao97/VieNeu-TTS/tree/85344322b7258b4e25479b692e8e3396baf9db34).
No exact SDK release tag was established; this source commit is an inspection reference, while
the release wheel hash and checked module hashes identify the runtime. Its Nano docstring
claims approximately 4,000 hours of Vietnamese training; the exact relationship to the linked
1000-hour corpus is unspecified.

## Exact Nano model and presets

Pin [model revision `aba295eb96a6fa6003ebe417cc1f2802a7adc1dc`](https://huggingface.co/pnnbao-ump/VieNeu-TTS-v3-Nano/tree/aba295eb96a6fa6003ebe417cc1f2802a7adc1dc).
The six preset-inference files total **281,756,486 bytes**, approximately **268.70 MiB**.
These publisher sizes and hashes matched the six installed regular files. The installed SDK
3.8.3 source and preset identities also matched the helper's checked pins; this verifies artifact
identity rather than model output or production rights.

| File | Bytes | SHA-256 |
| --- | --- | --- |
| `codec_decoder.onnx` | 99,319,941 | `b0ab15e7828a39d53679e25b1ba4ba415a61311307202a6323130b9e1cc3029d` |
| `duration_predictor.onnx` | 727,809 | `20fd7fa60006d0a48ee82e0451b3f920d2052588c083c756cce669f3947c0a68` |
| `text_encoder.onnx` | 26,519,943 | `204f02cccae1f16ccb2d3840f05721a37fe250b82cbd456337a0ccb49615e4bf` |
| `vector_estimator.onnx` | 155,132,418 | `c6c1d4398ca35d3ad1bd3f0459413d1b975d09e7f2f3a2b4493a7b92bbf6ce93` |
| `config.json` | 2,927 | `3762f1716fd8d7a1451ddf2e051c03b0a341e8adda4f1dbd0db0ae9777514e56` |
| `constants.npz` | 53,448 | `7c011938effe41687a9af85107a31a040dfcf0fb3d0f048ec10f4fb65c1a8829` |

The versioned SDK preset file is `vieneu/assets/voices_v3_nano.json`, 2,316,799 bytes;
SHA-256 `2ca4cbf475409e61fda662fbd3fb3a3ac3e980bbc3925fedb4d1199c4ccac3e7`.
It holds 11 named presets: Adam, Ái Hân, Mỹ Duyên, Đức Trí, Hữu Quân, Xuân Tiên, Mai Anh,
Trúc Ly, Anh Khôi, Minh Quân and Mạnh Dũng. Older package/card prose still says six; do not
infer availability from that prose or follow a mutable default voice.

The implemented Cantos research adapter admits only `duc-tri` → Đức Trí and `my-duyen` → Mỹ Duyên.
The publisher identifies these as male/female Northern Vietnamese storytelling presets;
voice character and gender labels are declarations, not local listening observations.
An unrecognized model, provider, version, voice or language must block before synthesis.
No default substitution, arbitrary embeddings or reference-audio enrollment is admitted.

## Controls and local process boundary

[Piper's versioned API](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/639388b6317fc4731e91d53da42aea68fd4166ff/docs/API_PYTHON.md)
exposes duration scaling, noise variation, normalization and volume; named drama emotion and
arbitrary pitch are not established. Its
[CLI](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/639388b6317fc4731e91d53da42aea68fd4166ff/src/piper/__main__.py)
strips input lines, opens output for overwrite and parses a configuration argument without
passing it to the loader. Its
[loader](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/639388b6317fc4731e91d53da42aea68fd4166ff/src/piper/voice.py)
uses CPU by default and ignores speaker selection for a single-speaker model. These behaviors
need explicit preflight and process/file protection if Piper is reconsidered.

Nano's [versioned sampler source](https://raw.githubusercontent.com/pnnbao97/VieNeu-TTS/85344322b7258b4e25479b692e8e3396baf9db34/src/vieneu/v3nano.py)
exposes duration speed, Euler steps, guidance, sampling sway, seed and CPU threads. Upstream
also accepts ignored arguments and warns/drops unknown phonemes. Cantos must reject unsupported
text/controls rather than inherit those behaviors. Experimental inline cues and a preset's
style tokens do not establish a controlled emotion or pitch contract.

The SDK's pinned phonemizer enables SEA-G2P `punc_norm`. For sentences shorter than five words,
that normalization can replace a terminal question mark or exclamation mark with a period.
The helper retains the upstream normalization recipe; expressive question/exclamation delivery
is therefore unverified and must be assessed by listening to the actual admitted sample.

The pinned sampler also clamps predicted duration to 15 seconds per chunk. The helper checks
every chunk's prediction first and rejects nonfinite/nonpositive/over-limit results before any
inference. This adds one text/duration graph evaluation per chunk; measured timings must include
that guard rather than borrowing upstream benchmark numbers.

The selected research profile fixes rate to 1,000 permille, pitch to zero, emotion to `neutral`
and intensity to zero; other values are unsupported. The fixed sampler profile is 16 steps,
guidance 3.0, sway 0.0, speed 1.0 and seed equal to the zero-based chunk index; ONNX uses the
CPU provider with two intra-operation threads and one inter-operation thread. These are adapter
policy values, not a claim that the model will perform emotionally neutral speech.
The target output is explicit mono signed 16-bit PCM WAV at 24,000 Hz. All six live outputs
passed the existing WAV inspector's neural profile and independent Python `wave`/SHA-256 checks,
including nonempty, nonzero PCM and scoped receipt inspection.

The [helper source](../../scripts/vieneu_nano_runtime.py) and
[pure admission contract](../../services/cantos-api/src/tts/neural_contract.rs) are implemented boundaries.
Their existence alone does not prove safeguards; fresh live and fault evidence is linked below.
The process boundary
must enforce current research eligibility, bounded input/output, deadline/resources, child
concurrency, exact model/preset bytes and create-only output. Intent and receipts must remain
bound to the frozen snapshot, dialogue, effective input, model/voice/version and output checksum.
No automatic repeat follows an ambiguous attempt. Production asset acceptance is outside this
research adapter's authority.

## Platform and disk admission

The selected core pins are `vieneu==3.8.3`, `sea-g2p==0.9.1`, `onnxruntime==1.24.4`,
`numpy==2.3.3` and `huggingface-hub==0.35.3`; required small transitive dependencies also need
an installation record. Published wheels support the intended CPU path:

| Package | Published artifact / host prerequisite | Compressed publisher size |
| --- | --- | --- |
| [VieNeu 3.8.3](https://pypi.org/project/vieneu/3.8.3/) | `py3-none-any`; Python 3.10+ | 2,643,059 bytes |
| [SEA-G2P 0.9.1](https://pypi.org/project/sea-g2p/0.9.1/) | `cp310-abi3-macosx_11_0_arm64`; macOS 11+ | 27,760,277 bytes |
| [ONNX Runtime 1.24.4](https://pypi.org/project/onnxruntime/1.24.4/) | `cp313-cp313-macosx_14_0_arm64`; other Python 3.11+ ARM64 wheels listed; macOS 14+ | 17,341,922 bytes |
| [NumPy 2.3.3](https://pypi.org/project/numpy/2.3.3/) | `cp313-cp313-macosx_14_0_arm64`; BSD license | 5,106,537 bytes |
| [Hugging Face Hub 0.35.3](https://pypi.org/project/huggingface-hub/0.35.3/) | `py3-none-any`; used offline during synthesis; Apache-2.0 | 564,262 bytes |

SEA-G2P ARM64 wheel SHA-256 is `685c906b47d420c1e212afb1fad1bc3c36aa9f6729e3e72d2e0aa38fc78e4f44`.
The listed CPython 3.13 ONNX Runtime wheel SHA-256 is
`e99a48078baaefa2b50fe5836c319499f71f13f76ed32d0211f39109147a49e0`.
NumPy's listed wheel SHA-256 is
`9dc13c6a5829610cc07422bc74d3ac083bd8323f14e2827d992f9e52e22cd6a6`;
Hugging Face Hub's wheel SHA-256 is
`0e3a01829c19d86d03793e4577816fe3bdfc1602ac62c7fb220d593d351224ba`.
The exact core wheel subtotal is 53,416,057 compressed bytes, before small transitive wheels.
These exact values come from each publisher's versioned PyPI JSON metadata; for example
[NumPy metadata](https://pypi.org/pypi/numpy/2.3.3/json) and
[VieNeu metadata](https://pypi.org/pypi/vieneu/3.8.3/json).
SEA-G2P is Apache-2.0 and ONNX Runtime is MIT; model rights remain a separate decision.
Wheel metadata does not establish a successful install, dependency import or actual CPU speed.

The controlled installation used **CPython 3.13.5** on **Apple M4, 10 cores, ARM64, 24 GiB RAM,
macOS 15.6**. The core and SDK pip reports record the five exact package versions and published
wheel hashes listed above. The SDK was installed without its optional dependency set. Read-only
inspection of the installed modules confirmed that the selected import chain requires NumPy,
ONNX Runtime, Hugging Face Hub and SEA-G2P; SEA-G2P's bundled dictionary is present. Guarded
dependency imports and real CPU synthesis passed in the six-case live run below.

| Retained installation | Regular-file bytes | Allocated disk space |
| --- | --- | --- |
| Isolated runtime | 284,249,853 | 293,680 KiB |
| Six-file Nano model | 281,756,486 | 275,168 KiB |
| Combined | **566,006,339** | **568,848 KiB = 555.515625 MiB** |

These are measured retained directory footprints. They exclude the existing Python interpreter,
temporary downloads, compiler/test space and generated audio evidence, and do not establish a
peak-space guarantee. The earlier 120–250 MiB runtime / 389–519 MiB combined planning estimate
underestimated the retained installation. The initial shared Mac observation was approximately
1.4 GiB free; future installation or evidence runs still need a fresh disk check.

Gradio/Web UI, librosa, Torch/CUDA, generic sound-file writing, cloning graphs, reference clips
and optional denoising are intentionally excluded from this preset-only path. They add storage,
dependencies or behaviors without a consumer here. The selected helper uses bounded offline
inference and explicit PCM encoding; it does not run the SDK's default Turbo factory or its
automatic download path. The successful live run verifies this minimal installed path rather
than assuming that optional dependencies can be omitted.

## Live evidence and resource coordination

The MyVa Bevy performance task shares this Mac. Research and text edits can proceed; model
downloads, heavy compilation and synthesis benchmarks wait for the parent's execution window.
Do not delete source, weights, existing samples, evidence or another project's caches.
The parent granted a controlled window for this installation, exact model download and serial
live run, with no overlapping MyVa build or benchmark. All six original sample cases passed in
40.00 seconds. [Implementation/run evidence](local-neural-tts.md) records commands, checks,
retained failed attempts and limits;
[machine-readable measurements](local-neural-tts-run.json) record exact source/package/input/WAV
identities and per-case observations. Ignored local weights, WAVs and private records remain
outside Git.

| Evidence required per admitted voice | Current result |
| --- | --- |
| Runtime versions, guarded imports and installed graph/preset checksums | **PASS**, exact pinned minimal runtime and CPU graphs |
| Real narration, question/exclamation, Vietnamese names/tones/punctuation and moderate dialogue WAVs | **PASS**, six original fixtures across Đức Trí and Mỹ Duyên |
| Nonempty structurally playable PCM, frames/duration, file size and SHA-256 | **PASS**, mono PCM16 at 24,000 Hz; Rust inspection plus independent Python checks |
| Per-invocation wall latency and RTF = wall seconds / inspected audio seconds | **PASS**, cold wall 6.38–7.32 seconds, audio 7.66–18.84 seconds, RTF 0.389–0.838 |
| Observed resident memory, with named measurement tool and units | **PASS**, sampled child maximum 1,021,820,928 bytes; `/usr/bin/time -l` maximum 1,023,328,256 bytes |
| Runtime/model disk usage and exact host profile | **PASS** for the retained footprint and M4 profile recorded above |
| Named Vietnamese listening reviewer; pronunciation, naturalness and emotional delivery | **NOT_RUN** |
| Fault-injected process/input/audio/scope/research-eligibility rejection tests | **PASS**, native fault/receipt tests and Python discovery's 103 tests including 23 helper regressions |

The cold timings include hashing, interpreter/session startup, all chunk duration preflights,
inference and private record handling, with two intra-op CPU threads and one inter-op thread.
This is one observation per fixture, with the host's iCloud `bird` process consuming approximately
one CPU core. RSS sampling and `time -l` observations do not establish an exact instantaneous
physical-memory peak or a hard macOS memory quota. No warmed or statistical benchmark is claimed.
Native workspace Clippy/tests, the fresh PostgreSQL 17 checks and Studio WASM/Trunk builds also
passed; their exact commands and scope are in the implementation evidence.

The [versioned SDK README](https://github.com/pnnbao97/VieNeu-TTS/blob/85344322b7258b4e25479b692e8e3396baf9db34/README.md)
claims Nano RTF 0.22 at 16 steps and 0.11 at eight steps with sway −1 on a 12th-generation
Intel CPU. The README identifies an i5 / Windows 11 profile, while the model card says i7;
that hardware inconsistency remains unresolved. These are publisher claims from another
machine and profile, not Mac measurements or promises for Cantos's two-thread process.
The cold scope above includes interpreter startup and model loading; future warmed measurements
require separate records. Original fixture text may be committed; downloaded weights, generated
WAVs and private run records stay outside Git. A checksum proves identity, not listening quality.

**Invariant:** public software packaging and immutable input identity cannot substitute for
current voice/model rights or authorize production. **Owner / boundary:** provider dispatch
and publication eligibility, using the existing immutable production document and receipts.
**Evidence level:** documented primary-source inspection plus verified installed artifact
identity, retained disk footprint/host profile and six real inspected local CPU renders.
**Provenance:** versioned publisher model/package metadata and the source links above. Real runtime
observations and synthetic fault fixtures are distinguished; neither establishes listening quality.
**Edge classes addressed by the research decision:** missing
voice grant, restrictive/unknown training lineage, mutable presets, ignored controls and
resource admission. **Residual risk:** training-source clearance, native dependency trust,
sampled memory/cold-run measurement limits, listener assessment and production eligibility
remain open. Reopen production adoption only after rights/provenance review and the relevant
dispatch, audio, approval and publication evidence; this document advances neither commercial
synthesis nor durable workers.
