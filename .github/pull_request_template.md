## Outcome

Describe the problem and resulting behavior. Link the issue with `Closes #...` only when this PR completes it.

## Scope

- Changed:
- Deferred or excluded:
- Related decisions or dependencies:

## Verification

List the checks actually run and their results. Include screenshots for UI changes and relevant failure/recovery evidence for pipeline changes.

```text
cargo xtask preflight --changed-only
```

Paste the preflight summary, plus any `cargo xtask verify` suite you ran separately. Report gates as
xtask printed them (`PASS`, `FAIL`, `NOT_RUN`, `UNSUPPORTED`) and explain every skipped or
unsupported gate. Add device or runtime checks once those implementations exist.

Follow the foundation's [local handoff policy](../.agents/skills/cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion)
and [independent gate ledger](../.agents/skills/cantos-engineering/references/verification-strategy.md#independent-verification-gates).
List affected layers/claims separately; state inapplicable scope with a reason. Keep status
(`PASS`, `FAIL`, `NOT_RUN`, `UNSUPPORTED`) separate from the existing evidence level. Name doubles
and distinguish mocked results from real infrastructure; include exact commands or review artifacts.

| Layer / claim | Status | Evidence level / command or artifact | Verified scope / gaps |
| --- | --- | --- | --- |
| L1 Architecture / abstraction | | | |
| L2 Domain / types / errors | | | |
| L3 Unit / property / static checks | | | |
| L4 Application / mocked orchestration | | | |
| L5 DB / API / E2E / runtime (split as needed) | | | |

Delivery progress, if applicable: `CORE_VERIFIED / IO_ACCEPTANCE_PENDING` for the named passed
core scope only; this is not an evidence level or a production-ready claim. GitHub CI may be
pending or `not checked`; no waiting is required. If Cargo/Rust is unavailable for docs-only
work, list fallback primitive results and unavailable xtask/preflight commands accurately.

## Deferred acceptance

List unrun DB/API/I/O/runtime checks, or `None` with a reason. Distinguish execution blockers
from actual failures. Use this checklist instead of creating an unrequested follow-up issue.

| Check / layer and status | Reason: environment, dependency or outside PR scope | Prerequisites and exact next command | Acceptance criteria | Next owner / residual risk |
| --- | --- | --- | --- | --- |
| | | | | |

Production and phase-exit acceptance remain separate. Describe how unfinished behavior stays
unreachable or behind existing fail-closed gates in the production-impact section.

## Compatibility and production impact

Describe migrations, Script IR/API compatibility, affected cache entries and audio revisions, provider costs, rights/approval changes, and rollout or recovery steps. Write `N/A` when irrelevant.

## Review checklist

- [ ] The PR targets `develop` and has one reviewable purpose.
- [ ] Acceptance criteria are met, or remaining work is explicitly linked.
- [ ] Documentation and reusable templates reflect the resulting behavior.
- [ ] Logs, examples, and fixtures contain no secrets or unapproved source material.
- [ ] Applicable checks and remaining limitations are reported accurately.
