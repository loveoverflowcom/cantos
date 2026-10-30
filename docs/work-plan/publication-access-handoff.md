# Publication lifecycle and Work/Unit access: design handoff (#634)

This is the S01 ([#647](https://github.com/loveoverflowcom/cantos/issues/647)) **source map,
boundary design and evidence plan** for epic [#634](https://github.com/loveoverflowcom/cantos/issues/634).
[ADR 0054](../decisions/0054-publication-generation-and-access.md) owns the D01–D08 choices and
their owner questions; this file owns the inventory, the proposed state/transaction/read
boundaries, the compatibility matrix, the behavior impact plan, the acceptance mapping and the
per-slice resume conditions. Everything here is **proposed** unless it cites an accepted record.
GitHub owns execution status and acceptance.

## Review boundary and baseline

| | |
| --- | --- |
| Delivers | Decision ledger (ADR 0054); this source map and boundary design; compatibility and rollback oracle; behavior impact plan; AC→slice→owner→oracle mapping; workload and recovery plan; successor contracts |
| Does not deliver | Any DTO, protobuf field, schema, migration, endpoint, query, Quint model, registry entry or client change; any accepted policy |
| Baseline | `develop@aa0bec8d`. The issue snapshot `d9f79480` differs only by #659/#660 (Theatre search UI, chips, localization); no publication, catalog, identity, contract or behavior source changed |
| Turso review input | [Epic addendum](https://github.com/loveoverflowcom/cantos/issues/634#issuecomment-6493415800), sourced at `75d4b795`; its schema/index/backup points are hypotheses, carried into D02/D08 and the workload plan |
| Closes | Nothing; S01 is `Part of #634` and of #647 |

## Source map at the baseline

| Path | Current fact | Role / owning slice |
| --- | --- | --- |
| [`catalog_publication_read.sql`](../../services/cantos-server/src/platform/turso/catalog_publication_read.sql) | `head_revision` on Works/Units; immutable digest-bound revisions; mutable `catalog_units.position`; lifecycle `draft`/`archived` | Generation pins revisions, never position (S02) |
| [Catalog v1](../../services/cantos-server/contracts/studio/catalog/v1/README.md) | Owner mutates; editor members edit metadata/children/order; archive, rights refs and visibility owner-only; ≤1,000 Units per Work | D01 baseline; D02 bound |
| [Publication v1](../../services/cantos-server/contracts/studio/publication/v1/README.md), [`publication.rs`](../../services/cantos-server/src/contexts/media/domain/publication.rs) | Exact-revision releases, clearance/approval, `MetadataChanged` blocker, owner-only | D06; S03 keeps every gate |
| [`audio_release_chain.sql`](../../services/cantos-server/src/platform/turso/audio_release_chain.sql) | Per-Unit `audio_publication_pointers(version)`; append-only withdrawals; immutable `audio_listener_works/units` anchors | Association target (S03); legacy backfill input (S02/S03) |
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

## State and transaction boundary (proposed)

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
transaction; eligibility filters before pagination, count, ordinal and ranking. Public cursors
keep binding presentation and query over immutable anchors; shared cursors also bind actor and
access scope. Projection rows commit with the publication, so there is no projection lag in v1.

## Access, identity, preview and media boundaries (proposed)

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
  by the Server.
- **Groups (S06).** Owner-scoped, flat, versioned membership; a group name is never identity;
  no Studio role is derived from membership.
- **Preview (S04, D03).** Exact saved revision set; `private, no-store`, no-referrer; no
  OpenGraph, sitemap, analytics or listening history; missing private asset is unavailable, never
  a public substitute; logout/account switch fences late responses.
- **Media (S07–S09, D08).** Grant issuance and every GET/HEAD/Range recheck subject, session,
  Work AND Unit policy, expiry and exact release identity. Web transport is qualified in real
  browsers; Android/iOS each record their own player result; an unrun target stays `NOT_RUN`.

## Compatibility matrix and rehearsal oracle

| Reader | Legacy-lane Work | Generation-lane PUBLIC Work | Generation-lane RESTRICTED Work |
| --- | --- | --- | --- |
| Default v1, `presentation=1`, `presentation=2` | Unchanged | Released Units in the active generation only; release-pinned text (old semantics) | Absent (fail closed) |
| `presentation=3` | Unchanged sample union | Q-D07c | Absent |
| Next `presentation` level (D07a) | Release rows in the new shape | Generation rows; `playback: null` or an associated release | Absent |
| Authenticated shared module | None | Optional dedup with public | Rows the caller may see |
| Old client build | Never sends new levels; unaffected | Same as its level | Absent |

Rehearsal oracle for S02/S03 (real files, not mocks): from the prepared sample root and a legacy
release fixture, capture default/1/2/3 responses; run the backfill; assert old-level responses are
byte-identical for PUBLIC Works, the new level lists the same public set with correct playback,
draft/unknown/`visibility=public`-without-release Works stay absent, sample withdrawal stays
permanent; reopen the files; rerun the backfill and assert no new rows. Old strict-reader
fixtures stay byte-for-byte; new fixtures are added beside them.

Rollback: the registry refuses an older binary on a newer `server.db`, so rollback means restoring
a pre-migration backup. That restore can predate later withdrawals or revocations; restricted
delivery therefore stays disabled after restore until the owner re-confirms policies (Q-D08d).
Never "roll back to run" by removing a gate.

## Behavior SSOT impact

Lookup: catalog → manifests → inventories → models → code, at the baseline. S01 is a design task:
**no catalog, manifest, model or run is written**; every model delta waits for its accepted row.

| Capability (lifecycle) | Relevance | S01 class | Planned delta, slice, prerequisite |
| --- | --- | --- | --- |
| `cantos.publishing.lifecycle` (draft, `model: null`) | Audio pointer/withdrawal; association | needs-decision + missing model | S03 records the association scope once D06 is accepted; no `.qnt` before an exact observation scope |
| none (candidate: metadata generation) | Generation, CAS, Work/Unit selection | missing capability | S02 registers a bounded draft at the generation domain owner after D02/D05 acceptance; candidate ID chosen there |
| none (candidate: Work/Unit access policy) | Work AND Unit, INHERIT, revocation monotonicity, no email principal | missing capability | S06 after D04/D05 acceptance; witnesses and negative controls per property |
| `cantos.media.sample-metadata` (draft, sample-scoped) | Backfill input | no change | Keep scope; S03 only if Q-D07c changes sample semantics |
| `cantos.media.public-playback-delivery` (draft) | Restricted grant/fetch | needs-decision | S07 adds subject/session binding after D08b/c |
| `cantos.media.public-cover-projection` (draft) | Cover stays release-pinned | no change | — |
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
| 19 Contract compatibility | S01–S10 | Each providing service | Old fixtures byte-stable; unsupported restricted fails closed |
| 20 Migration/rollback | S01, S02, S03, S06, S07, S12 | Publication, Ops | Rehearsal oracle above; post-restore fence |
| 21 Behavior SSOT | S01, S02, S03, S04, S06, S07, S10 | Behavior owners | Impact table above; witnesses and controls |
| 22 Backend qualification | S02, S03, S04, S06, S07, S10 | Server | Pure, Turso, HTTP, Identity RPC, bytes as separate rows |
| 23 Browser qualification | S04, S05, S08, S10 | Web | Real browser journeys, keyboard/focus/VI-EN |
| 24 CMP qualification | S05, S09, S10 | CMP | Android/iOS each; unrun stays `NOT_RUN` |
| 25 Performance | S01, S11 | Server, #384 | Budgets recorded first; plans and measurements on 0.8.3 |
| 26 Handoff/rollout | S01, S12 | All | Commands, SHAs, targets, deferred production acceptance |

## Workload, budget and recovery plan

S01 records what the owner must choose (Q-D08a/d); no number below is a target.

| Input for S11 | Owner value |
| --- | --- |
| Works; Units per Work (≤1,000); generations per Work | to choose |
| Grants per Work; groups per owner; members per group; shared Works per actor | to choose |
| Query mix: public list/search/detail/units, shared list, preview, grant issue, fetch admission, publish, revoke | to choose |
| Concurrency (readers with publish/revoke), cold and warm runs | to choose |
| p95 budgets per query class; publish write latency; RSS | to choose |

Method: `cargo xtask seed dataset` profiles ([localhost datasets](../deployment/localhost-datasets.md),
#568) and the [Turso read method](../../.agents/skills/cantos-engineering/references/turso-read-performance.md)
(#384): query plans on pinned 0.8.3, p50/p95/p99, statements per request, rows scanned/returned,
DB and index bytes. Index candidates from the Turso review (target-first grant index,
principal-first index, partial index on ACTIVE PUBLIC) stay hypotheses until measured. FTS, MVCC
and caches are out of scope unless measured benefit and parity evidence exist.

S12 recovery constraints: `backup_to` per file; `server.db` and `identity.db` restored as a pair
on disposable copies; no live file copy; owner-chosen RPO, RTO and retention; the post-restore
restricted-delivery fence; production restore and activation remain separate tasks.

## Successor handoff

A successor may start only when the listed rows are accepted and the predecessor's contract is in
the base; before that, only independent reading or a labeled prototype the user approves.

| Slice | Needs accepted | Receives | Must not |
| --- | --- | --- | --- |
| S02 [#648](https://github.com/loveoverflowcom/cantos/issues/648) | Q-D01a, Q-D02a–c, Q-D05a–b, Q-D07a–b | This boundary, rehearsal oracle | Read current heads/position into a generation; enable restricted |
| S03 [#649](https://github.com/loveoverflowcom/cantos/issues/649) | Q-D06a–c, Q-D07c | S02 schema, fixtures, generation API | Rebind audio implicitly; broaden the sample model |
| S04 [#650](https://github.com/loveoverflowcom/cantos/issues/650) | Q-D01b, Q-D03 | S02/S03 contracts | Forward Studio credentials to Theatre |
| S05 [#651](https://github.com/loveoverflowcom/cantos/issues/651) | S02–S04 rows | Server fixtures | Choose backend semantics in UI |
| S06 [#652](https://github.com/loveoverflowcom/cantos/issues/652) | Q-D01a, Q-D04a–c, Q-D05a | Identity RPC design above | Grant from email text; activate restricted |
| S07 [#653](https://github.com/loveoverflowcom/cantos/issues/653) | Q-D08b–c | S06 core | Activate before every read/bytes path is gated |
| S08 [#654](https://github.com/loveoverflowcom/cantos/issues/654) | Q-D04, Q-D08c | S07 contracts | Show sharing UI the backend cannot enforce |
| S09 [#655](https://github.com/loveoverflowcom/cantos/issues/655) | Q-D08c | S07/S08 contracts | Use stale offline restricted metadata as access |
| S10 [#656](https://github.com/loveoverflowcom/cantos/issues/656) | Q-D02, Q-D05a | S06–S09 | Add LOCK/Unlisted |
| S11 [#657](https://github.com/loveoverflowcom/cantos/issues/657) | Q-D08a | S10 | Claim O(1) or unmeasured latency |
| S12 [#658](https://github.com/loveoverflowcom/cantos/issues/658) | Q-D07, Q-D08d | S11 | Tick epic AC from partial PRs; deploy without a separate task |
