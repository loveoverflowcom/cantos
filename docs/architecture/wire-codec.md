# Wire codec: JSON and MessagePack

Cantos HTTP routes carry contract DTOs as JSON. A small set of Studio routes can also carry the
**same DTOs** as MessagePack, chosen per request with `Content-Type` and `Accept`. This is a
representation choice only: routes, business rules, authorization, revision concurrency,
idempotency, Script IR bytes and storage are identical for both. Nothing is persisted in
MessagePack.

This document owns the protocol rules. The decision and its alternatives are in
[ADR 0013](../decisions/0013-optional-messagepack-wire.md); measurements and test results are in
the [MessagePack wire evidence](../evidence/messagepack-wire.md); the working method for adding
or reviewing a codec change is the engineering skill's
[wire codec reference](../../.agents/skills/cantos-engineering/references/wire-codec.md).

Status: **implemented** on the server for the routes below; the Studio client is wired to the same
codec and **ships with JSON** (`REVISION_CODEC = Json`). MessagePack is a tested, switchable
foundation, not the default.

## Owning code

| Concern | Code |
| --- | --- |
| DTO shapes (one source of truth) | [`cantos-contracts`](../../packages/contracts/api/src/lib.rs) |
| Media types, named encoding, bounded decoding | [`cantos_contracts::wire`](../../packages/contracts/api/src/wire.rs) |
| Server negotiation, extractors, response adapter | [`http/wire.rs`](../../services/cantos-api/src/http/wire.rs) |
| Studio codec switch and response decoding | [`apps/cantos-studio/web/src/wire.rs`](../../apps/cantos-studio/web/src/wire.rs), used by [`api.rs`](../../apps/cantos-studio/web/src/api.rs) |
| Independent oracle and goldens | [`scripts/reference_msgpack.py`](../../scripts/reference_msgpack.py), [`msgpack-goldens.json`](../../packages/contracts/fixtures/studio/v1/msgpack-goldens.json) |

The codec lives at the HTTP edge. Script IR, revision rules, PostgreSQL and every domain module
see the same DTO values they always did and never import it.

## Scope

| Route under `/api/v1/studio` (also the deprecated `/api/v1` alias) | Request body | Response body |
| --- | --- | --- |
| `GET /scripts/{id}/head` | — | negotiated |
| `GET /scripts/{id}/revisions/{revision}` | — | negotiated |
| `POST /scripts/{id}/revisions` | negotiated | negotiated |
| `POST /validation` | negotiated | negotiated |

Every other route is JSON only and unchanged: session, history, reviews, sources, imports and
original downloads, adaptations and production. On those routes a MessagePack body is refused as
`400 invalid_request`, exactly as any non-JSON body was before, and `Accept` is ignored. Cantos
Theatre has no listener API yet and the CMP app has no client; neither negotiates anything.

## Media types

| Representation | Media type | Notes |
| --- | --- | --- |
| JSON | `application/json` | default; `application/*+json` is read as JSON |
| MessagePack | `application/msgpack` | the only spelling read or written |

`application/x-msgpack`, `application/vnd.msgpack` and `+msgpack` suffixes are refused until a
real consumer needs one. Parameters (`charset=…`) and ASCII case are ignored.

## Negotiation

### Request body: `Content-Type`

| `Content-Type` | Decoder |
| --- | --- |
| absent, empty or not ASCII | JSON, through the existing extractor: an invalid body is still `400 invalid_request` |
| `application/json`, `application/*+json` | JSON |
| `application/msgpack` | MessagePack |
| anything else | `415`, `invalid_request` with issue `{path: "content-type", rule: "unsupported_media_type"}` |

### Response body: `Accept`

JSON is chosen unless MessagePack is **strictly preferred**:

| `Accept` | Representation |
| --- | --- |
| absent, or no parseable media range | JSON |
| `*/*`, `application/*`, `application/json` | JSON |
| `application/msgpack` with a higher quality than JSON | MessagePack |
| JSON and MessagePack at equal quality (`application/msgpack, application/json`) | JSON |
| media ranges present, none allowing JSON or MessagePack (including `q=0`) | `406`, `invalid_request` with issue `{path: "accept", rule: "not_acceptable"}` |

Each representation's quality comes from the most specific matching range (RFC 9110 § 12.5.1):
`application/json;q=0, */*` refuses JSON and allows MessagePack. A range with an invalid quality
value is ignored. Repeated `Accept` fields form one list. A browser's default `Accept` and every
existing client keep receiving JSON.

### Order of decisions

```text
Origin (403, mutations) → session (401) → Accept (406) → Content-Type (415) → body (400) → handler
```

Negotiation finishes before the handler calls the store. A refused representation therefore never
follows a committed write, and a `406` on a save is a definitive "nothing happened".

Successful negotiated responses and `406` carry `Vary: Accept`. No entity tags exist; every API
response stays `Cache-Control: no-store` and `X-Content-Type-Options: nosniff`.

