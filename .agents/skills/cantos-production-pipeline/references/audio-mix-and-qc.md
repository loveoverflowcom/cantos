# Audio mix and QC

> **Who renders (ADR 0018).** Cantos neither mixes nor renders. Read the mix plan and profile as the *standard* an external producer delivers against; Cantos's part is measuring submitted audio and recording checksum-bound QC facts.

> **Scope.** Model the audio side of a Cantos run as typed values and pure functions around a thin
> tool shell: a versioned audio profile, a deterministic mix plan (placement, pauses, ducking,
> fades), measurements as facts, technical QC as a pure function of those facts, append-only QC
> records bound to an artifact checksum, a clear line between technical results and editorial
> review, synthetic test signals generated in code, listening review as its own evidence, and
> hostile-input robustness for every decoder and external tool. Use when touching mixing code, the
> audio or mastering profile, loudness or peak handling, QC findings or records, sound-asset
> handling, an audio tool invocation or a probe of untrusted audio.

The product rules live in [pipeline § Audio processing and QC](../../../../docs/product/production-pipeline.md#audio-processing-and-qc)
and [business rules 20–21](../../../../docs/product/business-rules.md#review-and-publication) (QC
covers the technical profile and editorial concerns; technical checks support but never replace
human review; approvals bind a specific render). [Work plan 030](../../../../docs/work-plan/030-mix-review-and-publish.md)
leaves the numeric targets open on purpose: loudness, peak and delivery format are chosen after
listening to representative drama content. This reference therefore gives **types and methods, not
numbers**. Every type, function and tool name below is a **proposal**; an audio tool such as
ffmpeg is a candidate that needs a recorded decision and a pinned, verified version.

## 1. Who decides what

| Decision | Lives in | Why |
|---|---|---|
| the target numbers and tolerances | a versioned **profile** value | a change must be visible, reviewable and part of the cache key |
| where every line, pause and cue sits in time; ducking and fades | pure `plan_scene_mix` | deterministic, testable without audio files |
| turning a plan into bytes | the tool shell behind an `AudioTool` port | the only part that touches a binary, a process or the disk |
| what a measurement says | pure `evaluate_technical_qc` over measurements | the verdict is data, reproducible from the facts |
| whether the drama sounds right | a named human reviewer | no metric certifies performance, pronunciation or balance |
| whether a render may be published | [`cantos-publication`](../../cantos-publication/SKILL.md) | gates and approvals are not this skill's decision |

Do not silently change dialogue text to fix an audio problem
([pipeline](../../../../docs/product/production-pipeline.md#audio-processing-and-qc)). Adjust the
profile or the plan; the text and its `SpokenContent` stay untouched.

## 2. The audio profile is a versioned value

```rust
// Illustrative and proposed. Numbers are decided by listening tests and recorded in an ADR.
pub struct AudioProfile {
    revision: AudioProfileRevision,
    processing_rate: SampleRateHz,
    channels: ChannelLayout,
    master_format: MasterFormat,
    delivery_formats: NonEmpty<DeliveryFormat>,
    loudness: LoudnessTarget,       // integrated target, tolerance and the standard measured against
    true_peak_limit: TruePeakDbtp,
    timing: TimingPolicy,           // pause lengths, scene gaps, cue and fade bounds
    tolerances: QcTolerances,       // duration drift, silence, clipping run length
}

impl AudioProfile {
    pub fn new(parts: ProfileParts) -> Result<Self, ProfileError>;   // rejects impossible combinations
}
```

- **Newtypes carry the unit**: `LoudnessLufs`, `TruePeakDbtp`, `SampleRateHz`, `DurationMs` cannot be
  swapped by accident ([`types-as-proofs.md`](../../cantos-engineering/references/types-as-proofs.md)).
- **Revision, not mutation.** Editing a target creates a new `AudioProfileRevision`. The revision is
  an input of the mix, master and rendition fingerprints, so a profile change invalidates mixes
  without regenerating unchanged speech
  ([`fingerprints-and-invalidation.md`](fingerprints-and-invalidation.md#3-mix-master-and-rendition-fingerprints)).
- **Mix in a processing format, encode delivery formats last.** Never mix in a lossy format.
- **Name the measurement standard** inside the target. "−x LUFS" without the algorithm and
  revision is not a number anyone can reproduce.
- **Frozen into the run.** The run's input document holds the profile revision it was frozen with
  ([durable jobs § 1](durable-jobs.md#1-create-a-run-from-frozen-inputs)).

## 3. A mix is a pure plan; the tool is a thin shell

```rust
// Illustrative and proposed.
pub struct MixInputs<'a> {
    pub speech: &'a [PlacedSpeech],       // accepted artifact checksum, duration, speaker, order
    pub cues: &'a [ResolvedCue],          // asset revision and checksum, anchor, gain, fades
    pub profile: &'a AudioProfile,
}

pub fn plan_scene_mix(inputs: &MixInputs<'_>) -> Result<MixPlan, MixPlanError>;
```

`MixPlan` is data: a timeline of clips with start and end, gain envelopes, fades and the ducking
rule that lowers music or ambience under speech. Because it is pure, its laws are cheap to state
and to check:

- every speech clip appears exactly once, in script order, with its deliberate pause preserved;
- no cue starts before its anchor resolves or runs past the scene end without a stated fade;
- fades fit inside their clip; gains stay inside the profile's bounds;
- the plan's total duration equals the sum of its parts and is the same on every run;
- the same inputs give the same plan (determinism), and reordering unrelated cues changes
  nothing else.

The shell then renders the plan:

```rust
// Illustrative and proposed. The port is named for the capability, not the binary.
pub trait AudioTool {
    fn render(&self, plan: &MixPlan, sources: &SourceBytes) -> Result<Rendered, ToolFailure>;
    fn measure(&self, bytes: &[u8], profile: &AudioProfile) -> Result<Measurements, ToolFailure>;
    fn identity(&self) -> ToolIdentity;     // name + exact version: part of every fingerprint
}
```

- **Tool identity is an input.** A new tool version can change bytes for identical inputs, so it
  changes the fingerprint and the QC record, like an adapter version
  ([provider adapters § 8](provider-adapters.md#8-adapter-versions-are-inputs)).
- **Subprocess discipline.** Build the argument list from typed values, never a shell string; set
  a timeout, cap captured output, run in a private temporary directory, drop network access if the
  platform allows, and treat a nonzero exit as `ToolFailure` with sanitized stderr.
- **Idempotent artifacts.** The output is stored under a content-derived key and verified by
  checksum before any artifact row exists, exactly like provider audio
  ([provider adapters § 6](provider-adapters.md#6-validate-the-bytes-before-acceptance)).

## 4. Measurement is a fact; QC findings are a pure function

Separate *what was measured* from *what that means*.

```rust
// Illustrative and proposed.
pub struct Measurements {
    pub container: ContainerFacts,       // format, rate, channels, codec
    pub duration: DurationMs,
    pub integrated_loudness: Option<LoudnessLufs>,
    pub true_peak: Option<TruePeakDbtp>,
    pub clipped_samples: ClipRuns,
    pub silent_spans: Vec<Span>,
    pub tool: ToolIdentity,
}

pub fn evaluate_technical_qc(
    measured: &Measurements,
    expected: &ExpectedAudio,        // duration range from the plan, profile, dialogue coverage
    profile: &AudioProfile,
) -> Vec<QcFinding>;                // every finding, sorted; never the first one only
```

`None` is not zero: a measurement the tool could not take (`integrated_loudness: None`) becomes a
finding (`MeasurementUnavailable`), never a pass. Findings are a closed enum with data:

```rust
pub enum QcFinding {
    AssetMissing { dialogue: DialogueId },
    AssetEmpty { artifact: ArtifactChecksum },
    Corrupt { artifact: ArtifactChecksum, defect: AudioDefect },
    DurationOutOfRange { expected: DurationRange, measured: DurationMs },
    TruePeakExceeded { limit: TruePeakDbtp, measured: TruePeakDbtp },
    LoudnessOutOfTarget { target: LoudnessLufs, measured: LoudnessLufs },
    ProfileMismatch { field: ProfileField },
    DialogueNotCovered { dialogue: DialogueId },
    MeasurementUnavailable { which: MeasurementKind },
    TextFidelityUnverified { dialogue: DialogueId },     // surfaced uncertainty, see § 6
}

impl QcFinding {
    pub fn severity(&self) -> Severity;    // exhaustive match: a new variant forces a decision
}
```

- **All findings at once.** Studio shows every problem in one pass; the first `?` hides the rest.
- **Dialogue coverage is checked against the plan**, not against a file count: every dialogue in the
  frozen inputs maps to an accepted speech artifact placed in the mix.
- **Severity is a pure match** with no wildcard arm, and blocking findings are exactly the ones
  [`cantos-publication`](../../cantos-publication/references/approvals-and-gates.md) turns into a
  `QcBlockingFinding` blocker.
- Tolerances live in the profile, so "close enough" is reviewed data, not a literal in a function.

## 5. QC records are append-only facts bound to a checksum

```rust
// Illustrative and proposed. Fields are private; the constructor sorts findings.
pub struct QcResult {
    id: QcResultId,
    kind: QcKind,                    // Technical | Editorial
    subject: ArtifactChecksum,       // the exact bytes evaluated
    input_revision: RunInputDigest,
    profile: AudioProfileRevision,
    recorder: Recorder,              // Tool(ToolIdentity) | Reviewer(ActorId)
    findings: Vec<QcFinding>,
    recorded_at: Timestamp,
}
```

- **Write once.** A new render or a re-evaluation appends a new result; nothing is edited. History
  stays queryable.
- **Bound to the bytes.** The publication gate joins a result to a candidate by `subject`; a
  result "passed" without a checksum is not evidence for this candidate
  ([approvals and gates](../../cantos-publication/references/approvals-and-gates.md)).
- **Reuse keeps its QC.** A scene mix reused unchanged keeps the result for its checksum; a new
  render does not inherit one. This is what makes "QC approval predates a new render" computable.
- **Execution status is not QC status.** A `succeeded` step is not a QC pass; a `completed` run is
  not an approved render ([durable jobs § 2](durable-jobs.md#2-three-state-machines-one-meaning-each)).

## 6. Technical pass is not editorial approval, and uncertainty is shown

Automated checks catch missing, empty or corrupt output, unexpected durations, clipping and peak
violations, profile mismatches and incomplete coverage. They cannot judge speaker identity,
pronunciation, performance, balance of speech against music and effects, or the final listening
experience. Those belong to an `Editorial` result recorded by a named reviewer.

- **Never certify text fidelity from an automatic alignment.** If a speech-alignment check exists
  it reports a confidence; low or unavailable confidence becomes `TextFidelityUnverified`, a
  visible finding for the reviewer, not a silent pass.
- **Editorial results use the same record shape**, with `Recorder::Reviewer(actor)`, and bind the
  same checksum, so a later re-render makes them stale by computation.
- **The evidence label is separate.** Measured values are `audio-measured` (name the tool, version
  and profile); a person listening is `audio-listened` (name the reviewer, the render checksum,
  the playback setup and the date). One never implies the other.

Use the shared [listening-review protocol](../../../../docs/production/listening-review-protocol.md)
and [template](../../../../templates/listening-review.md) for line, scene and episode evidence:
headphones **and** phone speaker, transcript-free orientation before transcript comparison,
dimension results and timestamped checksum-bound findings. `blocking` findings prevent editorial
acceptance; `advisory` findings remain recorded. No average hides a blocker, and unheard checks
stay `NOT_RUN`, never `N/A`. Thresholds stay proposed until an actual owner decision is recorded.
The manual record maps to the proposed `QcKind::Editorial` facts above; it does not implement
backend QC, measure loudness or grant rights/publication approval. Record absent profiles honestly.

## 7. Test signals are generated in code

Real drama audio cannot be committed (`.gitignore` excludes common audio formats) and a recording
cannot give an analytical answer. Generate signals in test code, where the right answer is known.

| Signal | Built by | Known answer |
|---|---|---|
| tone of amplitude *A*, rate *R*, *N* samples | a sine in code | sample peak `20·log10(A)` dBFS, duration `N / R`, nonzero energy |
| digital silence | zero samples | zero energy; detected as silent span |
| clipped square wave | alternating full-scale samples | exact count and length of clipped runs |
| truncated tone | cut mid-sample or drop the tail of a valid container | `Corrupt` / `AudioDefect::Truncated` |
| wrong format | valid container with another rate or channel count | `ProfileMismatch { field }` |
| two tones at different levels | sum with a known gain difference | ducking reduces the music by the planned amount |

- **Analytical oracles first.** Sample peak, duration, silence and clip counts have exact answers.
  Integrated loudness and true peak do not reduce to one line (filtering, gating, oversampling), so
  compare against the **published reference signals of the chosen standard**, cite the document and
  revision, and then cross-check one signal against the chosen tool. That is `differentially-tested`
  for the measurement code and `audio-measured` for a rendered result.
- **Sample peak is not true peak.** Name which one each check uses; test an inter-sample-peak
  signal if the profile limits true peak.
- **Tolerances are explicit** and come from the profile; never a bare `assert_eq!` on floats.
- **No provider audio and no real recordings in fixtures.** Provider samples used for a live run
  stay in ignored paths ([provider adapters § 7](provider-adapters.md#7-credentials-logs-fakes-fixtures-and-live-tests)).

## 8. Decoders are fuzz targets

Provider output, imported sound assets and creator uploads are untrusted bytes. Every probe,
parser, decoder and external-tool wrapper must be safe on hostile input **before** an artifact row
exists ([bytes validation](provider-adapters.md#6-validate-the-bytes-before-acceptance)).

- **Properties to hold on any input:** no panic, no unbounded allocation, no unbounded time, no
  read outside the buffer, a typed `AudioDefect` instead of a crash. Set explicit limits: maximum
  byte size, maximum declared duration and channel count, maximum number of chunks, a wall-clock
  timeout for any subprocess.
- **A fuzz target for each decoder-facing function** (cargo-fuzz is a candidate; method, corpus and
  crash handling in [`fuzzing.md`](../../cantos-engineering/references/fuzzing.md)), running with the
  resource limits above. Seed the corpus with synthetic headers and tiny valid files **built in
  code** ([§ 7](#7-test-signals-are-generated-in-code)); never seed with provider output or
  private recordings.
- **Minimized failures become named regression tests**, run in the ordinary suite.
- **Declared lengths lie.** A header that claims a huge duration or chunk count must be rejected
  by the limits, not allocated.
- **Honest label.** A clean fuzz run is `fuzz-tested`: a *robustness* statement. It says nothing about correct
  measurements, correct decoding or audio quality, and it is not a substitute for the examples in
  § 7. Report its duration, corpus origin and limits.
- The same posture applies to any parser of untrusted media metadata (cover images, tags) the
  pipeline adds later.

## 9. Rule cards

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| The profile is a versioned value and part of the fingerprint | a loudness change silently reuses old mixes | `AudioProfileRevision` in the mix key | targets as constants in a function | profile-change invalidation test | proposed · none |
| The mix is a pure plan | the mix only exists as tool side effects | `plan_scene_mix` returning data | arguments assembled inline in a shell command | plan laws as properties | proposed · none |
| Tool identity is an input | a new tool version silently changes bytes | `ToolIdentity` in fingerprint and QC | "latest ffmpeg" | fingerprint field test | proposed · none |
| Measurement is a fact; the verdict is pure | an unreproducible pass/fail | `evaluate_technical_qc(&Measurements)` | boolean `passed` from the tool wrapper | table test per finding | proposed · none |
| Unavailable is not a pass | a missing measurement reads as OK | `MeasurementUnavailable` finding | `unwrap_or(true)` | example per `None` | proposed · none |
| QC results are append-only and bound to a checksum | an approval rests on another render | `subject: ArtifactChecksum` | `episode.qc_passed = true` | stale-result join test | proposed · none |
| Technical pass is not editorial approval | a quality claim without a listener | separate `QcKind::Editorial` | merging both into one flag | review; type check | proposed · none |
| Never certify fidelity without evidence | an alignment guess reported as truth | `TextFidelityUnverified` | "alignment OK" at unknown confidence | low-confidence example | proposed · none |
| Subprocess arguments are typed and bounded | injection, hangs, memory blow-ups | argument array, timeout, capped output | `format!("ffmpeg -i {path} …")` through a shell | hostile-path example; timeout test | proposed · none |
| Decoders survive hostile bytes | a crafted file takes the worker down | limits plus a fuzz target | decode first, validate later | fuzz run; regression inputs | proposed · none |
| Test signals are built in code | unreproducible audio fixtures | analytical sine and silence | a committed recording | the generator is the fixture | proposed · none |

## 10. Evidence

| Claim | Oracle | Honest label |
|---|---|---|
| plan laws: coverage, order, pauses, fades, determinism | hand-written timelines plus generated plans | `example-tested`; `property-tested` |
| each QC finding is reachable and exact | baseline-clean signal plus exactly one defect per row | `example-tested` |
| the evaluator never short-circuits | one input with every defect, full sorted list asserted | `example-tested` |
| measurements match known signals | analytical answers; published references cross-checked with the tool | `differentially-tested` |
| a rendered scene or episode meets the profile | a named tool, version and profile on the exact render | `audio-measured` |
| the drama sounds right | a named reviewer on the named render checksum | `audio-listened` |
| a stale QC result cannot support a new render | candidate join test | `example-tested`; `property-tested` over mutations |
| probes and wrappers survive hostile bytes | fuzz target with limits ([`fuzzing.md`](../../cantos-engineering/references/fuzzing.md)) | `fuzz-tested` (robustness only; no correctness claim) |
| assertions kill defects in the planner and evaluator | mutation run on the pure modules | `mutation-tested` |
