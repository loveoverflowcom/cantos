# Audio drama quality plan

Status: planning, recorded 2026-10-12. The project-original
[reference episode and review notes](../production/examples/vietnamese-reference/episode_0001.r0001.review.md)
provide Step 1's text artifact, with [local structural evidence](../production/examples/vietnamese-reference/verification.md).
No audio or listening result is supplied by it; the other outcomes below remain planned.
This plan reconciles an
externally supplied research report on adapting novels into audio drama (Vietnamese, not
committed) with the actual Cantos contracts and pipeline, and orders the resulting work into
bounded GitHub issues. GitHub issues own execution status; this document owns the rationale and
recommended order. Editorial rules derived from the research live in
[audio drama craft](../production/audio-drama-craft.md).

## Why

The current pipeline can freeze inputs, synthesize single lines and record receipts, but nothing
yet makes an episode sound like drama rather than a sequence of read lines. The research describes
a production process built around human actors, studios and DAWs. Much of it does not apply to a
TTS pipeline with immutable revisions; what does apply is the craft of writing for the ear, a
consistent sound vocabulary, listening-led quality decisions and disciplined editing and mixing.
This plan keeps those parts and maps them onto existing owners.

## Baseline

Inspected on `develop` at `342d65f`:

- [Script IR 0.1.0](../architecture/script-ir.md) admits dialogue with a closed emotion set and
  `ambience`/`music`/`sfx` cues anchored to a dialogue's `start` or `end`. It has no pause,
  voice-mode, overlap, cue-extent, motif or location field; the [radio score](../production/radio-score.md)
  records such needs as unresolved controls.
- All castable voices admit only neutral delivery. eSpeak produces 22,050 Hz mono PCM, VieNeu
  research output is 24 kHz mono and the Google adapter requests 24 kHz LINEAR16 (mocked only).
  Listening quality is **NOT_RUN** ([020](020-cast-and-generate-dialogue.md)).
