# Resume mode — continue blocked or interrupted work

> **Scope.** Write a continuation prompt from a log, handoff, earlier prompt or half-finished
> code when a toolchain, disk, database, Kani, emulator, upstream or integration is missing, so
> the next agent continues without demanding the whole stack. Keep what is done and its real
> evidence; never swap a runtime for a successful mock. Read [the contract](prompt-contract.md),
> [blockers](blockers.md), [verification](verification.md), [the source map](cantos-context.md)
> and [the template](prompt-template.md). Failure classification belongs to
> [`cantos-diagnostics`](../../cantos-diagnostics/SKILL.md); cite its report when one exists.

## Procedure

1. Read the original prompt, the handoff or log, the source revision and the current diff when
   reachable. Keep the original outcome and later direction changes; do not restart the task.
2. Tabulate: done-with-evidence; source-written-unverified; actual `FAIL`; blocked edge; unknown.
   Record the scope, revision and machine of each piece of evidence.
3. A log from one machine proves nothing about the next machine's tools. Make a target probe the
   first step. Low disk never justifies adding a fake repository to the product.
4. Classify the root cause: environment, resource, unavailable upstream, unresolved contract,
   genuine regression, stale base, or missing authority for a new action.
5. Find what can proceed now: core and invariants, real adapter source, consumers of a stable
   contract, unit and error tests, harnesses, docs or a patch. Drop work already complete with
   valid evidence.
6. For an unstable contract, isolate the smallest seam; never choose missing auth, permission or
   durability semantics. Finish the independent work, then ask the needed decision.
7. Keep typed unavailable/pending behavior at runtime and doubles in tests or previews. Never
   return `Ok(mock)` to reach acceptance, switch a production provider automatically or create
   hidden fake data.
8. Choose checks the resources allow; when nothing builds, record `NOT_RUN` or `UNSUPPORTED` as
   the runner reports. Never promise RAM-only unit or Kani runs or a default large tool download.
9. Write the concrete continuation prompt, the deferred ledger and a resume condition per edge.
   When the user already granted commit/push/PR, keep that grant; do not stop for CI or ask
   again only because I/O evidence is missing. Keep `FAIL` and incomplete acceptance visible.

## Output

About 700–1,200 words unless the user wants a larger prompt, with one pseudocode block of 6–12
lines for the continue/defer branches and no repeated disclaimers. Summarize what is known, the
real blockers and what happens next; give one copyable prompt focused on the missing delta and
the checks/dependencies to reconnect. Create no plugin, issue, PR or installation procedure when
the user only needs a continuation prompt.
