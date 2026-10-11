# Script IR diff evidence

Review boundary: a standalone, read-only `diff-script` CLI and a pure `script_ir::diff_scripts`
function that compare two versions of one Script IR episode by stable ID and print a versioned JSON
report. It is a review aid for seeing what an AI or an editor changed. It is not a revision store,
a merge tool, a cache-reuse rule or an approval gate. No GitHub issue is linked yet; this record
does not complete or unblock any work item. Base: `4b2f786` on `develop`.

Delivers: the diff module, the CLI, code-built fixtures and tests. Does not deliver: revision
IDs or PostgreSQL reads (the CLI takes two files), HTTP or Studio surfaces, three-way merge,
word-level text diff, speech fingerprints or any decision about which audio can be reused,
approval invalidation, or AI calls. `apps/web`, the API contracts, migrations, TTS and CI are
untouched.

## Invariant and ownership

For two admitted versions of the same work, adaptation and episode, every difference in the
exported document is reported once, under the stable ID of the entity that differs, with the
field and the aspect it belongs to. A moved line is one `moved`, never a deletion plus an
addition, and inserting or deleting a sibling moves nobody. The report is a pure function of the
two inputs. Metadata is never hidden behind the content digest: provenance and rights references
are outside it, so equal digests with `identical: false` mean a metadata-only difference.

[`script_ir/diff/`](../../services/cantos-api/src/script_ir/diff/mod.rs) lives inside the Script IR module
because validated content has private fields; it has no HTTP, SQL, UI, provider, file-system or
clock dependency. It never parses JSON itself: both inputs go through `read_script`, the same
admission as `validate-script`, so text is NFC and whitespace normalized once and characters,
provenance sources, provenance references and pronunciation overrides are already in their
canonical order. [`diff-script`](../../services/cantos-api/src/bin/diff-script.rs) is the imperative
shell: argument parsing, a bounded read of each file, JSON output and exit codes. The only edit to
an existing source file is the export line in `script_ir/mod.rs`; no crate, dependency or
`Cargo.lock` change was needed. No ADR: the slice adds no dependency, schema, digest scheme or
persistence.

## What is compared

Entities are matched by `(kind, stable ID)`. Both scripts must share their work, adaptation and
episode IDs, otherwise nothing is reported and the CLI exits `1` naming every differing level.
Script IR keeps IDs unique across kinds within one script, but an ID may change kind between
versions; that is a different entity, so the old one is `removed` and the new one `added`.

| Entity | Fields compared (`field` → `aspect`) | Position |
| --- | --- | --- |
| `work` | `title` → title; `source_ref` → provenance; `rights_record_id` → rights | none |
| `adaptation` | `language` → language; `rights_record_id` → rights; `provenance_refs` → provenance | none |
| `episode` | `title` → title | none |
| `act` | `title` → title | index in its episode |
| `scene` | `title` → title | index in its act |
| `dialogue` | `speaker_id` → speaker; `text` → text; `delivery.emotion`, `delivery.intensity_permille` → delivery; `pronunciation_overrides` → pronunciation | index in its scene |
| `cue` | `kind`, `description`, `anchor.dialogue_id`, `anchor.edge`, `asset.id` → cue; `asset.rights_record_id` → rights | index in its scene |
| `character` | `name`, `role`, `personality` → character | none (kept sorted by ID) |
| `provenance` | `kind`, `source_record_id`, `generation_record_id` → provenance; `rights_record_id` → rights | none (kept sorted by ID) |

Optional values that are not set (`asset.*`, `generation_record_id`) are `null`. Characters and
provenance sources have no order, so they are only ever added, removed or modified.

## Report contract `cantos-script-diff-1`

| Field | Meaning |
| --- | --- |
| `report_version` | Report layout version; it changes whenever a field or its meaning changes |
| `schema_version` | The Script IR version both documents were read as (`0.1.0`) |
| `scope` | The shared `work_id`, `adaptation_id` and `episode_id` |
| `before`, `after` | `content_digest` of each side (`sir-c1:sha256:…`) |
| `content_digest_equal` | The two content digests match. Content excludes provenance and rights references |
| `identical` | There is no change of any kind, metadata included. Equivalent to equal `export_bytes()` |
| `summary` | `total`, `added`, `removed`, `modified`, `moved` and `by_aspect` |
| `changes` | Every difference, in the order below |