- The [audio mix and QC method](../../.agents/skills/cantos-production-pipeline/references/audio-mix-and-qc.md)
  proposes a versioned profile, a pure mix plan and checksum-bound QC records, deliberately
  without numbers. [#9](https://github.com/loveoverflowcom/cantos/issues/9) owns mixing/QC and is
  blocked by [#7](https://github.com/loveoverflowcom/cantos/issues/7).
- [`inspect-wav`](../evidence/wav-pcm-inspection.md) reads WAV structure, checksums and sample
  peak. No loudness, true-peak, conditioning or mix code exists.
- No sound asset library, sound bible, cue sheet, audio profile or drama listening protocol
  exists. The only rubric is the proposed TTS-line rubric in the
  [local neural listening handoff](../evidence/local-neural-tts-listening.md).

## Research reconciliation

| Research recommendation | Disposition | Cantos form | Owner |
| --- | --- | --- | --- |
| Dialogue, effects and music carry the story; omniscient narration is rare | Adopt | Narration with a recorded reason; scene orientation rules | [Craft guide](../production/audio-drama-craft.md), [#48](https://github.com/loveoverflowcom/cantos/issues/48) |
| Interior monologue as whisper, self-talk, letter or diary | Adapt | Technique recorded per thought; whisper/interior treatment stays an unresolved control until decided | Craft guide, [#49](https://github.com/loveoverflowcom/cantos/issues/49) |
| Deliberate silence; music for tension and transitions | Adapt | Planned pauses from the profile timing policy; room tone under silence; transition device in the sound bible | [#49](https://github.com/loveoverflowcom/cantos/issues/49), [#47](https://github.com/loveoverflowcom/cantos/issues/47), [#51](https://github.com/loveoverflowcom/cantos/issues/51) |
| Leitmotifs and recurring effects for symbols | Adopt | Story sound bible outside Script IR | [#47](https://github.com/loveoverflowcom/cantos/issues/47) |
| Spotting session for effects and music | Adopt | Per-episode cue sheet bound to the score hash | [#47](https://github.com/loveoverflowcom/cantos/issues/47) |
| Casting and rehearsal | Adapt | Voice auditions, a distinctness matrix and a pronunciation lexicon; there are no human actors in the current pipeline | [#50](https://github.com/loveoverflowcom/cantos/issues/50) |
| Microphones, pop filters, room treatment, gain staging | Not applicable now | Speech is synthesized; revisit if creator recordings become an input | [Deferred expansion](backlog/deferred-expansion.md) |
| Editing: denoise, crossfades, compression and limiting | Adapt | Clip conditioning (padding, level alignment, resampling, boundary fades); limiting belongs to the master profile | [#52](https://github.com/loveoverflowcom/cantos/issues/52), [#51](https://github.com/loveoverflowcom/cantos/issues/51) |
| Mix balance, panning, reverb for location | Adopt after a baseline mix | Ducking in [#9](https://github.com/loveoverflowcom/cantos/issues/9); acoustics and placement as a follow-up | [#9](https://github.com/loveoverflowcom/cantos/issues/9), [#53](https://github.com/loveoverflowcom/cantos/issues/53) |
| Binaural or 3D sound | Defer | Deferred expansion | [Deferred expansion](backlog/deferred-expansion.md) |
| 48 kHz/24-bit processing, WAV master, MP3/AAC at 128–192 kbps or more, −16 LUFS, true peak ≤ −1 dBTP | Candidate values | Verified against primary sources and chosen by listening in a profile ADR | [#51](https://github.com/loveoverflowcom/cantos/issues/51) |
| ID3/MP4 metadata | Candidate | The profile decides embedded metadata; the release manifest stays authoritative | [#51](https://github.com/loveoverflowcom/cantos/issues/51), [#11](https://github.com/loveoverflowcom/cantos/issues/11) |
| Sound effect and music libraries | Adapt | Rights-recorded intake with decided license classes and attribution | [#46](https://github.com/loveoverflowcom/cantos/issues/46) |
| DAW applications as the production tool | Not adopted | A mix is a deterministic plan rendered by a pinned tool; a creator may prepare assets elsewhere and submit them through asset intake | [#46](https://github.com/loveoverflowcom/cantos/issues/46), [#9](https://github.com/loveoverflowcom/cantos/issues/9) |
| Production roles (writer, dramaturg, director, sound designer, engineer) | Adapt | Roles map onto recorded approvals: script review, casting, QC acceptance and publication ([business rule 13](../product/business-rules.md)) | Existing rules |
| QA checklist: script sign-off, loudness, noise | Adopt | Ear-first checklist, listening protocol and technical QC | Craft guide, [#45](https://github.com/loveoverflowcom/cantos/issues/45), [#9](https://github.com/loveoverflowcom/cantos/issues/9) |
| Folder layout `script/`, `audio/` (raw, edited, mix, final), `sfx/`, `music/` | Adapt | The existing [story workspace](../production/story-workspace.md) `raw/`, `score/`, `audio/`; intermediates stay out of `audio/`; assets come from the library | Story workspace |
| Git LFS for audio files | Reject | Generated audio and story data never enter Git; use the Drive workspace and object storage | [AGENTS.md](../../AGENTS.md) |
| GitHub Actions to encode, publish or update RSS | Reject | `cargo xtask` is the single repository command surface; rendering and publication are durable backend jobs behind gates | [ADR 0012](../decisions/0012-xtask-command-surface.md) |
| RSS/podcast feed | Defer | Theatre is the distribution surface; syndication needs a product decision | [Deferred expansion](backlog/deferred-expansion.md) |
| Gantt timeline and per-role effort shares | Not adopted | Planning records order and dependencies, not deadlines or estimates | [GitHub planning](project-planning.md) |

## Target flow

The improved flow inserts ear-first review, spotting, auditions, conditioning and listening review
into the existing pipeline. It does not change publication gates.

```text
pinned score ─▶ ear-first review ─▶ sound bible + cue sheet ─▶ voice audition + lexicon
     ─▶ synthesis (#6/#7) ─▶ clip conditioning ─▶ mix plan + render (#9) ─▶ technical QC
     ─▶ listening review ─▶ story audit ─▶ approval and publication gates (#11)
```

## Steps

### Phase 1 — editorial process

No backend change. These can proceed now, in parallel with the current #6/#7 work, through the
agent-operated [story workspace](../production/story-workspace.md).

| Step | Issue | Outcome | Blocked by |
| --- | --- | --- | --- |
| 1 | [#44](https://github.com/loveoverflowcom/cantos/issues/44) | Original Vietnamese reference scene exercising the craft devices, used by every later listening decision | — |
| 2 | [#45](https://github.com/loveoverflowcom/cantos/issues/45) | Listening-review protocol and template for line, scene and episode renders | — |
| 3 | [#46](https://github.com/loveoverflowcom/cantos/issues/46) | Rights-recorded sound asset library and intake procedure | — |
| 4 | [#47](https://github.com/loveoverflowcom/cantos/issues/47) | Story sound bible and per-episode cue sheet records | — |
| 5 | [#48](https://github.com/loveoverflowcom/cantos/issues/48) | Ear-first checks in `cantos-radio-adapt` and `cantos-story-audit` | — |

For the next editorial handoff, define #45's protocol before conducting auditions or declaring
listening results. Reuse the exact #44 reference JSON/hash rather than independently editable
copies; its unresolved pause, interior-mode, overlap and cue-extent intentions feed #49's decision.

### Phase 2 — decisions

| Step | Issue | Outcome | Blocked by |
| --- | --- | --- | --- |
| 6 | [#49](https://github.com/loveoverflowcom/cantos/issues/49) | ADR placing pauses, voice mode, overlap and cue extent in the script, the mix plan or nowhere | — |
| 7 | [#50](https://github.com/loveoverflowcom/cantos/issues/50) | Voice auditions, distinctness matrix and pronunciation lexicon for the reference cast | [#44](https://github.com/loveoverflowcom/cantos/issues/44), [#45](https://github.com/loveoverflowcom/cantos/issues/45) |
| 8 | [#51](https://github.com/loveoverflowcom/cantos/issues/51) | Audio profile revision 1 chosen by measurement and listening | [#44](https://github.com/loveoverflowcom/cantos/issues/44), [#45](https://github.com/loveoverflowcom/cantos/issues/45), [#46](https://github.com/loveoverflowcom/cantos/issues/46) |

### Phase 3 — output engineering

| Step | Issue | Outcome | Blocked by |
| --- | --- | --- | --- |
| 9 | [#52](https://github.com/loveoverflowcom/cantos/issues/52) | Pure clip-conditioning planner: padding trim, level alignment, resampling, boundary fades | [#51](https://github.com/loveoverflowcom/cantos/issues/51) |
| 10 | [#9](https://github.com/loveoverflowcom/cantos/issues/9) | Existing baseline mix and versioned QC | [#7](https://github.com/loveoverflowcom/cantos/issues/7) |
| 11 | [#53](https://github.com/loveoverflowcom/cantos/issues/53) | Scene acoustics, narrator treatment and mono-compatible placement | [#9](https://github.com/loveoverflowcom/cantos/issues/9), [#47](https://github.com/loveoverflowcom/cantos/issues/47), [#51](https://github.com/loveoverflowcom/cantos/issues/51) |

Steps 8 and 9 are groundwork for #9 in the same way as [#21](https://github.com/loveoverflowcom/cantos/issues/21):
they do not complete #9 and do not change its blockers.

## What improvement means

Each measure's threshold is decided in its owning issue; this plan sets none.

- **Technical (`audio-measured`, against the profile):** integrated loudness and true peak
  within tolerance, no clipped runs, no digital-silence dropouts inside a scene, consistent
  dialogue level across voices, no clicks at clip joins and a format matching the profile.
- **Editorial (`audio-listened`, by a named reviewer):** rubric results under #45, blind-pass
  orientation answered for every scene, and every character pair in a scene distinguishable on
  headphones and a phone speaker.
- **Process (`documented`):** every episode handoff lists its unresolved controls by ID, the cue
  sheet covers every cue, motifs are used consistently and every narrator line carries a reason.

One label never implies another. A measured pass is not a listening pass, and neither is a
publication approval.

## Effect on the work queue

This plan does not reorder [#6](https://github.com/loveoverflowcom/cantos/issues/6),
[#7](https://github.com/loveoverflowcom/cantos/issues/7), [#8](https://github.com/loveoverflowcom/cantos/issues/8)
or [#9](https://github.com/loveoverflowcom/cantos/issues/9), grant provider or spending
authorization, or relax any rights, approval or publication gate. VieNeu remains research-only and
Google remains disabled without separate operator authorization. Phase 1 improves the artifact
workflow immediately; Phases 2 and 3 feed #9 and its successors.
