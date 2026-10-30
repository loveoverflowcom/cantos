# ADR 0054: Metadata publication generations and Work/Unit access (#634)

- Status: **accepted (owner, 30 October 2026, #647 decision review)** for D01–D08 as recorded
  in the ledger. Two values remain to be proposed and accepted: the S11 performance budgets
  (D08a) and the S12 RPO/RTO/retention values (D08d). **Not implemented.** Until each slice
  lands, the current runtime and ADRs [0030](0030-release-safe-public-metadata.md),
  [0032](0032-theatre-origin-playback-delivery.md), [0035](0035-theatre-catalog-unit-availability.md)
  and [0051](0051-metadata-only-public-discovery.md) describe actual behavior; ADRs
  [0028](0028-work-scoped-delegation.md), [0033](0033-release-pinned-public-cover.md) and
  [0046](0046-service-owned-contracts.md) are unchanged
- Date: proposed and accepted 2026-10-30
- Decision owner: Cantos product owner (answers recorded per row below); Publication/Media,
  Listening, Catalog and Identity owners implement their rows
- Related issue or PR: [#647](https://github.com/loveoverflowcom/cantos/issues/647) (S01);
  successors [#648](https://github.com/loveoverflowcom/cantos/issues/648)–[#658](https://github.com/loveoverflowcom/cantos/issues/658);
  related [#539](https://github.com/loveoverflowcom/cantos/issues/539),
  [#633](https://github.com/loveoverflowcom/cantos/issues/633),
  [#384](https://github.com/loveoverflowcom/cantos/issues/384),
  [#568](https://github.com/loveoverflowcom/cantos/issues/568)
- Supersedes / superseded by: when implemented, this record makes metadata authority
  generation-pinned for every Work (amends ADR 0030 option B), replaces ADR 0051's sample-only
  disclosure and `presentation=3` with generic publication (D07), replaces ADR 0035's
  "listenable now" unit list with the single generation representation (D06/D07), and extends
  ADR 0032 with subject/session-bound restricted grants (D08)

The [design handoff](../work-plan/publication-access-handoff.md) owns the source map, state and
transaction boundaries, the wire/data transition, the behavior impact plan, the
acceptance-to-slice mapping and the per-slice conditions. This record owns the choices.

## Context

Source-inspected at `develop@aa0bec8d` (the S01 snapshot `d9f79480` plus UI/localization-only
commits). Proposed, accepted, implemented and verified stay distinct.

- **Catalog already owns one editing head.** `catalog_works`/`catalog_units.head_revision`
  point at immutable digest-bound revisions; lifecycle is `draft | archived`. Work `visibility`
  lives in the Work revision details, and unit `position` is a mutable current field. Editors may
  change metadata, children and order; archive, rights references and visibility are owner-only
  ([Catalog v1](../../services/cantos-server/contracts/studio/catalog/v1/README.md)).
- **Audio publication is per Unit.** `audio_publication_pointers` holds one versioned active
  release per Unit; `audio_releases` pin exact Work/Unit/content revisions; withdrawals are
  append-only; every route is current-owner only ([Publication v1](../../services/cantos-server/contracts/studio/publication/v1/README.md)).
- **Listener reads are release-driven and read current visibility.** `ListenerPublication::resolve`
  requires both the pinned and the **current** Work visibility to be `public`
  (`contexts/media/domain/listening.rs`). Saving a newer head with `visibility=private` therefore
  hides the released Work today. Public order uses immutable anchors
  (`audio_listener_works.first_published_at`, `audio_listener_units.first_position`).
- **Metadata-only disclosure exists only for the prepared sample.** `catalog_sample_metadata_disclosures`
  (`server.db` v25) admits a single approved source hash; per-subject sequences are append-only
  and withdrawal is permanent. `presentation=3` exposes it; default, `presentation=1` and
  `presentation=2` stay release-only, and strict readers deny unknown fields
  ([Discovery v1](../../services/cantos-server/contracts/theatre/discovery/v1/README.md)).
- **Delivery permits are public-only.** `PlaybackPermit` (`contexts/media/listener_delivery.rs`)
  carries publication/rendition/object/hash/length/expiry and no actor, session or policy.
- **Identity exposes no share fact.** The private RPC offers signup, `AuthenticatePassword`
  and `GetSecurityState` ([identity.proto](../../services/cantos-identity/contracts/identity/v1/identity.proto));
  an authentication fact grants no resource access. Mailbox canonicalization is ASCII, untrimmed,
  local part preserved, domain lowercased (`services/cantos-identity/src/domain/email.rs`).
- **Delegation excludes these actions.** ADR 0028 represents only `work:read`, `content:read`
  and `content:write`; publication, rights, membership, archive and grant issuance have no
  delegated representation.
- **Engine.** `turso =0.8.3` with default features off (no FTS), one serialized
  `BEGIN IMMEDIATE` writer, separate readers, MVCC off, `UnixMicros` time and a verified
  `backup_to` ([store-turso](../../packages/store-turso/README.md)). The Server registry is
  checksummed and append-only through v30; an older binary refuses a file migrated by a newer
  one (`Refusal::NewerThanBinary`).
- **Behavior SSOT.** `cantos.publishing.lifecycle` is `draft` with `model: null`;
  `cantos.media.sample-metadata` is a draft model scoped to the sample
  ([catalog](../../specs/behavior/catalog.json)).

## Decision

### Owner direction for this epic: breaking changes during development

Accepted by the owner during the D07 review and applied to every row, including earlier answers:

1. Breaking wire and API changes are allowed while Cantos is in development, provided **every
   current client** — Theatre Web, Theatre CMP Android/iOS and Studio Web — is updated and fully
   tested in the same synchronized delivery. No code, alias, presentation level or fixture is
   kept only for previously released APKs or builds.
2. Existing data needs no compatibility either: data follows the new code and wire. No
   conversion or backfill command is required; prefer the best code quality and architecture.
3. Unchanged by the direction: fail-closed authorization, no disclosure of hidden or restricted
   data, revocation, rollback that never drops a gate, immutable settled records and rights/
   consent provenance for whatever is kept. Schema changes still append new registry versions
   (which may drop or replace obsolete tables); registered SQL is never edited, because the
   checksummed registry refuses divergent history.

The direction is recorded here for epic #634; recording it repository-wide (AGENTS,
CONTRIBUTING, skills) is a separate task.

### Shared principles

1. Catalog keeps the only editing head; publication never stores a second draft pointer.
2. Four independent axes: saved editing head; metadata publication lifecycle
   `NEVER_PUBLISHED | ACTIVE | WITHDRAWN`; audience `PUBLIC | RESTRICTED`; Unit mode
   `INHERIT | OVERRIDE_WITHIN_PARENT`. Catalog `archived` and audio withdrawal stay separate.
3. Effective access is Work gate **AND** Unit gate; grants within one policy are OR.
4. Owner active, archive, withdrawal, rights, audience policy and session/security state are
   live gates on every read, grant and fetch; a pin preserves a fact, not a right.
5. Metadata publication grants no audio, cover or transcript right (ADR 0033, ADR 0051).

### Decision ledger

| ID | Status | Owner answer (30 October 2026) | Consumed by |
| --- | --- | --- | --- |
| D01 Action authority | accepted | D01a owner-only for the five mutating actions; D01b preview by owner and current Catalog reader/editor members | S02, S04, S06, S08 |
| D02 Coherent generation | accepted | D02a full-copy generation; D02b explicit order sent at publish; D02c first metadata publication sets the recency anchor (`first_visible_at`) | S02, S03, S10 |
| D03 Preview origin | accepted | A: Studio-origin preview | S04 |
| D04 Mailbox sharing | accepted | D04a claim by the proven subject; D04b 14 days and at most 100 pending shares per Work; D04c later-created accounts may claim, and a claimed binding is immune to mailbox reuse | S06, S08 |
| D05 Audience scope | accepted | D05a PUBLIC or RESTRICTED + HIDE, Unlisted/LOCK deferred; D05b audience policy replaces Catalog `visibility` as the listener gate | S02, S06, S10 |
| D06 Audio compatibility | accepted | D06a display-only fields are Work title/synopsis and Unit title/description; D06b explicit owner association for display-only drift; D06c same-transaction association on audio activation | S03, S05 |
| D07 Wire, data and rollout | accepted | D07a one representation replaced in place; D07b superseded (no data conversion); D07c `presentation=3` retired | S02, S03, S05, S07, S09, S12 |
| D08 Workload and revocation | accepted; two values open | D08a S11 measures a baseline and proposes budgets; D08b admitted streams may finish; D08c cookie/Bearer with a 120 s bound-URL fallback; D08d post-restore fence, S12 proposes RPO/RTO/retention | S07, S09, S11, S12 |

#### D01 — Action-specific publication and sharing authority

One closed action set: `publish_metadata`, `withdraw_metadata`, `set_audience`,
`manage_viewer_grants`, `manage_viewer_groups` and `preview_revision`. The first five are
**current-owner only** through the Studio surface, checked in the settling transaction.
Preview is open to the owner and current Catalog reader/editor members, who can already read the
same revisions. ADR 0028 delegation credentials gain no new action. Theatre grants never confer a
Studio action. An editor or delegated publisher role is a future extension.

#### D02 — Coherent publication generation

An immutable **Work publication generation** pins the Work metadata revision and digest and an
ordered list of entries `(unit_id, unit metadata revision, digest, published_position, optional
audio association)`, plus actor, operation receipt and parent generation. One active pointer per
Work carries the lifecycle and a monotonic version for compare-and-set. Publishing or withdrawing
one Unit writes generation N+1 that copies unchanged entries. Entries are bounded by Catalog's
limit (1,000 units per Work, tombstones included); S11 measures the full-copy cost.

- **Order (D02b).** The publish command carries the explicit ordered selection. Studio defaults
  it from the Catalog order but sends it explicitly, and the server never reads
  `catalog_units.position` or unselected heads. A later generation may change public order. This
  replaces Discovery's "first published position, never reordered" rule.
- **Recency (D02c).** The first ACTIVE generation of a Work sets its immutable public recency
  anchor (`first_visible_at`), whether or not audio exists; later generations, releases and
  withdrawals never move it.

#### D03 — Preview origin

Option **A**: preview on the Studio origin over a Studio-protected, narrow preview DTO of one
exact saved revision set, rendered with Theatre presentation through a shared package owner. No
Studio cookie or Bearer reaches Theatre, and no Theatre app is imported into Studio. A
Theatre-domain preview (B) would need its own accepted protocol first.

#### D04 — Mailbox sharing and claim

The owner enters an email; the Server stores a **pending share** keyed by the Identity-canonical
mailbox, with scope, expiry and issuing receipt, and grants nothing. A signed-in Theatre subject
claims pending shares: the Server asks Identity, over the private RPC, for **that subject's**
verified canonical mailbox and binds matching pending shares to the stable actor in one audited
transaction. Identity canonicalizes owner-entered text without reporting whether an account
exists. Bounds (D04b): a pending share expires after **14 days**, and a Work holds at most
**100** pending shares. An account created after issuance may claim (D04c). After a claim the
grant belongs to the actor and never follows the mailbox. No email is sent in v1 ("Grant access by
email", not "Invite"). Runtime authorization uses only actor or owner-scoped group IDs.

#### D05 — Audience scope and the Catalog visibility gate

v1 audiences are `PUBLIC` and `RESTRICTED` (owner-scoped actor and flat group grants with
expiry and revocation). A Unit may only restrict within its Work. Denied Units are hidden: row,
count, ordinal, next/previous and search text. Unlisted, LOCK/teaser and tiers are deferred.
The versioned audience policy **replaces** Catalog `visibility` as the listener gate (D05b).
Under the D07 direction there is no legacy lane keeping the old gate, so saving a private r4
never hides a published r3. S02 decides whether Catalog `visibility` remains editorial metadata
or is removed.

#### D06 — Metadata/audio compatibility

A generation entry is associated with an audio release only when one of these holds:

- the release pins exactly the entry's Unit metadata revision; or
- the owner records an explicit association (actor and receipt) for a difference limited to the
  **display-only** fields (Work title/synopsis, Unit title/description), while the release's
  content revision, language and kind are unchanged.

Otherwise the entry is metadata-only (`playback: null`), or the previous generation stays active.
An associated row displays the generation's text. When an audio release activates, the same Turso
transaction writes generation N+1 with the association if the active generation has a compatible
entry. Otherwise Studio reports "audio ready, not associated". Nothing searches for the latest
audio.

#### D07 — Wire evolution, data and rollout

- **Wire (D07a).** The existing Discovery routes return **one** current representation built
  from generations. Default, `presentation=1`, `=2` and `=3`, together with their DTOs and
  fixtures, are removed in the delivery that updates every current consumer and its tests.
  Restricted/shared reads use a separate authenticated Theatre contract module. There is no
  compatibility alias or dual-read lane.
- **Data (D07b, superseded).** The first answer chose an explicit operator backfill; the data
  direction withdrew it. No conversion is required. New registry versions may drop or replace
  obsolete tables (sample disclosures, the listener index). Until a generation is published under
  the new model, existing Works and releases are absent from Theatre: fail-closed, never a fallback
  to release-only eligibility.
- **Sample (D07c).** `presentation=3` and the sample-only disclosure path retire. S03 decides
  whether the sample's non-login principal publishes through the generic command or the sample
  path is simply removed.
- **Synchronized delivery.** A breaking change lands in one PR, or one integration merge,
  together with every current consumer's update and tests. `develop` never carries a Server shape
  that a current client cannot read.
- **Rollback.** An older binary refuses a newer `server.db`, so rollback means a restore (D08d),
  never a binary swap that drops gates.

#### D08 — Workload, revocation and restricted transport

1. **Budgets (D08a).** S11 extends the #568 dataset profiles with grants, groups and
   generations, measures a real baseline on Turso 0.8.3 and proposes budgets. The owner accepts
   them before any optimization; no number is assumed here.
2. **Revocation (D08b).** A committed revoke, withdrawal, expiry or member removal denies every
   later grant issuance and every later fetch, including each Range/seek (per-fetch recheck,
   ADR 0032). An admitted response may finish within the existing 60 s body deadline; buffered
   bytes are not recalled.
3. **Read-your-write.** Projection writes commit in the publishing transaction (no async outbox
   in v1), so a receipt implies visibility on the next read.
4. **Transport (D08c).**
   - Web uses the host-only Theatre cookie (`Path=/api/v1/theatre`) on media requests, qualified
     in real browsers.
   - Native sends its Bearer when the platform player supports request headers.
   - Otherwise a grant of at most 120 s, bound to subject, session and policy version, is a
     bearer URL. The owner accepts that residual risk.
5. **Restore (D08d).** After any restore, restricted delivery stays disabled until the owner
   re-confirms policies. S12 proposes RPO, RTO and retention for acceptance.

## Alternatives

| Decision | Considered and not selected | Reason |
| --- | --- | --- |
| D01 | Editors publish; delegated publisher action | Widens roles beyond Catalog v1/ADR 0028; deferred |
| D02 | Independent per-subject pointers; delta chain | No coherent order/membership snapshot; read-time reconstruction |
| D02b | Always keep first-publication anchors | Public reorder would never be possible |
| D03 | Theatre-domain preview exchange | Needs a new cross-origin protocol; Studio-origin is sufficient for v1 |
| D04 | Resolve email to account at grant time; Server-side canonicalization | Account enumeration and mailbox-reuse transfer; two owners of one policy |
| D05 | Unlisted now; Catalog `visibility` as an extra AND gate | Out of epic scope; would hide r3 when r4 is saved private |
| D06 | Exact pins only; separate publish to attach audio | Owner chose explicit association and same-transaction attach |
| D07 | Next presentation level beside old levels; contract v2; operator backfill; `presentation=3` alias | Owner direction: no compatibility code or data conversion |
| D08 | Cancel admitted streams; no transport fallback; no post-restore fence | Complexity without recall of buffered bytes; platform gaps; resurrecting revoked access |

The whole-design alternatives rejected by the epic review also stay rejected: a single
`DRAFT | RESTRICTED | PUBLIC | HIDDEN` enum, a `draft_revision_id` on publication, JWT/Redis ACL
snapshots and PostgreSQL `TEXT[]`/GIN.

## Consequences

- Product and user impact: none until slices land. Then Theatre lists only generation-published
  Works, and existing Works stay absent until the owner publishes them.
- Compatibility and migrations: intentionally breaking for wire and data within the owner
  direction; every current client changes in the same delivery. Schemas append registry versions.
- Operations, storage and production cost: S11 measures generation storage and query cost; S12
  runs the restore drill. Production data conversion, migration and activation remain separate
  tasks.
- Implementation and recovery requirements: pure decisions callable from plain tests; CAS and
  receipts in one Turso transaction; no writer lock held across Identity RPC or object I/O.
- Deferred questions: D08a budgets; D08d RPO/RTO/retention; delegated publisher; Unlisted;
  LOCK/teaser; tiers/entitlements; scheduling; nested groups; FTS; CDN/edge delivery; offline
  restricted downloads; SSE.

## Validation and revisit conditions

Evidence for the context is source review at `develop@aa0bec8d`. No runtime, test, benchmark or
deployment ran for this record, and repository hygiene validates only this document. Each slice's
oracles in the handoff validate its rows. Revisit:

- D02 if S11 measures generation copy cost over budget;
- D08c if real Web/CMP transport qualification fails;
- D04 if Identity adds a mailbox change;
- the owner direction when Cantos leaves the development phase.
