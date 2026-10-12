# Vietnamese reference episode — revision 1 review notes

Derived from [episode_0001.r0001.script-ir.json](episode_0001.r0001.script-ir.json), the sole
authoritative performance text for [issue #44](https://github.com/loveoverflowcom/cantos/issues/44).
These notes describe intended performance, not an executable mix plan or a listening result.
Source file SHA-256: `240a4a8ac0d0f5663ae39311bd6c9b8201e495b9fdbd3ceb27442e1f07a30719`.
Reader output and commands are recorded in [local evidence](verification.md).
Regenerate the notes and evidence if the JSON bytes change; preserve IDs in subsequent revisions.

## Placement, authorship and rights

**Project-original material: no acquired text.** *Hai tiếng chuông bên cầu* was composed in
Vietnamese for the Cantos project in this issue's Codex authoring session, from the issue's
requirements and the [craft guide](../../audio-drama-craft.md). Its people, places, events,
dialogue, the fictional mother's words and cue descriptions are invented for this reference.
It is not an adaptation, quotation or translation of an acquired story, including the miniature
examples in the guides. It is intended as project-owned reference material for repository review
and later audio evaluation. This declaration does not record an owner's production-rights
decision, a voice license, an asset license or publication approval.

This reviewed `docs/production/examples/vietnamese-reference/` location makes the original text
available without changing the accepted/rejected contract corpus or its goldens. It is a bounded
repository example authorized by #44, not private/acquired story data from a
[story workspace](../../story-workspace.md). Future story/audio records remain outside Git.

The IR truthfully declares `kind: "generated"` for AI-assisted original authorship. The local
provenance record is this paragraph plus the pinned JSON/evidence: Codex authored revision 1
on 2026-10-12 under the request for #44; no imported manuscript, website story, provider audio,
sound asset or recorded voice was an input. The request specified two scenes and audition/craft
coverage; the [quality plan](../../../work-plan/audio-drama-quality.md),
[radio score](../../radio-score.md) and implemented 0.1.0 reader constrained the representation.

External references are **unregistered example identifiers**, not invented evidence of registry
acceptance: `ref-source-unregistered` identifies this reference source;
`ref-generation-44-r0001-unregistered` identifies this authoring session;
`ref-work-rights-unregistered` and `ref-score-rights-unregistered` identify pending source/score
rights records. Work `source_ref` and adaptation `provenance_refs` both resolve to `ref-origin`
inside the document. Before backend save or any production use, preserve the source and record
real owner-scoped source, generation and rights evidence, then issue a newly bound score revision.
The CLI does not resolve those external records. No cue selects an `asset`.

No source-name replacement is applicable: there is no acquired source or name-map snapshot.
The complete fictional name inventory is Yên Thư (`Thư`), Vũ Kha (`Kha`), Mộc Du (`bác Du`),
trạm Gió Nồm and cầu Mây, plus the work/episode/act titles in the JSON. `Người dẫn chuyện` is
the explicit storyteller role, not a fourth person present in the scenes. Thư's mother and
Kha's mother are separate, unnamed people; quoted parental words remain spoken by Thư or Du,
not an uncast fifth voice. Retain the Vietnamese spellings in every future rendering.

## Shape, story and estimated performance length

One work, one adaptation (`vi-VN`), one episode, one act, two ordered scenes; four characters,
exactly one narrator, 30 dialogues and 12 typed cues. All deliveries are explicitly
`neutral` / `0`; no performance capability, voice, sound asset or audio profile is selected.
Questions and exclamations are audition text, not instructions to synthesize shouting.

| Scene | Orientation and story function | Cast / sequence |
| --- | --- | --- |
| `ref-station` | Sorting room at trạm Gió Nồm, late afternoon (`ref-d01`). Thư returns after ten years; her mother died three years ago. The station closes tomorrow, prompting collection of the keepsake. | Narrator, Thư, Kha, Du; `ref-d01`–`ref-d15` |
| `ref-bridge` | Shelter at the foot of cầu Mây, twenty minutes later, dusk (`ref-d16`). All three arrive via the market path. Thư opens the box, hears the bell, chooses companionship; Du departs while the friends remain. | Narrator, Thư, Kha, Du; `ref-d16`–`ref-d30` |

The JSON contains **643 whitespace-delimited spoken units** (341 + 302). Vietnamese units here
are mostly syllables, not English-style words. An editorial estimate of 220–280 units per minute
gives roughly 2.3–2.9 minutes of speech; allowing roughly 20–40 seconds for turns, the transition,
effects and the deliberate holds gives **about 2.6–3.6 minutes performed**. These are planning
assumptions, not a measured duration or a prescribed pause/profile setting. Real timing awaits
rendering and listening; no audio was generated or heard in #44.

Forms of address: Thư/Kha use `mình`–`cậu`; both address Du as `bác` and themselves as `cháu`.
Du uses `bác`–`cháu`/`hai cháu`. Three diegetic speakers are active together; the narrator supplies
orientation outside their exchange. Du enters on `ref-c-enter`/`ref-d03`; all three leave the room
on `ref-d15`/`ref-c-leave-room`, arrive on `ref-d16`/`ref-c-arrive`, and Du alone exits on
`ref-d29`/`ref-c-du-exit`. No fourth visitor or returning mother is implied.

## Craft-device coverage

Rows use the device names from [Translating prose devices](../../audio-drama-craft.md#translating-prose-devices).
This is an original score, so the table maps designed story functions, not transformations of
source paragraphs. Every dialogue/cue belongs to a passage in the listening guide below;
there is no sampled or omitted source range.

| Prose device | Exact dialogue / cue IDs | Intended translation and limitation |
| --- | --- | --- |
| Description of a setting | `ref-d01`, `ref-c-room`; `ref-d16`, `ref-c-river` | Distinct sheltered room / outdoor river beds plus names and times that sound alone cannot identify. Beds have no encoded extent or acoustics. |
| Entrance or exit | `ref-d03`, `ref-c-enter`; `ref-d15`, `ref-c-leave-room`; `ref-d16`, `ref-c-arrive`; `ref-d29`, `ref-d30`, `ref-c-du-exit` | Door/footsteps and named greetings/farewells identify arrival and departure. Thư and Kha stay after Du leaves. |
| Interior thought | `ref-d11`, `ref-d12` | Narrator explicitly attributes a withheld question; Thư's following line is private interior speech. Its whisper/interior treatment is unresolved, not dialogue Du or Kha hears. |
| Emotion shown through the body (sweat, trembling) | `ref-d06`, `ref-d07`, `ref-c-breath` | Kha notices trembling; Thư's separate unsteady breath carries bodily tension. Spoken delivery stays neutral; a cue does not give the synthesizer a breathy mode. |
| Symbol, metaphor, recurring image | `ref-d07`, `ref-c-motif-a`; `ref-d26`, `ref-c-motif-b`; `ref-d15` | Two bell taps mean someone is waiting: remembered maternal welcome becomes a friend's present company. The light keepsake feels heavy to lift. Descriptions link the motif; there is no motif field or selected asset. |
| Time skip or scene change | `ref-d15`, `ref-c-transition`; `ref-d16`, `ref-c-river` | Wordless bridge and explicit twenty-minute marker carry the move via the market. Transition extent/crossfade is unresolved. |
| Hesitation or dramatic silence | End of `ref-d20` → `ref-d21`; `ref-c-motif-b` after `ref-d26` → `ref-d27`; `ref-c-river` | Hold the river bed after the mother's message and let the repeated taps settle before Kha replies. No pause field; punctuation is not a timing contract. |
| Interruption or overlapping speech | `ref-d21`, `ref-d22`, `ref-d23`, followed by `ref-d24` | Kha answers before Thư completes her request; Du interrupts his haste, then Thư completes her intention. Three voices in one exchange need clear attribution. Overlap timing is unresolved. |
| Visual-only action (a nod, a glance) | `ref-d25`; `ref-d28` | Narrator supplies the otherwise inaudible name under the lid; Thư states assent and a plan instead of relying on a visible nod. No extra nod cue or unseen letter is assumed. |
| Long exposition | `ref-d04`, `ref-d05`, `ref-d07`, `ref-d08`, `ref-d09`, `ref-d10`; `ref-d18`, `ref-d19`, `ref-d20` | Closing date, keepsake, loss, old invitation and safe route are distributed across plausible questions/replies by people who know them. Du alone knows the mother's final request. No source compression or invented adaptation coverage is claimed. |

## Narrator-line reasons

| Dialogue | Reason category | Necessary information beyond adjacent cues |
| --- | --- | --- |
| `ref-d01` | **place** | Names the postal sorting room; also marks late afternoon and Thư's protected pocket, introducing the paper in `ref-d09`. Rain alone cannot name the station. |
| `ref-d11` | **thought** | Attributes Thư's unspoken fear before private `ref-d12`. Neither the breath nor the bell exposes that question. |
| `ref-d16` | **time** | Twenty minutes have passed; the three people reached the named bridge via the safe market route. Footsteps/river alone cannot convey elapsed time. |
| `ref-d25` | **essential visual fact** | The inscription is Thư's name in the same purple ink; no letter is inside. A lid sound cannot establish handwriting or absence of a letter. |

## Audition coverage for every character

All examples below are excerpts/ID references derived from the JSON, not an alternate script.
Tone samples deliberately contain all six Vietnamese tones (ngang, huyền, sắc, hỏi, ngã, nặng).
These are pronunciation challenges, not a pronunciation-quality finding.

| Character | Question | Exclamation | Proper-name material | Tone-dense phrase and ID |
| --- | --- | --- | --- | --- |
| `ref-narrator` — Người dẫn chuyện | `ref-d11`: “bác Mộc Du có hiểu … không?” | `ref-d25`: “Kìa, … nét mực tím!” | `ref-d01`: Gió Nồm, Yên Thư; `ref-d16`: cầu Mây, Vũ Kha, Mộc Du | `ref-d01`: “Gió đẩy những giọt mưa xuống bậc thềm” (mưa / thềm / Gió / đẩy / những / giọt) |
| `ref-thu` — Yên Thư | `ref-d02`; `ref-d19` | `ref-d07`: “Đừng, Kha!” | `ref-d02`: Vũ Kha, Mộc Du; `ref-d07`: cầu Mây | `ref-d09`: “Tờ giấy cũ của mình vẫn in một vệt mực tím” (in / Tờ / giấy / của / cũ / một) |
| `ref-kha` — Vũ Kha | `ref-d04`; `ref-d06`; `ref-d17`; `ref-d27` | `ref-d14`: “Bác Du chờ cháu với!”; `ref-d17` | `ref-d06`: Yên Thư; `ref-d10`: cầu Mây | `ref-d06`: “Sợi chỉ đỏ đã buộc chặt quanh chiếc hộp bằng gỗ” (quanh / bằng / chiếc / chỉ / đã / Sợi) |
| `ref-du` — Mộc Du | `ref-d08` | `ref-d03`: “Yên Thư! Vũ Kha!”; `ref-d23` | `ref-d03`: Yên Thư, Vũ Kha; `ref-d13`: cầu Mây | `ref-d05`: “Mùi gỗ ẩm vẫn quẩn quanh chiếc hộp nhỏ” (quanh / Mùi / chiếc / ẩm / gỗ / hộp) |

Names/tones, final consonants, quoted speech and turn recognition await the pronunciation lexicon
and distinctness auditions in #50. No respelling override is guessed before hearing a problem.

## Cue intentions and placement

Every cue is an unselected intent. Anchor edges name placement, not duration. No description is
an instruction for a runner to parse. The future cue sheet must bind selected, rights-recorded
assets and exact score bytes before any render.

| Cue | Kind / anchor | Purpose and listening question |
| --- | --- | --- |
| `ref-c-room` | ambience / `ref-d01:start` | Orientation: can the listener hear sheltered interior space without rain obscuring names? Intended bed continues through the first scene. |
| `ref-c-enter` | sfx / `ref-d03:start` | Action: does the door lead naturally into Du's named greeting, without implying Thư/Kha have left? |
| `ref-c-box` | sfx / `ref-d05:end` | Action: does a small wooden object become tangible while its lid remains closed? |
| `ref-c-breath` | sfx / `ref-d07:start` | Emotion: is Thư's tension perceptible without a large gasp masking “Đừng, Kha”? |
| `ref-c-motif-a` | sfx / `ref-d07:end` | Memory/symbol: two taps from the still-closed box recall the welcome; Thư has only loosened the binding. Can the motif later be recognized? |
| `ref-c-leave-room` | sfx / `ref-d15:end` | Action: three people cross the threshold; the door ends the room scene. |
| `ref-c-transition` | music / `ref-d15:end` | Transition: follows the exit cue in tie order; wordless bridge carries time, then yields to the river. No melody, file or profile chosen. |
| `ref-c-river` | ambience / `ref-d16:start` | Orientation/silence: river and roof shelter distinguish the second place; retain the bed under deliberate holds. |
| `ref-c-arrive` | sfx / `ref-d16:start` | Action: follows the river cue in tie order; exactly three arrivals, then stillness under the shelter. |
| `ref-c-open-box` | sfx / `ref-d24:end` | Action: first lid opening, followed by the inscription. No paper effect, because the invitation was in Thư's pocket. |
| `ref-c-motif-b` | sfx / `ref-d26:end` | Symbol: recognizable repeat, now deliberately sounded by Thư in the open air; wait before Kha's reply. |
| `ref-c-du-exit` | sfx / `ref-d29:end` | Action: one set of footsteps departs toward the market; the two friends remain for `ref-d30`. |

## Unresolved controls — resolve before the affected render

“Required” means required for this intended performance, not implemented support or an approval.
Record any supported provider/mix mapping as frozen inputs, or review a score change explicitly.
Do not silently drop a control or treat neutral delivery as its implementation. No numeric
pause, gain, fade, location profile or stereo setting is chosen here; #49 owns placement of the
controls, #47 the cue sheet, and #51/#53 the profile/acoustics decisions.

| Control | Affected IDs / boundaries | Requested meaning | Required? / current limitation |
| --- | --- | --- | --- |
| Pause / deliberate silence | `ref-d20:end` → `ref-d21:start`, under `ref-c-river`; `ref-d26:end` after `ref-c-motif-b` → `ref-d27:start` | Let the mother's message land; later let two taps settle. Silence of speech, with river room tone continuing. | Required. 0.1.0 has no pause field; line boundaries, ellipses and provider padding do not implement a planned hold. |
| Whisper / interior voice | `ref-d12`, attribution in `ref-d11` | Quiet private speech heard by the audience, not by Kha or Du. Consider a whisper-like treatment only if intelligibility and provider capability support it. | Interior perspective required; literal whisper is an unresolved candidate. 0.1.0 has no voice mode. Neutral synthesis does not establish privacy; a supported treatment or reviewed narrator rewrite is needed. |
| Overlap / interruption | `ref-d21` → `ref-d22`; tail of `ref-d22` → `ref-d23`; return to solo `ref-d24` | Kha anticipates the unfinished question, Du checks him, Thư completes her request. Keep each speaker/word understandable. | Required for this exchange. Array order supplies a serial fallback for reading, not the intended onset/overlap schedule. |
| Cue extent — beds | `ref-c-room` over `ref-d01`–`ref-d15`; `ref-c-river` over `ref-d16`–`ref-d30`, including both holds | Room/river ambience persists, then ends at the appropriate scene boundary. | Required. A `start`/`end` anchor is a point, not a span or stopping instruction. |
| Cue extent — foreground events and music | `ref-c-enter`, `ref-c-box`, `ref-c-breath`, `ref-c-motif-a`, `ref-c-leave-room`, `ref-c-transition`, `ref-c-arrive`, `ref-c-open-box`, `ref-c-motif-b`, `ref-c-du-exit` | Bound each event's audible tail; complete the first two-tap motif before its next turn and the second before `ref-d27`; exit/music yield to `ref-d16`. | Required. All ten non-bed cues lack encoded duration; future selected asset length alone does not determine placement relative to speech. |
| Transition crossfade / masking / ducking | `ref-c-room`, `ref-c-leave-room`, `ref-c-transition`, `ref-c-river`, `ref-c-arrive`; every cue under speech | Signal a clean place/time change and protect the words. No implied return to the station or fourth arrival. | Mix resolution required. Fade/gain/ducking fields do not exist in 0.1.0. |
| Narrator and scene acoustics | Narrator: `ref-d01`, `ref-d11`, `ref-d16`, `ref-d25`; ordinary scene dialogue: `ref-d02`–`ref-d10`, `ref-d13`–`ref-d15`, `ref-d17`–`ref-d24`, `ref-d26`–`ref-d30`; private `ref-d12` belongs to the interior-mode row above | Keep narrator outside the room/river acoustic perspective; keep the three scene voices mutually distinguishable. | Future audition/mix review required. No cast or acoustic/placement settings are encoded; stereo is not assumed. |

## Listening guide per passage

These are future listening prompts, **not answers from audio**. All 30 dialogues and all 12 cues
are covered. Listen without the transcript first; later use the IDs to localize a finding.
Actual reviewers, devices, render hashes, rubric and decisions belong to #45 and later runs.

| Passage / complete dialogue range | Associated cues | Intention and future listening prompt |
| --- | --- | --- |
| Room opening, `ref-d01`–`ref-d05` | `ref-c-room`, `ref-c-enter`, `ref-c-box` | Locate the station and identify Thư, Kha and newly arriving Du. Can a listener say why they came, and who recognizes whom? Test the narrator against all three voices. |
| Keepsake and memory, `ref-d06`–`ref-d10` | `ref-c-breath`, `ref-c-motif-a` | Hear Thư's reluctance and connect two taps to the mother's old greeting; distinguish the pocket invitation from the still-closed box. Are Kha's help and the safe route clear? Compare all three tone-dense character samples. |
| Private fear and departure, `ref-d11`–`ref-d15` | `ref-c-room` continuing, `ref-c-leave-room`, `ref-c-transition` | Keep `ref-d12` private after attribution; Du proposes the bridge without answering a thought he did not hear. Can the listener distinguish interior voice from an ordinary reply and track all three leaving? |
| Bridge arrival and message, `ref-d16`–`ref-d20` | `ref-c-river`, `ref-c-arrive` | Understand the twenty-minute move and the mother's final message; preserve river tone in the hold after `ref-d20`. Can the listener locate the new shelter and explain the change from waiting for the mother to being awaited? |
| Hasty exchange and reveal, `ref-d21`–`ref-d25` | `ref-c-river` continuing, `ref-c-open-box` | Kha jumps in, Du interrupts, Thư finishes; narrator supplies the inaudible inscription. Can all three voices and each intended interruption be distinguished without losing words? The lid opens only here. |
| Repeated motif and farewell, `ref-d26`–`ref-d30` | `ref-c-motif-b`, `ref-c-du-exit`, `ref-c-river` continuing | Recognize the same two taps with changed meaning, hear the hold before a warm ordinary invitation, and track Du's sole exit. Can the listener tell the friends remain together and will take the safe route home? |

Residual risk: the text and coverage can be reviewed now; voice distinctness, pronunciation,
interior/overlap treatment, audible timing, motif recognition, sound balance and performance
duration await actual renders and named listening. Structural validation grants no backend,
production, rights or publication acceptance.
