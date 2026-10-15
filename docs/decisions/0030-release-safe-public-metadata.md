# ADR 0030: Release-safe public metadata decision gate

- Status: proposed — no P1 contract or runtime implementation accepted
- Date: 2026-10-15
- Decision owner: unassigned — product, Catalog and Publication owner review required
- Related issue or PR: [#177](https://github.com/loveoverflowcom/cantos/issues/177),
  prerequisite [#157](https://github.com/loveoverflowcom/cantos/issues/157)
- Supersedes / superseded by: N/A; existing Discovery v1 remains authoritative

## Context

The [source-bound review matrix](../work-plan/public-metadata-decision-handoff.md) inventories
the implemented public contract and the proposed P1 consumers. Genres/categories, public
contributors/credits and podcast seasons have no accepted authoring, rights or release contract.
The private [artwork scope](../work-plan/012-private-work-artwork.md#deferred-scope-with-a-concrete-trigger-rather-than-placeholder-fields)
explicitly deferred them. Prototype labels are not production fields or permission to publish
private production casting. Issue #157 requires an owner-reviewed P1 matrix before implementation.

Discovery already resolves an eligible release and reads its exact Work/unit revisions. It
already has q/kind/language filters, immutable listing anchors, bounded pages and opaque cursors.
Its current JSON DTOs deny unknown fields; optional additions are therefore **not automatically
backward compatible**. No existing Theatre Protobuf field numbers can be inferred from the
Studio transport pilot or the design prototype.

## Decision

**Recommendation for review, not an accepted decision:** use option B, release-pinned metadata,
for the first independently reviewed genre/filter slice. Preserve current visibility, withdrawal,
rights and owner checks as live eligibility gates. A pin preserves a fact, not a permanent right
to disclose it. Do not change existing title/language/order semantics to implement this proposal.

Do not implement P1 types, tables, migrations, protobuf fields, endpoints or client adapters
until the owner records the chosen rows below and #157's applicable acceptance evidence.
This ADR is not that acceptance, including if its documentation PR is merged. No prototype
runtime is included or requested by this decision package.

| Decision to record | Recommended starting point | Still required before its implementation |
| --- | --- | --- |
| Metadata authority | B: exact immutable metadata revision selected with the eligible release; no implicit live editorial override | Accept A/B/C; decide cross-unit representative replacement, correction and legacy-release behavior |
| Taxonomy and localization | Persisted, bounded curated vocabulary with stable IDs; revisioned assignments and explicit label/localization owner, not a Rust enum copied from mock labels | Name vocabulary editor/authority, permitted values, locale/fallback policy and measured cardinality/input bounds; decide whether genre and category are distinct |
| Filtering and facets | Extend the existing works/search path only for admitted keys; OR within a genre selection, AND with existing q/kind/language; preserve representative-first selection | Accept ANY versus ALL and canonicalization; approve limits, unknown/retired-ID handling and cursor version boundary; require a real consumer and cost evidence before a facet/count endpoint |
| Public attribution | Separate public, revision-bound attribution from Studio ownership, producer identity and private casting | Select public roles and Work/unit inheritance; record display-name publication authority, current revocation semantics, and whether loss of mandatory attribution suppresses a field or denies the release |
| Podcast grouping | Optional explicit podcast-only grouping; no inferred Season 1 and no reinterpretation of unit position | Decide group identity/order/display numbers, ungrouped placement, cross-release consistency and historical reorder policy; retain flat novel/story chapter order |
| Wire evolution | Preserve current v1 shape and strict readers while reviewing an explicit version/capability boundary | Select version/negotiation mechanism, presence/null/empty semantics, old-client fixtures and a Web/CMP shared decoding/drift-check method; no guessed JSON or protobuf field definitions |

The first proposed implementation outcome is intentionally narrow: a released Work can expose
and filter its explicitly approved genre assignments without altering old-client responses or
leaking draft metadata. Credits and seasons remain separate decisions/slices; they are not
placeholder fields in that first outcome. The owner may instead choose a different first
capability. The matrix and acceptance plan must then be updated before coding.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| A: mutable live Catalog plus publish snapshot | Fast editorial corrections; familiar Studio authoring | Public values/filter membership can change without release review; must define which snapshot wins and how old clients/cache/cursors behave | Not recommended for first slice; no safe editorial override policy exists |
| B: release-pinned projection | Matches current title/synopsis revision authority; reproducible attribution and genre assignment | Corrections require a new reviewed binding/release; vocabulary labels and cross-unit representative changes still need policy | Recommended, not accepted; minimizes new authority paths |
| C: immutable facts plus selected live editorial fields | Can separate stable credits from safe display corrections | Requires an explicit per-field allowlist, two version identities and cursor/cache invalidation; a live field is not inherently safe | Defer until a concrete correction use case justifies the extra policy |

For taxonomy, a compiled enum requires releases for editorial vocabulary changes; bounded
free-text is simpler to author but lacks stable identity, translation and reliable facets.
A persisted vocabulary adds management/versioning cost but can serve both consumers without
making display labels filter identities. None is accepted merely by listing these trade-offs.

## Consequences

- Product and user impact: no new visible fields or filters now. Missing data remains missing;
  do not invent author, season, genre, license, transcript or marker values in a live UI.
- Compatibility and migrations: none in this PR. A later expand/backfill/reader-switch plan
  must preserve historical snapshots/digests and old DTO fixtures; unknown legacy data must
  not be backfilled from private casting or present-day Catalog heads.
- Operations, storage and production cost: no new service, DBSP view, dependency, provider,
  media transform, RSS/XML/feed, object upload or public delivery activation.
- Implementation and recovery requirements: one typed pure-core rule and consumer contract
  per accepted slice, then bounded batched SQL and real PostgreSQL failure/concurrency evidence.
  Failure must leave the prior active release and immutable listing anchors intact.
- Deferred questions: all unaccepted rows above, #157 artwork authority, separate timed
  markers/transcript/license capabilities and real Web/CMP/public delivery acceptance.

## Validation and revisit conditions

Use the matrix's [behavior and evidence plan](../work-plan/public-metadata-decision-handoff.md#behavior-and-evidence-plan)
after the owner accepts exact semantics. It includes multi-genre selection, representative
replacement, cursor/query mismatch, private-name sentinels, podcast without seasons,
reorder/publish/withdraw and concurrent pagination. Proposed scenarios are not a new accepted
Quint capability or implementation evidence. Revisit B if an accepted editorial correction
requirement cannot be served with a new immutable binding, or if measurements justify a separate
facet read model. Repository hygiene validates this document, not the proposed runtime.
