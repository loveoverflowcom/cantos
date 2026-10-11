# Assurance report

Extends the [diff-review base report](../../cantos-engineering/references/diff-review.md#5-base-report).
Its Scope, Assessment, Finding and Remaining blocks, the categories (rule, compatibility, logic,
security, evidence), severity P0–P3, the confidence labels and the
[foundation evidence vocabulary](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified)
stay unchanged. This reference adds a finding class, the assurance ledger, per-target coverage
and an optional test focus. Like the [foundation report](../../cantos-engineering/SKILL.md#9-completion-report),
it ends with a residual-risk line that is never empty. It is the review's view of assurance, not
a registry of proofs.

## Finding classes

A class says how a finding is established and whether anything requires it. It is orthogonal to
the category, which says what the finding is about.

| Class | Meaning | Blocks? |
|---|---|---|
| **confirmed defect** | wrong behavior introduced or worsened by the change, shown from source or a reproduction | per severity |
| **required evidence gap** | evidence that a doc, accepted ADR, issue acceptance criterion, repository policy or the requester's review profile requires is missing, stale, weakened or failing | yes, as far as that requirement goes |
| **unresolved risk** | a serious suspicion not yet supported, stated with the check that would settle it | no; never dropped either |
| **non-blocking assurance opportunity** | a recommendation — a stronger type, a property, a fault point — that no requirement demands | no; carries no severity |

Only the first two may justify `changes-requested`. A recommendation does not become a blocker
because the reviewer values it, and a requirement is not waived because it is expensive. A
missing test is an evidence gap, not a reproduced production bug. The report creates no required
check, exit code or approval and claims no merge protection it has not inspected.

## Assurance ledger

One compact row for **every** changed durable or high-impact claim; the disposition is never
blank.

```text
| Claim | Owner / target | Required by | Evidence held → level | Disposition / next action |
```

`Required by` takes one of these forms; only the first five can produce a required evidence gap:

| Value | Example |
|---|---|
| `doc:<path#section>` | `doc:docs/product/business-rules.md#review-and-publication` |
| `adr:<number>` (accepted only) | `adr:0001` |
| `issue:#<n> <criterion>` | `issue:#7 "recovered workers do not overwrite a newer successful result"` |
| `policy:<file or skill rule>` | `policy:AGENTS.md#correctness-rules`, `policy:cantos-publication P6` |
| `review-profile:<who asked>` | `review-profile:requester asked for fault-injection evidence` |
| `recommendation` | the reviewer's own suggestion |

Dispositions: `held` (adequate and fresh) · `gap-required` · `gap-recommended` ·
`not-applicable: <reason>` · `question-for-owner`.

Expand a row to the full record when the claim is high-impact, its evidence is disputed, or a
finding depends on a type barrier:

```text
Claim + authoritative source (and its status: doc rule, accepted ADR, issue criterion)
Owner + module/symbols + targets
Required by
Type barrier and bypass audit: paths checked, paths still open
Techniques considered + why each applies or not
Evidence level and result + exact command + output location (or "not rerun", with provenance)
Input fingerprint, only where it differs from the review scope's
Assumptions, bounds, fakes in the loop; behavior not covered
Next verification action + owner
```

Cell rules:

- Levels come from the foundation vocabulary with their scope: `example-tested against a fake
  synthesizer`, `integration-tested (local PostgreSQL)`, `fault-injected at after-provider-response`.
  Evidence that exists but did not run in this review is cited with its provenance and marked
  `not rerun`.
- Keep core, integration, rendered, device and provider evidence in separate rows or cells;
  never aggregate them into one pass.
- The review scope carries one fingerprint (BASE/HEAD or the snapshot). Record a per-row
  fingerprint only for evidence produced from other inputs, such as an earlier CI run.

## Coverage

State coverage by path, module and claim **and** by target: Rust server, worker, Leptos Web
(Studio, Theatre), CMP common, Android, iOS, `packages/contracts/` fixtures, docs, skills and templates, CI
and scripts. `complete-for-declared-scope` names that scope; it never means the repository. List
what was not read and why — truncated patches, binary entries, generated files without their
generator, unreadable issue or Project data.

## Test focus

Include only when the change has observable behavior or a QA handoff. It is a section of the
report; write it to a file only on request, outside the source tree unless told otherwise.

```text
| Scenario | Claim | Production seam | Expected observation | Test layer | Missing evidence |
```

| Scenario | Claim | Production seam | Expected observation | Test layer | Missing evidence |
|---|---|---|---|---|---|
| edit the text of `dialogue-02` only | one changed line regenerates only that line and its mixes | regeneration planner | plan = {speech `dialogue-02`, mix `scene-01`, master}; nothing else | example table | property over random single-line edits |
| worker dies after the provider responds | no duplicate accepted output | accept-result write | one accepted artifact; second write affects zero rows | fault-injected | real PostgreSQL run |
| listener presses play five times | repeated play costs nothing | play handler | no job, outbox or provider row created | integration | none |

Expected observations come from the owning doc or accepted requirement, never from current
behavior. The layer is whatever the owner uses — unit, property, DOM, semantics, integration,
device.

## Language

Write prose in the requester's language. Keep symbols, paths, SHAs, commands, evidence terms,
class names, categories, severity and confidence labels and assessment values exactly as defined.
A Vietnamese finding title reads, for example: *[P1] Bản phát hành mới vẫn dùng phê duyệt của
bản render cũ* — class: confirmed defect · category: logic.

## Skeleton

```text
Scope
  BASE / HEAD / merge-base SHAs, or patch + snapshot fingerprint; diff form (three-dot or two-dot)
  Paths, modules, targets reviewed
  Coverage: complete-for-declared-scope | partial — what was not read and why

Assessment: changes-requested | no-actionable-findings | insufficient-evidence

Assurance ledger
  | Claim | Owner / target | Required by | Evidence held → level | Disposition / next action |
  (full records for high-impact or disputed claims)

Findings — by class, then severity
  [P1] <consequence> — class: confirmed defect · category: logic
    Location · rule/invariant + source · trigger → expected / actual · relation to the change
    Impact · confidence + why · smallest fix + regression test · evidence actually held

Pre-existing issues (not attributed to this change)
Checks: <command> → passed | failed | blocked | not-run | unavailable | zero-selected, with reasons
Test focus (when applicable)
Residual risk
```

`no-actionable-findings` keeps its diff-review meaning: no sufficiently grounded finding in the
declared scope — not a safety certificate, not an approval. Coverage is an independent axis:
report a confirmed serious finding even when the rest of the change is under-covered, and never
turn a missing tool or a skipped check into a pass.
