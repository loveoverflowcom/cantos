# Manuscript import API v1

Implemented private development boundary: [Axum](../../services/cantos-server/src/adapters/inbound/http/studio/imports.rs) →
[the existing PostgreSQL source registry](../../services/cantos-server/src/contexts/ingestion/postgres.rs), consumed by
Cantos Studio. The [JSON Schema](schema/manuscript/v1.schema.json) and
[shared raw Rust DTOs](api/src/imports.rs) are hand-authored. Independent literal wire examples
live in the [consumer tests](api/tests/import_wire.rs) and
[backend contract tests](../../services/cantos-server/tests/import_contract.rs). They validate transport shape;
the backend remains authoritative for byte limits, extraction and access decisions. No generated
OpenAPI or Kotlin bindings are claimed.

## Routes and access

| Route under `/api/v1/studio` | Request / result |
| --- | --- |
| `POST /imports` | `ImportRequest` → immutable `ImportResponse`, HTTP 200 for a new receipt or exact replay |
| `GET /imports/{SOURCE_ID}` | The authenticated import owner's preserved receipt |
| `GET /imports/{SOURCE_ID}/original` | Exact original bytes as `application/octet-stream` with an inert attachment filename |

These routes use the existing cookie/session boundary, mutation Origin guard, no-store and
nosniff policy described in [Studio v1](studio-v1.md). The backend selects the owner from the
current active session. Clients cannot supply an owner or choose the source ID. Minted source
IDs are `src_` followed by 32 lowercase hexadecimal UUID digits. Missing, inaccessible and
other actors' sources return 404. A revoked or expired session cannot replay or reopen a source.
Sources are private; no listener/public route is introduced.

Import appends to `source_records` and its owner-scoped source evidence registry. It does not
advance a script head, accept a script revision, replace a source, record an editorial review or
grant publication rights. Existing accepted revisions and evidence links remain immutable.

## Request and preservation

`ImportRequest` has `metadata` and `original_bytes`. Bytes travel as an array of JSON integers
from 0 to 255; the server preserves exactly that sequence. `metadata` contains:

| Field | Meaning and backend bounds |
| --- | --- |
| `operation_id` | Required canonical lowercase hyphenated UUID; stable across retries |
| `file_name` | Required inert display value, 1–255 UTF-8 bytes; never a storage path or response header |
| `format` | Closed spelling: `txt`, `markdown`, `docx`, `script_ir` |
| `reference` | Required creator-supplied source reference, 1–2048 UTF-8 bytes |
| `rights_holder` | Nullable/omittable creator assertion, at most 2048 UTF-8 bytes |
| `permission_evidence` | Nullable/omittable supplied evidence/reference, at most 2048 UTF-8 bytes |
| `usage_scope` | Nullable/omittable declared usage scope, at most 2048 UTF-8 bytes |

Present display fields must have non-whitespace content and contain no control characters.
Rights fields are recorded as supplied; an absent assertion remains absent. Import never infers
publication permission from authorship, format, a filename or successful parsing. No supplied
URL/reference is fetched during intake.

Original data is limited to 1 MiB before extraction; the JSON envelope is limited to 5 MiB.
The Schema measures string lengths in Unicode scalar values. Server validation additionally
applies the stricter UTF-8 byte bounds above; a multibyte value can therefore fit the Schema
and still be rejected by the backend. Schema acceptance also cannot prove authorization,
non-whitespace content, checksum integrity or safe/valid document content.

## Receipt and extraction

`ImportResponse` has source `id`, server-selected `imported_by`, server UTC `recorded_at`
(microseconds), lowercase SHA-256 `sha256`, `byte_len`, unchanged `metadata`, `outcome` and
nullable `original_text`. The hash is SHA-256 over exact original bytes, with no prefix, decoding
or normalization. Nonempty UTF-8 originals containing no NUL are exposed unchanged in
`original_text`; empty/binary data and text containing NUL use the original-byte route. That
route is authoritative for exact round trips.

`outcome` is a tagged union:

