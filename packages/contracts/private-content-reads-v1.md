# Private content reads v1

Implemented only under `/api/v1/studio`. Catalog, Editorial and Ingestion remain the content
owners. Binary media uses its own HTTP response, independently of JSON/MessagePack metadata.
No new route is added to the deprecated Studio alias, Theatre or Admin.

## Existing exact reads retained

| GET route after `/api/v1/studio` | Authority and contract |
| --- | --- |
| `/works`, `/works/{work}`, `/works/{work}/content-units`, `/content-units/{unit}`, `/works/{work}/history` | [Catalog v1](catalog-v1.md), existing authorized deterministic pagination |
| `/content-units/{unit}/content/revisions/{revision_id}` | [Content v1](content-v1.md), exact immutable accepted revision, never head substitution |
| `/scripts/{script}/revisions/{revision}` | [Studio v1](studio-v1.md), exact canonical Script IR export with content/export digests |
| `/imports/{source}/original` | [Import v1](manuscript-v1.md), independently authorized original bytes |
| `/scripts/{script}/audio-intents/{intent}` | [Intake v1](studio-audio-intake-v1.md), immutable legacy receipt and verified media metadata |
| `/content-units/{unit}/audio-submissions/{submission}` | [Lifecycle v1](audio-lifecycle-v1.md), exact admitted rendition/profile/provenance/review metadata |

Canonical bounded manuscript text, original imports and Script IR exports stay in PostgreSQL.
The owning readers verify their established digest schemes. Catalog existence does not grant
private script/source/rights access to a listener.

## Owner audio delivery

New GET/HEAD routes:

- `/scripts/{script}/audio-intents/{intent}/bytes`
- `/content-units/{unit}/audio-submissions/{submission}/renditions/{rendition}/bytes`

The second route pins an explicit immutable rendition ID; the first resolves the unique immutable
receipt of the exact intent. A pending intent/submission has no readable accepted bytes. A stale
head or brief never retargets historical preview. Current owner/session authorization is checked
before object I/O and again before returning the stream. Producer bearer credentials confer no
preview access. Object I/O never holds a metadata transaction open.

Full GET returns `200`, `Content-Type: audio/ogg`, exact `Content-Length`, `Accept-Ranges: bytes`,
a quoted strong ETag containing the accepted SHA-256, `Cache-Control: private, no-store, no-transform` and `nosniff`.
All current admitted profiles are the existing four mono Ogg/Opus envelopes. Profile/duration/
checksum metadata remains on the JSON receipt/rendition; delivery neither transcodes nor changes it.

Single closed (`bytes=0-1023`), open (`bytes=1024-`) and suffix (`bytes=-1024`) ranges return `206`
with exact interval length and `Content-Range: bytes <first>-<last>/<whole-length>`. Range-unit
names are case-insensitive. Unknown units are ignored/full `200`, as required by
[RFC9110](https://www.rfc-editor.org/rfc/rfc9110.html#section-14.2). A syntactically valid
multiple-range request is ignored in its entirety and returns full `200`; multipart bodies are
not supported. Malformed byte ranges, duplicate Range/If-Range fields and integer overflow
return `400` with existing `invalid_request`. A valid unsatisfiable range returns `416` and
`Content-Range: bytes */<whole-length>`. HEAD ignores Range and returns full metadata with no body.
If-Range with the exact strong ETag allows a range; a differing/weak/date validator falls back
to full `200`. Malformed evaluated If-Range returns `400`. Strong `If-Match` failure returns
`412` with `invalid_request`; weak `If-None-Match` equality (including `*`) returns empty `304`.
Entity-tag preconditions are evaluated before Range, after current authorization and configured
readiness of the exact recorded storage target. A configured replacement write target cannot
hide an unavailable historical target behind a 304/412/416; this is routing readiness, not
remote object verification. Repeated
list-valued entity-tag fields are combined with bounded parsing: at most 128 list members and
8 KiB per evaluated field-set, including two bytes of bookkeeping per header line. Irrelevant
headers remain ignored according to precondition precedence and HEAD semantics. No Last-Modified fact is
invented; date preconditions are ignored. All response paths remain no-store. These transport
helpers confer no publication access; see the [bounded prototype evidence](../../docs/evidence/playback-range-transport.md).

Unauthorized session `401`, wrong owner/resource or absent accepted rendition `404`, storage
access denial `403`, missing object `404`, timeout/outage/unsupported capability/busy `503`, and
corrupt recorded object `500` are explicit. Errors contain only existing safe API codes. Before
headers, preparation errors become those responses; a failure/truncation after headers aborts
the body and cannot change its already-sent status. The stream has a 60-second deadline checked
when demand-polled, exact-length enforcement and four request lanes retained until EOF/error/drop.
An unpolled body retains its lane until the next poll or drop; this is not an idle-client eviction
watchdog, and deployment must accept its transport idle/slow-client policy separately. The registry
separately caps all object operations at four; filesystem blocking work retains its own permit
through cancellation. Local range delivery hashes in bounded chunks then seeks the same verified
inode. S3 verifies all actual bytes then pins the delivered GET by version or strong opaque ETag;
its second response must confirm that pin. ETag is not treated as a provider checksum.

## Refreshable access

Owner GET routes ending in `/read-access` beside each `/bytes` route return `303` to the protected
bytes path, or a transient scoped S3 GET grant. Direct grants require declared capability,
actual-byte readback and a usable storage version ID; an unversioned provider falls back to the
API stream. Grant lifetime is 60 seconds; `X-Cantos-Read-Expires-At` is Unix seconds when a direct
grant is issued. Refresh by calling the authorized `/read-access` route again. Signed URLs are
bearer secrets, never persisted, logged or used as content/cache identity. Revocation prevents
new grants; a previously issued grant may remain usable until expiry. Browser CORS and actual
provider expiry/Range behavior require separate authorized acceptance.

Existing uploads retained: owner `POST /scripts/{script}/audio-intents` and
`PUT /scripts/{script}/audio-intents/{intent}/bytes`; owner
`POST /content-units/{unit}/audio-submissions`; scoped producer
`PUT /producer/audio-submissions/{submission}/bytes`. The scoped PUT remains bounded server
admission/finalization with actual inspection; direct temporary uploads and multipart are deferred.

## Proposed publication-bound read projection v1

Reserved dependency on #11 and #12, **not served or constructed by this PR**. The smallest future
read envelope pins `schema_version = publication-content-read-v1`, `publication_id`, `work_id`,
`content_unit_id`, exact `content_revision_id`, and approved `rendition_id`, SHA-256, byte length,
profile and duration. Optional text must be an explicitly approved publication text projection.
It contains no private Script IR, rights records, storage target/key or persistent signed URL.
Publication must supply the immutable approved manifest; listener authorization must supply the
separate access decision. No fake release, entitlement or public route is introduced.
