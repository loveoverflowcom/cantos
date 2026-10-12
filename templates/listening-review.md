# Listening review: [subject ID] — [review revision]

Follow the [protocol](../docs/production/listening-review-protocol.md). This blank template is
not listening evidence. Complete it outside Git, preserving private content and immutable
revisions under the [story workspace contract](../docs/production/story-workspace.md).
Replace instruction links with pinned repository references when copying outside the repository.
All unheard assessments start **NOT_RUN** with no numeric score. **N/A** requires actual listening
and no applicable content; an average cannot hide a blocking finding.

## Reviewer and record

- Review ID / revision: [ID / immutable revision]
- Named human reviewer / actor ID: [name; actor ID only if it exists]
- Vietnamese comprehension / accent familiarity: [description]
- Review date/time / timezone: [actual session dates; separate dates per setup if needed]
- Prior script/render exposure / device order: [none or exact exposure; blind limitations]
- Protocol revision: [commit/reference]
- Run / audit / related issue: [private references]
- Record filename / SHA-256: [final checksum in containing index, not self-hashed in this file]

## Exact subject and inputs

| Field | Recorded identity |
| --- | --- |
| Subject kind / scope | [line / scene / episode; IDs and full or excerpt coverage] |
| Exact played filename / private relative reference | [filename / reference] |
| Played file SHA-256 | [complete 64 hexadecimal characters, never a shortened digest] |
| Render revision / duration / timebase | [revision; duration; timestamps from file start, hh:mm:ss.mmm] |
| Excerpt parent filename / SHA-256 / range mapping | [exact identity and parent start/end, or not an excerpt] |
| Work / episode / scene / dialogue IDs | [stable IDs] |
| Score/script input revision / byte SHA-256 | [exact pinned revision and checksum] |
| Frozen run input revision / digest | [exact identity, or checksummed manual input manifest if no run exists] |
| Casting / pronunciation / name-map revisions | [pinned references/checksums, or absent with reason] |
| Provider / model / voice / adapter revisions | [actual render provenance] |
| Actual supported / effective performance controls | [controls and capability evidence; no imagined support] |
| Cue assets / mix plan / sound-bible revisions | [pinned references/checksums, or absent with reason] |
| Unresolved required controls / reviewed fallbacks | [affected IDs, owner/action and evidence, or none] |
| Audio-profile revision / checksum | [exact revision/reference when it exists; otherwise not defined] |
| Source / voice / model / sound rights and provenance | [private evidence references and unresolved decisions] |
| Permitted purpose / access scope | [research/editorial review scope] |

Use a separate subject record for different bytes/encodings. Check that both required setups
played the listed SHA-256. Missing identity is an explicit handoff blocker.

## Playback setup and coverage

Required setups: **headphones AND phone built-in speaker**. Optional laptop evidence is additional.
Use 1× speed. Duplicate rows for additional sessions/devices; keep every result distinct.

| Setup ID | Route / device make-model / OS | Headphones make-model and connection | Player / version | Speed / volume | EQ / spatial / normalization / mono behavior | Room / background |
| --- | --- | --- | --- | --- | --- | --- |
| H1 | [headphones; source device/OS] | [wired/Bluetooth and model] | [player/version] | [1×; setting] | [actual settings or unknown] | [conditions] |
| P1 | [phone built-in speaker; model/OS] | [not used] | [player/version] | [1×; setting] | [actual settings or unknown] | [conditions] |
| L1 (optional) | [laptop speaker; model/OS, or not used] | [not used] | [player/version] | [setting] | [settings] | [conditions] |

| Setup | Pass | Actual session date/time | Filename / full SHA-256 | Heard start–end ranges | Unheard ranges / interruption / adjustments / replays | Coverage status |
| --- | --- | --- | --- | --- | --- | --- |
| H1 | 1 without transcript | [date/time] | [exact filename/hash] | [ranges] | [gaps/replays or none] | NOT_RUN |
| P1 | 1 without transcript | [date/time] | [exact filename/hash] | [ranges] | [gaps/replays or none] | NOT_RUN |
| H1 | 2 with transcript | [date/time] | [exact filename/hash] | [ranges] | [gaps/replays or none] | NOT_RUN |
| P1 | 2 with transcript | [date/time] | [exact filename/hash] | [ranges] | [gaps/replays or none] | NOT_RUN |

Coverage status: `NOT_RUN`, `partial`, `complete`; no required setup/pass can be replaced by `N/A`.
Finish both transcript-free passes before revealing expected answers. If already script-familiar,
label the observations non-blind and leave blind coverage `NOT_RUN` until a fresh reviewer does it.

## Pass 1 — blind answers, recorded before transcript access

For every scene/setup, duplicate this row. For isolated lines state the heard context and limits.
Keep initial answers unchanged when adding replay notes and pass-2 comparisons.

| Scene/line ID / setup / heard range | Who speaks, to whom, entrances/exits and pair confusions | Where, and audible evidence/time | When / transition, and audible evidence/time | What changed / comprehension / emotional intent | Uncertainty / prior exposure / replay notes |
| --- | --- | --- | --- | --- | --- |
| [ID/setup/range] | [actual answer, or NOT_RUN] | [actual answer/time, or NOT_RUN] | [actual answer/time, or NOT_RUN] | [actual answer, or NOT_RUN] | [limitations] |

