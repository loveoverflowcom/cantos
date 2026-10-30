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
