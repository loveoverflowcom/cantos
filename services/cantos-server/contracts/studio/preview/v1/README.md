# Cantos Studio preview v1

A creator sees the Theatre presentation text of **one exact saved Work revision** before anything
is published ([ADR 0054](../../../../../../docs/decisions/0054-publication-generation-and-access.md)
D03: Studio-origin preview; D01b: owner and current Catalog reader/editor members). The route is
a read of the Catalog head under the existing Catalog authority. It is not a publication, grants no
listener access and never reads or writes a generation, audience policy, receipt, search row,
listening history, progress or bookmark.

| Method and path below `/api/v1/studio` | Request | Success |
| --- | --- | --- |
| `GET /works/{work_id}/preview?work_revision=R[&limit=N][&cursor=C]` | query below | `StudioPreview` |

Raw Rust DTOs live in [`contract/studio/preview.rs`](../../../../src/contract/studio/preview.rs),
the shape in [`preview.schema.json`](preview.schema.json) and literal examples in
[`fixtures/`](fixtures). JSON only; the Catalog `Accept`/`Content-Type` rules apply.

## Exact revision

`work_revision` is required. The preview shows the Work at that revision and its **active** Units
at their saved head revisions in Catalog order, each with its exact revision. Every Unit change
advances the Work revision, so "Work revision R is the saved head" identifies the whole saved
state. Catalog does not record which Units belonged to an older Work revision, so a revision that
is not the head is **not recoverable**: it answers `409 stale_revision` with `current_revision`,
never the newer head under the old banner and never a public or older copy. Studio therefore
previews after a successful save, or asks the creator to reload the newer saved revision.

An archived Work answers `409 catalog_conflict` (`lifecycle: archived`); archived Units are not
shown. `limit` is 1–50 (default 20). `ordinal` is the one-based rank among active Units across
pages, not the Catalog position.

## Narrow DTO

`PreviewWork` is `{id, kind, revision, title, synopsis, language}` and `PreviewUnit` is
`{id, kind, revision, ordinal, title, description}`. Owner, `visibility`, rights references,
source/Script IR/content-digest links, lifecycle, positions, digests and timestamps are absent.
The same shape as Theatre text does not make this the Studio DTO: a new Catalog field is not
exposed until a version of this contract names it.

v1 carries **no cover and no audio**. Studio shows both as unavailable and never substitutes a
public cover or release. The owner's separate private artwork and audio previews keep their own
exact bindings; member (reader/editor) cover and audio preview are not implemented here.

## Pagination

`next_cursor` is opaque, bound to actor, Work, revision and the Catalog epoch like the Catalog
page cursors. Any committed Catalog write, including a permission change, answers a continuation
with `409 stale_cursor`; restart without a cursor (which then answers `stale_revision` if the head
moved). A malformed or differently scoped cursor is `400 invalid_request`.

## Authority and errors

| Status | Code | Meaning |
| --- | --- | --- |
| `200` | — | the preview |
| `400` | `invalid_request` | unknown/duplicate query field, `work_revision` missing, zero or beyond 2^63−1, bad `limit`/`cursor` |
| `401` | `unauthenticated` | missing, invalid, inactive or expired Studio session |
| `404` | `not_found` | absent Work **or** no owner/member relationship; indistinguishable |
| `409` | `stale_revision` | `work_revision` is not the saved head; `current_revision` is populated |
| `409` | `stale_cursor` | the Catalog changed during a walk |
| `409` | `catalog_conflict` | the Work is archived |
| `422` | `invalid_catalog` | malformed Work ID |
| `503` | `unavailable` | storage/authority unavailable; never an empty preview |

Authentication and membership are checked inside the read transaction that reads the text, and
again before commit. A Studio session cannot be replayed on Theatre: the route exists only under
`/api/v1/studio`, and Theatre credentials never reach it.

## Headers

Every response carries `Cache-Control: no-store` and `X-Content-Type-Options: nosniff` (Studio
policy) and `Referrer-Policy: no-referrer`. The response sets no cookie and emits no OpenGraph,
sitemap, autocomplete, analytics or image URL; the route has no server-rendered HTML.

## Evidence boundary

Fixtures and the schema are checked against the Rust DTOs by the contract conformance suite.
File-backed Turso router tests cover authority, exact revision, side-effect absence and headers.
Studio Web (the Save-then-preview flow, banner, session fencing) consumes its own representation.
Browser, deployed and production behavior are separate acceptance.
