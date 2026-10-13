# Immutability: settled facts and value semantics

> **Scope.** Keep Cantos history trustworthy by never editing settled facts in place, and keep
> code predictable with value semantics. Covers immutable script revisions, frozen production
> inputs, append-only provider attempts, content-addressed artifacts, QC results, approvals,
> release manifests and progress operations; plus immutable values in Rust, Kotlin and
> Leptos/Compose state. Use when designing a table, a record type, an update path, a cache, or a
> state holder.

The product contract is in [business rules § Content and revisions](../../../../docs/product/business-rules.md#content-and-revisions)
and [production pipeline § Freeze production inputs](../../../../docs/product/production-pipeline.md#freeze-production-inputs).

## Two layers of immutability

| Layer | Meaning | Mechanism |
|---|---|---|
| **Settled facts** (data) | once accepted, a record never changes; corrections create a new record | append-only tables, new revision rows, content-addressed object keys, pointer swaps |
| **Values** (code) | a value does not change after construction; transformations return new values | owned values, private fields, no interior mutability in domain types, `val` + `copy` in Kotlin |

The first protects audit trails, approvals and caches. The second keeps reasoning local.

## Settled facts in Cantos

| Fact | Mutable phase | Settles when | After settling |
|---|---|---|---|
| Script revision | draft editing | submitted/accepted for production | new edits create a new revision with a new digest |
| Production inputs | none | a run is created | a changed draft starts a replacement run |
| Provider attempt | none | written before dispatch | outcome fields are appended once; never rewritten |
| Accepted artifact | none | checksum verified and accepted | a revised input produces a new artifact |
| QC result / finding | none | recorded against an artifact checksum | a new render gets new results |
| Approval | none | recorded with actor, scope and exact inputs | becomes *stale* by computation, never edited |
| Release manifest | staging assembly | marked ready | replacement is a new release; the active pointer moves |
| Progress operation | none | accepted by the server | later operations supersede it by revision |

"Stale", "superseded" and "retracted" are computed or recorded as *new* facts (a pointer move, a
retraction record), never by overwriting the old record's content.

Mutable rows are allowed for pointers and execution bookkeeping — the active-release pointer, a
job's lease owner and heartbeat — but each change must be a guarded transition with a
recorded reason, and the facts they point at stay immutable.

## Rules

| Rule | Failure it prevents | Good | Counterexample | Oracle |
|---|---|---|---|---|
| Never `UPDATE` content columns of a settled record | approval or cache silently refers to different content | insert revision 4; move draft pointer | `UPDATE dialogue SET text = $1 WHERE id = $2` on an accepted revision | migration review; a test that settled rows are byte-identical after an edit flow |
| Reference exact revisions, never "latest" | a run or approval drifts to new content | `ProductionRun { script_revision_id, script_digest }` | `SELECT … ORDER BY revision DESC LIMIT 1` inside a run | type: runs hold `ScriptRevisionId`, not `ScriptId` |
| Object keys are immutable identities | overwritten media under a published manifest | key derived from artifact ID or checksum | `episodes/{episode_id}/master.mp3` reused across renders | storage adapter test: second put to an existing key fails |
| Record attempts before effects | an ambiguous paid call cannot be reconciled | insert attempt row, then call the provider | call first, insert on success | fault-injection test killing the worker after dispatch |
| Derive, don't mirror | two sources of truth disagree | `approval.is_current_for(&candidate)` computed | a writable `is_stale` column updated by a trigger elsewhere | property test over input changes |

## Value semantics in Rust

- Domain types own their data and expose no `pub` fields; mutation goes through methods that
  preserve the invariant or, preferably, return a new value.
- Prefer `fn with_speaker(self, speaker: CharacterId) -> Self` or `fn apply(&self, edit) ->
  Result<Self, EditError>` over long-lived `&mut` state passed around.
- No `Cell`, `RefCell`, `Mutex` or `Arc<Mutex<_>>` inside domain values. Shared mutable state
  belongs to the shell (connection pools, caches) and is never visible to the core.
- `Clone` is cheap enough for small values; for large immutable collections consider `Arc<[T]>`
  or a persistent structure only after measurement.
- `&mut self` methods are acceptable for encapsulated builders and local algorithms whose
  intermediate states are never observed.

```rust
// Illustrative: an edit produces a new draft value; the old one is untouched.
impl ScriptDraft {
    pub fn replace_dialogue_text(
        &self,
        dialogue: DialogueId,
        text: SpokenText,
    ) -> Result<ScriptDraft, EditError> {
        let mut next = self.clone();
        let line = next.dialogue_mut(dialogue).ok_or(EditError::UnknownDialogue(dialogue))?;
        line.text = text;
        Ok(next)
    }
}
```

## Value semantics in Kotlin

Apply [FP-02/04 and TYPE-03/04](../../cantos-cmp-mobile/references/kotlin-conventions.md), the
canonical Kotlin rules for snapshots/deep immutability, invariant-preserving copy visibility,
retry-safe StateFlow transforms and exhaustive ADTs. `val`/`copy` alone proves neither validity
nor deep immutability; keep one definition of the Kotlin technique.

## UI state

Leptos signals and Compose state hold *values*. Replace the value; do not mutate an object inside
a signal behind the framework's back. Keep exactly one writable owner per fact and derive the
rest (`can_publish`, `is_empty`) as pure projections.

## Testing immutability

- Settled-record test: run the edit/regenerate/republish flow, then assert the earlier revision,
  artifact, approval and manifest rows are byte-identical to their snapshot.
- Rejection test: a rejected operation leaves state and rows unchanged.
- Storage test: writing to an existing immutable key fails or is a verified no-op for identical
  bytes.
- Property: for any edit sequence, every previously settled digest still recomputes to its stored
  value.

Report these as `example-tested` or `property-tested`. A private field or a missing mutator that
makes the edit unexpressible is `type-enforced`. A database privilege, trigger or constraint that
rejects the `UPDATE` is real enforcement, but its evidence is an `integration-tested` run against
PostgreSQL, not the migration text.
