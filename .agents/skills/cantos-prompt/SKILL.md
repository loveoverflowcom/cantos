---
name: cantos-prompt
description: >-
  Task entrypoint that authors self-contained implementation prompts for the Cantos repository in
  the CANTOS_PROMPT_INPUT_V1 contract, for any coding-agent runtime (Codex, Claude Code, ChatGPT
  or another work chat). Modes: one implementation prompt; split work into N prompts with
  ownership and integration order; review code, issues or PRs into evidence-backed repair
  prompts; resume blocked or interrupted work; sync repository skills with the external Cantos
  Prompt plugin. Reads current code, issues and owners, pins base and SHA, scopes one PR, adds
  pseudocode blocker branches and scoped verification. Read-only: authoring never edits code,
  comments, pushes or opens PRs. Its contract also defines how other entrypoints receive such
  prompts. Use when asked for a Cantos prompt, cantos prompt, prompt split, review-to-prompt,
  continuation prompt or prompt-contract change.
---

# Cantos prompt

Task entrypoint that turns a request about this repository into a prompt another coding agent
can execute without this conversation. It owns the authoring workflow, the
[`CANTOS_PROMPT_INPUT_V1` contract](references/prompt-contract.md) and its
[output template](references/prompt-template.md). It owns no engineering, domain, UI or
delivery rule: a prompt points the coder to the current owners instead of copying them.

## Boundaries

- **Cantos only.** For another project, say briefly that this skill is Cantos-specific and do
  not carry Cantos rules into it. Story ingest/adaptation/audio production use the story
  entrypoints directly; this skill writes engineering prompts, not producer briefs.
- **Authoring is read-only.** Generating, splitting, reviewing or resuming a prompt permits
  reading the repository, issues and PRs only. An action described *inside* a prompt (commit,
  push, PR, issue, comment, deploy) is not an action taken or authorized now.
