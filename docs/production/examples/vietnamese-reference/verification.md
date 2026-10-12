# Vietnamese reference episode — local evidence

Scope: [issue #44](https://github.com/loveoverflowcom/cantos/issues/44), reference text and derived
[review notes](episode_0001.r0001.review.md) only. No new reader/schema, application behavior,
backend records, provider call, production run, media or audio is delivered.

## Exact inputs and structural result

- Base: freshly fetched `origin/develop`, `7478236c1a23b045dc93fa690b65a9edc9215fb5`.
- Local branch: `feat/vietnamese-reference-scene`, created from that exact base on a clean tree.
  The prior CMP branch remains at `84eb0ca251a89a2f62d9bf9fc636df849dc7a95e`.
- Working directory: `/Users/manhblue/projects/loveoverflow/cantos` on macOS ARM64.
- Candidate: [episode_0001.r0001.script-ir.json](episode_0001.r0001.script-ir.json).
- File SHA-256: `240a4a8ac0d0f5663ae39311bd6c9b8201e495b9fdbd3ceb27442e1f07a30719`.
- Reader: existing `cantos-api` `read_script`, through `validate-script`;
  `rustc 1.87.0 (17067e9ac 2025-05-09)`, `cargo 1.87.0 (99624be96 2025-05-06)`.

Exact command from the repository root:

```sh
RUSTUP_TOOLCHAIN=1.87.0 cargo run --locked --bin validate-script -- docs/production/examples/vietnamese-reference/episode_0001.r0001.script-ir.json
```

Exit 0; stdout:

```text
sir-c1:sha256:696d318ba17c3386064af03a93bdff33f90b78c5c46e0edc6e1417ca70aac92f
```

Evidence level: `example-tested` for this file's shape, normalized text and internal references
through the actual reader. File SHA-256 identifies full authored bytes; `sir-c1` identifies the
canonical performance-content projection. They are different scopes. The reader does not
resolve source/generation/rights registry records, select a cast or establish listening quality.

## Claims and oracles

| Claim / invariant | Owner / failure mode | Oracle → evidence level |
| --- | --- | --- |
| Two-scene Vietnamese reference, four explicit roles, complete audition coverage | #44; missing a question, exclamation, name or tone challenge weakens later auditions | Entire JSON against the per-character and passage tables → `documented` editorial review |
| Every craft technique, cue and narrator line has an ID-bound reason | Craft guide / radio score; intent could be lost or a hidden direction spoken | Coverage, narrator, cue and control tables against all 30 dialogues / 12 cues → `documented` review |
| Only existing 0.1.0 fields, valid speakers and same-scene anchors | Script IR reader; structural rejection or unintended field dropping | Exact CLI above → `example-tested` |
| Original text, declared AI provenance, no acquired story or selected media | #44 / business rules; acquired content or invented rights clearance could be shared | Authorship and unregistered-reference declaration plus full changed-path audit → `documented` |
| Doc placement keeps old contract corpus, goldens and behavior unchanged | #44 / Script IR owner; new fixture expectations could drift | Changed-path review plus applicable local verification; results below |

No core, new type barrier, migration or ADR is needed: this adds a reader-consumed content example
and review guidance, not a new executable decision. Meaningful regression evidence for this
input is the existing reader; no test of a duplicate parser/validator was added.

## Local verification

All commands run from the working directory above with code at the base SHA and this seven-path
documentation/example change present. The exact reader was rerun after the dialogue fixes;
its final file hash and stdout are the values above. Logs remain under ignored
`target/issue-44-evidence/`; they are not media or a production record.

| Command | Actual result | Local output |
| --- | --- | --- |
| `RUSTUP_TOOLCHAIN=1.87.0 cargo run --locked --bin validate-script -- docs/production/examples/vietnamese-reference/episode_0001.r0001.script-ir.json` | Exit 0, exact `sir-c1` above | `validate.stdout.log`, `validate.stderr.log` |
| `RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp cargo xtask verify repository` | `PASS repository`; 202 Python tests | `repository.log` |
| `RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp cargo xtask verify rust` | Final `PASS rust-format`, `PASS rust-lint (workspace)`, `PASS rust-tests (workspace)`; 647 passed, 113 existing ignored tests | `rust.log` |
| `RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp cargo xtask verify contracts` | `PASS script-ir-reference`, `PASS contract-tests`, `PASS wav-reference`; 186 passed, 3 existing ignored tests | `contracts.log` |
| `RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp cargo xtask preflight --changed-only` | `PASS repository`, `PASS rust-format`; seven paths, no affected Cargo package | `preflight.log` |

The first sandboxed Rust run printed `FAIL rust-tests (workspace)`: eleven existing loopback HTTP
tests could not bind `127.0.0.1:0` (`PermissionDenied`, `Operation not permitted`). That output is
retained in `rust-sandbox.log`. The permitted local rerun outside that sandbox passed the whole
suite without changing source or tests; contracts also ran with local loopback permitted. These
HTTP tests use a scripted local transport, not Google or any paid provider.

The default Cargo lanes retain their existing ignored PostgreSQL, opt-in provider and specialized
tests; no `--ignored` run or real-database result is claimed. PostgreSQL, mutation, Web/WASM,
CMP/mobile and browser/runtime gates were not selected by changed-only preflight and were not
run: no persistence, validator/encoder or UI implementation changed. The extra Rust/contracts
suites meet the repository's Script IR verification rule; they do not give this reference audio
or production acceptance. No remote CI was polled.

A read-only whole-file inspection checked that all 30 dialogue and 12 cue IDs appear in the
derived notes, that the referenced IDs exist, all four speakers have questions/exclamations,
text is NFC, all deliveries are neutral/0, no assets are selected and the note hash matches the
JSON. Counts are 2 scenes, 4 characters, 30 lines, 12 cues and 643 spoken units (341 + 302).
This is `example-tested` metadata/coverage inspection, not a second semantic validator or an
implemented craft checker.

## Independent content, contract and coverage review

The separate read-only Codex reviewer `reference_review` assessed the final JSON and derived
notes as **no-actionable-findings**, with **complete-for-declared-scope** coverage: all 30
dialogues, 12 cues, four narrator reasons, ten craft rows, six listening passages and the four
related documentation changes. It inspected the actual wire/validation/canonical reader, CLI,
Schema and existing fixture. Its metadata audit checked all six tones in each audition phrase.
It read the main lane's CLI/repository/Rust outputs without rerunning Cargo or calling providers.

Review snapshot: JSON SHA-256 is the value above; derived review-note SHA-256 is
`ee99bd2fec127473c9665660ce7f6468b1b0fb1f11d2629adc4504f388241849`.
The reviewer rechecked these confirmed corrections against the initial draft:

- `ref-d03` now says “Lại đây” and recognizes Thư's voice from `ref-d02`, matching the friends
  already inside the room and Du's entrance; no unrepresented knock is assumed.
- `ref-d19` now expresses hope for a letter, preserving the first box opening at
  `ref-d24` / `ref-c-open-box`.
- The acoustic-control mapping separates all four narrator lines and private `ref-d12` from
  ordinary scene dialogue, so an affected-ID range cannot imply the wrong perspective.

Stable IDs were retained through all fixes. This is editorial/contract review, not production
approval; gate outputs and commit-byte freshness belong to the implementation handoff.

Schema: read `{0.1.0}`, write `0.1.0`, change **none**. Existing accept/reject corpus and golden
digests are unchanged. Stored revisions are never rewritten. New dialogue speech inputs comprise
`vi-VN`, the original line text and neutral/0 delivery; there is no prior render to invalidate.

Run/step identity: none. Crash points, duplicate-call oracle and accounting: not applicable,
no pipeline effect changed. Provider evidence: none, live and mocked generation **NOT_RUN**.
Audio facts: no audio rendered, no profile selected; `audio-measured` absent.
Listening: **NOT_RUN**, `audio-listened` absent. Remote CI: not checked, no push/PR authorized.

Residual risk: performance length is estimated; current 0.1.0 cannot encode the intended pauses,
interior mode, overlaps or cue extent. External evidence references are unregistered examples;
backend admission, owner rights decisions, cast/assets and actual listening remain separate gates.
