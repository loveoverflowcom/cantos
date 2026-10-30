# Studio metadata publication v1

Implemented Server core, routes and file-backed Turso persistence (`server.db` v31) for
[#648](https://github.com/loveoverflowcom/cantos/issues/648) (S02 of
[#634](https://github.com/loveoverflowcom/cantos/issues/634)), applying the accepted
[ADR 0054](../../../../../../docs/decisions/0054-publication-generation-and-access.md) rows D01a,
D02a–c, D05a–b and D07a–b. **No Theatre route reads generations yet**: Discovery keeps its current
release-driven shapes until S05 replaces them together with every current client (the
[synchronized-delivery rule](../../../../../../docs/work-plan/publication-access-handoff.md#wire-and-data-transition-d07)).
No Studio Web client calls these routes yet (S05). Browser, device and deployed acceptance are
separate.

## Operations

All JSON beneath `/api/v1/studio`, with the Studio session cookie and the surface's existing
Origin/CSRF admission. **Current owner only** (D01a): a reader/editor member, another actor or an
unknown Work all receive `404 not_found`; a missing session receives `401`.

| Method | Relative path | Request | Response |
| --- | --- | --- | --- |
| GET | `/works/{work_id}/metadata-publication` | — | `MetadataPublicationState` |
| POST | `/works/{work_id}/metadata-publication` | `MetadataPublicationRequest` | `MetadataPublicationState` as settled by that operation |

[Strict JSON Schema](metadata-publication.schema.json), the
[Server's Rust DTOs](../../../../src/contract/studio/metadata_publication.rs) and the
[fixtures](fixtures/) are versioned together and round-trip in
`tests/contract_conformance/metadata_publication_wire.rs`.

## Model

Catalog keeps the only editing head. Saving a draft (title, synopsis, visibility, rights
references, Unit text, reorder, a new or archived Unit) never touches publication. A
**generation** is an immutable full copy (D02a): one exact saved Work revision and an ordered,
duplicate-free list of at most 1,000 exact saved Unit revisions, each pinned with the SHA-256 of
its stored snapshot bytes. The one-based published position is the entry's index in the command;
the server never reads `catalog_units.position` or an unselected head (D02b). One **pointer** per
Work carries `version` (compare-and-set), `lifecycle` and the generation it names. The first
ACTIVE generation sets `first_visible_at_ms`; later generations and withdrawals never move it
(D02c).

| `lifecycle` | Meaning |
| --- | --- |
| `never_published` | No pointer exists. `version` is `0`; `generation` and `first_visible_at_ms` are `null` |
| `active` | The pointer names the generation the public projection reads |
| `withdrawn` | The Work is hidden; the pointer keeps naming its last generation and all history stays |

A failed or unread state is an error response, never `never_published`.

## Commands

Every command carries an `operation_id` (canonical lower-case UUID) and the `expected_version`
the caller read (`0` before the first publication).

| `action` | Effect | Requires |
| --- | --- | --- |
| `publish` | Generation N+1 from `work_revision` and the ordered `units` (`{unit_id, revision}`), ACTIVE. Also republishes after a withdrawal and reorders | `confirm_metadata_rights: true`; the Work revision cites at least one rights reference and each still resolves to the owner's rights evidence (the audio-release precedent); neither the revision nor the current Work is archived; each Unit belongs to this Work and neither its revision nor its current state is archived |
| `publish_unit` | Generation N+1 that adds or replaces one Unit at one-based `position` (1 … entries+1 after removing that Unit) and copies every other entry and the Work pin | ACTIVE pointer; `confirm_metadata_rights: true`; the active Work pin still passes the archive and rights gates; the Unit rules above |
| `withdraw_unit` | Generation N+1 without that Unit; the Work stays visible, possibly with zero Units | ACTIVE pointer; the Unit is published |
| `withdraw` | Pointer turns `withdrawn`; no generation is written | ACTIVE pointer; possible even when the Work is archived |

`confirm_metadata_rights` is the owner's explicit claim that this metadata may be shown publicly.
It is not legal clearance, and metadata publication never grants audio, cover or transcript
rights (ADR 0054 principle 5). A `publish`/`publish_unit` whose result equals the active
generation is refused as `unchanged` instead of minting a version.

The settling write unit authenticates the owner, then, under the single writer: looks up the
operation; compares `expected_version`; resolves and admits every selected exact revision; writes
the generation and its entries, one immutable event (the operation receipt: actor, action,
canonical payload SHA-256, settled version and generation) and the pointer; and commits. No RPC
or object I/O runs inside it.

| Situation | Outcome |
| --- | --- |
| Same `operation_id` and canonical payload | `200` with the state that operation settled (its version and generation), even after later versions |
| Same `operation_id`, different payload or another Work | `409 operation_reused`; nothing written |
| Lost response or timeout | Unknown outcome (`503 unavailable`); retry the same operation to reconcile, never a new ID |
| `expected_version` ≠ current | `409 stale_revision`; `current_revision` carries the current **pointer version** on this route; no selected revision is read |
| Two publishers read the same version | One settles; the other receives `409 stale_revision` with the new version |

## Errors

The existing Studio `ApiError` shape and codes; `issues` name the path and rule.

| HTTP | `code` | `issues` (`path` / `rule`) |
| --- | --- | --- |
| 400 | `invalid_request` | Malformed JSON, unknown field or action, invalid `operation_id`, `confirm_metadata_rights: false` |
| 404 | `not_found` | Unknown Work, not the current owner |
| 409 | `stale_revision` | — (`current_revision` = current pointer version) |
| 409 | `operation_reused` | — |
| 409 | `catalog_conflict` | `lifecycle`/`not_active`, `lifecycle`/`archived`, `unit_id`/`not_published`, `publication`/`unchanged`, `version`/`version_limit` |
| 422 | `invalid_catalog` | `units`/`limit_exceeded`, `units`/`duplicate_unit`, `units`/`unknown_unit` (also a Unit of any other Work), `work_revision`/`unknown_revision`, `position`/`out_of_range` |
| 422 | `evidence_unavailable` | `details.rights_refs`/`unavailable` |
| 500 | `corrupt_revision` | A stored generation, pin or pointer contradicts its invariants |

## Public projection (Server-internal until S05)

`contexts/catalog/application/metadata_publication_read.rs` is the read facade S05 composes into
the single Discovery representation. Within one caller-owned read snapshot it applies the v1
PUBLIC gates (D05a): ACTIVE pointer, active owner, current Work not archived; a Unit additionally
needs generation membership and a current Unit that is not archived. Catalog `visibility` is
editorial metadata and **not** a gate (D05b): saving a private r4 leaves the published r3 shown.
Eligibility filters before count, ordinal and pagination:

- Works: newest `first_visible_at_ms` first, then ascending Work ID, as a keyset; optional
  `kind`, editorial `language` (lower-cased) and a search over the pinned title and synopsis with
  Discovery's NFC/lower-case normalization.
- Detail: the list's exact gates; absent and hidden are indistinguishable.
- Units: published order, a consecutive one-based `ordinal` among visible Units and the published
  position as keyset. A continuation is bound to the generation it started on; a newer active
  generation restarts the walk.

Every row is rechecked against its pinned snapshot bytes. Works with audio releases, the prepared
sample's disclosures and public-visibility Works without a generation are absent from it. Exact
audio association (`playback`) is S03; RESTRICTED audiences are S06.

## Not covered

Theatre Discovery adoption and removal of the default/`presentation=1–3` shapes (S05), the
Studio Web publish workflow (S05), audio association on activation (S03), preview (S04),
RESTRICTED audiences, grants and groups (S06–S10), the retirement of the sample-only disclosure
lane (S03) and performance budgets (S11). Production migration and activation are separate tasks.
