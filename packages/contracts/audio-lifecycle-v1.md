# Private Audio Lifecycle v1

Implemented on the development host. [ADR0018](../../docs/decisions/0018-external-audio-production.md)
owns the external-production boundary. [Schema](schema/audio-lifecycle/v1.schema.json) and
`cantos-contracts` raw DTOs describe JSON transport; private domain values admit them. This
contract extends [Brief v1](production-brief-v1.md) and reuses the single
[PR69 inspector/private storage path](studio-audio-intake-v1.md). It does not publish a release.

## Concrete routes

All paths below begin `/api/v1/studio`. New routes have no deprecated `/api/v1` alias.

| Method / path | Authority | Result |
| --- | --- | --- |
| POST `/content-units/{unit}/audio-submissions` | Current Studio owner cookie |201 `AudioSubmissionResponse` |
| GET `/content-units/{unit}/audio-submissions/{id}` | Current Studio owner cookie |200 historical record/current state |
| DELETE `/content-units/{unit}/audio-submissions/{id}/upload-permission` | Current Studio owner cookie |204 append-only revocation; repeat succeeds |
| POST `/content-units/{unit}/audio-submissions/{id}/reviews` | Current Studio owner cookie |201 exact append-only review |
| POST `/content-units/{unit}/audio-submissions/{id}/release-candidates` | Current Studio owner cookie |201 immutable private candidate/current blockers |
| GET `/content-units/{unit}/audio-submissions/{id}/release-candidates/{candidate}` | Current Studio owner cookie |200 same candidate/current blockers |
| GET `/producer/audio-submissions/{id}` | Exact upload bearer |200 status for this submission only |
| PUT `/producer/audio-submissions/{id}/bytes` | Exact upload bearer |200 immutable admitted rendition; PUT includes finalization |

Owner JSON requests use `Content-Type: application/json`; successful/error responses are JSON.
`Accept: application/json` is supported; MessagePack is unsupported. Binary PUT requires
`Content-Type: application/octet-stream`, with JSON response acceptance. Every non-GET request
must carry `Origin` equal to the development host's configured `CANTOS_WEB_ORIGIN`, including
external CLI requests. All responses are `no-store`/`nosniff`. No URL or private storage key is
accepted from the caller or returned for public delivery.

## Capability and immutable binding

Prepare pins Work/unit UUID, ContentRevision UUID/full `cc-r1` digest, Brief UUID/version/full
`pb-r1` digest, explicit profile, exact expected SHA-256/size and producer declarations. Current
Content, guidance, Work rights, archived state and owner authority are resolved through owning
Catalog/Production/Editorial entrypoints. Narration and recording require no Script IR/cast.
Declared rights references must be nonempty and exactly match the Brief's Work rights references;
current references must remain available to the owner. This verifies recorded reference scope,
not license legality, voice/music clearance, content fidelity or rights term coverage.

The owner supplies fresh random32bytes as64lowercase hex and an expiry at most24hours away.
Only a domain-separated hash persists. The owner delivers the token through a separately
secured credential channel, never through these responses. A Brief read token, registered Studio
session token or already-used upload token cannot be reused. Both issuance directions serialize
under the Catalog fence; read permission never becomes upload permission. Producer endpoints
reject cookies, mixed cookie/bearer requests and duplicate Authorization fields. The upload
capability authorizes exactly one prepared submission and cannot review or create a candidate.

Immutable submission/request digests are `as-r1`/`as-request1`; rendition, review and candidate
digests are `ar-r1`, `aq-r1`, `rc-r1`. Each uses SHA-256 over the owning versioned canonical JSON
value with sorted object keys, meaningful array order, and `cantos/{scheme}\n` separation. Record
digests clear their own digest field before hashing. The request fingerprint includes the token
hash; no fingerprint contains the plain token. IDs are distinct canonical UUID types, separate
from digests and actor identities. Proof types have no `Deserialize` or unchecked raw constructor. Validating restoration still requires current owning-context linkage and authority in the application.

## Actual byte admission and recovery

Four explicit `opus-mono-20/24/32/40-v1` envelopes are accepted, without defaults/AAC fallback:
Ogg, one Opus stream, mono,48kHz, positive duration up to1hour, up to32MiB, observed coded packet
bitrate no more than120% of the nominal target. VBR has no lower floor. These are admission
limits, not guarantees of intelligibility or Web/Android/iOS playback.

