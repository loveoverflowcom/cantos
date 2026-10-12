# Studio revision API v1

**Mount and compatibility.** The routes below are served under `/api/v1/studio` (base path in
`cantos_contracts::routes::STUDIO_BASE`). The pre-split `/api/v1/<route>` paths remain a deprecated
alias served by the same handlers, with identical bodies and a `Deprecation: true` response header;
a session cookie from the alias is scoped to `/api/v1`, one from `/api/v1/studio/session` to
`/api/v1/studio`. Wire DTOs and error codes did not change. Listener and administration APIs are
separate groups (`/api/v1/theatre`, `/api/v1/admin`), reserved and empty.

Implemented development boundary: [Axum](../../services/cantos-api/src/http/studio.rs) →
[PostgreSQL](../../services/cantos-api/src/postgres.rs), consumed by [Leptos](../../apps/cantos-studio/web/src/api.rs).
The [Schema](schema/studio/v1.schema.json), [raw Rust DTOs](api/src/lib.rs) and
[fixtures](fixtures/studio/v1) are hand-authored. Both consumers test the same fixture source;
backend tests validate live responses with that Schema. No generated OpenAPI/Kotlin API exists.

## Routes

| Method/path under /api/v1/studio | Request / result |
| --- | --- |
| POST /session | SessionRequest token → SessionResponse actor_id and HttpOnly/SameSite=Strict cookie |
| DELETE /session | Revoke token in PostgreSQL, expire cookie → 204 |
| GET /scripts/{UUID}/head | Current authorized Revision |
| GET /scripts/{UUID}/revisions/{positive revision} | Pinned immutable Revision, current access still required |
| POST /scripts/{UUID}/revisions | SaveRevision → Revision; 200 for both acceptance and exact replay |

All mutations require the configured exact Origin. API responses carry no-store/nosniff.
An existing active, unexpired, unrevoked database session authenticates the actor; request
actor headers are ignored. Tokens are exactly 64 hex characters, stored as SHA-256 hashes.
The cookie lacks Secure only because this host deliberately supports loopback development HTTP.
Production sign-in, HTTPS and Secure cookies require a separate reviewed deployment.

Owners/editors read and write; readers only read. Unknown/inaccessible scripts return 404,
not an existence leak; a known reader's write returns 403. First save at base 0 atomically
creates an owner script with revision 1. Later saves require its locked current head. Each
read, save and retry checks current access. A revoked member/session cannot replay old results
through this API.

SaveRevision contains expected_revision (0..i64::MAX), operation_id (canonical lowercase UUID)
and script_json (complete Script IR JSON string). The server revalidates schema/text/semantics,
canonicalizes a complete export, resolves every source/rights/generation/asset reference to
an immutable registry record owned by the script owner, then appends within one transaction.
Registry existence/ownership does not assert legal publication eligibility or preserved source
bytes. Validated storage acceptance does not mean production approval.

## Retry and integrity

The durable key is (script_id, authenticated actor, operation_id). Reuse with the same expected
base and complete canonical export returns the original revision, actor and UTC microsecond
timestamp even if the head has moved. A changed snapshot or base returns operation_reused.
Prior-operation lookup precedes the stale-head decision, after current authorization. A
different new key with a stale base returns stale_revision/current_revision. There is no
deduplication by c1, e1 or text equality.

An unknown transport/commit outcome must retry the same key/base/export as the original actor.
A caller cannot infer rollback from losing an HTTP response. The Studio keeps that snapshot in
memory, blocks new save/read intents until reconciliation, and preserves separately edited text.
A browser reload loses this in-memory intent; durable client recovery is a future authoring gate.

Revision returns script_id, revision, accepted_by, accepted_at, content_digest, export_digest
and the exact complete canonical script_json. Accepted rows and their evidence links are
append-only. Reads re-admit canonical bytes and verify both digests; corruption returns 500
rather than normalized/repaired content.

```text
ExportDigest = "sir-e1:sha256:" + lowercase_hex(
    SHA256(b"cantos/script-export/e1\n" + complete_canonical_export_bytes)
)
```

e1 includes schema, provenance and rights references omitted by the unchanged c1 content
projection. Revision identity plus its full export/evidence remains the accepted fact.

## Errors and bounds

| HTTP | code | Meaning |
| --- | --- | --- |
| 400 | invalid_request | Invalid DTO, UUID or revision bounds |
| 401 | unauthenticated | Missing, invalid, inactive, expired or revoked session |
| 403 | forbidden | Reader write or invalid mutation Origin |
| 404 | not_found | No row in current actor scope |
| 409 | stale_revision | Head changed; current_revision supplied |
| 409 | operation_reused | Same actor/key bound to another base/export |
| 422 | invalid_script / evidence_unavailable | Invalid IR or missing owner evidence |
| 503 | unavailable | Pool, statement/lock/request deadline or unknown transport/commit result |
| 500 | corrupt_revision | Stored canonical bytes/digest failed admission |

