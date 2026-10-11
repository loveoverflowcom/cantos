# Canonical serialization and content digests

> **Scope.** The canonical byte encoding of validated Script IR values, which fields a content
> digest covers, the digest algorithm and its version prefix, the per-dialogue `SpokenContent`
> encoding handed to the production pipeline, golden digest fixtures and the equivalence laws
> that keep one value to one encoding. Use when changing the domain model, the encoder, the
> digest format, text normalization or anything stored next to a digest.

[Script IR § Revision and validation policy](../../../../docs/architecture/script-ir.md#revision-and-validation-policy)
requires a canonical serialization and digest policy before the first contract implementation;
[§ TTS cache boundary](../../../../docs/architecture/script-ir.md#tts-cache-boundary) says why
revision IDs alone are not cache keys. Nothing here is implemented. Record the final policy in
an ADR ([template](../../../../templates/adr.md)) and treat this reference as the method.

## Contents

1. Two values, two encodings, one owner
2. Field classification
3. Canonical encoding rules
4. Digest format and algorithm
5. Versioning the scheme
6. Tests
7. Mistakes that ship

## 1. Two values, two encodings, one owner

| Value | Encodes | Consumer | Must change when |
|---|---|---|---|
| `ContentDigest` of a `ScriptContent` | everything performed or shown from the script, in order | revisions, pinning, approvals, "nothing changed" detection | any classified-in field changes, including order |
| `SpokenContent` canonical bytes, per dialogue | effective language, spoken text, line delivery, line pronunciation overrides | the pipeline's speech fingerprint ([`fingerprints-and-invalidation.md`](../../cantos-production-pipeline/references/fingerprints-and-invalidation.md)) | a speech-affecting script field of *that* line changes |

Both are computed by Script IR from validated domain values. The pipeline never reaches into the
IR to pick fields for itself, and Script IR never folds voice, provider, model or output profile
into its bytes. That split is what lets a casting change invalidate speech without a new script
revision, and a reorder create a new revision without invalidating speech.

`SpokenContent` excludes the dialogue ID, act, scene, position and neighboring lines, so moving a
line or duplicating it with the same text yields identical bytes. Whether identical bytes from
two lines may share one artifact is the pipeline's reuse and rights decision, not Script IR's.

## 2. Field classification

Classify every field before it ships, in a reviewed table kept with the schema. Proposed starting
classification, using the seed [`episode-draft.json`](../../../../packages/contracts/examples/episode-draft.json):

| Field | Content digest | `SpokenContent` | Reason |
|---|---|---|---|
| work/adaptation/episode/act/scene/dialogue/cue/character IDs | in | out | identity of the content; speech is position- and identity-independent |
| titles (work, episode, act, scene) | in | out | listener-visible content; not spoken unless written into a line |
| act, scene and dialogue order | in | out | sequence is content; a reorder must not re-render speech |
| adaptation `language` | in | in, as the effective language of each line | it changes pronunciation |
| character `name`, `personality`, role | in | out | characterization; casting resolution is the pipeline's input |
| dialogue `speaker_id` | in | out (carried beside it in `SpokenLine`) | resolves the voice; the voice revision enters the fingerprint |
| dialogue `text`, `delivery`, `pronunciation_overrides` | in | in | the spoken performance |
| cue `kind`, `description`, `anchor`, `asset_id` | in | out | a cue change reruns the mix, not speech |
| character `voice_profile_id` (in the seed) | **out — move out of content** | out | casting is separately revisioned; a recast must not create a script revision |
| work `rights_status`, cue/work `rights_record_id` (in the seed) | **out — status is mutable** | out | rights are records owned by publication and rechecked at gates |
| episode `revision`, `status`, adaptation `revision` (in the seed) | **out — lifecycle metadata** | out | lifecycle lives in the revision record; inside content it changes the digest on submit |
| `schema_version` | out | out | an encoding detail; the scheme tag travels in the digest string |
| provenance, source spans, AI attempt IDs, review marks | out (stored immutably with the revision) | out | evidence of origin, not performance |
| author, timestamps, editor state, comments | out (not in the IR at all) | out | presentation and audit metadata |

The bold rows conflict with the seed fixture. Resolve them in the first schema ADR rather than
encoding today's shape by accident; until then, report them as open.

Two consequences to state in every change touching this table:

- An "out" field can still matter. Rights and approvals are checked by their owners against the
  revision ID; excluding them from the digest moves the check, it does not remove it.
- Moving a field from out to in changes every digest. That is a scheme version bump (§ 5).

## 3. Canonical encoding rules

Encode the validated domain value, never the input bytes. Proposed: a JSON subset that stays
compatible with RFC 8785 (JSON Canonicalization Scheme), so an independent JCS implementation in
another language can serve as a differential oracle.

| Rule | Failure it prevents | Good | Counterexample |
|---|---|---|---|
| encode from `ScriptContent`, after validation | key order or whitespace in an upload changes the digest | two key orderings of one script → same bytes | `sha256(request_body)` |
| object keys are fixed ASCII names sorted by code point | locale or UTF-16 sorting differences across languages | `{"delivery":…,"id":…,"speaker_id":…,"text":…}` | maps keyed by display name |
| arrays keep semantic order; sets are sorted by stable ID | insertion order of a set leaks into the digest | characters sorted by `id` | characters in the order the AI listed them |
| strings are already NFC and whitespace-normalized by their types | NFD input from another keyboard looks like an edit | `SpokenText` guarantees NFC; the encoder only asserts it | normalizing inside the encoder, so some paths skip it |
| integers or fixed-point only; no floats | `0.3` vs `0.30` vs `3e-1`, `-0`, and cross-language float printing | `"intensity_permille":300` | `"intensity":0.3` |
| one representation for absence: omit the field | `null`, `[]` and missing become three digests for one value | omit `pronunciation_overrides` when empty | `"pronunciation_overrides":null` |
| enum values have explicit, frozen spellings | renaming a Rust variant silently changes every digest | a hand-written encoder or explicit `rename` on a dedicated canonical struct | encoding with the wire DTO's derived names |
| no insignificant whitespace, UTF-8 without BOM, minimal escaping | serializer settings drift between versions | `SpokenText` rejects control characters, so only `"` and `\` are ever escaped | pretty-printed canonical form |

Omit-when-default needs one discipline: absence and the default must be the *same domain value*.
Then adding an optional field in a new schema version leaves every existing digest and every
`SpokenContent` byte string unchanged, which is what keeps unchanged speech reusable across a
schema upgrade.

Prefer a small hand-written encoder over generic `serde_json::to_vec` of a derived struct. The
encoder is the contract; a derive ties it to Rust identifiers, field declaration order and serde
behavior across upgrades.

```rust
// Illustrative and proposed: the encoder is explicit about every byte.
impl SpokenContent<'_> {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = CanonicalWriter::new();
        out.object(|o| {
            o.field("delivery", |d| self.delivery.encode(d));
            o.field("language", |v| v.str(self.language.as_str()));
            if !self.pronunciation.is_empty() {
                o.field("pronunciation", |v| v.array(self.pronunciation, |p, v| p.encode(v)));
            }
            o.field("text", |v| v.str(self.text.as_str()));
        });
        out.into_bytes()
    }
}
```

`CanonicalWriter` (proposed) asserts sorted keys in debug builds, so a field added out of order
fails a unit test instead of producing a second encoding.

## 4. Digest format and algorithm

```text
sir-c1:sha256:<64 lowercase hex>
│      │      └─ hash of: domain tag ‖ 0x0A ‖ canonical bytes
│      └─ algorithm
└─ canonical scheme: Script IR, canonical encoding version 1
```

- **Algorithm.** SHA-256 is the proposed default: it is available in Rust, Kotlin, browsers,
  Python and PostgreSQL `pgcrypto` for independent verification, and scripts are small enough
  that speed is irrelevant. BLAKE3 is a candidate only if a measured need appears. Record the
  choice in an ADR.
- **Domain separation.** Hash `b"cantos/script-content/c1" ‖ 0x0A ‖ bytes` for content and a
  different tag for `SpokenContent`, so identical bytes in two roles never collide by
  construction. The exact tag spelling belongs in the ADR.
- **The string is self-describing.** Store and compare the whole string, never the bare hex.
  Comparing digests with different scheme tags returns an error, not `false`.

```rust
// Illustrative and proposed. No `PartialEq` across schemes: comparison is explicit.
pub struct ContentDigest {
    scheme: CanonicalScheme, // closed enum: C1, later C2, …
    sha256: [u8; 32],
}

