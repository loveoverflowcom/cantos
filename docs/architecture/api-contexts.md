# Cantos API context boundaries

Status: implemented internal organization of the single `cantos-api` crate. The
[handoff evidence](../evidence/api-bounded-contexts.md) records verification separately.
[ADR 0018](../decisions/0018-external-audio-production.md) later removed the Synthesis context:
Cantos runs no text-to-speech, and the Media context only inspects audio that external producers
will submit. This document describes the resulting structure. The original reorganization added
no API surface, schema, production provider or deployment unit. [ADR 0001](../decisions/0001-modular-monolith.md) still owns the modular-monolith
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
| Media | Pure, read-only inspection of externally produced audio (WAV PCM); submission use cases when the API exists | None |

An upstream dependency uses its owner's named entrypoint, never its private application or
PostgreSQL module. Domain dependencies name `contexts::<owner>::domain` directly so an
application helper re-export cannot silently enter the pure core. Production's dated provider
capabilities are immutable data and validation rules under `domain/catalog/`; no provider process,
HTTP transport or credential exists in Cantos. Media's byte-only WAV inspection lives in its pure
domain and does no I/O.

`interfaces/http/` owns routing, cookie/origin/body checks, wire negotiation and error formatting.
Its handlers delegate to `Store` application methods. Three existing pure boundary operations
remain explicit: admitting Script IR request bytes, mapping Script IR diagnostics to wire issues,
and returning the immutable production catalog. HTTP owns no rights, approval, retry or job policy.
Studio, Theatre and Admin are separate API surfaces; they are not competing domain models.
Theatre and Admin routers still return the existing reserved responses.

`platform/` owns IDs, connection/session/migration plumbing, trusted local operator utilities and
the engine-independent incremental support, which no read model uses since the job-summary view
was removed. `platform/` names no business context. The operator utilities retain their existing
privileged fixture/source-recording SQL; they are not HTTP use cases.

## Transactions

Extracting a pure decision does not move locked facts outside the transaction. Editorial
append/replay, adaptation submission/acceptance and production freeze/approval authenticate,
lock/load current facts, decide, persist and commit under the same transaction. Exact retries
still reconcile their prior immutable result. Authorization and the expiry checks at the final
write boundary remain necessary after a lock wait.

Cantos runs no worker: there is no lease, fence, dispatch intent or reservation to protect. A
future submission use case carries the same rules as the others, and must additionally make
repeated uploads idempotent and bind each result to an exact revision and checksum.

SQL for business tables stays in the owning context's PostgreSQL adapter. Two existing read-only
joins remain explicit exceptions: Ingestion's linked-source query reads Editorial's
`revision_evidence` under the already-held script lock, and Editorial's active-reviewer query
joins platform `actors`. Neither exception admits a cross-context write or a query from another
file.

## Compatibility and checks

The crate-root module paths of the retained contexts remain hidden compatibility re-exports; the
`tts`, `worker`, `jobs` and `postgres::durable` aliases went with Synthesis. New backend context
code uses canonical owner paths; the architecture guard rejects legacy aliases there. HTTP/CLI wire DTOs, route groups, JSON/MessagePack negotiation,
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
