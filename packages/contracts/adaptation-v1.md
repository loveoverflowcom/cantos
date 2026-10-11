# Private adaptation tools and v1 compatibility

Cantos exports source-bound data and validates caller-created proposals. The user-controlled
Gemini / ChatGPT / Codex host owns generation and chooses when to call these tools. Cantos
performs no local inference or hosted inference API call. Script IR read/write remains
`0.1.0`; source/revision c1/e1 digests are unchanged.

## Tool transport and identity

[Four tool schemas](adaptation-tools-v1.json) describe the actual CLI surface in
[`scripts/cantos_adaptation_tool.py`](../../scripts/cantos_adaptation_tool.py). An authorized agent
terminal tool invokes one command with a JSON object on stdin. Each command calls the existing
Axum service; it does not implement another store or run shell commands from its inputs.

| CLI command | Authenticated HTTP operation | Purpose |
| --- | --- | --- |
| `context` | `POST /api/v1/adaptations/contexts` | Freeze a source/revision context; return 201 |
| `context-read` | `GET /api/v1/adaptations/{run}/context` | Reopen the same frozen caller context |
| `submit` | `POST /api/v1/adaptations/{run}/proposals` | Validate caller output and persist a valid/invalid receipt; return 200 |
| `review` | `GET /api/v1/adaptations/{run}/review` | Read caller context or historical run for editorial comparison |

Run `python3 scripts/cantos_adaptation_tool.py schemas` to discover the four names/arguments.
All other commands require an explicit `--base-url http://127.0.0.1:<port>` (numeric loopback,
explicit port) and an existing operator-issued development session in the process-local
`CANTOS_SESSION_TOKEN` environment variable. The CLI has no token argument or credential
provisioning step. The HTTP cookie is `cantos_session`; mutation Origin must match the host.
No token, source or exception body is echoed by local transport errors. Environment proxies and
redirects are disabled; the session cannot follow a redirect to another destination.

This is a usable local terminal/HTTP tool seam, not a configured MCP server, public ChatGPT
connector, Gemini account or production authentication deployment. A host needs an authorized
terminal tool on this checkout, or its own authorized adapter to these HTTP contracts. Registering
a cloud account/connector or provisioning its access is outside this change. No ChatGPT/Gemini
web account was configured or connected by these tests.

The tool result is `{ok, http_status, result}`. Exit 0 means an HTTP success (an invalid
proposal still has a successful, inspectable receipt); exit 1 means a parsed HTTP failure;
exit 2 means local rejection or transport/response uncertainty. A write timeout, malformed,
oversized or truncated reply reports `write_outcome_unknown_retry_exact_operation`: reconcile
using the same operation ID and exact arguments. The CLI invents no operation ID, changes no
base, performs no automatic retry and exposes no acceptance tool.

Example input for `context` (original synthetic IDs; use actual owned import receipt values):

```json
{
  "operation_id": "00000000-0000-4000-8000-000000000010",
  "source_id": "src_synthetic",
  "source_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
  "extractor_version": "cantos-import-1",
  "script_id": "00000000-0000-4000-8000-000000000011",
  "expected_revision": 0,
  "rights_authorization": true
}
```

Invoke `python3 scripts/cantos_adaptation_tool.py --base-url http://127.0.0.1:8080 context`
with that JSON on stdin. `submit` instead takes
`{"run_id":"<context UUID>","submission":{...}}`; `context-read` and `review` take
`{"run_id":"<context UUID>"}`. Retain operation IDs and receipts privately outside Git.
A supplied session authorizes only the current actor's backend permissions; model/source
content cannot grant tool privileges.

## Frozen context and declared provenance

`AdaptationContextRequest` pins source ID, exact SHA-256, extractor version, target script and
expected revision, plus an explicit export/adaptation rights assertion. Missing source permission
claims or changed source/version/base fail before creating context. Replaying an exact context
operation returns its original context; changing the request with the same operation ID fails.

The c1 context stores the complete import receipt, ordered extraction/warnings, optional exact
base export/digests, server-created generation/rights IDs, prompt/contract versions and prepared
system/user text. Its separately labelled typed SHA-256 covers these frozen inputs. Output schema
is fixed by contract version. The context response includes that schema, digest and pinned input.
Publication eligibility remains unknown; export/adaptation assertions are not legal clearance.

