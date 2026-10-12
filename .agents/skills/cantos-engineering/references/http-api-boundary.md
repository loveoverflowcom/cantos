# HTTP API boundary

> **Scope.** Keep Cantos Axum handlers thin and its contracts explicit: raw request DTOs,
> backend-enforced authorization, typed error mapping, idempotent retriable writes, versioned
> JSON Schema with fixtures, and shared DTOs for the Rust clients. Use when adding or changing an
> endpoint, DTO, error code, pagination, authentication rule or client contract.

Listener-facing contracts are owned by [`cantos-listening`](../../cantos-listening/references/listener-api.md);
this reference owns the general shape every endpoint follows. Implemented today: the Studio v1,
manuscript import, adaptation and production-input contracts in
[`services/cantos-api/src/http`](../../../../services/cantos-api/src/interfaces/http/mod.rs) (`studio`, `theatre`, `admin`), each with a
hand-authored JSON Schema, fixtures and raw DTOs in `cantos-contracts` shared by the server and the
Studio ([contracts README](../../../../packages/contracts/README.md)). There is no OpenAPI document,
no generated client and no listener API yet. Whether a route also carries MessagePack is owned by
[`wire-codec.md`](wire-codec.md).

For database-backed GET/read/fetch, load the foundation's mandatory
[read-performance workflow](postgresql-read-performance.md). Include internal reads behind
non-GET handlers; the HTTP verb alone does not decide DBSP suitability or freshness.

## One route group per surface

The host serves three groups under `/api/v1`: `studio` (implemented), `theatre` and `admin`
(reserved, empty, `404 not_found`). Each has its own router module in `src/http/`, its DTO module
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

- Domain errors are enums. The adapter maps each variant to an HTTP status and a stable,
  documented error code; the message is safe for display after localization on the client.
- Error bodies are always the JSON `ApiError`, whatever the request's representation.
- Never return raw SQL, provider or stack-trace text. Never put secrets, signed URLs or private
  manuscript text in error bodies or logs.
- Validation errors at a creator-facing endpoint list every independent field problem.
- Conflicts (stale revision, lost lease, duplicate operation) are distinct codes with the current
  server revision when the client needs it to reconcile.

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
