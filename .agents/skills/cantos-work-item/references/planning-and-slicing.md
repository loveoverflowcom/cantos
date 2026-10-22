# Planning and slicing

> **Scope.** Turn one work-plan item or issue into a PR-sized, reviewable outcome: read the spec
> as a contract, check prerequisites, slice vertically, write the review boundary, label
> prototypes, seed the ledger, decide when a decision record is needed and update the queue
> afterwards. The queue, planning guide and specs own order, scope and rationale; this file owns
> only the method of applying them.

## Read the spec as a contract for this PR

Every item spec under `docs/work-plan/` has the same sections. Extract each into a specific place
instead of re-reading the whole spec while coding:

| Spec section | Extract | Goes to |
|---|---|---|
| Why | the creator or listener outcome | PR Outcome |
| Scope / Non-goals | in and out of scope | boundary "does not deliver" line |
| Dependencies | prerequisites and every "inspect the actual … before choosing" instruction | prerequisite check; discovery tasks before design |
| Suggested sequence | candidate slices — a suggestion, not a mandate | slicing |
| Acceptance criteria | the criteria this PR covers | ledger rows and boundary |
| Review boundary | the phase-exit demonstration owned by the whole item | verification: which part this PR shows |
| Risks / unknowns | failure cases worth a test; spikes; open decisions | ledger rows, decision triggers |
| Follow-ups | excluded work; correctness follow-ups that block the next item | handoff Follow-ups |

The issue's acceptance checklist is the execution contract; the spec owns scope and rationale.
When the two disagree, report the difference with both sources quoted and ask which wins.

## Check prerequisites before starting

