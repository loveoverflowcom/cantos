# Listening-review protocol

Status: documented manual procedure for [issue #45](https://github.com/loveoverflowcom/cantos/issues/45).
The rubric and acceptance thresholds below are **PROPOSED** until a named owner records a
decision. This document supplies no scores, listened samples, approved threshold, backend QC
record or publication approval. The named-listener assessment in
[#6](https://github.com/loveoverflowcom/cantos/issues/6) remains **NOT_RUN**.

Use the [listening-review template](../../templates/listening-review.md) for a line, scene or
episode render. This is the canonical home of the six TTS dimensions originally proposed in the
[local neural handoff](../evidence/local-neural-tts-listening.md), with additional drama questions.
Do not create a provider-specific competing rubric. The [craft guide](audio-drama-craft.md)
owns dramatic intent; [business rules 20–21](../product/business-rules.md#review-and-publication)
own QC and approval validity. The
[audio mix and QC method](../../.agents/skills/cantos-production-pipeline/references/audio-mix-and-qc.md)
owns the proposed `QcKind::Editorial` shape and its separation from technical measurement.

## Pin the subject and the reviewer

Record these facts before playback:

- Review ID/revision, named human reviewer, Vietnamese comprehension/accent familiarity, date
  and timezone, and prior exposure to this script or render. Model/ASR output is separate evidence,
  never a substitute for the named human reviewer.
- Exact filename, private relative artifact reference, complete SHA-256 of the bytes played,
  render revision, duration/timebase and subject kind (`line`, `scene`, `episode`). For an excerpt,
  pin both its own checksum and the parent's checksum/range; never attribute an excerpt's review
  to unheard parent bytes. Each alternate encoding is a separate subject.
- Input identity: exact score/script revision and byte checksum; frozen input revision/digest
  when present, otherwise the checksummed manual input manifest. Pin casting, pronunciation/name
  map, provider/model/adapter, effective performance controls, cue assets/rights and mix plan as
  applicable. Missing required identity is a handoff blocker, not an invented revision.
- Audio-profile revision and checksummed reference when one exists. If none exists, write
  `not defined` and keep profile conformance and its acceptance decision unresolved; a research
  line can still be heard. Never fabricate revision 1 or approved numerical targets.
- Provenance/rights references, purpose and permitted review access. A listening pass cannot
  clear source, voice, training-data or sound-asset rights, authorize spending or enable dispatch.

Store completed records privately outside Git, under the
[story workspace contract](story-workspace.md). For final scene/episode audio, an immutable
`listening_<id>.rNNNN.md` belongs beside the run/audit records in `audio/` and in its index. Line
auditions and intermediate clips stay in their private research/intermediate workspace, with
the same manifest identities; they do not become final `audio/` handoffs. Never put credentials,
signed URLs, private transcripts or media in a public issue or repository record.

## Playback coverage

**Both headphones and a phone's built-in speaker are required.** Laptop speakers are optional
additional evidence and cannot replace either required setup. Record device make/model, OS,
headphone make/model and wired/Bluetooth route, player/version, channel/mono behavior if known,
speed, volume setting, EQ/spatial/normalization settings and room/background conditions. Use
normal speed (1×), a comfortable ordinary listening volume and the same unchanged bytes on each
setup. Record any adjustment and the ranges replayed; do not change the render to improve a score.
An unknown setting stays `unknown`, not a claim of disabled processing.

| Subject | Required coverage on each setup | Limits |
| --- | --- | --- |
| Line | Entire line including lead-in/tail; compare pinned companion lines when judging preset consistency | A single voice cannot establish pairwise cast distinctness or whole-scene orientation |
| Scene | Entire scene, entrances/exits, transitions in scope and all co-present speaker pairs, including narrator versus characters | Unheard adjacent scenes remain outside the decision |
| Episode | Entire episode, each scene and its boundaries; check recurring voices/motifs across scenes | Sampling opening/middle/ending cannot establish full-episode listening |

Write heard and unheard ranges per device and pass. A stopped/interrupted pass is partial. Retain
its observations, leave unperformed checks `NOT_RUN` and keep the overall decision incomplete.
Device differences remain separate findings/scores; a good headphone result cannot hide words
masked on the phone. An optional device's material defect also remains visible.

## Two passes

### Pass 1 — without transcript

The reviewer receives only the subject ID, safe playback instructions and scope; withhold the
transcript, speaker labels, cue sheet, scene synopsis and expected orientation answers. Listen
once through each required setup before revealing any of them. Record initial impressions and
uncertain passages before replaying, then record any replay separately. A repeated listen is no
longer a first encounter; record device order and prior hearing. Prior script familiarity must be
disclosed. If expected answers were already known, label that pass non-blind and obtain a fresh
reviewer for the blind check; until then, blind coverage is `NOT_RUN`.

For each scene on each setup, record the reviewer's answers, uncertainty and the timestamp of
the audible evidence, without prompting with an answer:

1. **Who?** Which voices/people are present, who speaks to whom, and who enters or leaves?
   Can every co-present pair be told apart, including narration? Names are useful, but identifying
   a consistent role/relationship also records what the ear actually understood.
2. **Where?** What place or acoustic situation does the sound suggest, and what establishes it?
3. **When?** What time/sequence is understood? Was a move, time skip or flashback recognizable?
4. **What happened?** What changed, and which words/cues/pauses carried that change? Where did
   comprehension or emotional intent become uncertain?

For an isolated line, record what can be understood; assess the applicability of scene context
after listening. Do not penalize a line for lacking information outside its declared scope.
Do not supply expected answers to repair a blind misunderstanding in this record.

### Pass 2 — with transcript

Reveal only the pinned transcript/score, cast mapping, pronunciation intent, cue/mix plan and
unresolved-control records. Listen through the same scope on each required setup, then replay
marked ranges to localize observations. Check actual words, tones, names, dialogue order and
completeness, speaker attribution, intended pauses, cue anchors/purpose and scene transitions.
Compare blind answers to the pinned intent and record differences; preserve the original answers.
Directions must not have leaked into spoken text. A measured alignment is not text-fidelity proof.

Record both the supported controls and the intended performance. Neutral/default synthesis does
not prove whisper, shouting, sobbing, overlap or an interior-voice treatment. An unsupported
required control needs correction or an explicitly reviewed fallback, bound to new inputs;
it cannot be relabeled `N/A` or silently credited as achieved. Assess emotional fit through the
actual writing, delivery, pacing and cues; do not reward an imagined provider capability.

## Shared dimensions and scores

Use these six existing TTS dimensions unchanged in meaning for every applicable subject. Proper
names are taken from its pinned text/name map or pronunciation lexicon; the neural handoff keeps
its own probe-name inventory. Do not reward a regional accent merely for matching personal
preference; distinguish legitimate accent from an incorrect meaning-bearing tone, name or word.

| Dimension | What to check |
| --- | --- |
| Lexical tones | Correct meaning-bearing Vietnamese tones and vowel/consonant distinctions; no dropped syllables |
| Proper names | Names and locations in the pinned subject, pronounced as intended |
| Pauses and phrasing | Sentence/semicolon/clause boundaries, breath groups and chunk joins; no interruption inside a word |
| Question/exclamation | The question remains recognizable as a question; a call/exclamation conveys its intended function within the neutral/default profile |
| Naturalness | Fluent rhythm and intelligibility; record robotic, strained, repeated or garbled passages |
| Voice consistency | Stable identity/accent/timbre within that preset across short, long and different sentence types |

Add these drama dimensions when the heard scope supplies their content:

| Dimension | What to check |
| --- | --- |
| Speaker distinctness | Every co-present pair and narrator/character pair is recognizable by voice on each setup; record confusions by IDs after the transcript pass |
| Orientation (who/where/when) | Blind answers agree with intended people, place, time and transitions; distinguish uncertainty from an incorrect inference |
| Pacing and silence | Turn gaps, deliberate holds and scene gaps support comprehension/tension; room tone persists where intended; padding does not masquerade as a planned pause |
| Dialogue-versus-bed balance | Speech stays intelligible over music, ambience and foreground effects, including phone playback; voice levels feel consistent |
| Cue timing and purpose | Each cue lands relative to the intended dialogue/action and carries orientation, action, emotion or transition; motif/extent/fade intent is heard where in scope |
| Audible artifacts | Clicks, level jumps, abrupt cuts, garbling and apparent digital-silence dropouts at joins/inside scenes; describe what was heard without claiming measured sample values |
| Emotional fit within supported controls | Actual neutral/default or explicitly supported delivery, writing, cues and pacing fit the scene; unresolved required performance is surfaced |

Score each applicable dimension from **1 to 5**, separately by device and by scene/voice where
needed. Shared anchors: **5** = accurate and fluent with no material issue; **4** = small audible
imperfections without changed words/meaning; **3** = noticeable issues that interrupt flow;
**2** = repeated errors or altered meaning; **1** = unusable/incomplete. For drama dimensions,
apply the same material-impact anchors to their stated questions and explain the score.
Do not average voices, scenes or devices into a replacement for their individual results.

- **NOT_RUN:** not heard/assessed, interrupted before assessment, missing playback device or
  missing necessary comparison/context. Leave the numeric score unassigned and state why/next action.
- **N/A:** only after listening, no applicable content in this declared scope; give the reason
  and heard range (e.g. no proper name or no bed was intended/present). Never convert `NOT_RUN`
  to `N/A`. A missing expected cue, word or control is a gap/finding, not non-applicability.
- **Scored:** an actual observation by the named reviewer, with device/pass/range and explanation.
  No inferred score from metadata, ASR, file existence, shell playback or validator output.

## Findings and editorial decision

Every audible finding records its ID, dimension, **exact filename and full SHA-256**, input/render
revision, device and pass, start/end time (`hh:mm:ss.mmm` from the first decoded frame of that
file), expected versus actually heard content, impact, severity and smallest next action/owner.
Also name dialogue/scene/cue IDs when available. An excerpt finding includes its parent range
mapping; never invent timestamps for unheard bytes. An uncertainty is described as uncertainty.

Use only `blocking` or `advisory` for editorial finding severity:

| Severity | Mapping / examples |
| --- | --- |
| `blocking` | Prevents editorial acceptance of the affected render under business rule 20: missing/wrong meaning-bearing dialogue, tones or names; speaker/orientation confusion that changes the understood story; words masked by beds/effects; materially disruptive clicks/cuts/dropouts; silently unmet required performance |
| `advisory` | Audible imperfection or suggested refinement without lost meaning, intelligibility, speaker identity or required dramatic intent; explain why its impact is limited |

Low scores still need findings with the actual impact; a score alone does not select severity.
Disputed material impact stays unresolved for the owner, never silently downgraded. Missing
device coverage, identity or rights decisions are separately listed handoff/acceptance blockers;
do not invent an audible finding or time range for an administrative absence.

Resolve the overall editorial decision in this order, retaining every finding and gap:

1. No listening: **NOT_RUN**. Partial or missing required coverage/identity: **INCOMPLETE**;
   observed blocking findings are still listed and require changes.
2. With complete coverage, any blocking finding or unmet criterion of the recorded accepted
   policy: **CHANGES_REQUESTED**, regardless of averages. State each unmet criterion even if its
   audible findings are advisory. A replacement render needs a fresh checksum-bound review,
   not a copied pass.
3. No blocking finding but unresolved threshold/policy or disputed impact:
   **OWNER_DECISION_REQUIRED**. Report the scores as observations, not accepted production quality.
4. Complete applicable checks, no blocking finding and a recorded accepted policy satisfied:
   **ACCEPTABLE_FOR_REVIEWED_SCOPE**. Preserve advisory findings and the exact scope limitations.

`audio-listened` labels only actual heard scope with reviewer/date/setup/checksum; a complete
record may still request changes. It never implies `audio-measured`, rights clearance or publication.
Business rule 21 binds any later approval to exact inputs/render. Changed text, casting, assets,
profile or bytes require review of the affected new candidate; retain earlier records immutably.

## Proposed thresholds and owner decisions

The inherited line proposal is **PROPOSED**: every applicable dimension at least 4; no
omitted/incorrect meaning-bearing word, tone or proper name; no clipped/garbled syllable; and
recognizable question/exclamation intent in short probes. Its principle remains: an average
cannot hide a critical error. No owner acceptance is recorded by this protocol.

For scenes/episodes, the **PROPOSED** extension is every applicable drama dimension at least 4
on each required setup, correct blind orientation for every scene, distinguishable co-present
speaker pairs, and no blocking findings. This is a candidate for owner review, not a numerical
backend gate. Loudness, peaks, gains, pause lengths and format targets belong to the separately
decided versioned profile in [#51](https://github.com/loveoverflowcom/cantos/issues/51).

Each threshold decision needs its ID/revision, scope (line/scene/episode, language and setups),
exact criteria, evidence reviewed, rationale and exceptions. Keep status **PROPOSED** until an
actual owner decides. Only after acceptance record approver identity, decision date/time and the
immutable decision reference; rejected/superseded proposals retain their history. Do not fill an
approver field with an agent, issue author or presumed owner merely to complete a template.
A listening reviewer and a threshold approver need not be the same person. No exception may
hide a blocking finding or waive a separate rights/publication gate.

The template maps human facts to the proposed QC shape: kind `Editorial`, subject = render
SHA-256, input revision = frozen digest/manual manifest, profile = recorded revision when present,
recorder = named reviewer (real actor ID only if one exists), findings and recorded date. This
mapping implements no database DTO, endpoint or persistence; [#9](https://github.com/loveoverflowcom/cantos/issues/9)
owns backend QC. A missing profile/actor is stated, not forged into that future shape.
In the proposed publication gate, checksum-bound blocking QC findings become `QcBlockingFinding`
blockers; advisory findings remain visible. This document changes no backend or approval gate.

## Handoff

Return the immutable review path/checksum, subject/input/profile identities, device/pass coverage,
dimension results, all findings, overall editorial decision and unresolved owner decisions.
Link it from the production run and audit index; keep measured technical evidence separate.
The [#44 reference](https://github.com/loveoverflowcom/cantos/issues/44) is intended future drama
material, not a prerequisite for writing this protocol. Pin its reviewed artifact only when
available; neither an unmerged score nor a validator pass establishes a render or listening result.
No named-listener assessment, provider eligibility, billing or publication is completed by #45.
