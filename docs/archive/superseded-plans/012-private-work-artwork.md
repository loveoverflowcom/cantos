# Private Work cover: the small Catalog / Media MVP

> **Historical snapshot — archived 2026-10-16.** Original path: `docs/work-plan/012-private-work-artwork.md`.
> Retained for traceability; dates, commands, permissions, blockers and task status below
> describe the original session and are not current execution instructions. Use the
> [current priorities](../../work-plan/current-priorities.md) and [documentation map](../../README.md) before starting work.

Status: **IO_ACCEPTANCE_PENDING**. The original 2026-10-13 planning requirements are retained;
implemented and executed scopes are recorded in the checkpoint below and its evidence ledger.
Source request: [#104](https://github.com/loveoverflowcom/cantos/issues/104), plus the user's
explicit instruction to select necessary MVP business fields/API/DB contracts in one planning
session, followed by a new, separate implementation session. Do not infer implementation
authorization from the research issue alone. The original planning commit contained no product
code, migration or external publication.

## 1. One reviewable outcome

An authenticated Work owner can admit one externally prepared PNG/JPEG cover into private
storage, inspect its exact accepted metadata/bytes, and select, replace or clear that cover with
an optimistic Work-revision check. Historical selections, receipts and bytes remain immutable.
That is the complete vertical slice. A stored cover is not a public image or a release approval.

### Why this subset now

- Work/unit identity, text/source/script revisions, private Opus admission, audio review,
  publication core and discovery DTOs already exist. Duplicating them adds no MVP value.
- The inspected Theatre Web mock has bundled cover art and a typographic fallback in
  apps/cantos-theatre/web/src/features/common.rs. Cover cards/details/library/player surfaces
  demonstrate the Work-level need. These are synthetic consumers, not a live artwork API.
- Studio has a real Work/content workspace, but no image control or image contract. This slice
  makes owner/API-based preparation, durable cover choice and exact private preview possible.
  It unblocks a later bounded Studio control without claiming that control is implemented now.
- One Work cover is enough for the first catalog identity. There is no demonstrated current
  per-unit artwork authoring requirement; do not add episode overrides or inheritance policy.
- Public audio delivery is already unavailable. Adding an image publication/copy/CDN pipeline
  would broaden the review boundary while its prerequisite is incomplete. Theatre responses
  stay unchanged and its typographic/mock fallback remains truthful.
- No P1 field is necessary to make this private cover workflow work. Mock genres do not choose
  a production taxonomy; a reference standard does not create a credits/transcript use case.

Non-goals: RSS/XML/feed runtime; every P1 metadata family; new WorkKind; AAC/TTS/mixing/encoding;
automatic thumbnails/crops/optimization; arbitrary URL ingestion; public image delivery;
production activation; new identity/permission framework; client redesign or native integration.

## 2. Pinned baseline and current coverage

Fresh fetch before inspection: origin/develop =
**b28b87856b30c78c2921aa1134ac657b0584ebd7**, the merged PR102 Studio Protobuf pilot.
The original main checkout at 389c5cb was clean, behind origin and left untouched. Planning uses
an isolated worktree at /workspace/scratch/429f9ca90843/cantos-catalog-media-plan-20261013,
branch plan/issue104-catalog-media-mvp, from that fresh base. All other worktrees/caches/evidence
are preserved. #104 was read in full through the connector; it had zero comments.

| Requirement / standard concept | Actual implementation at the pinned base | MVP disposition |
| --- | --- | --- |
| Work kind, title/synopsis/language, owner, visibility, rights refs | Catalog domain metadata/work; migration 0011; JSON Work routes | Preserve; no new enum, slug, external GUID or public-license field |
| Chapter/episode identity and order | Catalog unit/domain/reorder; 0011; chapter for novel/story, episode for podcast | Preserve; order is not podcast episode numbering or timed chapters |
| Immutable metadata/content revisions and operation receipts | Catalog storage/admission; 0011/0012; separate metadata/content sequences | Preserve exact bytes/digests; cover change advances metadata only |
| Plain text / recording / optional Script IR | Catalog content; 0012; Script/source refs are private and optional | Preserve; do not move canonical text/Script IR out of PostgreSQL |
| Frozen producer brief and separate producer grant | Production; 0013; Content/Brief pinning | Preserve; image admission does not need a production brief or grant |
| Actual audio facts and exact audio binding | Media audio_intake/lifecycle; 0010/0014; Opus mono profiles | Preserve; image does not use VerifiedAudio, duration or audio QC |
| Private object targets and owner byte reads | Pinned develop still has audio-only filesystem storage; PR90 is unmerged | Reuse PR90 after its separate review/integration gate; no second adapter |
| Distribution clearance, approval, immutable release and active pointer | Media publication; 0016; default UnavailableDelivery | Preserve; selected private image never enters current release manifest |
| Anonymous public discovery and immutable playback metadata | Media/Catalog listener facade; 0017; JSON; grant fails closed | Preserve DTOs/cursors/eligibility; no private cover projection |
| Listener Library/progress | Listening; 0019; separate sealed listener identity seam | Preserve; no cover-owned account state or listener auth shortcut |
| Protobuf transport | Four opt-in Studio revision/validation routes only; JSON default | Add only new artwork metadata roots; existing Catalog stays JSON-only |
| Image asset/variant, cover selection, alt/dimensions | No image domain/table/contract/route; mock assets only | Implement the one-cover private slice described here |
| Contributors, genres/tags, season/episode display | No production Catalog fields; genres exist only in mock UI | Defer all P1; no placeholder columns/messages |
| Timed chapters / transcript | No current production contract; nullable Script IR is unrelated | Defer; require a separate rendition/publication-bound asset later |
| Published/updated times and cache | Existing metadata times, release times, no-store metadata and private delivery policies | Preserve; image byte ETag derives from inspected SHA-256; no RSS TTL field |

Inspected authorities: AGENTS.md, README, current product/business rules, architecture/context
and wire docs, work queue/planning guide, Catalog/Content/Brief/Audio/Publication/Theatre
contracts and actual domain/application/storage/HTTP/SQL. Foundation, work-item, Script IR,
publication, listening and code-review skills were read. Some older skills/product overview
paragraphs still say features are absent: the concrete code/contracts above establish existence,
not full product acceptance. This task does not mass-rewrite their historical evidence.

### Dependencies and migration coordination

- PR90 [private storage](https://github.com/loveoverflowcom/cantos/pull/90) was open/unmerged and
  unmergeable at check, head **9306d22954b44aa9be390495aa0b2581c0738d22**. Its actual port,
  registry, object-ref types, filesystem/S3 methods and private-read contract were inspected via
  a read-only fetched branch. The write port takes VerifiedAudio; S3 PUT hard-codes audio/ogg.
- PR92/98/101/103 identity changes were separately under review, not in the pinned base.
  Their migration slots are 0018 / 0020 / 0021 / 0022. Do not cherry-pick or fix them here.
- The pinned registry contains 1–14,16,17,19 (17 entries). 0015 is Storage; 0016 Publication;
  0017 Discovery; 0018 Identity; 0019 Library; 0020 Studio OIDC; 0021 Theatre; 0022 Admin.
- **Coordinate/reserve 0023_private_work_artwork.sql** for this MVP. This is a proposed slot,
  not an applied file. Parent owns cross-session coordination. Before writing it, fetch latest
  develop, inspect the actual ordered registry/files and all concurrent reservations. If another
  authorized writer has already claimed 0023, use the next coordinated free slot and update
  this plan/contract/evidence; never rename a published/applied migration or edit old checksums.
- Implementation may complete image types, pure rules, JSON/Proto contracts, fixtures and
  deterministic fake use cases immediately. **Real storage integration waits for PR90's final
  reviewed boundary**. Prefer merged develop. Incorporating its exact verified branch into an
  isolated feature branch is allowed only when parent confirms it is needed and provides its
  final approved SHA; preserve dependency history and identify inherited versus own changes.
  Do not merge PR90 or unrelated PRs, or call the unreviewed storage branch accepted.

## 3. Ownership and construction barriers

Keep the existing modular monolith. No new crate/service, generic MediaAsset base class,
repository framework, image worker, event bus or provider adapter.

| Owner | Concrete responsibility / proposed files |
| --- | --- |
| Media domain | ImageIntent, ImageAssetId, immutable ImageAsset + one ImageVariant, ImageMime/Profile, WorkArtworkSelection; pure bounds/intake/replay/selection decisions under contexts/media/domain/image.rs and artwork.rs |
| Image byte boundary | Bounded PNG/JPEG decoder under contexts/media/image_intake.rs; only successful full decode/hash can construct private-field VerifiedImage |
| Media application | Owner-only prepare/admit/read/select use cases; transaction/effect ordering under image_application.rs/artwork_application.rs; no rule in handlers |
| Catalog | Work identity, authorization, lifecycle, revision/touch/history and catalog-clock authority; add one concrete owner-Work read/advance facade for Media, analogous to production_content, without requiring a unit/content head |
| Media PostgreSQL | Image intents/assets, effective artwork-change ledger and operation receipts; contexts/media/postgres/image.rs and artwork.rs; read through Catalog's facade, never private Catalog table shortcuts |
| Shared contract | Raw JSON DTOs, strict artwork_v1.proto, generated edge adapters/fixtures; packages/contracts/api and protobuf |
| HTTP | New Studio-only image/artwork handlers, existing auth/origin/CSRF and typed error envelopes; no SQL, identity minting, decoder rule or public promotion |
| Storage | Reuse final PR90 target registry, immutable private references and concrete adapters; adapt only its sealed write capability for actual VerifiedImage |

The Work cover is a Media-owned assignment to a Catalog-owned immutable Work revision. Keep
it in an explicit sidecar API/ledger instead of adding fields to WorkDetailsDto/Work snapshots.
This avoids breaking existing deny_unknown_fields clients or changing replay/history/digests,
and avoids a reverse Catalog → Media dependency. Media already consumes Catalog; retain that
direction. The Catalog facade atomically appends the touched Work snapshot and bumps its epoch
in the caller's transaction; only Media writes the artwork ledger. Return no raw SQL/Store
internals to HTTP. Update the architecture checker only if a justified concrete edge is new.

Private-field constructors reject invalid IDs, metadata, stored rows and restore documents.
An ImageRef is a checked identity/digest, not evidence of existence or ownership. The application
resolves the exact admitted image for the same Work/owner before selecting it. Composite SQL
FKs are a second barrier, not a substitute for authorization or constructors.

## 4. Field-by-field selection

Presence legend: R = required application value; N = nullable; A = absent from this slice;
D = server-derived. New metadata has JSON and opt-in Protobuf only on the new routes. SQL times
are nonnegative Unix milliseconds in bigint; revisions are 1..i64::MAX. Internal object locations
never appear in client documents. Existing domain limits and digest schemes remain unchanged.

### Existing fields that form the MVP foundation, with no new schema/transport work

| Field | Owner/type and presence | Existing SQL / transport | Validation, privacy and real consumer / rationale |
| --- | --- | --- | --- |
| work.id | Catalog WorkId, R | catalog_works + immutable snapshots; JSON string | Canonical resource UUID; never a title/URL/feed GUID. Studio identity and all downstream bindings |
| work.kind | Catalog WorkKind, R | catalog_works kind CHECK; JSON enum string | novel/story/podcast only. Existing CMS mapping; medium does not justify audiobook |
| work.owner_id | Catalog OwnerId, D/R | actors FK and immutable owner; JSON string | Authenticated owner only; not a contributor credit or public person identity |
| work.title | Catalog bounded String, R | Work snapshot; JSON | Nonblank, <=512 UTF-8 bytes, no controls. Existing Studio/public pinned Work title |
| work.synopsis | Catalog String, R, empty allowed | Work snapshot; JSON | <=16384 bytes, bounded multiline. Existing catalog description |
| work.language | Catalog language String, R | Work snapshot; JSON | Existing bounded common BCP-47 form <=63 bytes; no implicit translation/support claim |
| work.visibility | Catalog Visibility, R | Work snapshot; JSON | private/unlisted/public; owner-only change. Never a distribution/image grant |
| work.rights_refs | Catalog Vec<String>, R | Work snapshot; JSON | Existing <=64 distinct owner-bound evidence IDs; declarations are not public-license proof |
| work.lifecycle | Catalog Lifecycle, R | catalog_works + snapshot; JSON | draft/archived; archive retains history. Archived Work cannot acquire/change a cover |
| work.revision | Catalog Revision, D/R | catalog_work_revisions + head FK; JSON | CAS guard and immutable history. Artwork set/clear advances metadata, never content/audio |
| work.created_at | Catalog i64, D/R | Work snapshot; JSON | Existing authoritative ms, >=0. Not publication time |
| work.updated_at | Catalog i64, D/R | Work snapshot; JSON | Monotonic server ms; cover touch uses same policy; never freshness/rights proof |
| unit.id / work_id | Catalog ContentUnitId / WorkId, R | catalog_units + snapshots; JSON | Existing immutable unit/parent. Work-only artwork adds no unit reference |
| unit.kind / position | Catalog ContentUnitKind / u32, R/D | catalog_units checks/order; JSON | Existing kind mapping and stable occupied slots. Not season/episode number or timed marker |
| unit.source / script | Catalog exact SourceRef / ScriptRef, N | Original/source and Script IR remain PG; JSON | Same-owner exact digests/revisions; private, optional. No image/transcript alias |
| unit.content_digest | Catalog declared digest, N | Unit snapshot; JSON | Existing source/script checks, declaration when unlinked; not image byte proof |
| content revision/body/rights | Catalog immutable ContentRevision, R; optional script binding | 0012 content tables; JSON | Preserve separate metadata/content authority and every historical digest |
| audio rendition.id / binding | Media immutable AudioRendition, R | audio_renditions; JSON | Exact Content/Brief/unit scope; new cover never rebinds or rewrites audio |
| audio verified media | Media inspected MIME/codec/profile/size/SHA/duration, D/R | Immutable rendition snapshot; JSON | Existing real Opus inspection and profile policy; no AAC or image-as-audio workaround |
| release.id / manifest / published_at | Media Publication, D/R | 0016 immutable release/pointer; JSON | Existing exact approvals and actual publication ms. Private cover excluded |
| work.first_published_at_ms | Media public projection, D/R | Existing discovery facts; JSON | First actual publication stays stable across replacements; no new pubDate column |

### New image/selection fields, each justified by this workflow

| Field | Owner/type; presence | SQL entity / strategy | JSON / Proto presence | Validation / rights/privacy / consumer and rationale |
| --- | --- | --- | --- | --- |
| operation_id | Media command OperationId; R | artwork operations PK (owner,operation); intent also stores original operation | Required canonical UUID string; optional wire scalar must be present | Retry/reconcile identity. Fingerprint includes command kind, path Work and exact payload; no new authority |
| image intent.id | Media ImageIntentId; R, caller-minted | media_image_intents PK, immutable | Required string in request/receipt | Canonical UUID; enables lookup after lost prepare acknowledgement; duplicates cannot move between Works |
| intent.schema_version | Media reader/write version; D/R | Immutable intent document | Required string = image-intent-v1 | Closed restore version, no historical document rewriting |
| intent.work_id | Catalog WorkId; D/R | Work FK and immutable intent/document | Required receipt string, from path not body | Resolved authorized Work; no alternate caller parent or unit required |
| intent.owner_id | Catalog owner fact; D/R | Composite Work/owner FK | Required receipt string, never writable | Current Work owner; image scope does not follow a Studio member or producer token |
| intent.profile_id | Media ImageProfile; D/R | Immutable intent snapshot | Required receipt string = cover-image-v1 | Freeze PNG/JPEG/bounds policy at prepare; not caller-extensible |
| expected_sha256 | Media expected digest; R declaration | Intent column + immutable snapshot | Required lowercase 64-hex string | Actual upload/storage hash must match; declaration alone cannot construct VerifiedImage |
| expected_byte_length | Media bounded u64; R declaration | Intent bigint CHECK 1..8388608 | Required JSON integer / present Proto uint64 | <=8 MiB; reject zero/overflow/negative. Actual encoded byte count must match |
| content_type | Media ImageMime; R declaration | Intent CHECK image/png or image/jpeg | Required JSON string; present Proto enum, reject zero/unknown | Narrow requested format must match inspected bytes. No filename, arbitrary MIME or format guessing from path |
| declarations.provenance | Media bounded String; R caller-declared | Intent/asset immutable document | Required string, no null/default | Nonblank <=4096 UTF-8 bytes, bounded multiline; private source/tool/creator explanation, never a verified credit |
| declarations.rights_refs | Media Vec<evidence ID>; R declaration | Intent/asset immutable document | Required JSON list; repeated wire absence normalizes to empty, then validation rejects empty | 1..64 distinct existing owner-bound rights refs, existing token grammar. Record claims only; no distribution clearance or verified legal conclusion |
| intent.created_at_ms | Media i64; D/R | Immutable intent bigint/document | Required JSON integer / present Proto uint64 bounded to i64 | Authoritative transaction clock; no caller timestamp |
| intent.expires_at_ms | Media i64; D/R | Immutable intent bigint/document | Required JSON integer / present Proto uint64 bounded to i64 | Server policy: created +30 minutes, checked arithmetic. Pending admission ends at expiry; no caller expiry/renewal field |
| intent.digest | Media domain document digest; D/R | Stored ii-r1:sha256 digest | Required string | Canonical receipt integrity; excludes no declared business field; preserves exact replay |
| intent state | Media derived pending/admitted/expired; D | Not persisted as a mutable status | Required status response enum; reject zero/unknown wire value | Derived from accepted asset and server time. Accepted asset remains historical owner-readable after pending intent expiry |
| asset.id | Media ImageAssetId; D/R | media_image_assets PK, one accepted row per intent | Required string | Accepted ID uses intent's known UUID in a distinct type; no extra redundant version counter |
| asset.intent_id | Media ImageIntentId; R | Unique FK to exact intent/work/owner | Required string | Exact request provenance; same intent cannot finalize two different assets |
| asset.schema_version | Media reader/write version; D/R | Immutable asset document | Required string = image-asset-v1 | Closed reader version; unsupported version rejected, no history rewriting |
| asset.work_id / owner_id | Catalog scope facts; D/R | Composite intent/Work/owner FKs | Required receipt strings | Same authorized Work as prepare; asset cannot be selected by another Work, even under the same owner |
| variant.profile_id | Media ImageProfile; D/R | Immutable asset document | Required string = cover-image-v1 | Exactly one externally prepared cover variant in this MVP; no generic preset registry |
| variant.sha256 | Media inspected encoded digest; D/R | Asset column/document; internal object key equality | Required lowercase hex string | SHA-256 of every admitted encoded byte, checked after storage readback; immutable cache/object identity |
| variant.byte_length | Media inspected u64; D/R | Asset bigint CHECK 1..8388608 | Required integer / present uint64 | Real byte length, not expected length echoed back; separate from decoded allocation |
| variant.content_type | Media inspected ImageMime; D/R | Asset MIME CHECK/document | Required JSON string / present closed wire enum | PNG/JPEG only, actual full decoder result; bytes route emits this type with nosniff |
| variant.width_px | Media inspected u32; D/R | Asset integer CHECK 1..4096 | Required integer / present uint32 | Decoder-derived width; caller width not accepted. Layout/reference semantics |
| variant.height_px | Media inspected u32; D/R | Asset integer CHECK 1..4096 | Required integer / present uint32 | Decoder-derived height; caller height not accepted. Preserve rectangular covers |
| variant aspect ratio | Media derived integer ratio; D/A transport | No column | No new DTO field; derive width/height using checked integer arithmetic | Avoid float/redundant sources of truth. Not an assumed square or automatic crop |
| variant purpose | Media policy constant cover; D/A transport | No purpose table/column | No caller field | One Work cover consumer; no arbitrary namespace purpose tokens, banners/social/motion |
| asset.admitted_at_ms | Media i64; D/R | Immutable asset bigint/document | Required integer / present Proto uint64 bounded to i64 | After full decode/persist/readback and reauthorization; not publication approval/time |
| asset.digest | Media domain digest; D/R | ia-r1:sha256 column/document | Required string | Pins asset facts plus declarations; no signed URL, provider hostname or mutable default target in document |
| storage_target | Media StorageTargetId; internal R | Asset column, immutable recorded PR90 target | A from JSON/Proto | Final PR90 grammar; old asset reads resolve recorded target, never active write default |
| object_key | Media ObjectKey; internal D/R | Asset column = actual SHA-256 | A from JSON/Proto | Server-derived content key, no caller path/URL; private target only |
| work artwork | Media Option<WorkArtworkSelection>; N | Append-only media_work_artwork_changes at a Catalog Work revision | Response artwork null / absent message; request required null-or-value / oneof clear-or-set | None is a valid legacy Work. A clear is an explicit tombstone, never delete bytes/history |
| selection.image_asset_id | Media ImageAssetId; R when set | Composite FK to same Work/owner asset | Required nested string | Application resolves exact admitted asset before set; pending/cross-Work images fail closed |
| selection.image_asset_digest | Media asset digest; R when set | Composite digest FK + immutable selection snapshot | Required nested string | Must match accepted ia-r1 document; no latest-image or caller boolean |
| selection.alt_text | Media context-specific bounded String; R when set | Selection snapshot + <=1024-byte text column | Required nested string, no null/default | Nonblank single-line UTF-8, no controls; owner-authored description. No inferred visual truth or automatic caption |
| expected_work_revision | Catalog Revision; R mutation guard | Existing Work head locked/CAS/touched | Required integer / present uint64; 1..i64::MAX | Concurrent replace/clear must not overwrite; rejected request changes no metadata/artwork row |
| response.work_revision | Catalog Revision; D/R | Exact requested/current catalog_work_revisions row | Required integer / present uint64 | Pin current/effective or historical selection alongside existing Work APIs |
| response.changed_at_ms | Media change time; N/D | Latest artwork-change row at or before requested Work revision | Nullable JSON / optional Proto scalar | Absent for never-selected legacy Work; set for both selection and clear. No invented historical timestamp |
| raw image bytes | Media byte boundary; R only in binary PUT/GET | Private object store, never PostgreSQL | Raw PNG/JPEG, never JSON/base64/Proto wrapper | Bounded exact private preview; metadata negotiation and storage transfer are separate |

No ImageVariant list, variant ID, source-image graph, thumbnail size, focal point, crop box,
quality score, rights-approved boolean, public URL, artwork license or per-unit override is added.
Replacing encoded bytes creates a new intent/asset; replacing alt creates a new selection.

## 5. Additive database contract

One coordinated migration, provisionally 0023. No rewriting 0001–0022, historical snapshots,
content/export/brief/rendition/release digests, receipts or object identities. Register the new
version in services/cantos-api/src/platform/postgres/migrations.rs in sorted order, preserving
gaps and all final dependency entries. Reconcile counts/upgrade fixtures and context-scoped
test-role grants in the existing harness; never grant DDL or public access.

| Entity | Exact minimum durable contract |
| --- | --- |
| catalog_works owner pair | Add UNIQUE(id,owner_id), without changing existing identity/lifecycle semantics; supports cross-context FK checks without application table shortcuts |
| media_image_intents | id PK; work_id/owner_id composite FK to Catalog; operation_id; expected_sha256; expected_byte_length; content_type; profile_id; created_at_ms/expires_at_ms; immutable document bytes and ii-r1 digest; UNIQUE(id,work_id,owner_id), UNIQUE(owner_id,operation_id) |
| media_image_assets | id PK; intent_id UNIQUE; work_id/owner_id; composite FK to exact intent; storage_target/object_key; inspected sha256/byte_length/MIME/width/height/profile; admitted_at_ms; immutable document bytes and ia-r1 digest; UNIQUE(id,work_id,owner_id,digest) for exact selection binding |
| media_work_artwork_changes | work_id/work_metadata_revision PK, FK to catalog_work_revisions; owner_id composite Work FK; nullable image_asset_id/image_asset_digest/alt_text as an all-null clear or all-present set; composite FK to same Work/owner/digest asset; recorded_by = owner_id; recorded_at_ms; immutable change document/digest |
| media_artwork_operations | owner_id/operation_id PK; work_id; closed kind prepare_image or set_artwork; exact request fingerprint; original response bytes plus checksum; immutable, bounded receipt. Cross-kind or changed payload reuse conflicts |

Require asset.id = asset.intent_id; the accepted UUID is already known after prepare. All new
immutable tables reject UPDATE, DELETE and TRUNCATE using the existing settled-record
guard. Revoke PUBLIC. New application grants are only required SELECT/INSERT and the existing
Catalog-owned revision/touch access; no storage credentials, new OAuth grants or account policy.

CHECKs cover canonical UUIDs, digest schemes/hex, target grammar, supported MIME/profile,
encoded length 1..8 MiB, width/height 1..4096, bigint-safe pixel product <=16777216, monotonic
intent times, nonnegative acceptance/change time and positive revision. Metadata document/
operation-response bytes are 1..32768. Decode every stored document through constructors and
cross-check duplicated query columns against the checked document. SHA/checksum does not waive
admission. Use no unbounded JSONB metadata bucket or mutable is_verified/is_published flag.

Artwork is an append-only change log, not a new mutable Work head. For Work revision R, resolve
the newest artwork change with work_metadata_revision <= R. No row means no cover. A clear row
is a tombstone, so an earlier selection cannot reappear. The PK's descending scan suffices; do
not copy a selection into every unrelated Work revision. Read the requested Work snapshot through
Catalog's owner facade first; non-existent/future revisions are NotFound, not latest substitution.

Set/clear and Catalog Work.touch + snapshot/head/epoch advancement + operation receipt commit
in **one transaction** under the existing Catalog clock/Work/actor lock order. No second artwork
revision counter or distributed transaction. Exact retry returns its original response after
current authorization, even when its expected Work revision is now old. New stale commands are
conflicts; no row changes. A set/clear with the same desired value but a new operation is still
a new metadata revision, matching the existing explicit-edit semantics.

Image prepare/admission does not advance the Work metadata head: it creates unselected private
assets. Only selection/clear advances it. Metadata changes therefore retain the existing
MetadataChanged blocker for old audio candidates; no special bypass. Existing immutable releases
and retained-release behavior stay intact. Cover upload/selection never changes ContentRevision,
Script IR, Brief, audio bytes or old manifests.

Upgrade oracle: seed actual pinned-base schema, historical bytes/digests/receipts/active pointers
and gap versions; apply final merged dependency migrations and 0023; compare exact old values,
checksums and ID/FK ownership; migrate a second time; run old Catalog/Publication JSON fixtures
unchanged. No cover backfill or guessed timestamp for legacy Works.

Read inventory: intent/asset exact PK lookups, bounded effective-artwork lookup and existing
Catalog authorization/head facts. No new discovery joins or list endpoint. Measure these actual
queries with the application role and synthetic multi-owner data in disposable PostgreSQL;
retain request timings and EXPLAIN (ANALYZE, BUFFERS). Reject DBSP for this MVP's small indexed
point/range lookup workload; reconsider only with an observed later hot multi-consumer query.
No performance win is claimed by the plan.

## 6. Concrete routes and use cases

New paths are mounted only under /api/v1/studio, outside Catalog's existing JSON-only/body-limit
middleware. Use current Studio auth/origin/CSRF composition, including final merged identity
behavior. No deprecated aliases, producer bearer route, Theatre/Admin handler, new login or
permission-sharing flow. Owner only for every new path; editors/readers/other owners get 404,
missing/expired auth gets 401. Active owner facts and time are checked after waits and before
mutation/stream response, not just once in a handler.

| Method / path after Studio prefix | Application command/query and concrete result |
| --- | --- |
| POST /works/{work}/image-intents | PrepareImage: caller UUID + operation_id + expected hash/length/MIME + declarations; persist a private immutable owner intent; 201 ImageIntentDocument. No bytes or Work-head change |
| GET /works/{work}/image-intents/{id} | ReadImageIntent: exact receipt, derived state and optional accepted ImageAssetDocument; pending/expired are visible only to owner. Reconcile uncertain prepare/finalize outcomes here |
| PUT /works/{work}/image-intents/{id}/bytes | AdmitImage: raw PNG/JPEG, exact accepted header/length/hash and full bounded decoder; persist/readback through PR90 then reauthorize/commit one immutable asset; 200 ImageAssetDocument. No public promotion |
| GET /works/{work}/image-assets/{id} | ReadImageAsset: exact admitted immutable document, no storage URL/key/target. The receipt is acceptance-time evidence, not a live storage-health promise |
| GET / HEAD /works/{work}/image-assets/{id}/bytes | ReadPrivateImage: exact accepted bytes/headers through PR90 open_verified(range=None), authorized before I/O and again before response. 200 inspected MIME/length, strong SHA-based ETag, no-store, nosniff; HEAD no body |
| GET /works/{work}/artwork?work_revision=R | ReadWorkArtwork: optional R; omitted means current authorized Work head. Return work_id, exact work_revision, artwork null-or-value, nullable changed_at_ms. Exact historical read never follows latest |
| PATCH /works/{work}/artwork | SetWorkArtwork: operation_id, expected_work_revision, required artwork value or explicit null; resolve/verify the exact asset, then commit selection or clear with Work touch. 200 original immutable WorkArtworkResponse |

### Admission and effect order

1. Settle Origin/session, response Accept and request Content-Type/body limits before storage
   effects. Metadata bodies <=32 KiB; binary body <=8 MiB; binary PUT Content-Type must match
   the intent's PNG/JPEG type. No multipart/base64, remote-fetch, caller path, signed upload,
   direct public staging or dynamically selected provider.
2. Short initial transaction: authenticate, resolve owner Work through its facade, require draft
   for a new admission, load exact intent and current time. Capture the immutable expected facts,
   close transaction. Read rights evidence only through existing owner-bound Editorial access.
3. Outside SQL: hash every encoded byte, match expected hash/count/type, decode the entire image
   under cover-image-v1 policy, persist through the selected private target and verify actual
   stored bytes. Record the exact target returned by the registry. PUT/HEAD success is insufficient.
4. Short commit transaction: same Catalog fence/actor/Work lock order; reread/revalidate owner,
   activity, Work lifecycle, exact intent, expiry and accepted row; decide in the pure core;
   insert one asset or return its exact checked existing receipt; reauthenticate/time-check before
   commit. Network/storage never runs under metadata locks. Failure may leave a private orphan.
5. Lost response: read the known intent; replay the same PUT with the same bytes if needed.
   Same accepted bytes return the original asset after current ownership and object verification;
   different bytes/metadata cannot replace it. Accepted history/preview remains accessible to the
   current active owner after the pending intent expires. New admission at now == expires fails.
6. Set: authenticate/read exact asset and verify storage outside the final transaction; lock and
   recheck same asset/document/Work/owner, expected revision and clock inside the final transaction;
   append selection plus Work touch and receipt atomically. Clear needs no object I/O. Archive
   blocks new writes; owner historical reads remain allowed. No duplicate request is relabeled
   with a new actor, Work, target or operation ID.

### Image inspection and the smallest storage adaptation

Select image = 0.25.8, default-features = false, features png/jpeg, only after the implementation
records the pinned dependency decision, resolves its lock under the production toolchain pin and checks current
security advisories. There is no dependency/toolchain installation in this planning session.
Use actual PNG/JPEG decoders, not a hand-written magic/header-only acceptance pipeline.

cover-image-v1: encoded 1..8 MiB; width/height 1..4096; pixel product <=16777216 using checked/
widened integer arithmetic; decoded output <=64 MiB using the decoder's actual checked sample
layout/bytes-per-pixel, not an assumed four-byte RGBA buffer. A 16-bit PNG may therefore fail
the output bound despite valid dimensions; reject it without implicit conversion. PNG constructor receives limits before
metadata parsing; reject APNG. JPEG receives only the bounded input, then dimensions/output
limits before decode; reject non-identity EXIF orientation rather than rotate/re-encode it.
Reject unsupported formats including SVG/HTML/GIF/WebP/video in this first profile. Preserve
uploaded encoded bytes and metadata privately; never claim they were sanitized. Supported image
decodability and exact bytes are the claim, not complete standards conformance or visual quality.

Use two bounded admission/decode lanes: acquire the owned permit after cheap authentication/
intent checks but before buffering a binary body, so concurrent uploads cannot accumulate an
unbounded number of 8 MiB buffers. Reject busy admission with the existing bounded wait/deadline
policy; reauthorize and check expiry after the wait. Retain the permit until the buffered body
and blocking work are actually released, including caller timeout/cancellation. Keep PR90's
existing storage concurrency/read bounds.
No detached unbounded tasks, limit-disabled constructors or allocation inferred from hostile
declared length. Codec-specific allocations and malformed/truncated/animated/header-bomb fixtures
must be exercised; a library output-buffer limit is not a hard process RSS/CPU bound. If the
chosen decoder cannot enforce a required bound, fail closed/report it instead of trusting MIME.

After PR90 integration, adapt PrivateStorage.put_if_absent and StorageRegistry.persist with a
**sealed verified-private-object payload** constructible only from actual VerifiedAudio or
VerifiedImage. It carries immutable bounded bytes, inspected MIME and integrity. No public
from_bytes constructor, generic unverified blob upload or conversion of images into VerifiedAudio.
Retain existing audio constructors/profile limits and receipt bytes. The S3 PUT's MIME comes from
this sealed value, not the HTTP header. Reuse the same conditional-create/readback/version-pin
adapters and topology; no second S3/signing implementation, provider SDK or target policy change.
The generic maximum remains PR90's bound, with the stricter image policy checked separately.

Private image GET streams exact bytes through the registry without signed read-access grants
in this MVP. Ignore Range and return full 200; do not advertise image range support. Audio Range
and grants are unchanged. No 304/immutable-public-cache feature is added; authorize every read.
Absent object is 404, unavailable/busy target 503, stored integrity corruption 500; safe JSON
errors only. Failure after headers aborts the stream and cannot be relabeled success. Historical
target removal fails closed rather than falling back to the current target.

### Exact error behavior

Use existing ApiError where applicable, with closed new image/artwork domain reasons translated
once at HTTP. 400 malformed body/query/header/Proto shape; 413 body too large; 415 unsupported
request representation; 406 unacceptable response; 422 invalid fields/expected media mismatch/
unsupported or undecodable image; 409 reused operation/duplicate ID/stale Work revision/expired
pending intent/archived write; 401 unauthenticated; 404 unauthorized or wrong Work/asset; 503
dependency unavailable; 500 corrupt stored receipt/object. Reuse existing operation_reused,
stale_revision/current_revision and field-issue conventions. Never leak SQL, paths, credentials,
provenance text, signed URLs or foreign-resource existence in errors/logs.

## 7. JSON and Protobuf contract scope/presence

New artwork metadata is schema-first and supports JSON default plus opt-in application/protobuf.
No existing Catalog/Theatre/Publication/auth endpoint is migrated, and no new MessagePack consumer
is added. Raw image bytes remain their actual media type. JSON errors stay unchanged, including
when a request/success is Protobuf. Use Vary: Accept, no-store and existing media-type parameter
rules. No ProtoJSON or binary error envelope. Extend allowed-format composition narrowly rather
than copy the existing negotiation algorithm or accidentally enable legacy MessagePack here.

Proposed authority: packages/contracts/protobuf/artwork_v1.proto, proto2 package
cantos.studio.artwork.v1. Checked-in private prost edge types plus exhaustive fallible adapters;
public raw DTOs remain the JSON/application surface. Generated-message presence is not authority.

| Message | Fixed field numbers for this v1 implementation |
| --- | --- |
| ImageDeclarations | 1 provenance string; 2 rights_refs repeated string; reserve 3..15 |
| CreateImageIntentRequest | 1 operation_id string; 2 id string; 3 expected_sha256 string; 4 expected_byte_length uint64; 5 content_type ImageMime; 6 declarations message; reserve 7..15 |
| ImageIntentDocument | 1 schema_version string; 2 id string; 3 work_id string; 4 owner_id string; 5 operation_id string; 6 request CreateImageIntentRequest; 7 created_at_ms uint64; 8 expires_at_ms uint64; 9 digest string; 10 profile_id string; reserve 11..15 |
| ImageVariantMetadata | 1 sha256 string; 2 byte_length uint64; 3 content_type ImageMime; 4 width_px uint32; 5 height_px uint32; 6 profile_id string; reserve 7..15 |
| ImageAssetDocument | 1 schema_version string; 2 id string; 3 intent_id string; 4 work_id string; 5 owner_id string; 6 verified_media ImageVariantMetadata; 7 declarations ImageDeclarations; 8 admitted_at_ms uint64; 9 digest string; reserve 10..15 |
| ImageIntentResponse | 1 intent ImageIntentDocument; 2 asset optional ImageAssetDocument; 3 state ImageIntentState; reserve 4..15 |
| WorkArtworkSelection | 1 image_asset_id string; 2 image_asset_digest string; 3 alt_text string; reserve 4..15 |
| ClearArtwork | Empty message; all future field numbers require new schema review |
| SetWorkArtworkRequest | 1 operation_id string; 2 expected_work_revision uint64; oneof change: 3 artwork WorkArtworkSelection OR 4 clear_artwork ClearArtwork; reserve 5..15 |
| WorkArtworkResponse | 1 work_id string; 2 work_revision uint64; 3 artwork optional WorkArtworkSelection; 4 changed_at_ms optional uint64; reserve 5..15 |

ImageMime: 0 unspecified (rejected), 1 png, 2 jpeg. ImageIntentState: 0 unspecified (rejected),
1 pending, 2 admitted, 3 expired. Each scalar is optional on the wire but application-required
unless explicitly nullable above; unknown enum values and missing required nested messages fail.
SetWorkArtwork JSON requires artwork: null (clear) or a complete selection object (set); omission
is invalid/empty intent, not a clear. Proto requires exactly one oneof arm; reject both even when
prost would otherwise keep the last. An empty present ClearArtwork is explicit clear; an absent
arm is invalid. Response artwork/changed_at absence maps to JSON null, with no manufactured default.

Strict metadata prescan: <=32 KiB envelope, depth <=4, total fields <=256, rights entries <=64
at each declaration, checked lengths/cursors, canonical varints, exact declared wire types,
valid UTF-8, unknown/reserved fields and duplicate singulars rejected. Bound u32 and enum values
before narrowing; application validates i64-backed timestamps/revisions. Never add unbounded
map/arbitrary-JSON/float fields. Reuse bounded scanner primitives while preserving old pilot's
different limits and fixtures. Its current scalar seen mask is u16; keep new fields <=15 or
deliberately expand and test the mask before any future higher number. No silent shift overflow.

Extend the existing independent Python wire oracle and isolated prost/protoc generation check
for the new schema; retain exact old generated output/goldens and warm-schema-change detection.
Add presence, null/absent/set/clear, enum-zero/unknown, duplicate-oneof, maximal integer,
Unicode, size/depth/field-budget and both-direction JSON/Proto replay fixtures. Kotlin fixture
compatibility should consume the new independent corpus via the existing fixture-only harness,
without adding a native route, UI serializer or authority. Its current lack of enum/oneof cases
is a concrete new test obligation, not permission to invent a mobile client.

Canonical intent/asset/change/operation digests use reviewed JSON domain documents with named
ii-r1 / ia-r1 / wa-r1 / image-operation1 schemes. The same semantic request produces the same
fingerprint in JSON and Proto; fingerprint the validated explicit intent, not transport bytes or
applied/latest state. For new writes only, canonicalize distinct rights_refs in ascending UTF-8
token order before constructing receipts/fingerprints; array order is not a rights priority.
Capture cross-transport/reordered-set tests. Do not normalize arbitrary Unicode provenance/alt
or change old Catalog arrays. Persist original response bytes, never reserialize old
Catalog or audio responses through these new DTOs.

Digest preimages are fixed ordered structs, not maps or recursive digest-including documents:
intent = schema_version/id/work_id/owner_id/operation_id/request/profile_id/created_at_ms/
expires_at_ms; asset = schema_version/id/intent_id/work_id/owner_id/verified_media/declarations/
admitted_at_ms; artwork change = work_id/work_metadata_revision/owner_id/artwork/recorded_by/
recorded_at_ms. Their respective digest field is excluded from its own preimage; internal storage
target/key is also excluded from client asset documents, with separate checked query columns.
Operation preimage = kind/path Work/validated complete request, including explicit clear versus
set. Use the existing deterministic serde document convention and scheme-prefixed SHA-256, and
review independent literal goldens before adapters/SQL. Receipt-row checksums cover exact stored
response bytes. These are new schemes only, never a change to historical Catalog/audio hashing.

## 8. Compatibility, history and rollback

- Existing WorkDetailsDto, CatalogWork, Catalog mutation/history responses and persisted Work
  snapshots remain exactly their current shape. No optional field is smuggled into strict old
  clients. The new endpoint makes artwork an explicit capability.
- Legacy Works read as artwork null and changed_at_ms null. After selection, a later unrelated
  Catalog edit retains that effective selection. Clear cannot resurrect it. GET at an earlier
  Work revision reproduces the prior selection, subject to current owner access.
- Existing content/audio/release rows and byte/digest identities are unchanged. The old candidate
  metadata guard may require a new candidate after cover selection; do not reuse stale approval.
  Previously active immutable audio stays under existing retained-release/rights policy.
- Roll back by disabling/removing only the new route composition or running the prior application;
  keep additive tables, dependency migration entries, private objects and historical storage
  targets. The old application ignores the sidecar and still reads unchanged Catalog snapshots.
  Do not drop 0023 or delete orphan/old image bytes. Re-enable and read the same known receipts.
- No Theatre field, image URL, new release-manifest field, public bucket ACL or public staging
  key is added. Caller provenance/rights and all private artwork remain excluded from listener
  projections. New image publication later needs its own rights/approval/snapshot/delivery review.
- No automatic retry on ambiguous acknowledgement with a new operation/codec. Reconcile the
  known receipt/intent first. Timeouts can leave private orphans; deletion/retention is separate.

## 9. Verification, in required L1 → L5 order

The rows below are implementation obligations, **not results**. Core work need not wait for
PR90, identity integration or a PostgreSQL server. Passed core evidence never substitutes for
the later real-storage/HTTP/SQL gate. Keep a claim ledger with exact command, source SHA, layer,
status, evidence level, actual failure and residual acceptance. Full logs belong in ignored
artifacts/agent-runs/, with safe summaries in docs/evidence/private-work-artwork.md.

| Layer | Concrete obligations / independent oracle | Completion evidence |
| --- | --- | --- |
| L1 architecture | Review Media → Catalog facade, decoder/storage at outer edge, no reverse Catalog import, no public projection, no new provider/crate/framework; JSON/Proto adapters do not own business rules | Documented owner/import review plus repository/architecture checker; no compile-pass claim before execution |
| L2 domain/types/errors | Private ImageAsset/VerifiedImage fields; exact fallible restore; distinct image/audio/intent IDs; MIME/profile enums; explicit clear/set; exact stale/expiry/rights/corruption outcomes; rejected transitions leave state unchanged | Type-enforced construction and exhaustive mappings; compile-fail/doctest showing caller metadata cannot forge VerifiedImage or pass it as VerifiedAudio |
| L3 examples/property | Run pure tests before wiring DB/HTTP. Boundary matrices for encoded length 0/1/max/max+1, dimensions 0/1/4096/4097/u32::MAX, checked pixel/output size, i64 clock/expiry/revision limits, canonical UUID/hash/UTF-8 bounds. A naive independent artwork-history model checks set/edit/clear histories and no resurrection. Fixed-seed property traces check CAS, same-operation replay, cross-kind/payload conflict and unchanged rejections | Exact structured assertions; nonzero discovered test counts; independent expected documents/digests rather than calling the encoder to calculate expected results |
| L3 byte/wire boundary | Independently produced tiny PNG/JPEG fixtures with named expected dimensions/hash; truncated/header-only/format-spoof/polyglot/ancillary-overload/huge-dimension/APNG/nonidentity-orientation cases; malformed bytes never panic. JSON/Proto enum, absent/null/clear, duplicate/unknown/reserved fields, varint and nested budgets. Independent Python/prost/Kotlin fixture corpus and warm schema-change regeneration | Example-/property-/differential-tested only for executed cases. Check full supported decode and admission limits; record permissive decoder/trailing-byte behavior instead of claiming full format conformance |
| L4 application | Exercise actual Media application and Catalog facade orchestration with named FakePrivateStorage, fixed Clock and controlled inspector/identity evidence. Record read/auth/inspect/persist/readback/reauth/commit ordering. Reject archive/foreign owner/member/expired intent/stale revision/missing or foreign rights evidence/revoked session; no object I/O while a SQL transaction is held. Inject ambiguous write receipt, target unavailable, readback mismatch, expired intent after slow storage, lost response and accepted retry | Mock-integration-tested with named doubles. No fake test establishes PostgreSQL constraints, S3 behavior, network auth or decoder resource safety |
| L5 real SQL | Disposable PostgreSQL 17, final migrations plus 0023, app-role constraints/permissions, exact old snapshots/digests unchanged, repeated migration idempotence, same-owner/work composite FKs, immutable triggers and column/document mismatch rejection. Two real connections race prepare IDs, duplicate PUT admission and artwork CAS against Catalog edit/clear; exactly one accepted state/receipt, no half touch or orphan ledger row | Integration-tested only after actual PostgreSQL run; changed table/index/migration registry assertions are not proof by inspection |
| L5 real HTTP/storage | Actual loopback host + PostgreSQL + private filesystem; then the final PR90 S3 adapter against its controlled local protocol peer, with no paid/live provider. Drive complete prepare → PUT → inspect metadata/exact bytes → select → historical read → replace → clear and replay across JSON/Proto. Test cookies/CSRF/origin, foreign/member/session expiry/revocation, unsupported Accept/MIME, oversized/chunked body, private HEAD/GET/no-store/nosniff and no Theatre leakage | Real HTTP/private-storage integration, separately from Router tests. Preserve audio/private Range/version/read-grant regressions. A local S3 peer is adapter protocol evidence, not live-cloud acceptance |
| L5 restart/fault/security | Crash after object write before asset commit; restart host with same durable storage and retry known intent. Crash after selection commit before response; reconcile same operation and original response after later edits. Restart disposable PG and host, change active write target while retaining old target, verify pinned old object version, fail closed on missing target/object or changed bytes. Catalog epoch and publication MetadataChanged guard stay atomic; no new jobs/provider calls/public paths | Named fault points, observable completion barriers and real restart logs; no sleeps-as-correctness oracle, no zero-test/zero-assertion PASS |

Add focused parameterized resource tests before HTTP activation: two active decode permits,
third admission bounded by the existing wait/deadline policy, cancellation retains a permit
until the blocking decoder exits, and object-stream permits are released on drop. Measure
hostile-fixture peak memory/elapsed time in a disposable process where feasible. A library
allocation limit or timeout is not a killable hard process-memory/CPU bound; record the actual
residual, and do not activate a route that demonstrably escapes the chosen admission budget.
No general benchmark/fuzzer infrastructure is required. A small deterministic malformed corpus
and an existing available fuzz primitive are adequate initial escalation; report unrun fuzzing.

### Worthwhile bounded proof, scoped to the actual production kernel

Reuse the existing Catalog Revision factory/successor and its ordinary edge tests. The only new
Kani candidate is the small pure image-size admission kernel: full symbolic u64 encoded length,
u32 width and height and raw u8 bytes-per-pixel; checked supported layouts use 1/2/3/4/6/8 bytes.
It accepts exactly 1..8388608 bytes, dimensions 1..4096, an admitted layout and a safe widened
pixel/output-byte product within the cover-image-v1 limits. Compare against an independent u128
specification; accepted output must equal the exact size and fit the target usize. Reject zero,
oversized dimensions/length, unsupported layout and exhausted output without panic. Generate invalid raw inputs;
do not assume valid dimensions or the answer. Include acceptance, each rejection boundary,
exact maxima and full machine-width values as cover goals. No decoder, allocation, JSON,
PostgreSQL, authorization, async storage or concurrency claim is in this proposition.

First implement and run table/property tests. If the real kernel remains the proposed small,
loop-free high-impact numeric boundary, register image-bounds in the existing typed xtask/Kani
registry and runner selection tests; call the actual production symbol, not a proof-only copy.
Record source/configuration/lock identities, target width, unwind bound (2 if still loop-free),
every assumption/stub (expected none), every cover result and safety/unwinding checks. Keep the
existing Kani 0.68.0/compiler nightly-2026-08-21/CBMC 6.11.0 pins and the production rust-toolchain.toml pin.
Never disable overflow/unwinding checks or install/upgrade a verifier to make planning succeed.
If the kernel changes or tooling is unavailable, record why property tests are the adequate
core evidence and the proof is NOT_RUN/UNSUPPORTED; no blanket bounded-model-checked claim.

The pinned actual xtask registry contains six selectors: catalog-revision, wav-payload,
submission-window, publication-commit, listener-access and listener-reconcile. Some older
README/reference paragraphs list three; use actual --help/registry, not those stale counts.
image-bounds is **proposed and not currently a runnable selector**.

### Commands and evidence discipline

Use the existing single xtask surface. After choosing implementation files, run focused
production-library tests with the discovered exact test selectors, then:

- cargo xtask verify repository
- cargo xtask verify rust
- cargo xtask verify contracts
- cargo xtask verify postgres
- cargo xtask verify kani --harness catalog-revision (existing; opt-in, if capability is present)
- cargo xtask verify kani --harness image-bounds (only after the new selector is implemented)
- cargo xtask preflight --changed-only

Do not pretend these are executed by listing them. Capture every gate as printed: PASS, FAIL,
NOT_RUN or UNSUPPORTED. A Cargo.toml/lock, shared contract or schema change can broaden preflight
to Web/CMP/other gates; inspect its dry-run and preserve that selection. Do not weaken the
selector or claim native/browser acceptance from corpus/build evidence. No client UI is touched
in this slice, so a new visual/interaction run is not implied. Existing audio listening/QC,
live identity issuer, cloud delivery and Android/iOS runtime remain separately scoped acceptance.

Planning-branch checks are docs-only: repository, changed-only preflight and diff hygiene.
All new-feature unit/property/Kani/application/SQL/HTTP/storage/restart tests are **NOT_RUN** in
this planning session because there is no implementation. The final planning handoff records
actual docs checks separately; no inherited evidence is promoted to new-feature evidence.

## 10. Logical commits, acceptance and residual decisions

One feature branch and one narrow outcome. Commit boundaries are review aids, not a requirement
to manufacture empty commits or carry a red intermediate state.

1. **Contract and pure admission/selection core:** reviewed artwork contract, JSON/Proto presence,
   schema/adapters/independent fixtures, checked IDs/bounds/receipt/history/CAS decisions and
   passing core tests. Register the small worthwhile proof only after its ordinary tests. This
   commit can proceed while PR90 is being reviewed; no runtime writes are exposed yet.
2. **Verified image and shared private storage:** bounded actual PNG/JPEG inspector, sealed
   VerifiedImage → final PR90 private-object write capability, MIME-correct filesystem/S3 reuse,
   readback and audio regressions. Rebase onto final storage/identity integration first. Keep
   inherited dependency changes separate; no second provider adapter or generic media framework.
3. **Additive persistence and use cases:** coordinated 0023, Catalog Work facade/atomic touch,
   immutable intent/asset/change/operation reads and writes, fake effect-order/retry tests followed
   by real SQL constraints/upgrade/concurrency evidence.
4. **Studio metadata/byte HTTP composition:** owner-only routes, narrow JSON/Proto opt-in,
   raw-byte transfer, negotiated errors/private headers, current reauthorization and real local
   HTTP/storage/restart/security tests. No handler contains rules or SQL; no Theatre/client UI.
5. **Evidence and handoff:** update contract/architecture/read inventory/work queue with what
   actually shipped; record exact command/source scopes, dependency SHAs and deferred acceptance.
   Do not close broad Studio/Publication/Listening issues on this private API evidence.

### Acceptance criteria for this MVP

- Owner can complete one cover workflow using a known intent and operation UUID, including
  recovery after a lost acknowledgement; every accepted preview matches inspected immutable bytes.
- A set/replace/clear commits exactly one new Catalog metadata revision and ledger/receipt;
  stale, foreign, invalid, archived or revoked commands change no state. Explicit clear persists.
- Historical selection at each exact Work revision is reproducible after unrelated edits,
  retries and restarts. Wrong Work, missing/future revision or digest mismatch fails closed.
- Caller MIME/hash/length are declarations; only full supported decode plus verified storage
  readback produces an asset. Dimensions are inspected, alt/provenance/rights claims are bounded.
- JSON default and new metadata Proto represent the same intent/presence and operation identity;
  old JSON/proto/audio/catalog receipts, digests and public DTOs remain byte/shape compatible.
- New objects are private on recorded provider-neutral targets; no URL fetch, public staging,
  provider credential change, image publication grant, job or generation path exists.
- Additive migration/real SQL/HTTP/storage/concurrency/restart and negative evidence are recorded
  separately from passed core/fake tests. Any unresolved L5 gate remains explicitly deferred,
  with the responsible next owner and safe activation boundary; no all-green completion claim.

### Deferred scope, with a concrete trigger rather than placeholder fields

| Deferred | Trigger / next separately scoped decision |
| --- | --- |
| Studio image control and Theatre image projection/delivery | A bounded Studio interaction request, then approved image release snapshot/rights/delivery authority and real consumer; private storage alone does not authorize public display |
| Generated thumbnails/optimization/multiple variants/per-unit art | Measured transfer/render need plus external producer or separately bounded safe inspection/transform policy; this MVP accepts one prepared variant |
| Contributor credits | Real author/narrator/editor display, credit privacy and Work/unit attribution/override policy |
| Genres/tags | Product-owned bounded taxonomy and filter/search need; bundled mock labels are insufficient |
| Seasons/episode display numbers | Podcast sequencing/display requirement beyond stable unit order; integer/decimal and label semantics chosen explicitly |
| Timed markers | Exact rendition-bound navigation requirement, timestamp/duration/version policy |
| Public transcript/captions | Accessibility/publication requirement, independent approved text asset and rights; never alias private Script IR |
| Public license/media alternatives/RSS export | Separate distribution/interoperability request; preserve publication clearance and actual byte proof |
| Object deletion/retention/dedup accounting | Explicit recoverability/retention/operator policy; current immutable orphan/historical bytes remain private |

### Residual risks and conservative decisions already chosen

- **Storage dependency:** PR90 is still unmerged at inspection. Core-first is safe; real adapter
  activation requires final reviewed storage plus current identity. Parent coordinates the merge
  and 0023 reservation; Ultra must refresh develop and report any changed boundary before wiring.
- **Decoder safety:** image 0.25.8 is a compatible pinned candidate, not a completed security
  review. Ultra rechecks current advisories/transitives/MSRV and real hostile fixtures before
  adopting it. If a material unsafe gap is found, report a small bounded decoder decision; do not
  silently enable extra formats, raise limits or upgrade the production compiler.
- **Resource bounds:** <=8 MiB input, <=64 MiB decoded output and two decode permits are explicit
  controls, not a hard total-memory or CPU proof. PNG metadata/codec internals and cancelled
  blocking tasks need the named measurements/tests; unresolved dangerous behavior blocks HTTP
  activation without blocking delivery of independently passed pure/contract work.
- **Private metadata:** unchanged PNG/JPEG can contain EXIF/text/profile information. Owner-only
  exact preview is acceptable here; public publication later needs an explicit privacy/sanitization
  policy. This slice does not claim to remove metadata or establish visual quality.
- **Rights:** required provenance and existing owner evidence references record claims. Actual
  legal/distribution approval remains Publication's responsibility. No contributor or credit
  identity is inferred from the uploader. Recheck existing owner-bound evidence; do not invent
  a new revocation/rights-status field or legal policy for private admission.
- **Product value:** this is an API/DB vertical slice and durable Work choice, not a new visible
  Studio/Theatre screen. It deliberately does not complete #66 or public artwork acceptance.
- **Revision behavior:** changing a cover invalidates old candidate metadata approval through
  the existing guard; a new candidate/approval may be needed. No special bypass is selected.
- **Concurrency/cost:** retain the existing global Catalog clock lock and bounded point/keyset
  reads. No measured contention problem justifies a new head/event system or DBSP now. Private
  failed admissions can leave immutable objects; no unrequested cleanup/delete runs.
- **Identity:** use only the final owner session seam; Studio membership and producer/listener
  grants never become image authority. Do not change OIDC scopes, credentials or provider setup.
- **Protobuf maturity:** opt-in new metadata only; browser/native/real issuer/public delivery
  acceptance from the pilot remains outstanding. Corpus compatibility is not client activation.

No unanswered product choice is required to start the core. A materially changed storage/schema
boundary or demonstrated decoder safety failure is an implementation dependency/acceptance
blocker to report, not a reason to expand this small MVP.

See the [checked primary-source reference](catalog-media-standards-reference.md) and the
[compact new-session Ultra handoff](../handoffs/catalog-media-mvp-ultra-handoff.md).

## Implementation checkpoint (2026-10-13)

The bounded core, metadata contract/fixtures, full decoder, application orchestration and thin
Studio adapter are implemented on refreshed develop `007ae8f`. See the
[execution ledger](../../evidence/private-work-artwork.md) for actual PASS/FAIL/NOT_RUN scopes.
The additive 0023 source and ignored SQL acceptance tests do not establish database execution.
The default host keeps artwork routes unmounted pending the parent's separately reviewed PR90
integration; the current object port refuses persistence/verification rather than claiming
mock-backed storage success. Status: **IO_ACCEPTANCE_PENDING**. Draft publication is a checkpoint,
not completion of issue #104 or authorization to merge.
