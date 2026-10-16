# Content Identity → Production Brief handoff

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/content-identity-production-brief-handoff.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

This is the PR 1 implementation boundary, not execution status for a broad issue.
Authoritative model: [ADR 0019](../../decisions/0019-content-identity.md).
API: [Content Identity v1](../../../packages/contracts/content-v1.md).
Evidence and limits: [content identity](../../evidence/content-identity.md).

## Available to session 2

- Catalog Work/ContentUnit identity, owner/kind, metadata history and membership stay intact.
- `catalog::domain::content` supplies admitted immutable revision values, distinct revision IDs,
  content/full-revision digests, normalized manuscript text and a pure sequence decision.
- `Store::append_content`, `content_revision`, `content_history` implement Studio authorization,
  exact owner-bound source/Script IR admission, atomic receipts and bounded reads.
- Source records and Script IR exports retain their existing identity, bytes and digests.
  Adaptation continues accepting into Editorial; binding to a Catalog revision is explicit.
- Migration **0012_content_identity.sql** adds content revisions/heads and immutability gates.
  Intake owns **0010**; Catalog owns **0011**. This branch registers 1–9, 11, 12. When intake
  joins develop, reconcile the registry to 1–12/count 12 and retain both upgrade regressions.
  Never edit an applied SQL file or infer that the lower absent version already ran.

## Required next boundary

Implement an immutable versioned Production Brief that pins Work, ContentUnit,
ContentRevision UUID and `cc-r1`, plus exact guidance/profile/lexicon/casting revisions where
applicable. The body variant chooses ordinary text, preserved source, exact Script IR or a
recording brief. No cast/scene hierarchy is required for prose or an already-recorded podcast.
Copy the reviewed inputs into the immutable brief, or pin immutable owning-context records
with digest revalidation. Source briefs also pin the immutable extraction receipt/extractor
version used to derive instructions. Script IR retains its own effective line languages; wrapper
language must never silently override them. Never resolve content/latest or legacy Catalog links
during delivery.

Producer authorization remains a separate scope from Studio cookies. A Work reader does not
automatically gain underlying editorial/ingestion endpoint access; decide the brief's authorized
byte-resolution path explicitly and prove cross-owner isolation. Use existing context public
entrypoints rather than querying another context's tables or introducing generic repositories.
Changes to content, rights or guidance make an old brief stale by comparison; they never
retarget or rewrite it. Rights references and attribution notes are declarations, not approval.

Keep external production under ADR 0018. No TTS/provider dispatch, worker, spending reservation,
mixer, upload framework or publication lifecycle was added by PR 1. Independent draft PR69
contains intake issue64 work; it has not been used as evidence of upload/publication support.
Inspect its actual merge state before session 3 and coordinate its identity linkage separately.

## Compatibility cautions

Existing units do not acquire content through migration. Explicitly append an authorized
revision; do not backfill a draft navigation link into accepted content. Catalog `source`,
`script`, `content_digest` are compatibility metadata, and changes to them do not move content
head. Embedded Script IR work/episode identifiers are independent of Catalog UUIDs.
Use `cc-r1` when pinning provenance/rights; equal effective `cc-c1` never renews an approval.
The global Catalog write/read fence still imposes initial-scale contention and cursor
invalidation. Production scale, external producer auth and Studio content editing UI remain
separate acceptance, with no permission implied by this handoff.
