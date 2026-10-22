# Public Work cover implementation handoff

Review boundary: make [#171](https://github.com/loveoverflowcom/cantos/issues/171)'s next
implementation review reproducible without selecting the unaccepted public-image policy.
[ADR 0033](../decisions/0033-release-pinned-public-cover.md) owns the proposed cover rule;
this handoff adds neither acceptance nor a second artwork contract. GitHub owns execution
status. The source inventory below is pinned to `origin/develop`
`f6204e8bc8cb5361f93917c09dfe9d01aa6ef359`; re-read the issue, comments and current decisions
before resuming implementation.

## Entry gate and owner receipt

Issue #171 explicitly requires implementation to remain blocked if
[#157](https://github.com/loveoverflowcom/cantos/issues/157) has not selected delivery.
ADR 0033 remains proposed, with its decision owner unassigned. Its existing kernel and draft
behavior model are review inputs, not permission to expose an image. The
[work-item prerequisite rule](../../.agents/skills/cantos-work-item/SKILL.md#2-confirm-the-prerequisites-are-accepted)
does not let a merged proposal substitute for owner acceptance.

Product, Catalog and Publication owners must record the following in #157 or its accepted
decision before the corresponding runtime work. Assignment is required; this table assigns
no person and accepts none of ADR 0033's recommendations.

| Receipt item | Decision required for this artwork slice |
| --- | --- |
| Authority and scope | Who can disclose/revoke an exact image, how that authority is authenticated, how image rights differ from private admission and audio approval, and whether disclosure expires |
| Release identity | Exact representative-release/Work-revision binding, legacy releases with no image, replacement across units, clear, correction and withdrawal behavior; reconcile any departure from ADR 0033's kernel |
| Delivery and privacy | Chosen image delivery mode, publication-class object isolation, actual-byte identity, GET/HEAD behavior, denial semantics and races with revoke/withdrawal/visibility changes |
| Cache and revocation | Immutable image/version identity, validators, cache authorization and retention policy, stale capability behavior and the permitted revocation lag; digest identity alone does not revoke a cached image |
| Wire and rollout | Approved public fields and absence/null semantics, explicit compatibility/version boundary, actual JSON/Protobuf route coverage, old-reader behavior and rollback |
| Consumers and acceptance | Shared Web/CMP fixture ownership and decoding method, fallback semantics, exact acceptance evidence links and remaining independently gated work |

Only the applicable P0 artwork and wire decisions are needed here. This does not require or
accept #157's unrelated P1 genres, contributors, seasons, markers or transcripts. Conversely,
accepting those P1 rows would not accept this P0 gate.

## Existing seams to reuse after acceptance

| Owner / implemented seam | What it supplies | What it does not supply |
| --- | --- | --- |
| [Private Artwork v1](../../packages/contracts/artwork-v1.md), [Media artwork SQL](../../services/cantos-server/src/contexts/media/postgres/artwork.rs) | Owner-only image admission and immutable set/clear history; `effective` reads the last change at or before an exact Work revision, retaining clear tombstones | Public rights, an anonymous preview URL, publication promotion or public cover DTO |
| [Listener publication SQL](../../services/cantos-server/src/contexts/media/turso/listener.rs), [Discovery v1](../../packages/contracts/theatre-discovery-v1.md#public-eligibility-and-representative-metadata) | Current eligibility plus exact release-pinned metadata; shared representative selection for list/detail; bounded batched reads | Image/disclosure resolution; present-day private head is not the release's cover |
| [Pure cover kernel](../../services/cantos-server/src/contexts/media/domain/public_cover.rs) | Proposed `project_public_cover` over eligible `ListenerPublication`, artwork history, exact admitted asset and explicit disclosure; typed total fallback | A production caller, persisted disclosure authority or public wire serialization |
| [Theatre DTO](../../packages/contracts/api/src/theatre.rs), [wire projection](../../services/cantos-server/src/contexts/media/listener_projection.rs) | Strict existing `WorkSummary` for list/detail, with no cover field | Automatic compatibility for an optional addition: `deny_unknown_fields` readers reject extra properties |
| [Private storage](../architecture/object-storage.md), [origin playback seam](../../services/cantos-server/src/contexts/media/listener_delivery.rs) | Existing private adapters and a separate composition-gated audio `PublicationObjects` port | A promoted public-image adapter; private storage has no promotion/ACL operation, and the audio route is not an image endpoint |

[ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md) is also proposed and concerns
audio. Its grant TTL, MIME, URL shape and headers do not select image delivery. Do not add an
image response to a Studio handler or widen the existing audio route as a substitute for #157.
The [P1 wire review](../decisions/0030-release-safe-public-metadata.md) identifies the strict-reader
hazard but supplies no accepted Theatre Protobuf field numbers or cover DTO.

## Next review sequence

1. Record the owner receipt above and reconcile the owning ADR/contract. Preserve private
   artwork authority; no existing owner URL, rights reference or receipt becomes public.
2. Review one consumer-ready projection contract and shared fixture set before changing
   serialization. Preserve old v1 fixtures and test both old-reader and new-reader rollout.
   Keep raw PNG/JPEG bytes outside metadata envelopes; document transport per actual route.
3. Reconcile and verify the pure cover rule against that contract, then supply its facts through
   Media's application and Catalog's existing metadata facade. Reuse exact revision/history
   semantics and fail closed on cross-owner/image/release mismatches. A broken optional cover
   must not suppress an otherwise eligible title or playable release.
4. Add the reviewed disclosure persistence and bounded read path, with additive migrations
   only where needed. Before changing reads, follow the
   [PostgreSQL read workflow](../../.agents/skills/cantos-engineering/references/postgresql-read-performance.md):
   inventory list/search/detail, retain representative-first filtering and opaque keysets,
   compare query counts/plans and avoid per-item artwork queries. No measured need for DBSP
   or another storage service is established here.
5. Integrate the accepted publication/storage delivery boundary, then real SQL and byte-path
   acceptance below. No disclosure defaults, private-bucket public ACL, credentials or live
   configuration are authorized by this handoff. Keep public activation closed until its gates
   pass; consumer adoption remains with Web #158 and CMP #159, alongside #13/#14 and #129/#130.

## Acceptance matrix for the later implementation

These are required scenarios to implement after the policy is accepted, not runnable test
names or results. Match the assertions to the accepted decision if it differs from ADR 0033.
The [existing core evidence](../evidence/public-media-delivery-core.md) remains historical and
does not cover the missing SQL, storage or consumer path.

| Scenario | Required oracle and layer | Remaining dependency / next owner |
| --- | --- | --- |
| Publish A, privately replace with B, then clear | Disposable real SQL: list and detail still select A for the same eligible release; restart preserves the binding; no private B metadata or bytes escape | Accepted pinning rule; Catalog/Media |
| Publish B, then withdraw its release | Real SQL: the same representative algorithm used for title selects the corresponding cover (or no eligible Work); no independent cover selector revives a withdrawn release | Accepted representative/withdrawal rule; Publication/Media |
| No image, missing asset, foreign owner/Work/release, corrupt history or byte mismatch | Exact core fallback and API assertions; title/units remain available when independently eligible; private sentinels absent from both success and errors | Accepted failure contract; Media/API |
| Undisclosed, revoked, expired or wrong-image disclosure | Exact pure denial plus real SQL/HTTP checks; cache and previously issued delivery access obey the accepted withdrawal/lag policy | Accepted authority and revocation policy; Publication/Media |
| Revoke, hide, archive or withdraw while bytes open | Fault injection at authorization/open/recheck/response boundary on real storage; no byte sent outside the accepted access window | Chosen delivery mode and controllable fixture; Media/storage |
| Public GET/HEAD, missing/corrupt object and retained old identity | Actual allowed PNG/JPEG bytes, hash/length/MIME/dimensions and response headers through the intended host; private reads deny anonymous and foreign-owner access | Publication-class adapter, authorized disposable storage/host; storage/deployment |
| Strict old/new reader and metadata privacy | JSON schema/DTO fixtures plus applicable Protobuf goldens; serialized output excludes provenance, rights references, owner, receipt, storage key and Script IR | Accepted wire boundary; contracts/Web/CMP |
| Same fixtures on Web and CMP | Decoding/drift checks and named no-image/error/revocation observations; image failure preserves title and listening; fallback accessibility/text behavior observed on each affected surface | Consumer implementation and local browser/device; Web #158/CMP #159 |

Use the [verification cadence](../../.agents/skills/cantos-engineering/references/verification-cadence.md)
for focused development checks, affected-layer checkpoints and final-diff
`cargo xtask preflight --changed-only`; reuse attributable results and do not repeat suites
already covered for the same inputs. The later implementation's affected suites include
`cargo xtask verify rust`, `cargo xtask verify contracts` and `cargo xtask verify postgres`
(a disposable native cluster, never an inherited database). Select behavior/conformance and
renderer checks according to the actual changed inputs and the [xtask contract](../../xtask/README.md#preflight).
These commands cannot establish a cover test exists: register the new SQL/storage scenarios
in the canonical gates before citing a result.
Real provider/deployed-host or device acceptance needs the named authorized environment;
neither a mock fixture nor an agent-authored hash is that evidence.

## Handoff evidence boundary

This documentation-only slice provides a source inventory, decision receipt and acceptance
plan (`documented`). Its final-diff local gate output and independent review belong in its PR.
It changes no domain, DTO, schema, migration, fixture, storage object or client behavior.
No release or manifest is produced; no cache/audio identity, rights or approval is changed.
No performance measurement is claimed. L2/L4 implementation evidence is inapplicable here;
runtime SQL/API/storage/browser/device acceptance for #171 remains **NOT_RUN / BLOCKED** on
the prerequisite policy and subsequent implementation. The pure kernel's inherited evidence
does not complete any of #171's end-to-end acceptance criteria. Keep #171 open until its
required contract, projection, real SQL and delivery/consumer evidence are actually accepted.
