# Structured observability

> **Scope.** How Cantos code should emit runtime signals that an agent or a human can filter,
> correlate and trust: where events are emitted, their names and fields, levels, forbidden
> content, correlation identifiers, verbosity and when an event is a tested contract. Use when
> adding or reviewing diagnostic output, a logging or tracing dependency, request correlation,
> metrics or a telemetry service. Reading and investigating signals belongs to
> [`cantos-diagnostics`](../../cantos-diagnostics/SKILL.md).

## Repository reality

**These conventions are proposed. None is enforced by a tool.** Today `cantos-api` has no
`tracing`, `tracing-subscriber`, request ID, metric or trace pipeline. `cantos-server` writes a
few plain stderr lines; the CLIs (`validate-script`, `inspect-wav`, `diff-script`) write data
to stdout and errors to stderr; xtask prints `==> <step>` headers and a summary table; runners
write their own logs under `target/`. Durable facts such as submission receipts, approvals and
revisions are PostgreSQL rows, not log lines.

Adopting a logging or telemetry dependency needs a concrete consumer and a stated reason
([decision 0001](../../../../docs/decisions/0001-modular-monolith.md)): a reviewed work item that
names the operator question the events answer. Until then, apply these rules to any diagnostic
output a change adds, and do not add a dependency to satisfy them.

## OBS-01 — emit at the shell, never in the core

