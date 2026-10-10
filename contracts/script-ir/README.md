# Script IR 0.1.0

The backend implements this contract. [ADR 0003](../../docs/decisions/0003-script-ir-contract.md)
records its scope and compatibility policy. The pre-existing
[`0.1.0-draft` example](../examples/episode-draft.json) remains an illustration and is
**not** an accepted version or an implicit converter input.

- [JSON Schema](schema.json): strict JSON shape, with no unknown fields.
- [Accepted original Vietnamese sample](fixtures/accepted/episode.json): two speakers and two scenes.
- [Rejection expectations](fixtures/rejected/expected.json): shape and semantic cases, with exact
  ordered `code` and JSON Pointer `path` diagnostics.
- [Golden canonical bytes](fixtures/accepted/episode.canonical.json) and
  [digest](fixtures/accepted/episode.digest.txt): independent Python stdlib oracle. The canonical fixture has one final LF for repository
  text-file hygiene; that LF is excluded from the canonical bytes and digest.

Schema validation is the first layer. The authoritative pure Rust validator in
[`script_ir.rs`](../../apps/server/src/script_ir.rs) also checks IDs, reference resolution,
completeness, supported language, text, pronunciation and cue rights pairing. A JSON Schema
pass alone never creates accepted content. Optional scalar/object fields admit `null` as absence;
optional lists admit absent or empty arrays, never `null`. The canonical writer omits absent
options and empty lists.

`source_ref` and `rights_ref` resolve through the authorized revision API. They are not URLs,
media paths, inline rights status or evidence of permission. The IR cannot assert that a source
record exists or that publication is allowed. Lifecycle state, actor, timestamps and original
source/provenance live in the revision and source records.

## Canonical content

The complete wire form preserves references and emits `schema_version: "0.1.0"`. Performance
canonical bytes exclude the version, work source/rights references and cue rights references.
All other admitted fields are included. Character order is a set sorted by stable ID; ordered
acts, scenes, dialogue, pronunciation and cue arrays keep their order. Keys are ASCII and sorted;
strings are normalized NFC; the encoder emits compact UTF-8 JSON with integer numbers.

The digest is `cantos-script-v1:sha256:<64 lowercase hexadecimal digits>` over
`b"cantos/script-content/v1\n" + canonical_bytes`. It identifies performance content and is
**not** a speech cache key or an integrity checksum for the revision's provenance envelope.
The stored canonical wire value and reference columns must also agree on reads.

To reproduce the golden independently:

```sh
python3 contracts/script-ir/check_fixtures.py
```

The dependency-free command checks canonical bytes/digest. With the optional `jsonschema`
package installed, `python3 contracts/script-ir/check_fixtures.py --schema` additionally checks
all fixtures against Draft 2020-12. The Rust test `committed_rejection_corpus_has_exact_ordered_issues`
checks exact semantic errors; `committed_golden_matches_independent_python_canonical_bytes_and_digest`
checks the independent golden. Run both through `cargo test -p cantos-server script_ir`.