PR69's strict Ogg framing/CRC/granule parser, bounded ffprobe inspection and FFmpeg decode to
null inspect actual received bytes; there is no encoder/mixer/speech generator. The shared
adapter has2inspection lanes retained through HTTP cancellation and4request lanes before binary
buffering. CPU6seconds, wall8seconds per subprocess, output8MiB per stream, descriptors32,
local-file-only protocols and fixed tool arguments bound inspection. Other Unix hosts use a
hard512MiB address-space limit. macOS rejects lowering `RLIMIT_AS`: it uses a5ms sampled512MiB
resident-memory ceiling for the tool leader. This is not a hard virtual/group-total memory cap;
trusted configured tools are required. The unreaped leader protects its process-group identity
until descendants are signalled and the leader is reaped.

Private owned mode0700 directories and immutable mode0400 SHA-256 objects are synchronized and
read back before SQL acceptance. Initial authorization releases its transaction before actual
inspection/storage. Commit reacquires the Catalog fence, revalidates exact inputs/capability,
selects at most one rendition and checks authority again after INSERT. Every accepted read
checks actual private size/hash. Failed inspection leaves no accepted rendition. Metadata
failure/cancellation can leave a private orphan; a retry reuses verified bytes and one receipt.
Normal staging cleanup deletes only the request's temporary name, never a shared immutable key.
Crash-orphan retention/collection is deferred; no automatic deletion or S3/CDN claim is made.

## State, reviews and candidate blockers

`awaiting_bytes → admitted` is the only admission transition. Exact duplicate PUT replays the
same rendition; contradictory settled bytes conflict. Expired/revoked/stale authority refuses
PUT, even on replay. Owner reads preserve immutable history and project `stale`, `revoked`,
`admitted`, `expired` or `awaiting_bytes` from current facts, in that precedence. The owner may
revoke a stale submission; it stays visibly stale and its capability is permanently revoked.
Expiry ends producer access; an admitted record remains admitted for owner history/review.

QC, rights and editorial decisions are distinct closed kinds. Approved/rejected/withdrawn
records pin rendition digest, checksum, owner reviewer, server sequence/time, notes and evidence
references. They preserve named declarations; the server does not claim a human listened or that
QC findings are accurate. Latest decision per kind wins. At most1000reviews per rendition are
accepted; status returns the latest3. Existing exact operation retries preserve record identity.

A candidate copies the rendition/Content/Brief/profile identity, actual checksum, latest review
IDs and Work/unit metadata revisions. Its readiness collects every sorted blocker, including
missing/rejected/withdrawn reviews, stale Brief, revoked/expired upload permission, unrecorded
rights, changed review IDs and changed metadata. Historical candidate/retry preserves the record
while reporting today's blockers. `public_delivery_not_implemented` always blocks publication.
There is no publisher permission, release manifest, active release pointer, public media URL or
Theatre handler. No existing release can be altered by these operations.

## Errors, migration and limits

401 invalid/expired/revoked producer access;404 owner/scope mismatch;400 invalid bindings,
provenance or new byte digest/size;409 stale production inputs or operation conflict;413 body
limit;405 unsupported method;406 unacceptable response format;415 unsupported transport;422 actual media rejection/evidence unavailable;503 inspection,
storage or database unavailable;500 corrupt persisted bytes/snapshot. Error codes/field rules
are stable raw contracts; UI owns localized copy.

Applied SQL0010 is reused byte-for-byte;0011–0013 stay unchanged. Migration0014 adds immutable
submission/revocation/rendition/review/candidate tables with FKs, bounded snapshots and unique
operation/rendition/sequence guards. Registry1–14/count14 includes the previously missing0010;
upgrade tests cover databases where0011–0013 were already applied. App role needs SELECT/INSERT
on new Media tables; existing Brief read-grant issuance additionally needs SELECT on
`audio_submissions` to reject credential reuse. No UPDATE/DELETE/TRUNCATE grant is needed.

Production identity/TLS/credential delivery, object-store grants, S3/CDN, reference-aware orphan
cleanup, stronger rights/QC policy, publication, playback/interoperability and live UI integration
remain separate work. No TTS, transcoding, mixing, generation jobs or DBSP consumer was added.
