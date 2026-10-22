# Split mode — several self-contained prompts

> **Scope.** Divide Cantos work into N prompts for separate work chats or PRs: parallel
> BE/Web/CMP/infra, several chats feeding one PR, or dependent PRs. Apply
> [the contract](prompt-contract.md), [the source map](cantos-context.md) and
> [the template](prompt-template.md) directly; read [blockers](blockers.md) and
> [verification](verification.md) when the scope needs them. Do not route back through the
> `SKILL.md` mode table.

## Procedure

1. Read the request, the current base, issues/PRs and the owner skills closest to each outcome.
   Pin the SHA and record sources not read. Never infer independence from directory names alone.
2. Keep work chats, branches, commits and PRs distinct: three chats can feed one PR. Never turn
   "one PR" into three or force one commit unless the user asked.
3. Build the scope table: outcome, owned paths, shared paths, contract producer and consumers,
   dependencies, acceptance, integration order. Exactly one owner edits a shared contract,
   schema or generated artifact; consumers never invent their own endpoint or DTO.
4. Slice by outcome and invariant. When the existing contract is already right, pin its SHA;
   never create a commit or PR just to re-pin an unchanged contract. Run in parallel only on a
   stable contract; otherwise one task settles the contract while consumers do independent work.
5. Give every prompt enough repository, base, SHA, decision, contract and owner context to open
   in a fresh chat. Link the same contract version; no prompt depends on another chat's memory.
6. Name the integration owner, the realistic merge or cherry-pick order, checkpoints to report,
   the conflict strategy and files no two tasks may edit. Two chats never share a working tree;
   each executing task uses its own worktree or branch.
7. **One PR:** choose one integration branch or worktree that receives the tasks' commits. One
   owner opens or updates the PR after integrating; other tasks hand over commits or patches.
   Never hard-code commit SHAs that do not exist yet; the author performs no cherry-pick.
8. **Several PRs:** give each its base and order; stacked PRs differ from parallel ones. After a
   merge, consumers rebase and revalidate the contract under the repository workflow; old
   results do not cover the new diff.
9. Give each prompt a short pseudocode block for its real environment or dependency branches,
   labeled as not runnable code, and drop branches that do not apply. Test doubles live only at
   the boundary; unrun real integration is deferred, never faked in production.
10. Each task runs its focused checks; the integration owner owns the overall handoff and reuses
    evidence with provenance. Do not repeat full preflight and repository scans in every chat.

## Output

- One paragraph fixing how the work is split, then a compact ownership/dependency table.
- Exactly the number of prompts requested, unless a real conflict makes that impossible; then
  explain the conflict.
- Each prompt copyable on its own, usually 500–1,000 words depending on scope.
- A concrete execution and integration order. When a product or security decision is missing,
  still hand out the independent parts and ask only the blocked question.

Author prompts only: no branch, PR, issue edit or code change unless execution was granted.
