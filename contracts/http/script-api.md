# Creator script API v1

The Axum implementation exposes the routes below. The authoritative transport shapes
are the named `$defs` in [script-api.schema.json](script-api.schema.json), a hand-authored
JSON Schema Draft 2020-12 contract. API `/v1` is independent of the embedded
[Script IR 0.1.0 schema](../schema/script-ir/0.1.0.schema.json). Fixtures are original synthetic
content with synthetic actor IDs and dates; they do not represent a production account
or cleared publication rights.

## Authentication and transport

Private routes require exactly one `Authorization: Bearer <credential>` header using
the [creator credential decision](../../docs/decisions/0003-creator-authentication.md).
The credential identifies the actor on the server. Requests cannot supply `actor_id`,
`owner_id`, timestamps, digests or accepted status. Only the script owner can read or
change it; an inaccessible script/version/revision returns the same `not_found`
response as a missing one. Authentication runs before private request processing.

JSON request bodies use `Content-Type: application/json`; the body limit is 2 MiB.
Every response, including errors and health checks, has `Cache-Control: no-store`.
An authentication error also has `WWW-Authenticate: Bearer`. Use HTTPS outside the
documented loopback development environment. Credentials are never URL parameters.

## Routes and payloads

| Method and route | Request definition | Success | Response definition |
| --- | --- | --- | --- |
| `POST /v1/scripts` | `CreateRequest` | `201` | `ScriptView` |
| `GET /v1/scripts/{script_id}` | None | `200` | `ScriptView` |
| `POST /v1/scripts/{script_id}/versions` | `SaveRequest` | `200` | `SavedVersion` |
| `GET /v1/scripts/{script_id}/versions/{version}` | None | `200` | `SavedVersion` |
| `POST /v1/scripts/{script_id}/revisions` | `AcceptRequest` | `200` | `AcceptedRevision` |
| `GET /v1/scripts/{script_id}/revisions/{revision_id}` | None | `200` | `AcceptedRevision` |
| `GET /v1/scripts/{script_id}/history?after=0&limit=50` | Query parameters below | `200` | `History` |
| `GET /healthz` | No authentication | `200` | `Health`, exactly `{"status":"ok"}` |

`script_id`, `revision_id`, `actor_id` and `operation_id` are UUIDs. Clients should
emit the standard hyphenated UUID representation; responses use lowercase. An
operation ID must not be the nil UUID. Editorial IDs within Script IR and source/rights
records are distinct stable strings, scoped to the script; they are not HTTP resource
IDs or credentials. Timestamps are server-generated RFC 3339 UTC values. Versions
are positive signed 64-bit integers; clients must preserve integer precision.

Creation stores the submitted source text without Unicode or newline normalization,
its SHA-256, a pending rights record and immutable saved version 1 with `base_version: 0`.
The response includes all of them. `source.id` must match the `source_record_id` of the
provenance entry named by the content's `work.source_ref`, and `rights.id` must match
`work.rights_record_id`. Source reference and rights evidence
are opaque metadata, never URLs that the server fetches. The `imported` source kind
records provenance of a supplied structured script; it does not implement file import.

