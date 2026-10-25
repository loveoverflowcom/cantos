# Public Work cover implementation handoff

Review boundary: make [#171](https://github.com/loveoverflowcom/cantos/issues/171)'s
implementation review reproducible against the accepted policy. The accepted
[ADR 0033](../decisions/0033-release-pinned-public-cover.md) owns the cover rule and the
[Discovery v1.2 section](../../services/cantos-server/contracts/theatre/discovery/v1/README.md#public-cover-v12-accepted-not-served)
owns the wire; this handoff adds no second artwork contract. GitHub owns execution status.
The source inventory was first pinned to `f6204e8bc8cb5361f93917c09dfe9d01aa6ef359` and
refreshed for the #157 decision at `e082d2d8df51056678611361d0c39b590e650acf` (native Turso
`server.db`); re-read the issue, comments and current decisions before resuming implementation.

## Entry gate and owner receipt

The owner accepted [ADR 0033](../decisions/0033-release-pinned-public-cover.md) for
[#157](https://github.com/loveoverflowcom/cantos/issues/157) on 2026-10-25, so #171's policy
prerequisite is met. The receipt below records each selected answer and its owning text; it
accepts no implementation, which still needs the acceptance matrix further down.

| Receipt item | Accepted answer (owner) | Still open for #171 |
| --- | --- | --- |
| Authority and scope | Current active Work owner records an append-only, digest-bound clearance with a required `valid_until_ms`, or a revocation; latest by server sequence wins; separate from admission, visibility, clearance and audio approval | Studio route/DTO shape, idempotency key and UX copy; reviewed alongside #171 |
| Release identity | Image effective at the Work revision pinned by the representative release; private replacement/clear changes nothing until a newer release pins it; withdrawal hides it; no backfill | None beyond the real SQL scenarios |
| Delivery and privacy | Origin GET/HEAD `/api/v1/theatre/works/{work_id}/cover/{sha256}`, re-projected on every request; one anonymous `not_found` for every denial | Composition gate, private-object read and actual-byte check |
| Cache and revocation | Strong `ETag: "{sha256}"`, `Cache-Control: public, no-cache`; revocation effective on the next request | Exact CORP/CORS headers for deployed origins (ADR 0041); CMP offline retention |
| Wire and rollout | Discovery v1.2 `presentation=2` → `CoveredWorkPage`/`CoveredWorkSummary` with required nullable `cover`; default and `presentation=1` unchanged; JSON only | Admitting `presentation=2` in the closed query and cursor identity |
| Consumers and acceptance | [Contract section](../../services/cantos-server/contracts/theatre/discovery/v1/README.md#public-cover-v12-accepted-not-served) and `fixtures/cover/`; Web #158 and CMP #159 adopt after #171 | Web/CMP decoding and fallback observations |

#157's P1 metadata rows are separate ([ADR 0030](../decisions/0030-release-safe-public-metadata.md#157-capability-matrix));
accepting them would not change this P0 gate.

## Existing seams to reuse after acceptance

| Owner / implemented seam | What it supplies | What it does not supply |
| --- | --- | --- |
| [Private Artwork v1](../../services/cantos-server/contracts/studio/artwork/v1/README.md), [Media artwork SQL](../../services/cantos-server/src/contexts/media/turso/artwork.rs) (Turso; [historical PG](https://github.com/loveoverflowcom/cantos/blob/7057ab41720b9bb341b27bae8a08f0ea7211b6ae/services/cantos-server/src/contexts/media/postgres/artwork.rs)) | Owner-only image admission and immutable set/clear history; `effective` reads the last change at or before an exact Work revision, retaining clear tombstones | Public rights, an anonymous preview URL, publication promotion or public cover DTO |
| [Listener publication SQL](../../services/cantos-server/src/contexts/media/turso/listener.rs), [Discovery v1](../../services/cantos-server/contracts/theatre/discovery/v1/README.md#public-eligibility-and-representative-metadata) | Current eligibility plus exact release-pinned metadata; shared representative selection for list/detail; bounded batched reads | Image/disclosure resolution; present-day private head is not the release's cover |
| [Pure cover kernel](../../services/cantos-server/src/contexts/media/domain/public_cover.rs) | Accepted `project_public_cover` over eligible `ListenerPublication`, artwork history, exact admitted asset and explicit disclosure, typed total fallback; `admit_cover_fetch` serves only the currently shown SHA-256 | A production caller, persisted disclosure authority or route |
| [Theatre DTO](../../services/cantos-server/src/contract/theatre/discovery.rs), [wire projection](../../services/cantos-server/src/contexts/media/listener_projection.rs) | Strict `WorkSummary`/`PresentedWorkSummary` without a cover; accepted v1.2 `CoveredWorkSummary`/`CoveredWorkPage`/`CoverImage` DTOs with schema/fixture conformance | A projection that fills them or a query that selects them |
| [Private storage](../architecture/object-storage.md), [origin playback seam](../../services/cantos-server/src/contexts/media/listener_delivery.rs) | Existing private adapters and a separate composition-gated audio `PublicationObjects` port | A promoted public-image adapter; private storage has no promotion/ACL operation, and the audio route is not an image endpoint |

[ADR 0032](../decisions/0032-theatre-origin-playback-delivery.md) concerns audio (accepted for
step-1 origin integration). Its grant TTL, MIME, URL shape and headers do not select image delivery. Do not add an
image response to a Studio handler or widen the existing audio route; the accepted cover
route is its own Theatre handler. Theatre stays JSON; no Protobuf field numbers are selected.

## Next review sequence

1. Done by #157: owner receipt, accepted ADR and Discovery v1.2 contract. Preserve private
   artwork authority; no existing owner URL, rights reference or receipt becomes public.
2. Done by #157: the consumer-ready contract and `fixtures/cover/` with old-reader rejection
   tests. #171 keeps old v1 fixtures unchanged and adds the served-response conformance.
   Keep raw PNG/JPEG bytes outside metadata envelopes.
3. Reconcile and verify the pure cover rule against that contract, then supply its facts through
   Media's application and Catalog's existing metadata facade. Reuse exact revision/history
   semantics and fail closed on cross-owner/image/release mismatches. A broken optional cover
   must not suppress an otherwise eligible title or playable release.
4. Add the reviewed disclosure persistence and bounded read path, with additive migrations
   only where needed. Before changing reads, follow the
   [Turso read workflow](../../.agents/skills/cantos-engineering/references/turso-read-performance.md):
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
`cargo xtask verify rust`, `cargo xtask verify contracts` and `cargo xtask verify turso`
(file-backed `server.db` fixtures, never an inherited database; `verify postgres` is retired). Select behavior/conformance and
renderer checks according to the actual changed inputs and the [xtask contract](../../xtask/README.md#preflight).
These commands cannot establish a cover test exists: register the new SQL/storage scenarios
in the canonical gates before citing a result.
Real provider/deployed-host or device acceptance needs the named authorized environment;
neither a mock fixture nor an agent-authored hash is that evidence.

## Handoff evidence boundary

The original slice provided a source inventory and acceptance plan (`documented`). The #157
decision slice adds the accepted receipt above, the Discovery v1.2 schema/fixtures with shared
Rust DTOs and conformance tests (`contract-tested`) and the pure `admit_cover_fetch` rule
(`example-tested`). It changes no migration, route, query admission, storage object or client
behavior. No release or manifest is produced; no cache/audio identity, rights or approval is
changed. No performance measurement is claimed. Runtime SQL/API/storage/browser/device
acceptance for #171 remains **NOT_RUN** until its implementation. The pure kernel's inherited evidence
does not complete any of #171's end-to-end acceptance criteria. Keep #171 open until its
required contract, projection, real SQL and delivery/consumer evidence are actually accepted.
