# Cantos API context boundaries

Status: implemented internal organization of the single `cantos-api` crate. The
[handoff evidence](../evidence/api-bounded-contexts.md) records verification separately.
This reorganizes the existing backend; it adds no API surface, schema, production provider or
deployment unit. [ADR 0001](../decisions/0001-modular-monolith.md) still owns the modular-monolith
choice, and [ADR 0015](../decisions/0015-api-surface-boundaries.md) owns Studio/Theatre/Admin isolation.

## Ownership and dependencies

`services/cantos-api/src/contexts/` is organized by business capability, with a pure `domain`,
private `application` use cases and a private `postgres` adapter in each context. The one
versioned `script_ir/` module is the shared kernel, outside those contexts. Domain functions
receive explicit facts and return decisions or immutable values. Application methods on the
concrete `Store` gather facts, call those functions, persist results and own transaction commit.
No repository trait or generic effect framework is needed for the existing PostgreSQL consumer.

| Context | Owns | Allowed upstream contexts |
| --- | --- | --- |
| Editorial | Accepted revisions, review, script access and evidence links | None |
| Ingestion | Bounded extraction, source bytes and immutable import receipts | Editorial |
| Adaptation | Frozen caller context, proposal admission, acceptance and cancellation | Editorial, Ingestion |
| Production | Casting capabilities, settings, rights, estimates, frozen inputs and approval | Editorial |
| Synthesis | Durable jobs, attempts, receipt/audio admission, speech identity, reuse and regeneration | Editorial, Production |

An upstream dependency uses its owner's named entrypoint, never its private application or
PostgreSQL module. Domain dependencies name `contexts::<owner>::domain` directly so an
application helper re-export cannot silently enter the pure core. Production's dated provider
capabilities are immutable data and validation rules under `domain/catalog/`; provider processes,
HTTP transports and credentials belong to `synthesis/providers/`. Synthesis byte-only WAV/audio
admission lives in its domain, while filesystem clip probing stays under `media/`.

`interfaces/http/` owns routing, cookie/origin/body checks, wire negotiation and error formatting.
Its handlers delegate to `Store` application methods. Three existing pure boundary operations
remain explicit: admitting Script IR request bytes, mapping Script IR diagnostics to wire issues,
and returning the immutable production catalog. HTTP owns no rights, approval, retry or job policy.
Studio, Theatre and Admin are separate API surfaces; they are not competing domain models.
Theatre and Admin routers still return the existing reserved responses.

`platform/` owns IDs, connection/session/migration plumbing, trusted local operator utilities and
the incremental engine support. Its sole context-type dependency is the feature-gated
`synthesis::summary_view::AttachedView` attachment required by
[ADR 0017](../decisions/0017-experimental-dbsp-job-summary.md). The operator utilities retain
their existing privileged fixture/source-recording SQL; they are not HTTP use cases.

## Transactions and worker execution

Extracting a pure decision does not move locked facts outside the transaction. Editorial
append/replay, adaptation submission/acceptance and production freeze/approval authenticate,
lock/load current facts, decide, persist and commit under the same transaction. Exact retries
still reconcile their prior immutable result. Authorization and the expiry checks at the final
write boundary remain necessary after a lock wait.

Synthesis application modules own enqueue, status, control, preparation, dispatch authorization,
attempt observation/settlement, output acceptance/quarantine and explicit not-executed
reconciliation. They keep dispatch intent, reservation, fencing and selected-result persistence
inside their transactions. The PostgreSQL adapter is split by jobs, leases, attempts, observations,
acceptance, reconciliation and clips. Lease claim/heartbeat and clip selection retain their
concrete atomic persistence boundaries; this is not a claim that every adapter is a single SQL
statement.

Worker modules divide runtime/heartbeat, dispatch, recovery/reuse and the private output journal.
They retain the existing explicit local eSpeak or synthetic development provider configuration.
A recorded ambiguous attempt still blocks automatic redispatch; a stale lease still cannot select
output; reuse still creates no provider attempt. No external exactly-once guarantee is introduced.

SQL for business tables stays in the owning context's PostgreSQL adapter. Two existing read-only
joins remain explicit exceptions: Ingestion's linked-source query reads Editorial's
`revision_evidence` under the already-held script lock, and Editorial's active-reviewer query
joins platform `actors`. Neither exception admits a cross-context write or a query from another
file. Synthesis loads stored frozen-document bytes through Production's transaction helper.

## Compatibility and checks

The existing crate-root module paths remain hidden compatibility re-exports, including
`postgres::durable` and `tts`. New backend context code uses canonical owner paths; the architecture
guard rejects legacy aliases there. HTTP/CLI wire DTOs, route groups, JSON/MessagePack negotiation,
error mapping, migrations, stored bytes and historical digest inputs retain their existing contracts.
No removal date for these library aliases is implied by this refactor.

`cargo xtask verify repository` runs the dependency guard in `scripts/check_repository.py` and
its unit tests through the existing repository gate. It checks grouped and aliased imports,
fully qualified paths, `self`/`super` references, context directions, private application/adapter
modules, domain I/O/runtime/clock/randomness dependencies, HTTP SQL/policy access and statically
named table ownership. Comments and ordinary/raw string literals do not create import edges.
Pure in-memory `std::io::{Cursor, Read}` parsing remains allowed.

This is a source-level guard with Rust privacy and compiler checks as additional evidence.
It does not expand macros, resolve arbitrary transitive re-exports, parse dynamic SQL or prove
that arbitrary handwritten handler code contains no policy. Review and the behavioral suites
remain necessary. Read queries and schema are preserved; the refactor claims no read-performance
improvement and does not change the experimental DBSP adoption decision.
