# Authenticated shared discovery and delivery v1

ADR 0054 D05/D08; task #653. This is a separate authenticated Theatre module.
Public Discovery keeps its current representation. Shared metadata reuses the strict
`MetadataWorkPage`, `MetadataWorkSummary`, `MetadataUnitPage`, `PublicationManifest` and
`PlaybackAuthorization` shapes owned by Discovery v1 and its schema/fixtures. Theatre Web (#654) consumes these routes explicitly through its Shared Library and `#shared/{work}` locator; public Discovery remains anonymous. CMP adoption belongs to #655.

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

## Web consumer boundary (#654)

The Web Library places Shared after Continue and scopes every cursor, response and claim receipt
in RAM to the current account/session generation. An internal login return contains only a
canonical Work ID, never a title, cover or token. Each shared request uses the Theatre lease,
exact session fence and current response check. A changed account/route aborts pending private
reads and suppresses late successes and failures. A claim retry after an unknown acknowledgement
reuses the same operation; the receipt is history, so a fresh catalog read is explicit.

Restricted Web listen controls and their imperative selection entrypoint remain unavailable.
A real local browser campaign found a teardown panic when an active private source lost its
session; metadata navigation is handled separately, and full cookie/HTTPS/seek and source
teardown acceptance is still owed in #654/#676. There is no runtime activation override.

The retained player adapter source uses one audio element. Shared manifest and authorization requests
never fall back to public routes; admitted URLs must use the configured API origin and exact
shared-media path. The element selects `crossorigin="use-credentials"` before attaching a private
source. Public sources retain the existing element transport. Private source identity includes its Theatre
session; losing the account clears the element, timers, requests and display facts. Private
sources cannot enter the public Library/progress journal or restore path, and those controls
remain unavailable. A current media/manifest denial also removes private detail facts.

Source and native core/DOM checks do not qualify browser cookie/media transport. The ordinary
executable still starts with restricted serving disabled. The explicit ignored test fixture in
`tests/support/web_sharing_fixture.rs` owns only disposable synthetic accounts/content and two
real native files, a real Identity process and loopback HTTP. Its optional supplied 60-second
mono Opus bytes are independently inspected by the caller; seeded rights/QC/publication facts
are synthetic. The HTTP metadata mode deliberately does not replace the HTTPS media-origin
invariant. Full browser playback/seek requires a separately trusted local HTTPS terminator and
HTTPS-origin build; no browser certificate validation or production setting is weakened here.
