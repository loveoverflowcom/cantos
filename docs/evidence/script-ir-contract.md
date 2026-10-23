# Script IR contract evidence

## Scope and provenance

Work item: [010](../assurance/script-editor-acceptance.md), contract criterion of
[#1](https://github.com/loveoverflowcom/cantos/issues/1). Base: `develop` / `origin/develop` at
`7e393361f412b87a34b8121528de78416b721e8c`; implementation branch `feat/script-ir-contract`.
The base tree was clean. The historical `contracts/examples/episode-draft.json` is unchanged.
Fixtures reuse its original sample and add synthetic Vietnamese dialogue and evidence IDs;
there is no private manuscript, copyrighted story, downloaded media or provider response.

Review boundary: validate and export an original structured episode through the real Rust CLI;
does not deliver a Studio save/retrieve journey, Axum host, migrations, identity/permissions,
accepted revision persistence, importer, Narrative Forge adapter or production gate. This is
local domain/CLI evidence; no PostgreSQL, object storage, AI or device integration was invoked.
The [ADR](../decisions/0002-script-ir-contract.md) is proposed for production adoption, not accepted
by an invented owner. [The contract](../../services/cantos-server/contracts/README.md) records the implemented policy.

## Completion ledger

Invariant: admission returns immutable content only after complete shape and semantic checks;
identity/order/reference/evidence meaning survives canonical export, and moving unchanged dialogue
does not alter its speech bytes. An invalid candidate cannot alter a previous value because the
reader has no writes, global state, clock or ID generation.

Owner / boundary: `apps/server/src/script_ir` is the pure domain/codec module. Raw DTOs are private
to it; only `validation::into_content` builds domain fields after validation. `ScriptContent`
has no public fields, setters, Deserialize or Default. It exposes immutable exports/digests and
read-only speech lines. Domain `model` uses no `serde_json::Value`, HTTP, SQL, UI, AI provider or
Narrative Forge type. Wire JSON decoding and the file CLI are separate boundaries. There is one
server Cargo package, consumed by the CLI and test corpus, and no speculative port/service.

Schema version: read `{0.1.0}`; write `0.1.0`; first implemented contract, with deliberate
rejection of the older unsupported illustrative `0.1.0-draft` seed. No converter exists and
no stored revisions are rewritten. Fixture set: 2 accepted documents, 24 rejected documents
with a literal cases manifest, plus the historical rejection and programmatically varied shape,
text, ordering and digest cases. Semantic fixtures assert complete exact reports; shape-rule
fixtures assert exact JSON Pointer/rule reports. Deserialization fixtures assert their stage.

Digest scheme: `sir-c1:sha256`, hashing `cantos/script-content/c1` + LF + content bytes. First
goldens were authored by `scripts/reference_script_ir.py --write-goldens`, independently of Rust;
there were no prior goldens to replace. Full canonical exports preserve source/generation/rights
references excluded from content digests. Speech bytes include effective language, text, delivery
and pronunciation; exclude IDs, order, speaker and cue fields. The pipeline must still combine
casting/provider/model/settings/output inputs and check rights before reuse.

| Claim | Oracle / actual seam | Evidence level |
| --- | --- | --- |
| Exact versions, closed shapes, non-empty ordering levels, bounded IDs/text/arrays | Committed rejection corpus and JSON Schema differential checks against `read_script`; missing/unknown/null at every object; numeric and boundary examples | `example-tested`, `differentially-tested` |
| Stable IDs and ordering | Reorder scenes/dialogues: digest changes, original IDs and speech map remain equal; cast set order canonicalizes | `example-tested` |
| Speakers/narrator/cue locality/provenance links | Single-fault and multi-fault literal diagnostics in `cases.json` against the real validator | `example-tested` |
| Pronunciation targets and overlap | Missing target and overlapping respelling fixtures with exact paths/issues | `example-tested` |
| Metadata stays present without granting rights | Synthetic original/imported/generated provenance and changed rights references survive export; content/speech digest stays equal | `example-tested` |
| Canonical content and per-line speech bytes | Python `json`/`hashlib` byte and digest literals compared to Rust output | `differentially-tested` |
| Vietnamese normalization/equivalence/sensitivity | NFD, combining-mark order, Windows-1258-decoded mixed form, whitespace/removed characters, tone placement, punctuation, field edits; canonical round trips | `example-tested` |
| Canonical length after normalization | U+0344 expansion regression failed before the guard and passed after it | `example-tested` |
| No unchecked content construction via external serde/fields | Private module/fields, one validation conversion, no domain Deserialize/Default, no public mutation path; source audit | `type-enforced`, `statically-checked` (compile/lint), source review |
| CLI calls real admission and rejects a bad speaker | Actual compiled CLI subprocess, digest compared to a literal and invalid-input exit code 1 | `example-tested` |
| Important guards/encoder assertions fail on defects | Seven fixed mutations in a disposable copy, baseline test suite and actual mutated suite results | `mutation-tested` within the named substitutions only |

These are bounded examples and differential checks, not a property/fuzz campaign or proof of
provider, database or application behavior. No fake backend/provider is used to claim an
integration result. Placeholder evidence records do not establish permission.

## Commands and observed results

Working directory: repository root. Toolchain: Cargo 1.87.0, rustc 1.87.0, rustfmt 1.8.0,
clippy 0.1.87; Python 3.13. Commands execute against this implementation's working tree before
the focused commit. Final commit/remote/CI identity is reported in the delivery message.

| Command | Result / scope |
| --- | --- |
| `cargo fmt --all -- --check` | passed; Rust formatting |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | passed; Rust lint |
| `cargo test --workspace --locked` | passed; 12 Script IR integration-file tests executing the real library/CLI; library/bin/doctest targets select zero tests and are not counted as additional coverage |
| `python3 scripts/reference_script_ir.py` | passed; both accepted byte/digest goldens reproduced by the independent oracle |
| `TMPDIR=/private/tmp python3 scripts/mutate_script_ir.py` | passed; 7 killed, 0 survived, 0 timeout, 0 unviable after a passing 12-test baseline |
| `python3 scripts/check_repository.py` | passed; text/links/JSON/skill structure only |
| `TMPDIR=/private/tmp python3 -m unittest discover -s scripts -p 'test_*.py'` | passed; 44 bootstrap/initializer tests |
| `python3 -m py_compile scripts/reference_script_ir.py scripts/mutate_script_ir.py` | passed; Python syntax only; there is no configured/installed Python formatter |
| `git diff --check` | passed; whitespace |

Initial sandbox Cargo DNS access was blocked; the authorized dependency download succeeded via
sandbox escalation. Git fetch also needed escalation for read-only Git metadata. No remote
authentication/access denial was bypassed. Python temp tests use `/private/tmp` to avoid the
pre-existing `/var`/`private/var` alias issue, without changing unrelated tests.

Mutation scope (runner v1): skip duplicate identity guard; admit unknown speaker; allow zero
narrators; skip same-scene cue anchor check; skip work provenance resolution; skip normalized
length guard; encode all speech intensities as zero. All are viable and killed by assertions;
there are no survivors to classify. This finite set says nothing about other possible mutations.

## Self-review and coverage

Entered through `cantos-code-review`: pinned base and inventoried the complete local diff; reviewed
source/DTO/construction paths, Schema, every accepted/rejected fixture and literal oracle, root
manifest/lockfile, test/runner code, CI and every changed doc against base policy. No second agent
or independent reviewer was used. The review did not modify the tree; fixes belong to the
implementation loop. Assessment for the final declared scope: `no-actionable-findings`.

Review snapshot SHA-256: `b971c70033a0f6ab1cedcf6f45d98d557f676fabe953cd927892e8fb392950b0`.
It hashes sorted tracked/untracked regular-file path + NUL + bytes + NUL, excluding symlinks,
ignored artifacts and this report to avoid self-reference. Mutation input SHA-256 values:
`validation.rs` = `fde69dfa25c85c2340cab09d903a7b2c8ba6b1c28ae6872717923fc5c98ae2e0`;
`canonical.rs` = `93d110361fb44ae904a571f3e711e47f98adc564c58481e3a33b0a4ad769ebe6`.

| Changed durable claim | Required by | Assurance disposition |
| --- | --- | --- |
| Version, identity, speakers, order, cues and text validity | issue:#1 contract criterion; policy:cantos-script-ir SIR-1/2/3/5/6/11 | held by exact fixture/shape tests and public-boundary source audit |
| Canonical encoding/digest and script-side speech separation | doc:architecture/script-ir; policy:cantos-script-ir SIR-7 | held by independent goldens and field/order examples |
| Source/generation/rights references remain inspectable | doc:product/business-rules content/rights rules | held for shape/references/export; external eligibility remains outside this slice |
| Settled content cannot be mutated or forged by callers | policy:cantos-engineering types/immutability/core | held for in-memory domain values; database immutability is not applicable here |
| Reproducible contract verification and accurate implementation claims | policy:AGENTS.md; policy:cantos-engineering evidence | held for local commands; CI must be attributed to the pushed SHA separately |
| Immutable revision saves/auth/restarts and a Studio consumer | issue:#1 remaining criteria | not-applicable: explicitly outside this session's contract boundary, required next |

Coverage: `complete-for-declared-scope` for the Rust contract/CLI, contracts, fixtures, Python
oracle/mutation runner, manifests/lockfile, CI and edited docs/instructions. Worker, Studio/Theatre
Web, CMP/Android/iOS and storage/provider targets are not implemented/touched. Pipeline retry,
publication and UI passes are not applicable; rights reference shape does not add a publication
gate. Dependency API/pin checks and successful builds do not constitute a dependency security
audit. The schema differential corpus is finite, not exhaustive equivalence.

## Handoff and residual risk

Continue #1 in the next sequential session with Axum/PostgreSQL, creator identity and backend
permission checks; store complete exports and source/rights references in immutable accepted
revision records; pin revision ID + content digest; implement compare-and-swap saves and idempotent
operation IDs; verify migrations/restarts, stale updates and unauthorized reads/writes, then
demonstrate a minimal Leptos save/retrieve consumer. Do not mark #1 or 010 complete on contract
evidence alone. The queue preserves this order and does not duplicate GitHub execution status.

Some Script IR skill reference introductions still describe the pre-implementation bootstrap;
their illustrative lifecycle/adapter APIs remain proposals. A later focused skill maintenance
change should reconcile that repository-reality snapshot with the linked contract. No skill
policy was weakened to permit this implementation.

Not covered / residual risk: production adoption is not accepted; record existence, tenant/actor
authorization, rights expiry/scope and publication approval are not checked here. IDs are checked
within a document, not against a prior revision. Import conversion/coverage, normalization notes,
broader language/Unicode policy, cue-markup human review and provider capability checks remain.
`ContentDigest` excludes evidence metadata; persistence must prevent metadata mutation and must
not collapse revisions by that digest. Axum/PostgreSQL/Leptos builds, app saves/restarts, DB fault
injection, fuzz/model/property campaigns, TTS/audio QA and mobile tests: **NOT_RUN**, because those
targets/integrations were not delivered or configured in this session. Remote CI is checked only
after the push at its exact SHA; no pre-push CI pass is claimed.
