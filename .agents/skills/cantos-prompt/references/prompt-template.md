# Output template for one coding task

> **Scope.** The skeleton of one copyable prompt. Fill it with sourced facts and the task's
> decisions; leave no placeholder the author could have resolved. `UNKNOWN` is only for what
> has no evidence yet. Keep pseudocode in a fenced text block, never presented as compilable
> Rust. Headings may be written in the user's language ([aliases](prompt-contract.md#heading-aliases));
> the contract line stays verbatim.

## Skeleton

```text
# CANTOS — <outcome>
Contract: CANTOS_PROMPT_INPUT_V1

## Outcome and deliverables
<the result the user needs; PR/commit/work-chat counts when fixed>

## Repository, base and sources
Repository: loveoverflowcom/cantos
BASE_REF: <resolved branch, or the resolution rule when unknown>
Snapshot: <SHA and sources read, or UNKNOWN>
<issue/PR head; code areas reviewed; source links>
At session start compare the latest HEAD. If the code already meets the outcome, report the
evidence and do only what is missing. Never overwrite newer changes or open a no-op PR.

## Scope and owners
<verified modules/paths, shared-file owner, non-goals, contract boundary>
Read AGENTS.md, .agents/skills/README.md and <owner SKILL.md files> in the current checkout.
<user decisions; assumptions; contract version; conflicts>

## Execution
TARGET_ENVIRONMENT = UNKNOWN unless the target machine supplied evidence.
Probe only what this task needs on the coder's machine; keep unrelated working-tree changes;
resolve and fetch the base once. Turn claims and invariants into core, shell and UI steps as
relevant. No fake code in live composition.

PSEUDOCODE — guidance for the agent, not an existing API or macro:
  evidence = inspect_current_repo_and_task_scope()
  environment = probe_only_needed_target_capabilities()
  reconcile_snapshot_and_user_decisions(evidence)
  implement_independent_in_scope_work()
  if dependency_contract_is_stable:
      implement_consumer_and_real_adapter_where_possible()
      run_available_deterministic_boundary_tests()
  else if reversible_boundary_can_be_defined:
      isolate_minimal_provisional_contract()
      preserve_explicit_unavailable_behavior()
  else:
      finish_unaffected_work()
      record_the_exact_product_or_security_question()
  if resources_and_target_infra_are_available:
      run_scoped_real_checks()
  else:
      record_missing_checks_and_resume_conditions()
  preserve_actual_failures_and_acceptance_gaps()
  deliver_requested_source_and_authorized_git_actions()

## Invariants and acceptance
<correct/incorrect behavior table; runtime failure behavior; mocks are not real acceptance>
<behavior SSOT: capability, lifecycle and class from the impact procedure, or the no-model-change reason; model writes only when classified and granted>
<partial-source acceptance if the user allowed it; what stays unwired>

## Verification
<claim → check/command → applicability → evidence required>
<focused DEV, CHECKPOINT and HANDOFF per the owners; no whole-repository run without cause>
<Kani/Quint when they fit; a missing tool never becomes PASS>

## Deferred work and final report
<ledger for expected gaps: ID, scope, reason, runtime behavior, resume condition>
Report changed files/symbols, commands actually run and their results, failures, unverified
parts, next integration conditions, and real commits/PRs only if granted and done.
AUTHORIZED_ACTIONS: <the exact grant; never widen to deploy, issues or comments>
```

The pseudocode is a menu: delete the branches that do not apply to the task.

## Adjust by purpose

| Purpose | Adjustment |
|---|---|
| pure domain | drop unrelated database branches; focus on the invariant and its oracle |
| auth or database | add the actor/session/permission boundary and the transaction, replay and failure semantics from the contract; never invent a product rule |
| small UI | drop mandatory core/Kani layers; keep the layout, interaction and accessibility evidence that applies |
| legacy removal | inventory references by function, new wiring, dependency/tooling/docs cleanup in scope; separate source deletion from live data or service cleanup |
| N work chats | each prompt carries its own header, contract version, shared ownership and dependencies, never "see the previous prompt"; when the user wants one PR, say how the work reaches it ([split](split.md)) |

When the exact command is unknown, write the step that discovers the real suite, harness or
capability first. Never turn a template placeholder into a command presented as existing.
