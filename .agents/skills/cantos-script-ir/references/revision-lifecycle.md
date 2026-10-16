# Draft saves, immutable revisions, diffs and pinning

> **Scope.** The editorial lifecycle of a script: append-only saved versions behind a mutable
> draft head, optimistic concurrency on save, idempotent retries, submission into an immutable
> `ScriptRevision`, revision diffs that explain and drive invalidation, and pinning revisions in
> production, QC and approvals. Use when implementing or reviewing script save, submit, reopen,
> conflict handling, revision history, "what changed" views or any code that selects a script
> revision for production.

Product rules: [business rules 3 and 5](../../../../docs/product/business-rules.md#work-chapter-and-episode-identity),
[Script IR § Revision and validation policy](../../../../docs/architecture/script-ir.md#revision-and-validation-policy)
and the [Script/editor acceptance criteria](../../../../docs/assurance/script-editor-acceptance.md#acceptance-criteria)
(no silent overwrite of a competing edit, prior versions unaltered, reopen after restart, an
interrupted save preserves the last valid script). Immutable save/revision behavior exists;
the concrete types and algorithms below remain illustrative until matched to current source.

Current authority: accepted saves are immutable storage revisions; editorial/production and
publication approval are separate gates. Provider/render/cache examples below apply to external
producer tooling, not a Cantos API worker. Use the owning contracts and current source rather
than implementing these sketches as a replacement revision model.

## Contents

1. Records and states
2. Saves: compare-and-swap with idempotent retry
3. Submission
4. Enforcing immutability
5. Revision diff
6. Pinning, never "latest"
7. Identity across edits
8. Tests and mistakes

## 1. Records and states

```text
Draft (one per episode)                  mutable: only the head pointer and its sequence move
  └─ SavedVersion #1, #2, #3 …           append-only: content, base, operation ID, provenance
         │ submit(#3)                    the exact version the creator reviewed
         ▼
ScriptRevision R1, R2 …                  immutable: content, canonical bytes, digest, schema,
                                         scheme, provenance, the saved version it came from
         │
         ├─ pinned by production runs    (cantos-production-pipeline)
         └─ referenced by QC/approvals   (cantos-publication)
```

Business rule 3 permits mutable drafts; it does not require them. Making every save an immutable
saved version and keeping only the head pointer mutable satisfies "prior saved versions are not
altered", gives undo and history for free and reduces the draft to one compare-and-swap. Status
facts about a revision (superseded, withdrawn from a run) are append-only events that reference
it, never edits of its row.

```rust
// Illustrative and proposed. No `&mut self` method exists on a revision.
pub struct ScriptRevision {
    id: ScriptRevisionId,
    episode: EpisodeId,
    number: RevisionNumber,     // human-facing, per episode, monotonic
    from: SavedVersionId,
    content: ScriptContent,
    digest: ContentDigest,
    schema: SchemaVersion,
    provenance: RevisionProvenance,
}
```

Whether unsubmitted intermediate saves are retained forever or pruned is a product retention
decision. Never prune a version that a revision, a proposal or an audit record references.

## 2. Saves: compare-and-swap with idempotent retry

Every save names the head it was edited from and carries a client-generated operation ID. The
decision is pure; the shell executes it in one transaction.

```rust
// Illustrative and proposed.
pub struct SaveDraft {
    pub draft: DraftId,
    pub base_seq: u64,
    pub operation: OperationId,
    pub content: DraftScript,
}

pub enum SaveDecision {
    Append { seq: u64 },
    AlreadyApplied { version: SavedVersionId },          // same operation, same content
    Conflict { head_seq: u64, head: SavedVersionId },    // someone saved since base_seq
    OperationReused { version: SavedVersionId },         // same operation, different content
}

pub fn decide_save(
    head: &DraftHead,
    request: &SaveDraft,
    prior: Option<&SavedVersionSummary>, // the version already stored under this operation ID
) -> SaveDecision;
```

Check `prior` first: a retry whose first attempt committed must return `AlreadyApplied` even
though the head has moved past `base_seq`, or a lost response turns into a false conflict.

```sql
-- Illustrative: one transaction. Zero rows updated → roll back and return Conflict.
INSERT INTO script_saved_versions (id, draft_id, seq, base_seq, operation_id, content, ...)
VALUES ($id, $draft, $base_seq + 1, $base_seq, $op, $content, ...);

UPDATE script_drafts
   SET head_version_id = $id, head_seq = head_seq + 1
 WHERE draft_id = $draft AND head_seq = $base_seq;
```

A unique constraint on `(draft_id, operation_id)` closes the race where two retries of the same
operation both pass the pure check ([`persistence.md`](../../cantos-engineering/references/persistence.md)).

| Situation | Required result | Counterexample |
|---|---|---|
| two tabs save from the same base | the second gets `Conflict` with the current head and a diff against its base | last write wins |
| the response to a committed save is lost; the client retries | `AlreadyApplied` with the same version | a duplicate version or a false conflict |
| the server restarts mid-save | the transaction either committed or did not; reopen shows the last committed head | a half-written version visible as head |
| a client reuses an operation ID with new content | `OperationReused`, surfaced as a client bug | silently treating it as a new save |

On conflict, the editor shows both versions and the creator resolves; resolution is an ordinary
save whose base is the current head. Automatic merging is out of scope (real-time collaboration
is a [editor non-goal](../../../../docs/assurance/script-editor-acceptance.md#non-goals)); a
line-level three-way merge keyed by stable IDs is the natural later shape.

If content ever moves to object storage, write the object first under a content-addressed key,
then commit the row. An orphaned object is garbage; a row pointing at a missing object is
corruption.

## 3. Submission

```rust
// Illustrative and proposed.
pub enum SubmitError {
    NotHead { submitted: SavedVersionId, head: SavedVersionId },
    Rejected(ValidationReport),               // blocking diagnostics, all of them
    NothingChanged { existing: ScriptRevisionId },
}

pub fn decide_submit(
    head: &DraftHead,
    version: &SavedVersion,
    previous: Option<&ScriptRevision>,
) -> Result<NewRevision, SubmitError>;
```

- Submission names the saved version the creator reviewed, so a concurrent edit cannot slip
  unreviewed text into a revision. Rejecting a non-head version (`NotHead`) is the conservative
  proposal; whether an older version may be submitted is a product decision to record.
- Validation is `validate_for_submission` from
  [`schema-versioning.md`](schema-versioning.md#4-diagnostics-collect-all-at-the-human-boundary-fail-fast-inside):
  all blocking diagnostics at once.
- Canonical bytes and the digest are computed in the pure core before the transaction. The
  revision row, its canonical bytes and its digest are inserted together.
- Submitting the same saved version twice returns the same revision (unique constraint on the
  source version). Content identical to the previous revision returns `NothingChanged` rather
  than minting a revision with an equal digest; record this as a proposed policy.

## 4. Enforcing immutability

| Layer | Mechanism | Oracle | Status |
|---|---|---|---|
| domain | private fields, no `&mut self` methods, corrections build a new value | review; compile-time | proposed |
| database | application role without `UPDATE`/`DELETE` on revision tables, or a rejecting trigger | integration test: an `UPDATE` must fail | proposed |
| load | recompute the digest under the stored scheme from stored canonical bytes; mismatch → `RevisionCorrupted { id }` | test with a tampered row | proposed |
| API | no endpoint edits a revision; "edit" means a new save on the draft | contract review | proposed |

An in-memory store proves none of the database rows
([`immutability.md`](../../cantos-engineering/references/immutability.md)); report the database
claim as `integration-tested` only after it ran against PostgreSQL.

## 5. Revision diff

The diff is keyed by stable ID. A text or position diff reports a moved line as delete + insert,
which is the false invalidation this design exists to prevent.

**Implemented subset.** `script_ir::diff_scripts` and the read-only `diff-script` CLI
([evidence](../../../../docs/evidence/script-diff.md)) compare two admitted scripts of one episode and
report a flat, versioned list of added, removed, modified and moved entities with field aspects. It
takes two files, not revision rows, and it does not compute `respoken` or drive invalidation; the
`RevisionDiff` below stays the proposed shape for the revision store.

```rust
// Illustrative and proposed. A line absent from every map is unchanged; no "changed nothing" state.
pub struct RevisionDiff {
    pub added: BTreeSet<DialogueId>,
    pub removed: BTreeSet<DialogueId>,
    pub respoken: BTreeMap<DialogueId, SpokenFields>, // text | delivery | pronunciation | language
    pub recast: BTreeMap<DialogueId, SpeakerChange>,  // speaker_id changed
    pub moved: BTreeMap<DialogueId, Move>,            // order within a scene, or another scene
    pub scenes: BTreeMap<SceneId, SceneDelta>,        // cues, title, membership
    pub characters: BTreeMap<CharacterId, CharacterDelta>,
}

pub fn diff(previous: &ScriptContent, next: &ScriptContent) -> RevisionDiff;
```

A line may appear in `respoken` and `moved` at once. `respoken` is defined as "the line's
`SpokenContent` bytes differ" ([`canonical-digest.md`](canonical-digest.md)), so the diff and the
pipeline can never disagree about what changed in speech.

What each delta means downstream is owned by
[production pipeline § Instructions and external production](../../../../docs/product/production-pipeline.md#instructions-and-external-production)
and composed in [`fingerprints-and-invalidation.md`](../../cantos-production-pipeline/references/fingerprints-and-invalidation.md).
Script IR's job is to report the delta precisely:

| Delta | Script IR reports | Decided downstream |
|---|---|---|
| text, delivery, line pronunciation, language | `respoken` with the field set | speech re-render and dependent mixes |
| speaker | `recast` with old and new character | whether the resolved voice changed |
| order only, same scene | `moved` | the scene mix |
| another scene | `moved` with both scenes | both scene mixes |
| cue edit | `scenes[…].cues` | the scene mix only |
| characterization edit | `characters[…]` | whether resolved delivery depends on it |

The fingerprints remain the authority for speech reuse; the diff explains it to the creator
(stale clips, regeneration scope before it starts, per
[the UI system](../../../../docs/design/ui-system.md)) and cross-checks it.

## 6. Pinning, never "latest"

```rust
// Illustrative and proposed. The shell loads the stored row; the check itself is pure.
pub struct PinnedScriptRevision {
    id: ScriptRevisionId,
    digest: ContentDigest,
}

impl PinnedScriptRevision {
    /// The only constructor: recomputes the digest of an immutable stored revision under its
    /// stored scheme and refuses a mismatch.
    pub fn verify(stored: &StoredRevision) -> Result<Self, PinError>;
}
```

- Production runs, provider requests, QC results and approvals store `(revision ID, digest)`.
  Frozen production inputs are the pipeline's
  ([production instructions and external production](../../../../docs/product/production-pipeline.md#instructions-and-external-production)).
- The Studio head query returns a `DraftHead`, a different type that no production or publication
  function accepts. There is no `latest_revision()` in application code used by workers.
- An edit during a run never changes the run. Studio may label the run stale by comparing its
  pinned digest with the newest revision; retargeting is an explicit new run.

## 7. Identity across edits

| Edit | ID outcome | Speech outcome |
|---|---|---|
| move a line | keeps its ID | unchanged |
| fix a typo | keeps its ID | `respoken` |
| split a line | the creator-designated part (default: the first) keeps the ID; the rest get new IDs | all parts `respoken` or added |
| merge two lines | the survivor keeps its ID; the other is removed | survivor `respoken` |
| duplicate a line | the copy gets a new ID | added; byte-identical `SpokenContent` may be reusable downstream |
| delete then undo | restores the same ID and content | unchanged |

An ID is never reused for a different line later in the same script's history. IDs are minted by
the shell (an ID source passed into the edit function), so the core stays deterministic and
tests can use a fixed sequence.

## 8. Tests and mistakes

| Claim | Test | Evidence level |
|---|---|---|
| stale base never overwrites | `decide_save` table: stale base → `Conflict` | `example-tested` |
| retries are idempotent | committed-then-retried save → `AlreadyApplied`; integration with the unique constraint | `example-tested`, then `integration-tested` |
| revisions cannot change | `UPDATE` on a revision row fails against PostgreSQL | `integration-tested` |
| reopen after restart | save, restart the backend, reopen, compare digests | `integration-tested` |
| the diff is keyed by identity | property: reorder-only edit sequences → only `moved`/`scenes` populated | `property-tested` |
| the diff agrees with bytes | property: `respoken` keys equal the IDs whose `SpokenContent` bytes differ | `differentially-tested` |
| a run's inputs are pinned | edit and submit during a run; the run's revision and digest are unchanged | `example-tested` |

Mistakes that ship: an `updated_at`-based "last write wins"; `UPDATE script_revisions SET …` for
a "small fix"; a worker calling `latest()`; position-keyed diffs; a retry path that treats its own
committed save as a conflict; minting IDs inside the pure core from a random source.
