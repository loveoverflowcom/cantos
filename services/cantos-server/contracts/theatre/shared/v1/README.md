# Authenticated shared discovery and delivery v1

ADR 0054 D05/D08; task #653. This is a separate authenticated Theatre module.
Public Discovery keeps its current representation. Shared metadata reuses the strict
`MetadataWorkPage`, `MetadataWorkSummary`, `MetadataUnitPage`, `PublicationManifest` and
`PlaybackAuthorization` shapes owned by Discovery v1 and its schema/fixtures. No current
client is silently switched to these routes; Web and CMP adopt them in #654/#655.

## Routes

All paths are below `/api/v1/theatre/shared`:

- `GET /works` and `/search`: `q`, `kind`, `language`, `limit` (1–50), `cursor`.
  Lists only RESTRICTED Works with current actor/group candidates. Candidates are deduplicated,
  hydrated in bounded pages and admitted through Catalog's Work AND Unit evaluator before
  metadata, counts, ordering or search results are returned. Only admitted matches determine
  page boundaries and cursors. An exhausted bounded lookahead returns unavailable rather than a
  cursor over denied rows or a false terminal page.
- `GET /works/{work}` and `/works/{work}/units`: the current generation, with the same gates.
  Unit lists accept `limit` and `cursor`. The current persisted Unit mode is INHERIT;
  #656 owns persisted overrides and must extend the common read admission before enabling them.
- `GET /publications/{publication}` and
  `POST /publications/{publication}/playback-authorizations`: exact generation association only.
  The POST accepts an empty body or `{}`. No old release or editorial head is a fallback.
- `GET|HEAD /media/{grant}`: the URL from authorization, including each Range/seek.

Every request requires an Identity-backed Theatre cookie or native Bearer. Studio/development,
wrong-surface, duplicate or mixed credentials fail. Cookie POST requires the exact Theatre
Origin; native Bearer POST may omit Origin. The public playback POST exemption does not match
these paths. An authenticated request rejected by policy receives generic `404 not_found`;
Identity/storage outages remain `503 unavailable`. Cursors use `400 invalid_cursor` to request
restart on account/session/query/order/generation/access-policy change or expiry. Cursor state
is process-local and opaque; restart invalidates it.

## Media and credentials

A restricted grant HMAC includes exact release/rendition/manifest/object/hash/length, expiry,
and a hash of actor, subject, session, channel, security epoch/version, process admission epoch,
purpose `media`, generation/access-policy fence. It is never a transferable public permit.
A URL copied to another account/session, or used without its credential, returns no bytes.
Issuance, each GET/HEAD/Range and post-object-open repeat current admission. Expiry is the minimum
of 120 seconds, session/access/idle/absolute deadline, rights and relevant grant/member expiry.
Authorization precedes conditional and Range response planning, including HEAD/304/416.
An admitted response may finish within the existing 60-second body deadline; buffered bytes
cannot be recalled (D08b). No offline restricted entitlement is issued.

Web must use the host-only Theatre cookie (`Path=/api/v1/theatre`) on the API media origin,
with credentialed exact-origin CORS and `crossorigin="use-credentials"` where applicable.
HTML audio cannot attach a custom Authorization header. Native players must attach their
Theatre Bearer to every open/seek/refresh, exclusively on the configured API origin.
Refuse redirects and off-origin credential forwarding. Unsupported transports stay unavailable; this slice
adds no bearer-URL fallback. Browser and Android/iOS player qualification belong to #654/#655.
URLs, tokens and private object IDs must not enter logs or durable client identity.

## Covers, other surfaces and activation

Shared metadata currently returns `cover: null`; no restricted cover credential transport is
activated. The existing public cover route rechecks PUBLIC generation eligibility before any
conditional response and after open. There is no Theatre transcript/download route. Existing
Library/Continue/Saved/History/bookmark restore remains public-only and re-admits releases;
restricted items stay unavailable there until an authenticated Library projection is supplied.
No static private-object route, legacy presentation selector or old publication route grants
restricted authority.

All shared responses, including errors and HEAD, are `private, no-store`, with no-referrer and
nosniff. There is no application ACL cache or outbox-based admission. The trusted host must opt
in to `ListenerService::with_restricted_serving()` for each process, after policy confirmation;
default assembly and every restart remain disabled. No persisted flag automatically re-enables
a restored database. The explicit API-only restricted router is the composition seam; production
activation, restore confirmation and browser/device qualification remain separate operations.
