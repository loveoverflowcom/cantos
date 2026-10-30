# ADR 0054: Metadata publication generations and Work/Unit access (#634)

- Status: **proposed** — S01 design reconciliation for epic
  [#634](https://github.com/loveoverflowcom/cantos/issues/634). Every D01–D08 row below is a
  recommendation with an exact owner question. None is accepted until the owner records
  acceptance for that row; ADRs [0028](0028-work-scoped-delegation.md),
  [0030](0030-release-safe-public-metadata.md), [0032](0032-theatre-origin-playback-delivery.md),
  [0033](0033-release-pinned-public-cover.md), [0035](0035-theatre-catalog-unit-availability.md),
  [0046](0046-service-owned-contracts.md) and [0051](0051-metadata-only-public-discovery.md)
  remain authoritative within their recorded scopes
- Date: 2026-10-30
- Decision owner: Cantos product owner; the Publication/Media, Listening, Catalog and Identity
  owners for the rows that name them
- Related issue or PR: [#647](https://github.com/loveoverflowcom/cantos/issues/647) (S01);
  successors [#648](https://github.com/loveoverflowcom/cantos/issues/648)–[#658](https://github.com/loveoverflowcom/cantos/issues/658);
  related [#539](https://github.com/loveoverflowcom/cantos/issues/539),
  [#633](https://github.com/loveoverflowcom/cantos/issues/633),
  [#384](https://github.com/loveoverflowcom/cantos/issues/384),
  [#568](https://github.com/loveoverflowcom/cantos/issues/568)
- Supersedes / superseded by: N/A. If accepted, D02/D06 amend the authority source in ADR 0030
  (release-pinned → generation-pinned for generically published Works), D02/D05/D07 generalize
  ADR 0051 beyond the prepared sample, D06 amends ADR 0051 decision item 2 and ADR 0035's
  "listenable now" unit list for the new representation only, and D08 extends ADR 0032's
  public-only permit to subject/session-bound restricted grants

The [design handoff](../work-plan/publication-access-handoff.md) owns the source map, state and
transaction boundaries, compatibility matrix, behavior impact plan, acceptance-to-slice mapping
and per-slice resume conditions. This record owns only the choices.

## Context

Source-inspected at `develop@aa0bec8d` (the S01 snapshot `d9f79480` plus UI/localization-only
commits). Proposed, accepted, implemented and verified stay distinct below.

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
  append-only through v30, and an older binary refuses a file migrated by a newer one
  (`Refusal::NewerThanBinary`).
- **Behavior SSOT.** `cantos.publishing.lifecycle` is `draft` with `model: null`;
  `cantos.media.sample-metadata` is a draft model scoped to the sample
  ([catalog](../../specs/behavior/catalog.json)).

The epic's D01–D08 are proposals. The owner must choose each before the slice it blocks starts;
independent work continues without them.

## Decision

### Shared principles

These restate accepted rules or are part of D02/D05 where marked; they add no authority by
themselves.

1. Catalog keeps the only editing head; publication never stores a second draft pointer (fact).
2. Four independent axes (D05): saved editing head; metadata publication lifecycle
   `NEVER_PUBLISHED | ACTIVE | WITHDRAWN`; audience `PUBLIC | RESTRICTED`; Unit mode
   `INHERIT | OVERRIDE_WITHIN_PARENT`. Catalog `archived` and audio withdrawal stay separate.
3. Effective access is Work gate **AND** Unit gate; grants within one policy are OR (D05).
4. Owner active, archive, withdrawal, rights, audience policy and session/security state are
   live gates on every read, grant and fetch; a pin preserves a fact, not a right (ADR 0030 B).
5. Metadata publication grants no audio, cover or transcript right (ADR 0033, ADR 0051).

### Decision ledger

| ID | Status | Accepted source today | Recommendation | Blocks |
| --- | --- | --- | --- | --- |
| D01 Action authority | needs-decision | Catalog v1 and Publication v1 make visibility, rights, archive and audio publication owner-only; ADR 0028 has no delegated publication | Closed action set; owner-only publish, withdraw, audience, grants and groups; preview by owner and current Catalog reader/editor members | S02, S04, S06, S08 |
| D02 Coherent generation | needs-decision | ADR 0030 B principle only | Immutable Work generation with ordered exact Unit entries and one CAS-versioned active pointer | S02, S03, S10 |
| D03 Preview origin | needs-decision | ADR 0041 surface CORS, Identity auth v1 surface cookies, [repository layout](../architecture/repository-layout.md) forbids app-to-app imports | A: Studio-origin preview with a narrow DTO and shared presentation owner | S04 |
| D04 Mailbox sharing | needs-decision | Identity owns mailbox proof and canonicalization; no share RPC | Pending email share in Server, claimed by a signed-in subject whose verified mailbox Identity reports | S06, S08 |
| D05 Audience scope | needs-decision | ADR 0051 rejected `visibility=public` as publication | PUBLIC or RESTRICTED, Unit restrict-within-parent, HIDE only; audience replaces Catalog visibility as the listener gate on the generation lane | S02, S06, S10 |
| D06 Audio compatibility | needs-decision | Releases pin exact revisions; `MetadataChanged` blocker; ADR 0051 item 2 | Exact-pin association by default, explicit owner association for display-only drift, same-transaction association on audio activation | S03, S05 |
| D07 Wire, legacy and rollout | needs-decision | ADR 0046; strict readers; ADR 0030 wire row open; ADR 0051 item 3 | Next `presentation` level for public generation reads, separate authenticated shared module, per-Work lane cutover by explicit idempotent backfill | S02, S03, S05, S07, S09, S12 |
| D08 Workload and revocation | needs-decision | #384 SQL-first; ADR 0032 120 s grant and per-fetch recheck; Web cookie / native Bearer | Owner-chosen budgets before S11; per-fetch revocation; owner-accepted restricted transport; fail-closed restricted delivery after restore | S07, S09, S11, S12 |

#### D01 — Action-specific publication and sharing authority

Recommendation: one closed action set — `publish_metadata`, `withdraw_metadata`, `set_audience`,
`manage_viewer_grants`, `manage_viewer_groups`, `preview_revision`. The first five are
current-owner only through the Studio surface, checked in the settling transaction. Preview is
open to the owner and current Catalog reader/editor members, because they can already read the
same revisions. ADR 0028 delegation credentials gain no new action. Theatre grants never confer
any Studio action.

| Option | Benefits | Costs | Assessment |
| --- | --- | --- | --- |
| Owner-only (recommended) | Matches existing owner-only visibility/rights/audio publication; fail-closed | Teams cannot delegate publishing | Smallest authority delta |
| Editors may publish | Team workflow | Widens the editor role past ADR 0028 and Catalog v1 | Needs a role-model decision |
| New delegated publisher action | Explicit, revocable | New credential/action design, audit and tests | Deferred extension |

Owner questions: **Q-D01a** accept owner-only for the five mutating actions in v1?
**Q-D01b** preview for current reader/editor members, or owner only?

#### D02 — Coherent publication generation

Recommendation: an immutable **Work publication generation** pins the Work metadata revision and
digest and an ordered list of entries `(unit_id, unit metadata revision, digest,
published_position, optional audio association)`, plus actor, operation receipt and parent
generation. One active pointer per Work carries `NEVER_PUBLISHED | ACTIVE | WITHDRAWN` and a
monotonic version for compare-and-set. Publishing or withdrawing one Unit writes generation
N+1 that copies unchanged entries. The command carries the explicit ordered selection; the
server never completes a generation from current Catalog heads or `catalog_units.position`.
Entries are bounded by Catalog's existing limit (1,000 units per Work, tombstones included);
full-copy storage cost is measured in S11 before any delta encoding.

| Option | Benefits | Costs | Assessment |
| --- | --- | --- | --- |
| Full-copy generation (recommended) | One snapshot answers membership, order and pins; simple reads; CAS on one row | O(entries) rows per publish | Bounded by the 1,000-unit limit |
| Independent per-subject pointers (sample style) | Cheap writes | No coherent order/membership snapshot; Work/Unit races | Rejected for the generic lane |
| Delta chain | Small writes | Read-time reconstruction, harder recovery | Only if S11 measures a need |

Owner questions: **Q-D02a** accept the full-copy generation model and its bound?
**Q-D02b** is published order the explicit order sent at publish (recommended), or must it
always preserve the existing first-publication anchors? **Q-D02c** does a metadata-only first
publication set the public recency anchor, as `first_visible_at_ms` does for the sample?

#### D03 — Preview origin

Recommendation **A**: preview on the Studio origin over a Studio-protected, narrow preview
DTO of one exact saved revision set, rendered with Theatre presentation through a shared package
owner. No Studio cookie or Bearer reaches Theatre; no Theatre app is imported into Studio.
**B**, a Theatre-domain preview, needs its own scoped exchange protocol (owner, Work, revision,
purpose, short expiry, exact Origin/CORS, revocation) recorded before S04. **C**, keeping the
debug fixture, does not meet the outcome.

Owner question: **Q-D03** accept A for v1, or is a Theatre-domain preview required (then B is
designed and accepted before S04)?

#### D04 — Mailbox sharing and claim

Recommendation: the owner enters an email; the Server stores a **pending share** keyed by the
Identity-canonical mailbox, scope, expiry and issuing receipt, and grants nothing. A signed-in
Theatre subject claims pending shares; the Server asks Identity, over the private RPC, for **that
subject's** verified canonical mailbox and binds matching pending shares to the stable actor in
one audited transaction. Identity canonicalizes owner-entered text without reporting whether an
account exists. No email is sent in v1 ("Grant access by email", not "Invite"). Runtime
authorization uses only actor or owner-scoped group IDs.

| Option | Benefits | Costs | Assessment |
| --- | --- | --- | --- |
| Claim by proven subject (recommended) | No account enumeration; binding needs the subject's own verified mailbox | Two new narrow RPCs; claim UX | Fits Identity/Server ownership |
| Resolve email→subject at grant time | Immediate grant | Owner learns account existence; mailbox reuse moves access | Rejected |
| Server re-implements canonicalization | No RPC | Two owners of one policy | Rejected (ADR 0046) |

Owner questions: **Q-D04a** accept the claim model? **Q-D04b** pending-share lifetime and
per-Work pending bound (no default is assumed)? **Q-D04c** may an account created after the
share was issued claim it (recommended yes), and is a claimed binding immune to later mailbox
reuse (recommended yes)?

#### D05 — Audience scope and the Catalog visibility gate

Recommendation: v1 audiences are `PUBLIC` and `RESTRICTED` (owner-scoped actor and flat group
grants with expiry and revocation). A Unit may only restrict within its Work. Denied Units are
hidden (row, count, ordinal, next/previous, search text). Unlisted, LOCK/teaser and tiers are
deferred. For a Work on the generation lane, the versioned audience policy **replaces** Catalog
`visibility` as the listener gate; keeping `visibility` as an extra AND gate would make saving a
private r4 hide the published r3. The legacy release lane keeps today's gate until cutover (D07).

Owner questions: **Q-D05a** accept `PUBLIC | RESTRICTED` + HIDE with Unlisted/LOCK deferred?
**Q-D05b** accept that Catalog `visibility` stops gating listener reads for generation-lane
Works, so it remains editorial metadata there?

#### D06 — Metadata/audio compatibility

Recommendation: a generation entry is associated with an audio release only when (a) the release
pins exactly the entry's Unit metadata revision, or (b) the owner records an explicit association
for a **display-only** difference while the release's content revision, language and kind are
unchanged. Otherwise the entry is metadata-only (`playback: null`) or the previous generation
stays active. Display-only fields proposed: Work title/synopsis, Unit title/description. An
associated row displays the generation's text, which amends ADR 0051 item 2 for the new
representation only. When an audio release activates, the same Turso transaction writes
generation N+1 with the association if the active generation has a compatible entry; otherwise
Studio reports "audio ready, not associated". Nothing searches for the latest audio.

Owner questions: **Q-D06a** accept the display-only field list? **Q-D06b** accept explicit
owner association for display-only drift (versus exact pins only)? **Q-D06c** accept
same-transaction association on audio activation (versus a separate publish action)?

#### D07 — Wire evolution, legacy cutover and rollout

Recommendation: public generation reads use the **next `presentation` level** on the existing
Discovery v1 routes, because cursor identity already binds the level; default, 1 and 2 stay
release-only and 3 stays the sample union. Restricted/shared reads use a separate authenticated
Theatre contract module. Each Work is on exactly one lane: legacy release lane or generation
lane. An explicit, idempotent backfill moves a Work by writing generation 1 from exact eligible
release pins and `first_position` anchors, and sample disclosures with their pins and consent
provenance; Catalog heads and `visibility=public` without a release are never backfilled.
Rollout order: Server contract and fixtures, Web, CMP; restricted activation only after S07.
Because an older binary refuses a newer `server.db`, rollback is a restore (D08), never a
binary swap that drops gates.

Owner questions: **Q-D07a** next `presentation` level (recommended) or a v2 route? **Q-D07b**
backfill as an explicit operator command (recommended) or at migration time? **Q-D07c** after
cutover, does `presentation=3` remain a compatibility alias or retire with its fixtures?

#### D08 — Workload, revocation and restricted transport

Recommendation: (1) the owner records the dataset profile and p95 budgets before S11; no number
is assumed here. (2) Revocation: a committed revoke, withdrawal, expiry or member removal denies
every later grant issuance and fetch (per-fetch recheck, ADR 0032); an admitted response may
finish within the existing 60 s body deadline; buffered bytes are not recalled. (3) Projection
writes commit in the publishing transaction (no async outbox in v1), so a receipt implies
visibility on the next read. (4) Restricted media: Web uses the host-only Theatre cookie
(`Path=/api/v1/theatre`) on media requests, qualified in real browsers; native sends its Bearer
when the platform player supports headers; otherwise a ≤120 s grant bound to subject, session and
policy version is a bearer URL whose residual risk the owner must accept. (5) After any restore,
restricted delivery stays disabled until the owner re-confirms policies.

Owner questions: **Q-D08a** dataset profile and budgets? **Q-D08b** accept the revocation
semantics in (2)? **Q-D08c** accept the restricted transport in (4), including the bearer-URL
fallback? **Q-D08d** RPO/RTO/retention and the post-restore fence in (5)?

## Alternatives

Per-decision alternatives are in each row above. The rejected whole-design alternatives are the
epic's single `DRAFT | RESTRICTED | PUBLIC | HIDDEN` enum (mixes four axes), a
`draft_revision_id` on publication (second editing head), JWT/Redis ACL snapshots (contradicts
fresh security-state admission) and PostgreSQL `TEXT[]`/GIN (not the engine).

## Consequences

- Product and user impact: none now. Accepted rows unlock their slices; unaccepted rows keep
  today's release-gated, sample-only and public-only behavior.
- Compatibility and migrations: none now. Every later schema is an appended `server.db` (or
  `identity.db`) version; registered SQL is never edited.
- Operations, storage and production cost: none now; generation storage and query cost are
  measured in S11; restore drills are S12.
- Implementation and recovery requirements: pure decisions callable from plain tests; CAS and
  receipts in one Turso transaction; no writer lock held across Identity RPC or object I/O.
- Deferred questions: delegated publisher, Unlisted, LOCK/teaser, tiers/entitlements,
  scheduling, nested groups, FTS, CDN/edge delivery, offline restricted downloads, SSE.

## Validation and revisit conditions

Evidence for the context is source review at `develop@aa0bec8d`; no runtime, test, benchmark or
deployment ran for this record. Repository hygiene validates this document only. Each accepted
row is validated by its slice's oracles in the handoff. Revisit D02 if S11 measures generation
copy cost over budget, D08 if real Web/CMP transport qualification fails, and D04 if Identity
adds mailbox change.
