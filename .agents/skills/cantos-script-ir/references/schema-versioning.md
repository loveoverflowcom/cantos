# Script IR schema versioning and validation

> **Scope.** Version identifiers, the JSON Schema and the semantic validator, raw DTOs and their
> conversion into domain values, diagnostics for humans versus fail-fast domain errors, schema
> versus semantic compatibility, converters with provenance and the compatibility fixture corpus.
> Use when adding or changing a Script IR field, enum, version, converter, validator rule,
> fixture or any code that deserializes a script.

The product requirement is in
[Script IR § Revision and validation policy](../../../../docs/architecture/script-ir.md#revision-and-validation-policy)
and [`packages/contracts/README.md`](../../../../packages/contracts/README.md): a JSON Schema, a semantic validator,
explicit compatibility rules, actionable rejection of unsupported versions and converters that
preserve provenance. None of these exists yet; the shapes below are proposals to settle in the
first contract ADR.

## Contents

1. Two validators with one owner each
2. Version identifiers and dispatch
3. Raw DTO → validated domain value
4. Diagnostics: collect-all at the human boundary, fail-fast inside
5. Schema compatibility is not semantic compatibility
6. Converters
7. The compatibility fixture corpus
8. Tests and report

## 1. Two validators with one owner each

| Layer | Checks | Owner | Consumers |
|---|---|---|---|
| JSON Schema (Draft 2020-12, proposed) | shape: required fields, types, enum spellings, string limits, no unknown fields | `packages/contracts/` (proposed path `packages/contracts/schema/script-ir/<version>.schema.json`) | external producers, generated clients, editors with schema support |
| Semantic validator (Rust, pure) | cross-references and meaning: unique IDs, speakers resolve, cue anchors resolve, ordering, text invariants, submission completeness | the Script IR domain module | every Cantos entry path |

The Rust validator is authoritative. The JSON Schema is a published shape contract, so it must
never accept a document the Rust shape layer rejects or the reverse. Keep them in agreement with
a differential test over the fixture corpus, not by review:

```text
for every shape fixture f:   schema_accepts(f) == rust_shape_accepts(f)
for every semantic fixture:  schema_accepts(f) && rust_validate(f) == expected exact report
```

Whether the schema is hand-written or generated from Rust types (a `schemars`-style generator is
a candidate) is an ADR decision. If generated, the committed file names its source and generation
command ([`packages/contracts/README.md`](../../../../packages/contracts/README.md)) and a drift test regenerates it
and compares bytes.

## 2. Version identifiers and dispatch

| Rule | Why | Good | Counterexample |
|---|---|---|---|
| `schema_version` is required and parsed into a closed enum | a missing or free-form version is unreadable intent | `SchemaVersion::V0_1Draft` from `"0.1.0-draft"` | `Option<String>` defaulting to "latest" |
| the reader accepts an exact listed set; no ranges | a minor version may add meaning an older reader would drop | read `{0.1.0-draft, 0.2.0}` | accept any `0.*` |
| the writer emits exactly one current version | two write versions double the fixture matrix and the converters | `write_current(&ScriptContent)` | echoing the input's version back |
| a pre-release tag is its own version | `-draft` content has no stability promise | `0.1.0-draft` converts like any other version | treating `0.1.0-draft` as `0.1.0` |

Dispatch reads the version first with a minimal probe, then parses with that version's DTO. Avoid
a `serde_json::Value` round trip: it costs memory on large manuscripts and can lose number
fidelity.

```rust
// Illustrative and proposed.
#[derive(serde::Deserialize)]
struct VersionProbe {
    schema_version: Option<String>, // every other field is ignored by the probe only
}

/// Parses any readable version and upgrades it to the current raw shape.
/// Validation into domain values happens afterwards, in one place.
pub fn read_script(bytes: &[u8]) -> Result<Upgraded, ReadError> {
    let probe: VersionProbe = serde_json::from_slice(bytes).map_err(ReadError::Malformed)?;
    match SchemaVersion::parse(probe.schema_version.as_deref())? {
        SchemaVersion::V0_1Draft => Ok(v0_1::parse(bytes)?.upgrade()?),
        SchemaVersion::V0_2 => Ok(Upgraded::already_current(v0_2::parse(bytes)?)),
    }
}

pub struct Upgraded {
    pub raw: current::RawScript,
    pub notes: Vec<ConversionNote>,
}
```

The rejection must tell the creator what to do, in Studio copy and in the API error code:

```rust
// Illustrative and proposed.
pub enum VersionError {
    Missing,
    Unsupported { found: String, newest_supported: SchemaVersion, hint: VersionHint },
}

pub enum VersionHint {
    NewerThanThisServer, // "This script was saved by a newer Cantos. Update before importing."
    NotAScriptIrFile,    // the probe found no recognizable version at all
    Retired,             // an old version whose converter was deliberately removed
}
```

Retiring a readable version is a product decision with a migration plan; until then every
committed fixture of that version must keep reading.

## 3. Raw DTO → validated domain value

Each version has raw DTOs that mirror the wire exactly and carry no invariants. Domain types are
built only through constructors.

```rust
// Illustrative and proposed: wire shape for one version.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDialogue {
    pub id: String,
    pub speaker_id: Option<String>,
    pub text: String,
    pub delivery: RawDelivery,
    #[serde(default)]
    pub pronunciation_overrides: Vec<RawPronunciationOverride>,
}

// Domain: private fields, constructed only through validation.
pub struct Dialogue {
    id: DialogueId,
    speaker: CharacterId,
    text: SpokenText,
    delivery: Delivery,
    pronunciation: Vec<PronunciationOverride>,
}
```

| Rule | Failure mode | Oracle |
|---|---|---|
| no `#[derive(Deserialize)]` on a domain type with an invariant | serde builds a `SpokenText` that was never NFC-normalized or an empty `DialogueId` | a test that deserializes the invalid form directly and expects failure |
| leaf refined types use `#[serde(try_from = "String")]` when they must be deserializable | a fixture loader or row decoder bypasses the constructor | the same test, through every decoder that exists |
| `deny_unknown_fields` on every raw struct | a newer producer's field is dropped silently | an unknown-field fixture per struct level |
| no `#[serde(flatten)]` on raw structs | serde does not support `deny_unknown_fields` together with `flatten`; unknown fields slip through | review; the unknown-field fixtures catch regressions |
| tagged enums (`#[serde(tag = "kind")]`), never `untagged` | untagged enums accept the first variant that fits and report useless errors | a fixture whose shape fits two variants |
| no `#[serde(default)]` on a field that carries performance meaning | absent and "default" become indistinguishable, so missing intent looks deliberate | a missing-field fixture expecting `MissingField` |

Stored rows follow the same path: a row decoder produces a raw value and the domain constructor
validates it. An invalid stored revision is corruption or version skew, not user error: fail
loudly with the revision ID; never filter it out
([`boundary-hardening.md`](../../cantos-engineering/references/boundary-hardening.md),
[`persistence.md`](../../cantos-engineering/references/persistence.md)).

## 4. Diagnostics: collect-all at the human boundary, fail-fast inside

A creator fixing an imported script needs every problem at once; a domain function receiving an
already validated value needs none. Use both shapes, each in its place.

```rust
// Illustrative and proposed.
pub struct Diagnostic {
    pub path: IrPath,
    pub issue: ValidationIssue,
    pub severity: Severity,
}

pub enum Severity {
    Blocking, // prevents submission for production
    Review,   // visible to the creator; does not block by itself
}

pub enum ValidationIssue {
    DuplicateId { id: String, first_seen: IrPath },
    MissingSpeaker,
    UnknownSpeaker { speaker_id: String },
    EmptySpokenText,
    CueAnchorUnresolved { dialogue_id: String },
    CueLikeMarkupInSpokenText { excerpt: String },
    IntensityOutOfRange { permille: u32 },
    // one variant per rule; never a catch-all `Invalid(String)`
}

pub fn validate(raw: &RawScript) -> Result<ScriptContent, ValidationReport>;
```

- `validate` walks the whole document and accumulates. It calls the fail-fast leaf constructors
  (`SpokenText::new`, `DialogueId::new`) and turns each error into a `Diagnostic` with a path, then
  continues.
- `IrPath` addresses by stable ID where one exists
  (`episode/act:act-01/scene:scene-01/dialogue:dialogue-02/speaker_id`) and falls back to a JSON
  Pointer only before IDs are known. Index paths break as soon as the editor reorders lines.
- Diagnostics are sorted in document order so a test can assert the exact `Vec` and the editor
  list does not jump between saves.
- `Severity` is the validator's classification. Which findings block submission is product
  policy: link the owning document when a severity is chosen and report an undocumented choice
  as a gap.
- Inside the domain, functions take `ScriptContent` and return a single precise error. Never
  thread a `ValidationReport` through business logic.

Map diagnostics to HTTP at the edge: a stable error code per `ValidationIssue` variant, the path
and a Studio-safe message, never a Rust `Debug` string
([`http-api-boundary.md`](../../cantos-engineering/references/http-api-boundary.md)).

## 5. Schema compatibility is not semantic compatibility

A document can parse under a new schema and still mean something different. Classify every
change on both axes before writing code.

| Change | Old documents parse? | Same performance meaning? | Required action |
|---|---|---|---|
| add optional field whose absence equals the old behavior | yes | yes | new version; canonical encoding omits the field when absent so existing digests and `SpokenContent` bytes stay equal |
| add required field | no | — | new version and a converter supplying an explicit value plus a conversion note |
| rename a field or enum value | no | yes | converter; golden digests unchanged because the canonical form encodes the domain value |
| change a unit or scale (intensity `0.0–1.0` float → permille integer) | often yes: both are numbers | **no** | converter with a stated rounding rule, a property over the whole old domain and a fixture at each boundary |
| widen an enum (new emotion) | yes | yes | new version; old readers must reject documents using the new value rather than map it to a neighbor |
| narrow an enum (retire an emotion) | no | no | converter mapping to a named replacement with a note, or rejection naming the value |
| change ID format or scope | no | identity is at risk | never silent: a persisted old → new mapping and a migration plan |

The unit-change row is the one that ships bugs: `0.57 * 100.0` is `56.99999999999999` in `f64`,
so an `as u8` conversion yields `56`. Parse the decimal literal textually, or convert with
explicit rounding, and test every representable old value.

## 6. Converters

```rust
// Illustrative and proposed: one pure step per version.
pub fn upgrade(doc: v0_1::RawScript) -> Result<(v0_2::RawScript, Vec<ConversionNote>), ConvertError>;

pub struct ConversionNote {
    pub path: IrPath,
    pub kind: ConversionKind, // Renamed | Rescaled | Defaulted | Dropped | Approximated
    pub detail: String,       // e.g. "intensity 0.3 → 300 permille"
}
```

- Converters are pure, total over their input version and chained (`v0_1 → v0_2 → current`), so
  each step is tested once and the oldest fixture exercises the whole chain.
- A converter never consults the database, the clock or a provider. If a value needs a human
  decision, return a note with `Severity::Blocking` instead of guessing.
- Reading an old stored revision converts in memory. The stored bytes, digest and schema version
  are never rewritten ([business rule 5](../../../../docs/product/business-rules.md#content-and-revisions)).
- Persisting a converted copy creates a new draft version whose provenance records
  `ConvertedFrom { schema_version, source_revision, converter_version, notes }`.
- `Dropped` is allowed only for fields the target version declares lossy; anything else is a
  `ConvertError`. Losing performance meaning silently is the failure this section exists to
  prevent.

## 7. The compatibility fixture corpus

Proposed layout (confirm against the repository first):

```text
packages/contracts/fixtures/script-ir/
  0.1.0-draft/accept/two-speakers-narrator.json      + .expected (digest, notes)
  0.1.0-draft/reject/missing-speaker.json            + .expected (exact diagnostics)
  0.2.0/accept/…
  unsupported/0.9.0-from-future.json                 + .expected (VersionError)
```

- Every readable version keeps its fixtures forever. Adding a version adds a directory; it never
  edits an older one.
- Each reject fixture contains exactly one fault and expects the exact ordered diagnostics; one
  multi-fault fixture proves collection. Single-fault fixtures make an exact assertion possible.
- Each accept fixture records the expected content digest literally
  ([`canonical-digest.md`](canonical-digest.md)), so a converter or encoder change fails loudly.
- Fixtures use original or permitted text with realistic Vietnamese (`Ánh đèn cuối sân khấu`,
  `Người dẫn chuyện`, `Ngày mai, mình có diễn tiếp không?`), never a private manuscript.
- Expected files change only through a named regeneration command (proposed) and a PR that says
  why each value changed. A test harness never rewrites fixtures.

The existing [`episode-draft.json`](../../../../packages/contracts/examples/episode-draft.json) is a seed,
not yet a fixture: it stores intensity as a float, embeds `voice_profile_id` and a mutable
`rights_status`, and carries `revision`/`status` inside content. Decide each in the first schema
ADR ([`canonical-digest.md` § Field classification](canonical-digest.md#2-field-classification)).

## 8. Tests and report

| Claim | Test | Evidence level |
|---|---|---|
| every rule rejects its fault with the exact variant | table test over single-fault fixtures | `example-tested` |
| all faults are reported in document order | multi-fault fixture, exact `Vec` equality | `example-tested` |
| JSON Schema and Rust shape agree | differential run over the corpus | `differentially-tested` |
| old versions keep reading | every versioned accept fixture → expected digest | `differentially-tested` (golden) |
| refined types cannot be forged by serde | direct deserialization of invalid forms | `example-tested` |
| conversion preserves meaning | property over the old value domain; lossless converter preserves the digest | `property-tested` |
| assertions are strong | mutation run over the validator and converters | `mutation-tested` |

Report additions: read set and write version, the change class from § 5 on both axes, fixtures
added, converter notes introduced and whether any committed expected file changed.
