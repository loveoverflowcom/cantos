# ADR 0028: Publication streaming decisions remain evidence-gated

- Status: proposed
- Date: 2026-10-14
- Decision owner: Cantos project; acceptance and operator assignment required
- Related issue: [#120](https://github.com/loveoverflowcom/cantos/issues/120)
- Supersedes / superseded by: N/A; preserves ADR 0018 and the current private-read contract

## Context

The pinned implementation has `PlaybackDelivery` and `PublicationDelivery`, immutable
Publication/Discovery JSON contracts, and separate owner-authorized private storage. It has no
accepted production publication-class byte reader. A private `open_verified` or presigned preview
cannot become listener authority. A signer alone cannot enforce current withdrawal on each fetch.

The requested draft implements reusable HTTP mechanics with the existing Studio consumer and a
**test-only Theatre contract fixture**. It is not public playback, a promotion implementation,
codec admission, an accepted deployment mode, or a substitute for #10/#11/#13/#14 acceptance.
The normal host and publication activation remain unavailable. No decoder, index, cache or wire
schema is selected by this draft.

## Decision

No production delivery alternative is selected yet. Reuse the existing delivery ports; bind
adapter grants to the sealed publication/rendition/manifest/SHA/object/length/expiry identity.
Keep ordinary composition fail-closed. HTTP mechanics may be shared, but authentication,
authorization, error projection and storage class are not shared between Studio and Theatre.

Existing private audio reads consume a single HTTP transport kernel: strong SHA-derived ETag,
GET/HEAD, single normalized byte ranges, conditional entity tags and pull-driven exact-length
bodies. Valid multipart requests are ignored in full (200), not presented as multipart support.
`If-Match` precedes `If-None-Match`, which precedes `If-Range`/Range. There is no Last-Modified
fact; date preconditions are ignored and date If-Range falls back to the full representation.
Authenticated/private transport remains no-store; audio carries no-transform and no encoding.
These HTTP mechanics follow [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.html), not a new
entitlement, retention or public-cache policy.

[ADR 0032](0032-theatre-origin-playback-delivery.md) later **proposes** the origin/API proxy row
of the "Authorized delivery" track for #170. That proposal is not acceptance; every evidence
requirement below still applies before any host composes it.

## Alternatives

| Open track | Alternatives | Evidence required before selection |
| --- | --- | --- |
| Authorized delivery | Protected API proxy; short-lived edge-authorized direct URL; hybrid direct + proxy fallback | Same admitted bytes through actual GET/HEAD/200/206/416; pinned object version; expiry/withdrawal/revocation; public-vs-private isolation; logs, cache keys, throughput and memory/cost |
| Seeking | Native HTTP Range baseline; digest/version-bound sparse page index; targeted failing-client demux/index | Paired startup/seek p50/p95, request count, bytes, accuracy and Opus pre-roll on named engines; choose thresholds only after baseline |
| Apple playback | Proven native AVPlayer; audited small native Opus adapter; separately approved external-producer compatible packaging | Actual target device/OS start/seek/pause/interruption/refresh observations; quality, file size, maintenance/licensing; WebKit support is not AVPlayer evidence |
| Web buffering | Native metadata preload/player-driven requests; early preload hint; measured custom media chunking | Named browser/OS and throttled-network traces for startup/rebuffer/requests/bytes; no mandatory 64–128 KiB prebuffer |
| Cache security | No-store protected path; entitlement-checked edge cache; publicly cacheable content only under an accepted withdrawal policy | Exact cache key, auth-at-edge/revalidation, expiry/retraction lag, signed-query log redaction and no cross-grant access |

## Consequences

- Product: reviewable transport groundwork; publication streaming and client/native acceptance remain pending.
- Compatibility: existing Theatre JSON DTOs and schemas are unchanged; trusted Rust adapters must use
  `MediaGrant::verified_for(&permit, ...)`. No migration, Protobuf expansion or audio rewrite occurs.
- Operations/cost: no provider call, upload, publication, CDN purge, deployment or credentials.
- Recovery: dropping a response drops its source and held concurrency lane. Oversize/truncation,
  dependency error or deadline terminates the body with a typed failure; no successful stub/retry.
- Bounds: one demand-polled upstream chunk is retained; upstream frame allocation remains the
  adapter's responsibility. `Body::from_stream` proves neither zero-copy nor a total-RSS ceiling.

## Validation and revisit conditions

The contract fixture uses locally owned arbitrary transport bytes with actual SHA/length and
named synthetic publication/SQL authority. It is compiled only for tests and rechecks eligibility
before every GET/HEAD/conditional result. It does not prove a playable Ogg/Opus profile or real
storage isolation. Production composition must not copy this fixture.

Before selecting or activating any alternative, #10/#11 owners must record admission samples,
provider topology/version, exact commands and redacted real delivery observations. #13/#14 owners
must supply actual browser/native evidence; #51/#64 own profiles and any alternate rendition;
#16 owns completed offline lifecycle. See the [evidence/deferred acceptance ledger](../evidence/playback-range-transport.md).
