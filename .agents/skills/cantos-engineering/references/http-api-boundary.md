# HTTP API boundary

> **Scope.** Keep Cantos Axum handlers thin and its contracts explicit: raw request DTOs,
> backend-enforced authorization, typed error mapping, idempotent retriable writes, versioned
> JSON Schema with fixtures, and shared DTOs for the Rust clients. Use when adding or changing an
> endpoint, DTO, error code, pagination, authentication rule or client contract.

Listener-facing contracts are owned by [`cantos-listening`](../../cantos-listening/references/listener-api.md);
this reference owns the general shape every endpoint follows. Implemented today: the Studio v1,
manuscript import, adaptation and production-input contracts in
[`services/cantos-api/src/adapters/inbound/http`](../../../../services/cantos-api/src/adapters/inbound/http/mod.rs) (`studio`, `theatre`, `admin`), each with a
hand-authored JSON Schema, fixtures and raw DTOs in `cantos-contracts` shared by the server and the
Studio ([contracts README](../../../../packages/contracts/README.md)). There is no OpenAPI document,
no generated client. The [Theatre Discovery/Playback v1 core](../../../../packages/contracts/theatre-discovery-v1.md) adds a strict public-only listener JSON boundary; actual PG/public delivery and generated-client acceptance are pending. Whether a route also carries MessagePack is owned by
[`wire-codec.md`](wire-codec.md).

For database-backed GET/read/fetch, load the foundation's mandatory
[read-performance workflow](postgresql-read-performance.md). Include internal reads behind
non-GET handlers; the HTTP verb alone does not decide DBSP suitability or freshness.

## One route group per surface

The host serves three groups under `/api/v1`: `studio` (implemented), `theatre` (anonymous Discovery/Playback core, fail-closed delivery) and `admin`
(reserved, empty, `404 not_found`). Each has its own router module in `src/adapters/inbound/http/`, its DTO module
in `cantos-contracts`, its own authentication and authorization, and its own `Store` methods or
adapter; shared code is middleware, error mapping, domain modules and the PostgreSQL authority.
Rules: a handler never calls another group's handler; a session for one surface grants nothing on
another (cookies are scoped by `Path`); a reserved group gets routes only with real behavior, never
mock data; the unprefixed `/api/v1/*` Studio paths are a deprecated alias that shares Studio
handlers and gets no new routes. Oracle: `services/cantos-api/tests/surface_routes.rs`.

## A handler is a shell

```text
extract + authenticate          Axum extractors, session → Actor
decode raw DTO                  serde, syntax only
convert to domain input         TryFrom, field-level diagnostics
authorize                       pure: (actor, resource, action) → Permit | Denied
decide                          pure core
persist / enqueue               adapter, one transaction, outbox
map outcome → response          typed error → status + stable code
```

```rust
// Illustrative.
async fn save_draft(
    State(app): State<AppState>,
    actor: Actor,
    Path(script_id): Path<ScriptId>,
    Json(request): Json<SaveDraftRequest>,
) -> Result<Json<DraftSaved>, ApiError> {
    let edit = DraftEdit::try_from(request)?;
    let draft = app.scripts.load_draft(script_id).await?;
    let permit = authorize(&actor, &draft, Action::EditScript)?;
    let saved = app.scripts.save_draft(permit, draft.apply(edit)?).await?;
    Ok(Json(DraftSaved::from(saved)))
}
```

The handler holds no business rule, builds no SQL and inspects no provider payload. A route that
negotiates its representation takes `Accepts` and `WireBody<T>` instead of `Json<T>` and answers
through `respond`, so the codec is settled before the store is called ([`wire-codec.md`](wire-codec.md)).

## Authorization lives in the backend