## Encoding rules

- **Named maps.** A DTO is a MessagePack map keyed by its serde field names, in contract field
  order on output, any order on input. Never positional arrays: a top-level array is refused as
  the mark of a positional encoder.
- **Same DTO semantics.** `deny_unknown_fields` refuses unknown keys in both representations;
  missing, duplicate and mistyped fields are refused; an absent optional key and an explicit null
  both mean `None`, and output always writes the key with `nil`/`null`. Unit enum variants are
  their snake_case strings; internally tagged enums keep their tag field.
- **Scalars.** Integers use the smallest MessagePack format; strings are raw UTF-8, never
  normalized or escaped; invalid UTF-8 is refused rather than replaced. Contract DTOs use no
  float, binary or extension values, and no type whose serde form depends on
  `is_human_readable` (MessagePack is not human readable, JSON is).
- **Script IR stays a string.** `script_json` carries the exact JSON text. Canonical bytes and
  the `sir-c1`/`sir-e1` digests are computed over that text by the unchanged Script IR reader;
  MessagePack only stops JSON from escaping it a second time.

[`msgpack-goldens.json`](../../packages/contracts/fixtures/studio/v1/msgpack-goldens.json) pins the
bytes for each Studio fixture. They are written by an independent Python encoder and must equal
`rmp-serde`'s output byte for byte; see the [contracts README](../../packages/contracts/README.md).

## Limits

| Limit | Value | Applies to |
| --- | --- | --- |
| Request envelope | 8 MiB (`DefaultBodyLimit`), read before decoding | both |
| Script IR text | 2 MiB, by the Script IR reader after decoding | both |
| MessagePack nesting | fewer than 16 arrays/maps (`MAX_MSGPACK_DEPTH`) | MessagePack |
| Length prefixes | checked against the remaining input before use; serde caps preallocation at 1 MiB | MessagePack |
| Values per body | exactly one; trailing bytes are refused | both |
| Request deadline | 15 s, unchanged middleware | both |

The 8 MiB envelope exists for JSON's escaping overhead; MessagePack bodies are smaller for the same
text and need no separate limit.

## Errors

Every failure is the existing JSON `ApiError {code, current_revision, issues}` with an existing
`ErrorCode`, whatever the request's `Content-Type` or `Accept`. No error code was added; old
clients decode every response they could decode before. A malformed MessagePack body is the same
`400 invalid_request` as malformed JSON. If MessagePack encoding of a successful result ever
failed, the response is `503 unavailable`: the write may have committed, so the outcome must stay
ambiguous for the client to reconcile, never a definitive failure.

## Guarantees that do not change

- Session cookie, Origin check and authorization run before negotiation and decide identically
  for both representations.
- The idempotency key is `(script, actor, operation_id)` over the expected base and canonical
  export. The representation is not part of it: a retry of a lost response replays the original
  revision whichever representation it uses, and a changed snapshot is still `operation_reused`.
- Stale saves, immutable revisions, c1/e1 and error semantics are untouched.
- `application/msgpack`, like `application/json`, is not a CORS-safelisted type, so a cross-site
  form cannot send it; the API remains same-origin only.

## Client rules

The Studio declares one constant, `REVISION_CODEC`, for the four routes:

| Codec | Request headers | Body |
| --- | --- | --- |
| JSON (shipped) | `Content-Type: application/json`, `Accept: application/json` | the bytes `serde_json` produced before this change |
| MessagePack | `Content-Type: application/msgpack`, `Accept: application/msgpack, application/json;q=0.5` | named map |

- A response is decoded by **its own** `Content-Type`: errors are JSON, and a server or proxy may
  answer JSON to a MessagePack request. A JSON build never decodes MessagePack.
- A success whose body cannot be decoded is `unavailable`, the editor's existing ambiguous
  outcome: it keeps the actor, operation, base and snapshot and reconciles that operation.
- A request is sent once. No failure is retried in the other representation, because the first
  request may have committed; only the constant changes the codec.

## Rollback

The server serves both representations permanently, so rollback is client side:
`REVISION_CODEC = WireFormat::Json` (the current value), rebuild and redeploy the Studio. To remove
MessagePack from the server as well, route the four handlers back to `Json` and delete
`http/wire.rs`; no stored data depends on it.

## Extending to more routes, Theatre and CMP

A new route, surface or client adopts MessagePack only through the procedure in the
[wire codec reference](../../.agents/skills/cantos-engineering/references/wire-codec.md#extending-to-theatre-and-cmp):
a measured benefit on its own payloads, the full test matrix, goldens for its DTOs and a
per-client switch whose default stays JSON until that evidence exists. Listener routes stay JSON
until then. A Kotlin client must decode by response `Content-Type`, write named maps and pass the
committed goldens before it sends `Accept: application/msgpack`; no Kotlin binding exists today.
