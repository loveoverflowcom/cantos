# Cantos Studio Catalog API v1

The Studio catalog manages immutable Work and ContentUnit identity, revisioned metadata,
ordering and soft archive. Server-owned raw Rust DTOs live in
[`contract/studio/catalog.rs`](../../../../src/contract/studio/catalog.rs), shape and supported values in
[`schema/catalog/v1.schema.json`](catalog.schema.json), and literal acceptance and
rejection examples in [`fixtures/catalog/v1`](fixtures/README.md).
This is the backend slice of [#66](https://github.com/loveoverflowcom/cantos/issues/66);
Studio Web catalog UX has a separate acceptance boundary.

Catalog visibility is metadata. `private`, `unlisted` and `public` all remain authenticated
Studio data; none publishes an audio release or grants anonymous access. Publication, external
audio handoff, producer scopes, Theatre routes and Studio Mobile are outside this contract.
Saving any Catalog revision never publishes it: an owner pins exact saved revisions into a
metadata generation through [metadata publication v1](../../metadata-publication/v1/README.md),
whose public gate is that generation, not `visibility` (ADR 0054 D05b).
See [ADR 0015](../../../../../../docs/decisions/0015-api-surface-boundaries.md) and
[ADR 0018](../../../../../../docs/decisions/0018-external-audio-production.md).

## Routes and representation

Only `/api/v1/studio` mounts these routes. The deprecated unprefixed Studio alias, reserved
Theatre router and reserved Admin router gain no Catalog endpoints. Route templates are named
in [`api/src/routes.rs`](../../../../src/contract/routes.rs).

| Method and path below `/api/v1/studio` | Request | Success |
| --- | --- | --- |
| `POST /works` | `CreateWorkRequest` | `CatalogMutationResponse` |
| `GET /works` | bounded query below | `CatalogWorkPage` |
| `GET /works/{work_id}` | — | `CatalogWork` |
| `PATCH /works/{work_id}` | `PatchWorkRequest` | `CatalogMutationResponse` |
| `DELETE /works/{work_id}` | `ArchiveWorkRequest` JSON body | `CatalogMutationResponse` |
| `GET /works/{work_id}/history` | bounded history query | `CatalogWorkPage` |
| `POST /works/{work_id}/content-units` | `CreateContentUnitRequest` | `CatalogMutationResponse` |
| `GET /works/{work_id}/content-units` | bounded query below | `CatalogContentUnitPage` |
| `POST /works/{work_id}/content-units/reorder` | `ReorderContentUnitsRequest` | `CatalogMutationResponse` |
| `GET /content-units/{unit_id}` | — | `CatalogContentUnit` |
| `PATCH /content-units/{unit_id}` | `PatchContentUnitRequest` | `CatalogMutationResponse` |
| `DELETE /content-units/{unit_id}` | `ArchiveContentUnitRequest` JSON body | `CatalogMutationResponse` |

Catalog supports JSON only: requests carrying a body use `Content-Type: application/json`;
successful responses and `ApiError` errors use `application/json`. These endpoints do not
advertise MessagePack. The optional MessagePack support of existing editorial endpoints does
not apply automatically to Catalog. Unsupported representation headers are rejected before
effects: a MessagePack body receives `415`, and an Accept value with no acceptable JSON
representation receives `406`; both use `invalid_request`. If JSON has positive quality it
remains acceptable even when MessagePack is preferred. Bodies exceeding 128 KiB receive
`413` with `invalid_request`. Errors keep the shared JSON envelope. Responses contain no private source body or
Script IR body and receive the existing `no-store` and `nosniff` policy.

## Identity and revisions

Work kinds are fixed as `novel | story | podcast`; novel and story contain `chapter`, while
podcast contains `episode`. Work and unit IDs are stable canonical lowercase UUIDs supplied on
creation; titles never identify records. The server derives Work ownership from the Studio
session. Creation and edits reject client ownership, lifecycle, revision, timestamps and other
unknown fields. Units inherit ownership through their immutable `work_id` parent. Creating a
story or podcast requires no cast, scene, manuscript or Script IR.

Lifecycle is `draft | archived`. `DELETE` appends an archived snapshot; it never destroys the
Work, its units, original sources or history. Archived unit positions remain tombstones.
The API has no restore endpoint. Successful mutation replies preserve the resulting immutable
`work` and affected `units`; reorder replies contain an empty `units` list, so callers read the
bounded unit page to obtain the final order.

Revisions start at 1 and stay within signed 64-bit range. Work edits, archive and
reorder carry `expected_revision` for the Work. Unit creation carries
`expected_work_revision`; a unit edit/archive carries both its `expected_revision` and the
parent's `expected_work_revision`. Every child mutation appends a Work revision in the same
transaction. Work history returns complete immutable Work metadata snapshots, including Work
revision advances from child changes. It does not include historical unit bodies, mutation
receipts or an audit actor record. Timestamps are nonnegative integer Unix milliseconds from
the transaction clock; `updated_at` never precedes `created_at`.

## Creation and partial update

Work metadata is `details: {title, synopsis, language, visibility, rights_refs}`. Title is
nonblank and at most 512 UTF-8 bytes. Synopsis permits line breaks and is at most 16,384 bytes.
Language is a bounded common BCP-47 spelling, at most 63 bytes. Rights references are at most
64 distinct `[a-z0-9_-]` identifiers of 1–64 characters. Those references are declarations,
not verified rights or approval facts. Unit metadata is
`details: {title, description, source?, script?, content_digest?}` with the same title bound
and the same 16,384-byte multiline description bound. Control characters are refused, except
newline, carriage return and tab in multiline fields. JSON Schema character limits do not
replace the server's UTF-8 byte checks.

PATCH uses a nonempty `details` object:

| Input for one field | Meaning |
| --- | --- |
| field omitted | preserve its current value |
| field supplied with a value | replace that field |
| `source`, `script` or `content_digest` supplied as `null` | explicitly clear that optional binding |
| required metadata supplied as `null` | `invalid_catalog` |

An empty synopsis or description is an explicit replacement with empty text. An empty
`rights_refs` list clears declarations; `null` does not. Identity, kind, ownership, lifecycle,
position and timestamps are not patchable. Independent semantic field problems accumulate
under the shared `issues: [{path, rule}]` envelope; syntactically malformed JSON or wrong DTO
shape is `invalid_request` and stops semantic validation.

## Immutable links and authorization

These optional links are compatibility/navigation metadata. The separate
[Content Identity API](../../content/v1/README.md) owns managed immutable content revisions; editing or
clearing Catalog links never moves its content head. Production briefs must pin that exact
ContentRevision rather than these fields. Existing units acquire no content by migration.

Optional source is `{id, sha256}` pointing at an existing preserved manuscript source. Optional
Script IR is `{id, revision, export_digest}` pointing at an exact accepted revision, using
`sir-e1:sha256:<64 lowercase hex>`; a latest pointer is never accepted. The transaction resolves
the records, checks ownership against the Work owner and compares their authoritative checksums.
A script member cannot use membership to attach another owner's script to a Work. These links
preserve the existing source/Script IR historical identities and bytes.

`content_digest` is either a raw lowercase SHA-256 or a `sir-c1:sha256:` digest. With a Script IR
link it must match that revision's content digest; with only a source link it must match source
SHA-256. Without a link it is a caller declaration, without a server byte-integrity assertion.
Changing or clearing links requires explicit PATCH values; omission preserves them. None of
these links proves rights clearance, listened quality or publication readiness.

Every operation requires a current Studio session. The owner can read and mutate. Current
reader membership permits reads and denies writes. Current editor membership permits metadata,
child mutation and reorder; Work or unit archive and rights-reference/visibility changes require
owner. Cross-owner strangers receive `404 not_found`, and session/authorization is checked
inside the same transaction as reads/writes after lock waits and before commit. Retry receipts
never bypass current session or permissions. Membership itself has no Catalog HTTP endpoint
in this slice; tests and the existing application command can establish membership facts.

## Deterministic pages and cursor staleness

Work and unit list queries accept `limit` from 1 to 50, `include_archived` as a boolean, and
optional `cursor`. History accepts `limit` and `cursor` only. The server default limit is
20, and `include_archived` defaults to false. Responses are `{items, next_cursor}` with `null` for the last
page; there is no unbounded list or total count. Work order is stable UUID order, unit order is
ascending `(position, id)`, and history order is ascending revision. A Work retains at most
1,000 units, including archived tombstones.

Treat cursor text as opaque. It is a hexadecimal cursor of at most 2,048 characters encoding internal JSON,
scoped to actor, query, parent and archive filter plus the Catalog epoch. A different actor,
query, parent or filter cannot reuse it. Any committed Catalog mutation, even for an unrelated
Work/owner, invalidates outstanding cursors with `409 stale_cursor`; restart the query without
a cursor. Malformed, oversized or wrongly scoped cursors receive `400 invalid_request`.
The Catalog clock deliberately serializes mutations and stabilizes current permissions/pages;
this global contention and unrelated-query invalidation are initial-scale limits, without a
production throughput or latency claim. The encoding is not a token to inspect or construct.

## Retries, conflicts and atomic reorder

Every mutation carries a stable canonical UUID `operation_id`, scoped Catalog-wide to the
authenticated actor. Keep the same ID and exact submitted command when retrying after a lost
response. The receipt commits atomically with snapshots and the Catalog epoch. An exact retry
returns the originally accepted immutable `CatalogMutationResponse`, including its original
revisions/timestamps; it never appends another revision. Reuse with another command, target,
expected revision or payload yields `409 operation_reused`. Current authorization must still
permit the operation on every replay. PATCH identity uses submitted fields including
absence/null/value, so retries do not depend on later current metadata.

Stale expected revisions return `409 stale_revision` with the authoritative conflicting
`current_revision`. Duplicate entity IDs and archived edit targets use `409 catalog_conflict`.
Invalid parent-kind bindings, invalid reorder sets and the retained-unit capacity limit use
`422 invalid_catalog`.
Reorder supplies every currently active unit ID exactly once in `ordered`; duplicate, missing,
archived, unknown or foreign IDs are refused atomically. Active units permute occupied active
slots while archived tombstone positions remain fixed. A concurrent edit/archive/reorder must
present the current Work revision; a failing operation changes neither positions nor history.

## Shared errors

Errors always use the shared `ApiError`:

```json
{"code":"stale_revision","current_revision":4,"issues":[]}
```

| HTTP status | Code | Meaning |
| --- | --- | --- |
| `400` | `invalid_request` | malformed JSON, wrong DTO shape, or invalid query/cursor syntax |
| `401` | `unauthenticated` | missing, invalid, inactive or expired Studio session |
| `403` | `forbidden` | known member lacks the action, or Origin refusal |
| `404` | `not_found` | absent or inaccessible Catalog resource |
| `409` | `stale_revision` | expected revision differs; `current_revision` is populated |
| `409` | `operation_reused` | actor's operation ID was previously committed with different input |
| `409` | `stale_cursor` | Catalog epoch changed; restart page traversal |
| `409` | `catalog_conflict` | duplicate identity, archived mutation or exhausted revision range |
| `413` | `invalid_request` | body exceeds 128 KiB |
| `415` | `invalid_request` | body Content-Type does not select JSON |
| `406` | `invalid_request` | Accept excludes JSON |
| `422` | `invalid_catalog` | IDs/operation UUIDs/revisions/metadata/link/kind/reorder semantic validation failed |
| `422` | `evidence_unavailable` | linked source/script/rights fact unavailable, wrong owner or checksum mismatch |
| `500` | `corrupt_revision` | persisted evidence fails its integrity admission |
| `503` | `unavailable` | storage/authority unavailable; no SQL or internal error text is returned |

The HTTP adapter and tests own exact status mapping; this document tracks that mapping.
The existing error codes for editorial integrity are retained when linked evidence is corrupt or
unavailable. Shared root enum additions do not create endpoints in other surface groups.

## Verification boundary

Run the canonical repository gates with Rust 1.98.1: `cargo xtask verify repository`,
`cargo xtask verify rust`, `cargo xtask verify contracts`, `cargo xtask verify turso`, and
`cargo xtask preflight --changed-only`. Shared consumer tests establish raw wire shapes and
PATCH representation; HTTP tests establish routes, error/status/schema conformance and guards;
native file-backed/restart suites exercise transaction, retry and immutable history
behavior ([mapping](../../../../../../docs/architecture/server-turso/remaining-pg-retirement.md)). A DTO or fixture alone does not establish an implemented HTTP endpoint, a browser UX,
production rights, publication or external audio intake. Report commands actually run and all
remaining gates accurately.