impl ContentDigest {
    pub fn same_content(&self, other: &Self) -> Result<bool, SchemeMismatch> {
        if self.scheme != other.scheme {
            return Err(SchemeMismatch { left: self.scheme, right: other.scheme });
        }
        Ok(self.sha256 == other.sha256)
    }
}
```

## 5. Versioning the scheme

The schema version describes the wire; the canonical scheme describes the encoding of the domain
value. They move independently.

| Change | Scheme bump? | What happens to stored digests |
|---|---|---|
| new schema version, lossless converter, new optional field omitted when absent | no | unchanged; the golden corpus proves it |
| field moves between in and out, encoding rule changes, enum spelling changes | yes (`c1` → `c2`) | stored digests keep their `c1` tag and stay valid for `c1` |
| hash algorithm change | yes | as above |

- Never recompute and overwrite a stored digest. Integrity checks recompute under the stored
  scheme.
- To compare an old revision with a new one under a new scheme, read both, convert in memory and
  canonicalize both under the current scheme. Never compare a `c1` string with a `c2` string.
- A scheme bump must state its effect on `SpokenContent`. If `SpokenContent` bytes change, every
  speech fingerprint changes and the pipeline would re-render everything; that needs explicit
  review and a pipeline-side plan, not a side effect of a refactor.

## 6. Tests

| Claim | Oracle | Evidence level |
|---|---|---|
| the encoding is stable | golden fixtures: each accept fixture's expected digest committed as a literal string | `differentially-tested` |
| equivalent inputs → one encoding | property: permute keys, re-space JSON, convert text to NFD, vary absent/empty forms → identical bytes | `property-tested` |
| canonical form is idempotent | property: `canonical(decode(canonical(x))) == canonical(x)` | `property-tested` |
| every in-field is covered | property or table: change exactly one in-field → digest changes | `property-tested` |
| every out-field is excluded | change exactly one out-field (provenance, review mark) → digest unchanged | `example-tested` |
| reorder never touches speech | property over reachable scripts: a reorder-only edit changes `ContentDigest` and leaves every `SpokenContent` unchanged | `property-tested` |
| schema upgrade keeps speech | every `0.1.0-draft` fixture: upgraded `SpokenContent` bytes equal those of the same script authored in the current version | `differentially-tested` |
| other implementations agree | an independent JCS/SHA-256 implementation (e.g. a stdlib Python script, proposed) reproduces the golden digests | `differentially-tested` |
| the encoder's assertions are real | mutation run over the encoder: a dropped field or swapped key must fail a test | `mutation-tested` |

Generate script values for properties through the public constructors and legal edits, never by
filling private fields
([`property-and-differential-testing.md`](../../cantos-engineering/references/property-and-differential-testing.md)).
A failing property's shrunk case becomes a committed `#[test]` before the fix.

## 7. Mistakes that ship

- Hashing request bytes or a pretty-printed export, so formatting is part of identity.
- Floats anywhere in hashed data, including "just for intensity".
- `#[serde(default)]` plus `skip_serializing_if` on some fields and not others, so absence has
  two encodings.
- Re-normalizing text inside the encoder: it hides entry paths that skipped `SpokenText::new`.
- Using the content digest as the speech cache key, which re-renders every line after a title
  edit.
- Recomputing stored digests after an encoder change, which makes approvals point at values that
  never existed.
- A test that computes the expected digest with the same encoder. The golden literal is the
  oracle; the encoder is the thing under test.
