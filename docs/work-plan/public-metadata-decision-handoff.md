# Public metadata: owner decision and implementation handoff

This is a **proposal and source inventory**, not an accepted P1 contract or a prototype runtime.
It supports [#177](https://github.com/loveoverflowcom/cantos/issues/177) after the applicable
[#157](https://github.com/loveoverflowcom/cantos/issues/157) decision gate. The
[proposed ADR 0030](../decisions/0030-release-safe-public-metadata.md) owns the alternatives and
six decisions to record; this file owns their field/consumer inventory and proposed evidence
boundary. GitHub owns execution status and acceptance. Neither document closes either issue.

## Review boundary and baseline

This package delivers a reviewable choice of metadata authority, a field-to-source/release/
rights/consumer matrix, compatibility hazards and a bounded implementation/evidence sequence.
It delivers **no new DTO, protobuf field, schema, migration, endpoint, query or client adapter**.
The independent [Discovery v1 continuation repair](#existing-discovery-v1-continuation-repair)
corrects an existing application guard; it does not implement or accept P1 metadata.
The first recommended runtime slice is genres plus existing works/search filtering, only after
the owner selects the vocabulary and semantics. Public attribution and optional seasons must
not slip into that PR as unreviewed fields.

Source inventory refreshed to `develop@c6353c77b59161b2600d83c330db9c01c334e0ec`, including
Membership proposal #182, Studio private artwork #185 and Theatre local-run #186. Earlier
inventories used `a079e72` and `6818642`; the public DTOs/schema/query, projection, revision
facade, listener readers and behavior catalog cited below are unchanged by these integrations.
Issue #177 and #157 were re-read with their comments at 15:24 UTC on 2026-10-15; neither had comments or
linked acceptance evidence. The consumer issues #158/#159 also explicitly defer P1 wiring
until the relevant public capability is accepted.
The following are **inspection facts**, not newly executed runtime tests:

| Implemented authority | Source and observed boundary |
| --- | --- |
| Public Work/unit wire shape | [strict shared Rust DTOs](../../services/cantos-server/src/contract/theatre/discovery.rs), [JSON Schema](../../services/cantos-server/contracts/theatre/discovery/v1/discovery.schema.json) and [literal fixtures](../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/); no genre/credits/season fields |
| Query keys and typed rejection | [HTTP Theatre owner](../../services/cantos-server/src/adapters/inbound/http/theatre.rs): works/search accept q, kind, language, limit and cursor; both query structs deny unknown fields and translate rejection to invalid_request |
| Pure eligibility and query normalization | [Media listening core](../../services/cantos-server/src/contexts/media/domain/listening.rs): exact pinned/current identity checks, current visibility/archive/owner/rights gating; bounded q/language/limit |
| Metadata and media projection | [listener projection](../../services/cantos-server/src/contexts/media/listener_projection.rs): released Work/unit text, actual released audio language, immutable listing anchors; no private envelope fields |
| Revision authority and batched reads | [Catalog facade](../../services/cantos-server/src/contexts/catalog/turso/listener.rs) loads exact release-pinned revisions plus current eligibility snapshots, checks their digests and bounds requests to 51; no per-item Catalog query |
| Representative selection and indexing | [Media Turso adapter](../../services/cantos-server/src/contexts/media/turso/listener.rs): newest eligible active release per Work by published time descending and release ID ascending, then filters; immutable first-publication/first-position anchors |
| Cursor binding | [cursor vault](../../services/cantos-server/src/contexts/media/listener_cursor.rs) binds the normalized DiscoveryQuery and typed keyset; 15-minute expiry, 4096 entries, restart invalidation, no query/private identity in token |
| Current live Web consumer | [discovery reader](../../apps/cantos-theatre/web/src/live/discovery.rs) fetches works with limit 20; [public shelf](../../apps/cantos-theatre/web/src/features/public_home.rs) renders kind/title/description. Mock Explore genres and mock episode labels are not live consumer or taxonomy evidence |
| Existing behavior catalog | [Quint catalog](../../specs/behavior/catalog.json) contains private audio admission, publishing lifecycle and progress, not a public metadata capability; no metadata model acceptance may be inferred |

Retain [Discovery v1](../../services/cantos-server/contracts/theatre/discovery/v1/README.md) and
[Publication v1](../../services/cantos-server/contracts/studio/publication/v1/README.md) as the actual contract authorities.
The [private artwork plan](../archive/superseded-plans/012-private-work-artwork.md#deferred-scope-with-a-concrete-trigger-rather-than-placeholder-fields)
requires a separate use case/decision for each P1 family. Its historical private-cover baseline
is not current public metadata acceptance. Do not reimplement q/kind/language search v1.

The newer [Studio private artwork UI](../evidence/studio-private-artwork-ui.md) consumes the
existing owner-only artwork contract; it grants no public image or metadata disclosure. The
[Theatre local-run repair](../evidence/theatre-local-run.md) adds actual API/proxy catalog
readiness, including a valid empty page; startup is not a nonempty publication, P1 metadata,
public delivery or client acceptance test. The separate
[Membership publication proposal](../product/membership-plan-publication.md) also remains a
proposal, not a commercial or metadata source. None supplies #157's missing P1 decision.

## Existing Discovery v1 continuation repair

The [existing pagination contract](../../services/cantos-server/contracts/theatre/discovery/v1/README.md#opaque-keyset-pagination)
allows an empty page with a next cursor when eligibility changes during a bounded read.
The [Turso reader](../../services/cantos-server/src/contexts/media/turso/listener.rs)
retains the scanned continuation after its final expiry check. Previously,
[`ListenerService::units`](../../services/cantos-server/src/contexts/media/listener_application.rs)
discarded that continuation on an empty first page by returning `not_found`.

The application now returns `not_found` on an empty initial unit page only when the read also
has no continuation. An empty page with a cursor can resume through the existing Work-bound
vault and a fresh read. A later page may legitimately be empty and terminal. Originally
private or absent records still have no eligible scanned continuation; this repair selects no
new disclosure policy, source, field or wire version.

`empty_first_unit_page_preserves_continuation_and_cursor_scope` reproduces the former
`NotFound` result before the repair and exercises the real application, vault and projection
with the named `UnitWalkRead` double afterward. It covers both a visible next unit and a
terminal empty continuation, exact resumed Work/keyset/limit, and wrong Work/route/limit
rejection before another read. `empty_catalog_and_private_or_absent_release_are_distinct`
also checks the empty-initial/no-continuation `NotFound` boundary.

These are native Rust **mock-integration-tested** application scenarios with synthetic
publication facts. They do not establish a PostgreSQL expiry race, live HTTP or Web/CMP
pagination acceptance. The independent review and final local gate results belong in the PR;
all proposed P1 scenarios below retain their own decision and implementation gates.

## Field, source, release, visibility and consumer matrix

“Candidate” means an owner must approve the source/behavior before any field is encoded. The
names below describe concepts, not selected wire names, types, protobuf tags or SQL columns.
W01–W03/M01–M06 are the design ranges cited by #177, not evidence that either client consumes
these proposed fields. #158/#159 own the exact Web/CMP mapping and runtime acceptance.

| Field / capability and user task | Source / scope | Release binding / optionality | Visibility / rights requirement | FE consumer / disposition |
| --- | --- | --- | --- | --- |
| Work identity, kind, title and synopsis; find the released work | Existing Catalog Work; novel/story/podcast only | Exact metadata revision of representative eligible release; current required v1 fields | Existing ListenerPublication eligibility; never draft head or Studio details DTO | Existing Discovery contract for #13/#14; preserve unchanged |
| Language; filter by the audio heard | Existing release's Content/audio language, not Work editorial language | Exact release manifest; current required v1 field | Same eligible representative as title and filters | Existing q/kind/language path; preserve unchanged |
| First publication and unit order; stable browse walk | Existing listener Work/unit index anchors | First committed publication/position; not latest release time or current draft order | Re-evaluate eligibility on each page; anchors survive withdrawal | Existing cards/unit list; no ranking, season or episode number inference |
| Genre/category identity; browse by topic | Candidate product-owned bounded vocabulary and Work assignments; no production source exists | Recommend immutable assignment and vocabulary/label revision pinned with selected release; missing is not an invented genre; empty versus unavailable needs acceptance | Only approved public assignments; no private editorial tags or hidden-work counts | Proposed W01–W03/M01–M06 chips/filters under #158/#159, discovery #13/#14; first candidate slice after vocabulary decision |
| Taxonomy display label/localization | Candidate vocabulary-owned labels, independent from UI resource locale and audio language | Decide pinned label revisions versus explicitly versioned live labels, locale request and fallback; ID stays stable | Label changes must not silently disclose a hidden/private taxonomy; labels never grant access | Web/CMP resolve the same public ID/label contract; no label-derived filter key or per-client taxonomy copy |
| Work contributors / public credits | Candidate independently authored public attribution with grant/evidence; not owner account, private cast or producer binding | Recommend explicit revision-bound display name/role facts; Work/unit inheritance and optional/withheld states need acceptance | A distribution claim does not grant permission to publish a person's name; check grant scope and current withdrawal policy separately | Proposed detail/credit surfaces #158/#159; defer until roles, authoring and privacy/revocation decisions |
| Unit contributors / performance attribution | Candidate unit-specific public attribution; no implicit cast-to-credit conversion | Exact unit/release binding; decide override/addition/ordering and representative changes; absent means no approved display, not “anonymous author” | Never expose provider voice IDs, private names, contact details, rights references or evidence | Proposed unit details on Web/CMP; a Work-level credit must not misattribute another episode |
| Optional podcast season / episode display | Candidate explicit podcast editorial grouping separate from unit identity and position | Podcast may have no season; decide group revision, ordering, ungrouped placement and display numbers; novel/story stays flat | Public groups may contain only currently eligible units; never reveal private episode existence/count | Proposed podcast details #158/#159; defer until reviewed grouping and public reorder policy; no Season 1 fallback |
| Public credits/license text | Candidate expressly approved public attribution/license statement, not private distribution clearance | Separate revision and publication scope; absence is not a license grant or guessed copyright | Decide exact allowed public text and mandatory attribution/withdrawal behavior; private evidence stays private | Detail/license consumer must be named before schema; separate slice |
| Timed markers | Candidate rendition/publication-bound navigation asset | Exact audio identity, timing units, duration bounds and stale marker policy; optional | Independent admission/publication; never treat ContentUnit as an audio marker | Separate player/navigation slice; not selected by #177 |
| Public transcript | Candidate separately approved plain/timed text asset | Exact language/MIME/revision and release/rendition relation; optional | Separate text publication rights; private Script IR, manuscript/source and production instructions are not transcript endpoints | Separate accessibility/transcript slice; not selected by #177 |
| Public artwork | Separate #157 P0 image projection/delivery authority | Exact approved image/release identity; no image or denial retains truthful fallback | Private artwork admission/owner URL grants no public display right | #158/#159 fallback consumer only until #157 P0 acceptance; not implemented here |

For every new optional field the owner must distinguish at least: not authored, explicitly
empty/not applicable, unavailable/withheld and a supplied approved value. The public response
must not reveal a private reason or evidence. Whether those distinctions are public states,
omission or null is a wire decision, not a requirement to expose all four statuses.

## Owner review worksheet

Record the following evidence in the owning #157/#177 discussion or accepted decision, not as
a second execution-status board in this file. Each accepted item needs the named owner,
exact selected semantics, source/contract revision, concrete consumer and explicit exclusions.
An empty worksheet, a recommendation, a merged document or a passing fixture is not acceptance.

| Evidence required for a proposed genre-only first slice | Decision boundary to resolve |
| --- | --- |
| Scope and dependency receipt | Name the chosen capability and the applicable #157 acceptance evidence; identify any still-blocking dependency. Explicitly defer credits/seasons and all excluded families rather than invent their values or declaring the whole issue accepted |
| Source and write authority | Name vocabulary curator separately from the authorized Work-assignment writer and the real authoring/source consumer. Define allowed actions, revision guard, retries and how the approved assignment reaches a release; do not grant new delegate/global taxonomy rights by inference |
| Release and vocabulary identity | Select A/B/C and bind assignments plus label/localization facts consistently. Under recommended B, pin their accepted revisions; any live-label exception is an explicit authority/compatibility choice, not an implicit escape from B |
| Representative and legacy records | Decide how a newer eligible unit release selects Work metadata and how old releases without genre data remain truthful. No union across unrelated releases, backfill from a draft head or rewrite of historical digests |
| Public vocabulary, removal and privacy | State which IDs/labels are publicly discoverable and what withdrawing a vocabulary entry means for pinned assignments and cursors. Validate unknown values only against the admitted public vocabulary; hidden/private and absent IDs must not become distinguishable existence probes |
| Filter and query budget | Accept ANY/ALL, canonical ID ordering/deduplication, exact typed errors, query/selection bounds and representative-before-filter semantics. Record measured SQL/cardinality budgets and the conditions for any separate facets endpoint |
| Wire and client receipt | Select an explicit compatible version/capability boundary, optionality and literal old/new fixtures. Name the Web/CMP decoding/drift-check consumer and distinguish contract review from actual renderer/native acceptance |

Accepting these applicable decisions can prepare a genre-only implementation boundary while
other families remain deferred. It does not implement the capability, approve a prototype,
accept unrun PostgreSQL/client behavior or waive an unmet dependency. The excluded attribution
and season rows need their own applicable decisions only when their slices are selected.

## Compatibility and query rules to settle before coding

1. **Strict old readers:** current Rust DTOs use deny_unknown_fields and the schema closes
   object shapes. Adding an optional JSON property to current responses can break old clients.
   Do not relax their strictness or silently widen v1; select a version/capability mechanism and
   run old-client fixtures before selecting any new field. Theatre routes currently use JSON;
   the [Protobuf ADR](../decisions/0023-versioned-protobuf-wire.md) does not create Theatre
   messages or authorize guessed tags. Keep media bytes outside all metadata envelopes.
2. **One representative:** select the same eligible representative for work detail, list,
   filtering and any later facet calculations. Under recommended B, another unit's newer
   release can legitimately change Work metadata; decide that explicitly. Do not match genres
   from an old nonrepresentative release and then display a newer nonmatching representative.
3. **One canonical filter:** decide ANY/ALL, ordering/deduplication of IDs, retired vocabulary,
   unknown values and limits. Unknown keys already fail with typed invalid_request; preserve
   that behavior. A well-formed query with genuinely no matches may return an empty page;
   malformed/unsupported input must not masquerade as that result.
4. **Cursor identity:** bind every admitted selector, normalization/version and vocabulary
   semantics to the cursor, alongside existing q/kind/language/limit and route/work scope.
   Changed filter must fail with typed invalid_cursor. Preserve deterministic tie-breaks,
   expiry/capacity bounds and immutable anchors. The current keyset walk is not a frozen
   cross-page snapshot; document insertion/removal/replacement behavior honestly.
5. **Bounded PostgreSQL work:** apply selected-representative filters before limit, avoid
   duplicate Works for multi-genre joins, and retain batched metadata admission. Follow the
   [read-performance owner](../../.agents/skills/cantos-engineering/references/turso-read-performance.md)
   before choosing indexes, scanning JSON, DBSP or another endpoint. Measure eligible and
   ineligible high-cardinality data, query/label length and selection count. No unbounded LIKE,
   per-item metadata/grant lookup or full-catalog facet count without a measured accepted bound.
6. **Privacy before convenience:** serialization and filter/facet results must use the same
   admitted public facts. Display-name revocation can affect historical pin disclosure; owner
   must decide suppression versus whole-release denial for mandatory credits before exposing
   them. Immutable history does not override privacy/rights withdrawal. No automatic union of
   private casting, producer metadata, Studio owner profile or credits from other unit releases.
7. **Ordering remains separate:** a season number or episode display label cannot become a
   unit ID, keyset or position. Current first-public-position semantics survive republish and
   draft reorder. Versioned public reordering/grouping requires an explicit policy and tests,
   not an ORDER BY change hidden in the metadata slice.
8. **Vocabulary lifecycle:** a public filter validator must not look up private taxonomy entries
   and expose their existence through distinct errors, labels, counts or cursor behavior.
   Decide removal/retirement and its effect on pinned assignments before building the read
   index. Any live vocabulary fact affecting matching/presentation requires its own explicit
   version and cursor compatibility policy; immutable release binding alone does not settle it.

## Behavior and evidence plan

All scenarios in this section are **proposed / NOT_RUN for P1**. Existing source tests are
inventory only, not fresh runtime evidence. Model authors must first review requirements
through [cantos-behavior](../../.agents/skills/cantos-behavior/SKILL.md); do not mark a metadata
capability accepted or clone the audio model merely to populate the catalog.

| Claim / failure class | Cheapest independent oracle after decisions | Additional boundary acceptance |
| --- | --- | --- |
| Approved projection only; absent value is truthful | Literal shape/semantic fixtures for every admitted field and absent/empty/withheld state; reject forged revision/source | Real HTTP response plus private-name/provider/storage/rights-evidence sentinels absent from body, errors, cursors, facets and logs |
| No implicit current-head or unrelated-unit attribution | Pin old revision, edit draft, replace one unit release, compare exact expected representative and credits/genres | PostgreSQL publish/replacement/withdrawal and restart; failed replacement leaves prior release unchanged |
| Multi-genre semantics and exact errors | Hand-written ANY/ALL table, duplicate/permuted IDs, unknown/retired IDs, max bounds, mixed q/kind/language and exact errors | Real SQL differential results against reviewed pure oracle; no duplicate works or private facet counts |
| Vocabulary authoring/removal has no hidden authority path | Separate curator/Work-writer allow/deny, stale revision and retry tables; compare absent/private/withdrawn-ID public responses with an independent expected response | Real SQL atomic source-to-release binding, concurrent vocabulary retirement and negative HTTP/error/facet/cursor leakage checks; no timing noninterference claim from matching JSON alone |
| Stable bounded pagination | Cursor mismatch/expiry/restart/type/work cases and equal-anchor tie-break fixtures | Concurrent publish/withdraw/reorder/genre replacement between pages; unchanged anchors once, continue through empty page with next cursor |
| Podcast without season and novel chapters | Explicit no-season podcast, mixed grouped/ungrouped episodes, duplicate display labels and flat novel/story fixtures | Real SQL order/revision rejection and Web/CMP decode/render using exactly the accepted shared fixture |
| Old-client compatibility | Preserve current v1 literal responses/readers; new-version positive and old-reader rejection tests | Version negotiation/unsupported version and actual Web/CMP drift checks; a Rust build is not CMP wire acceptance |
| Bounded query cost and migration safety | Query inventory and old-row snapshots; no applied migration edits or invented old values | Disposable PostgreSQL upgrade/rollback-read plan, constraints, EXPLAIN ANALYZE with measured representative cardinalities and batch/query counts |
| Credits withdrawal cannot leak old names | Exact scope/expiry/revocation truth table, private/public same-name collision and missing-grant denial | Real TOCTOU/revocation reads during publication and paging, plus dependent consumer/cache behavior |

Existing projection tests include
`manifest_projects_exact_release_identity_and_omits_private_envelope` and
`summaries_use_published_audio_language_and_listing_anchors` in the projection owner; the cursor
test `opaque_tokens_bind_query_expire_and_never_encode_internal_values` covers today's selectors.
They are not genre/credit/season or real PostgreSQL acceptance.

## Recommended review sequence and stopping conditions

1. Product/Catalog/Publication owners record the applicable ADR choices and #157's exact matrix
   acceptance for the selected first capability. For genre-only work, use the worksheet above
   and explicitly defer attribution/seasons rather than requiring their eventual design first.
   Name the real authoring/source and Web/CMP consumers. Leave excluded families out of types
   and migrations; no dependency is waived and no owner is assigned by this document.
2. For an accepted genre slice, agree the versioned contract/presence/compatibility corpus and
   pure assignment/filter/revision/error semantics; execute L1–L3 before introducing I/O. Keep
   current v1 responses and the rest of search unchanged.
3. Add only the necessary persistence/index projection and publication binding. Coordinate the
   actual next free migration slot on refreshed develop; expand/backfill without rewriting
   old snapshots/digests, fail closed for unapproved data and prove atomic rollback/retry.
4. Run real PostgreSQL/HTTP/privacy/concurrent pagination and query-cost acceptance. Adopt that
   same contract in Web/CMP under #158/#159 with explicit missing-field fallbacks. No hand-copied
   Kotlin DTO or fixture label in live UI; do not claim UI/native acceptance from backend tests.
5. Open a separate bounded credits or season slice only after its authoring/rights/order
   decisions, accepted contract and immediate consumer exist. Timed markers, transcript, license,
   artwork delivery and RSS/feed stay outside the genre slice.

At runtime HANDOFF use `cargo xtask preflight --changed-only` on the final diff; the current
implemented selectors include `cargo xtask verify rust`, `cargo xtask verify contracts` and
`cargo xtask verify postgres` when additional focused evidence is needed. Follow the repository's
[verification cadence](../../.agents/skills/cantos-engineering/references/verification-cadence.md),
not repeated broad campaigns after each edit. Proposed new test names/benchmarks are not
implemented commands. Owner-reviewed criteria, actual results and every NOT_RUN/BLOCKED boundary
belong in each runtime PR; no CI polling, merge or issue closure is implied.
