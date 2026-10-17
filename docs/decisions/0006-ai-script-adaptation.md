# ADR 0006: Host-owned generation, Cantos-owned adaptation data pipeline

- Status: proposed for production adoption; implemented local tool pipeline under review
- Date: 2026-10-10
- Decision owner: Cantos maintainers
- Related issue: [#3](https://github.com/loveoverflowcom/cantos/issues/3)
- Extends: [ADR 0005](0005-manuscript-import.md)

## Context and confirmed scope

The importer preserves exact source bytes, extraction blocks, warnings and creator rights claims.
The existing revision store admits complete validated Script IR exports with current authorization,
optimistic concurrency and operation receipts. AI output is untrusted editorial content and cannot
grant rights, establish reliable attribution or move a script head.

The user's confirmed decision is to control Gemini / ChatGPT / Codex generation externally and
ask that host to call Cantos tools. Cantos must supply the data pipeline, not local inference or
OpenAI/Gemini API calls. The previous local-adapter direction and live-local-model prerequisite
are superseded, not passed. No key, paid-call budget, account setup or cloud connection is required
by Cantos' data pipeline. The external host's generation and disclosure choices remain its user's
responsibility; this change sends no manuscript to an inference service.

## Decision

Reuse the modular Axum service, source registry, Script IR admission and immutable revision
authority. Expose four bounded authenticated tools over existing HTTP and a thin standard-library
Python CLI: context creation, exact context read, proposal submission and editorial review read.
An agent with an authorized terminal tool can invoke the CLI using JSON stdin and an existing
process-local Studio session. Publish tool schemas and examples. Do not build another engine,
service, generic provider abstraction, MCP framework or credential authority.

Context creation pins source ID/SHA-256, extraction version, exact base revision and its c1/e1
digests, actor's export/adaptation authorization, prompt/contract versions and prepared text.
Missing source claims, expected-source/version drift or stale base fail closed. The c1 frozen
context receives its own typed digest and server-created evidence IDs. Source and model content
remain inert data without host/tool privileges.

A caller supplies a separate submission operation, exact context digest, bounded narrow proposal
JSON and declared host/provider/model/config/prompt metadata. Supplied usage/cost is a caller
assertion with explicit caller-declared basis, not a provider attestation. Missing facts remain
unknown. Cantos cannot establish which model created a result, its billing or quality.

Validate declarations and the closed output contract, construct trusted identities/provenance,
then call the actual Script IR structural and semantic validator. Preserve unknown speakers,
source warnings, omission/coverage claims and unsupported performance intent as review findings.
Structural coverage cannot prove faithful meaning or correct attribution.

Persist an immutable valid/invalid submission receipt. Invalid domain output leaves its context
awaiting an explicit correction with a new operation ID. The first valid proposal and status
commit atomically; a later distinct submission conflicts instead of relabelling or overwriting it.
An exact retry returns the first receipt, including after editorial acceptance. HTTP malformed/
oversized requests and invalid bindings fail without a submission. No context schedules inference,
creates a provider attempt or retries external generation.

Studio is an editorial consumer: open a saved context/run, compare source and proposal, edit bounded
JSON, explicitly confirm review and accept. The separate acceptance mutation uses the existing
authorized revision-save transaction, CAS and idempotent receipt. Cancellation fences selection,
and stale head or changed bindings cannot replace source or accepted history. The CLI tool surface
contains no acceptance mutation. Full scene/character authoring remains #4.

A lost write response can still hide a committed context/submission. The CLI performs no automatic
retry or ID generation, reports exact-operation reconciliation for uncertain writes and retains
bounded structured errors without token/source echo. It uses numeric loopback HTTP, explicit port,
no environment proxy or redirect, bounded request/response reads and timeouts. This development
transport does not imply production identity/TLS or a configured public ChatGPT/Gemini connector.

## Compatibility and persistence

The original migration 0005 extended adaptation runs with immutable input versions and
append-only caller submission receipts. Its old `a1` compatibility policy is superseded by the
owner decision in [#238](https://github.com/loveoverflowcom/cantos/issues/238), dated 2026-10-17:
current runtime exposes only caller-authored `c1` contexts, proposals and reviewed acceptance.
Provider-run starts/reads/cancellations, `a1` decoding and legacy review DTOs are removed, without
rewriting existing immutable receipts. The `workflow: "caller"` review shape is unchanged.

The distinct [#239](https://github.com/loveoverflowcom/cantos/issues/239) owns fresh-schema
replacement and controlled deployed-database transition. Historical migration bytes and evidence
remain auditable; runtime compatibility is not a prerequisite to retaining that history.
Production inventory, backup/restore validation and approved cutover remain separate gates.
This clarification does not promote this ADR's proposed production adoption status.

## Alternatives and consequences

| Option | Benefit | Limitation | Decision |
| --- | --- | --- | --- |
| Existing authenticated HTTP plus thin CLI | Concrete tool consumer, no new framework/credentials, shared domain/store | Host needs authorized terminal/HTTP adapter; no public connector configured | Selected |
| MCP server | Standard host discovery | Adds SDK/runtime and a new integration surface without a current configured consumer | Deferred until a concrete host requires it |
| Cantos-owned local or cloud inference | Automated generation | Contradicts the confirmed host-owned scope | Removed / out of scope |
| Model emits trusted Script IR/evidence or automatically accepts | Fewer editorial steps | Lets untrusted output fabricate authority or overwrite competing edits | Rejected |

The merge gate is real CLI/HTTP context → valid/invalid submission → review/accept/reopen against
fresh disposable PostgreSQL, with ownership/revocation, source/version/base binding, duplicate/
retry/restart, bounds and transaction-fault evidence, mandatory checks and fresh review.
Actual model generation and editorial quality remain **NOT_RUN**, a separate nonblocking fact
for this pipeline scope. [Execution evidence](../evidence/ai-script-adaptation.md) must distinguish
the actual tool/storage path from synthetic content.

No TTS, media asset, publication, deployment or native mobile behavior changes. Revisit for a
concrete MCP/remote host consumer, production identity/TLS, larger chapters/chunking, rights
eligibility or full Studio authoring; keep the same source and revision authority.
