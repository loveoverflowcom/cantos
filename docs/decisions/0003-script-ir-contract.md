# ADR 0003: Minimal validated Script IR and canonical content

- Status: proposed for maintainer review; implemented for the script persistence slice
- Date: 2026-10-09
- Decision owner: Cantos maintainers
- Related issue or PR: [#1](https://github.com/loveoverflowcom/cantos/issues/1)
- Supersedes / superseded by: N/A

## Context

The script revision backend needs a real validated value before it can persist revisions safely.
The repository's `0.1.0-draft` sample includes proposed lifecycle, casting and mutable rights
fields; it was explicitly not a contract. No predecessor implementation exists on the selected
`origin/develop` base. This slice therefore implements the minimum content foundation consumed
by its save/read API. Import, adaptation, casting and production remain separate work.

Owning product rules are [Script IR](../architecture/script-ir.md),
[business rules](../product/business-rules.md#content-and-revisions) and
[work item 010](../work-plan/010-import-and-edit-script.md).

## Decision

The only readable and writable schema is `0.1.0`, defined by
[`contracts/script-ir/schema.json`](../../contracts/script-ir/schema.json). There is no automatic
`0.1.0-draft` conversion: that illustration has no reliable source record and uses uncommitted
float delivery semantics. Unknown versions and fields are rejected. Future schema versions need
an explicit conversion and compatibility corpus; stored revisions are never rewritten in place.

A strict wire DTO converts into a private immutable `ScriptContent` through a pure accumulating
validator. Domain content has no `Deserialize`, public fields or mutable accessors. Candidate
values stay private until every issue has been checked. IDs and clocks are supplied by the caller;
there is no Axum, SQL, provider or storage dependency in validation or digest computation.

The minimum admitted shape retains Work → Adaptation → Episode → Act → Scene → Dialogue, explicit
characters and scene cues. Stable IDs use 1–64 ASCII lowercase letters, digits, `_` and `-`; all
entity IDs are globally unique within a script. Reference IDs are validated but do not mint new
entities. IDs are opaque; the server never derives them from position or text.

Exactly one character has the `narrator` role. All dialogues have an explicit resolving speaker;
cue anchors resolve inside their own scene. Work, episode, act and scene titles and dialogue text
are nonempty. Character, act, scene and dialogue collections are nonempty. An optional cue asset
requires both `asset_ref` and `rights_ref`; neither carries a public URL. Existence and ownership
of source/work rights records are application checks. Cue asset authorization remains a future
production/publication gate; accepting editorial content is not authorizing asset use.

Supported language tags are the explicit set `vi-VN`, `en` and `en-US`. Adding a language is a
reviewed contract change, not acceptance of arbitrary strings that a later provider cannot honor.
Delivery is optional, with a closed emotion enum and integer `intensity_permille` in 0–1000.
JSON integer spellings such as `300.0` and `3e2` normalize to the same integer 300, matching
JSON Schema mathematical integer semantics; fractional values are rejected.
Pronunciation permits at most 64 inspectable surface→respelling overrides, applied to
left-to-right non-overlapping literal occurrences of each surface;
missing and overlapping targets are rejected. Provider phonemes and per-span languages are deferred.

Derived human text is NFC, trimmed and whitespace-collapsed to U+0020. U+200B, U+FEFF and U+00AD
are removed; other admitted format-control ranges and non-whitespace C0/C1 controls are rejected.
Author punctuation and Vietnamese tone placement stay intact. Display labels have a 1024 UTF-8
byte storage limit, characterization/cue descriptions 4096, and dialogue text 16384. These are
storage bounds, not grapheme-count limits for a future editor. Preserved source bytes are not
normalized by this module.

### Canonical scheme

All admitted performance fields are included: stable entity IDs, titles, characterization,
roles, language, ordered structure, explicit speaker, dialogue text/delivery/pronunciation,
cue kind/description/anchor and selected asset identity. Version, work `source_ref`/`rights_ref`
and cue `rights_ref` are excluded because they identify wire syntax or external evidence.
Mutable rights status, casting profiles, audit metadata and revision lifecycle are never IR fields.

Characters form a set sorted by stable ID. Other arrays retain semantic order. Absent options and
empty optional lists are omitted. Keys are sorted ASCII; strings are NFC and encoded as compact
UTF-8 JSON; numbers are integers. The canonical encoder explicitly sorts keys independently of
serde feature flags. The complete JSON wire record retains source and rights references; canonical
performance bytes are not a transport document because they omit required envelope references.

The digest is `cantos-script-v1:sha256:<hex>` over the bytes
`b"cantos/script-content/v1\n" + canonical_performance_bytes`. SHA-256 is already a server
consumer for credential digests; `unicode-normalization` implements NFC and `serde`/`serde_json`
serve the HTTP and persistence wire boundary. Resolved dependency versions are pinned in
[`Cargo.lock`](../../Cargo.lock); dependency upgrades must preserve the golden vectors. No new
crate or service is added. The digest scheme is independent of wire schema version and must bump
when canonical classification or bytes change. It is not a pipeline speech fingerprint.

The content digest does not authenticate excluded references. The persistence adapter retains
and checks the complete normalized wire value and its source/rights reference columns on reads.
Publication and later production authorization must use exact revision metadata as well as the
performance digest.

## Alternatives

| Option | Benefits | Costs and limitations | Decision |
| --- | --- | --- | --- |
| Store arbitrary JSON | Small adapter | Admits unresolved speakers and silently drops meaning later | Rejected |
| Reuse the draft example unchanged | Familiar shape | Mixes lifecycle, rights status, casting and float performance data | Rejected |
| Full importer and production IR | More capabilities immediately | No current consumer for converters, render mapping or voice resolution | Deferred |
| Small strict version and pure validator | Reviewable, durable input to revisions | Explicitly limited language/pronunciation vocabulary | Selected |

## Consequences

Every save is complete enough for this editorial contract; incomplete editor buffers are not
saved revisions. Acceptance remains an editorial milestone, not production or publication
eligibility. Authorization, provenance existence, optimistic concurrency and actor history are
owned by the application and PostgreSQL adapter.

Import conversion logs, Narrative Forge exchange, per-dialogue `SpokenContent`, diff classification,
voice completeness and production freeze are deferred to their actual consumers. No speech cache
reuse or re-render behavior is claimed in this slice. In particular, work item 010 remains partial.

## Validation and revisit conditions

The committed corpus checks exact ordered issue codes/paths, a multi-fault report, unknown
version/field rejection and a two-scene Vietnamese accepted sample. Independent Python stdlib
canonical bytes and SHA-256 digests form the golden oracle. Focused Rust examples check NFC/NFD,
Windows-1258-derived Unicode, whitespace, tone placement, set versus semantic order, detached
immutable exports, field sensitivity and excluded references. The optional Python JSON Schema
oracle checks the same corpus's structural classification.

Revisit the contract when an actual importer needs incomplete drafts, another language is
required, a provider needs a new performance setting, or production consumes per-line speech
inputs. The evidence ledger records a bounded stable-ID mutation campaign. Broader generated-property,
whole-validator mutation and fuzz campaigns have not run; examples and independent golden
evidence do not establish arbitrary-input robustness.
