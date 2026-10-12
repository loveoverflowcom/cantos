# Versioned Script IR

Script IR `0.1.0` has an implemented [JSON Schema, reader and semantic contract](../../packages/contracts/README.md)
with [local test evidence](../evidence/script-ir-contract.md). The
[policy ADR](../decisions/0002-script-ir-contract.md) remains proposed for production adoption.
The unchanged [example](../../packages/contracts/examples/episode-draft.json) is historical illustrative
`0.1.0-draft` JSON, explicitly unsupported by this reader; it is not a Narrative Forge format.

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

The [contract](../../packages/contracts/README.md) specifies canonical content/export/speech encodings,
tagged SHA-256 digests and the exact read/write set `{0.1.0}`. Schema compatibility and semantic
compatibility both matter: accepting a new field must not silently discard performance meaning.
Reject unsupported versions with an actionable error; future explicit converters must preserve
provenance. No converter exists. The [local persistence slice](../evidence/script-revision-persistence.md)
stores every accepted validated save as an immutable complete export. Acceptance here means
storage admission, not editorial/production/publication approval. A full-export `sir-e1:sha256`
digest covers metadata excluded from c1; revision identity and access still remain authoritative.

The [editorial handoff extension](../decisions/0004-editorial-handoff.md) separately preserves
source text and records explicit owner reviews. New reviews require the current head; repeated
review of an already reviewed revision returns its original immutable fact after later edits.
Private source reads follow current script access and source evidence links. Storage acceptance,
editorial review, rights clearance and production/publication approval remain distinct gates.

[Production inputs v1](../../packages/contracts/production-v1.md) keeps casting/performance/pronunciation
outside the unchanged Script IR contract. A frozen planning candidate pins the complete
accepted revision/export/c1/e1, reference-only resolved lines, required rights claim versions and
budget/rate policy under `production-p1`. That scope includes evidence c1 deliberately excludes;
current input/cost approval cannot be inferred from storage acceptance, editorial review or c1.
The candidate produces no audio and is not a speech artifact cache fingerprint.

## TTS cache boundary

A speech fingerprint must include effective spoken text, language, pronunciation, delivery/emotion controls, voice profile revision, provider/model revision, synthesis parameters and output profile. Script revision IDs alone are neither sufficient cache keys nor a reason to rerender unchanged speech. Rights/tenant authorization is checked separately before reuse. The implemented `speech-v1` fingerprint and its
vector corpus are in the [clip reuse contract](../../packages/contracts/clip-reuse-v1.md).

Scene mix fingerprints additionally include ordered speech assets, timing, cue assets, gain/ducking instructions and mixing-tool/profile revisions. Episode fingerprints cover scene order, transitions and mastering/rendition settings. Changing a mix does not imply changing speech.

## Narrative Forge integration

Reuse concepts, not internal types by assumption. Introduce an import/export adapter only after inspecting the actual versioned Narrative Forge contract. Keep its mapping and round-trip fixtures at the boundary; log lossy conversions for review. Cantos remains runnable without that repository, database or service.
