# ADR 0019 — Content identity beneath the Studio Catalog

- Status: accepted for this bounded implementation under the requester's delegated design authority.
- Scope: Domain Model & Content Identity; production/publication acceptance remains separate.
- Related: [ADR 0001](0001-modular-monolith.md), [ADR 0018](0018-external-audio-production.md).

## Context

Catalog already persists Work and ContentUnit identities, metadata snapshots, ordering and
membership. Editorial persists immutable Script IR exports; ingestion preserves original
source bytes and import receipts; adaptation accepts external proposals into editorial.
Making every unit a Script IR episode would exclude ordinary prose and recorded podcasts.
Treating a mutable Catalog link or its caller-declared digest as content authority would let
metadata edits silently change a later production brief.

## Decision

Retain Catalog as the Work/ContentUnit authority and add append-only ContentRevision records
inside that context, using the existing PostgreSQL store and authorization lane. Introduce no
service, crate, generic repository or provider boundary.

```text
WorkId (owner, kind) → ContentUnitId (one parent, chapter/episode)
                           ↓ 0..many immutable revisions
                    ContentRevisionId + cc-r1 digest
                           ↓ body, one of
                    recording | text | preserved source | exact ScriptRef
                                                            ↓
                                                editorial ScriptRevision
```

The graph describes references, not ownership transfer. Script IR's embedded `work.id` and
`episode.id` remain historical interchange IDs; they are not Catalog UUIDs. A script can be
referenced by several authorized units. Neither a Script ID nor a source ID is re-minted or
rewritten to fit Catalog. References validate the Work **owner**, not just script membership.

| Fact | Single authority |
| --- | --- |
| Work kind/owner; unit parent/kind; metadata lifecycle/order | Catalog existing aggregates/history |
| Unit's accepted content version, text, language, pinned references and declared attribution/rights | Catalog ContentRevision |
| Exact original bytes, checksum, extraction/import receipt | Ingestion |
| Canonical Script IR export, `sir-c1` content and `sir-e1` export digest, accepted revision | Editorial / Script IR |
| Proposal/source comparison and acceptance | Adaptation; accepted output still belongs to Editorial |
| Production brief, accepted media, release | Separate subsequent slices; no implementation here |

UUID identity, sequence, effective content digest and full revision digest are different values.
Metadata history and content sequence have independent counters. A content append advances the
Work guard/epoch atomically but leaves unit metadata snapshots unchanged. Text normalization and
digest schemes are defined by [Content Identity v1](../../packages/contracts/content-v1.md).
Recording content asserts no accepted audio. Imported/adapted/generated attribution and rights
references remain declarations, never clearance or model-quality evidence.

The current content head is mutable under compare-and-swap. Revision rows, sources and script
exports remain immutable. Pure constructors and the content sequence decision reject invalid
values without I/O; application coordinates authorization, source/script/right admission,
clock, receipt and atomic writes; PostgreSQL and HTTP map those decisions.

## Compatibility and alternatives

- Do not backfill content from mutable Catalog links or infer rights/provenance. Existing units
  have no content head until an explicit authorized append. Migration 0012 adds tables only;
  0001–0009 and Catalog 0011 keep their original checksums; intake 0010 remains independent.
- Catalog DTOs/routes and legacy editorial aliases keep their behavior. The optional Catalog
  source/script/digest fields remain compatibility/navigation metadata. They neither mirror nor
  update managed ContentRevision, even when subsequently edited or cleared. New briefs must
  consume ContentRevision, never those fields. Removing them needs separate versioning.
- Rejected alternative: duplicate source/script bodies in Catalog, or move editorial/ingestion
  tables. That creates competing authorities and unnecessary migration risk.
- Rejected alternative: use the unit metadata sequence as a content sequence. Reorder, title
  and archive would falsely change content identity.
- Defer DBSP: these bounded indexed point/history reads have no repeated aggregation. The
  [read inventory](../evidence/content-identity.md) records the local baseline and limits.

## Consequences and next handoff

The global Catalog clock still serializes writes and permission-sensitive reads. This slice
inherits that initial-scale limitation. Content history returns at most 50 summaries with
ascending immutable sequence pagination; access is checked afresh on every page and replay.
No permission is inherited transitively into source/script endpoints.

Production Brief must pin `(WorkId, ContentUnitId, ContentRevisionId, cc-r1)` and any exact
ScriptRef, separately pin guidance/profile revisions, and resolve authorized source/script
bytes through their owning contexts. Equality of `cc-c1` alone never validates approval or
rights. Brief staleness is computed against the current content head; old briefs never retarget.
See the [session 2 handoff](../work-plan/content-identity-production-brief-handoff.md).
