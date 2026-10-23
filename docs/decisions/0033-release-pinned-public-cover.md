# ADR 0033: Release-pinned public Work cover (proposal for #157 P0)

- Status: **proposed** — executable kernel only; no wire field, storage, delivery or decision source
- Date: 2026-10-16
- Decision owner: unassigned — Product, Catalog and Publication owners via
  [#157](https://github.com/loveoverflowcom/cantos/issues/157)
- Related issue or PR: [#171](https://github.com/loveoverflowcom/cantos/issues/171) (implementation after #157),
  [ADR 0030](0030-release-safe-public-metadata.md) (P1 metadata gate)
- Supersedes / superseded by: N/A; [Artwork v1](../../services/cantos-server/contracts/studio/artwork/v1/README.md) stays owner-only

## Context

Studio has a private, owner-only Work cover. Each set/clear is an immutable revision-bound
change that advances the Catalog metadata revision, and `artwork_at(work, changes)` resolves
the cover at an exact Work revision; a clear is a tombstone. Every audio release already pins
the Work metadata revision it was published against, and Theatre projects title/description
from that pinned revision. `WorkSummary` has no cover field, and its strict readers reject
unknown fields. Private admission carries rights references and provenance that must stay
private. #157 has not chosen the public-display authority or a delivery mode.

## Decision

**Recommendation for #157, not an accepted contract:**

1. The public cover candidate is the artwork effective at the Work revision pinned by the
   eligible representative release — never the current private head. Replacing or clearing
   the private cover after publication changes nothing public until a new release pins it.
2. Showing that image additionally requires an explicit **public-display decision for that
   exact image asset digest** (cleared until a time, or revoked). Private admission, Work
   visibility and audio publication approval are separate facts and never imply it.
3. Projection is total: missing, foreign, corrupt, undisclosed, revoked or expired cover facts
   fall back to "no cover" without affecting title, units or playback.
4. Public facts are limited to asset ID, asset digest, alt text, MIME, pixel dimensions, byte
   length and SHA-256 — no rights references, provenance, owner, intent, receipt or storage key.

Implemented as the pure `public_cover::project_public_cover` kernel, which only an eligible
`ListenerPublication` can call. Today no shell supplies anything but `Undecided`, so no cover can
be shown; no DTO, schema, migration, route, storage path or client change is made.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Release-pinned cover + explicit per-image disclosure (recommended) | Reproducible per release; private replacement isolated; consent is a separate, revocable fact | Needs a new disclosure authority/record and a release-time UX; corrections need a new release or a disclosure change | Matches title/synopsis pinning and #157's "not one boolean" requirement |
| Current private head shown when the Work is public | Simple, immediate edits | Leaks drafts, breaks release reproducibility and cache identity | Rejected |
| Publication approval implies cover disclosure | No new record | Conflates audio rights with image rights; cannot revoke a cover without withdrawing audio | Rejected pending owner input |

## Consequences

- Product and user impact: none now; consumers keep the stable no-image fallback.
- Compatibility and migrations: deferred. A cover field in Discovery needs a versioned wire
  decision (ADR 0030 "wire evolution") and old-client fixtures.
- Operations, storage, and production cost: public image delivery is not chosen; ADR 0032's
  origin model is a candidate for bytes, with its own acceptance gates. No thumbnails/resizing.
- Implementation and recovery requirements: the disclosure record, its writer authority,
  SQL reads of the pinned history, cache identity (asset digest), delivery and denial behavior.
- Deferred questions: who may disclose/revoke, whether disclosure expires, delivery mode,
  Protobuf/JSON field shape, and Web/CMP fallback fixtures.

## Validation and revisit conditions

Kernel example and bounded-exhaustive tests, a registered (not run) Kani harness and the
draft Quint capability `cantos.media.public-cover-projection` exercise these rules. They do not
accept #157. Revisit if the owners choose publication approval as the disclosure authority or
require live editorial cover corrections.
