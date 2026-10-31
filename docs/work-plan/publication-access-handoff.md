# Publication lifecycle and Work/Unit access: design handoff (#634)

This is the S01 ([#647](https://github.com/loveoverflowcom/cantos/issues/647)) **source map,
boundary design and evidence plan** for epic [#634](https://github.com/loveoverflowcom/cantos/issues/634).
[ADR 0054](../decisions/0054-publication-generation-and-access.md) owns the D01–D08 choices,
accepted by the owner on 30 October 2026, including the owner's breaking-change direction. This
file owns the inventory, the state/transaction/read boundaries, the wire/data transition, the
behavior impact plan, the acceptance mapping and the per-slice conditions. The boundaries apply
the accepted rows; table, module and route names in them are design, not implemented code.
GitHub owns execution status and acceptance.

## Review boundary and baseline

| | |
| --- | --- |
| Delivers | Accepted decision ledger (ADR 0054); this source map and boundary design; wire/data transition and rollback oracle; behavior impact plan; AC→slice→owner→oracle mapping; workload and recovery plan; successor contracts |
| Does not deliver | Any DTO, protobuf field, schema, migration, endpoint, query, Quint model, registry entry or client change; the D08a budgets and D08d RPO/RTO/retention values |
| Baseline | `develop@aa0bec8d`. The issue snapshot `d9f79480` differs only by #659/#660 (Theatre search UI, chips, localization); no publication, catalog, identity, contract or behavior source changed |
| Turso review input | [Epic addendum](https://github.com/loveoverflowcom/cantos/issues/634#issuecomment-6493415800), sourced at `75d4b795`; its schema/index/backup points are hypotheses, carried into D02/D08 and the workload plan |
| Closes | Nothing; S01 is `Part of #634` and of #647 |

## Source map at the baseline

| Path | Current fact | Role / owning slice |
| --- | --- | --- |
| [`catalog_publication_read.sql`](../../services/cantos-server/src/platform/turso/catalog_publication_read.sql) | `head_revision` on Works/Units; immutable digest-bound revisions; mutable `catalog_units.position`; lifecycle `draft`/`archived` | Generation pins revisions, never position (S02) |
| [Catalog v1](../../services/cantos-server/contracts/studio/catalog/v1/README.md) | Owner mutates; editor members edit metadata/children/order; archive, rights refs and visibility owner-only; ≤1,000 Units per Work | D01 baseline; D02 bound |
| [Publication v1](../../services/cantos-server/contracts/studio/publication/v1/README.md), [`publication.rs`](../../services/cantos-server/src/contexts/media/domain/publication.rs) | Exact-revision releases, clearance/approval, `MetadataChanged` blocker, owner-only | D06; S03 keeps every gate |
| [`audio_release_chain.sql`](../../services/cantos-server/src/platform/turso/audio_release_chain.sql) | Per-Unit `audio_publication_pointers(version)`; append-only withdrawals; immutable `audio_listener_works/units` anchors | Association target (S03); obsolete listener index may be dropped (D07b) |
| [`listening.rs`](../../services/cantos-server/src/contexts/media/domain/listening.rs) | Eligibility requires pinned **and current** Work `visibility=public` | Gate replaced on the generation lane (D05b, S02) |
| [`media/turso/listener.rs`](../../services/cantos-server/src/contexts/media/turso/listener.rs), [`catalog/turso/listener.rs`](../../services/cantos-server/src/contexts/catalog/turso/listener.rs) | Representative-release selection; search `instr(search_text, …)`; keyset by anchors | Public projection (S02), performance (S11) |
| [Discovery v1](../../services/cantos-server/contracts/theatre/discovery/v1/README.md) | Default/1/2 release-only; 3 sample union; strict readers; cursor binds presentation | D07 version boundary |
| [`catalog_sample_metadata_disclosures.sql`](../../services/cantos-server/src/platform/turso/catalog_sample_metadata_disclosures.sql), [`metadata_disclosure.rs`](../../services/cantos-server/src/contexts/catalog/domain/metadata_disclosure.rs), [`sample_metadata.rs`](../../services/cantos-server/src/contexts/catalog/application/sample_metadata.rs) | Single approved source hash; per-subject append-only sequence; permanent withdrawal | Sample transition input (S03); never generalized silently |
| [`listener_delivery.rs`](../../services/cantos-server/src/contexts/media/listener_delivery.rs), [`listener_media.rs`](../../services/cantos-server/src/contexts/media/listener_media.rs), [ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md) | `PlaybackPermit` has no actor/session; 120 s grant; per-fetch and post-open recheck; anonymous 404 | Restricted permit (S07) |
| [`http/theatre.rs`](../../services/cantos-server/src/adapters/inbound/http/theatre.rs) | `public_playback_post` exempts only the anonymous playback POST from Origin checks | Authenticated restricted POST must not inherit it (S07) |
| [identity.proto](../../services/cantos-identity/contracts/identity/v1/identity.proto), [`email.rs`](../../services/cantos-identity/src/domain/email.rs) | No share/mailbox fact RPC; canonical mailbox: ASCII, untrimmed, local part kept, domain lowercased | D04 RPCs (S06) |
| [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) | Web cookie `HttpOnly; SameSite=Strict; Path=/api/v1/{surface}`, host-only; native opaque Bearer | D08 transport (S07–S09) |
| [store-turso](../../packages/store-turso/README.md), [Server registry](../../services/cantos-server/src/platform/turso/mod.rs) | `turso =0.8.3`, FTS off, one `BEGIN IMMEDIATE` writer, MVCC off, `UnixMicros`, verified `backup_to`; registry v1–v30; older binary refuses newer file | Transaction design; rollback and restore (S12) |
| [Behavior catalog](../../specs/behavior/catalog.json), [inventory](../../specs/behavior/inventory.md) | `cantos.publishing.lifecycle` draft, `model: null`; sample model draft and sample-scoped | Impact plan below |
| Studio [`publication.rs`](../../apps/cantos-studio/web/src/live/publication.rs), [`catalog_controls.rs`](../../apps/cantos-studio/web/src/live/catalog_controls.rs), [`authoring_preview.rs`](../../apps/cantos-studio/web/src/authoring_preview.rs), [`audio_preview.rs`](../../apps/cantos-studio/web/src/live/audio_preview.rs) | Visibility patch separate from audio publish; preview is a debug fixture; private audio preview is exact | S04/S05/S08 consumers |
| Theatre Web [`public_work.rs`](../../apps/cantos-theatre/web/src/features/public_work.rs), [routes](../../apps/cantos-theatre/web/src/app/routes.rs); CMP [`PublicCatalog.kt`](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/domain/catalog/PublicCatalog.kt), [`PublicCatalogHttp.kt`](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/adapters/http/PublicCatalogHttp.kt) | Optional playback / `withoutAudio`; CMP public metadata cache | S05/S08/S09 consumers |

### Drift observed, not fixed in S01

- Discovery v1's "Origin-mode media (proposed ADR 0032)" section and the
  [documentation map](../README.md) call ADR 0032 a proposal that no host composes, while ADR 0032
  records acceptance for step-1 origin integration and the v1.2 cover section describes a
  composing public-media host. Owner: Publication/Media contract; correct in S07 or separately.
- Some skill prose still names signed URLs, CDN or Moka; #384 and ADR 0032 govern. Not a
  requirement to restore those mechanisms.

## State and transaction boundary

| Command | Expected version (CAS) | Writes in one transaction | Never |
| --- | --- | --- | --- |
| Save draft (existing Catalog) | Work/Unit revision | Catalog revision, head, receipt | Touch publication, audience or projection |
| Publish selection | Work active-pointer version | Generation N+1, entries, pointer, receipt, projection rows | Read current position or unselected heads |
| Publish or withdraw one Unit | Work active-pointer version | Generation N+1 copying unchanged entries | Create a second Unit authority |
| Set audience / grants / groups | Policy version | Policy version, grant/group rows, receipt | Publish the current head |
| Withdraw Work | Work active-pointer version | Withdrawal record, pointer `WITHDRAWN`, receipt | Archive or withdraw audio |
| Republish | Work active-pointer version | New generation with current rights/policy rechecks | Revive old grants, approvals or receipts |
| Audio activation (D06c) | Audio pointer version and Work active-pointer version | Audio pointer and, when compatible, generation N+1 with association | Search for "latest audio" |

Settlement order on the single writer: parse typed IDs and bounds; authenticate the surface and
the D01 action; perform any Identity RPC or object I/O **before** `begin_write`; inside the
writer re-read owner, archive, rights, approval, policy and pointer versions, compare-and-set,
then write immutable rows, the pointer, the receipt and projection rows, and commit. No writer
lock is held across an RPC or object open.

| Situation | Required outcome |
| --- | --- |
| Same operation ID and canonical payload | Same settled outcome and generation |
| Same operation ID, different payload | Typed conflict (`409`), nothing written |
| Lost response or timeout | Unknown outcome; reconcile by operation ID, never mint a new revision |
| Stale expected version; two concurrent publishers | One commits; the other gets a typed `409` with the current version |
| Restart or reopen | Pointers, receipts, policy versions and revocations persist; no replay resurrects data |

Read boundary: each page reads the active pointer, generation entries and live gates in one read
transaction; eligibility filters before pagination, count, ordinal and ranking. Work lists key on
the immutable recency anchor (D02c). Because public Unit order is explicit per generation (D02b),
a Unit-list cursor also binds the generation it walks; a newer generation restarts the walk.
Shared cursors also bind actor and access scope. Projection rows commit with the publication, so
there is no projection lag in v1.

## Access, identity, preview and media boundaries

```text
CanViewWork(s, w)  = pointer(w) = ACTIVE ∧ owner active ∧ ¬archived(w) ∧ rights(w) ∧ WorkAudienceAllows(s, w)
CanViewUnit(s, u)  = CanViewWork(s, parent(u)) ∧ u ∈ entries(active generation) ∧ ¬archived(u)
                     ∧ rights(u) ∧ (mode(u) = INHERIT ∨ UnitAudienceAllows(s, u))
CanPlay(s, u, r)   = CanViewUnit(s, u) ∧ association(u) = r ∧ eligible(r)
                     ∧ fetch admission (PUBLIC: guest; RESTRICTED: valid subject and session)
```

- **Identity (S06, design only).** Two narrow private RPCs, with names and field numbers chosen
  by the Identity owner: canonicalize owner-entered mailbox text with no account-existence
  result; return the calling Server's **current session subject's** verified canonical mailbox
  with `security_version` and `instance_epoch`. No public lookup endpoint; no `identity.db` read
  by the Server. Pending shares expire after 14 days, at most 100 per Work (D04b); a claim binds
  the actor once and never follows later mailbox reuse (D04c).
- **Groups (S06).** Owner-scoped, flat, versioned membership; a group name is never identity;
  no Studio role is derived from membership.
- **Preview (S04, D03).** Exact saved revision set; `private, no-store`, no-referrer; no
  OpenGraph, sitemap, analytics or listening history; missing private asset is unavailable, never
  a public substitute; logout/account switch fences late responses.
- **Media (S07–S09, D08).** Grant issuance and every GET/HEAD/Range recheck subject, session,
  Work AND Unit policy, expiry and exact release identity; an admitted response may finish within
  the 60 s body deadline (D08b). Web sends the Theatre cookie; native sends its Bearer where the
  player supports headers, otherwise a ≤120 s subject/session/policy-bound URL (D08c). Web is
  qualified in real browsers; Android/iOS each record their own player result; an unrun target
  stays `NOT_RUN`.

## Wire and data transition (D07)

The owner direction in ADR 0054 replaces the old-reader compatibility plan. There is no legacy
lane, no compatibility alias and no data conversion.

| Surface | After the generation slices land |
| --- | --- |
| Discovery list/search/detail/units | One representation from active PUBLIC generations; rows carry `playback: null` or an associated release |
| `presentation=1`, `2`, `3` and the default shape | Removed with their DTOs and fixtures; S02 decides whether a leftover `presentation` parameter is `invalid_request` or ignored, never a fallback to another shape |
| Restricted/shared reads | Separate authenticated Theatre module (S07) |
| Existing Works, releases and sample disclosures | Absent until a generation is published under the new model; obsolete tables may be dropped by a new registry version |
| Current consumers (Theatre Web, CMP Android/iOS) | Updated and tested in the same delivery as each breaking change |

Delivery rule: a slice that changes a wire shape includes every current consumer's adoption and
tests, or keeps the shape unchanged until the slice that adopts it. `develop` never carries a
Server shape a current client cannot read. S02/S05 therefore plan the Discovery switch as one
delivery or keep the existing shape until S05.

Fail-closed oracle for S02/S03 (real files, not mocks): from the prepared sample root and a
release fixture, apply the new registry versions; assert every Theatre path returns no Work or
Unit, without fallback to release-only eligibility. Publish a generation and assert exactly its
entries, order and associations appear; withdraw it and assert absence again. Reopen the files and
assert pointers, receipts and withdrawals persist.

Rollback: the registry refuses an older binary on a newer `server.db`, so rollback means restoring
a pre-migration backup. That restore can predate later withdrawals or revocations, so restricted
delivery stays disabled after restore until the owner re-confirms policies (D08d). Never "roll
back to run" by removing a gate.

## Behavior SSOT impact

Lookup: catalog → manifests → inventories → models → code, at the baseline. The accepted rows are
an **approved requirement change**, but S01 changes no code. The procedure carries each write
bundle together with its implementation, so S01 writes **no catalog, manifest, model or run**.

| Capability (lifecycle) | Relevance | Class | Planned delta and slice |
| --- | --- | --- | --- |
| `cantos.publishing.lifecycle` (draft, `model: null`) | Audio pointer/withdrawal; association | approved change + missing model | S03 records the D06 association scope; no `.qnt` before an exact observation scope |
| none (candidate: metadata generation) | Generation, CAS, explicit order, Work/Unit selection | missing capability | S02 registers a bounded draft at the generation domain owner (D02/D05); candidate ID chosen there |
| none (candidate: Work/Unit access policy) | Work AND Unit, INHERIT, revocation monotonicity, no email principal | missing capability | S06 (D04/D05); witnesses and negative controls per property |
| `cantos.media.sample-metadata` (draft, sample-scoped) | Retires with the sample path (D07c) | approved change | S03 deprecates or removes it together with the sample code |
| `cantos.media.public-playback-delivery` (draft) | Restricted grant/fetch | approved change | S07 adds subject/session binding (D08b/c) |
| `cantos.media.public-cover-projection` (draft) | ADR 0033 cover authority is unchanged | no change in S01 | S02/S03 state how the single representation selects a cover; change the capability only if its guard changes |
| `cantos.identity.native-mailbox-proof`, `…native-session-admission` (draft) | Claim and restricted admission | no change in S01 | S06/S07 reuse; extend only if a guard changes |
| `cantos.listening.progress` (needs-decision) | Restore paths reauthorize | needs-decision | S07/S09 regression; no model change implied |

Model PASS never stands for implementation conformance; `--conformance` only for registered
observers of a production seam.

## Acceptance mapping

| Epic AC | Slices (owner of final evidence: S12 #658) | Owners | Oracle and layer |
| --- | --- | --- | --- |
| 01 Design authority | S01, S12 | Product, Publication | ADR rows accepted per slice; docs hygiene |
| 02 Revision ownership | S02 | Catalog, Publication | No second head; save never publishes; pure + Turso |
| 03 Draft isolation | S02, S05, S10 | Publication, Listening | Save r4 (title/order/cover/visibility) leaves r3 projection identical; Turso + HTTP |
| 04 Coherent publication | S02, S05, S10 | Publication | Per-Unit publish keeps other entries; order independent of `position` |
| 05 Transaction/retry | S02, S11 | Publication | Replay, changed payload, lost ack, concurrent publisher, reopen on real files |
| 06 Metadata/audio separation | S02, S03, S05, S09 | Publication, Listening | `playback: null` dispatches nothing; Web/CMP state tests |
| 07 Audio compatibility | S03, S05 | Publication | Exact/explicit association; incompatible blocked; withdraw immediate |
| 08 Lifecycle actions | S02, S03, S05, S06, S10 | Publication | Audience-only never publishes; republish rechecks |
| 09 Secure preview | S04 | Studio, Listening | Exact revision, narrow DTO, no-store, no mutation; browser |
| 10 Studio UX | S05, S08, S10 | Studio UI | Dirty buffer, CAS conflict, matrix; real browser |
| 11 Verified email binding | S06, S08 | Identity, Server | Forged/unverified/reused mailbox denied; RPC + HTTP |
| 12 Viewer groups | S06, S08 | Server | Bounded listing, expiry/removal/disable isolation |
| 13 Parent/child policy | S06, S07, S10 | Server, Listening | Work AND Unit; HIDE leaks nothing; Quint + Turso |
| 14 Restricted bytes | S07, S08, S09 | Media | Wrong subject/old grant/after revoke denied on GET/HEAD/Range; real bytes |
| 15 Revocation/recovery | S06, S07, S08, S09, S10, S11 | Server, Media | Expiry, removal, switch, withdrawal, restart |
| 16 Public search | S02, S07, S10, S11 | Listening | Public-only eligibility before pagination; no ACL N+1 |
| 17 Shared discovery | S07, S08, S09, S10, S11 | Listening | Dedup, scoped cursor, login-return |
| 18 Cache/privacy | S04, S07, S08, S09, S10, S11 | Listening, clients | No restricted data in public/persistent cache; logout fences |
| 19 Contract compatibility | S01–S10 | Each providing service | Owner direction: every current consumer updated with each contract change and its tests; unknown or unsupported restricted access fails closed |
| 20 Migration/rollback | S01, S02, S03, S06, S07, S12 | Publication, Ops | Append-only registry versions; fail-closed oracle above; rollback/reopen never drop a gate; post-restore fence |
| 21 Behavior SSOT | S01, S02, S03, S04, S06, S07, S10 | Behavior owners | Impact table above; witnesses and controls |
| 22 Backend qualification | S02, S03, S04, S06, S07, S10 | Server | Pure, Turso, HTTP, Identity RPC, bytes as separate rows |
| 23 Browser qualification | S04, S05, S08, S10 | Web | Real browser journeys, keyboard/focus/VI-EN |
| 24 CMP qualification | S05, S09, S10 | CMP | Android/iOS each; unrun stays `NOT_RUN` |
| 25 Performance | S01, S11 | Server, #384 | Budgets recorded first; plans and measurements on 0.8.3 |
| 26 Handoff/rollout | S01, S12 | All | Commands, SHAs, targets, deferred production acceptance |

The epic's AC-19 and AC-20 wording still asks for old strict readers, backfill and preserved
sample/legacy history; the owner direction in ADR 0054 replaces those parts. The rows above
state the accepted oracle. The GitHub issue text is unchanged by this PR.

## Workload, budget and recovery plan

D08a: S11 extends the #568 profiles, measures a baseline and proposes the values below for owner
acceptance before any optimization. No number here is a target.

| Input for S11 | Value |
| --- | --- |
| Works; Units per Work (≤1,000); generations per Work | S11 proposes |
| Grants per Work; groups per owner; members per group; shared Works per actor; pending shares per Work (≤100, D04b) | S11 proposes |
| Query mix: public list/search/detail/units, shared list, preview, grant issue, fetch admission, publish, revoke | S11 proposes |
| Concurrency (readers with publish/revoke), cold and warm runs | S11 proposes |
| p95 budgets per query class; publish write latency; RSS | S11 proposes, owner accepts |

Method: `cargo xtask seed dataset` profiles ([localhost datasets](../deployment/localhost-datasets.md),
#568) and the [Turso read method](../../.agents/skills/cantos-engineering/references/turso-read-performance.md)
(#384): query plans on pinned 0.8.3, p50/p95/p99, statements per request, rows scanned/returned,
DB and index bytes. Index candidates from the Turso review (target-first grant index,
principal-first index, partial index on ACTIVE PUBLIC) stay hypotheses until measured. FTS, MVCC
and caches are out of scope unless measured benefit and parity evidence exist.

S12 recovery constraints: `backup_to` per file; `server.db` and `identity.db` restored as a pair
on disposable copies; no live file copy; the accepted post-restore restricted-delivery fence
(D08d); S12 proposes RPO, RTO and retention for owner acceptance; production restore and
activation remain separate tasks.

## Successor handoff

Every D01–D08 row is accepted. A successor starts when its predecessor's contract is in the base
and follows the synchronized-delivery rule above. Only the listed open values still need owner
acceptance.

| Slice | Accepted decisions it consumes | Still open | Receives | Must not |
| --- | --- | --- | --- | --- |
| S02 [#648](https://github.com/loveoverflowcom/cantos/issues/648) | D01a, D02a–c, D05a–b, D07a–b | — | This boundary, fail-closed oracle | Read current heads/position into a generation; ship a wire change without its consumers; enable restricted |
| S03 [#649](https://github.com/loveoverflowcom/cantos/issues/649) | D06a–c, D07b–c | — | S02 schema and generation API | Rebind audio implicitly; keep a sample-only lane |
| S04 [#650](https://github.com/loveoverflowcom/cantos/issues/650) | D01b, D03 | — | S02/S03 contracts | Forward Studio credentials to Theatre |
| S05 [#651](https://github.com/loveoverflowcom/cantos/issues/651) | D02b, D06, D07a | — | Server fixtures | Choose backend semantics in UI |
| S06 [#652](https://github.com/loveoverflowcom/cantos/issues/652) | D01a, D04a–c, D05a | — | Identity RPC design above | Grant from email text; activate restricted |
| S07 [#653](https://github.com/loveoverflowcom/cantos/issues/653) | D08b–c | — | S06 core | Activate before every read/bytes path is gated |
| S08 [#654](https://github.com/loveoverflowcom/cantos/issues/654) | D04, D08c | — | S07 contracts | Show sharing UI the backend cannot enforce |
| S09 [#655](https://github.com/loveoverflowcom/cantos/issues/655) | D08c | — | S07/S08 contracts | Use stale offline restricted metadata as access |
| S10 [#656](https://github.com/loveoverflowcom/cantos/issues/656) | D02, D05a | — | S06–S09 | Add LOCK/Unlisted |
| S11 [#657](https://github.com/loveoverflowcom/cantos/issues/657) | D08a | Budgets (S11 proposes) | S10 | Claim O(1) or unmeasured latency |
| S12 [#658](https://github.com/loveoverflowcom/cantos/issues/658) | D07, D08d | RPO/RTO/retention (S12 proposes) | S11 | Tick epic AC from partial PRs; deploy without a separate task |

### S02 delivered contract (#648)

S02 implements the Server generation authority without changing any Theatre wire shape: the
[Studio metadata publication v1](../../services/cantos-server/contracts/studio/metadata-publication/v1/README.md)
routes, the pure core [`metadata_publication.rs`](../../services/cantos-server/src/contexts/catalog/domain/metadata_publication.rs),
`server.db` v31 [`catalog_metadata_publications.sql`](../../services/cantos-server/src/platform/turso/catalog_metadata_publications.sql)
and an unrouted public read facade. Choices this boundary left to S02:

| Question | S02 answer |
| --- | --- |
| Catalog `visibility` | Kept as editorial Studio metadata and never read by the generation lane (D05b). Removing the field changes Catalog v1 and Studio Web, so it belongs with the S05 Studio workflow |
| Discovery switch | Kept unchanged: no current client can read a generation representation yet, so S05 replaces the shapes together with Theatre Web and CMP. The leftover-`presentation` question moves to S05 |
| Rights at publish | At least one rights reference on the pinned Work revision, each resolving to the owner's rights evidence (the audio-release precedent), plus the owner's explicit `confirm_metadata_rights`. Withdrawal never needs rights |
| Per-Unit commands | `publish_unit` (add or replace at an explicit one-based position) and `withdraw_unit`; each writes generation N+1 copying every other entry. A result equal to the active generation is refused as `unchanged` |
| State read | Current owner only, like every mutating action (D01a); member preview is S04 (D01b) |

Seam for S03: inside one write unit, `turso::metadata_publication::state` reads the pointer,
`metadata_publication::decide` settles the change and `insert_generation` + `settle` write the
generation, its sealing event (receipt) and the pointer. Audio association needs a new registry
version keyed by `(work_id, generation, unit_id)` and a change that copies the active generation
with the association; generations, entries and events are append-only and an entry cannot be
added once its generation is sealed. Still open for S03: the sample-only disclosure lane and
`presentation=3` remain live; the release listener index is unchanged.

Fail-closed evidence (real files): a Work with an eligible audio release and a public-visibility
Work without a generation are absent from the generation reader; publishing shows exactly the
selected entries, order and pinned text; withdrawal hides the Work again; pointers, receipts and
withdrawals persist across reopen. "Every Theatre path" applies when S05 routes Theatre through
this reader.

### S03 delivered contract (#649)

S03 implements D06a–c on the generation lane without changing any Theatre or Studio Web wire
shape: `server.db` v32 [`catalog_metadata_audio_associations.sql`](../../services/cantos-server/src/platform/turso/catalog_metadata_audio_associations.sql),
the pure compatibility matrix and association in [`metadata_publication.rs`](../../services/cantos-server/src/contexts/catalog/domain/metadata_publication.rs),
Media's [`generation_audio.rs`](../../services/cantos-server/src/contexts/media/generation_audio.rs)
and the additive `associate_audio` action and entry `audio` field of
[Studio metadata publication v1](../../services/cantos-server/contracts/studio/metadata-publication/v1/README.md#audio-association),
which no current client reads yet. Choices this boundary left to S03:

| Question | S03 answer |
| --- | --- |
| Where association lives | A pin `(release_id, release_digest, basis)` per generation entry, written before the sealing event; changing it writes generation N+1 (`associate_audio` or `activate_audio`). The event table is rebuilt in v32 only to admit these two actions; every existing row is copied |
| Catalog/Media boundary | Catalog owns the rows and the compatibility decision and never reads Media tables; Media answers the `ActiveAudioRelease` port (the release the Unit's audio pointer names now) and owns the live playback gate. D06c runs inside Media's activation write unit through `catalog::activate_audio` |
| Exact rule (D06) | Same Unit revision. As a fail-closed tightening the Work's kind, language (case-insensitive) and rights references (as a set) must also be unchanged; Work title/synopsis may differ |
| Catalog `visibility` | Not compared: it is editorial metadata and never a listener gate (D05b). Recorded here for owner review together with its removal in S05 |
| Display-only drift (D06a/b) | Unit title/description (and Work title/synopsis) only, with an unchanged Unit kind, source, Script IR and content digest; associated only by the owner's `associate_audio` with `confirm_display_only_drift` |
| Metadata changes after association | Same Unit pin under the same Work pin keeps the association; a new Unit or Work pin keeps nothing (an `exact` release is re-offered at publish, a `display_only` one needs the owner again); republication after `withdrawn` revives no owner decision |
| Withdrawal, revocation, expiry, supersession | Live gate at every read (pointer still names the release, no tombstone or revocation, latest clearance/approval still the manifest's, access window); the generation keeps the historical pin and no cleanup write is needed |
| Sample-only lane (D07c) | Removed, not migrated: the sample principal gets no generic publishing path. `presentation=3` and the sample disclosure tables are retired in S05 together with the Discovery switch and every current client, because Theatre Web still reads `presentation=3` today |
| Data (D07b) | No backfill. Existing releases associate when their owner publishes a generation (exact) or associates explicitly |

Seam for S04/S05: `PublicUnit::audio` from the catalog read facade plus
`media::playable_releases(tx, …)` in the same read snapshot give each visible Unit `None`
(metadata-only, `playback: null`) or the exact `AudioRelease` to project; S05 maps that to the one
Discovery representation and deletes the sample lane. Studio reads "audio ready, not associated"
from the metadata publication state. Evidence: pure matrix/association/live-gate tests,
file-backed Turso tests (exact offer, display-only confirmation, incompatibility, no substitute,
withdrawal/supersession/revocation, replay, concurrent associations, reopen, schema barriers), a
real-file v31→v32 upgrade preserving publish/withdraw/republish history and its guards, and the
real Studio HTTP activation journey.

### S04 delivered contract (#650)

S04 implements D01b and D03 without a registry change or a generation read:
[Studio preview v1](../../services/cantos-server/contracts/studio/preview/v1/README.md)
(`GET /api/v1/studio/works/{work_id}/preview?work_revision=R`), the pure admission
[`preview.rs`](../../services/cantos-server/src/contexts/catalog/domain/preview.rs), the Catalog read
[`preview.rs`](../../services/cantos-server/src/contexts/catalog/application/preview.rs) and Studio Web's
[`preview.rs`](../../apps/cantos-studio/web/src/live/preview.rs) /
[`preview_panel.rs`](../../apps/cantos-studio/web/src/live/preview_panel.rs). Choices this boundary
left to S04:

| Question | S04 answer |
| --- | --- |
| What "exact revision set" means | The saved Catalog head the creator names: Work revision R and the active Units at their head revisions, each echoed with its revision. Every Unit change advances the Work revision, so R identifies the set; Catalog does not record Unit membership per older Work revision, so an older R is `409 stale_revision`, never the newer head under the old banner |
| Authority | Owner and current reader/editor members through the existing Catalog `Action::Read` check, inside the read transaction and again before commit. A stranger, a guest and an absent Work are `404`/`401` and indistinguishable. Delegation credentials and Theatre sessions never reach the route |
| Archived Work | `409 catalog_conflict` (`lifecycle: archived`); archived Units never take a rank |
| DTO | Allowlist `{id, kind, revision, title, synopsis, language}` / `{…, ordinal, description}`; no owner, visibility, rights, link, lifecycle, position, digest or timestamp. `ordinal` counts active Units across pages |
| Cover and audio | Not in v1: Studio shows both unavailable and never substitutes a public asset. The owner's private artwork and audio previews keep their own exact bindings; member cover/audio preview is not implemented |
| Pagination | Catalog's epoch-bound cursor, scoped to actor, Work and revision; any Catalog write ends a walk with `stale_cursor` |
| Headers | `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`; no cookie, no HTML, so OpenGraph, sitemap, autocomplete and image URLs do not exist |
| Studio flow | "Preview saved version" asks for the saved revision it holds; "Save, then preview" arms a follow-up for its own acknowledged save only. A failed or unknown save, an unsaved draft elsewhere and any other operation open nothing. The reply must name the requested Work and revision or it is a reply mismatch |
| Fencing | The preview is a Facts value: dropped when the saved head moves past it, the Work changes, the actor changes or the Work pane is covered by another route; a pending read is cancelled by the existing request owner |

Presentation is bounded and Studio-owned on the shared tokens and `cantos-ui` components. There is
no concrete second consumer for a Theatre-shared presentation package yet, so none was added and
Theatre Web is unchanged. Evidence: pure admission/projection/reducer tests, contract fixtures in
both readers, real-file Turso Studio router tests (authority, exact revision, side-effect absence
over Catalog and publication tables, headers, paging) and a real-browser run of the Studio WASM
against a synthetic API (not Server or Identity).

### S05 consumer contract (#651)

Studio uses the existing Work draft/CAS, workspace mount and request authority for metadata
publication. An explicit state read distinguishes unread/error from `never_published`, active
from withdrawn, and current state from historical operation receipts. The bounded review starts
with the exact existing published pins; only explicitly selected loaded drafts are added or
updated. The full final membership/order is visible with keyboard reorder/removal controls and
one final rights confirmation. Save/preview, metadata withdraw, Catalog archive and audio
withdraw retain separate intents. Conflict preserves the selection; an explicit reload/compare
can rebase its pointer version. Timeout/cancel retains the original operation and payload for
replay, and a replay receipt never masquerades as the current pointer.

Theatre Web and CMP use the one default [generation metadata representation](../../services/cantos-server/contracts/theatre/discovery/v1/README.md).
The Server composes Catalog's exact pins and Media's live association gate. Old presentation
selectors are rejected. A sample attestation or audio release without a generic generation is
absent. Metadata-only rows dispatch no manifest/playback request. Manifest/grant/media reads
recheck the exact current association and full release integrity; cover reads use the generation
Work revision with independent image clearance and reproject after object I/O.

No registered SQL is changed and no old row is converted into generic publication authority.
Private sample seed provenance and its historical test oracles remain separate from anonymous
Discovery. The sample behavior ID is deprecated; generic generation/association remains draft
with no model until its accepted observation scope includes S06 audience transitions.

The current state DTO exposes revision/digest/order/audio pins, not historical field text.
Studio reads exact earlier/selected cover identities through the existing artwork revision
endpoint and compares revision/order changes. It never presents a newer head's text as a
historical snapshot. Full historical Work field text comparison remains a contract gap. Library projection and later restricted/cache/native qualification retain their
own S06–S09 scope; source/JVM/browser evidence does not qualify Android or iOS.

S06 receives unchanged publication command/CAS/receipt semantics and the generic public read
composition. It must keep restricted access unavailable until every required route and bytes
path enforces the new audience rules. No global Studio navigation, shared UI primitive,
provider, deployment or production migration is introduced by S05.


### S06 access control-plane contract (#652)

S06 provides [Studio viewer access v1](../../services/cantos-server/contracts/studio/viewer-access/v1/README.md),
[Theatre claim v1](../../services/cantos-server/contracts/theatre/viewer-access/v1/README.md),
private Identity `CanonicalizeMailbox` / `GetVerifiedMailbox`, Server registry v33 and the
[Work/Unit core](../../services/cantos-server/src/contexts/catalog/domain/viewer_access.rs).
The existing Identity v3 verified-mailbox rows suffice without a new Identity migration.

A share expires after 14 days and at most 100 live unclaimed rows exist per Work. Claim reads
only the current session subject, with operation/epoch/version/time binding outside the writer,
then re-admits the same session and settles actor grants and immutable claim receipts atomically.
A receipt is historical; replay after revoke or reopen never writes another grant. Reuse of a
mailbox cannot move an already claimed share. An already-granted actor is deduplicated against
its existing grant, including that grant's expiry. Group membership and grant expiry use trusted
microseconds; group membership changes have a separate group version. Policy changes never
move the metadata generation or editing head; public widening rechecks published-pin rights.

S07 must compose the pure Work AND Unit evaluator with fresh session/security, current owner,
rights, membership, policy and publication inputs at every metadata and bytes admission. It
must include policy and group versions in its invalidation/fetch design and preserve the
post-restore fence. S06 immediately hides restricted Work policies from public readers and
supplies no restricted serving path. Unit override persistence/activation/UI remain S10; the
pure core already prevents a child grant from bypassing its Work or publishing an absent Unit.
The [draft model inventory](../../services/cantos-server/behavior/catalog/viewer_access.md)
states its finite assumptions and separate native evidence. Browser/native and production
acceptance, along with the existing formal auth/restore gaps, remain separate obligations.
Queue: no reorder; S07 still consumes this boundary after it is integrated.


### S07 consumer contract (#653)

The [authenticated shared module](../../services/cantos-server/contracts/theatre/shared/v1/README.md)
uses the current generation and the S06 Work AND Unit evaluator. The current persisted Unit
mode is INHERIT; S10 must extend this common admission before exposing overrides. Candidate
indexes (v34) narrow actor/group Work IDs, dedup before page bounds and never grant access.
Hydration is bounded per request; its per-Work statement cost and the conservative global
publication/policy/group cursor fence require S11 measurement before any performance claim.
No ACL cache, asynchronous projection, FTS or MVCC is introduced.

Shared `/works`, `/search`, Work/Units, manifest, playback authorization and media routes
reuse Discovery's strict shapes without switching current clients. Cursors bind session, query,
representation and the current publication/access fence. Grant HMAC subjects additionally bind
purpose `media`, exact bytes and a new activation epoch for the process. Deadline admission
is bounded by session/access/rights/grant/member expiry; the clock is read again after SQL.
Every fetch is re-admitted before conditional/Range planning and after object-open I/O.

The explicit `router_with_restricted_media` trusted composition opts in for one process;
ordinary executable assembly remains closed on startup and restore, requiring later policy
confirmation/activation work. Shared cover output is null and no private cover route is
activated. Public artwork, old manifest/media and Library/restore cannot bypass a restricted
policy. Library's public projections now require the current generation association rather
than its former release-only relation.

S08 consumes the cookie contract and qualifies the real browser player; S09 sends Bearer on
every native fetch/seek/refresh and qualifies Android and iOS separately. There is no URL-only
fallback in this implementation. Unsupported transports and private Library restoration remain
unavailable. S12 owns the disposable restore-pair drill and any production activation; no
production effect follows from the local router/TCP/byte tests. Admitted response bodies retain
the existing 60-second bound and already buffered bytes cannot be recalled.


### S08 Web consumer boundary (#654)

Studio Work Details mounts an app-owned Access panel on the existing workspace/request/draft
owners. It consumes S06 DTOs/fixtures and keeps reads, historical operation receipts and dirty
input separate. Audience confirmations name the policy/publication read versions; the Work
policy applies to current published INHERIT units and never publishes a draft. Public widening
still requires the Server's settling rights gate. Email text is submitted unchanged to Identity
canonicalization; pending never means invitation delivery or account lookup. Groups are listed
and can receive Work grants; creation is supported. Global member removal/revoke stays closed
without the complete affected-Work impact contract. Claimed/expired/revoked share history is
unavailable in the current state projection and is explicitly identified as such.

Theatre Web adds Shared after Continue, explicit claim and the generic `#shared/{work}` login
return/detail locator. It consumes the S07 strict generation shapes and current cookie lease.
No private cover is invented. Private metadata/cursors/receipts stay in the current session's
RAM; route/account changes cancel reads and fence late results. Private source identities carry
the Theatre session, use shared manifest/authorization paths and require the exact configured
API media origin. The retained audio adapter selects credentialed CORS for shared sources, but Web listen controls
and their imperative entrypoint are unavailable. The teardown panic that the real local campaign
found with an active private source on admission loss is repaired
([evidence](../evidence/private-web-teardown-676.md)); qualifying HTTPS/cookies/seek, revoke and
stopping an active source when a Shared read is refused remain #654 acceptance carried to the
#676 review ledger. No runtime switch activates it.
Metadata-only navigation uses immediate fenced state purge without notifying departed controls.
Logout or a current denial is intended to clear private media/display facts; the active-source
browser claim is unaccepted. Restricted Saved/History/Continue,
bookmark and restore remain unavailable rather than projecting cached private metadata through
a public API. New Web session lifecycle/native DOM observations remain separate from the
backend's finite draft models, browser media, CMP, native and production acceptance.

S09 consumes the service-owned shared/viewer-access contracts, not this Web runtime. It must
qualify Bearer on each Android/iOS open/seek/refresh separately and preserve exact release pins,
no public fallback, session purge and the unavailable private restore boundary. S10 still owns
persisted Unit overrides and S12 owns restore/activation. No shared component API, global Studio
navigation, migration or production activation is selected by S08.

### S09 CMP consumer boundary (#655)

Android and iOS composition roots supply a separate shared metadata reader through their
existing native Identity coordinator. Library places its app-owned Shared composition after
Continue; a neutral typed Work locator retains the optional login return. Authenticated
20-row pages and Work/Unit detail reuse the providing service's strict generation shape,
without implementing the public cache port. Private data/cursors stay in the visible route's
RAM, bounded to 1,000 retained rows. Session incarnation/route/request fences, cancellation and
a synchronous root admission check discard old account data and late replies. A current denial
or invalid cursor discards the whole walk; private cover and durable Library/restore stay
unavailable. No OS share App Link or native pending-share claim is introduced.

The native media transports remain a delivery gap: Android currently hands a URL to
`MediaPlayer`, and iOS hands a URL to AVPlayer. They do not yet establish Bearer on every
open/seek/refresh and redirect refusal. Private Play therefore has a visible reason and no
dispatch path; actual Android/Kotlin iOS/Swift decoder edges also refuse shared-media and
session-required locations and clear their prior source/display facts. No public fallback,
bearer-URL substitute, second player or activation override is added. S09 acceptance remains
incomplete until safe media sources and named Android/iOS/runtime/accessibility evidence exist.
JVM semantics and APK compilation do not qualify those native behaviors. S10 may consume the
unchanged service contract and must preserve the private native Play boundary; S12 still owns
restore/activation. The source-bound [S09 record](../evidence/cmp-shared-catalog-655.md) and single
review ledger #676 keep those gaps distinct from merged implementation progress.


### S10 Unit policy, chapter matrix and HIDE (#656)

The Work policy command now replaces an exact Unit's INHERIT/restricted policy using the same
Work CAS/event/receipt. Independent Work and Unit grants/groups intersect; child authority never
publishes or bypasses its parent. A new append-only migration registers normalized current Unit
projections and a synchronous actor/archive visibility clock. All common public/shared metadata,
count/ordinal/page and every-fetch seams include current Unit admission. Public-only Library
filters denied rows before pagination, retains account records, and issues scope-bound cursors.
Outward Unit positions are consecutive visible ordinals, not published gaps.

Studio Work Details adds a paged 20-row chapter matrix with exact saved/published pins, current
parent gate and Unit policy. Audio association is labeled separately from live readiness.
Only an explicitly loaded row can be selected; policy replacement preserves dirty input and
requires explicit reload/compare after conflict. Per-Unit publish/withdraw keeps existing Work
metadata and other entries through the existing generation CAS. Web/CMP cursor rejection clears
old rows/count/queue candidates; fresh Unit loading starts with fresh Work detail. CMP public
persisted metadata is no longer accepted as present authority.

No private Web/native Play, restricted Library restoration, private cover or production serving
is activated. The source-bound [S10 evidence](../evidence/unit-policy-hide-656.md) separates
core/model/Turso/HTTP/Studio/Web/CMP evidence and unresolved browser/device/formal acceptance.
S11 receives the principal-lane/two-bounded-Unit-query/Library predicate/scope query inventory;
it must measure actual Turso costs rather than infer performance from bounded output. S12
still owns paired restore/activation acceptance. The single #676 ledger retains inherited and
S10 residuals; merged source alone does not establish all #656 acceptance criteria.

### S11 publication/access measurement boundary (#657)

The [source-bound measurement report](../evidence/publication-access-performance-657.md)
records the opt-in dataset extension, actual plans/costs and pending owner budget/adoption
decisions. It does not certify populated Library, mixed-write performance or production
capacity. S12 consumes these observations with the explicit limitations retained.

### S12 integration/recovery boundary (#658)

The [source-bound S12 ledger](../evidence/publication-access-readiness-658.md) maps epic AC01–26
to actual native/source/browser/device evidence and open criteria. The
[operator recovery runbook](../deployment/publication-access-recovery.md) records a NO_GO:
verified quiesced snapshots preserve schema/immutable data, and default restricted delivery stays
closed after restore, but restoring both files from before logout/disable resurrects the old
session. Identity restore epoch rotation, Server resync and post-cut policy/revocation authority
are unsupported; an old pair stays offline. Hashes/startup and default restricted OFF do not
establish full security recovery. No new security protocol, activation switch, production
operation or accepted performance/RPO/RTO/retention value is delivered by this slice.

The bounded schema-test repair names the three existing v35 tables omitted by its expected set;
two existing viewer-access registry index/count omissions are also corrected without changing
model semantics/lifecycle. Registered SQL/history remain unchanged. Private Web/native playback, cover/Library restore,
formal/UI qualification and named browser/device gaps remain separate in the single #676
review ledger. Source merge and a completed readiness dossier do not accept those layers.
