# Verification cadence

> **Scope.** Own when Cantos checks run and when existing evidence can be reused. All skills
> and delivery workflows reference this policy instead of adding per-edit or per-commit gates.
> [Verification strategy](verification-strategy.md) owns what evidence a claim needs;
> [local handoff](local-execution.md#local-handoff-and-pr-completion) owns delivery eligibility,
> unavailable tooling and deferred acceptance. This policy changes no xtask selector or CI gate.

## Milestones and risk

Choose checks from the changed claim and its consumers. Count neither edited lines nor commits
as a risk measure. Group related edits into a coherent checkpoint; a save, small correction,
commit, push or progress update does not by itself require another verification campaign.

| Milestone | Trigger | Run |
| --- | --- | --- |
| DEV | investigate a failure or change a behavior while the slice is in progress | the cheapest focused regression, syntax/static check or scoped formatter that answers the current question; do not automatically expand to module suites or preflight |
| CHECKPOINT | a coherent rule, module or component is ready for a dependent layer or review | applicable module tests, format/lint and contract/renderer checks; reuse covered focused results and broaden only for a changed boundary, regression or unresolved risk |
| HANDOFF | the completed review boundary is ready for user acceptance or opening/updating its PR | `cargo xtask preflight --changed-only --base "$BASE_REF"` on the final diff, plus required evidence outside its plan; one run covers the same commit/push/PR handoff |
| CRITICAL | an edit reaches a security, durability, publication, contract or synchronization invariant | add the owner's targeted failure/boundary evidence before accepting that claim or dependent work; combine it with the relevant checkpoint/handoff rather than creating a full-preflight loop |

Keep the [L1–L5 order](verification-strategy.md#layered-verification) within the affected scope.
Ordinary behavior changes still need their cheapest deterministic regression; retain the
[foundation exemptions](../SKILL.md#5-every-behavior-change-gets-the-cheapest-deterministic-regression-evidence)
for wording, formatting and other non-behavior work. Read-only reviews run only checks needed
to settle their claims; do not build an application just to obtain a baseline.

## Plan the handoff once

1. Resolve the base through [CONTRIBUTING](../../../../CONTRIBUTING.md#branch-and-pull-request-workflow).
   Inventory the complete review diff and applicable owner obligations. Discover implemented
   commands in the [xtask contract](../../../../xtask/README.md#preflight); use
   `cargo xtask preflight --changed-only --base "$BASE_REF" --dry-run` when scope is unclear.
2. Use preflight for the gates it already covers, including `repository` and `rust-format`.
   Do not routinely run `verify repository`, whole suites, format/lint and then the same gates
   again in preflight. Earlier focused/checkpoint checks remain useful feedback; they are not
   prerequisites to repeat immediately before preflight. Run additional owner checks only
   where they establish a required claim absent from that plan.
3. Run the final handoff plan, retain its exact summary and supplement uncovered evidence.
   Missing tools and partial delivery use [local handoff](local-execution.md#progressive-commit-push-and-pr);
   documents/skills alone may use its narrow [Cargo-unavailable fallback](local-execution.md#docs-only-fallback-when-xtask-cannot-start).
   Never hide a failure, call a skipped gate passed or change CI/required checks for a green exit.

`--changed-only` includes the whole branch since its merge base, plus staged, unstaged and
untracked inputs. It is not an incremental check since the last edit or verification run.
Shared verification/configuration inputs and uncertain impact can select all gates. Schedule
that cost at HANDOFF; do not change the base to `HEAD`, omit branch changes or skip gates merely
to obtain a cheaper passing plan. Documentation-only selection still includes Rust formatting
when xtask is available; do not claim the runner already optimizes it away.

## Reuse and invalidation

Before rerunning a successful check, identify the changed input, newly uncovered claim or
unresolved failure it would address. Record the existing exact command, scope, source revision
and dirty-content identity, fixtures, dependency/lockfile versions, relevant configuration and
toolchain/environment, result and output location in the working evidence ledger. Use Git's
content identity or explicit digests where a revision alone does not identify the checked tree.

- Reuse a passing result only while all its applicable inputs and claim scope remain covered.
  A commit, push or PR creation with identical content does not invalidate it. A checkpoint
  preflight already covering the final diff can serve the handoff; cite it instead of rerunning.
- A source, test, fixture, generated input, dependency, build/verification configuration or
  toolchain change invalidates the affected evidence. Changes to a shared dependency, merge or
  rebase require impact review of the combined tree; broaden when coverage is uncertain.
  Documentation-only edits invalidate repository hygiene evidence, not unrelated runtime claims.
- After editing content following preflight, do not describe its old summary as a pass for the
  final tree. During DEV rerun only affected focused checks; refresh preflight once at the next
  HANDOFF. Keep still-valid earlier evidence labeled with its checked scope and inputs.
- After a failure, fix or diagnose the cause and rerun the affected check first. Repeat broader
  checks only for changed inputs, uncovered risk or a new handoff. Preserve earlier failures
  and use [diagnostics](../../cantos-diagnostics/SKILL.md) rather than rerunning until green.

This is evidence reuse by inspection, not an implemented cache. xtask still executes every gate
in its selected plan. Do not invent cache hits, a changed-since-last-run flag or a cached
aggregate PASS. Measuring gate costs, finer diff selection, safe caching and independent
execution when a tool is missing are separate tooling work.

## Critical and platform obligations

| Changed risk | Required scope from the owner |
| --- | --- |
| authentication, capabilities, ownership or revocation | allow/deny boundaries, cross-owner/resource scope, expiry/revocation and replay cases |
| SQL, transactions, migrations or concurrent writes | real authoritative-engine semantics (current PostgreSQL or accepted Turso), rollback/retry/race checks and compatibility; mocks do not accept these claims |
| private intake, rights, approval or publication | byte/binding integrity, duplicate/lost-response and stale-revision rejection, crash/replacement behavior at the changed step |
| API/Script IR/Protobuf or another shared contract | affected producer/consumer fixtures, compatibility and exact error mapping; compilation alone is insufficient |
| progress, bookmarks, outbox or offline synchronization | ordering, conflict/replay, cancellation, restoration and stale-response evidence for the changed flow |
| shared tooling, dependencies or toolchain | affected consumers and conservative handoff selection; no exemption based on a small diff |

Keep UI reference/interaction/accessibility and motion evidence for the affected scenarios at
component checkpoints and final handoff. Refresh observations affected by later edits, not the
whole matrix after every padding adjustment. Shared component qualification still precedes
first real app adoption under the [preview owner](../../cantos-ui-design/references/preview-catalog.md).
CMP desktop/preview gates are not fully provisioned in GitHub CI: retain applicable local
`cargo xtask ui-preview cmp verify` / `cargo xtask theatre-preview verify` evidence and required
renderer observations. Reuse them only with matching inputs; a Rust/Web CI pass, host render or
screenshot never supplies Android/iOS runtime, native playback or screen-reader acceptance.

These are scheduling rules, enforced by agent/reviewer judgment. Repository checks validate
skill structure and links, not whether an agent followed the milestones. Missing required
evidence stays explicit under local handoff; production and phase-exit gates are unchanged.
