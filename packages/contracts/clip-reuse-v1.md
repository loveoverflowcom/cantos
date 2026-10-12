# Clip reuse v1 — speech fingerprint, verified clips and regeneration plan

Status: implemented bounded backend contract for the cache/reuse and partial-regeneration
criteria of [#7](https://github.com/loveoverflowcom/cantos/issues/7) and the planning half of
[#8](https://github.com/loveoverflowcom/cantos/issues/8). It adds no Studio route, no shared HTTP
DTO and no UI; it closes neither issue. The [evidence report](../../docs/evidence/speech-clip-reuse.md)
records what was actually run, the [decision](../../docs/decisions/0016-speech-fingerprint-and-clip-reuse.md)
records the choices, and the [pipeline](../../docs/product/production-pipeline.md#cache-and-partial-regeneration)
stays the product source of truth. [Durable jobs v1](durable-jobs-v1.md) owns the execution
boundary this contract extends; [production input v1](production-v1.md) owns the frozen documents.

Everything here is **local and development-only**. The only object store is the worker's private
filesystem output root. No object storage, CDN, signed URL, mixing, QC record or publication exists,
so nothing here makes audio publishable. A clip is verified evidence of *these bytes*, not an
approval.

## Two identities, two questions

| Identity | Question | Includes | Use |
| --- | --- | --- | --- |
| `durable-input-v1:sha256:…` | Is this step linked to exactly this frozen dialogue? | dialogue, character, rights record and every other resolved field | step linkage inside one run; never a cache key |
| `speech-v1:sha256:…` | Would a provider receive the same synthesis request? | only fields that can change the audio | cross-run clip reuse and regeneration planning |

`durable-input-v1` is unchanged and still authorizes no reuse.

## Speech fingerprint `speech-v1`

```text
SpeechFingerprint = "speech-v1:sha256:" + lowercase_hex(
    SHA256(b"cantos/speech-fingerprint/v1\n" + canonical_input_bytes)
)
```

`canonical_input_bytes` is compact UTF-8 JSON with ASCII keys in sorted order, the minimal
escapes (`"`, `\`, `\b \f \n \r \t`, other controls below U+0020 as `\u00xx`), integers only
and no trailing newline:

```text
{"adapter_version", "language", "model_id", "output_contract",
 "performance":{"emotion", "intensity_permille", "pitch_semitones", "rate_permille"},
 "pronunciation":[{"replacement", "surface"}…], "provider_id", "text", "voice_id"}
```

| Field | Why it is effective |
| --- | --- |
| `text` | the exact effective bytes sent to the provider, after resolved pronunciation replacements; not normalized again |
| `language` | request language tag |
| `provider_id`, `model_id` | the engine and its exact version (`espeak-ng-1.52.0`) |
| `voice_id` | the selected voice |
| `adapter_version`, `output_contract` | processing code and the audio format the bytes must satisfy |
| `performance.rate_permille`, `pitch_semitones` | synthesis settings, fixed-point integers |
| `performance.emotion`, `intensity_permille` | delivery; `null` means "inherit", which is not `neutral` |
| `pronunciation` | the whole resolved list, sorted by surface; duplicate surfaces are rejected |

Not in the key, by type: `dialogue_id`, `character_id`, `voice_rights_record_id`, scene, position,
episode, job, run, script revision, snapshot digest, settings or catalog version, rights status and
budget. The constructor destructures `ResolvedSynthesisInput` without `..`, so a new field does not
compile until it is classified as effective or identity-only.

`canonical_input_bytes` is bounded at 2 MiB (`MAX_CANONICAL_BYTES`, also the registry's `CHECK`); a request beyond
it has no fingerprint (`input_too_large`). Admission already bounds a line to about 106 KB of pronunciation rules
plus 64 KiB of text, so this is headroom, and it guarantees that a request with a fingerprint can always be
registered: the registry can never fail an acceptance on size.

The constructor destructures `ResolvedSynthesisInput`, `SynthesisPerformance` and every `ResolvedPronunciation`
without `..`, so a new field at any of those levels does not compile until it is classified.

Consequences, deliberately:

- **Moving, reordering or renumbering a line changes nothing.** Two characters with the same voice
  and wording share one clip.
- **A pronunciation change regenerates every line that carries that list** (business rule 16 as
  written). A per-line projection would change the product rule and needs that document updated
  first.
- **Unicode-equivalent but byte-different text is a different request.** NFC admission is upstream;
  the fingerprint hashes what a provider would receive, so a miss is the only possible error.
- **Rights are never keyed.** A lapsed licence blocks; it does not trigger a regeneration that would
  be equally unlicensed.

### Model pinning

Only a provider/model pair whose identity is exact in the frozen input has a fingerprint:
today `espeak-ng-local` + `espeak-ng-1.52.0`. Google voice names carry no revision and VieNeu is
research-only, so `SpeechInput::from_resolved` returns `model_not_pinned` for them and they are
never registered or reused. Admitting an unpinnable model needs a recorded refresh-epoch mechanism
and a scheme or compatibility decision; it is not implemented.

### Versioning

Any change to the field set, key order, escaping or hash prefix is a new scheme (`speech-v2`) with a
reviewed migration note; stored fingerprints and canonical documents are never rewritten. Goldens
change only by a deliberate `--write-goldens` run of the independent oracle.

### Vectors and oracle

[`fixtures/clip-reuse/v1/speech-vectors.json`](fixtures/clip-reuse/v1/speech-vectors.json) holds 16
accepted inputs, 5 refused inputs and the same/different relations. It is written by
`scripts/reference_speech_fingerprint.py`, a standard-library implementation of the rules above that
shares no code with Rust. Rust must reproduce every canonical document and digest, every refusal code
and every relation. After a deliberate change:

```sh
python3 scripts/reference_speech_fingerprint.py --write-goldens   # review the diff
```

## Verified clip registry

Migration `0008_speech_clips.sql`.

| Record | Meaning / mutability |
| --- | --- |
| `speech_clips` | one immutable registration of an accepted attempt's audited output: owner, fingerprint, canonical input bytes (at most 2 MiB), producer (`local_espeak` or `synthetic_fake`), voice rights record, output contract, SHA-256, byte length, private path and the source attempt |
| `durable_steps.speech_fingerprint` | the step's position-free identity; `NULL` for steps created before the migration, which therefore never reuse |
| `durable_steps.reused_clip_id` | write-once reference to the clip that satisfied the step instead of a provider attempt |

PostgreSQL itself refuses forged or mismatched rows, independent of application code:

- a `CHECK` recomputes the fingerprint from the stored canonical bytes with `sha256()`;
- a trigger requires the row to restate the **accepted** observation of its attempt: same owner (run
  creator), same producer (run mode), same output path and the receipt's `audio_sha256`, **and** to be the clip of
  that attempt's own step: the step's recorded `speech_fingerprint`, and the voice-rights record and output contract
  of the frozen line for that dialogue. A hash-consistent document for some other request, or another licensed
  record, is refused;
- `speech_clips` rows cannot be updated, deleted or truncated;
- a step can adopt a clip only for the same fingerprint, owner and producer, only once, and only while
  it has no provider attempt; a step that adopted a clip can never gain an attempt or reservation (a dispatch
  hangs off an attempt). Those inserts read the step row `FOR SHARE`, which conflicts with the adopting update, so
  a concurrent writer that bypasses the application's job lock serializes with it: whichever commits second sees
  the other. These are single-database guards against application bugs; a PostgreSQL superuser remains outside
  them.

Registration happens inside `accept_step`'s transaction, after the receipt and WAV audit, so an
accepted step cannot lose its clip to a crash. A step with no recorded fingerprint (created before the
migration) or whose recorded fingerprint differs from the recomputed one, and a line with no fingerprint, is
simply not registered: acceptance is never failed by the registry. Registration is a fact about the audited bytes: **a completed job
is not a reusable clip** until a later lookup also passes every check below. There is no backfill for
runs created before the migration.

## Reuse decision

`resolve_reuse(request, rights, candidates)` is pure. A candidate is a hit only if all hold:

| Check | Refusal |
| --- | --- |
| same `speech-v1` fingerprint | `fingerprint_mismatch` |
| same creator (script owner) | `owner_scope` |
| same voice rights record as the requesting line | `voice_rights_record` |
| same producer: a `synthetic_fake` clip never satisfies a `local_espeak` run and the reverse | `producer_mismatch` |
| same output contract | `output_contract_mismatch` |
| the object was probed (`NotProbed` is never a hit) | `unverified` |
| the object exists and opens safely | `unavailable` (missing, may be transient) |
| the bytes are valid audio for the contract (no link, device, oversize, malformed) | `artifact_invalid` |
| the observed SHA-256 equals the registered one | `checksum_mismatch` |

Rights are a separate input, `RightsStanding::{Current, Lapsed}`, derived from the requester's current
authority (approval, editorial review, rights and budget eligibility, session). `Lapsed` returns
`Blocked` even for a perfect clip and for an empty registry: producing anew is no fix. Candidates are
examined oldest first by registration order and the first valid one wins whatever order the caller
supplies, so a later duplicate never flips a choice. At most 16 registered clips per fingerprint are
considered; more than that fails closed as a miss. The shell narrows the registry query to the request's
creator, fingerprint, producer, voice-rights record and output contract *before* that bound, so clips that
could never be reused cannot crowd out valid ones; the decision is still taken by `resolve_reuse`, again inside
the selecting transaction. A plan lists only clips of the requested producer; clips with another voice-rights record or output contract
can still occupy its 16-per-fingerprint window, so a plan can under-forecast hits relative to the worker. A plan
probes every candidate it lists, so its cost is bounded by lines × 16 object reads.

The probe reads through the same hardened private reader recovery uses: the exact attempt directory
inside the configured output root, no symlink, hardlink or non-regular file, a size bound, then WAV
inspection under the clip's own output contract and a SHA-256 of the bytes. A worker whose root cannot
see a clip simply misses.

The selection cannot be handed an unobserved digest: `select_reused_clip` requires a `VerifiedObject` witness, which has
private fields and no public constructor. Only `ClipProber::inspect` issues one, and only when the object was
read safely, inspected under the clip's contract and hashes to exactly the registered checksum. The transaction
still re-runs the whole decision with it.

## Worker protocol

```text
claim step (fence n) ─▶ reuse_candidates       read-only: authority, lease, no prior attempt, rows
                    ─▶ probe objects           no transaction, bounded file reads
                    ─▶ [failpoint before_reuse]
                    ─▶ select_reused_clip      ONE transaction, fenced:
                         authority · lease · job state · no attempt · full resolve_reuse
                         with the observed digest · step succeeded · job completed if last
         any conflict ─▶ ordinary prepare_step (reserve · intent · dispatch) re-reads everything
```

A reused step has **no attempt, reservation, dispatch or observation**: a cache hit is never a
provider send and spends nothing. Crash and race behaviour:

| Event | Result |
| --- | --- |
| worker dies before the selection commits | nothing durable; the lease expires and another worker repeats the lookup |
| stale fence, expired lease, cancel, pause | `LeaseLost`; no selection |
| two workers select one step | exactly one succeeds; the other gets `LeaseLost` |
| a provider attempt is already recorded for the step | no lookup; `prepare_step` reconciles it as before (`uncertain_attempt`), never bypassed by a hit |
| a step's stored fingerprint differs from the recomputed one | no reuse and no registration; the dispatch path's own linkage check takes over |
| authority lapsed (rights, approval, session) | no hit; the job pauses `authorization_invalid` as before |
| clip, digest or authority changed between lookup and selection | `Conflict` (an authority lapse included); the dispatch path takes over and pauses the job as usual |

The `JobSummary` printed by the CLI gains `reused_steps`: of the `completed_steps`, those satisfied by a
verified clip. It deserializes as zero when absent, so reports written before the registry still read.

## Regeneration plan

```text
production-jobs plan <script-id> <next-snapshot-id> (--local-espeak|--test-fake)
    [--previous <snapshot-id>] [--output-root <absolute-private-directory>]
```

Read-only; requires `CANTOS_ENV=development`, `CANTOS_DATABASE_URL` and `CANTOS_SESSION_TOKEN`. It starts
no job, calls no provider and creates no reservation. Output is one JSON report
([schema](schema/clip-reuse/v1.schema.json), [fixture](fixtures/clip-reuse/v1/plan-edit-one-line.json)).

- **`gate`** mirrors the worker's authority test: a current approval over eligible inputs, else
  `blocked` with finding codes. A closed gate makes every line `blocked`; a hit never widens it.
- **Lines**, in script order, each with the position-free fingerprint (or none) and:
  - `change` against the previous snapshot by dialogue ID: `added`, `unchanged` (with
    `placement_changed` when only its scene differs; a reorder inside one scene shows as that scene's
    `speech_sequence_changed`), `changed` with the exact fields, or `incomparable`;
  - `action`: `reuse` a verified clip, `produce` with the cache reason (`no_candidate`, `refused` with
    each clip's refusal, or `no_fingerprint`), or `blocked`.
  An edited line produces; reverting the edit is a hit on the history; an unchanged line whose object is
  missing or corrupt is produced and says why.
- **Scenes and episode**: a scene mix is `stale` when the scene is new, its ordered fingerprints differ
  from the previous snapshot, or any of its lines will be synthesized or is blocked (a mix hashes audio
  bytes, so new bytes stale it even when the speech is unchanged). Its QC goes stale with it. The episode
  master and QC go stale on any stale scene, removed scene or changed scene order. No mix, QC or approval
  record exists yet; these marks name work, they persist nothing.
- **Cost** separates `cache_hits`, `cache_misses` and `blocked`; `provider_dispatches_expected` equals the
  misses when the gate is open. `miss_estimate` and `hit_estimate_avoided` are planning estimates under the
  recorded rate (`planning_estimate_not_a_charge`), rounded per dialogue exactly as the frozen estimate;
  an unrecorded rate is `unavailable`, never zero.
- **`object_probes`** is `not_performed` unless `--output-root` is given. Without probes nothing is verified,
  so no clip counts as a hit and every candidate is refused as `unverified`.

## Not claimed

- Exactly-once synthesis. Execution is at-least-once with idempotent selection. Two runs that miss the same
  fingerprint at the same moment both dispatch; both outputs register and the oldest valid one is selected
  afterwards. Cross-run in-flight de-duplication is not implemented.
- Any object storage, CDN, signed URL, retention policy or host power-loss durability. A selected clip is not
  re-verified after the selection; a consumer that reads the bytes (the future mixer) must re-check the
  checksum.
- Google or VieNeu reuse, a live provider call, audio quality, listening review or publication eligibility.
- A persisted stale mark, mix fingerprint, QC fingerprint or Studio endpoint. The plan is CLI JSON; a Studio
  route needs its own review under `/api/v1/studio/*`.
- Previous-clip identity in staleness: a scene whose fingerprints are unchanged and whose lines all reuse is
  reported unchanged even if the registry later resolves a different clip for a fingerprint. That needs mix
  records, which do not exist.

## Verify

```sh
cargo xtask verify repository   # includes the oracle's Python tests
cargo xtask verify contracts    # vectors, plan schema and fixture, planner laws
cargo xtask verify postgres     # registry, selection, fencing and crash tests on a new cluster
```
