# ADR 0033: Release-pinned public Work cover (#157 P0)

- Status: **accepted (owner, #157, 2026-10-25)** — policy and Discovery v1.2 contract. The
  Server backend (disclosure persistence and [Studio route](../../services/cantos-server/contracts/studio/cover-disclosure/v1/README.md),
  `presentation=2` projection and the composition-gated origin cover route) is implemented by
  [#171](https://github.com/loveoverflowcom/cantos/issues/171); Web/CMP adoption is #158/#159
- Date: proposed 2026-10-16; accepted 2026-10-25
- Decision owner: Cantos product owner (selected all four #157 P0 options in the delivery
  session for [#157](https://github.com/loveoverflowcom/cantos/issues/157)); deployment owner
  retains activation acceptance
- Related issue or PR: #171 (implementation), [ADR 0030](0030-release-safe-public-metadata.md)
  (P1 metadata matrix), [ADR 0032](0032-theatre-origin-playback-delivery.md) (audio origin mode)
- Supersedes / superseded by: N/A; [Artwork v1](../../services/cantos-server/contracts/studio/artwork/v1/README.md) stays owner-only

## Context

Studio has a private, owner-only Work cover. Each set/clear is an immutable revision-bound
change that advances the Catalog metadata revision, and `artwork_at(work, changes)` resolves
the cover at an exact Work revision; a clear is a tombstone. Every audio release already pins
the Work metadata revision it was published against, and Theatre projects title/description
from that pinned revision. `WorkSummary` and `PresentedWorkSummary` have no cover field, and
their strict readers (Server, Theatre Web, CMP) reject unknown fields. Private admission carries
rights references and provenance that must stay private. Theatre Web and CMP already render a
deterministic kind-glyph fallback and invent no artwork.

Facts at `develop@e082d2d8`: Server persistence is embedded Turso (`server.db`); the pure
`public_cover` kernel and the draft Quint capability `cantos.media.public-cover-projection`
exist; no shell supplies a disclosure, so no cover can be shown. ADR 0032's origin route serves
audio only, and no promoted public image object, CDN or provider presigned read exists.

## Decision

The owner accepted the four recommended options. Every rule below is policy; nothing here
claims the runtime that #171 must still build.

1. **Which image.** The public cover candidate is the artwork effective at the Work metadata
   revision pinned by the eligible representative release, selected by the same algorithm as
   title and description — never the current private head. Replacing or clearing the private
   cover after publication changes nothing public until a newer eligible release pins it.
   Withdrawal, loss of public visibility, archival or owner disablement remove the cover with
   the release, because only an eligible `ListenerPublication` may request one.
2. **Who discloses.** Showing that image additionally requires an explicit **public-display
   decision for that exact image asset digest**, recorded by the authenticated current active
   Work owner through Studio. It is an append-only fact, separate from private admission, Work
   visibility, distribution clearance and audio publication approval; none of them implies it,
   and it implies none of them.
   - A clearance names the image asset digest and a required `valid_until_ms` strictly after
     its recording time; disclosure is never perpetual by default. Expiry is half-open.
   - A revocation names the image asset digest and takes effect for the next request.
   - The latest decision by server sequence for that Work and digest wins; caller timestamps
     never order decisions. Renewal or re-disclosure is a new decision.
   - Delegates, producers, Admin and Theatre credentials gain no disclosure authority by
     inference. A decision about another image is a binding defect, never consent.
3. **Projection is total.** Missing, foreign, corrupt, undisclosed, revoked or expired cover
   facts, or an unavailable cover delivery, yield "no cover" without affecting title, units or
   playback. The public response never says why.
4. **Public facts.** The wire exposes only the delivery URL, byte SHA-256, MIME, pixel
   dimensions, byte length and the alt text bound to the pinned selection. The image asset ID
   and asset digest stay server-side, as do rights references, provenance, owner, intent,
   receipt, disclosure history and storage key. Raw image bytes never enter a metadata envelope.
5. **Delivery: origin, revalidated on every use.** Bytes are served by the Server at
   `GET|HEAD /api/v1/theatre/works/{work_id}/cover/{sha256}`. Every request re-resolves the
   representative eligible release, re-projects the cover with the current disclosure and clock,
   and serves bytes only when the projection shows exactly that SHA-256. Responses carry a
   strong `ETag: "{sha256}"` and `Cache-Control: public, no-cache`, so caches must revalidate
   before each use and a revocation or withdrawal is effective on the next request. Every
   denial reason is the same anonymous `not_found`. No grant token, CDN, purge, thumbnail,
   resize or transcode is selected.
6. **Wire: opt-in Discovery v1.2.** The closed `presentation` query admits a new value `2` on
   `GET /works`, `GET /search` and `GET /works/{work_id}`, returning `CoveredWorkPage` /
   `CoveredWorkSummary`: every `PresentedWorkSummary` field plus a required nullable `cover`.
   `null` is the only "no cover" representation. Default v1 and `presentation=1` responses stay
   byte-for-byte unchanged; the capability is part of cursor identity. The units route accepts
   `presentation=2` and answers exactly as for `presentation=1`. Theatre stays JSON; no
   Protobuf message or field number is selected.

The [Discovery v1 contract](../../services/cantos-server/contracts/theatre/discovery/v1/README.md#public-cover-v12-composition-gated)
owns the exact field shapes, fixtures and HTTP table. The pure kernel owns rules 1–3 and the
delivery admission of rule 5 (`public_cover::project_public_cover`, `admit_cover_fetch`).

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Release-pinned cover + owner per-image disclosure | Reproducible per release; private replacement isolated; consent is a separate, revocable, expiring fact | Needs a disclosure record, Studio action and release-time UX; corrections need a new release or a disclosure change | **Selected** |
| Disclosure folded into audio publication approval | One owner action | Cannot revoke a cover without withdrawing audio; conflates audio and image rights | Rejected |
| Current private head shown when the Work is public | Simple, immediate edits | Leaks drafts, breaks release reproducibility and cache identity | Rejected |
| Origin route, revalidate every use | Revocation effective on the next request; cheap 304s; reuses origin storage and transport | One origin round trip per image use; no shared-cache offload | **Selected** |
| Origin route, `max-age=300` | Fewer requests | Up to five minutes of post-revocation display in browsers and shared caches | Rejected for the first slice |
| Short-lived grant URL like audio | Tight revocation | `<img>` URLs in lists expire after 120 s; per-card grant issuance | Rejected |
| Public bucket/CDN | Offload | No provider presigned/public read; revocation needs purge; no cache evidence ([ADR 0038](0038-publication-streaming-evidence-gates.md)) | Rejected now |
| `presentation=2` opt-in | Old strict readers unchanged; one request per page | Another closed capability value and fixture set | **Selected** |
| Separate `/works/{id}/cover` metadata endpoint | No DTO change | N+1 requests per page | Rejected |
| Discovery v2 routes | Clean break | Duplicates every route, cursor and fixture for one field | Rejected |

## Consequences

- Product and user impact: none until #171 ships; consumers keep the stable fallback. After
  #171 an owner must explicitly disclose each cover image, and a newer release is needed to
  move the public cover to a newer private image.
- Compatibility and migrations: additive. #171 appends the disclosure table at the next free
  `server.db` migration version; registered versions are never edited. Old releases without a
  pinned image or disclosure keep `cover: null`; nothing is backfilled from private heads.
- Operations, storage and production cost: image bytes are read through the existing private
  object adapter by the origin route; no new service, CDN, public bucket or provider call.
- Implementation and recovery requirements (#171): disclosure persistence and Studio route,
  bounded batched reads of pinned history plus disclosures for a whole page (no per-item
  query), composition-gated delivery that answers `cover: null`/`not_found` when the host does
  not compose it, actual-byte SHA/length checks on read, and the acceptance matrix in the
  [cover handoff](../work-plan/public-cover-implementation-handoff.md#acceptance-matrix-for-the-later-implementation).
- Deferred questions: whether CMP may keep a cover for offline downloads beyond revalidation,
  exact CORP/CORS headers for the deployed origins (ADR 0041), Studio disclosure UX copy, and
  any later shared-cache lag; each needs its own decision when its consumer exists.

## Validation and revisit conditions

The pure kernel's example, bounded-exhaustive and registered (not run) Kani checks, the
`admit_cover_fetch` tests and the draft Quint capability exercise rules 1–3 and 5. Discovery
v1.2 schema/DTO/fixture conformance tests exercise rule 6. #171 adds file-backed `server.db`
tests through the production Studio services, `ListenerService` and HTTP router over a real
private filesystem object root (rules 1–6). Deployed TLS/proxy/CDN behavior, CORP/CORS, browser
and device behavior remain NOT_RUN until their owners (#158/#159, deployment). Revisit if a shared cache or CDN is proposed, if the owner wants live
editorial cover corrections without a release, or if image rights expiry must bound clearance.
