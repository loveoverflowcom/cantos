# Review mode — evidence-backed repair prompts

> **Scope.** Read code, issues, PRs or an earlier prompt, find what is really missing and turn
> it into the next prompt. Answers "how far did this get", "which prompt was executed", "what
> mock, legacy, wiring or test is still open". The read-only contract, scope pinning, lenses and
> severity come from [`cantos-code-review`](../../cantos-code-review/SKILL.md) and its
> [diff-review owner](../../cantos-engineering/references/diff-review.md); this file adds only
> the conversion into prompts. Read [the contract](prompt-contract.md),
> [the source map](cantos-context.md), [verification](verification.md) and
> [the template](prompt-template.md); read [blockers](blockers.md) when blockers exist.

## Review

1. Resolve repository, base, SHA and the actual PR head. Read the original request, issue
   acceptance, diff, implementation, composition, tests and the owning docs. Name what was not read.
2. Separate states: proposed; source implemented; wired into the default runtime; compile/test
   observed; integration accepted; deployed. Never infer them from file names, PR bodies,
   checkboxes or a merged status.
3. For mock or legacy questions, trace usages and composition: a production fallback, a
   test-only double, an explicit preview, migration history and stale docs are different. Do not
   list every occurrence of `postgres`, `kanidm` or `mock` for deletion.
4. Trace the real path: entrypoint → application → adapter → persistence/auth contract. Focus on
   reachable defects, invariants, dependencies, missing wiring or missing evidence.
5. Check consumer/server contracts and classify stateful impact with the behavior
   [impact procedure](../../cantos-behavior/SKILL.md#ssot-impact-procedure): a code-only stateful
   delta missing the model or binding sync its class needs is a finding. Unrun Kani or Quint is
   an evidence gap, not a runtime bug; an existing test is not proof it ran.
6. Write each finding with a reasoned severity, path/symbol/line when observed, source SHA or
   link, current and expected behavior from a sourced rule, impact and fix. Keep confirmed
   findings apart from hypotheses and unknowns; never invent line numbers or status.
7. Compare existing issues and PRs to avoid re-assigning finished work. Do not close, label,
   comment, open issues or edit PRs: the request is a review.

## Findings into prompts

- Prioritize outcome, correctness and integration. Group defects with one cause into one
  outcome; do not create many tiny PRs because many files changed.
- Keep the prompt/PR count the user fixed. Otherwise default to one bounded repair prompt, or
  propose a few slices only when they are truly independent.
- Use [split mode](split.md) for multi-chat requests; each prompt still carries its own snapshot,
  ownership, contract, regression oracle and integration boundary.
- Route each repair to its receiving entrypoint: one logic defect to `cantos-logic-fix`, one UI
  defect to `cantos-ui-fix`, wider work to `cantos-work-item`; the receiver re-verifies every
  finding at its HEAD.
- With no confirmed defect, state the review scope and residual uncertainty. Emit a
  qualification or discovery prompt only when that gap is useful; never invent a bug.
- When checking PostgreSQL/Kanidm removal in source, keep the source removal the user allowed
  apart from VPS operation and live data. Do not block a source refactor on unaccepted
  deployment when the user accepted broken seams; still report those seams plainly.

## Presentation

Lead with the status conclusion and prioritized findings, each fact sourced. Then the complete
prompts, then what could not be concluded. Never claim a whole-repository review after reading
one area, or a passing test/build because a fixture or harness appears in the diff.
