# Theatre Discovery and Playback v1

Implemented backend core, **IO_ACCEPTANCE_PENDING**. All routes are JSON beneath
`/api/v1/theatre`; no Studio handler, session cookie or editor authority is reused.
Anonymous access is limited to current public releases. Production listener identity,
entitled access and retained-release/history access are separate dependencies.

## Operations

| Method | Relative path | Response |
| --- | --- | --- |
| GET | `/works?q=&kind=&language=&limit=&cursor=` | `WorkPage` |
| GET | `/search` with the same query fields | `WorkPage` |
| GET | `/works/{work_id}` | `WorkSummary`, including released description |
| GET | `/works/{work_id}/units?limit=&cursor=` | ordered `UnitPage` |
| GET | `/publications/{publication_id}` | `PublicationManifest` |
| POST | `/publications/{publication_id}/playback-authorizations` | `PlaybackAuthorization` or explicit error |

IDs are canonical lower-case UUIDs and are immutable identity, never a title, slug or order.
`publication_id` is the exact immutable AudioRelease ID. Units retain Chapter/Episode kind.
The playback POST accepts an empty body or `{}`; maximum body 32 KiB. Anonymous/native callers
need no Origin or Studio cookie. This exception is limited to this exact public POST; Studio
CSRF remains unchanged. Authentication must be added through the listener identity boundary,
not by adopting the Studio cookie. Playback never invokes production, encoding or transcoding.

## Public eligibility and representative metadata

A release must have an active pointer, complete admitted immutable manifest, matching
normalized Media identities/digests/sequences, unchanged latest distribution clearance and
approval, a current active owner, unexpired rights and no release withdrawal or submission
revocation. Both its pinned Work visibility and current Work visibility must be Public;
current Work/unit archival removes access. Catalog's `draft` lifecycle is CMS metadata state,
not an audio publication state; an approved active audio release is the publication authority.
Private/unlisted/draft-only/staged/unapproved/retracted/superseded/expired records and absent
records return the same anonymous `not_found` response. No previous-authorization history
exists in this slice, so there is no public 410 existence disclosure or retained-release access.

For a work, choose the newest eligible active release by `(published_at DESC, release_id ASC)`.
List and detail use the same representative. Search and kind/language filters apply after this
selection. Title and description come from the exact Work metadata revision pinned by that
release. Unit titles/descriptions come from their own pinned metadata revisions. Draft head
text, source, private rights/audit notes, provider identities, producer bindings and storage keys
never enter the public projection. `language` is the actual released Content/audio language;
a Work's editorial language may differ. No genre, cover, author, popularity or rank is invented.

Public discovery remains useful metadata when playback delivery is unavailable. The normal
Publication host cannot activate releases because its delivery adapter is unavailable. A
previously activated eligible record may be discoverable, while playback explicitly fails closed.
A database failure returns unavailable, never a mock/empty-success catalog.

## Opaque keyset pagination

Default limit 20; valid 1–50. No offset or page-number parameter. A cursor is `lc1_` followed by
256 random bits encoded as hex. It is a reference into a bounded process-local vault, not base64
JSON, an internal ID or an authorization token. It binds version, normalized search, kind,
audio language, limit, route kind and (for units) work ID. Tampering, wrong query/type/work,
expiry, server restart or unknown version returns `invalid_cursor` and requires a fresh walk.
The vault holds at most 4096 cursors, discards expired entries on issue and expires them after
15 minutes. Capacity/randomness/lock failure returns typed unavailable without evicting a live
walk or exposing state. Re-reading a cursor does not extend its lifetime.

Work order is `(first_published_at DESC, work_id ASC)`. The first successfully committed public
index writer stores its release's actual publication timestamp as the immutable work anchor.
This recent shelf means first publication of a work, not a ranking or latest-rendition shelf.
Republishing, changing representative metadata or withdrawing another unit never moves that
anchor. Unit order is `(first published position ASC, unit_id ASC)`, fixed when a unit is first
publicly indexed. Republish does not adopt a later draft reorder; a versioned public reorder
policy is deferred. Position is display order, not identity.

