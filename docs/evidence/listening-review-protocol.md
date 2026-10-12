# Listening-review protocol handoff

Recorded 2026-10-12 for [issue #45](https://github.com/loveoverflowcom/cantos/issues/45).
Boundary: reusable manual listening procedure and a blank record template, plus their consumers.
No sample was rendered, played, perceived or scored in this work. No backend QC, automated
perceptual metric, provider/billing operation or publication was implemented or exercised.

## Source and prerequisites

- Branch: `docs/listening-review-protocol`, created after fetching latest `origin/develop` at
  `7478236c1a23b045dc93fa690b65a9edc9215fb5`. The starting tree was clean.
- Issue #45 and its comments were read through GitHub; it declares no blockers and has no comments.
  The private Project's execution status was not read or changed. GitHub owns execution status.
- The prior #44 branch `feat/vietnamese-reference-scene` remains at
  `582e24e01be70709718cb0d59e7d05760068a010`. It was read with `git show` only, not cherry-picked.
  Its original Vietnamese reference has two scenes, four roles, 30 dialogues and 12 cues;
  its recorded validator result is historical evidence, not re-run here. Its text is not on this
  branch and no link depends on those unmerged local files. #45 needs no rendered #44 dependency.
- Source rules: [business rules 20–21](../product/business-rules.md#review-and-publication),
  [quality plan](../work-plan/audio-drama-quality.md), [craft guide](../production/audio-drama-craft.md),
  [audio/QC method](../../.agents/skills/cantos-production-pipeline/references/audio-mix-and-qc.md)
  and the original [neural rubric](local-neural-tts-listening.md).
- Actual production/adapter sources were inspected: frozen planning inputs and neutral-only neural
  capabilities exist; `QcKind`/`QcResult` remain illustrative proposals in the method. No production
  database, private audio, model, historical ZIP or scorecard was changed.

## Claim and assurance ledger

Invariant: editorial evidence names the reviewer and the exact heard bytes/inputs/setup/scope;
missing checks and blocking findings cannot become accepted quality through averages or paperwork.
Owner: production/editorial documentation; runtime behavior and publication authority are outside
this change. Evidence level is `documented`, with repository hygiene `statically-checked` and
existing Python adapter/unit checks `example-tested`; none establishes `audio-listened`.

| Claim / failure mode | Owner / requirement | Oracle and evidence held | Disposition |
| --- | --- | --- | --- |
| One shared TTS rubric, no provider fork | #45 reuse criterion; [protocol](../production/listening-review-protocol.md#shared-dimensions-and-scores) | All six dimension meanings and anchors traced to the original handoff; its specific name inventory remains there; definitions moved to one canonical table | held — documented |
| Line/scene/episode identity and limits cannot imply unheard scope | #45 template criterion | [Template](../../templates/listening-review.md) fields checked for name/date/device/setup, full SHA-256, input/profile revision, excerpts, per-pass ranges and decision | held — documented |
| Two passes on headphones and phone speaker | #45 procedure/devices criteria; craft guide | Protocol/template traced together, including blind questions, transcript reveal order, familiarity, partial coverage and optional laptop | held — documented |
| Every finding pins bytes, time, expected/heard content and severity | #45 finding criterion; business rule 20 | Finding block and blocking/advisory mapping reviewed against proposed Editorial/QcBlockingFinding boundary | held — documented |
| Scores and averages cannot hide blockers or unmet accepted criteria | #45 severity/threshold criteria | Manual decision walkthrough below; overall decision explicitly lists blockers and unmet accepted-policy criteria | held — documented |
| NOT_RUN is not N/A; no invented acceptance or profile | #45 rules/threshold criteria; original handoff | Absent profile, unsupported required controls, missing comparison and threshold decisions traced through protocol/template | held — documented; thresholds remain question-for-owner |
| Evidence consumers resolve to the same protocol/template | #45 link criterion | Docs/template maps, production run, audio/QC method, production/audit skills and neural handoff checked; repository link/skill gate PASS | held — statically-checked links, documented semantics |
| Rights and exact-candidate approval remain separate | Business rules 20–21; publication gate method | No current approval recorded; unchanged-byte reuse versus affected new-candidate review checked; no backend or public-delivery change | held — documented |

## Separate read-only review

This Codex session performed a separate read-only pass using `cantos-code-review`, its diff-review
contract and assurance passes. This is not a second human listener or a threshold approval.
Scope was the full local tracked/untracked documentation, skills and templates against the base
above; the two main final subjects were pinned by byte SHA-256:

- Protocol: `b3d20e64bb9c1072c706f7c95250605ae731bb0b04e2e928c0f9eb0ab82726a0`.
- Template: `df5c2aaa07dfe640c59cb5eac3eae694660513de151b63636f3b8e82733624c5`.

Coverage: complete-for-declared-scope. Rules, compatibility, evidence, immutable input/approval
identity and rights/publication boundaries were checked against their pre-change owners.
Runtime type/construction, concurrency, DBSP/read performance, UI and native playback passes
are not applicable: no code, schema, query, DTO, fixture, audio or renderer changed.

The first pass found that an accepted threshold could be unmet without any blocking audible
finding, leaving the overall decision undefined. After ending review mode, the implementation
added explicit `CHANGES_REQUESTED` for that case, then the read-only pass rechecked it. Final
assessment: **no-actionable-findings** for this documentation scope. This is no runtime assurance
or approval. No review comments, issues, labels, PRs or remote CI actions were posted.

Manual walkthrough cases (document review, not executed audio or backend tests):

| Case | Required record / decision |
| --- | --- |
| Nothing heard | NOT_RUN, scores unassigned |
| Headphones only, or interrupted episode | INCOMPLETE; phone/unheard checks NOT_RUN; observed blockers retained |
| No name/bed content after actual listening | N/A only for that dimension/scope, with reason/range |
| Missing expected cue or required unsupported control | Gap/finding, never N/A or credited capability |
| Script-familiar reviewer | Non-blind observations retained; blind check NOT_RUN until a fresh reviewer |
| High average with a meaning-changing phone finding | Blocking finding retained; CHANGES_REQUESTED with complete coverage |
| No blocker, but accepted minimum unmet | CHANGES_REQUESTED; each unmet criterion named |
| Complete observations, threshold still proposed | OWNER_DECISION_REQUIRED; no fabricated approver |
| New render, input/profile change or excerpt-only hearing | New affected candidate review; original history retained; no unheard-parent acceptance |

## Local commands and results

Working directory: `/Users/manhblue/projects/loveoverflow/cantos`. Checks ran on the base above
with only this documentation/skill/template diff, using Rust/Cargo **1.87.0**. Final raw logs are
local ignored files in `target/listening-review-protocol-45/`; they are not shipped listening evidence.

| Exact command | Result / scope | Local output |
| --- | --- | --- |
| `RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp cargo xtask verify repository` | PASS repository: links, JSON/skill hygiene, 202 Python tests, unstaged/staged whitespace | `target/listening-review-protocol-45/repository.log` |
| `RUSTUP_TOOLCHAIN=1.87.0 TMPDIR=/private/tmp cargo xtask preflight --changed-only` | PASS repository and rust-format; no Cargo package affected, no skips | `target/listening-review-protocol-45/preflight.log` |

No app, contract, mutation, PostgreSQL, Web, CMP or integration gate was selected for this
docs-only diff; no result is claimed for those suites. Remote CI was not checked or polled.

## Remaining owner decisions and handoff

The rubric/line and drama acceptance proposals remain **PROPOSED**. The owner must record the
actual threshold decision with scope/evidence and approver/date on acceptance. Named Vietnamese
listening on both required devices, #6 rights/commercial acceptance, #44 production binding and
rendering, #50 auditions and #51 profile decisions remain separate. No relevant rights, approval,
spending, backend or publication gate has advanced. The queue recommends #46 asset intake, then
#47/#48 editorial preparation without changing the MVP #6/#7/#9 order.

No application core, type barrier or regression test was added: this changes only the manual
record/procedure and its references. Crash/retry, provider timeout, leases, cancellation, cost
reservation and publication races are not exercised here and remain the runtime owners' gates.
Only a local commit is authorized for this handoff. The #44 and #45 branches are prepared for a
subsequent publication decision; no push, PR, merge or issue closure occurs in this task.
