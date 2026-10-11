# Neural TTS boundary review and conditional delivery

Review date: 2026-10-11. [Draft PR #26](https://github.com/loveoverflowcom/cantos/pull/26)
targets `develop`; [issue #6](https://github.com/loveoverflowcom/cantos/issues/6) remains open.
**Production integration and merge: blocked.** Listening is NOT_RUN and production rights are
NO_GO. Confirmed source defects were fixed and tested independently of those acceptance gaps.

## Scope and assessment

Freshly fetched base/merge-base: `83eab6061554539193e4d61f7d267de2b55b095a`.
Reviewed PR head: `f75402aaab5ee003d12e2de2da148106c2e07014`; branch `feat/local-neural-tts`.
The checkout was clean before review. PR scope used `BASE...HEAD`; the five substantive changes
below were additionally inspected as each commit's parent-to-commit diff. Merge commits and
documentation-only follow-ups are not counted twice. These are the five recent changes most
relevant to the requested TTS/production/WAV boundaries, not the five newest unrelated changes.

| Commit | Reviewed concern |
| --- | --- |
| `ac9d3bbb883766de47e5c988a4543e3936c719a3` | Neural research invocation, fixed helper, model preflight, private CLI and receipts |
| `8705dda862f4a293448df1c27c1bc708ec295695` | Existing eSpeak process boundary, receipt association and retry uncertainty |
| `b693fd11e8974812dfba4ffd37ea9ca363b1c9c8` | Frozen exact production inputs, applicable rights, approval invalidation and persisted snapshots |
| `efa5ae1ca1fb8258e1167159d908bb405452c3de` | Stable-ID Script IR diff and bounded read-only CLI admission |
| `4b2f78688048cf73e65d7b9fd11e9ae4fe42bdd4` | WAV parser, PCM/hash/profile inspection and CLI file admission |

Method: Cantos code-review, engineering, production-pipeline, Script IR and publication rules;
a separate read-only review phase preceded edits. No additional reviewer agent was invoked.
Assessment after fixes: **changes-requested for production/merge**, due to the two required
evidence gaps below. This does not convert a missing audition into a code defect.

Coverage is **partial** across those commits: the declared backend/TTS/CLI paths were read,
including `tts/{neural,neural_contract,local,attempt,audio,receipt}.rs`,
`production.rs`, persisted production snapshots, relevant migration/contracts, Script IR diff
and WAV modules, `scripts/vieneu_nano_runtime.py`, probe/inspection binaries and relevant
evidence/ADR records. Representative tests and the neural suites were inspected; all required
local suites were run. Production HTTP transport received targeted boundary inspection.
The complete Leptos UI diff in the casting commit, unrelated editor implementation, historical
tests line-by-line, CMP/platform code and unrelated commits were not reviewed exhaustively.
There are no new UI, mobile, CI, schema, paid-provider, worker or publication changes in this
fix. WASM compilation below is separate from runtime interaction/accessibility evidence.

## Findings and fixes

**[P2] Writerless FIFOs block before file admission** — class: confirmed defect; category:
logic; confidence: high. `inspect-wav::read_bounded` and `diff-script::read_bounded` opened the
path with `File::open` before checking the opened descriptor's regular-file type. On Unix, a
FIFO without a writer blocks in open and never reaches that check, contradicting bounded
read-only input admission. The defect originates in the WAV/diff CLI commits, not the neural
model. Actual binaries reproduced the hang for WAV input and either diff operand; each was
killed/reaped by a one-second external deadline.

The new actual-CLI regression failed before the fix (`CLI blocked on a writerless FIFO`,
2.01 seconds). Both CLIs now open with Unix `O_NONBLOCK` and then inspect that same descriptor.
Regular-file symlink behavior is retained. The regression now rejects both the FIFO and its
symlink, in all three operand positions, with exit 2, no JSON/stdout and no mutation. Its child
guard kills/reaps on failure. No generic abstraction or new dependency was introduced.

**[P2] Expired preflight deadline still attempts neural process creation** — class: confirmed
defect; category: logic; confidence: high. `tts::neural::unix::execute_with_observer` configured
and spawned the child before its first monitor-loop deadline check. Model hashing, request
encoding or scheduling can consume the caller's remaining wall budget before that boundary.
Inference must be refused when the deadline is already expired, rather than starting work and
immediately killing it.

The new expired-deadline regression uses an absent launcher to distinguish refusal from even
an attempted spawn. It failed before the fix with `ProcessStartFailed` instead of `TimedOut`.
A deadline check immediately before `.spawn()` now makes it pass; no child/input evidence is
created. The public timeout classification remains conservative `MayHaveExecuted`; no retry
permission or exactly-once assertion was introduced. This is admission hardening, with the
effective normalization/inference/PCM recipe, provider/model/voice and adapter identity unchanged.

The separate fix/test commit is `c6025cda01877ef4fb55211e92540aecfb69ec00`. It also supplies an
opt-in four-probe listening fixture; the original six texts and existing eSpeak source remain
unchanged. Full source fingerprints and observations are in the
[machine-readable record](local-neural-tts-review-run.json).

**[P2] Actual listening acceptance is missing** — class: required evidence gap; category:
evidence; confidence: high. Required by the requester's review profile and #6's language/voice
verification. No audio-perception tool is available in this session; model and human scores are
both NOT_RUN. The [listening handoff](local-neural-tts-listening.md) supplies ten actual WAVs,
an explicit six-dimension 1–5 rubric, null scorecards and passage/timestamp requirements.
Installed normalization actually replaces terminal `?`/`!` with `.` for the two <5-word texts.
The resulting intonation has not been heard. A named Vietnamese-speaking listener and owner
acceptance must settle this gate, including consistency across each preset's five samples.

**[P2] Production provenance/rights clearance is missing** — class: required evidence gap;
category: evidence; confidence: high. Required by AGENTS.md's provenance/rights rule, ADR 0009,
#6 and the explicit conditional-integration request. The publisher's commercial grant and
speaker-consent assertion are present, but the training lineage and applicable rights have not
been independently resolved for the intended production scope. Details follow. Local research
authorization or a frozen planning approval cannot supply missing third-party rights.

## Assurance ledger

| Claim / owner | Required by | Fresh evidence / level | Disposition |
| --- | --- | --- | --- |
| Exact model, preset, language, neutral/default controls and effective text / neural contract, production | ADR 0009; production contract | Field mutations, pronunciation replacement and bounded text examples / example-tested | held within admitted profile |
| Full document/input/dialogue binding / neural request and receipts | AGENTS.md correctness; TTS receipt contract | Reconstructed scope/revision/provider/digest/checksum mutations; ten actual audio receipt audits / example-tested and provider-live-tested | held; not asset acceptance |
| Research grants cannot authorize production / neural invocation | ADR 0009; requester | Production-purpose refusal with invalid paths and current planning approval / example-tested | held; production dispatch disabled |
| Bounded process/file/pipe/resource admission / neural and CLIs | Requester; engineering boundary policy | FIFO/deadline red-to-green tests, model/type/hash, polling/timeout/output/stderr/RSS/reap/concurrency faults / fault-injected with fake processes | held within documented limits |
| Fixed offline helper and trusted installation / Python infrastructure | ADR 0009 | Guard/recipe regressions (23 helper tests inside Python suite), model/SDK pins and real four-probe synthesis / example-tested and provider-live-tested | held for research; native trust/same-owner race remain |
| Intent precedes uncertain execution; destinations cannot overwrite / private probe | AGENTS.md duplicate-delivery rules; receipt contract | CLI intent/failure/no-overwrite tests and retained failed/real attempts / example-tested and provider-live-tested | held; no automatic retry/acceptance |
| Existing eSpeak behavior / local adapter | Compatibility requirement | Existing fake-process/receipt tests plus actual three-voice generation and repeat-destination refusal / example-tested and provider-live-tested | held; listening NOT_RUN |
| Frozen approval/rights/budget invalidation and persistence / production | Production contract; #5 | Fresh disposable PostgreSQL, HTTP process/database restart, immutable replay and current eligibility checks / integration-tested | held for planning; zero generation/cost attempts |
| Actual audible tones/names/prosody/naturalness/identity / quality owner | Requester; #6 | No audio perception; actual WAVs and rubric only / NOT_RUN | gap-required |
| Exact production training/preset rights / rights owner | AGENTS.md; ADR 0009; requester; #6 | Publisher cards/license metadata read; unresolved corpus relation and independent rights / source-inspected | gap-required; NO_GO |
| Steady-state speed or hard physical-memory quota / research measurements | No such new claim | Cold per-fixture observations; sampled RSS only | not-applicable: neither claimed |
| Worker crash/retry/cancellation/leases/cost reservations and publication acceptance / later pipeline | Later #7–#11 | No worker, billing, asset acceptance or publication added | not-applicable: later scopes, not silently passed |

## Official license and provenance recheck

The exact Nano model card at revision `aba295eb96a6fa6003ebe417cc1f2802a7adc1dc` was fetched
as 15,403 bytes, SHA-256 `b384262645214848872036daae39265a268b86c4403f44d6ff4caddf5a183df1`.
It states that Apache-2.0 covers graphs, configuration, constants and bundled preset assets;
permits commercial preset-generated output; and asserts speaker/rightsholder consent. It also
says the detailed internal corpus pipeline is undisclosed. A standalone `/LICENSE` at that
revision returned 404; the explicit card grant and SDK license were still read, rather than
equating that response with an absent grant.
[Pinned card](https://huggingface.co/pnnbao-ump/VieNeu-TTS-v3-Nano/blob/aba295eb96a6fa6003ebe417cc1f2802a7adc1dc/README.md),
[SDK 3.8.3 metadata](https://pypi.org/project/vieneu/3.8.3/).

The model's dataset association points to VieNeu-TTS-1000h. Its official page declares
CC-BY-NC-4.0 and gated research/education access. This does not by itself prove how a dataset
restriction applies to released weights/output; that legal relationship is unresolved, not
adjudicated here. The publisher's affirmative artifact/output grant is preserved in the
record. Cantos's production gate remains NO_GO until the rights owner can record applicable
clearance for the pinned weights/codec/preset lineage and intended commercial use, including
the relationship to that corpus and the consent scope for Đức Trí/Mỹ Duyên.
[Model association](https://huggingface.co/pnnbao-ump/VieNeu-TTS-v3-Nano),
[official dataset terms](https://huggingface.co/datasets/pnnbao-ump/VieNeu-TTS-1000h).
No gated agreement was accepted and no dataset was downloaded.

The five Piper cards were freshly fetched at the documented voice revision
`c10ece1aade47bb51c153c893d14e5bf8e5b7117`. Vivos declares CC-BY-NC-SA-4.0 and scratch training;
VAIS declares CC-BY-4.0 but finetuning from Lessac, whose card links separate terms;
25hours_single declares an unknown dataset license and finetuning from Ryan, whose card declares
CC-BY-NC-SA-4.0. The Lessac terms page timed out on this recheck; its historical noncommercial
restriction is recorded in the earlier [comparison](local-neural-tts-research.md), not freshly
verified by that timeout. The maintained Piper engine's GPL grant does not clear individual
voices or their pretraining lineage. No Piper model/runtime was installed or used as a workaround.
[Engine license declaration](https://raw.githubusercontent.com/OHF-Voice/piper1-gpl/639388b6317fc4731e91d53da42aea68fd4166ff/setup.py),
[Voice cards](https://huggingface.co/rhasspy/piper-voices/tree/c10ece1aade47bb51c153c893d14e5bf8e5b7117/vi/vi_VN),
[engine voice policy](https://github.com/OHF-Voice/piper1-gpl/blob/639388b6317fc4731e91d53da42aea68fd4166ff/docs/VOICES.md).

All 21 installed runtime package versions matched the existing wheel manifest. The actual
installed publisher metadata and license/notice inventory were inspected, without installing
anything. These are software declarations, separate from training/voice rights:

| Component version | Declared software license |
| --- | --- |
| vieneu 3.8.3; sea-g2p 0.9.1; flatbuffers 25.12.19; hf-xet 1.7.0; huggingface-hub 0.35.3; requests 2.34.2 | Apache-2.0 |
| onnxruntime 1.24.4; charset-normalizer 3.5.2; filelock 4.1.0; PyYAML 6.0.3; urllib3 2.8.0 | MIT |
| fsspec 2026.9.0; idna 3.20; protobuf 7.36.2 | BSD-3-Clause |
| numpy 2.3.3; mpmath 1.3.0; sympy 1.14.0 | BSD (retain each bundled notice) |
| packaging 26.3 | Apache-2.0 OR BSD-2-Clause |
| certifi 2026.7.22 | MPL-2.0 |
| tqdm 4.70.1 | MPL-2.0 AND MIT |
| typing_extensions 4.16.0 | PSF-2.0 |

NumPy and ONNX Runtime bundle additional license/third-party notices. Flatbuffers' installed
wheel has an Apache declaration but no license file in its file inventory. A future runtime
redistribution must collect the complete required notices, including bundled components; this
PR ships no runtime/model bundle. The machine record retains package-level declarations and
license-file paths. This inventory is not a complete redistribution/legal clearance.

## Fresh checks and actual delivery

All Cargo commands used `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0`; dependency checks/tests used `--locked --offline`. Python temporary
path-sensitive runs used `TMPDIR=/private/tmp`. Raw logs remain ignored under
`target/neural-review-20261011/checks/`; the machine record retains commands and counts.

| Check | Result |
| --- | --- |
| `python3 scripts/check_repository.py` | PASS |
| `python3 -m unittest discover -s scripts -p 'test_*.py'` | PASS, 103 tests including 23 helper regressions |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | PASS |
| `cargo test --workspace --locked --offline` | PASS, 342 passed, 54 explicitly ignored |
| `python3 scripts/reference_script_ir.py` | PASS, independent c1 oracle |
| `python3 scripts/mutate_script_ir.py` | PASS, 9 killed, 0 survived/timeouts/unviable |
| `cargo build --locked --offline --bin inspect-wav`; `python3 scripts/reference_wav_inspection.py` | PASS, independent Python wave/hashlib across five PCM formats; ffprobe unavailable |
| `PATH=/opt/homebrew/opt/postgresql@17/bin:$PATH python3 scripts/test_postgres.py` | PASS, 51 PostgreSQL tests plus actual CLI/HTTP/restart/replay journeys; new disposable cluster stopped |
| Studio WASM clippy with `-D warnings`; `NO_COLOR=true trunk build --locked` in `apps/web` | PASS, compilation only |
| Opt-in installed neural short-punctuation test | PASS, four real scoped WAVs; 23.51 seconds |
| Opt-in installed eSpeak three-voice test | PASS, three real scoped WAVs and refusal to overwrite; 0.46 seconds |
| Ten WAVs re-audited by current `inspect-tts-receipt` plus Python wave/hashlib and ZIP byte checks | PASS |
| Library upload → supported materialization → byte/xattr verification | PASS |
| `git diff --check` | PASS |
| Model/human audition, Studio interaction/accessibility, mobile runtime, production end-to-end | NOT_RUN |

The first PostgreSQL attempt hit the sandbox's loopback-bind restriction before cluster
creation; the authorized rerun used the existing native PostgreSQL 17 tools in a **new**
disposable cluster. The running native service/inherited database was not substituted.
The first short-probe timing-wrapper failure and previous neural failures remain preserved;
see the listening handoff. No test failure was relabeled a pass.

The ten-sample ZIP is 2,560,714 bytes, SHA-256
`e81ca874ed998eb46a40bf41e62c6bcb4117699e6817b57d0ff242128cbca839`.
Library identity attributes were applied and verified on both the source ZIP and downloaded
copy. Their bytes match exactly, and every archived WAV matches its source/receipt. Delivery
details are retained privately under ignored local evidence and supplied to the requester in
chat; the public evidence record contains no Library file ID or download URL. Delivery does
not establish model audition or public publication.

Automatic approval review rejected the first PR-body update because it would expose the
Library download link to the GitHub audience without authorization. The current public
documents and PR body omit that link and its file ID. The previous documentation commit
already contains the old reference in Git history; a follow-up commit cannot erase history.
No force push or file deletion was performed. The requester must decide whether to authorize
that disclosure or authorize retiring/replacing the Library delivery before claiming the
historical reference has been resolved.

## Conditional next action and residual risk

First complete a named listening review and the applicable rights/provenance record for the
existing pin; keep the research refusal throughout. Only after both gates pass should a
separately scoped minimal production-dispatch connection be implemented and tested against
current eligibility, exact snapshot identity and receipts. No worker/billing/mix/publication
expansion is authorized by this review. PR #26 remains draft/unmerged and #6's commercial/full
acceptance remains outstanding. GitHub owns execution status; the queue owns this order.

Residual risk: no listening or production rights clearance; normalization may erase short
question/exclamation punctuation; sampled RSS/advisory locking/Python audit hooks are not hard
OS isolation; native dependencies and same-owner installation replacement remain trusted;
partial cross-commit/UI coverage and no mobile/runtime-accessibility/production run. Initial
free disk was about 3.6 GiB and about 2.7 GiB was observed after checks; no existing target
evidence/model/audio was deleted and no new model/runtime was downloaded.