`summary.by_aspect` always lists all eleven aspects (`text`, `speaker`, `delivery`,
`pronunciation`, `order`, `cue`, `character`, `title`, `language`, `provenance`, `rights`), so
"no cue changes" reads as `0`. A change counts once under each aspect it involves; a move counts
under `order`; an added or removed entity counts under the aspects of its set fields.

Each element of `changes` has a `change` and an `entity`, a stable `id`, and:

| `change` | Extra fields |
| --- | --- |
| `added` | `values`: the set fields as `{ field, aspect, value }`; `at`: `{ parent, index }` for positioned entities |
| `removed` | the same, taken from the `before` version |
| `modified` | `fields`: each differing field as `{ field, aspect, before, after }` |
| `moved` | `from` and `to`, each `{ parent, index }` |

A line that was both edited and moved yields a `modified` and a `moved`. Indices are zero-based
positions among all siblings of that version, added and removed ones included. Order: entity kinds
as in the table above; within a kind, added, modified and moved changes in `after` document order,
then removed changes in `before` document order. Adding or removing a scene lists the scene and
each of its lines and cues individually.

**Moves.** An entity moved when its parent changed, or when it is not part of the longest run of
siblings (matched by ID) that kept their relative order, found as a longest increasing
subsequence in O(n log n). The number of reported moves within a parent is therefore the minimum.
When several minimal sets exist, for example after swapping two neighbours, the one reported is
deterministic but arbitrary: exactly one of the two is `moved`.

**Command.** `diff-script <before.json> <after.json>` prints the report to stdout. Each file is
refused above 2 MiB (`MAX_DOCUMENT_BYTES`, the Script IR admission bound) before it is read, and
again if it grows while being read; the limit is fixed. Exit codes: `0` the report was printed,
whether or not the versions differ; `1` an input was rejected by Script IR admission, or the
versions are not of one work, adaptation and episode (message on stderr, no report; validator
diagnostics are printed one JSON object per line, at most 20); `2` usage error, unreadable,
non-regular or oversized input, or an output failure (message on stderr, no report). It opens
inputs read-only and writes only to stdout.

## Claim and oracle ledger

| Claim / failure mode | Cheapest adequate oracle | Evidence |
| --- | --- | --- |
| Identical versions report nothing; the envelope is exact | Literal expected report | example-tested |
| Representation differences (character, provenance and reference order, NFD, spacing, NBSP, override order) are not changes | Reordered/NFD/spaced twin is `identical`, and its `export_bytes()` equal the original's | example-tested; differentially-tested against export bytes |
| Each of the 26 single-field edits surfaces exactly that entity, ID, field and aspect | Table of edits with literal expected values | example-tested |
| `identical` agrees with the canonical export; `content_digest_equal` is true exactly for metadata fields | Export bytes differ for all 26 edits; digest flag from the table (9 metadata fields) | differentially-tested |
| Changed speech fields match the line speech bytes | `SpokenLine::canonical_bytes` differs exactly for text, delivery and pronunciation edits (and for all lines on a language edit), never for speaker, cue or metadata edits | differentially-tested |
| Rights-only and provenance-only edits are visible although the content digests are equal | Three rights edits at once; the committed `two-scenes.json` versus `provenance.json` pair | example-tested |
| Moves are minimal and sufficient, indices are real, membership is exact | Independent textbook LCS table over every ordered selection of 1–6 lines from a pool of five existing lines and one new line (1,956 arrangements) | differentially-tested (bounded exhaustive) |
| A moved, inserted or deleted line never appears as delete + add | Literal reports for within-scene, cross-scene, scene, act and cue moves, insert at the front and delete in the middle | example-tested |
| Unicode is compared on normalized text and reported exactly | Diacritic-only edit, a non-BMP character, NFD twin, UTF-8 CLI output | example-tested |
| Different work, adaptation or episode is refused with every differing level | Exact `ScopeDifference` list and CLI message | example-tested |
| The CLI refuses bad usage, missing, directory and oversized input, and rejected scripts, naming which side, with no report | Exit codes and stderr for usage, missing, directory, `MAX` and `MAX + 1` bytes, six rejection classes in either position, the diagnostic cap | example-tested |
| The CLI is read-only and deterministic | Input files and their directory unchanged after four runs; byte-identical output twice | example-tested |
| Assertions kill injected defects | `cargo mutants` over the module and CLI | mutation-tested (see record) |