ApiError has code, nullable current_revision and issues [{path, rule}]. IR diagnostics expose
stable codes/paths, not raw manuscript values or SQL errors. Input IR is bounded to 2 MiB bytes;
the HTTP envelope limit is 8 MiB, allowing JSON escaping overhead. Pool size 8, wait/connect
5s, SQL 5s, lock 3s, idle transaction 10s and HTTP deadline 15s bound this development slice.
These are resource guardrails, not measured latency targets or production sizing.

Version 1 admits Script IR 0.1.0 only. Any changed wire semantics needs a compatibility decision
and shared fixture updates; no persisted older version conversion exists. See
[persistence evidence](../../docs/evidence/script-revision-persistence.md).

## Editorial handoff extensions

These additive v1 routes use the same cookie, Origin guard, no-store responses and error shape.
Existing save/read DTOs and Script IR c1/e1 remain unchanged. New raw DTOs, Schema definitions
and fixtures live alongside the original contract.

| Route under /api/v1/studio | Request / result |
| --- | --- |
| GET /scripts/{UUID}/history | Optional `after_revision` (default 0) and `limit` (default 20, range 1–50) → History |
| POST /scripts/{UUID}/reviews | ReviewRequest `{operation_id, revision}` → Review; owner only |
| GET /scripts/{UUID}/sources/{SOURCE_ID} | Source with exact UTF-8 text/hash, only if linked from the script's immutable history |

History contains ascending `revisions` summaries (script/revision, accepted actor/time, c1/e1),
`reviews` for those same revisions, and nullable `next_after`. Open a manuscript through the
existing pinned revision route; history omits its body. Each page checks current access and
holds the script's share lock while reading and integrity-checking its rows. New revisions
between pages appear after the previous cursor; pages are not a frozen full-history snapshot.
An empty/last page has `next_after: null`. Invalid bounds or unknown query fields return 400.
No cache or incremental read model bypasses authorization.

A new review must name the locked current head, with a canonical UUID operation key. A retry
with the same actor/key/revision returns the first review even if the head advances; changing
its revision returns `operation_reused`. A new key for an already reviewed revision also
returns its first review and records that receipt. An old unreviewed revision returns
`stale_revision`. Review and receipt commit atomically. Editors/readers cannot review; readers
and editors can inspect history/sources while their current script access permits it.
Review has `id`, script/revision, original operation ID, reviewed actor/time and the pinned
revision's c1/e1. Review is editorial evidence, not production or publication approval.

Source has `id`, `reference`, `original_text`, `sha256` and `recorded_at`. Original text is
bounded to 1 MiB UTF-8 bytes, preserved without normalization and hash-checked on read. It is
available only through a currently authorized script and its source evidence links. Missing,
unlinked and inaccessible sources return 404. A registry-only legacy source remains valid
for existing save/read semantics but returns 404 here until explicitly preserved by an operator.
Source text is never embedded into a publication manifest or public route. Rights assertions,
generation and selected asset references still require their existing registry ownership checks
and downstream eligibility gates.

## Nonpersistent validation preview

`POST /api/v1/studio/validation` accepts the closed `ValidateScriptRequest {script_json}` and returns
`ScriptValidationResponse {issues: []}` for valid Script IR. Invalid IR returns the existing
`422 invalid_script` error with the actual reader's ordered paths/rule codes. Authentication,
exact Origin, no-store/nosniff, the 8 MiB request envelope, 2 MiB Script IR bound and HTTP
deadline are shared with the revision routes. No script ID is accepted or looked up.

The preview writes no records, resolves no registry evidence, proves no target permission and
moves no revision head. Any authenticated actor can validate text they supply. Acceptance/save
remain the authority for current access, provenance evidence, optimistic concurrency and immutable
storage. Existing v1 DTOs, Script IR 0.1.0, c1/e1, old fixtures and migrations remain unchanged.

## Representations

`GET /scripts/{UUID}/head`, `GET /scripts/{UUID}/revisions/{revision}`, `POST /scripts/{UUID}/revisions`
and `POST /validation` carry their DTOs as JSON by default or as MessagePack (`application/msgpack`,
named maps) when the request asks for it. `Content-Type` selects the request decoder and `Accept`
the response encoder; errors stay JSON `ApiError` with the codes above. A `Content-Type` that is
neither JSON nor `application/msgpack` on these routes is `415 invalid_request`, and an `Accept`
that allows neither is `406 invalid_request`, each with one issue naming the header. Successful
negotiated responses carry `Vary: Accept`. Every other route in this contract is JSON only.

The DTOs, Schema, fixtures, retry semantics and c1/e1 digests are unchanged: `script_json` is the
same text in both representations, and a retry may use either representation with the same
operation key. [`msgpack-goldens.json`](fixtures/studio/v1/msgpack-goldens.json) pins the
MessagePack bytes; `save-revision.json` and `validate-script.json` are the request fixtures it
covers. Rules, limits and rollback: [wire codec](../../docs/architecture/wire-codec.md).
