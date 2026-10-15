# Commit and pull request

> **Scope.** What happens when a slice is ready for review under the foundation's local handoff
> policy, **only when the user asks**
> to commit, push or open a PR: the pre-commit audit, the commit, the push, the PR body built from
> [`.github/pull_request_template.md`](../../../../.github/pull_request_template.md), evidence
> labels and what to do after the PR opens. The template and
> [CONTRIBUTING](../../../../CONTRIBUTING.md#branch-and-pull-request-workflow) own the fields and
> the workflow; this file owns how to fill them truthfully.

## Commit audit and final handoff

Use the [foundation cadence](../../cantos-engineering/references/verification-cadence.md).
Intermediate commits use applicable CHECKPOINT evidence; a review-ready handoff uses HANDOFF.
Audit each commit's content, but do not rerun the full verification set merely because a commit,
push or PR packages content already checked. Review evidence in this order; fix or report failures.

| Step | Do | Failure it catches |
|---|---|---|
| 1. inventory | `git status --short` and `git diff --stat`; every path belongs to the review boundary | unrelated cleanup, stray files |
| 2. evidence inventory | identify applicable format/lint/test and owner evidence, its covered inputs and remaining gaps under [§ 8](../../cantos-engineering/SKILL.md#8-clean-syntax-and-formatting-before-completion); use implemented commands and retain valid results | stale evidence, missing regression or formatting |
| 3. final handoff gates | `cargo xtask preflight --changed-only` on the final diff, or cite a still-valid run covering it; it includes repository/format and selected suites, so do not run their equivalents again. Supplement uncovered owner evidence. For missing prerequisites use [local handoff](../../cantos-engineering/references/local-execution.md#progressive-commit-push-and-pr) or the narrow [docs-only fallback](../../cantos-engineering/references/local-execution.md#docs-only-fallback-when-xtask-cannot-start). Intermediate commits do not trigger this step | broken links, skill structure, whitespace, uncovered gates |
| 4. content audit | read the full diff for the items in the next table | secrets and private material in history |
| 5. stage by path | `git add <path> …`; review `git diff --cached` | `git add -A` sweeping untracked captures or `.env` files |

An unavailable I/O gate does not stop independently verified work from reaching steps 4–5;
apply the foundation's policy and retain the incomplete verification in the PR. An actual
failure must be fixed or reported, never converted to a pass.

Never in a commit, even in a fixture or a test:

| Material | How it slips in | Check |
|---|---|---|
| provider credentials, tokens, `.env` values | copied into a test or example config | search the staged diff for keys, `Authorization`, `Bearer`, `secret`, `token` |
| signed URLs | pasted from a log into a fixture or doc | search for signature query parameters and expiring URLs |
| private manuscripts, real personal data | "realistic" fixture text | fixtures are original or permitted, like [the example episode](../../../../packages/contracts/examples/episode-draft.json) |
| generated audio, downloaded media | test output beside the source | `git check-ignore -v <file>`; [`.gitignore`](../../../../.gitignore) covers common audio extensions and `/artifacts/`, not every media type |
| screenshots with private content | evidence copied into `docs/` | evidence stays under the ignored `artifacts/` tree and is attached, not committed |

A secret that reached a commit is reported to the user immediately; rewriting history or rotating
credentials is their decision, not an agent's.

## Commit

- Follow the history's style (`git log --oneline`): `feat:`, `fix:`, `docs:`, `chore:` and an
  imperative subject of at most 72 characters naming the outcome, e.g.
  `feat: save script revisions and reopen them after restart`.
- The body says why, what the boundary excludes and `Refs #<n>`; add only the trailers your
  runtime's instructions require. Do not invent co-authors or reviewers.
- One commit per coherent step is fine; do not amend, squash or rewrite pushed commits unless
  the user asks.

## Push and open the PR

```sh
git push -u origin <branch>
gh pr create --base develop --head <branch> --title "<type>: <outcome>" --body-file <scratch file>
```

Write the body file outside the repository. A request to open a PR authorizes that PR against
`develop`; choose a descriptive title and a regular PR unless the user asks for a draft. Never
push to `develop` directly and never force-push a branch someone else may use. Labels, reviewers,
assignees, milestones and Project status are separate GitHub writes: confirm each, and use only
labels that `gh label list` shows exist.

## Fill every template field from evidence

| Template field | Write | Source | Common mistake |
|---|---|---|---|
| **Outcome** | the problem and the resulting behavior in one paragraph; `Closes #n` only when this PR completes the issue, otherwise `Part of #n` | the review boundary statement | `Closes #1` on the first of four slices |
| **Scope — Changed** | behavior, not file names | the ledger | a file list |
| **Scope — Deferred or excluded** | the boundary's "does not deliver" line, item by item | the boundary statement | omitted, so reviewers assume completeness |
| **Scope — Related decisions** | ADR or brief paths with status `proposed` or `accepted by …` | decision records | citing an unaccepted ADR as settled |
| **Verification** | each check that actually ran, its exact command and result; checks not run, with the reason | tool output | "all tests pass" with no command; listing a check that never ran |
| **Verification — layer gates / verified scope** | independent status and actual evidence level for each affected layer/claim; named doubles vs real infrastructure | [foundation ledger](../../cantos-engineering/references/verification-strategy.md#independent-verification-gates) | using a passed unit suite to mark DB/API acceptance passed |
| **Deferred acceptance** | each missing check, reason (environment/dependency/outside scope), prerequisites, exact next command, acceptance criteria, next owner and residual risk | foundation ledger and local handoff | omitting a blocked PostgreSQL run or creating an unrequested follow-up issue |
| **Compatibility and production impact** | the five questions below, or `N/A` with a reason | the ledger and owner skills | `N/A` for a change that alters a fingerprint input |
| **Review checklist** | tick only what is true now | — | ticking "acceptance criteria are met" for a partial slice |

For UI changes, attach inspected captures per the renderer loop
([`cantos-ui-design`](../../cantos-ui-design/SKILL.md#completion-report)). `gh pr create` uploads no
local images; tell the user which files to attach, or describe them as `evidence-local-only`.

### Compatibility and production impact — five questions

| Question | Answer with | Owner to consult |
|---|---|---|
| **Migrations**: can new code read old rows, can old code run on the new schema during rollout or rollback, is any step irreversible? | the forward/backward plan and the old-row test, or "no persisted data exists yet" | [persistence](../../cantos-engineering/references/persistence.md) |
| **Script IR / API**: did `schema_version`, a DTO, an error code or a field meaning change; which consumers (Studio, Theatre Web, CMP) are affected? | compatibility class, the new fixture added beside the old one, consumer status | [schema-versioning](../../cantos-script-ir/references/schema-versioning.md), [http-api-boundary](../../cantos-engineering/references/http-api-boundary.md) |
| **Cache and audio invalidation**: did any fingerprint input, canonicalization or dependency edge change? | which accepted artifacts become invalid, which stay reusable, and the test that shows it | [fingerprints-and-invalidation](../../cantos-production-pipeline/references/fingerprints-and-invalidation.md) |
| **Provider costs**: does the change add billable calls, alter retries or budget checks; were live calls made while developing? | the estimate, the enforcement point, and live calls made (count, cost, account alias — never credentials) | [cost-and-budget](../../cantos-production-pipeline/references/cost-and-budget.md) |
| **Rights and approval**: does it change when rights are checked or what invalidates an approval? | the gate affected and its test | [approvals-and-gates](../../cantos-publication/references/approvals-and-gates.md) |

Add rollout and recovery steps when any answer is not `N/A`.

## Label live and mocked evidence

[Mocked and live evidence are labeled separately](../../../../docs/work-plan/README.md#shared-completion-evidence),
with levels from the [foundation vocabulary](../../cantos-engineering/SKILL.md#6-never-collapse-evidence-into-the-word-verified):

| What ran | Write | Never write |
|---|---|---|
| a fake synthesizer in tests | `example-tested against a fake synthesizer (mocked)` | "TTS works" |
| production handler/application/adapter flow with external doubles | `mock-integration-tested (<flow>; <named doubles>)` | unqualified `integration-tested` or "live provider verified" |
| tests against a real local PostgreSQL | `integration-tested (local PostgreSQL <version>)` | `integration-tested` for an in-memory store |
| a local S3-compatible server | `integration-tested (local S3-compatible server)`; the deployed CDN path not exercised | "delivery verified" |
| one real provider call under a cost cap | `provider-live-tested (<provider alias>, 1 call, <cost>)` | the provider name with credentials or a request ID that exposes an account |
| Compose desktop preview | `semantics-tested` or `screenshot-inspected` on the desktop host; not `device-tested` | "works on Android" |

## After the PR opens

- Follow the foundation's [local handoff policy](../../cantos-engineering/references/local-execution.md#local-handoff-and-pr-completion).
  Return the PR link and local results when the PR opens; do not wait or poll for GitHub Actions.
  [`repository-checks.yml`](../../../../.github/workflows/repository-checks.yml) runs the xtask
  repository, Rust, Web and PostgreSQL jobs automatically. Describe remote CI as `not checked`
  unless actually read; it is not an agent completion requirement. Do not dispatch or re-run it unasked.
- A review is a separate read-only pass through
  [`cantos-code-review`](../../cantos-code-review/SKILL.md). Answering review comments, pushing
  fixes and resolving threads are further outward actions: confirm before each kind.
- Moving the issue to `In review` follows the
  [status policy](../../../../docs/work-plan/project-planning.md#status-policy) and is a GitHub
  write: propose it, do it after confirmation.

## Example body (illustrative)

```markdown
## Outcome

A creator can save a Script IR draft of "Ánh đèn cuối sân khấu — Một lời hẹn" as revision 1 and
reopen it unchanged after a backend restart. Part of #1.

## Scope

- Changed: revision persistence, the save/reopen endpoint, the semantic validator for speakers.
- Deferred or excluded: editing into revision 2, stale-save conflicts, import formats, editor UI.
- Related decisions or dependencies: docs/decisions/0002-… (proposed, awaiting the owner).

## Verification

- `<formatter> --check`, `<linter>`: pass (commands from the workspace manifest).
- `<test command> revision_reopens_with_same_digest_after_restart`: pass —
  integration-tested against local PostgreSQL <version> (live).
- `<test command> validator`: 6 passed — example-tested, exact `UnresolvedSpeaker` variant.
- `cargo xtask preflight --changed-only`: PASS (repository, rust-format, rust-lint, rust-tests,
  postgres); `web-build`: NOT_RUN (`--skip web-build`, no UI change).
- Not run: no UI in this slice; no provider involved.

## Compatibility and production impact

- Migrations: first migration; no persisted data exists yet. Applied migrations are never edited.
- Script IR/API: introduces `schema_version` 0.1.0-draft reading; old fixture kept unchanged.
- Cache/audio, provider costs, rights/approval: N/A — no generation or publication path exists.
```