## Verification record

Environment: Ubuntu 26.04.1 LTS on Linux 7.0.0-38 x86_64, Rust 1.87.0, Python 3.14.4,
cargo-mutants 27.1.0. Fixtures are synthetic and built at test time from one in-code base document,
plus the two committed `accept/` fixtures; no manuscript, provider output or media is involved.

| Result | Exact command | Scope |
| --- | --- | --- |
| pass | `cargo +1.87.0 fmt --all -- --check` | formatting |
| pass | `cargo +1.87.0 clippy --workspace --all-targets --locked -- -D warnings` | lint |
| 169 passed, 0 failed, 40 ignored | `cargo +1.87.0 test --workspace --locked` | all Rust tests: 32 in `tests/script_diff.rs` and 4 unit tests of the move finder; the 40 ignored are the existing PostgreSQL suites, not run here |
| pass | `python3 scripts/reference_script_ir.py` | existing Script IR digest oracle; `script_ir/mod.rs` gained exports only |
| 96 mutants: 63 caught, 1 missed, 32 unviable | `RUSTUP_TOOLCHAIN=1.87.0 cargo mutants --file 'apps/server/src/script_ir/diff/*.rs' --file apps/server/src/bin/diff-script.rs -p cantos-server --jobs 4 --timeout 180 -- --test script_diff --lib` | module and CLI |
| pass | `python3 scripts/check_repository.py` (233 text files, 15 skills), `python3 -m unittest discover -s scripts -p 'test_*.py'` (68 tests), `git diff --check` | repository checks |

A first mutation run left five survivors, each traced to a weak assertion and fixed with a test
before the final run: trailing-space padding could not detect a read that stopped one byte early
(padding is now leading), the diagnostic cap had no boundary test (now 1, 20, 21 and 22 issues),
`summary.removed` was never asserted (now checked with per-aspect counts for a removed scene), and
strictness of the increasing-run finder was untested (now a unit test). The one remaining
survivor, `take(MAX_DOCUMENT_BYTES + 1)` → `take(MAX_DOCUMENT_BYTES * 1)`, changes behavior only
for a file that grows between the size check and the read, a race no test drives; the guard
rejects such a file after reading at most one extra byte. The 32 unviable mutants do not compile
(they need `Default`). Mutation logs stay under the ignored `artifacts/script-diff/`.

## Not covered / residual risk

- The diff is a reviewer's aid. A `modified` dialogue does not decide re-rendering, and an
  unmodified one is not proof that its audio is reusable: the production speech fingerprint also
  covers voice, provider, settings and pronunciation profile, none of which Script IR holds.
  Moves and cue edits are reported for the scene mix but are not mapped to any artifact.
- Both inputs must pass Script IR 0.1.0 admission. A draft with unresolved speakers or an invalid
  reference is refused instead of partially compared. The historical `0.1.0-draft` sample is
  unsupported.
- The CLI compares two files. It does not resolve revision IDs, verify a stored digest, read
  PostgreSQL or know who authored a change; no revision store was involved, so nothing here is
  `integration-tested`.
- Text changes are reported as whole before/after values, not as a word-level diff, and the
  report contains story text exactly as the inputs do: treat it as private as the manuscripts.
- Within a parent, equally small move sets are arbitrary (see Moves), so a swap of two lines
  names one of them. The count of moves is minimal; which line "moved" is not meaningful then.
- Memory is a few times the input size, bounded by two 2 MiB files. No coverage-guided fuzzing
  ran; the parsers it depends on are the existing admission code.
- The speech-bytes and export-bytes checks live in the tests only. The module itself does not
  compute `respoken`, and `report_version` should change if it ever does.