- **Execution is a separate grant.** When the user also asks to carry the prompt out, emit it,
  then execute it through the routed entrypoint under the
  [receiving procedure](references/prompt-contract.md#receiving-a-prompt), keeping the
  authority already granted without asking again and adding none.
- **One contract, two sides.** Other entrypoints accept these prompts through the same contract;
  never add a per-runtime or per-entrypoint variant of it.

## Runtimes and the external plugin

The prompt and this skill are runtime-neutral. Invoke it as `$cantos-prompt` in Codex,
`/cantos-prompt` in Claude Code, or by reading this file in any other agent. The authoring
session (for example ChatGPT with a GitHub connector) and the coder's session are different
machines with different tools: name capabilities ("read the file", "run the shell command",
"open the PR with `gh` or the session's GitHub connector"), never one runtime's tool identifiers,
and follow the foundation's
[runtime portability](../cantos-engineering/references/local-execution.md#runtime-portability).

The external **Cantos Prompt** plugin packages the same workflow as five skills. Inside a
checkout, this repository copy is the reconciled owner; the plugin remains a producer where no
checkout exists. Both emit the same contract version, and
[skills-sync](references/skills-sync.md) keeps them aligned.

| Request | Mode | Read | Plugin skill name |
|---|---|---|---|
| one implementation prompt, or rewrite an existing prompt | implement | this file and the template | `cantos-prompt` |
| N prompts, parallel work chats, BE/Web/CMP/infra ownership | split | [split.md](references/split.md) | `cantos-prompt-split` |
| read code/issue/PR, find what is missing, emit repair prompts | review | [review.md](references/review.md) | `cantos-prompt-review` |
| continue after a blocker, failure, missing environment or interruption | resume | [resume.md](references/resume.md) | `cantos-prompt-resume` |
| align repository skills or the plugin with this contract | skills-sync | [skills-sync.md](references/skills-sync.md) | `cantos-prompt-repo-skills` |

A combined request runs the needed modes and returns one coherent output, never duplicate
prompts. Respect the number of prompts, PRs and commits the user fixed.

## The authoring order

```text
1  normalize the request: outcome, constraints, counts, explicit base, granted actions   § 1
2  read just enough current evidence; pin base and SHA                                    § 2
3  turn the request into one reviewable execution scope                                  § 3
4  write the prompt from the template; add blocker branches                             § 4
5  check the prompt before returning it                                                  § 5
```

### 1. Normalize the request

Record the outcome and why it is needed; the number of PRs, work chats and commits when fixed;
the explicit base, if any; the actions the user granted the coder; decisions the user made now.
Current user decisions win over older repository guidance; name the rule the coder must update
instead of re-asking for a confirmation the user already gave. Keep unknowns as `UNKNOWN`.

### 2. Read just enough current evidence

1. Use [the source map](references/cantos-context.md) as starting hints, never as proof that a
   path, API or capability still exists.
2. Prefer the current repository: a checkout with a known revision, or the session's GitHub
   access. Read [`AGENTS.md`](../../../AGENTS.md), [the skill index](../README.md) and the owners
   of the changed area; then the named issue/PR, manifests, contracts, implementation and tests.
   Run independent reads in parallel.
3. Pin the commit SHA for every claim about code. When a PR is involved, read its **head**, not
   only its base.
4. Resolve the base as the coder will:
   [explicit user base → accepted work-item integration/staging base → `develop`](../../../CONTRIBUTING.md#branch-and-pull-request-workflow).
   Never default every task to a refactor branch or to `develop` by habit.
5. Read live issue/PR state before stating status. Merged is not accepted; a remembered
   conversation is not evidence that PostgreSQL, Kanidm, an auth path or a mock was removed.
6. If GitHub or the checkout is unreachable, still write a useful prompt from what is real:
   record the source gap, mark unverified SHAs and paths `UNKNOWN` and make discovery the
   coder's first step. Do not keep asking the user for details the coder can find.

Start at the files nearest the outcome and widen only for a concrete dependency or conflict.
Do not scan the monorepo or every PR to write a small repair prompt.

### 3. Turn the request into an execution scope

- One reviewable outcome with observable acceptance and concrete non-goals, following the
  [review boundary statement](../cantos-work-item/references/planning-and-slicing.md#the-review-boundary-statement).
- Keep implementation, integration, verification and production acceptance separate. When the
  user accepts partially wired source, say so and name what stays broken; that acceptance never
  becomes permission to touch live data or services.
- Point to the foundation's functional core, layered verification and validated types; do not
  demand a newtype, typestate or crate for every value.
- Never reselect PostgreSQL, Kanidm or a fake runtime because a machine lacks infrastructure.
  Read the current architecture owners and the user's migration decisions.
- Draft the claim → failure mode → oracle → check table with
  [verification](references/verification.md), and the branches with
  [blockers](references/blockers.md) when the task has logic, I/O, Kani, Quint or environment
  conditions.

### 4. Write the prompt

Use [the template](references/prompt-template.md) and the field rules in
[the contract](references/prompt-contract.md#fields). Reply in the user's language (Vietnamese
when they write Vietnamese); keep identifiers, paths, commands and the contract line verbatim;
instructions about code and docs follow the repository language (English) unless asked.

`TARGET_ENVIRONMENT` is `UNKNOWN` until the coder probes its own machine: disk, SDK, `PATH` or
toolchain seen by the author never describes the coder's Mac, Linux host, VPS or work chat.
When the environment is missing, the prompt keeps the coder moving on independent work; real
adapters and harnesses may be written before they can run. Doubles live only at a named test or
preview boundary; a runtime missing its dependency returns a typed unavailable/pending result,
never fake authentication, permission, receipt, sync acknowledgement or publication.
When an unresolved product or security decision changes correctness, the prompt finishes the
independent work first and then asks one focused question.

Keep a single prompt near 700–1,400 words and a small task shorter; see
[length and pseudocode](references/prompt-contract.md#length-and-pseudocode). Add no lecture,
framework macro, test campaign or multi-PR plan the task does not need.

### 5. Check before returning

- Right repository and base; sources pinned; assumptions and `UNKNOWN` labeled.
- No invented path, command, flag, issue, test result or coder environment.
- Current owner skills are cited; no business policy is duplicated into the prompt.
- Every blocker has work to continue now, the deferred part, its runtime behavior and the
  condition to resume.
- No mock success on a live path; no `FAIL` or `NOT_RUN` turned into `PASS`.
- Kani and Quint are selected by scope and owner policy, never promised for the whole system.
- Scope and actions match the request; actions in the prompt are not presented as done.
- The output is a complete, copyable prompt whose essentials live inside it.

Present the key findings briefly before the prompt when they matter, with real source links.
Never return only an outline or ask the user to fill in what could be determined.

## Report

Return, in order: a short findings note (sources and SHAs read, gaps, conflicts); the prompt
or prompts as copyable blocks; for split work, the ownership table and integration order;
what stays unresolved. State plainly that nothing was edited, committed or posted, unless the
user separately asked for execution and it actually happened.

## References — load when the order reaches them

| Need | Reference |
|---|---|
| contract fields, heading aliases, authority, evidence words and the receiving procedure | [`prompt-contract.md`](references/prompt-contract.md) — always |
| source and owner map; facts to re-read instead of trusting a snapshot | [`cantos-context.md`](references/cantos-context.md) |
| the output skeleton and per-purpose adjustments | [`prompt-template.md`](references/prompt-template.md) |
| missing environment/dependency branches and the deferred ledger | [`blockers.md`](references/blockers.md) |
| evidence per scope, Kani/Quint selection and check cadence | [`verification.md`](references/verification.md) |
| split, review, resume and skills-sync modes | [`split.md`](references/split.md), [`review.md`](references/review.md), [`resume.md`](references/resume.md), [`skills-sync.md`](references/skills-sync.md) |
