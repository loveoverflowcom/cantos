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

## Compatibility and production impact

Describe migrations, Script IR/API compatibility, affected cache entries and audio revisions, provider costs, rights/approval changes, and rollout or recovery steps. Write `N/A` when irrelevant.

## Review checklist

- [ ] The PR targets `develop` and has one reviewable purpose.
- [ ] Acceptance criteria are met, or remaining work is explicitly linked.
- [ ] Documentation and reusable templates reflect the resulting behavior.
- [ ] Logs, examples, and fixtures contain no secrets or unapproved source material.
- [ ] Applicable checks and remaining limitations are reported accurately.
