# Audio drama craft

Status: proposed editorial guidance for the [radio score](radio-score.md) workflow, reconciled
from an external research report on novel-to-audio-drama adaptation (see the
[audio drama quality plan](../work-plan/audio-drama-quality.md)). It describes intent and review
questions. No validator, mixer or Studio feature implements these rules yet; where Script IR
0.1.0 cannot express an intent, it remains an unresolved control under the radio-score rules.

This document owns ear-first adaptation and sound-design intent. The
[radio score](radio-score.md) owns the mapping to Script IR, proper-name replacement and coverage;
the [production pipeline](../product/production-pipeline.md) and the
[audio mix and QC method](../../.agents/skills/cantos-production-pipeline/references/audio-mix-and-qc.md)
own profiles, mix plans, measurements and QC records. Numbers (loudness, pause lengths, gains)
belong to the versioned audio profile, not to this guide.

## The listener has no page

A reader can reread, see a paragraph break and look at a speaker tag. A listener hears one
stream once. Prose relies on description and interior narration; audio drama carries the same
story through dialogue, sound, music and deliberate silence. Adaptation therefore moves meaning
between channels while preserving events, chronology, character knowledge and tone. It does not
add events to remove narration.

Every scene answers three questions for the ear within its first moments:

1. **Where are we?** An establishing ambience or one signature sound, or a brief narrator line
   when sound alone cannot distinguish the place.
2. **Who is speaking?** A character new to the scene is named, addressed or introduced in the
   first exchanges. Vietnamese forms of address (`anh`/`em`, `chú`/`cháu`, `cô`/`trò`) carry
   relationships; keep them consistent per relationship, because they also tell the listener
   who is speaking to whom.