| Field | Contract |
|---|---|
| Level / scope | MUST for domain, Script IR, pure reducers and pure decisions on every platform |
| Rule / rationale | The pure core returns typed outcomes; the imperative shell (HTTP handler, application orchestration, adapter, runner, effect interpreter) turns an outcome into an event. A logger inside the core is an I/O dependency, makes equal inputs produce unequal effects and is invisible to tests. |
| Good / counterexample | the intake shell maps `Err(AudioIntakeError::BitrateLimit)` to its response and one event with `outcome = error.code()` (`"bitrate_limit"`) vs. the validator calling a logger or `eprintln!` before returning |
| Oracle / enforcement | import and call review of domain modules; the [functional core](functional-core.md) and ERR-02 in [typed errors](typed-errors-and-composition.md); manual |
| Exception | marked temporary diagnostic output during an investigation, removed before handoff ([cleanup](../../cantos-diagnostics/references/failure-triage.md#5-temporary-instrumentation)) |

## OBS-02 — name events and fields, not prose

| Field | Contract |
|---|---|
| Level / scope | SHOULD for every new runtime event |
| Rule / rationale | An event has a stable `snake_case` name of subject plus past-tense outcome and typed fields. Outcome codes reuse an existing stable code, such as `AudioIntakeError::code()` or the wire `ErrorCode`, never `Debug` output. Quantities carry units in the field name. A reader filters by name and field instead of parsing sentences. |
| Good / counterexample | `event=audio_submission_rejected outcome=bitrate_limit profile=opus_mono_24 actual_bitrate_kbps=64 duration_ms=18` vs. `"upload failed!!! bad file, see above"` |
| Oracle / enforcement | review of new event names against existing outcome codes; manual |
| Exception | process startup banners and CLI usage text written for a human at a terminal |

Prefer identifiers, counts, sizes, durations and hashes over content. Keep one event per
outcome at the boundary that owns it; do not log the same failure at every layer it crosses.

## OBS-03 — the level follows the error's meaning

Classify with [ERR-01](typed-errors-and-composition.md#err-01--classify-by-meaning-and-owner), then:

| Situation | Level |
|---|---|
| unexpected fault, corrupt persisted invariant, impossible adapter response; an infrastructure failure that fails the operation | `ERROR` |
| infrastructure failure recovered by a bounded retry; an outcome-unknown write awaiting reconciliation; security-relevant rejections such as a revoked producer grant being used or a declared and actual SHA-256 differing | `WARN` |
| lifecycle facts: startup with non-secret configuration summary, migration applied, submission accepted, release state changed | `INFO` |
| expected domain or validation rejection of caller input; branch decisions, timings, cache hits | `DEBUG` |
| per-item inner-loop detail | `TRACE` |

A caller's invalid input is an expected result, never `ERROR`. A business-significant
rejection that operators track may be `INFO`; decide that per event, not per error type.

## OBS-04 — no secrets or private content, at the source

| Field | Contract |
|---|---|
| Level / scope | MUST for every log, span, metric label, panic message and diagnostic file |
| Rule / rationale | Never emit passwords, tokens, producer grants, `Authorization` or `Cookie` headers, connection URLs, signed URLs, provider credentials, manuscript, script or dialogue text, briefs' private instructions, voice or audio bytes, personal data, raw AI prompts or model outputs. Downstream redaction is a second line, not the control. `CatalogError` already states the source-side rule: its variants never carry private prose or an unbounded caller value. |
| Good / counterexample | `content_unit_id`, `revision`, `sha256`, `byte_len`, `utterance_count` vs. a request body, `{token:?}` or a dialogue line in a rejection event |
| Oracle / enforcement | newtypes with redacting `Debug` for secrets (as `SignedUrl` in [`cantos-publication`](../../cantos-publication/SKILL.md)); review of every field added to an event; manual |
| Exception | none for secrets. Content excerpts only in a local, ignored diagnostic file under explicit user request |

## OBS-05 — correlate with bounded identifiers

| Field | Contract |
|---|---|
| Level / scope | SHOULD once a request or operation spans more than one module or process |
| Rule / rationale | A request or operation identifier is created at the boundary that accepts the work and carried as a span or event field through the shell; W3C `traceparent` is the candidate format if OpenTelemetry is adopted. High-cardinality identifiers (`request_id`, `trace_id`, `run_id`, user or work IDs) are fields, never metric labels or Loki index labels. Metric labels come from closed sets: route template, method, status class, outcome code. |
| Good / counterexample | a `request_id` field on every event of one Studio save; a `route="/api/v1/studio/works/{work_id}"` label vs. a label holding the concrete work ID |
| Oracle / enforcement | review of label sets; proposed, no telemetry exists |
| Exception | none |

Web and CMP clients reuse the backend's event names and send the same correlation identifier
when one exists; they do not invent a parallel vocabulary. Their reducers never log
([FP-01](../../cantos-cmp-mobile/references/kotlin-conventions.md#fp-01--pure-functional-core)).

## OBS-06 — verbosity is configuration, scoped and reversible

| Field | Contract |
|---|---|
| Level / scope | MUST for any filter or level change |
| Rule / rationale | The default level is `INFO`. Raise detail per module and per process for an investigation, for example `RUST_LOG=warn,cantos_api::contexts::media=debug` once `tracing` with an environment filter exists, with JSON output for machine reading. Never commit a raised default or require debug output for correctness. |
| Good / counterexample | a one-command environment variable during a diagnosis vs. `debug` persisted in `.cargo/config.toml`, a service manifest or CI |
| Oracle / enforcement | review of configuration diffs; manual |
| Exception | none |

## OBS-07 — logs are not the assertion

| Field | Contract |
|---|---|
| Level / scope | SHOULD for tests |
| Rule / rationale | Tests assert typed outcomes, state and rows. Log text is not an oracle: it is brittle and proves only that a line was printed. Assert an event only when it is a documented operational contract (an alert, audit or operator workflow depends on it), using a captured test subscriber, and name that consumer. |
| Good / counterexample | `assert_eq!(outcome, Err(AudioIntakeError::BitrateLimit))` and the unchanged submission row vs. `assert!(stderr.contains("bitrate"))` as the only check |
| Oracle / enforcement | [verification strategy](verification-strategy.md); review |
| Exception | CLI contracts whose stdout or stderr is the documented interface |

## Telemetry infrastructure — proposed, not implemented

A collector with log, trace and metric stores (for example OpenTelemetry Collector, Loki, Tempo,
Prometheus) would run as Podman services under [ADR 0021](../../../../docs/decisions/0021-podman-service-runtime.md):
its own ADR, a [service inventory](../../../../services/README.md) entry, explicit
`cargo xtask services up`, per-worktree isolation through `--instance` and no automatic start
from build or test. Agents could then query LogQL/PromQL against their own instance. None of
this exists; no report may claim `production-observed` from local logs.

## Review questions

- Does any domain, Script IR or reducer module now import a logger or print?
- Does each new event name an outcome with typed fields, at the boundary that owns it, once?
- Is the level consistent with ERR-01, with caller validation never at `ERROR`?
- Could any field carry a secret, a URL, story content or personal data?
- Are high-cardinality identifiers kept out of labels?
- Is a raised level or a `CANTOS_DIAG` marker left in the diff?
- Is a new dependency justified by a named consumer, or added to satisfy a convention?
