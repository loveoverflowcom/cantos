# Local neural listening handoff

> **Historical (ADR 0018).** This evidence describes the in-process text-to-speech runtime that
> [ADR 0018](../decisions/0018-external-audio-production.md) removed on 2026-10-12. The code, tests
> and contracts it names no longer exist; links to them are pinned to commit `2de8c69`, the last
> commit that contained them. Nothing here describes current behavior.

Recorded 2026-10-11 for [draft PR #26](https://github.com/loveoverflowcom/cantos/pull/26).
Production remains disabled. This handoff supplies actual research WAVs and a proposed human
rubric; it does not supply listening scores or creator acceptance.

## Evaluation status

**Model audition: NOT_RUN. Human audition: NOT_RUN.** This review environment has no tool that
accepts audio for perception. WAV inspection, hashes, phoneme strings, ASR transcripts and
shell playback cannot substitute for hearing the renders. All six dimension scores and the
overall judgment for every sample remain unassigned. A named Vietnamese-speaking listener
must record results against the exact WAV checksum; changing the recipe requires new evidence.
Use the shared [listening-review protocol](../production/listening-review-protocol.md) and
[template](../../templates/listening-review.md) for that assessment; #45 supplies the record
and procedure only and does not complete this #6 named-listener task.

The six original fixtures are unchanged in
[neural-vietnamese.json](https://github.com/loveoverflowcom/cantos/blob/2de8c69/scripts/fixtures/neural-vietnamese.json). Four additional
[short probes](https://github.com/loveoverflowcom/cantos/blob/2de8c69/scripts/fixtures/neural-vietnamese-short.json) cover each admitted voice
with `Em về chưa?` (three words) and `An ơi!` (two words). All ten WAVs are native 24 kHz,
mono signed 16-bit PCM, with frozen-input identity and scoped receipt checks. Actual synthesis
and structural validation establish playable bytes, not speech quality.

## Proposed rubric

Listen at normal speed, first without reading the transcript, then while checking it, on both
headphones and a phone's built-in speaker; record each device/setup and its coverage under the
shared protocol. Compare all five samples of each voice
for consistency; evaluate Đức Trí and Mỹ Duyên separately. Do not reward a different regional
accent merely for matching personal preference. Distinguish a legitimate accent from an
incorrect lexical tone, name or word.

The six original TTS dimensions, shared 1–5 anchors and NOT_RUN versus N/A rules now live in the
[canonical rubric](../production/listening-review-protocol.md#shared-dimensions-and-scores),
with their meaning preserved. For these probes, the proper-name inventory is Nguyễn Ánh,
Mỹ Duyên, sông Hương, Thừa Thiên Huế, Cần Thơ, Vọng Đài, Anh Minh and An. Apply the additional
drama dimensions only where the actually heard scope has applicable content; isolated clips
cannot establish scene orientation or pairwise cast distinctness.

The inherited line acceptance remains
[PROPOSED](../production/listening-review-protocol.md#proposed-thresholds-and-owner-decisions),
subject to an actual owner's recorded agreement. No approver or production threshold is supplied
by this handoff. Preserve blocking findings regardless of averages and keep rights acceptance separate.

## Scorecard

All six dimension scores and overall judgment are **NOT_RUN** in every row below. The ZIP
manifest supplies the full text, exact WAV SHA-256, input digest, dialogue/model/voice identity,
frame count and filename for each row. No media is committed to Git.

| Sample | Preset | Passage to scrutinize | Audition/scores |
| --- | --- | --- | --- |
| narration | Đức Trí | “Ngoài hiên, mưa rơi nhẹ;” and the final phrase | NOT_RUN |
| question | Mỹ Duyên | “ngày mai chúng mình có diễn tiếp không?” | NOT_RUN |
| exclamation | Đức Trí | “An ơi, đừng vội đi!” | NOT_RUN |
| proper-names | Mỹ Duyên | All names and locations listed above | NOT_RUN |
| tones | Đức Trí | “Bà bảo bé bỏ”, “Má mỉm cười, mở cửa” | NOT_RUN |
| moderate-dialogue | Mỹ Duyên | Long phrasing and “Có lẽ đó chỉ là gió;” | NOT_RUN |
| short-question-duc-tri | Đức Trí | “Em về chưa?” | NOT_RUN |
| short-question-my-duyen | Mỹ Duyên | “Em về chưa?” | NOT_RUN |
| short-exclamation-duc-tri | Đức Trí | “An ơi!” | NOT_RUN |
| short-exclamation-my-duyen | Mỹ Duyên | “An ơi!” | NOT_RUN |

For each judgment use the shared template to record listener name, date, both devices/setups,
filename/checksum, input revision and profile revision when present, applicable dimension scores,
overall decision, exact expected/actually heard excerpt, start/end audio timestamps and severity.
The bundle's `scorecard.json` starts with null scores, separate model/human status and an empty
issues list. Fill observed issues; do not fabricate excerpts or timestamps before listening.
The historical ZIP/scorecard is unchanged by this documentation update; retain it and add a new
immutable review linked by exact WAV hashes rather than overwriting its original NOT_RUN evidence.

## Observed normalization risk

The installed pinned SDK/SEA-G2P path was executed on the four short probe texts, separately
from inference. For both presets, its normalized text and final phonemes end in a period:

| Original | Normalized chunk | Final phonemes |
| --- | --- | --- |
| `Em về chưa?` | `em về chưa.` | `ˈɛm vˈe2 tʃˌyə.` |
| `An ơi!` | `an ơi.` | `ˈaːn ˈəːj.` |

This confirms the known <5-word punctuation transformation. It does **not** establish what
intonation a listener hears. The pinned normalization/recipe was preserved, not silently
changed to obtain a better result. Unsupported speed, pitch or expressive controls remain
rejected; only neutral/default controls are admitted.

## Recovery and delivery

The historical six-sample run remains under ignored UUID
`85f8c3d4-6be8-4d4c-baa6-fde7a535d5ca`. The new four-sample run is
`7764b94f-d5be-4bb6-af55-05a127f5d64f`, executed serially in 23.51 seconds with the installed
model/runtime and fresh frozen inputs, intents, receipts and audits. It is one cold observation
per probe, not a steady-state benchmark. The short clips last 0.76–1.35 seconds; cold CLI wall
times of 5.825–5.955 seconds yield RTF 4.325–7.664. Short-clip startup cost must not be hidden
by the six longer samples' earlier RTF values.

A preceding sandboxed attempt is preserved under UUID `359a2dbe-41f0-41fb-b74f-c9392e93e83b`.
Its first synthesis produced a success receipt and WAV, but `/usr/bin/time -l` failed to read
macOS kernel counters, making the test fail. That observed success was checked before the
explicit fresh four-probe run outside that restriction. No original destination was overwritten,
and no automatic provider retry or exactly-once claim was added.

The listening ZIP contains only the ten original research WAVs, manifests, scorecard and this
rubric. It omits model files, runtime, credentials and local filesystem paths. Each archived
WAV is compared byte-for-byte/hash-for-hash with its source and receipt. Library delivery and
round-trip byte verification, when successful, are recorded in the companion
[review evidence](local-neural-tts-review.md). A local path is not a remote audio download link.
