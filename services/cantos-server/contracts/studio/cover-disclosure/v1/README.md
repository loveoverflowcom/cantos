# Studio cover disclosure v1

The owner's public-display decision for one exact admitted Work image, as accepted in
[ADR 0033](../../../../../../docs/decisions/0033-release-pinned-public-cover.md) rule 2. It is a
sibling of [private Work artwork v1](../../artwork/v1/README.md) and is mounted with those routes
(only when private media storage is configured). A decision is separate from private admission,
Work visibility, distribution clearance and audio publication approval: none of them implies it,
and it implies none of them. It changes no release, no Work revision and no private artwork.

## Route

`POST /api/v1/studio/works/{work}/cover-disclosures`: `RecordCoverDisclosureRequest` →
`CoverDisclosureResponse` (200). JSON only: the request must be `application/json`, and an
`Accept` that selects anything but JSON is `406`. No Protobuf message or field number is
selected. The response is `no-store`. There is no read or list route yet; Studio UX copy and a
"current decisions" view are open work alongside #171.

| Field | Request | Response |
| --- | --- | --- |
| `operation_id` | Caller-minted UUID; the replay key per owner | — |
| `work_id` | — (path) | The Work |
| `sequence` | — | Server order per Work, from 1; the latest per Work and image digest is current |
| `image_asset_digest` | `ia-r1:sha256:…` of an admitted image of this Work | Same |
| `decision` | `cleared` or `revoked` | Same |
| `valid_until_ms` | Required: exclusive expiry for `cleared` (1..2^53-1), explicit `null` for `revoked` | Same |
| `recorded_at_ms` | — | Server time of the recording |

The [JSON Schema](cover-disclosure.schema.json) and [fixtures](fixtures/) are checked against
the Server's [Rust DTOs](../../../../src/contract/studio/cover_disclosure.rs) by
`tests/contract_conformance/cover_disclosure_wire.rs`.

## Rules the Server enforces

- Only the authenticated current active Work owner records a decision. Delegates, producers,
  Admin and Theatre credentials have no route here; another actor receives the Catalog
  `not_found`.
- The digest must name an admitted image of exactly this Work and owner (`422`
  `image_asset_digest`/`unknown_image`); a database trigger independently refuses any other row.
- A clearance's `valid_until_ms` must be strictly after the recording time (`422`
  `valid_until_ms`/`invalid_disclosure_window`). Expiry is half-open. Renewal and re-clearance
  are new decisions; a clearance is never perpetual by default.
- Decisions are append-only: no update or delete exists, and the table refuses both. The latest
  decision by server `sequence` for the Work and digest wins; caller timestamps never order them.
- Replaying the same `operation_id` with the same request returns the originally recorded
  decision unchanged, even after its window closed. The same `operation_id` with a different
  request (or another Work) is `409 operation_reused`.

Showing a cover still requires the image to be the one pinned by the representative eligible
release ([Discovery v1.2](../../../theatre/discovery/v1/README.md#public-cover-v12-composition-gated));
a decision about any other image is never consent.

## Evidence boundary

File-backed `server.db` tests exercise the production service, the SQL barriers and this route
through the production router with a development Studio session (`src/test_support/cover_turso_tests.rs`).
No browser, deployed host or Studio UI observation exists.
