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
| GET, HEAD | `/media/{grant}` | Raw `audio/ogg` bytes or `ListenerError`; **composition-gated**, see [origin-mode media](#origin-mode-media-proposed-adr-0032) |

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
A Work or unit saved without such a release, even with Work visibility `public`, is therefore
absent (`200` with `items: []`, or `not_found`); metadata-only discovery is only proposed in
[ADR 0051](../../../../../../docs/decisions/0051-metadata-only-public-discovery.md).
Private/unlisted/draft-only/staged/unapproved/retracted/superseded/expired records and absent
records return the same anonymous `not_found` response. No previous-authorization history
exists in this slice, so there is no public 410 existence disclosure or retained-release access.

For a work, choose the newest eligible active release by `(published_at DESC, release_id ASC)`.
List and detail use the same representative. Search and kind/language filters apply after this
selection. Title and description come from the exact Work metadata revision pinned by that
release. Unit titles/descriptions come from their own pinned metadata revisions. Draft head
text, source, private rights/audit notes, provider identities, producer bindings and storage keys
never enter the public projection. `language` is the actual released Content/audio language;
a Work's editorial language may differ. No genre, author, popularity or rank is invented, and
default v1 carries no cover (see [public cover v1.2](#public-cover-v12-composition-gated)).

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
milliseconds, actual Ogg/Opus codec/channel/rate/profile and Content language. `duration_ms`
(here and in `UnitSummary`) is the verified rendition duration, always `1..=3,600,000`; the
server never emits `0` or an "unknown" value, although the schema's `minimum` is still `0`
([#172](https://github.com/loveoverflowcom/cantos/issues/172); a `minimum: 1` tightening is left open in
[ADR 0035](../../../../../../docs/decisions/0035-theatre-catalog-unit-availability.md)). It never contains
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
Range/seek, audio/ogg content type, cache isolation and grant refresh. The proposed origin-mode
adapter below does so only together with its media route; no host composes it, and no public
activation, real storage, CDN or CORS verification occurred. Private Storage PR90 is not public
delivery and is not incorporated.

All JSON responses currently use `Cache-Control:no-store` and nosniff because eligibility is
mutable even when manifest identity is immutable. No signed URL is a catalog/cache identity.
Conditional ETag caching, per-origin CORS and native/client adoption are deferred acceptance.

## Origin-mode media (proposed ADR 0032)

**Implemented core, composition-gated, IO_ACCEPTANCE_PENDING.** [ADR 0032](../../../../../../docs/decisions/0032-theatre-origin-playback-delivery.md)
is proposed, not accepted. Only trusted host code can compose `OriginPlayback`; the default host
does not, so `GET /media/{grant}` answers `not_found` and playback POST stays
`rendition_unavailable`. No production publication-class object reader exists yet (#10/#11).

When composed, `PlaybackAuthorization.url` is `https://{api origin}/api/v1/theatre/media/{grant}`
with `grant` = `pg1_` + 32 hex publication UUID + 16 hex expiry + 64 hex HMAC-SHA256 (116 ASCII
characters, lower case). The grant is an opaque bearer capability: clients must not parse,
log or persist it, and it names no private object. `expires_at_ms` is
`min(issued + 120000, rights valid_until)`.

Every GET/HEAD re-resolves current eligibility, authenticates the grant against the exact sealed
identity (publication, rendition, manifest digest, SHA-256, object, byte length), opens the
object and re-admits the grant before sending bytes. HTTP mechanics are the shared transport:
strong ETag `"{sha256}"`, `Accept-Ranges: bytes`, single ranges (open, closed, suffix) → 206 with
`Content-Range`; unsatisfiable → 416 `bytes */{length}`; multi-range → full 200; `If-Match`,
`If-None-Match` (304), entity-tag `If-Range`; date validators are ignored. `Content-Type:
audio/ogg`, exact `Content-Length`, no content encoding, `private, no-store, no-transform`,
`nosniff`. HEAD has no body.

| Media response | HTTP | Code / retryable | Client action |
| --- | --- | --- | --- |
| Bytes | 200 / 206 / 304 | — | Play; never swap to another publication |
| Malformed Range or validator | 400 / 412 / 416 | `invalid_request` / false | Fix the request |
| Unknown, forged, tampered, expired, withdrawn, revoked, superseded, owner-disabled or unconfigured | 404 | `not_found` / false | Re-POST the authorization for the same `publication_id`; `200` with the same `sha256` resumes at the current position, `404` means stop |
| Lanes exhausted, storage timeout/unavailable | 503 | `unavailable` / true | Retry with backoff |
| Eligible release whose promoted object is missing | 503 | `rendition_unavailable` / true | Retry later; do not fall back to another rendition |
| Stored bytes contradict the sealed identity | 500 | `unavailable` / false | Stop and report |

Clients should refresh shortly before `expires_at_ms`. Theatre Web (#13), CMP (#14/#16) and the
player UX issues (#154/#155) own player integration; none is implemented by this contract change.
The Theatre Web guest slice ([#269](https://github.com/loveoverflowcom/cantos/issues/269),
[evidence](../../../../../../docs/evidence/theatre-web-guest-playback.md)) consumes the routes above unchanged:
it pins the manifest, POSTs an empty body, plays `PlaybackAuthorization.url` exactly as given
through one audio element, refreshes 30 s before expiry for the same `publication_id` and compares
every authorization's publication, rendition and SHA-256 with the pin. No wire field was added.
Browser-origin addressing of a deployed API (CORS, CDN) remains separate acceptance.

## Errors and compatibility

### Presentation metadata v1.1.0 (explicit opt-in)

[#251](https://github.com/loveoverflowcom/cantos/issues/251) adds the closed query
`presentation=1` to `GET /works`, `GET /search`, `GET /works/{work}` and
`GET /works/{work}/units`. Omitting the query preserves every default v1 field and value;
existing strict Rust and Kotlin readers reject unknown fields, so enriched responses use
separate reviewed DTOs rather than adding fields to `WorkSummary` or `UnitSummary`.
`presentation=2` selects [public cover v1.2](#public-cover-v12-composition-gated); any other value is `invalid_request`. The capability is part of cursor identity:
a cursor cannot switch between default and presentation responses.

| Opt-in DTO | Added field | Meaning |
| --- | --- | --- |
| `PresentedWorkSummary` / `PresentedWorkPage` | `unit_count` | Positive number of currently eligible public units in the Work; drafts, withdrawn, expired and otherwise ineligible releases do not count |
| `PresentedUnitSummary` / `PresentedUnitPage` | `ordinal` | Positive one-based rank among currently eligible units ordered by immutable first-published position, then unit UUID |

Both integers are bounded by JSON's exact integer maximum `9,007,199,254,740,991`.
`ordinal` is a display rank, never an identity, an authored episode number or a season number.
Sparse/duplicate editorial positions still produce consecutive ranks. It is independent of the
page boundary and applies across the whole currently eligible Work list. An eligible single-unit
Work has `unit_count=1` and `ordinal=1`; no zero/unknown sentinel is emitted.

Counts and ordinals describe eligibility at each request's evaluation time. They are not frozen
for the lifetime of a cursor: publication, withdrawal or a rights-window expiry can change them
between pages. Immutable anchors and existing keyset traversal remain authoritative; clients
must not use count/rank as a cursor, completion check or playback authorization. Republish does
not change the immutable public ordering anchor. Metadata uses released public revisions and
actual audio language, with the same admission rules as default discovery.

Theatre Web adopts the capability ([#270](https://github.com/loveoverflowcom/cantos/issues/270)):
every Work list/detail and Unit list read sends `presentation=1` and decodes the `Presented*`
DTOs (the Work's `unit_count`, each unit's `ordinal`, shown as a public-list count/position, never
an identity or authored number). Other clients continue using default v1; their decoding/adoption
and runtime UI acceptance require separate work. Genres, credits,
seasons and authored episode numbering remain with #177 and its decision gate below.

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

[Strict JSON Schema](discovery.schema.json),
[shared Rust DTOs](../../../../src/contract/theatre/discovery.rs) and [original fixtures](fixtures/)
are versioned together and verified by schema/DTO round trips. This follows the repository's
current hand-authored schema/shared-Rust method. Proposed OpenAPI/code generation and Kotlin
wire generation are **not implemented**; client adoption must establish drift-checked generation
before claiming Web/CMP wire acceptance. No hand-written Kotlin wire DTO was added here.

New endpoints are additive. Earlier Theatre mock playback vectors remain separate. Times are
explicit Unix milliseconds (`*_at_ms`), not float seconds; these new fields choose the existing
backend time representation instead of the earlier illustrative RFC3339 listener proposal.

## Public cover v1.2 (composition-gated)

[ADR 0033](../../../../../../docs/decisions/0033-release-pinned-public-cover.md) was accepted by
the owner for [#157](https://github.com/loveoverflowcom/cantos/issues/157), and
[#171](https://github.com/loveoverflowcom/cantos/issues/171) implements it in the Server:
`presentation=2` is admitted on every route below, and the cover route is mounted. A cover is
shown only on a host that composes cover delivery, which today is the public-media host
(`CANTOS_PUBLICATION_ROOT`, `CANTOS_PUBLIC_MEDIA_ORIGIN` and the playback key) that also has
private media storage configured; the cover URL uses that same explicit HTTPS origin. Every other
host answers `presentation=2` with `cover: null` and the route with `not_found`. The owner
discloses each image through the [Studio cover disclosure](../../../studio/cover-disclosure/v1/README.md)
route. Clients must still use URLs exactly as given and adopt this level only under #158/#159.

| Request | v1.2 response |
| --- | --- |
| `GET /works?…&presentation=2`, `GET /search?…&presentation=2` | `CoveredWorkPage` |
| `GET /works/{work_id}?presentation=2` | `CoveredWorkSummary` |
| `GET /works/{work_id}/units?…&presentation=2` | `PresentedUnitPage`, identical to `presentation=1` |
| `GET`, `HEAD /works/{work_id}/cover/{sha256}` | Raw `image/png` or `image/jpeg` bytes, or `ListenerError` |

`CoveredWorkSummary` is every `PresentedWorkSummary` field plus a **required nullable**
`cover`. `null` is the only no-cover state: no artwork, an undisclosed/revoked/expired image,
corrupt or foreign facts and uncomposed cover delivery all serialize identically and never
explain why. Omitting `cover` is invalid. The presentation level is cursor identity, so a
`presentation=1` cursor is `invalid_cursor` under `presentation=2` and vice versa.

`CoverImage` carries exactly `url`, `sha256` (of the image bytes), `content_type`
(`image/png` or `image/jpeg`), `width_px`/`height_px` (1–4096), `byte_length` (1–8,388,608)
and `alt_text` (non-blank, at most 1024 UTF-8 bytes; the schema's character bound is looser).
These are the private Artwork v1 admission bounds; no resize or thumbnail is produced.
Asset IDs and digests, rights references, provenance, owner, receipts, disclosure history and
storage keys are never present. The cover is the image pinned by the same representative
release as `title`/`description` and is shown only while the owner's disclosure of that exact
image is current; a newer private image appears only after a newer eligible release pins it.

`url` is `https://{public API origin}/api/v1/theatre/works/{work_id}/cover/{sha256}` with the
summary's own `work_id` and `cover.sha256`. Clients use it exactly as given and must not derive
another URL from a summary. Every GET/HEAD re-resolves the representative release, re-projects
the cover with the current disclosure and clock and serves bytes only when it shows exactly
that SHA-256. A stale URL after republish, revocation, expiry or withdrawal is `not_found`.

| Cover response | HTTP | Code / retryable | Client action |
| --- | --- | --- | --- |
| Bytes | 200 / 304 | — | Show; `ETag: "{sha256}"`, `Cache-Control: public, no-cache`, nosniff, exact `Content-Type`/`Content-Length` |
| Unknown Work, malformed or stale SHA-256, ineligible release, no current disclosure, uncomposed delivery | 404 | `not_found` / false | Show the no-cover fallback; refresh the summary later |
| Storage timeout/unavailable | 503 | `unavailable` / true | Keep the fallback; retry with backoff |
| Stored bytes contradict the admitted SHA-256/length | 500 | `unavailable` / false | Keep the fallback; report |

`If-None-Match` with the strong ETag (or its weak spelling, or `*`) returns 304 after the same
re-projection; `no-cache` requires revalidation before each use, so revocation takes effect on
the next request rather than after a cache lifetime. Ranges are not served (`Accept-Ranges:
none`; a `Range` header receives the full 200). HEAD has no body. Only a 200/304 from this route
carries `public, no-cache`; every error stays `no-store`. The Server hashes the stored object
before sending any byte and re-projects again after opening it, so a decision committed while
the object was opening still denies. A cover failure never changes the summary's other fields,
unit lists or playback.

The [schema](discovery.schema.json) defines `CoverImage`, `CoveredWorkSummary` and
`CoveredWorkPage`; the [cover fixtures](fixtures/cover/) show a shown cover and `null`; the
[shared Rust DTOs](../../../../src/contract/theatre/discovery.rs) round-trip them and the
conformance tests prove that default and `presentation=1` readers reject v1.2 responses.
The fixtures live in a subdirectory so the default-v1 CMP corpus is unchanged. Theatre Web
([#158](https://github.com/loveoverflowcom/cantos/issues/158)) now sends `presentation=2` on its
Work list, search and detail reads, admits a cover only in the exact delivery shape above and shows
the fallback for `null` or any image failure; CMP adopts it under #159. #171 selects no CORP/CORS header for the deployed origins
(ADR 0041): the route answers with the Theatre surface's existing CORS boundary, and an
explicit `Cross-Origin-Resource-Policy` and any offline cover retention on CMP remain open
decisions for the deployment owner and #159.

## Client state coverage (#172) is a review input, not a wire change

The [coverage matrix](../../../../../../docs/work-plan/theatre-catalog-state-handoff.md) maps each Web/CMP
catalog state to an existing field and lists gaps. Under the accepted
[ADR 0035](../../../../../../docs/decisions/0035-theatre-catalog-unit-availability.md) the unit list stays
"listenable now": `UnitSummary` gains no availability, reason, count or unknown-duration field,
and a row's `unavailable` state is derived by the client from evidence it already holds. The
shared fixtures [`units_empty_continuation.json`](fixtures/units_empty_continuation.json),
[`units_end_empty.json`](fixtures/units_end_empty.json) and
[`units_republished.json`](fixtures/units_republished.json) show the empty
intermediate page, the terminal empty page and a version change using only the v1 fields. A cursor is
not consumed by a read, so a failed page is retried with the same cursor until it expires.

## P1 metadata is not part of this contract

[#157](https://github.com/loveoverflowcom/cantos/issues/157) accepted only the release-pinned
authority principle for future public metadata ([ADR 0030](../../../../../../docs/decisions/0030-release-safe-public-metadata.md)).
Genre/taxonomy is open pending its vocabulary decision; contributors, credits/license, podcast
seasons, timed markers and public transcripts are deferred. None is a wire field here, and
[#177](https://github.com/loveoverflowcom/cantos/issues/177) needs its own accepted rows before
any field. Existing strict v1 readers reject unknown properties, so even optional response
fields require deliberate compatibility/version review. Current q/kind/language filters,
cursor behavior and public projections remain unchanged.

## Shared transport boundary

The [#120 transport draft](../../../../../../docs/evidence/playback-range-transport.md) introduced the shared
HTTP mechanics. [#170's core](../../../../../../docs/evidence/public-media-delivery-core.md) now drives the
Theatre contract fixture through the production `/media/{grant}` route and `OriginPlayback`,
with named eligibility and publication-object doubles over arbitrary transport bytes. That is
mock-integration evidence, not playable-media admission, a public storage adapter or live
delivery. No wire field, provider or runtime switch is introduced, and the normal composition
remains unavailable. Cache security beyond no-store remains an
[evidence-gated decision](../../../../../../docs/decisions/0038-publication-streaming-evidence-gates.md).
