# ADR 0051: Metadata-only public discovery without playable audio (#539)

- Status: **accepted direction for the prepared sample only** (owner, 27 October 2026);
  implementation and production acceptance are separate. The existing release-gated API
  remains authoritative until a versioned implementation is accepted and deployed
- Date: 2026-10-27
- Decision owner: Cantos product owner, with Publication and Listening owners for the
  disclosure and playback rules
- Related issue or PR: [#539](https://github.com/loveoverflowcom/cantos/issues/539);
  [#66](https://github.com/loveoverflowcom/cantos/issues/66) (Catalog identity),
  [#11](https://github.com/loveoverflowcom/cantos/issues/11) (audio publication),
  [#427](https://github.com/loveoverflowcom/cantos/issues/427) (sample catalog)
- Supersedes / superseded by: N/A. Would amend [ADR 0030](0030-release-safe-public-metadata.md)
  (release-pinned metadata authority) and [ADR 0035](0035-theatre-catalog-unit-availability.md)
  (anonymous unit list means "listenable now") if accepted

## Context

The [proposed #574 image extension in ADR0033](0033-release-pinned-public-cover.md#proposed-574-metadata-only-extension--not-accepted-or-activated)
adds a separately gated exact-revision image-consent mechanism. It remains disabled in production
composition and does not infer image rights from the accepted metadata direction. Fictional image
permissions and a non-login operator intake path remain prerequisites; audio/playback are unchanged.

#539 asks that Works and chapters/episodes be saved before audio exists, that audio be added
later without recreating them, and that metadata which is allowed to be public be shown by
Theatre with a disabled Play control while no playable media exists. It also reports
`GET /theatre/works` answering `200` with `items: []`.

Confirmed at `develop@939fe317` (source and tests, not production data):

- **Catalog is independent of audio.** `CreateWorkRequest` and `CreateContentUnitRequest`
  (`services/cantos-server/src/contract/studio/catalog.rs:103,127`) carry no audio field.
  Catalog `Lifecycle` is only `draft | archived` and `Visibility` is
  `private | unlisted | public` (`contexts/catalog/domain/metadata.rs:79,107`). Audio
  submissions bind an existing `work_id`/`unit_id`; a release pins the then-current metadata
  revisions (`contexts/media/lifecycle_application.rs:686`). Adding audio creates no Catalog
  identity or revision: `metadata_saved_without_audio_stays_undiscoverable_until_a_release_keeps_its_ids`
  in `services/cantos-server/tests/support/studio_cms_publication_http_turso.rs` exercises this
  over real Studio HTTP and Turso.
- **Anonymous discovery is release-driven by accepted policy.** Every Theatre list, search,
  detail and unit read selects from the eligible-release relation
  (`contexts/media/turso/listener.rs:284-302`): active pointer, current clearance and approval,
  no withdrawal or revocation. `ListenerPublication::resolve` additionally requires pinned and
  current Work visibility `public`, no archival, an active owner and a valid rights window
  (`contexts/media/domain/listening.rs:164-181`). The
  [Discovery v1 contract](../../services/cantos-server/contracts/theatre/discovery/v1/README.md#public-eligibility-and-representative-metadata)
  states that Catalog `draft` is CMS state and an approved active release is the publication
  authority. ADR 0030 makes public metadata release-pinned; ADR 0035 rejected projecting
  unpublished units to anonymous callers.
- **The wire cannot express "visible but not playable".** `UnitSummary` requires
  `publication_id`, `published_at_ms` and `duration_ms`; `WorkSummary` requires
  `first_published_at_ms`; all public DTOs deny unknown fields
  (`src/contract/theatre/discovery.rs`). Theatre Web and CMP therefore never receive a listed
  unit without a release. Web already disables Listen with a reason until the list offers a
  unit (`apps/cantos-theatre/web/src/features/public_work.rs:460`, test
  `listen_is_blocked_with_a_reason_until_the_list_offers_a_unit`); neither client builds a
  media URL itself.
- **`200 items: []` is the designed answer** when no Work has an eligible release, including a
  `public` Work saved without audio (same regression) and the #427 sample's six private drafts
  ([seeded catalog boundary](../deployment/theatre-seeded-catalog.md#public-catalog-and-client-boundary)).
  Whether the reported production response has this cause is **UNKNOWN**: no production
  database or runtime was read.

Assumption to confirm: #539's "metadata allowed to be public" means an explicit owner act, not
the existing `visibility` field, because that field is set at creation, has no rights or
approval record, and already cannot stand in for cover disclosure (ADR 0033).

## Decision

On 27 October 2026 the owner approved option C for the six prepared sample Works and their
chapters: make their metadata public after an explicit rights/source confirmation, while
unavailable audio remains unplayable. The sample's current private/draft state is not itself
publication; it needs the admitted owner action. This is not consent to publish other owners'
content or other private drafts, cover assets or audio. Catalog create/edit without audio needs no change.
The existing release-gated API stays in force until a separate versioned implementation is
verified and deployed; an owner decision alone does not publish data.

On 27 October 2026 the owner additionally selected a non-login principal for this sample.
It owns only the pinned sample identities and has no password, token, session or administrator
role. A separately reviewed stopped-database operator path may act for that principal only
with an explicit source/metadata-rights assertion and an auditable, reversible decision; it
must not simulate a Studio session or weaken ordinary Studio authentication. The selection
does not close registration or authorize production execution by itself. The operator and
database barriers described here remain implementation work until independently tested.

The stopped-file operator records an explicit, self-declared operator reference with the
source and rights assertion; it is not represented as an authenticated human Studio session.
The initial bounded flow publishes once, supports an append-only withdrawal and refuses a
new publication after withdrawal. See the [operator procedure](../deployment/sample-metadata-operator.md).

Recommended option **C**: an explicit, owner-recorded metadata disclosure.

1. A Work disclosure and a per-unit disclosure are immutable, digest-bound records that pin an
   exact Work/unit metadata revision, name the owner who recorded them and carry the metadata
   rights references. Withdrawal is a new record; history is never edited. Visibility
   `public`, non-archival and an active owner stay live gates, as for releases.
2. Discovery reads the union of disclosed metadata and eligible releases. A unit row exposes
   playback only through an eligible release; a disclosed unit without one has no
   `publication_id` and no playback authorization. Metadata shown for a released unit stays
   release-pinned (ADR 0030 B) unless the owner chooses otherwise.
3. Old readers keep today's shapes: the new state needs an explicit version boundary (for
   example a new `presentation` level or a v2 route) whose unit row carries
   `playback: null | {publication_id, published_at_ms, duration_ms}`. Existing v1,
   `presentation=1` and `presentation=2` responses keep listing released content only.
4. Web and CMP render a disclosed but unreleased unit with a disabled Play and a short reason,
   dispatch no manifest, authorization or media request for it, and enable Play only for a row
   that carries an eligible release.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| A. Keep release-gated discovery (current) | No change; no disclosure surface; satisfies ADR 0030/0035 | Saved metadata is invisible to listeners until audio is approved; does not meet #539's display goal | Default until a decision is recorded |
| B. Treat Work `visibility=public` as metadata publication | Small query change | Visibility is a mutable form field with no approval, rights or per-unit control; a draft becomes public by its creation default; contradicts ADR 0033 and the seeded-catalog boundary | Rejected: bypasses the publication gate |
| C. Explicit owner metadata disclosure | Separates metadata admission from playback; revocable and auditable; keeps old readers stable | New records, migration, Studio route and UX, a versioned Discovery shape and Web/CMP adoption; amends ADR 0030/0035 | Owner selected for the prepared sample; broader policy and implementation remain separate |
| D. Studio-only "listener preview" of saved metadata | No public disclosure | Owners see a preview, listeners see nothing | Rejected as the #539 outcome; may complement C |

## Consequences

- Product and user impact: the sample may be disclosed through an implemented and authorized
  production flow; listeners can then see the specifically admitted metadata before audio, with
  an explicit not-yet-playable state. The decision itself changes no deployed data.
- Compatibility and migrations: none now. C needs a new Turso schema version and a versioned
  Discovery shape; strict v1 readers must not receive rows without `publication_id`.
- Operations, storage, and production cost: none now. C adds disclosure records and a wider
  discovery query that needs measured Turso read evidence.
- Implementation and recovery requirements: C must keep private, unlisted, archived, withdrawn
  and undisclosed records indistinguishable from absent ones, and never mint a release,
  placeholder URL or `.invalid` media to make a row look playable.
- Broader product questions not resolved by the sample-only approval: who may record or withdraw a disclosure (owner only or a
  delegate role); whether a Work disclosure implies its units or each unit needs its own;
  whether disclosed metadata is pinned or follows later Catalog edits; ordering and
  `first_published_at` for an unreleased Work; whether search and kind/language filters cover
  disclosed-only Works; cover disclosure for unreleased Works (ADR 0033 is release-pinned);
  rights evidence required for metadata; the version boundary and CMP/Web rollout order.

## Validation and revisit conditions

Evidence for the context: source review at `939fe317` and the regression above
(`example-tested`, real Studio HTTP, embedded Turso file and origin publication storage). Not
covered: production data, the deployed API, browsers and devices.

Implement the approved sample slice with an explicit owner decision, revision-bound records,
independent Work/unit consent, revocation, rights/source evidence and a new Discovery shape.
Keep unresolved broader product choices out of that sample-specific authorization. Review
domain and persistence, Discovery contract, Studio disclosure, Web and CMP with independent
regression and acceptance evidence; do not treat one layer's PASS as completion of the others.
