# ADR 0030: Release-safe public metadata decision gate

- Status: **accepted in part (owner, #157, 2026-10-25)** — option B is the authority principle
  for every future public metadata field; genre/taxonomy is open and every other P1 family is
  deferred (see [#157 capability matrix](#157-capability-matrix)). No P1 field, contract or
  runtime is accepted
- Date: proposed 2026-10-15; partly accepted 2026-10-25
- Decision owner: Cantos product owner (#157 delivery session); vocabulary and per-family
  owners still to be named when each slice is selected
- Related issue or PR: [#177](https://github.com/loveoverflowcom/cantos/issues/177),
  prerequisite [#157](https://github.com/loveoverflowcom/cantos/issues/157)
- Supersedes / superseded by: N/A; existing Discovery v1 remains authoritative

## Context

The [source-bound review matrix](../work-plan/public-metadata-decision-handoff.md) inventories
the implemented public contract and the proposed P1 consumers. Genres/categories, public
contributors/credits and podcast seasons have no accepted authoring, rights or release contract.
The private [artwork scope](../archive/superseded-plans/012-private-work-artwork.md#deferred-scope-with-a-concrete-trigger-rather-than-placeholder-fields)
explicitly deferred them. Prototype labels are not production fields or permission to publish
private production casting. Issue #157 requires an owner-reviewed P1 matrix before implementation.

Discovery already resolves an eligible release and reads its exact Work/unit revisions. It
already has q/kind/language filters, immutable listing anchors, bounded pages and opaque cursors.
Its current JSON DTOs deny unknown fields; optional additions are therefore **not automatically
backward compatible**. No existing Theatre Protobuf field numbers can be inferred from the
Studio transport pilot or the design prototype.

## Decision

**Accepted as a principle (owner, #157):** use option B, release-pinned metadata,
for the first independently reviewed genre/filter slice. Preserve current visibility, withdrawal,
rights and owner checks as live eligibility gates. A pin preserves a fact, not a permanent right
to disclose it. Do not change existing title/language/order semantics to implement this proposal.

Do not implement P1 types, tables, migrations, protobuf fields, endpoints or client adapters
until the owner records the chosen rows below for that slice. #157 accepted only option B and
the status matrix above; the per-slice rows remain to be decided. No prototype
runtime is included or requested by this decision package. The independent
[existing Discovery v1 continuation repair](../work-plan/public-metadata-decision-handoff.md#existing-discovery-v1-continuation-repair)
in the accompanying PR changes no P1 choice or contract.

| Decision to record | Recommended starting point | Still required before its implementation |
| --- | --- | --- |
| Metadata authority | B: exact immutable metadata revision selected with the eligible release; no implicit live editorial override | Accept A/B/C; decide cross-unit representative replacement, correction and legacy-release behavior |
| Taxonomy and localization | Persisted, bounded curated vocabulary with stable IDs; revisioned assignments and explicit label/localization owner, not a Rust enum copied from mock labels | Name vocabulary curator and Work-assignment writer separately, permitted values, locale/fallback policy and measured cardinality/input bounds; decide whether genre and category are distinct |
| Filtering and facets | Extend the existing works/search path only for admitted keys; OR within a genre selection, AND with existing q/kind/language; preserve representative-first selection | Accept ANY versus ALL and canonicalization; approve limits, unknown/retired-ID handling and cursor version boundary; require a real consumer and cost evidence before a facet/count endpoint |
| Public attribution | Separate public, revision-bound attribution from Studio ownership, producer identity and private casting | Select public roles and Work/unit inheritance; record display-name publication authority, current revocation semantics, and whether loss of mandatory attribution suppresses a field or denies the release |
| Podcast grouping | Optional explicit podcast-only grouping; no inferred Season 1 and no reinterpretation of unit position | Decide group identity/order/display numbers, ungrouped placement, cross-release consistency and historical reorder policy; retain flat novel/story chapter order |
| Wire evolution | Preserve current v1 shape and strict readers while reviewing an explicit version/capability boundary | Select version/negotiation mechanism, presence/null/empty semantics, old-client fixtures and a Web/CMP shared decoding/drift-check method; no guessed JSON or protobuf field definitions |

The first proposed implementation outcome is intentionally narrow: a released Work can expose
and filter its explicitly approved genre assignments without altering old-client responses or
leaking draft metadata. Credits and seasons remain separate decisions/slices; they are not
placeholder fields in that first outcome. The owner may instead choose a different first
capability. The matrix and acceptance plan must then be updated before coding.

### Minimum gate for the first genre slice

For that narrow outcome, the applicable authority, taxonomy/localization, filtering and wire
choices need explicit owner acceptance; #157 settled only the authority principle. Attribution
and podcast grouping are deferred and must stay absent from the new runtime contract. The
accepted P0 cover decision (ADR 0033) is not a metadata decision and waives no prerequisite of
the selected slice. Record the exact applicable gate and source/consumer evidence through the matrix's
[owner review worksheet](../work-plan/public-metadata-decision-handoff.md#owner-review-worksheet).

An accepted genre slice must include a usable, authorized source of assignments, not just a
public read type with invented fixture data. Vocabulary curation and assigning vocabulary IDs
to a Work are different authorities. No existing Studio ownership, delegate action or producer
grant silently authorizes a new global taxonomy operation. Exact authoring actions and their
revision/retry semantics still need review before selecting their implementation.

## #157 capability matrix

The owner accepted option B as a principle and the status column below. **Accepted** is policy
within the stated scope; **open** is the next candidate whose listed decision is still missing;
**deferred** needs the listed trigger before any design or field. A status grants no wire field,
table, migration or client adapter; each accepted runtime slice still needs its own contract,
behavior evidence and consumer. "Pinned" means fixed by the eligible representative release.
WorkKind stays `novel`/`story`/`podcast`; design labels such as "Audiobook" add no enum value.

| Capability and user task | Scope | #157 status | Today / proposal | Absence and optionality | Privacy and rights | Persistence / transport | Behavior acceptance | Consumer and order |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Title, synopsis, kind, audio language, first-publication order, `unit_count`/`ordinal`; find and browse released Works | Work/unit at pinned revisions; language from the release | Accepted, implemented (v1, v1.1) | [Discovery v1](../../services/cantos-server/contracts/theatre/discovery/v1/README.md) | Required; empty synopsis is a value | Projection never reads WorkDetails/Studio DTOs, rights or owner | `server.db` revisions; JSON | Existing projection/cursor tests; native HTTP parity is #333/#335 | Web/CMP today; unchanged |
| Public cover; recognize a Work | Work, pinned by the representative release; exact image digest | **Accepted** ([ADR 0033](0033-release-pinned-public-cover.md)) | Discovery v1.2 `presentation=2`, origin cover route | Required nullable `cover`; `null` for every no-cover reason | Owner per-image disclosure, expiring and revocable; no asset/rights/provenance facts | Disclosure table (#171); JSON metadata, raw bytes at origin | Kernel + `admit_cover_fetch` tests, draft Quint capability; SQL/HTTP/storage NOT_RUN | #171 → Web #158, CMP #159 |
| Authority for any new public metadata field | Release/revision | **Accepted** (option B) | Pin facts with the representative release; current visibility, withdrawal and rights remain live gates | — | A pin preserves a fact, not a permanent right to disclose it | Per field | Per slice | Every P1 slice below |
| Genre/taxonomy; browse and filter by topic | Work assignment pinned with the release; vocabulary entries | **Open**: next candidate | Persisted curated vocabulary with stable IDs (no enum from mock labels) | Not authored vs explicitly none vs withheld must be distinguished before a field | No private tags, hidden-Work counts or existence probes via filter errors | Vocabulary + assignment tables; extend works/search under a new capability value | Decide vocabulary curator vs Work-assignment writer, label localization, ANY/ALL, retirement; then the [worksheet](../work-plan/public-metadata-decision-handoff.md#owner-review-worksheet) | #177 → #158/#159 filters |
| Contributors/roles (author, narrator, host, performer); see who made it | Work or unit; distinct from owner account, cast and producer | **Deferred** | None | Absent ≠ anonymous | Display-name consent and withdrawal; no provider voice IDs or private names | — | Trigger: roles, Work/unit inheritance and consent/revocation decision | After genre; #177 |
| Public credits/license text | Work/unit statement | **Deferred** | None | Absence is not a license grant | Never inferred from distribution clearance | — | Trigger: approved statement source and mandatory-attribution policy | Named detail consumer first |
| Podcast season / episode number | Podcast-only grouping; separate from ContentUnit position | **Deferred** | None; `ordinal` is a display rank only | Podcasts may have no season; novel/story stay flat | Groups may contain only eligible units | — | Trigger: group identity/order, ungrouped placement, reorder policy | #177 after genre |
| Timed markers; jump within one audio | Exact rendition/release | **Deferred** | None | Optional | Independent admission/publication | Separate asset | Trigger: timing units, duration bounds, stale-marker policy | Player slice (#12/#13) |
| Public transcript; read or follow along | Separate text asset per language/MIME/version | **Deferred** (#178) | None | Optional | Separate text publication rights; never private Script IR | Separate asset | Trigger: plain vs timed, rights and accessibility owner | #178 |
| RSS/XML feed | — | **Rejected here** | #104 stays research only | — | — | No feed runtime | — | Backlog |

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| A: mutable live Catalog plus publish snapshot | Fast editorial corrections; familiar Studio authoring | Public values/filter membership can change without release review; must define which snapshot wins and how old clients/cache/cursors behave | Not recommended for first slice; no safe editorial override policy exists |
| B: release-pinned projection | Matches current title/synopsis revision authority; reproducible attribution and genre assignment | Corrections require a new reviewed binding/release; vocabulary labels and cross-unit representative changes still need policy | Accepted as the authority principle by #157; each field still follows the capability matrix above |
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
  per accepted slice, then bounded batched SQL and real `server.db` failure/concurrency evidence.
  Failure must leave the prior active release and immutable listing anchors intact.
- Deferred questions: all unaccepted rows above, separate timed
  markers/transcript/license capabilities and real Web/CMP/public delivery acceptance.

## Validation and revisit conditions

Use the matrix's [behavior and evidence plan](../work-plan/public-metadata-decision-handoff.md#behavior-and-evidence-plan)
after the owner accepts exact semantics. It includes multi-genre selection, representative
replacement, cursor/query mismatch, private-name sentinels, podcast without seasons,
reorder/publish/withdraw and concurrent pagination. Proposed scenarios are not a new accepted
Quint capability or implementation evidence. Revisit B if an accepted editorial correction
requirement cannot be served with a new immutable binding, or if measurements justify a separate
facet read model. Repository hygiene validates this document, not the proposed runtime.
