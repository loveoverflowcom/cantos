# Versioned Script IR

Script IR `0.1.0` has an implemented [JSON Schema, reader and semantic contract](../../contracts/README.md)
with [local test evidence](../evidence/script-ir-contract.md), and the
[script revision backend](../../apps/server/README.md) persists it as immutable revisions. The
[policy ADR](../decisions/0002-script-ir-contract.md) remains proposed for production adoption.
The unchanged [example](../../contracts/examples/episode-draft.json) is historical illustrative
`0.1.0-draft` JSON, explicitly unsupported by this reader and rejected by the live API; it is not
a Narrative Forge format.

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

Assign stable IDs independently of row position. Moving a dialogue changes scene sequencing but
does not inherently change its spoken render. The implemented contract rejects duplicate IDs,
missing/unknown speakers, empty sequence levels, invalid references and unknown schema versions;
array order is the only sequence representation. Required voice casting remains a separate
production input gate, not a Script IR validation claim.

Treat imported/AI-adapted content as editable drafts. Accepting a script produces an immutable revision and content digest. Provider requests and approvals reference that revision, not a mutable "latest" row. An edit produces a new revision and invalidates only dependent render/QC/approval artifacts.

The [contract](../../contracts/README.md) specifies canonical content/export/speech encodings,
tagged SHA-256 digests and the exact read/write set `{0.1.0}`. Schema compatibility and semantic
compatibility both matter: accepting a new field must not silently discard performance meaning.
Reject unsupported versions with an actionable error; future explicit converters must preserve
provenance. No converter is implemented; persisted revisions store the complete canonical export
([ADR 0004](../decisions/0004-editorial-persistence.md)).

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