Every endpoint decides `(actor, resource, action)` on the server, fail-closed. A hidden or
disabled button is presentation, never protection. Studio roles (edit, produce, accept QC,
approve publication) and listener access are distinct actions even when one creator holds all of
them ([business rules § Rights and access](../../../../docs/product/business-rules.md#rights-and-access)).
Test each action with an allowed actor, a denied actor and an actor from another tenant or
account.

## Errors

Apply foundation [typed errors and composition](typed-errors-and-composition.md). The Rust API
application is **DomainError → ApplicationError → ApiError**, per bounded context. These are
roles, not three compulsory type names or a request to refactor the backend in a skills edit.

| Boundary | Owns | Must not expose inward |
|---|---|---|
| Pure context domain | validated factories, exact rejection ADT, correction facts | `StoreError`, SQL/SDK/HTTP/framework types, strings as error protocols |
| Context application and its ports | domain refusal plus capability-specific access/conflict/dependency/outcome-unknown cases; effect orchestration | concrete driver exceptions or a repository-wide domain mega-enum |
| Concrete infrastructure adapter | driver/OS/byte failures mapped into the port's owned typed vocabulary | raw foreign failures to callers or a dependency on HTTP mapping |
| HTTP adapter | exhaustive application outcome → status + existing `ErrorCode` + safe `ApiError` fields | HTTP status/codes in domain decisions |

Prefer `Result<T, ContextError>` and `?`; use explicit `map_err` or a narrow `From` conversion
where ownership changes. Mapping several foreign failures to one safe public code may be
intentional, but preserve required internal distinctions for reconciliation/corruption/retry.
Do not add a wrapper that merely renames an adequate outcome, or blanket-convert every failure
to `InvalidRequest`/`Unavailable`. Classification is based on meaning, not a driver message.

### Current code and incremental application

[`CatalogError`](../../../../services/cantos-api/src/contexts/catalog/domain/error.rs) is pure;
[`CatalogStoreError`](../../../../services/cantos-api/src/contexts/catalog/application/mod.rs)
wraps domain/storage errors and context cases;
[`catalog_error` / `domain_error`](../../../../services/cantos-api/src/adapters/inbound/http/studio/catalog.rs)
maps them to responses. Shared
[`StoreError`](../../../../services/cantos-api/src/platform/postgres/mod.rs) still mixes
application outcomes with storage failures; its PostgreSQL conversion currently maps driver
failures to `Unavailable`. This is an implementation observation, not a claim that context-owned
port errors or a complete taxonomy are implemented everywhere. Improve only a touched boundary
with its callers/tests; do not move infrastructure types into domain to imitate current glue.

### Preserve the wire contract and audit exhaustiveness

- [`cantos_contracts::ApiError` and `ErrorCode`](../../../../packages/contracts/api/src/lib.rs)
  are the current wire types. Retain the existing snake_case codes and the fields `code`,
  `current_revision`, `issues`; there is no free-form public exception/message field.
  Internal error reshaping alone must not rename codes, change statuses or redefine their meaning.
- Error bodies remain JSON regardless of negotiated success representation. Locale copy is
  a client projection of approved codes/field rules; never add raw SQL, paths, stack traces,
  private prose or signed URLs to help debugging. Restricted diagnostics stay at the shell.
- Exhaustively map every closed domain/application variant without a wildcard. A payload may
  delegate to a second exhaustive mapper. Open foreign values need a protocol/unsupported
  case in the concrete adapter, not a new catch-all domain variant.
- Creator-facing validation collects independent `FieldIssue` paths/rules. Conflicts retain
  `current_revision` where the existing contract needs it. Do not turn corrupt stored evidence
  into a caller validation issue or a lost write response into an established rejection.
- Add exact mapping tests over variants and payload boundary classes: status, serialized code,
  issues, revision and disclosure. Compiler exhaustiveness catches a missing arm; tests catch a
  wrong arm. Keep old fixtures and client decode tests; use the [wire codec matrix](wire-codec.md#3-test-matrix-for-any-codec-change)
  for negotiated routes. A code/shape change needs the owning contract's compatibility process.

Pure mapping tests need no database. Handler tests with named fakes establish L4 wiring; real
PostgreSQL acceptance still establishes constraints/transaction/replay semantics at L5. No
panic/try-catch business flow, generic monad framework, error mega-enum or new error dependency
is required to apply these rules.

## Retriable writes

Clients send a stable operation ID (idempotency key) on any write they may retry. The server
records it in the same transaction as the effect, returns the original result for a duplicate,
and rejects reuse of the key with different content. Test duplicate delivery explicitly.

## Contracts

- One source of truth per contract, versioned, with serialized fixtures in `packages/contracts/`.
  Today that is a hand-authored JSON Schema plus raw Rust DTOs in `cantos-contracts`, used by the
  server and the Studio and checked against the same fixtures on both sides. Kotlin types, when a
  CMP consumer exists, are generated or drift-checked against those fixtures; generated files name
  their source and generation command.
- Identifiers are opaque stable IDs, never titles or storage paths.
- Pagination, ordering, server timestamps and revisions are explicit fields.
- Compatibility: additive changes are safe only when consumers ignore unknown fields
  deliberately; removing or retyping a field is a version change with a migration window.
- A contract test asserts the handler's real response against the schema and fixtures, and a
  client test decodes the same fixtures. A round trip through new code alone proves nothing about
  old clients.

## Evidence

| Claim | Evidence |
|---|---|
| authorization decision | `example-tested` table over actors × actions on the pure authorizer |
| handler wiring and status mapping | handler test with a fake application state (`example-tested` against a double) |
| constraint, transaction or idempotency behavior | `integration-tested` against PostgreSQL |
| schema conformance | contract test over fixtures for each supported version |
| a client decodes the response | the client's own fixture test on its platform |
| a second representation carries the same facts | the [wire codec test matrix](wire-codec.md#3-test-matrix-for-any-codec-change) |