3. **When are we?** A time skip or flashback is marked by the story's transition device (see
   [sound bible](#sound-bible-and-motifs)) or a short narrator time marker.

Further orientation guidelines (review prompts, not validator rules):

- Mark entrances and exits audibly: a door, footsteps, a greeting or a farewell.
- Keep few active speakers in one exchange. More than three characters trading lines needs
  clearly distinct voices or narration support; record the reason when a scene exceeds it.
- Prefer a name over an ambiguous pronoun when several people of the same address form are
  present.
- Do not make characters narrate visible action ("Tôi đang mở cửa đây") or explain facts both
  speakers already know. Exposition moves into speech only when that character could plausibly
  know and say it.

## Narration with a purpose

The narrator stays when sound and plausible dialogue cannot carry necessary meaning. Each
narrator line in the adaptation review notes records one reason category: **place**, **time**,
**thought**, **attribution** or **essential visual fact**. A narrator line that repeats what an
adjacent cue already conveys is a review finding.

The share of narration per scene is a review signal, not a target. A high share prompts a look
for sound or dialogue alternatives; a low share is not success if it was reached by inventing
lines, flattening voices or losing source meaning ([radio score](radio-score.md#light-dialogue-forward-adaptation)).

## Translating prose devices

| Prose device | Audio technique | Expressible in Script IR 0.1.0 | Remaining gap |
| --- | --- | --- | --- |
| Description of a setting | Establishing ambience plus one signature effect; a narrator line only if still ambiguous | `ambience`/`sfx` cue anchored at the `start` of the scene's first dialogue | How long a bed runs (cue extent); location acoustics |
| Entrance or exit | Door/footsteps effect and a greeting that names the character | `sfx` cue plus dialogue | — |
| Interior thought | Narrator line; whispered self-talk; a letter or diary read aloud; a dramatized memory scene when the source contains it | Narrator dialogue or character dialogue | Whisper and interior-voice treatment are not expressible; record an unresolved control |
| Emotion shown through the body (sweat, trembling) | Breathing or heartbeat effect, tension in music, the line's delivery | `sfx`/`music` cue; `delivery.emotion` | Current castable voices admit only neutral delivery, so emotion rests on writing, pacing and cues |
| Symbol, metaphor, recurring image | A recurring motif or signature sound registered in the sound bible | Cue description naming the motif; the same `asset` ID when selected | No motif field; the sound bible/cue sheet holds the link |
| Time skip or scene change | Music bridge or sting; change of ambience; brief narrator time marker | `music` cue at the `end` of the scene's last dialogue; the next scene's ambience | Transition timing and crossfades belong to the mix plan |
| Hesitation or dramatic silence | A planned pause with room tone continuing | Not expressible | Pause control; record an unresolved control |
| Interruption or overlapping speech | Overlap timing between two lines | Not expressible | Overlap control; record an unresolved control |
| Visual-only action (a nod, a glance) | Convert to speech when meaning-bearing; otherwise omit | Dialogue, or an omission row in the coverage table | — |
| Long exposition | Split across characters who know it, summarize in narration, or omit with a recorded reason | Dialogue or narrator | Coverage table records the compression |

Original miniature example, written for this guide:

> Trên chuyến đò cuối ngày, Lam ngồi im nhìn mặt sông. Nàng nghĩ: nếu mai cha không về, ai sẽ
> giữ cái quán? Gió lạnh dần, tiếng mái chèo khua đều.

A review rendering can open with river ambience and regular oar strokes anchored at the start of
the first line, so the place needs no narration. The rising cold wind becomes an ambience change,
not a sentence. Lam's thought has three legitimate treatments: a narrator line; Lam whispering to
herself, which needs an interior/whisper treatment that 0.1.0 cannot express and must be recorded
as an unresolved control; or nothing, if a later scene dramatizes the same worry. Giving the
ferryman a new question so Lam can answer aloud invents an exchange the source does not contain;
it is an editorial addition that must be labeled, justified and reviewed, and a silence or music
cue is usually the better choice. In an acquired story, every proper name here passes through the
[name map](radio-score.md#proper-name-map).

## Silence and pacing

- **Planned, not inherited.** Deliberate pauses come from the score's intent and the profile's
  timing policy. Silence that a TTS engine adds at the start or end of a clip is padding, not a
  pause; conditioning removes it before the plan places lines.
- **Turn gaps differ by context.** A continuation by the same speaker, a reply, a speaker change
  after a question and a scene change each need different gaps. Back-to-back clips with no gap
  sound mechanical; uniform long gaps sound slow.
- **Dramatic silence keeps the room.** Hold ambience or room tone under a pause. Digital silence
  sounds like a dropout and is a QC finding.
- **Short and placed.** One clear pause after a revelation does more than many long ones.

## Voices for the ear

- **Distinct within a scene.** Characters who share a scene, and the narrator relative to every
  character, must be distinguishable by voice alone on a phone speaker. Audition candidate voices
  side by side before freezing casting.
- **Consistent across episodes.** A character keeps one voice for the whole adaptation; a recast
  is a recorded casting change that invalidates dependent speech.
- **Pronounced as intended.** Renamed proper names, Sino-Vietnamese terms and loanwords need a
  pronunciation check; inspectable `pronunciation_overrides` carry the fix, never hidden markup.
- **Write within the voice's range.** When a line depends on a performance the cast voice cannot
  deliver (sobbing speech, shouting across a field), record the unresolved control, rewrite with
  source support, or carry the moment with an effect and a neutral line. Never let neutral
  delivery silently replace a required performance.

## Sound bible and motifs

A story keeps one sound vocabulary so recurring places, people and symbols sound the same in
every episode:

- **Motifs:** a short musical theme or signature effect tied to a character, theme or symbol,
  with the meaning it carries and when it may appear. Reusing the same selected asset revision
  keeps it recognizable; variations are deliberate.
- **Location palettes:** the ambience and acoustic character of each recurring place.
- **Transition device:** how the story signals a scene change, a time skip and a flashback.
- **Narrator treatment and voice notes:** how the narrator sits relative to scenes, and casting
  notes per character.

Every cue needs a dramatic purpose: orientation, action, emotion or transition. Avoid music with
sung lyrics under dialogue, because voices compete for intelligibility. Script IR 0.1.0 has no
motif, location or extent fields, so these decisions live in workspace records outside the
script, planned in the [quality plan](../work-plan/audio-drama-quality.md).

## Mix intent

These are intentions for the mix plan and the editorial review; the profile supplies numbers.

- Dialogue intelligibility comes first. Beds duck under speech; foreground effects land on their
  anchors and do not mask words.
- Dialogue level stays consistent across voices and providers, so no character is louder merely
  because of the engine.
- The narrator sits apart from the scene's acoustic space; scene voices share that space.
- Stereo placement, if the profile is stereo, stays conservative and mono-compatible, because
  phone speakers fold to mono.
- Joins between clips are free of clicks and level jumps.
- Check on headphones and on a phone speaker before claiming a listening result.

## Ear-first review checklist

Use it in adaptation review and in the [story audit](../../.agents/skills/cantos-story-audit/SKILL.md).
Record findings against dialogue/cue IDs; a question that cannot be answered is a finding, not a
pass.

1. Can a listener tell where each scene happens, who speaks and when it occurs, without the
   transcript?
2. Is every new speaker in a scene identified early, and is every entrance and exit audible?
3. Does each narrator line have a recorded reason, and does none repeat an adjacent cue?
4. Which technique carries each interior thought, and is any unexpressible treatment recorded as
   an unresolved control?
5. Are editorial additions labeled with a source relationship, and were no events invented?
6. Does every cue have a dramatic purpose, a correct anchor and a sound-bible motif or location
   where one applies?
7. Are forms of address and character voices consistent with earlier scenes and episodes?
8. Are required pauses, overlaps, whispers and cue extents recorded with their affected IDs?
9. Are renamed names and difficult terms covered by the pronunciation check?
10. For a render: are speech levels consistent, pauses intentional, joins clean, beds below
    speech, and was it heard on headphones and a phone speaker by a named reviewer?
