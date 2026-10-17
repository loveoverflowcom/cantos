---
name: cantos-script-ir
description: >-
  Rule owner for Cantos Script IR and editorial revisions: the Work → Adaptation → Episode → Act →
  Scene → Dialogue → Audio hierarchy as validated typed values with stable IDs, an explicit
  narrator and typed sound cues; schema versions, converters and compatibility fixtures;
  canonical serialization and content digests; immutable revisions, optimistic draft saves and
  revision diffs; untrusted TXT/Markdown/DOCX import and AI adaptation output; the Narrative
  Forge exchange boundary; Vietnamese Unicode text. Use when touching Script IR schemas,
  fixtures, validators, dialogue or cue models, script save/submit, digests, NFC normalization,
  import parsing or adaptation output.
---

# Cantos Script IR

Rule owner for how Cantos represents, versions, digests, revises and admits script content. The
product rules live in [Script IR](../../../docs/architecture/script-ir.md),
[business rules § Content and revisions](../../../docs/product/business-rules.md#work-chapter-and-episode-identity),
[Script/editor acceptance reference](../../../docs/assurance/script-editor-acceptance.md)
and [`packages/contracts/README.md`](../../../packages/contracts/README.md). This skill owns the working method: how
to model those rules as types, where each decision lives, how to test it and which evidence
proves it. It composes [`cantos-engineering`](../cantos-engineering/SKILL.md) and reuses its
order, [evidence vocabulary](../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
and [report](../cantos-engineering/SKILL.md#9-completion-report) without redefining them.

## Ground rules

- **Implemented slice.** [Script IR 0.1.0](../../../packages/contracts/README.md), its schema/validator,
  immutable PostgreSQL revisions, bounded import and caller-authored adaptation exist.
  Provider inference and historical `a1` runtime adapters are outside the API; production
  planning and external media acceptance have their own current contracts. [`episode-draft.json`](../../../packages/contracts/examples/episode-draft.json) is a
  historical `0.1.0-draft` sample rejected by the reader. Inspect current code/tests before
  treating the proposed types and examples below as implemented.
- **Docs win for product behavior.** When this skill and an owning document disagree, follow the
  document, report the conflict and fix the stale side when it is in scope.
- **The IR is a pure domain value.** It imports no Axum, SQL client, Leptos, Compose, provider
  SDK, `serde_json::Value` or Narrative Forge type. Wire DTOs, rows and provider payloads convert
  at the edge ([`decoupling.md`](../cantos-engineering/references/decoupling.md)).
- **Library names are candidates.** `serde`, `unicode-normalization`, `unicode-segmentation`,
  `sha2`, `jsonschema`, `proptest` and `cargo-mutants` each need a recorded decision
  ([ADR template](../../../templates/adr.md)) and a pinned, verified version before use.

## Ownership

| This skill owns | It hands off to |
|---|---|
| hierarchy types, stable IDs, speakers, cues, provenance shape | — |
| schema versions, raw DTOs, converters, validator, diagnostics, fixtures | HTTP mapping → [`http-api-boundary.md`](../cantos-engineering/references/http-api-boundary.md) |
| canonical encoding, content digest, per-line `SpokenContent` | fingerprint composition → [`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md) |
| draft saves, immutable revisions, revision diff, pinning | approval invalidation → [`cantos-publication`](../cantos-publication/SKILL.md) |
| import, adaptation-output validation, conversion logs | rights evidence → publication; cost records → pipeline |
| Narrative Forge exchange adapter | — |
| Vietnamese normalization, offsets, pronunciation data | editor input → [`script-editor.md`](../cantos-leptos-web/references/script-editor.md); display → [`cantos-ui-design`](../cantos-ui-design/SKILL.md) |

## The required order for a Script IR change

```text
read script-ir.md, business rules § Content and revisions, the work item, packages/contracts/
        ↓
classify: content model | wire schema | canonical encoding | lifecycle | untrusted entry path
        ↓
state the compatibility effect: stored revisions, fixtures, digests, SpokenContent bytes
        ↓
raw DTO → accumulating validator → domain value; close every other construction path
        ↓
keep validate · convert · canonicalize · digest · diff pure (no I/O, clock or ID minting)
        ↓
write fixtures first: accepted, rejected with the exact variant, golden digest
        ↓
implement; focused tests, then properties; mutation-test the validator when it changes
        ↓
report with the Script IR fields (§ Completion report)
```

## The typed hierarchy

| Concept | Proposed domain shape | What the type guarantees |
|---|---|---|
| Work | `Work { id, title, source: SourceRef, rights: RightsRef }` | rights are a reference, never a status copied into content |
| Adaptation | `Adaptation { id, language: LanguageTag, characters }` | BCP 47 tag; character IDs unique |
| Episode → Act → Scene | `Vec` order is sequence; no index fields | each ID unique within the script |
| Dialogue | `Dialogue { id, speaker: CharacterId, text: SpokenText, delivery, pronunciation }` | speaker resolves; text is non-empty, NFC, whitespace-normalized |
| Character | `Character { id, name, role, characterization }` | narration has a real speaker (`Người dẫn chuyện`); casting lives outside |
| Sound cue | `SoundCue { id, kind: CueKind, anchor: CueAnchor, asset: Option<AssetRef> }` | closed kind; anchor resolves inside its scene |
| Audio | not a field | production manifests associate audio with dialogue identity |

Drafts admit the incomplete states an editor needs, each as a named variant; submission converts
them into the strict types or returns every reason it cannot:

```rust
// Illustrative proposed draft API; inspect the implemented strict reader separately.
pub enum SpeakerRef {
    Resolved(CharacterId),
    Unresolved { label: String }, // e.g. an AI-proposed "Bà cụ" not yet in the cast
}

pub fn validate_for_submission(draft: &DraftScript) -> Result<ScriptContent, ValidationReport>;
```

`ScriptContent` has no `SpeakerRef`, so production code cannot receive an unresolved speaker.

## Rules

Each rule states the failure it prevents, a good example and a counterexample, its oracle, its
enforcement status and its exception. Today every rule is enforced by manual review only; the
named tests are proposed until the code exists.

**SIR-1 Mint stable IDs; never derive them.** IDs are opaque, ASCII (`[a-z0-9_-]`, bounded),
unique within the script and minted by the shell, then passed into the core.
Fails when an ID comes from position, text or a content hash: a reorder becomes delete + add, a
typo fix orphans its approvals. Good: moving `dialogue-02` keeps its ID. Counterexample:
`format!("{scene}-{index}")`, or keeping IDs an AI response invented. Oracle: reorder property,
`DuplicateId` table test. Exception: fixtures may use readable IDs; code treats them as opaque.

**SIR-2 Every line has an explicit speaker.** Narration is a dialogue spoken by the narrator
character. Fails when `speaker_id: null` or a magic `"narrator"` string means narration: casting,
QC and invalidation cannot see it. Good: `speaker_id: "narrator"` resolving to a character named
`Người dẫn chuyện`. Counterexample: an `is_narration: bool` beside an optional speaker. Oracle:
`MissingSpeaker` and `UnknownSpeaker` table tests. Exception: none in submitted content; drafts
may hold `SpeakerRef::Unresolved`.

**SIR-3 Sound cues are typed events, never text.** Fails when `[tiếng mưa rơi]` sits inside
spoken text: TTS reads it aloud and the mix cannot find it. Good: a `CueKind::Ambience` cue
anchored to a dialogue edge. Counterexample: cue instructions in `text` or in a free-form note
the mixer parses. Oracle: `CueLikeMarkupInSpokenText` review finding;
`CueAnchorUnresolved` table test. Exception: bracketed text the creator confirms is spoken.

**SIR-4 Audio is referenced, never embedded.** The IR holds no bytes, URLs, object keys or render
status. Fails when a render detail enters content: every render changes the digest and private
storage paths leak into exchanged files. Good: a production manifest maps a dialogue's
`SpokenContent` digest to an asset. Counterexample: `dialogue.audio_url`. Oracle: the schema has
no such field and rejects unknown fields. Exception: none.

**SIR-5 Raw → validated → domain.** Wire DTOs are raw; domain types with invariants never derive
`Deserialize` over private fields; `#[serde(try_from = "Raw…")]` or an explicit `TryFrom` is the
only path. Fails when serde, a row decoder or a fixture loader constructs what the constructor
rejects. Oracle: a deserialization test per refined type that must fail. Exception: types with no
invariant beyond their primitive.
→ [`schema-versioning.md`](references/schema-versioning.md),
[`boundary-hardening.md`](../cantos-engineering/references/boundary-hardening.md)

**SIR-6 Version explicitly; reject what you cannot read faithfully.** A closed set of readable
versions, one write version; an unknown version or an unknown field fails with an actionable
error; converters are pure, logged and provenance-preserving. Fails when a reader drops a field
it does not understand and silently loses performance meaning. Good:
`UnsupportedSchemaVersion { found: "0.3.0", newest_supported: "0.2.0" }`. Counterexample:
`#[serde(default)]` everywhere plus ignored unknown fields. Oracle: the compatibility fixture
corpus. Exception: none for content; envelope metadata may define its own policy.

**SIR-7 Canonicalize validated values; digest semantics, not presentation.** One encoding per
domain value: sorted ASCII keys, semantic array order, NFC text, integers or fixed-point only,
omit-when-default, a scheme tag in the digest. Fails when key order, NFD input or float
formatting changes a digest and fakes a content change. Good: intensity as permille `300`.
Counterexample: hashing the uploaded bytes, or `0.3` as `f64`. Oracle: golden digest fixtures and
equivalence properties. Exception: none; a new encoding is a new scheme version.
→ [`canonical-digest.md`](references/canonical-digest.md)

**SIR-8 Revisions are immutable; drafts move by compare-and-swap.** Saved versions and submitted
revisions are append-only; a save names its base and a stale base is a conflict, never a silent
overwrite; a retried save with the same operation ID returns the first result. Fails when two
tabs overwrite each other or a lost response turns a retry into a conflict. Oracle: conflict and
idempotent-retry tests; an integration test proving revision rows reject `UPDATE`. Exception: the
draft head pointer is the only mutable field.
→ [`revision-lifecycle.md`](references/revision-lifecycle.md)

**SIR-9 Pin revisions; never resolve "latest".** Production runs, provider requests, QC and
approvals reference `(ScriptRevisionId, ContentDigest)`. Fails when a run started on revision 3
renders revision 5's text. Good: `PinnedScriptRevision` constructible only by loading an immutable
revision and re-verifying its digest. Counterexample: `script_repo.latest(episode_id)` inside a
worker. Oracle: an edit-during-run test asserting the run's inputs are unchanged. Exception:
Studio views may show the head, labeled as a draft.

**SIR-10 Every untrusted source enters as a draft through one validator.** Imports, AI
adaptation output and Narrative Forge files keep their preserved source, provenance and a
conversion log; a failure never replaces the last valid script. Fails when an adaptation invents
`Bà cụ`, drops a paragraph or writes over the draft head. Oracle: a hostile-output fixture corpus
and a failure test asserting the head and source are unchanged. Exception: none.
→ [`import-and-adaptation.md`](references/import-and-adaptation.md),
[`narrative-forge-adapter.md`](references/narrative-forge-adapter.md)

**SIR-11 Normalize text once, at every entry path.** `SpokenText::new` applies NFC and the
whitespace policy; offsets declare their unit; pronunciation overrides are typed data, not inline
markup. Fails when the NFD form of `Người dẫn chuyện` from another input method shows up as a
changed line and re-renders paid speech. Oracle: NFC/NFD vector tests through every entry path.
Exception: preserved source bytes stay byte-exact; only derived text is normalized.
→ [`vietnamese-text.md`](references/vietnamese-text.md)

## The split with the production pipeline

Script IR owns the *content*; the pipeline owns the *fingerprint*. Script IR supplies, per
dialogue, the effective script-side speech inputs — resolved language, spoken text, line delivery
and line pronunciation overrides — as a position-independent canonical value, plus the speaker
used to resolve casting. The pipeline combines those bytes with voice profile revision,
provider/model, synthesis settings, pronunciation profile and output profile
([`fingerprints-and-invalidation.md`](../cantos-production-pipeline/references/fingerprints-and-invalidation.md)).

```rust
// Illustrative and proposed: the only speech surface Script IR exposes to production.
pub struct SpokenLine<'a> {
    pub dialogue: DialogueId,
    pub speaker: CharacterId,     // resolves casting; not itself speech content
    pub content: SpokenContent<'a>,
}

impl ScriptRevision {
    pub fn spoken_lines(&self) -> impl Iterator<Item = SpokenLine<'_>>;
}

impl SpokenContent<'_> {
    pub fn canonical_bytes(&self) -> Vec<u8>; // excludes ID, position, scene, act, neighbors
}
```

Script IR guarantees: the bytes change exactly when a speech-affecting script field changes;
moving a line never changes them; lossless schema conversion and unrelated optional additions
never change them. If an adapter sends neighboring lines as prosody context, that context is a
new effective input the pipeline must add; flag it, because the position-independence guarantee
no longer covers that render.

## Minimum evidence for a Script IR change

Apply the foundation's [cheapest-regression rule](../cantos-engineering/SKILL.md#5-every-behavior-change-gets-the-cheapest-deterministic-regression-evidence);
these are the Script IR minimums.

| Change | Minimum evidence |
|---|---|
| validation rule | single-fault fixture per rule asserting the exact `ValidationIssue` and path; one multi-fault fixture asserting the whole ordered report |
| schema or converter | every committed fixture of every readable version still reads; unknown version and unknown field rejected exactly; converter notes asserted |
| canonical encoding or digest | committed golden digests unchanged, or a scheme bump with every changed golden listed; equivalence and sensitivity properties |
| lifecycle | stale-base conflict, idempotent retry and edit-during-run tests; immutability `integration-tested` once PostgreSQL exists |
| import or adaptation | malformed and hostile corpus with exact errors; coverage of omitted source; source and head unchanged on failure |
| text handling | NFC/NFD and Windows-1258 vectors from [`vietnamese-text.md`](references/vietnamese-text.md) through each entry path |

Properties worth writing: decode ∘ encode round trip; canonical idempotence; equivalent inputs
(key order, NFD, whitespace) → identical bytes; reorder-only edits leave every `SpokenContent`
unchanged; the revision diff agrees with a naive per-ID byte comparison. The validator, canonical
encoder, converters and diff classifier are pure and small: they are the first
[mutation-testing](../cantos-engineering/references/mutation-and-formal.md) candidates.

## Completion report

Use the [foundation report](../cantos-engineering/SKILL.md#9-completion-report) and add:

```text
Schema version:            read {…}; write …; change: none | additive | breaking (+ converter)
Fixture set:               <paths>; accepted N, rejected M (exact variants); added/changed
Digest scheme:             <tag, algorithm>; golden digests changed: none | list + reason
Migration / compatibility: old fixtures read: yes/no; stored revisions rewritten: never; notes
SpokenContent impact:      unchanged | changed for <fields> → expected re-render scope
```

## References — load the one the change touches

| The work is about… | Reference |
|---|---|
| version identifiers, raw DTOs, JSON Schema, diagnostics, converters, the fixture corpus | [`schema-versioning.md`](references/schema-versioning.md) |
| canonical bytes, field classification, digest format, golden digests, equivalence laws | [`canonical-digest.md`](references/canonical-digest.md) |
| drafts, saved versions, submission, conflicts, revision diffs, pinning | [`revision-lifecycle.md`](references/revision-lifecycle.md) |
| TXT/Markdown/DOCX/structured import, AI adaptation output, provenance, adaptation cost | [`import-and-adaptation.md`](references/import-and-adaptation.md) |
| Narrative Forge exchange, mapping tables, round-trip fixtures, lossy conversion | [`narrative-forge-adapter.md`](references/narrative-forge-adapter.md) |
| NFC/NFD, offsets, search and compare, whitespace, pronunciation data, test vectors | [`vietnamese-text.md`](references/vietnamese-text.md) |

## Compose with

[`cantos-engineering`](../cantos-engineering/SKILL.md) first —
[`types-as-proofs.md`](../cantos-engineering/references/types-as-proofs.md),
[`boundary-hardening.md`](../cantos-engineering/references/boundary-hardening.md),
[`immutability.md`](../cantos-engineering/references/immutability.md),
[`functional-core.md`](../cantos-engineering/references/functional-core.md),
[`persistence.md`](../cantos-engineering/references/persistence.md) and
[`property-and-differential-testing.md`](../cantos-engineering/references/property-and-differential-testing.md).
Fingerprints, regeneration and paid-call cost go to
[`cantos-production-pipeline`](../cantos-production-pipeline/SKILL.md); rights evidence and
approval invalidation to [`cantos-publication`](../cantos-publication/SKILL.md); the Studio
editor to [`cantos-leptos-web`](../cantos-leptos-web/SKILL.md) with
[`cantos-ui-design`](../cantos-ui-design/SKILL.md).