`source.original_text` must contain non-whitespace text and is limited to 1,048,576
UTF-8 bytes. Source references and rights evidence references are nonblank, limited
to 2,048 UTF-8 bytes; the rights holder is nonblank and limited to 512 bytes. These
fields reject NUL. Schema `maxLength` checks character count; the application additionally
enforces these byte limits. Script content passes the separate Script IR reader; see
[Errors](#errors) for how its failures are reported.

Source and rights records are immutable for a script in this slice. Subsequent saves
must preserve the work ID, adaptation ID, episode ID, work source record ID and work rights
record ID. A correction
to the source or rights evidence currently requires creating a new script. Every rights
record remains `pending_review`; saving or accepting an editorial revision does not
clear production, adaptation, voice or publication rights.

## Saves, acceptance and retries

A save sends `operation_id`, `base_version` and complete Script IR `content`.
`base_version` must equal the head observed by the client. In one transaction the server
appends the next saved version with its actor and advances the head. It never changes
the prior content. Even an unchanged content submission creates a new version when
sent with a new operation ID. Competing saves from the same base cannot both succeed;
the rejected request gets `conflict` with the current `head_version`. Clients retain
their candidate, read the current head and resolve the difference before a new save.

Acceptance sends `operation_id` and the current head `version`. The server records an
immutable accepted revision pointing to that saved version. Later editing creates more
saved versions, while the accepted revision continues to return its original content,
digest, actor and timestamp. There is no PATCH, PUT or DELETE operation for saved versions
or accepted revisions.

Clients must retain the same operation ID and request when retrying a lost response.
Creation keys are scoped to the authenticated creator. Saved-version keys are scoped
to the owned script and include its initial creation operation; acceptance has a separate
operation-key scope for that script. Exact retries return the original result,
even after the head advances. Reusing an operation ID with a different validated request
returns `operation_reused`. Request equality for script content uses its normalized
validated representation; source text on creation is compared without normalization.

A creation retry always returns the initial `ScriptView` whose head is version 1;
use GET to read the current head. Retrying an acceptance with its original operation ID
returns its accepted revision after later edits. Accepting an already accepted saved
version with a different operation ID returns the original accepted revision and records
the new operation as an alias; its response `operation_id` remains the original one.
An attempt to newly accept an unaccepted older version returns a head conflict. Authentication
and authorization still apply to every retry.

The digest is `sir-c1:sha256:<64 lowercase hexadecimal characters>` over the canonical
content bytes, as specified by the [Script IR contract](../README.md#canonical-bytes-and-digest-c1).
Response `content` is the complete canonical export document, including provenance and rights
references the digest omits; those references are not proof of distribution permission. The independent source `sha256` is plain lowercase hexadecimal over the
exact UTF-8 `original_text` bytes.

## History pagination

`after` defaults to 0 and must be nonnegative. `limit` defaults to 50 and must be
between 1 and 100. Unknown or malformed query parameters return `invalid_request`.
The response contains `versions` and `revisions`, each ordered by ascending saved
version, filtered to `version > after` and independently limited to `limit` entries.
No client clock determines their ordering. Each entry records the original actor,
operation ID and server timestamp; rejected requests append no editorial history.

This endpoint inspects version ranges, not an incremental event feed or a snapshot of
concurrent activity. A version may be accepted after a client has already advanced
past it, so refresh an earlier range after acceptance. To traverse both independently
bounded lists, track each list's cursor or advance only past the smallest last version
among lists that reached the limit, deduplicating records on repeated pages. Advancing
to the greatest returned version can skip entries from the other list. There is no
public script catalog or global cross-script audit endpoint in this slice.

## Errors

All errors use `{"error":{"code":"..."}}`, with only the additional fields shown.
SQL errors, credentials and private text are never returned.

| HTTP status | Code | Additional fields / meaning |
| --- | --- | --- |
| `400` | `invalid_request` | Malformed JSON, UUID/path/query, unknown request field, invalid source metadata or invalid version/operation input |
| `401` | `unauthenticated` | Missing, malformed, unknown, expired or revoked credential, or disabled creator |
| `404` | `not_found` | Missing or inaccessible resource; unknown route |
| `405` | `method_not_allowed` | Unsupported method for the route |
| `409` | `conflict` | Required positive integer `head_version` |
| `409` | `operation_reused` | Operation ID was previously used with different input |
| `413` | `payload_too_large` | JSON request exceeds 2 MiB |
| `422` | `identity_changed` | Work/adaptation/episode/source/rights identity changed, or source/rights input does not match content |
| `422` | `invalid_script` | Required `reason` and `issues`, below |
| `500` | `storage_error` | Database failure or invalid stored content; no write success is claimed |
| `503` | `storage_unavailable` | Public health check cannot query PostgreSQL |

`invalid_script` carries the Script IR reader's `reason` and an `issues` array whose items
depend on it: `shape` → `{path, rule}` with JSON Pointer paths; `semantic` → `{path, issue}`
where `issue.code` names the validation issue and paths use stable IDs; `invalid_document` →
`{line, column}` only, because serializer messages can quote input; `document_too_large` →
`{max_bytes}`; `unsupported_schema_version` → `{supported}`; `missing_schema_version` → none.
Semantic issues may echo the submitter's own identifiers or pronunciation surfaces, never other
creators' content.

For ambiguous write failures, retry the unchanged request with its original operation
ID. Do not invent a new operation ID solely because a response was lost.

## Compatibility and evidence

Requests reject unknown fields. Response schemas also reject unknown fields in contract
tests so accidental drift is visible. A client may deliberately ignore additive response
fields, but a producer must change the contract and compatibility fixtures before relying
on them. Removal, type changes or changed version/retry meaning require a new API version
or an explicitly reviewed migration. Script IR unsupported versions remain a separate
`invalid_script` reason; they are not silently interpreted as the current version.

The first consumer is the creator API integration path and documented command-line
development flow. No generated Leptos or Kotlin client exists yet. Their future fixture
tests must decode this corpus before claiming client compatibility.

[Fixture manifest](fixtures/manifest.json) names each route definition used by
`apps/server/tests/contracts.rs`. Those tests validate shape, decode requests through
production DTOs and check source/digest/audit fixture consistency. PostgreSQL integration
tests validate real handler responses against the same schema. Passing fixture tests
alone does not prove database authorization or concurrency behavior.

Schema IDs are stable identifiers, not an instruction to fetch from the network.
Tests register the committed [Script IR schema](../schema/script-ir/0.1.0.schema.json) at its
`$id` using the shared test helper and validate UUID/date-time formats explicitly. The fixture set
reuses the [`two-scenes`](../fixtures/script-ir/0.1.0/accept/two-scenes.json) Script IR fixture;
the version 1 digest equals its reviewed golden, and response content and digests were produced
by `validate-script --export`. Changes to the contract or expected digest require a reviewed
explanation.

```sh
cargo test -p cantos-server --test contracts
```