`SubmitAdaptationProposalRequest` supplies a distinct `operation_id`, exact `context_digest`,
untrusted `proposal_json` and `generation` declarations: required `host_tool` and pinned
`prompt_version`; optional provider/model/configuration JSON, token usage and cost.
These are **caller-declared and unverified**, including any usage or monetary values. Absent
values remain unknown, never zero. A supplied cost must use `basis: "caller_declared"`, with
USD integer cents or VND integer dong. Model identity, authorship, actual billing and quality are
not attested by Cantos. Tool names, source strings and proposal text are inert data.

Each submission has an immutable receipt, output SHA-256, actor/time, generation declarations
and valid/invalid result. Invalid domain output leaves the context `awaiting_proposal`; an
explicit correction uses a new operation ID. An exact retry returns the original receipt even
after acceptance. The first valid submission stores the proposal atomically and changes the run
to `succeeded`; a distinct later submission conflicts instead of replacing it. Invalid HTTP
shape, oversized body, bad binding or invalid metadata fails before recording a submission.

## Admission, bounds and editorial acceptance

The model's narrow `cantos-adaptation-1` output is **not authoritative Script IR**. Its
[closed schema](schema/adaptation/cantos-adaptation-1.schema.json) separates scenes, narrator/
speakers, emotion/intensity, prosody/pacing notes, ambience/music/SFX cues, citations, omissions
and review notes. Unknown fields, IDs, evidence, assets, approvals and Markdown fences are
rejected. Nullable speaker is required; unsigned indices/intensity need literal integer tokens.
Cantos mints trusted IDs/evidence, preserves existing work/adaptation/episode identity, and calls
the actual Script IR structural/semantic validator.

Unknown speakers, uncovered/omitted blocks, source warnings and unsupported performance controls
remain findings. Citation coverage is a caller claim requiring complete source comparison; it
does not prove faithful meaning or correct attribution. Pacing/prosody notes remain proposals.

The application caps source context at 24 KiB/256 blocks, prepared text at 96 KiB, output at
256 KiB, scenes at 64, suggested characters at 128, total nodes at 1000, aggregate citations at
1024 and findings at 2000. Caller configuration is a JSON object bounded to 16 KiB, depth 8 and
256 nodes including keys. Context HTTP bodies are capped at 64 KiB; submission bodies at 1 MiB.
The CLI bounds stdin/encoded requests to 1 MiB and responses to 8 MiB, with a 10-second timeout.
These are data-pipeline limits, not claims about an external model's tokenizer or context fit.

Studio reads `/review`, displays preserved source/proposal and declared provenance, permits
bounded JSON correction, and requires explicit review/accept. Separate
`POST /adaptations/{run}/accept` accepts `operation_id`, `expected_revision`, complete edited
`script_json` and `reviewed_findings: true`. It uses the existing authorized revision-save
transaction and receipt. Stale head, changed trusted bindings, cancellation or invalid Script IR
cannot overwrite source/history. Acceptance grants no production or publication approval.
The tool surface does not expose this editorial mutation.

## Historical compatibility

Migration 0005 extends the existing run/proposal/acceptance authority with an immutable
`input_version` and append-only submission receipts. c1 contexts never dispatch inference or
create provider attempts. Historical a1 inputs keep their original decoder, field order and
fingerprint; optional `local_model_digest`/legacy config keys stay solely for stored-record
compatibility. Historical `GET /adaptations/{run}`, cancellation and acceptance receipts remain
readable. The retired start/provider compatibility endpoints return unavailable/null and cannot
generate. Expired historical dispatched attempts may reconcile to ambiguity without redispatch.

[Shared DTOs](api/src/adaptations.rs) and the [HTTP schema](schema/adaptation/v1.schema.json)
define both caller and historical wire shapes. Actual CLI/socket/PostgreSQL and Studio evidence
belongs in the [execution record](../../docs/evidence/ai-script-adaptation.md). Synthetic content and
model generation quality are distinct; no live model acceptance is implied by these contracts.
