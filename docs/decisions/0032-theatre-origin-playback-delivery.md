# ADR 0032: Theatre origin playback delivery (mode A)

- Status: **accepted for step-1 origin integration** — owner requested configured API/storage
  and production catalog composition; deployed delivery/performance and native playback acceptance remain pending
- Date: 2026-10-16
- Decision owner: Cantos product owner (step-1 real-data integration request); deployment owner retains activation acceptance
- Related issue or PR: [#170](https://github.com/loveoverflowcom/cantos/issues/170), RFC
  [#120](https://github.com/loveoverflowcom/cantos/issues/120), prerequisites #10/#11
- Supersedes / superseded by: N/A; selects one row of [ADR 0038](0038-publication-streaming-evidence-gates.md)'s
  "Authorized delivery" track. ADR 0028's evidence gates still apply to deployment acceptance.

## Context

Theatre already resolves an exact eligible release and returns a `PlaybackAuthorization`, but
the default `PlaybackDelivery` is `UnavailablePlayback`, so every playback request is 503. The
contract requires that current withdrawal/revocation applies on **every** GET/HEAD and Range.

Facts that constrain the choice, inspected on `develop` `f3619bf`:

- A signer alone cannot enforce withdrawal after it issues a URL (ADR 0028).
- The only configured provider example declares `range_reads = true`,
  **`presigned_reads = false`** (`services/cantos-api/config/storage.longvan.example.toml`).
  Option B therefore has no provider to sign with today.
- One Range/validator/body transport kernel (`audio_transport`) already serves Studio's private
  reads; its single-range normalization is shared and property-tested.
- No CDN, edge authorization or cache-key evidence exists for any deployment.

## Decision

Select **mode A — origin/API-controlled Range** as the first delivery mode,
with an HMAC-sealed short-lived grant per playback authorization:

1. `POST …/playback-authorizations` resolves the eligible release, bounds the grant to
   `min(now + 120 s, rights valid_until)` (`grant_expiry`), seals it and re-resolves before
   answering. `url` is `{public origin}/api/v1/theatre/media/pg1_…`.
2. The token names only the publication UUID and expiry. Its HMAC-SHA256 authenticates the
   complete sealed identity (publication, rendition, manifest digest, SHA-256, private object
   ID, byte length) without revealing the object ID. Tokens are canonical, fixed length (116).
3. `GET`/`HEAD …/media/{grant}` re-resolves current eligibility, authenticates the token against
   that exact identity, decides with `decide_media_fetch`, plans the HTTP response, opens the
   promoted publication object, then **re-admits the same grant from a fresh read** before any
   byte is sent. Every denial is the same anonymous `404 not_found`.
4. Responses stay `private, no-store, no-transform`, `nosniff`, `Referrer-Policy: no-referrer`,
   no content encoding, strong SHA ETag, GET/HEAD, single range, 200/206/304/412/416.
   Multi-range requests are answered in full (200). No shared or browser cache is relied on.
5. Composition: one trusted `OriginPlayback` value enables grant issuance and the media route
   together. The unconfigured host composes neither; the route answers `not_found`.
   The configured server requires a separate publication filesystem root, HTTPS media origin and
   operator-owned raw 32-byte key file. The same adapter promotes inspected immutable renditions
   for publication admission and opens their exact verified bytes for delivery. Private and
   publication roots cannot overlap. Incomplete or invalid configuration fails startup.

The first adapter is a separate local publication filesystem class; it does not widen access to
private storage. CDN, cache policy beyond no-store, seek index, prebuffer, offline lifecycle,
codec/profile and client player remain outside this decision.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| A: origin/API-controlled Range (recommended) | Withdrawal, revocation, supersession, owner disablement and expiry are checked on every request, before and after object open; reuses the existing transport kernel; provider-neutral; no provider signing needed | All bytes cross the API; throughput, memory and egress cost must be measured; each seek is one authorized request; no cache offload | Only option whose revocation semantics are enforceable without new edge infrastructure |
| B: short-lived direct signed object URL | Offloads bytes to storage/CDN | Revocation lag equals URL lifetime; signed query strings risk logs and cache keys; current provider config has no presigned reads | Rejected now; revisit only with a provider that signs and an accepted bounded-lag policy |
| C: hybrid edge-authorized CDN + origin fallback | Scale and caching with per-request edge authorization | Requires edge auth, cache-key isolation and purge evidence; two paths to keep identical | Deferred until a CDN exists and mode A measurements show a need |

## Thresholds, rollback and acceptance gates

| Item | Proposed value | Status |
| --- | --- | --- |
| Grant TTL | 120 s, never beyond the rights window | Implemented constant (`GRANT_TTL_MS`), pre-existing value |
| Clock rollback | A grant expiring more than one TTL after the read clock is `Unbounded` and denied; clients refresh | Implemented; operational impact NOT_RUN |
| Concurrent media responses | 32 lanes per process, held until the body ends or is dropped; exhaustion is retryable 503 | Implemented starting bound, **not measured** |
| Body deadline | Existing 60 s demand-polled transport deadline | Existing; slow-client policy still pending (ADR 0028) |
| Key | 32 random bytes from operator secret storage; rotation invalidates outstanding grants (≤120 s); clients refresh | Explicit private key-file loader; deployed rotation acceptance pending |
| Rollback | Remove all public-media configuration; playback returns `rendition_unavailable` and media `not_found` | Unconfigured host remains fail-closed |

Deployment acceptance before public activation (owner: Publication/Media + deployment):
publication-class promotion and a `PublicationObjects` adapter (#10/#11) reading the exact
version-pinned object; real-bytes GET/HEAD/206/416 with checksum through the deployed proxy;
withdrawal/expiry/restart/key-rotation behavior; access logs that redact `/theatre/media/*`;
proxy no-transform/no-cache; Theatre CORS if Web uses a cross-origin API; measured p50/p95 start
and seek, throughput, memory per lane and egress. Missing permission or environment is
`BLOCKED`, never assumed. Web/Android/iOS player interop stays with #13/#14 (#16 offline).

## Consequences

- Product and user impact: none until composed. Afterwards one listener request per seek; a
  grant refresh is needed for long sessions (clients refresh before `expires_at_ms`, or after a
  `404` from the media route by re-POSTing; the authorization answer distinguishes withdrawal).
- Compatibility and migrations: no DTO, schema or migration change; `PlaybackAuthorization.url`
  gains a concrete origin-mode shape. One additive route under `/api/v1/theatre/media/{grant}`.
- Operations, storage, and production cost: API egress for all audio; lane bound and memory
  must be measured. The grant key is a new secret with rotation semantics above.
- Implementation and recovery requirements: dropping a response drops its source and lane;
  bytes are never sent after a failed post-open recheck; storage failures map to typed
  `rendition_unavailable`/`unavailable`/corrupt responses, never another rendition.
- Deferred questions: B/C, CDN caching, listener identity/entitlement claims in the token,
  key IDs for overlapping rotation, CORS, seek index, prebuffer and offline downloads.

## Validation and revisit conditions

Implemented evidence (core only): the pure `playback_grant` kernel with example, property and
tamper tests; production route mock integration over named eligibility and publication-object
doubles; Kani harness registered (not run); Quint capability
`cantos.media.public-playback-delivery` (draft, model-only). See the
[evidence ledger](../evidence/public-media-delivery-core.md). Revisit when a provider supports
signed reads with bounded revocation lag, when measured origin throughput/cost fails a target
the owner sets, or when listener entitlement requires per-identity grants.
