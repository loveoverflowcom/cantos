# Quint behavior foundation evidence — 14 October 2026

Related: [issue 106](https://github.com/loveoverflowcom/cantos/issues/106),
[ADR 0027](../decisions/0027-quint-executable-behavior.md),
[capability inventory](../../specs/behavior/inventory.md) and
[separate implementation-conformance handoff](../work-plan/quint-conformance-handoff.md).
This is **PR A: executable model foundation**, not implementation or production acceptance.

PR 117 was rebased from original head `0b6c3db04ed47f371cb9158e0328a0a7f322b625`
onto develop `1c1d2107f1ca1aacac8c66b7b418c1070b35160c`. The records below remain
historical evidence for their named inputs; fresh gate results for the rebased source are
recorded separately in the PR ledger. Quint now uses ADR 0027, preserving Actor identity
ADR 0025 and Theatre Mobile storage ADR 0026.

## Scope and provenance

- Branch: `feat/issue106-quint-behavior-foundation`, initially verified against develop
  `83f06f9e449df854bc8632562e3c94bb4ccae43e` (including merged PR 112 and PR 114).
  The historical integration at `d76cdb51258b874ff3f68d987de64d7aa690402e` incorporated
  develop `e2267357de156e906c3e5eb6123a5b5d55388806` (PR 115 and PR 116), preserved Actor
  identity ADR 0025 and then renumbered Quint to ADR 0026. That reconciliation was
  documentation-only; the model/runner/catalog were unchanged.
- One draft capability: `cantos.media.private-audio-admission`, starting with one valid
  synthetic prepared submission. Approved private-intake requirements, actual pure seams,
  finite assumptions and deferred boundaries are separated in the inventory.
- Node 24.19.0, npm 11.9.0, Quint 0.33.0; official registry integrity lock and explicit local
  setup receipt. The TypeScript backend runs unchanged; no solver or production dependency.
- Configured seed 106; property campaign 500 samples × at most 30 steps; witness/control
  campaigns at most 1,000 samples × 30 steps each; 60-second child bounds.
- Expanded model bundle: `target/verification/behavior/cantos.media.private-audio-admission-506b0548e46048319ea8b202dd19cabb/`.
  That bundle records the earlier develop HEAD `3668cb5` plus dirty input hashes. Original bytes under `inputs/`,
  commands, installed-tool identities, raw ITF, concrete trace seeds, logs and counterexamples
  are retained locally in ignored output, not committed as bulk evidence. The PR description
  carries any later final-commit run identity.

| Input | SHA-256 |
| --- | --- |
| Catalog | `3d9f321c301e4539dce6fb8e3d346ae382188c2dc21ed521a828a7456c3097e4` |
| Model | `4306bcf7cbd78177c95be2c56422e98dcd00a23d0b9574daf67d56bf2f083d53` |
| Named scenarios | `d0710483277baa2febf01828d4ba0554e4ff1b0ddf67ee5432286bc61a630a52` |
| npm lock | `9ad19cea47ce82960b62aabf510c01a0381db7355fbce0bd0093df5a2f8a66d0` |
| Runner | `5afee79fff1d2f4952d510c2b4dee8da18c14b1105c5581408260f723380bb34` |

## Executed model evidence

`cargo xtask verify behavior`: **PASS**, with six separate PASS stages:

| Stage | Evidence | Scope |
| --- | --- | --- |
| Type/effect checks | Official Quint output for model and scenario modules | Registered Boolean property/witness and run declarations |
| Named examples | 19 exact selected runs and complete ITF traces | Admission; exact/lost-response replay; digest and length conflicts; rejected media and abandoned attempt retries; mid-inspection revoke/stale/expiry; owner-history/error precedence; lower clock bounds; recovered current authority and retained revocation |
| Sampled properties | Seven invariants, configured bounded campaign | Rejection preservation, immutable receipt, producer authority, payload ownership, inspected settlement, owner projection and settled shape |
| Non-vacuity | Eleven actual traces violating the negation of required witnesses | Reached inspection/admission/replay/rejection/history/restoration antecedents; no proof of exhaustive reachability |
| Negative controls | Seven mutants typechecked and violated their own registered invariant | One sensitive inserted fault per property; parser/runtime errors never count as killed mutations |
| Freshness | Original and final source/tool checksums agree | Selected files, recorded inputs and installed runtime/tree/receipt; not isolation against concurrent mutate-and-restore writes |

Evidence levels are type/effect-checked, `example-tested`, bounded sampled `property-tested`
and deliberate model `mutation-tested`. **Not** solver `bounded-model-checked`, exhaustive
proof, Rust refinement, actual bytes/storage acceptance or device verification.

## Review-driven regressions

- Retain exact original input bytes, so dirty models remain reconstructible after edits.
- Hash explicitly selected model/scenario files even outside default discovery; unit tests
  independently change each source and detect stale snapshots.
- Preserve failure-stage status and earlier results when the tool fails or nominal passing
  scenario output lacks ITF. These are runner unit doubles, not live integration evidence.
- Model recovered current authority as an external fact over the unchanged binding, including
  admission/replay after archive restoration and revocation that restoration cannot clear.
- Mark named assertion checkpoints `Assertion`/`NoOutcome`, so a later observer preserves
  them without replaying an admission effect twice.
- Separate original-model witness traces from deliberately faulty mutant traces in PR B:
  mutant outcomes cannot become positive expected behavior for a correct implementation.

## Native repository gate ledger

The historical PR description records exact commits and the complete changed-only preflight
summary on the pre-integration source tree. Its sixteen PASS gates are not upgraded to a
full-preflight PASS: the existing Script IR mutation baseline timed out, and the independent
PostgreSQL attempt failed before SQL suites because the executor rejected its Unix socket.
Repository, behavior and formatting reconciliation checks are distinct from broader native
results, which retain their original source identity. Focused runs:

- `python3 -m unittest discover -s scripts -p 'test_verify_behavior.py'`: PASS, 21 tests.
- `cargo test -p xtask --locked --offline`: initial attempt 139 passed/17 failed because the
  executor's temporary roots are within synthetic Git checkouts. Existing private-input
  guards were not changed. Using an isolated RAM-backed temporary directory outside Git,
  the same tests passed: 156 passed/0 failed.
- `cargo xtask verify repository`: PASS. The final preflight repository stage ran 375 Python
  tests and passed link/skill/JSON/architecture checks and whitespace checks.

Local model/native gates do not establish the product/runtime claims below. Remote CI is
reported independently in the PR; local delivery does not wait for it or substitute it for
missing local evidence.

## Deferred acceptance and residual risk

| Layer | Status and reason | Next step / owner |
| --- | --- | --- |
| Real Rust implementation conformance | Historical PR A: NOT_RUN; no observation adapter in that slice | The [PR B record](quint-code-conformance.md) holds the later independent pure-core evidence, now included in PR 117; fresh rebased results remain separate |
| Constructor, foreign-binding and numeric/hash completeness | Outside this finite already-valid submission model | Preserve existing typed native regressions; any wider model requires reviewed scope |
| Real authentication, SQL/concurrent commits, byte inspection, private provider and restart | No such implementation call in the model gate | Existing Media/intake/storage acceptance owners retain their own native/local/live tests; other preflight tests are separate evidence |
| Leptos/CMP UI, accessibility and native-device runtime | NOT_RUN for model scenarios: no UI or device implementation changed | Later shared stable-ID scenarios with real independent observations, preserving native/platform tests |
| Exhaustive/temporal/liveness proof | NOT_RUN: no solver selected | Separate reviewed model, proof obligations, backend and budgets if needed |

No API/Protobuf/database migration, production reducer, provider call, paid generation or
public audio access is added. Publication/listening are inventory entries without executable
models. A passing draft model does not accept the requirements, prove implementation
conformance, authorize a merge or finish issue 106.