Every page re-evaluates current visibility. Insertion/deletion/replacement can change which
transient items appear, while unchanged visible anchors appear once without offset shifts.
A bounded candidate scan can yield a short/empty page with a next cursor if eligibility changes
or time expires during the read. Continue until `next_cursor:null`; an empty intermediate page
is not the end. Immutable work/unit anchors survive withdrawal, so republish cannot resurrect
an already traversed anchor later in the walk.

## Playback and cache

The immutable public manifest pins rendition ID, manifest digest, SHA-256, bytes, integer
milliseconds, actual Ogg/Opus codec/channel/rate/profile and Content language. It never contains
an expiring URL, private object ID, clearance/approval or QC evidence. There is one actual
rendition, with no invented AAC or automatic quality fallback.

`PlaybackDelivery` is a distinct trusted adapter consuming a sealed public-only permit. The
application bounds a grant to 120 seconds and the rights window, then performs a fresh PG
eligibility read after signing. It rejects stale, expired, out-of-window or non-range grants.
The trusted Rust adapter constructs `MediaGrant::verified_for` with that exact sealed permit;
the returned grant must preserve publication/rendition/object, manifest version/digest,
SHA/length and access window. Replaying another permit fails closed. The constructor rejects
non-HTTPS or credential-bearing locations, fragments, whitespace/control characters and
unbounded expiry. This binding is not actual-byte or current-eligibility verification.
The default adapter returns `rendition_unavailable`, with no URL. A real adapter must validate
public-object SHA/length/identity and enforce current revocation/withdrawal on every GET/HEAD,
Range/seek, audio/ogg content type, cache isolation and grant refresh. No public adapter, public
activation, file Range/CDN probe or CORS verification occurred here. Private Storage PR90 is
not public delivery and is not incorporated.

All JSON responses currently use `Cache-Control:no-store` and nosniff because eligibility is
mutable even when manifest identity is immutable. No signed URL is a catalog/cache identity.
Conditional ETag caching, per-origin CORS and native/client adoption are deferred acceptance.

## Errors and compatibility

`ListenerError` contains only `code` and `retryable`. Exact mapping:

| Code | HTTP | Retryable |
| --- | --- | --- |
| invalid_request | 400 | false |
| invalid_cursor | 400 | false |
| not_found | 404 | false |
| rendition_unavailable | 503 | true |
| unavailable | 503 | true |
| unavailable (stored corruption) | 500 | false |

Host refusal of an unimplemented/non-public mutation still uses the pre-existing host ApiError.
Known Theatre handlers, query/body rejection, fallback and timeout use the Theatre error contract.
SQL, paths, audit data and exceptions never appear in it.

[Strict JSON Schema](schema/theatre/discovery/v1.schema.json),
[shared Rust DTOs](api/src/theatre.rs) and [original fixtures](fixtures/theatre/discovery/v1/)
are versioned together and verified by schema/DTO round trips. This follows the repository's
current hand-authored schema/shared-Rust method. Proposed OpenAPI/code generation and Kotlin
wire generation are **not implemented**; client adoption must establish drift-checked generation
before claiming Web/CMP wire acceptance. No hand-written Kotlin wire DTO was added here.

New endpoints are additive. Earlier Theatre mock playback vectors remain separate. Times are
explicit Unix milliseconds (`*_at_ms`), not float seconds; these new fields choose the existing
backend time representation instead of the earlier illustrative RFC3339 listener proposal.

## Shared transport prototype boundary

The [#120 transport draft](../../docs/evidence/playback-range-transport.md) reuses HTTP mechanics
with the real Studio private-reader consumer and a test-only Theatre JSON/application/byte
fixture. The fixture pins actual arbitrary transport bytes but mocks publication/SQL/codec
authority; it is not playable-media admission, a public storage adapter or live delivery.
No new endpoint, wire field, signer, provider or runtime switch is introduced. The normal
PlaybackDelivery/PublicationDelivery composition remains unavailable. Production mode and
cache security remain [proposed evidence-gated decisions](../../docs/decisions/0027-publication-streaming-evidence-gates.md).
