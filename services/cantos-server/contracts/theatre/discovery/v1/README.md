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

The canonical [metadata schema](metadata.schema.json) is versioned with the Server's
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

Covers use the artwork effective at the generation's exact Work revision and the existing
independent, current per-image public-display clearance. Metadata publication grants no image
rights. The shown SHA URL is reprojected on every GET/HEAD and again after private object open;
a private replacement never changes it. Runtime output is AVIF or null; historical PNG/JPEG
wire enum fixtures establish no runtime fallback. Cover responses retain `public, no-cache`,
ETag, no ranges and `nosniff`; denied/error responses are no-store.

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
| `GET/HEAD /works/{work}/cover/{sha256}` | Only the exact currently shown cover bytes |

[Playback schema](discovery.schema.json) still owns `PublicationManifest`,
`PlaybackAuthorization`, `ListenerError` and image facts. Earlier release-summary definitions
and fixtures are retained as historical strict-rejection oracles, not served representations.
Origin playback keeps the [ADR 0032](../../../../../../docs/decisions/0032-theatre-origin-playback-delivery.md)
grant, Range and no-store rules; an uncomposed delivery host stays unavailable. Real-file/router
checks, real private bytes, browser, Android/iOS and deployed delivery remain separate evidence.
RESTRICTED audiences and subject/session media admission belong to S06–S09, not these public reads.
