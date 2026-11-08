# Theatre Discovery and playback v1

Anonymous Theatre uses one published-generation representation under `/api/v1/theatre`.
[ADR 0054](../../../../../../docs/decisions/0054-publication-generation-and-access.md)
D02/D06/D07 and [ADR 0055](../../../../../../docs/decisions/0055-development-phase-compatibility.md)
replace the former release-only/default and `presentation=1–3` lanes. Every presentation
selector now receives `400 invalid_request`; there is no alias, backfill or old-client lane.

## Metadata reads

| GET | Response / semantics |
| --- | --- |
| `/works`, `/search` | `MetadataWorkPage`; `q`, `kind`, `language`, `limit` (1–50, default 20), opaque `cursor` |
| `/works/{work}` | `MetadataWorkSummary`; the exact active generation's saved Work revision |
| `/works/{work}/units` | `MetadataUnitPage`; `limit`, `cursor`; exact generation membership and explicit published order |

The canonical [metadata schema](metadata-webp.schema.json) is versioned with the Server's
[DTOs](../../../../src/contract/theatre/discovery.rs) and [fixtures](fixtures/metadata/).
Each Web/CMP consumer owns its representation and checks it against these same schema/fixture
bytes. Objects are closed; nullable `cover`, `playback` and `next_cursor` keys are required.
A Work may contain zero published Units. Metadata-only Units have `playback: null`: no lock,
entitlement, manifest lookup or playable placeholder follows from that absence.

The reader composes Catalog's generation facade and Media's exact audio live gate in one read
snapshot. Only ACTIVE generations with an active owner and non-archived Work/Unit are visible.
Saved draft titles, synopsis, visibility, order and new Units never replace published pins.
Search/count/ordinal/keyset eligibility is applied before pagination. Unit continuations bind
to their Work, query/limit and generation; republishing invalidates a previous walk with
`400 invalid_cursor`. Hidden and absent direct IDs are indistinguishable `404 not_found`.
Sample attestations and existing audio without a generic generation grant no visibility.

Covers use the artwork effective at the active generation's exact Work revision and
independent current per-image clearance. A Work carries one nullable cover. Search and
lists return the `thumbnail` variant (up to 384 × 576 and 256 KiB), and detail returns `full`
(up to 1200 × 1200 and 2 MiB). The closed `variant` field must match the segment in the
exact HTTPS URL. Both variants are opaque lossy `image/webp` with their own dimensions,
length, digest and strong ETag. After the padded VP8 chunk, the sole trailing RIFF chunk
is `CTVR` with a little-endian size of two, version byte `1`, and ASCII `T` (thumbnail)
or `F` (full). The chunk carries no source or storage identity. The canonical active cover fixtures are
[webp_works.json](fixtures/metadata/webp_works.json) and
[webp_work.json](fixtures/metadata/webp_work.json). The earlier `covered_work*` metadata
snapshots and the earlier metadata schema preserve the pre-WebP wire for source-bound CMP migration testing.

GET and HEAD reproject the pinned source and verify the exact private rendition before
any successful response. A valid matching `If-None-Match` returns an empty 304 with the
same ETag and `public, no-cache`. A malformed or nonmatching validator receives 200 if
the cover is admitted. Revocation, expiry, withdrawal, wrong binding and missing or
damaged objects give anonymous `not_found` and `no-store`. No private source identity,
rights or storage fields enter the wire. Image ranges are unavailable.

## Exact audio and byte delivery

`playback` names the generation entry's exact associated `publication_id`, publication time
and duration. There is no latest-release fallback. Its live gates include the audio pointer,
withdrawal/revocation, current clearance/approval and the half-open rights window. Display-only
drift needs the owner's separate association confirmation; content/language/kind incompatibility
stays metadata-only. An association does not rewrite any release or manifest.

| Route | Contract |
| --- | --- |
| `GET /publications/{publication}` | Public `PublicationManifest`, only for the current exact generation association |
| `POST /publications/{publication}/playback-authorizations` | Empty body or `{}`; bounded authorization for that exact release |
| `GET/HEAD /media/{grant}` | Origin bytes, sealed grant validation and current generation/association recheck before and after object open |
| `GET/HEAD /works/{work}/cover/{variant}/{sha256}` | Only the exact currently shown cover bytes |

[Playback schema](discovery.schema.json) still owns `PublicationManifest`,
`PlaybackAuthorization`, `ListenerError` and image facts. Earlier release-summary definitions
and fixtures are retained as historical strict-rejection oracles, not served representations.
Origin playback keeps the [ADR 0032](../../../../../../docs/decisions/0032-theatre-origin-playback-delivery.md)
grant, Range and no-store rules; an uncomposed delivery host stays unavailable. Real-file/router
checks, real private bytes, browser, Android/iOS and deployed delivery remain separate evidence.
RESTRICTED audiences never enter these public reads, manifests or grants, even when a cookie
is present. The separate [shared module](../../shared/v1/README.md) owns authenticated
subject/session media admission; Web/CMP adoption remains in S08/S09.

S10 (#656) additionally hides every restricted Unit from anonymous reads before count, ordinal,
page selection and direct lookup. Both outward `position` and `ordinal` describe the consecutive
visible sequence, with no hidden-position gaps. Cursor scopes include current policy/group,
expiry and transactional archive/actor changes, alongside publication versions. Consumers must
discard a refused walk and re-read Work details/counts before fetching its Units; cached public
metadata cannot establish present access. Saved/Continue/History/bookmark projections omit
currently denied rows before page selection, while retaining account-owned records/receipts.