## Pass 2 — transcript comparison

- Pinned transcript/cast/lexicon/cue plan checked: [identities from the input table]
- Blind answers versus intended orientation by scene/setup: [differences and finding IDs]
- Words/tones/names, order/completeness and directions leaked into speech: [observations/IDs]
- Each co-present pair, including narrator/characters, by scene/setup: [recognized/confused,
  exact ranges and finding IDs; no pairwise claim from one voice alone]
- Pauses, transitions, motifs, cue anchors/extent and balance: [observations/IDs]
- Actual controls versus required intent / explicit fallback: [observations/IDs]
- Missing/unheard comparisons: [NOT_RUN and next action; never guessed scores]

## Dimension results

Definitions and shared 1–5 anchors live in the [canonical rubric](../docs/production/listening-review-protocol.md#shared-dimensions-and-scores).
Duplicate the table per scene/voice when needed. In each setup cell record `NOT_RUN`, `N/A` with
reason, or a numeric score with pass/range/explanation and finding IDs. An unused optional laptop
stays `not used`; it supplies no evidence. No average replaces these rows.

| Dimension | H1 result / range / explanation | P1 result / range / explanation | Optional setup result | Finding IDs |
| --- | --- | --- | --- | --- |
| Lexical tones | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Proper names | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Pauses and phrasing | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Question/exclamation | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Naturalness | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Voice consistency | NOT_RUN | NOT_RUN | [not used] | [IDs; companion file identities if compared] |
| Speaker distinctness | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Orientation (who/where/when) | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Pacing and silence | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Dialogue-versus-bed balance | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Cue timing and purpose | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Audible artifacts | NOT_RUN | NOT_RUN | [not used] | [IDs] |
| Emotional fit within supported controls | NOT_RUN | NOT_RUN | [not used] | [IDs] |

## Checksum-bound findings

Duplicate this block for every **observed** audible finding; remove the blank block when none
were found, only after completed listening. Keep administrative gaps in the next section.

- Finding ID / dimension: [ID/dimension]
- Severity: [blocking / advisory; actual impact rationale]
- Exact filename / full SHA-256: [file/hash actually heard]
- Render revision / input revision or digest / profile revision: [identities; not defined if absent]
- Setup / pass / scene-dialogue-cue IDs: [setup/pass/IDs]
- Start / end timestamp: [hh:mm:ss.mmm–hh:mm:ss.mmm, relative to the exact played file]
- Excerpt parent SHA-256 / mapped range: [identity/range if applicable]
- Expected content or intent: [exact pinned words or cue/control intent]
- Actually heard content: [reviewer's observation; preserve uncertainty]
- Impact / confidence / repeatability: [why blocking or advisory; initial versus replay hearing]
- Smallest next action / responsible owner: [correction/review action; no implicit paid regeneration]
- Follow-up review reference: [new immutable record; prior finding retained]

## Coverage and acceptance blockers

- Required device/pass/range/dimension gaps: [NOT_RUN/partial checks, reasons and next actions]
- Missing subject/input/profile identities or comparisons: [gaps and effect on decision]
- Unresolved required controls / disputed severity: [IDs and responsible owner]
- Separate rights, technical measurements or publication blockers: [references; no invented
  timestamp or audible finding for an administrative absence]

## Threshold policy / owner decision

- Decision ID/revision / immutable reference: [reference, or no decision recorded]
- Status: **PROPOSED** [change only with a recorded actual owner decision]
- Scope: [line/scene/episode; language, devices and applicable dimensions]
- Exact criteria under consideration: [inherited line proposal / drama proposal / linked alternative]
- Evidence reviewed / rationale / exceptions: [references, or not yet reviewed]
- Approver / acceptance date-time: [leave unassigned while PROPOSED; record only actual acceptance]
- Rejected/superseded decision references: [history, if any]

Proposals are not approved thresholds. Any accepted exception stays visible and cannot hide a
blocking finding or waive rights/publication gates. Profile numerical targets need their own decision.

## Overall editorial decision and handoff

- Overall decision: **NOT_RUN** [then INCOMPLETE / CHANGES_REQUESTED /
  OWNER_DECISION_REQUIRED / ACCEPTABLE_FOR_REVIEWED_SCOPE, in protocol precedence order]
- Reason / policy reference / blocking and advisory finding IDs: [all results/gaps, no average override]
- Evidence level and exact heard scope: [audio-listened only after actual named-human listening;
  date, setups, filename/hash and ranges; otherwise absent]
- Separate measured technical evidence: [tool/version/profile/reference, or NOT_RUN]
- Publication/rights approval: [separate reference or absent; never inferred from listening]
- Proposed QC-shape mapping: [Editorial; subject SHA-256; input revision; profile if present;
  recorder/reviewer; findings; recorded_at. No backend record created by this template.]
- New render / revised inputs needing another review: [scope/checksum or none]
- Immutable report path/checksum and containing index: [private references]
- Production run / story audit handoff: [private references]
- Next responsible person/action / remaining owner decisions: [specific next step]
