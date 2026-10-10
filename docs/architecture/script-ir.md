# Versioned Script IR

Status: the [0.1.0 contract](../../contracts/script-ir/README.md) and pure Rust validator are implemented with canonical bytes, digests and fixtures. The old [example](../../contracts/examples/episode-draft.json) remains an illustrative `0.1.0-draft` design input and is rejected by the live API. It is not a Narrative Forge format.

## Purpose

Preserve the hierarchy **Work → Adaptation → Episode → Act → Scene → Dialogue → Audio** without tying Cantos to Narrative Forge storage or code. Script IR describes performance content and source provenance. Production manifests separately associate stable dialogue/scene identities and their exact revisions with audio assets.

| Concept | Required meaning |
| --- | --- |
| Work | Identity of the original source and rights/provenance references |
| Adaptation | A specific dramatization, language and version of that source |
| Episode | Ordered acts and an immutable accepted script revision |
| Act / Scene | Dramatic grouping and sequence; scene sound cues and timing intent |
| Dialogue | Stable identity, explicit speaker, spoken text, delivery intent and optional pronunciation overrides |
| Character | Consistent characterization and a separately revisioned voice/casting profile |
| Audio | Generated rendition referenced by a production manifest, never embedded bytes in Script IR |

Narration uses an explicit narrator character/speaker. Sound effects, music and ambience are typed cues rather than instructions hidden in spoken text. Each cue has an explicit anchor and rights reference when an actual asset is selected.

## Revision and validation policy

Assign stable IDs independently of row position. Moving a dialogue changes scene sequencing but does not inherently change its spoken render. Duplicate IDs, missing speakers, invalid ordering, unresolved required voice casting and unknown schema versions must be caught before production.

Treat imported/AI-adapted content as editable drafts. Accepting a script produces an immutable revision and content digest. Provider requests and approvals reference that revision, not a mutable "latest" row. An edit produces a new revision and invalidates only dependent render/QC/approval artifacts.

The first contract pins its JSON Schema, semantic validator, canonical serialization/digest policy and compatibility fixtures in [contracts/script-ir](../../contracts/script-ir/README.md). Schema compatibility and semantic compatibility both matter: accepting a new field must not silently discard performance meaning. Reject unsupported versions with an actionable error; explicit converters preserve provenance.

## TTS cache boundary

A speech fingerprint must include effective spoken text, language, pronunciation, delivery/emotion controls, voice profile revision, provider/model revision, synthesis parameters and output profile. Script revision IDs alone are neither sufficient cache keys nor a reason to rerender unchanged speech. Rights/tenant authorization is checked separately before reuse.

Scene mix fingerprints additionally include ordered speech assets, timing, cue assets, gain/ducking instructions and mixing-tool/profile revisions. Episode fingerprints cover scene order, transitions and mastering/rendition settings. Changing a mix does not imply changing speech.

## Narrative Forge integration

Reuse concepts, not internal types by assumption. Introduce an import/export adapter only after inspecting the actual versioned Narrative Forge contract. Keep its mapping and round-trip fixtures at the boundary; log lossy conversions for review. Cantos remains runnable without that repository, database or service.

## Implemented editorial persistence

The [server API](../../contracts/http/script-api.md) stores every save as an immutable version
behind a guarded draft head. Acceptance records the exact reviewed saved version; later edits
leave accepted content unchanged. Source and rights references are private immutable records,
with rights assertions pending review. New acceptances require the current head; retrying an
already accepted version returns the original revision. These are editorial facts, not production
or publication approval. See [ADR 0004](../decisions/0004-editorial-persistence.md) and the
[evidence ledger](../evidence/script-revisions.md).
