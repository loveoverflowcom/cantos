# Run record

> **Scope.** Keep full command output on disk, outside the conversation and outside Git, so the
> context holds only summaries and the next agent or human can resume from one file. A run
> record is an **index**: the evidence is the tool output it points to. Agent-written text in it
> is never verifier output.

Contents: [location](#1-location), [capture](#2-capture-idiom), [commands ledger](#3-commandsjsonl),
[summary](#4-runmd), [redaction](#5-redaction), [cleanup](#6-cleanup-and-retention).

## 1. Location

```text
artifacts/agent-runs/<run-id>/
  run.md              summary for the next reader (template in § 4)
  commands.jsonl      one mechanical line per executed command (§ 3)
  logs/NN-<step>.log  full combined stdout and stderr, untruncated, one file per command
```

- `/artifacts/` is already ignored by the root `.gitignore`, beside the UI inspector's
  `artifacts/ui-evidence/<run-id>/`. Do not add a second ignored root such as `.artifacts/`.
- `<run-id>` is a sortable UTC stamp plus a slug: `2026-10-13T0415Z-intake-profile`.
- `NN-<step>` is a two-digit sequence and a lowercase slug (`03-verify-rust`,
  `04-baseline-verify-rust`). Slugs use only `a-z`, `0-9` and `-`.
- Runners that already write their own evidence (the PostgreSQL gate under `target/`, Compose
  preview under `artifacts/ui-evidence/`) keep it there. Point to those paths; do not copy them.
- Open a record when output exceeds one screen, when a baseline or rerun is needed, or when the
  work may continue in another session. A single short command needs none.

## 2. Capture idiom

Redirect the whole command to its log, then read the log progressively:

```sh
RUN=artifacts/agent-runs/2026-10-13T0415Z-intake-profile
step=03-verify-rust
mkdir -p "$RUN/logs"
{ printf '$ %s\n' 'cargo xtask verify rust'
  printf '# cwd=%s head=%s dirty_paths=%s\n' "$PWD" "$(git rev-parse --short HEAD)" "$(git status --porcelain | wc -l | tr -d ' ')"
} >"$RUN/logs/$step.log"
start=$(date +%s)
RUSTUP_TOOLCHAIN=1.98.1 cargo xtask verify rust >>"$RUN/logs/$step.log" 2>&1; code=$?
printf '{"step":"%s","exit":%d,"seconds":%d,"log":"logs/%s.log"}\n' \
  "$step" "$code" "$(( $(date +%s) - start ))" "$step" >>"$RUN/commands.jsonl"
tail -n 3 "$RUN/logs/$step.log"
```

- Never pipe the command itself into `head`, `tail` or `grep`: the exit code and the full output
  are lost. With `tee`, set `set -o pipefail` first.
- The header records the command text, directory, HEAD and dirty count. Record exported
  variables by **name and non-secret value** only (`RUSTUP_TOOLCHAIN=1.98.1`), never an
  environment dump, connection URL or token.
- Run one heavy lane at a time ([resources](../../cantos-engineering/references/local-execution.md#resources)).
- This is a capture convention around existing commands, not a new build, test or run
  entrypoint. A machine-readable xtask summary is **proposed**, not implemented; do not hand-write
  JSON that imitates tool output.

## 3. commands.jsonl

One line per executed command, written by the capture idiom, never edited afterwards:

| Field | Meaning |
|---|---|
| `step` | the log's sequence and slug |
| `exit` | the shell exit status actually observed |
| `seconds` | wall time, for spotting slow gates and repeated runs |
| `log` | path of the full output, relative to the run directory |

Gate statuses are not copied here: they live in the xtask summary at the end of each log and
are transcribed into `run.md` with a line pointer. Reading the ledger is D0 for agent activity:

```sh
python3 -c 'import json,sys; [print(r["step"], r["exit"], r["seconds"]) for r in map(json.loads, open(sys.argv[1]))]' "$RUN/commands.jsonl"
```

## 4. run.md

Write it for a reader who has not seen the conversation. Keep it short; point, do not paste.

```markdown
# <run-id> — <the question in one line>

- Task / trigger:
- Source: base <sha> · HEAD <sha> · dirty <n> paths · rustc <version> · other tools from doctor
- Environment: exported variables by name and non-secret value
- Failure signature:
- Classification: environment | pre-existing | flaky | regression | test-defect | unknown — oracle

## Hypotheses
| # | Hypothesis | Predicts | Check (log pointer) | Result | Status |

## Gate ledger
| Layer / gate | Printed status | Log pointer | Note |

## Loop health
commands <n> · identical reruns <n> · fix attempts per signature <n> · deepest depth D<k>

## Cleanup
instrumentation: `git grep` clean | n/a · verbosity: default · worktrees removed · processes stopped

## Next step
owner and action; what a resuming agent should read first
```

A resuming agent reads `run.md` and `commands.jsonl` first (D0), opens logs only where a pointer
leads and treats every claim in them as diagnostic data to re-check against the current HEAD:
a recorded result says nothing about code changed after it.

## 5. Redaction

- Logs may contain disposable test tokens and database URLs (for example a runner's `run.json`),
  private file paths, manuscript or dialogue text and producer identifiers. They stay local.
- Excerpts in a report, PR, issue or chat carry the minimum lines and replace tokens, URLs,
  credentials, signed URLs and story content with `<redacted>`.
- Never upload a run directory or raw log to an external service, issue, PR or tool without the
  user's request; never print environment variables or credential files to capture them.
- Follow the foundation's [secrets and media rules](../../cantos-engineering/references/local-execution.md#secrets-private-content-and-media).

## 6. Cleanup and retention

Before handoff, prove and record:

```sh
git grep -n 'CANTOS_DIAG' -- . ':!.agents/skills'   # no temporary instrumentation left
git status --short                                  # only intended changes; no run files staged
git worktree list                                   # no baseline worktree from this run remains
```

- Stop processes and disposable clusters this run started (`--keep` clusters print their stop
  command); never stop or delete what another session owns.
- Restore default verbosity: nothing persisted in `.cargo/config.toml`, profiles, `.env`,
  service manifests or CI.
- Keep the run directory until the handoff is accepted; delete only run directories this
  session created, and never `target/` evidence that a document under `docs/evidence/` cites.
- Durable knowledge does not live in a log: a fixed bug leaves a regression test and a report;
  a recurring environment trap is reported to the user for the owning document.