| `status` | Required content |
| --- | --- |
| `parsed` | `extraction` with `extractor_version`, ordered `blocks`, `warnings` and nullable `script_json` |
| `failed` | `error` with stable `code` and nullable UTF-8 input byte `offset` when known |

A parse failure is a persisted HTTP 200 receipt, so the creator can compare, download or retry
opening the preserved source. It does not create a ready script. Reopening returns the original
stored outcome; no automatic reparse uses a newer extractor.

Each block has zero-based `index`, closed `kind` (`paragraph`, `dialogue`, `narration`,
`scene_cue`, `sound_cue`), `text`, nullable `speaker`, `scene` and `cue_kind`. Prose labels and
scene suggestions are lexical evidence requiring creator review. An unlabeled/ambiguous line
keeps its unknown speaker; a parser never invents a cast identity. Warnings contain a stable
`code` and nullable block index. Warning/error code vocabularies are strings rather than closed
wire enums; consumers show an unknown code safely and preserve the receipt.

For valid Script IR, `script_json` is the complete normalized export admitted by the existing
`0.1.0` validator. Speaker/scene values then come from validated identities; supplied sound cue
kinds are the closed spellings `ambience`, `music`, `sfx`. A structured import still does not
save/accept a revision or establish ownership/eligibility of its referenced rights records.
For prose, `script_json` is absent/null and extracted text is separate from original bytes.

Unknown object fields, enum values and union branches fail wire decoding. Nullable fields may
be omitted on input; serialization emits them explicitly. Optional values never imply a resolved
speaker, approved source or granted right.

## Retry and errors

The durable operation key is `(authenticated import owner, operation_id)`. An exact replay of
the same bytes and metadata returns the first receipt, including its ID, timestamp and extraction
failure. Reusing a key with any changed bytes or metadata returns `operation_reused`; identical
bytes under a fresh key create a distinct preserved source. A lost HTTP response does not prove
rollback. Retry the unchanged request with the same key, including after a server restart.
The receipt and source evidence commit in one transaction.

| HTTP | Error code | Meaning |
| --- | --- | --- |
| 400 | `invalid_request` | Malformed DTO, metadata/source ID, or decoded byte size above the limit; no receipt stored |
| 413 | `invalid_request` | JSON envelope exceeds the HTTP body limit; no receipt stored |
| 401 | `unauthenticated` | Missing/invalid/inactive/expired/revoked session |
| 403 | `forbidden` | Invalid mutation Origin |
| 404 | `not_found` | No import in the current owner's scope |
| 409 | `operation_reused` | Existing actor/key bound to another original/metadata snapshot |
| 503 | `unavailable` | Bounded connection/statement/lock/request failure or uncertain commit result |
| 500 | `corrupt_revision` | Stored source bytes, hash, metadata or receipt failed integrity admission |

Errors use the existing Studio [`ApiError` shape](schema/studio/v1.schema.json). For a decoded
request, HTTP 400 `invalid_request` collects every independent metadata/decoded-byte issue in
`issues: [{path, rule}]`. Paths are JSON Pointers such as `/metadata/file_name`,
`/metadata/reference`, `/metadata/operation_id`, `/metadata/rights_holder`,
`/metadata/permission_evidence`, `/metadata/usage_scope` and `/original_bytes`. Rule codes are
`nonblank`, `byte_length`, `control_character` and `canonical_uuid`; values are never echoed.
Wire syntax failures use the same error shape without claiming field-level extraction.
Diagnostics expose codes/offsets, never raw SQL, uploaded manuscript excerpts, internal paths
or provider output. TXT/Markdown encoding,
DOCX ZIP/XML safety and Script IR semantic admission are enforced by the bounded extractor;
they are not guaranteed by this wire Schema. DOCX content/macros are never executed, external
resources are never loaded, and the API introduces no adaptation/TTS provider call.

Production identity/TLS, rights eligibility, full scene editing, adaptation, TTS, public delivery
and mobile runtime acceptance remain separate work.
