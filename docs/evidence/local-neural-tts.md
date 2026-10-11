# Local neural Vietnamese TTS evidence — issue #6

This is part of [#6](https://github.com/loveoverflowcom/cantos/issues/6), within the user's
authorized local research scope. [ADR 0010](../decisions/0010-local-neural-tts-research.md)
records the bounded selection; [primary-source comparison](local-neural-tts-research.md) separates
engine/model/preset rights from unresolved training lineage. Production dispatch is disabled.
Commercial synthesis, durable workers, clip cache, mixing/publication, Studio/player changes,
S3/CDN, CMP and new CI remain outside this slice. Existing eSpeak source/samples are preserved.

The later [five-commit boundary review](local-neural-tts-review.md) records separate CLI/deadline
fixes, fresh local gates, official rights rechecks and a byte-verified Library listening handoff.
Its [rubric and short punctuation probes](local-neural-tts-listening.md) supplement the original
six samples below. Historical timings/source fingerprints remain historical; listening is still
NOT_RUN and production remains NO_GO.

## Invariant and owner

**Invariant:** only exact admitted research inputs may invoke the pinned local neural runtime;
bounded inspected output stays tied to its snapshot/dialogue/model/voice and never becomes
production authorization, accepted audio, paid usage or a retry permission.

**Owner / boundary:** `tts::neural_contract` owns pure descriptor/text/control validation;
`tts::neural` reconstructs frozen documents and owns bounded local process/model I/O. The
production module resolves the same text predicate after pronunciation replacement. Existing
receipt/recovery/audio modules inspect scope, uncertainty and actual WAV bytes. The explicit
`probe-neural-tts` CLI owns create-only private records. No provider SDK enters Script IR/domain
rules; the fixed Python helper is an infrastructure dependency. No SQL query/index/read model
changes; the existing [production read assessment](casting-production-inputs.md) still applies.

## Assurance ledger

| Claim | Failure mode | Oracle / intended evidence |
| --- | --- | --- |
| Exact provider/model/voice/language/default controls | Upstream silently falls back, clamps or drops an intent | Pure field-mutation examples and pinned helper preflight |
| Same text rule after pronunciation and at invocation | Replacement creates unsupported markup/Unicode | Exact field finding and direct request rejection |
| Full frozen document reconstruction | Forged text/digest or another revision receives audio | Scope/revision/provider mutation examples |
| Production refused before I/O | Research grants or caller assertions become dispatch authority | Production-purpose call with invalid paths; exact refusal |
| Model/runtime/preset preflight | Mutable/wrong/missing files impersonate selected engine | Artifact hashes/types/versions and helper regressions |
| Bounded child, stdout/stderr, RSS and concurrency | Hang, resource blow-up, secret echo or leaked child | Fault injection against synthetic process fixtures |
| Create-only intent precedes execution | Crash erases possible execution or repeats overwrite evidence | CLI failure receipt/destination-refusal regression |
| Complete nonempty 24 kHz PCM/hash | Empty/truncated/wrong-format output appears playable | Existing inspector and neural profile/receipt tests |
| Audio is research evidence only | Receipt inspection launders audio into an accepted asset | Serialize-only audit assessment and production refusal |

Each implemented claim must have fresh evidence below before being reported as verified.
Mocked process WAVs are synthetic signals, not neural speech. The receipt remains caller-reported,
and offline audit remains `offline-inspection-only-no-dispatch-or-acceptance`.

## Selected profile and identities

- Provider: `vieneu-nano-local`; model:
  `vieneu-nano-aba295eb96a6fa6003ebe417cc1f2802a7adc1dc`.
- Adapter: `vieneu-nano-local-v1`; output contract: `wav-pcm-s16le-mono-24000-vieneu-v1`;
  inspection profile: `cantos-vieneu-nano-tts-pcm-1`.
- Voices: `duc-tri` → Đức Trí, `my-duyen` → Mỹ Duyên. These are publisher presets, not cloning
  requests or proof of an independently verified speaker-consent record.
- Languages: `vi`/`vi-VN`; rate 1000 permille, pitch zero, explicit neutral and zero intensity.
  The engine's speed/steps/guidance/noise controls and emotion/tag mechanisms are not additional
  supported Cantos controls. Unsupported required intent blocks before inference.
- Fixed recipe: CPU, two intra-op threads/one inter-op thread, 16 flow steps, guidance 3,
  sway 0, speed 1, seed equal to the zero-based chunk index. Exact text normalization/chunking and PCM conversion are pinned
  helper/SDK behavior; inspect the source rather than assuming high-level SDK defaults.
  Every chunk's duration prediction must be finite, positive and at most 15 seconds before
  any inference; the guard adds one text/duration graph evaluation per chunk to timings.
- Input ≤2,000 NFC UTF-8 bytes; stdout WAV ≤16 MiB, duration ≤120 seconds, stderr ≤128 KiB;
  total deadline/CPU budget ≤120 seconds. One cooperative child per OS user under advisory lock.
  macOS RSS is sampled and an observed over-limit child is killed/reaped; sampling can miss
  transient peaks and is not a hard physical quota. Consecutive inspection failures receive
  at most 50 ms polling grace with deadline/pipe checks still active; persistent failure kills/reaps.
  Scheduler delays can extend observation intervals.

Research output has no cache/accepted-artifact interface. Frozen rights/budget/approval records
remain unchanged; the dispatch refusal is independent of a frozen planning snapshot. Runtime and
model preflight assume a private stable installation; native dependency identity and same-owner
replacement during use remain explicit limitations before production use.

## Resource coordination and execution

Base: freshly fetched `origin/develop` at `83eab6061554539193e4d61f7d267de2b55b095a`, clean at
start. Branch: `feat/local-neural-tts`; [draft PR #26](https://github.com/loveoverflowcom/cantos/pull/26)
targets `develop`. Tested implementation commit: `ac9d3bbb883766de47e5c988a4543e3936c719a3`;
subsequent documentation records pin that source and link the PR.
[Machine-readable evidence](local-neural-tts-run.json) records exact tested helper/adapter/CLI
and binary digests, all 21 installed package versions/wheel hashes, measurements and input/WAV
hashes. Host: Apple M4, 10 CPU cores, ARM64, 24 GiB RAM, macOS 15.6 (24G84), CPython 3.13.5,
Rust 1.87.0. No OS/platform emulation was used.

Initial Data-volume free space was about 1.4 GiB. The parent completed cleanup and granted the
Cantos heavy-work window with about 6 GiB free while MyVa stayed on light work. Builds and sample
invocations were serial, with no concurrent MyVa benchmark or build. The pre-existing iCloud
`bird` process used approximately one CPU core throughout; measurements include that background
load. All heavy work finished with about 3.2 GiB free and no remaining Cantos build, synthesis
or test PostgreSQL process. Cantos deleted no existing evidence, model, source or other project's
cache. The runner stopped its newly initialized disposable cluster.

The model's six exact files total 281,756,486 bytes (268.704 MiB), all matching pinned SHA-256.
The isolated runtime uses 284,249,853 regular-file bytes / 293,680 KiB allocated; the model uses
275,168 KiB allocated. Combined retained installation: **555.515625 MiB allocated**, within the
600 MiB budget. The earlier 120–250 MiB runtime estimate was low. This footprint excludes the
existing Python framework interpreter, compiler outputs, run evidence and download temporaries.
The full 21-package wheel manifest is in the JSON evidence; no Torch, CUDA, cloning, training,
Gradio, librosa or optional sound-file library was installed.

Installation used the existing framework Python to create private ignored
`target/neural-tts-runtime`, then these two official-PyPI commands (absolute interpreter path
abbreviated here):

```sh
python -I -m pip --isolated --disable-pip-version-check install --index-url https://pypi.org/simple --no-cache-dir --only-binary=:all: --report target/neural-tts-core-install.json numpy==2.3.3 onnxruntime==1.24.4 sea-g2p==0.9.1 huggingface_hub==0.35.3
python -I -m pip --isolated --disable-pip-version-check install --index-url https://pypi.org/simple --no-cache-dir --only-binary=:all: --no-deps --report target/neural-tts-sdk-install.json vieneu==3.8.3
```

Only the six pinned public model artifacts were downloaded from the exact Hugging Face revision
using TLS and the installed standard CA bundle. A first framework-Python certificate-verification
failure occurred before any model bytes were saved; using the venv's installed `certifi` CA bundle
resolved it without disabling TLS. The restricted training dataset was not downloaded, and no
new explicit access agreement, credential, paid API or cloud synthesis was used. Model files and
private records remain under ignored local paths, outside Git.

## Actual versus mocked results

**Actual installation and synthesis: PASS.** The opt-in live test invoked the final embedded
helper serially for all six original fixtures, using the real CPU ONNX graphs and named presets.
Each output passed Rust's distinct neural WAV profile, scoped receipt audit and an independent
Python `wave`/SHA-256 check. Each contains nonzero signed 16-bit mono PCM at 24,000 Hz. This
establishes structurally playable audio bytes; actual playback/audition remains NOT_RUN.

| Original fixture | Preset | Audio seconds | Cold CLI seconds | RTF | Sampled child RSS MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| Narration | Đức Trí | 9.220 | 6.688 | 0.725 | 913.797 |
| Question | Mỹ Duyên | 7.680 | 6.380 | 0.831 | 974.484 |
| Exclamation | Đức Trí | 7.660 | 6.421 | 0.838 | 928.672 |
| Proper names | Mỹ Duyên | 7.840 | 6.455 | 0.823 | 918.984 |
| Vietnamese tones | Đức Trí | 8.500 | 6.497 | 0.764 | 894.000 |
| Moderate dialogue | Mỹ Duyên | 18.840 | 7.320 | 0.389 | 974.172 |

Exact frame counts, sizes, checksums, nonzero PCM peaks and both RSS observations appear in the
JSON evidence. The largest `/usr/bin/time -l` maximum RSS was 1,023,328,256 bytes (975.922 MiB).
The Rust monitor's sampled child maximum differs slightly from that tool's observation. Neither
measurement proves the instantaneous physical peak or establishes a hard macOS memory quota.
This is one cold invocation per fixture, not a statistical, warmed or steady-state benchmark.

The live test passed in 40.00 seconds. It retained frozen documents, conservative intents,
success receipts, audits, WAVs, run observations and raw `time -l` files in the private local run
`target/neural-tts-evidence/85f8c3d4-6be8-4d4c-baa6-fde7a535d5ca/`. The final binary SHA-256 was
`d45c4b35db590c9c4d09d6cc4983c6056622d0d745588de901e76a4f4f56b2cd`; its embedded helper bytes
matched the reviewed helper SHA-256
`6f11ba4c33ca1733d91b9aeffef718affdedec198d63b9158b1a5003cd9c415a` before execution.
The later PostgreSQL runner rebuilt the same source with `cargo build -p cantos-server --bins`;
that dev-profile binary has a different checksum recorded separately in the JSON evidence.
It was built after the six samples and is not their executable fingerprint.

Every Cargo command used `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0`. The actual live command also used the private absolute Python/model
paths, `--locked --offline`, and:

```sh
cargo test -p cantos-server --test tts_neural_receipt installed_neural_runtime_generates_private_original_vietnamese_samples --locked --offline -- --ignored --nocapture
```

Timing includes frozen-document handling, model hashing, fresh interpreter/session loading,
all chunk duration preflights/inference, monitoring and private records. Two intra-op CPU
threads, one inter-op thread and disabled ORT thread spinning bound the recipe. No GPU fallback
was used. Publisher speed claims describe a different machine/profile, not this measurement.

**Failed real attempts are preserved.** The first failed during an import-time filesystem
capability probe (6.74 seconds, 540,770,304 bytes maximum RSS); an inference-disabled diagnostic
identified the denial. The import bootstrap hook now translates blocked filesystem mutations
to `PermissionError` so the dependency can recover; it permits no writes, network or child
processes. The full guard is added before graph loading. Its native dependencies remain trusted;
Python audit hooks are not an OS sandbox.

The second failed with `ResourceInspectionFailed` (7.50 seconds, 926,056,448 bytes maximum RSS),
not the 4 GiB threshold. A macOS exit-transition race is a plausible inference, not proven by the
log. The bounded 50 ms inspection-grace policy and recovery/refusal/reap tests were added before
an explicit new manual research run. Both original uncertain failure receipts/intents remain
unchanged in their UUID directories, with no accepted output asset and no automatic retry.
The old second attempt's possible inference execution remains unresolved; no exactly-once claim
is made. The owned child was stopped/reaped before subsequent explicit research execution.

**Listening: NOT_RUN.** No named evaluator has auditioned these renders. Pronunciation,
naturalness, emotional delivery and creator acceptance cannot be inferred from valid PCM. SEA
normalization can turn terminal question/exclamation punctuation into a period for sentences
shorter than five words; the adapter does not promise expressive punctuation or emotion.

## Checks and review

| Check executed against this source | Result |
| --- | --- |
| Repository checker, local links/JSON/skills | PASS |
| Python discovery with `TMPDIR=/private/tmp` | PASS, 103 tests including 23 helper regressions |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS |
| `cargo test --workspace --locked --offline` | PASS; opt-in database/model tests reported separately |
| Explicit live neural test above | PASS, six real WAVs and receipts |
| `python3 scripts/reference_script_ir.py` | PASS, independent canonical/digest goldens |
| `TMPDIR=/private/tmp python3 scripts/mutate_script_ir.py` | PASS, 9 mutants killed, none survived/timed out/unviable |
| `cargo build --locked --offline --bin inspect-wav` then WAV oracle | PASS, five PCM formats against Python wave/hashlib; ffprobe absent |
| `python3 scripts/test_postgres.py` with PostgreSQL 17 binaries on PATH | PASS, newly initialized disposable cluster, actual HTTP/app-role/restart/replay tests |
| Studio WASM Clippy, locked/offline, warnings denied | PASS |
| `NO_COLOR=true trunk build --locked` from `apps/web` | PASS |
| `git diff --check` | PASS |

Native tests cover admission/provider/model/voice/controls/text bounds; frozen scope and
pronunciation replacement; missing/wrong model/runtime; process crash, timeout, stdout/stderr
limits, RSS limit and transient/persistent inspection faults with kill/reap; cross-process
concurrency despite different temporary directories; corrupt/empty/truncated/wrong-rate WAV
and checksum mismatch; wrong dialogue/revision; no production dispatch; and create-only paths,
symlink refusal and output records anchored after directory rename. Synthetic fault WAVs are
not neural speech. Production-purpose refusal occurs before any model/process I/O even for a
planning-approved snapshot. Existing eSpeak source behavior was unchanged and its tests passed.

Two test defects were corrected before the passing rerun: a temporary string borrow in a unit
fixture and a pronunciation test that selected the wrong sorted character binding. The live
failures above prompted the guarded import initialization and bounded RSS observation fixes.
All native Clippy/tests and Python regressions were rerun after the final source changes.

Independent read-only agent review covered process safety, pure input validation, private
paths, rights/production refusal, receipt scope and helper identity. Its confirmed temporary-path
lock-namespace finding was fixed with a stable per-user namespace and a two-process test.
Follow-ups found no remaining actionable source defects in directory-descriptor output handling,
import guard order or resource inspection grace. Reviewer helper/adapter digests are recorded in
machine evidence; review did not independently rerun models or heavy checks. Static review is
neither merge approval nor runtime proof.

No CI workflow was added or expanded. Studio runtime interaction/accessibility, mobile runtime,
audio quality and end-to-end production are NOT_RUN; this slice changes none of those user flows.
The browser build establishes compilation only. A historical #25 pass is not used as evidence
for these changes.

## Residual risk and next task

Production training/voice provenance and named-listener quality remain unresolved. Runtime
native dependencies and same-owner hostile replacement are outside the helper's audit-hook
trust boundary. macOS RSS is sampled, and cold-run results have one observation per fixture with
background host load. Research PCM/receipts cannot become accepted production assets. No current
permissions, reservation, durable job/retry/reconciliation, paid cost, publication or mobile
production evidence is established.

Next recommend named Vietnamese audition and applicable model/voice/training provenance review,
then the separate unfinished commercial #6 provider/terms/budget/live-sample decision within its
authorized scope. Do not advance automatically to #7. The requester later directed this PR's merge into `develop` (2026-10-11); it does not close #6.
