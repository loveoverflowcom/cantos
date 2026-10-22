# Skills-sync mode — keep the repository and the plugin on one contract

> **Scope.** Review whether the repository skills and the external Cantos Prompt plugin still
> agree on `CANTOS_PROMPT_INPUT_V1`, then emit a prompt that fixes the drift. By default this
> mode only writes the prompt; it edits the repository or the plugin only when the user asks.
> Read [the contract](prompt-contract.md), [the source map](cantos-context.md),
> [blockers](blockers.md), [verification](verification.md) and [the template](prompt-template.md).

## Ownership

Repository skills own implementation, delivery and receiving; the plugin and this skill produce
input. Never copy the whole plugin into the repository, add a parallel owner skill, or duplicate
`.claude/skills` entries; the repository keeps one canonical skill per
[the index rules](../../README.md#runtime-discovery). Plugin packaging (manifests, marketplace
metadata, connector IDs) stays outside the repository.

## Read before writing the prompt

Read the current [`AGENTS.md`](../../../../AGENTS.md), [`CONTRIBUTING.md`](../../../../CONTRIBUTING.md),
[the skill index](../../README.md), [`cantos-work-item`](../../cantos-work-item/SKILL.md) with its
[planning](../../cantos-work-item/references/planning-and-slicing.md) and
[PR](../../cantos-work-item/references/pull-request.md) references, and the foundation's
[local execution](../../cantos-engineering/references/local-execution.md),
[verification strategy](../../cantos-engineering/references/verification-strategy.md) and
[cadence](../../cantos-engineering/references/verification-cadence.md). Read the other receiving
entrypoints only when a concrete conflict appears. When the user supplies the plugin, compare
its contract, template headings and mode list with this skill's copies.

Verify every path at the pinned ref. Do not mechanically replace every "stop" in the skills:
keep stops that protect phase exit, merge protection, production or durable I/O acceptance.

## What the prompt must require

1. One receiving procedure, linked from the entrypoints and the index; plain-language requests
   stay valid without the template, and unknowns are never invented.
2. User decisions, current code and owner docs reconciled; the coder's environment kept apart
   from the author's.
3. Generic halts replaced by blocker classes (environment/resource, stable upstream contract,
   provisional reversible boundary, unresolved product/security, actual `FAIL`): finish
   independent work, report the gap, then ask.
4. Phase exit, merge protection, production and durable-I/O acceptance retained; a user's
   acceptance of partial source is not proof or production safety.
5. No silent live fallback; test/preview isolation; fail-closed or pending behavior. No default
   PostgreSQL/Podman install or Kanidm recovery to run one part of the work.
6. Invariants, validated types, Quint and Kani reused from their owners; no Kani for Kotlin,
   docs or cosmetic UI; a written harness is not a proof; product semantics unchanged.
7. Focused DEV, affected CHECKPOINT, changed-only HANDOFF; evidence reused; no CI wait to return
   a requested PR; no base reset to shrink scope; no skipped `FAIL`.
8. Handoff lines for source, compile, test, proof and I/O apart; a deferred ledger; only the Git
   actions granted. Inherited permission replaces step-by-step re-asking without adding any.
9. Stale architecture or auth text fixed only when proven and needed by the workflow, by
   pointing to owners rather than copying them. Source legacy removal and live-system work are
   separate scopes.
10. Symlinks, references, anchors and the index consistent; broken workflow links in the read
    scope fixed, old anchors never cited as existing. Repository hygiene per current tooling;
    wording-only changes need no new test or heavy application build.
11. A contract change bumps the version and lists what the plugin must change in the same
    prompt, so both sides move together.

## Acceptance to keep

- Template prompts and natural-language requests both work; unknowns stay unknown.
- Missing infrastructure, Kani or disk does not block independent work or produce a runtime mock.
- A genuine `FAIL` stays failed; an unrun check never becomes `PASS`.
- One PR from several chats keeps ownership and integration explicit when the input says so.
- Auth, newtypes, Kani and Quint follow the owner and risk; no feature expansion.
- No runtime, gate, CI or architecture change outside the docs/skills scope.
- The handoff lists changed files, rules before and after, commands actually run and missing
  acceptance.

Output: a short sourced review and one self-contained prompt for a docs/skills PR on the
requested base. Never say the repository changed when only a prompt was delivered.