1. List the issue's immediate blockers from its native `blocked by` links and from the
   [dependency chain](../../../../docs/work-plan/project-planning.md#dependency-chain) and
   [current priorities](../../../../docs/work-plan/current-priorities.md). A disagreement between
   them is reported, not resolved by preference.
2. For each blocker find recorded acceptance: Project status `Done`, or a complete acceptance
   checklist with linked evidence and, where it applies, the
   [phase-exit evidence](../../../../docs/work-plan/project-planning.md#phase-exit-and-review-evidence).
   "Merged" and "closed" are not acceptance.
3. Read the blocker's merged code for the contracts you will consume — real types, routes,
   migrations and fixtures. That code, not the earlier spec, is what this slice builds on.
4. Parallel work, including tasks split across several prompts or work chats, never waives
   acceptance of the immediate blocker. Each split task edits only its assigned paths and hands
   its commits to the named integration owner ([split ownership](../../cantos-prompt/references/split.md)).

## Slicing rules

| Rule | Failure it prevents | Good | Counterexample | Oracle · status | Exception |
|---|---|---|---|---|---|
| **Vertical slice with an observable outcome** | layers nobody can demonstrate; reviewers cannot check behavior | "a creator saves a script revision and reopens it after a backend restart": migration, validator, endpoint and test together | "add every MVP table", "add DTOs for all endpoints" | the boundary names a demonstration or test a reviewer can rerun · manual | an independently verified core with a reproducible test and explicit I/O acceptance under [local handoff](../../cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion), or a requested toolchain-only bootstrap claiming no behavior |
| **Contract first, with its immediate consumer** | a speculative contract the first consumer contradicts; dead fields | progress DTO, schema and fixture land with the endpoint and a reader (client code or a contract test the client issue will reuse) | OpenAPI for every future listener endpoint, no caller | every new field has a reader in the same PR · manual; contract tests proposed once schemas exist | a field reserved by an accepted ADR |
| **Migrations carry a forward/backward plan** | new code cannot read old rows; rollback strands data; revisions lost | expand → backfill → switch readers → contract in a later PR, with an old-row fixture test | renaming a column and its readers in one step; editing an applied migration | migration test against real PostgreSQL loading pre-change rows ([persistence](../../cantos-engineering/references/persistence.md)) · manual | no shared or deployed data exists yet — state that explicitly; still never edit an applied migration |
| **No gate-less path to a gated action** | generation or publication reachable before its checks exist | the generate endpoint lands with its budget reservation, or is not routed | "budget enforcement follows next PR" while generation is callable | review of reachable routes and UI actions · manual | test-only code paths |
| **Correctness before expansion** | optional scope while data loss, permission bypass or unbounded billing is open | fixing duplicate result association before mixing (020 follow-ups) | adding provider auto-routing with an ambiguous-charge path open | [backlog rule](../../../../docs/work-plan/backlog/deferred-expansion.md) · manual | none |
| **No speculative structure** | crates, traits or services with no consumer | one server package until a second build target exists | empty `tts`, `mixer` and `publisher` crates "for later" | [decision 0001](../../../../docs/decisions/0001-modular-monolith.md), [decoupling](../../cantos-engineering/references/decoupling.md) · manual | an accepted ADR naming the consumer |

A slice may still be small: one acceptance criterion with its failure cases is a good PR.
"Small" never means omitting the regression evidence or the docs update for that criterion.
Use the foundation's [progressive delivery policy](../../cantos-engineering/references/local-execution.md#progressive-commit-push-and-pr)
when an infrastructure gap prevents a full vertical demonstration; name the accepted core
claims and deferred I/O claims separately in the review boundary. This does not waive product
dependency acceptance or turn a passed core into a completed feature.

## The review boundary statement

Write it before coding; it becomes the PR Outcome and the handoff's first lines.

```text
Item / issue:        010 / #1 — Establish authenticated script persistence and contracts
This PR delivers:    a creator saves a Script IR draft as revision 1 and reopens it,
                     byte-identical, after a backend restart
It does not deliver: editing into revision 2, conflict handling, import formats, the editor UI
Acceptance covered:  "reopen the saved result after a backend restart" (issue checklist item 2)
Evidence planned:    restart round-trip → integration-tested (real PostgreSQL);
                     validator errors → example-tested with exact variants
Closes the issue:    no — "Part of #1"
```

The issue title and checklist position above are illustrative; quote the real issue. `Closes #n`
belongs only to the PR whose merge completes every acceptance criterion of that issue. The
phase-exit demonstration (for 010: import → adaptation review → edit → save → reopen) is evidence
for the item, usually assembled after its last PR; no single slice claims it.

## Prototypes before a gate

This section concerns an **unaccepted product dependency**, not a missing local execution
environment for independent work; the latter follows the foundation's local handoff policy.

Contract-fixture experiments before a gate are labeled as prototypes. A prototype is allowed
only when the user chooses it over waiting (in conversation or in a submitted prompt's
`DECISIONS`), and then:

| Must | Because |
|---|---|
| say `Prototype against contract fixtures — not acceptance evidence` first in the PR Outcome and handoff | reviewers and the Project must not read it as progress toward `Done` |
| open it as a draft PR unless the user decides otherwise; never `Closes` | the gate it anticipates has not passed |
| list every provisional fixture and the contract version it assumes | the real contract may differ |
| keep evidence levels honest: `example-tested` against fixtures, nothing `integration-tested` against the unaccepted dependency | the dependency's behavior is not yet a fact |
| record the re-verification to run once the prerequisite is accepted | a prototype silently promoted to accepted work is the failure this rule exists for |

Example: Theatre Web player work for #13 while #12's listener API is unaccepted runs against
fixture responses for a progress conflict on `Một lời hẹn`; it proves the UI renders the
conflict choice, not that the server returns it.

## A worked ledger

For the boundary above (illustrative; real names come from the code you write):

| Claim / invariant | Source | Failure mode | Cheapest oracle | Target → held | Live / mocked |
|---|---|---|---|---|---|
| a saved revision reopens with the same canonical digest after restart | 010 acceptance | text re-serialized or lost; digest drifts and later cache keys change | integration test: save, drop the pool, start a new process, reload, compare digest | integration-tested → pending | live PostgreSQL |
| a dialogue with an unknown `speaker_id` cannot be accepted | [Script IR validation](../../../../docs/architecture/script-ir.md#revision-and-validation-policy) | a broken script reaches production | table test asserting `UnresolvedSpeaker { dialogue_id }` exactly | example-tested + type-enforced constructor → pending | n/a |
| Vietnamese text survives save unchanged, e.g. `Ngày mai, mình có diễn tiếp không?` | [Vietnamese text](../../cantos-script-ir/references/vietnamese-text.md) | silent normalization changes text and fingerprints | golden fixture plus a property over generated diacritic strings | property-tested → pending | n/a |
| revision 1 is never updated in place | [business rule 3](../../../../docs/product/business-rules.md#content-and-revisions) | history rewritten | no update path in code; database constraint or privilege test once persistence exists | type-enforced + integration-tested → pending | live PostgreSQL |
| an unauthenticated request cannot read the revision | [business rule 13](../../../../docs/product/business-rules.md#rights-and-access) | private draft exposure | handler test asserting the exact rejection | example-tested → pending | mocked identity |

The `held` column is filled from tool output only. A row that ends `pending` or `not run` moves to
the residual-risk line, not out of the report.

## When a decision record is needed

| Situation | Record | Template | Who accepts |
|---|---|---|---|
| a choice the docs list as open: providers, Script IR schema, identity/roles, audio profile, CDN, offline policy | ADR | [`adr.md`](../../../../templates/adr.md) | the user or a named owner |
| a library, toolchain or test-database strategy the slice introduces | ADR | `adr.md` | the user or a named owner |
| an item [the mobile doc requires before implementation](../../../../docs/architecture/mobile.md#implementation-decisions-to-record) | ADR | `adr.md` | the user or a named owner |
| a multi-PR outcome whose flow, states or contracts need agreement first | feature brief | [`feature-brief.md`](../../../../templates/feature-brief.md) | the user |
| a production run used as evidence | production record | [`production-run.md`](../../../../templates/production-run.md), per [`cantos-production-pipeline`](../../cantos-production-pipeline/SKILL.md) | its reviewers |
| making an episode revision public | publication record | [`publication-checklist.md`](../../../../templates/publication-checklist.md), per [`cantos-publication`](../../cantos-publication/SKILL.md) | the required approvers |

- Place an ADR at `docs/decisions/NNNN-short-decision.md` beside
  [0001](../../../../docs/decisions/0001-modular-monolith.md); place a feature brief where the issue
  says, or propose a path and ask. Link either from the issue (a GitHub write: confirm) and the PR.
- Draft with status `proposed`, real alternatives and the evidence for each. Record the version
  and the date you checked official documentation for any pinned dependency.
- Implementing behind a proposed decision is acceptable when the user agrees; the PR then says the
  decision is unaccepted and lists what changes if it is rejected.

## Updating the queue after implementation

| What happened | Update | Where |
|---|---|---|
| this PR completes an item's last issue | make the next task obvious | the "Start with" line of the [queue](../../../../docs/work-plan/README.md) |
| a correctness dependency was discovered | add it as a dependency with the reason | the dependent spec's Dependencies; the issue's `blocked by` (confirm) |
| an issue was split or added (with confirmation) | add or change its row | [dependency chain](../../../../docs/work-plan/project-planning.md#dependency-chain) or [current priorities](../../../../docs/work-plan/current-priorities.md) |
| optional expansion came up | add a row with the evidence needed to promote it | [backlog](../../../../docs/work-plan/backlog/deferred-expansion.md) |
| scope or rationale proved wrong | correct the owning spec section, with the user's agreement | the item spec |
| a step inside an issue | usually nothing; say "queue: no change" and why | handoff |

Never write status, assignees, dates, checkmarks or "in progress" into Markdown. Renumbering spec
prefixes is allowed by the queue but breaks links and habits; propose it, do not do it in passing.

## When the item blocks midway

First finish the in-scope work that does not depend on the blocker
([blocker classes](../../cantos-prompt/references/blockers.md#decide-by-cause)). Then record the
reason and the native blocking relationship on the issue, move its GitHub Project status back and
state the next action — each a GitHub write to confirm first. Locally, keep the work on its
branch; push only when asked. The handoff names the blocker, the evidence gathered so far and
what remains unproven.
