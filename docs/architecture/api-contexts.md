# API Bounded Contexts and Dependency Policy

> [ADR 0018](../decisions/0018-external-audio-production.md) is the active ownership rule.
> This architecture removes the old server-side `synthesis` bounded context.

## Dependency direction

```text
              Script IR (pure shared kernel)
                        ▲
 editorial ◄── ingestion       adaptation (externally authored proposals)
       ▲              ▲        ▲
       └───────── production (planning/rights/instructions only)
                        ▲
                     media assets/admission/review (implemented)

 catalog ──► editorial
    └──────► ingestion
```

`contexts/editorial` owns canonical revisions and permissions. `ingestion` can read editorial
contracts; `adaptation` can read editorial/ingestion contracts; `production` reads editorial
to freeze guidance, rights and budget claims. Production Briefs additionally consume Catalog
transactional owner/content facts and Ingestion receipts through their owning entrypoints;
production remains downstream of Catalog and adds no table shortcut or generation engine. `catalog` uses editorial and ingestion entrypoints
to validate owner-bound references to existing revisions, rights records and manuscript sources;
it does not read those contexts' tables directly. `contexts/media/domain/wav_inspection` is a
pure inspector, with no provider/model dependency. Media also reads editorial entrypoints for
owner/revision authorization in its narrow private development intake; it does not import
production adapters or move context-specific storage into platform plumbing. Media consumes Catalog domain/facts and Production admitted Briefs through owning entrypoints. The sole reverse Production→Media inquiry is `upload_token_registered`, needed to reject bearer-secret reuse across read/upload issuance under the same Catalog fence. It permits no table/adapter shortcut or general reverse dependency. `interfaces/http`
owns routers and maps
errors; `platform` owns pool/session/migrations/IDs and generic incremental computation,
not business-context shortcuts. The source checker in `scripts/check_repository.py`
enforces the allowed context edges.

`src/composition.rs` owns the concrete application `Store` and aggregate `StoreError`. It
joins the PostgreSQL pool with Media's private-storage registry; it is not a new context or
generic repository framework. `platform/postgres` implements connection/session plumbing and
retains compatibility reexports of those types, with no Media import. Application use cases
map their owned errors into the aggregate; pure domain decisions do not construct it.

Media owns the private read facade used by Studio HTTP. Its use cases resolve owner-authorized
exact receipt/rendition locations, perform object I/O outside transactions and reauthorize the
same location before delivery. The HTTP shell consumes reviewed scope/range/stream types from
that facade; it never imports the storage registry, selects a provider or resolves arbitrary
object keys. See [private storage](object-storage.md) and
[ADR 0022](../decisions/0022-provider-independent-private-storage.md).

## API surfaces

| Prefix | Who uses it | State |
| --- | --- | --- |
| `/api/v1/studio/*` | Studio Web catalog/editorial/import/adaptation/planning; private intake, exact owner reads and publication authority | Partially implemented; opt-in Studio OIDC sessions |
| `/api/v1/theatre/*` | Listener discovery/playback/progress | Public Discovery/Playback and Library/Progress cores implemented; default playback unavailable and listener authentication denied; production acceptance pending |
| `/api/v1/admin/*` | Administrative operations | Reserved; not implemented |
| Studio producer instructions | Exact read-only external bearer grant | [Brief v1](../../packages/contracts/production-brief-v1.md) implemented for development |
| Studio private audio lifecycle | Exact owner/upload scopes | [Audio Lifecycle v1](../../packages/contracts/audio-lifecycle-v1.md) implemented in development; separate [Publication v1](../../packages/contracts/publication-v1.md) core with default unavailable delivery |
| Studio private audio delivery | Current owner session; exact intent/rendition | [Private content reads v1](../../packages/contracts/private-content-reads-v1.md): protected GET/HEAD/Range and refreshable access; no listener access |

Catalog has pure domain, transactional application and PostgreSQL modules plus thin JSON HTTP
adapters exclusively under `/api/v1/studio`. Its [contract](../../packages/contracts/catalog-v1.md)
defines work/content-unit mutations, deterministic pages and immutable Work history.
[Content Identity](../../packages/contracts/content-v1.md) adds separate immutable content,
plain text/recording support and exact owned source/Script IR bindings in the same context.

The legacy alias `/api/v1/*` retains preceding Studio handlers. New private byte/read-access
routes exist only under Studio; they are not added to the deprecated alias. Do not duplicate
handlers or invent implemented route claims from proposal documents. Shared DTOs remain in
`packages/contracts/api`; binary delivery has its own versioned HTTP contract.

## Current contexts

- `editorial`: accepted Script IR revisions, review and authorization.
- `ingestion`: original manuscript bytes and extraction receipts.
- `adaptation`: caller-owned proposal/acceptance, no model inference.
- `catalog`: work/content-unit identities, metadata, authorization and immutable revisions through
  application use cases, PostgreSQL persistence and the authenticated Studio HTTP API.
- `production`: declarative settings/rights/casting, immutable snapshots and approvals, plus
  Catalog-bound [Production Briefs](../../packages/contracts/production-brief-v1.md) and scoped
  read-only producer grants. Source/export bytes are copied through owning-context entrypoints.
  Existing provider catalog entries are **reference planning metadata**, not live adapters.
- `media`: structural WAV inspection, explicit profile/binding rules, bounded Opus-byte
  inspection, private intake/admission/review, immutable location metadata, filesystem and
  generic S3-compatible adapters, exact owner-authorized preview, publication authority and
  public release eligibility. Private content-addressed bytes stay distinct from approved
  publication objects; default public delivery is unavailable.
  [ADR 0020](../decisions/0020-studio-private-audio-intake.md)
  retains intake safeguards; [ADR 0022](../decisions/0022-provider-independent-private-storage.md)
  describes the storage extension and separate real-provider acceptance.

## Listener account state

`contexts/listening` is downstream of Catalog and Media. It owns Library/progress/history,
conditional revisions and durable operation receipts; Media continues to own public eligibility,
publication and playback delivery. Its [v1 contract](../../packages/contracts/listener-library-v1.md)
and [consumer handoff](../work-plan/listener-library-progress-handoff.md) define separate bearer
identity with default-deny composition and PostgreSQL authority. The separate
[Platform identity registry](identity-registry.md) binds external identities but supplies no
listener credential verification or authority from a Studio session.
Catalog/Media exports provide owned transaction fences and bounded eligibility batches; Listening
never reads their private tables directly. HTTP can compose the service/auth port, validate bearer
syntax and map typed errors, but cannot mint principals or run domain reconciliation.

The architecture checker names this context/dependency direction and admits only the two host
composition imports in its test-only PostgreSQL regression module. That module reuses the canonical
publication fixture to test sealed synthetic principals without a production mock-auth feature.
This narrow test seam does not admit interface calls from production Listening adapters.

## Next contexts

Complete broader Studio Web, provider delivery and Theatre identity/native acceptance
separately. The bounded [Audio Lifecycle](../../packages/contracts/audio-lifecycle-v1.md) links
Catalog-bound Briefs and private admission/review; Media's
[Publication core](../../packages/contracts/publication-v1.md) owns approvals and atomic manifest
pointers, and Listening owns account read models. Favor typed IDs, pure state
machines and immutable versioned records. No TTS worker queue, local neural runtime,
commercial speech-provider adapter, synthesis retry system or speech-clip registry belongs here.

## Transactions and external effects

Creator authorization, upload intent, finalize idempotency, observed storage bytes,
review permissions and publication must not race across owners or revisions. Database
transactions lock the scoped authoritative rows; object uploads complete outside them and
are verified before a separate finalized commit. All media stays private until approval.

The historical `durable_*` and `speech_clips` SQL objects remain in applied migrations
for upgrade integrity, but none of the active code writes generation jobs. A future schema
migration should add content/asset tables, not rewrite old migration checksums.

## Compatibility and verification

Keep surviving `revisions`, `production`, `postgres` and read-only `wav_inspection`
aliases temporarily. Deleted provider/`tts`/`worker` legacy library reexports intentionally
have no replacement: callers should integrate the external-audio API, not vendor SDKs.

Use `cargo xtask verify repository`, `verify rust`, `verify contracts`,
`verify postgres` and `preflight --changed-only`. Tests of old TTS dispatch are removed
with the retired runtime; write new tests for upload replay, ownership, byte integrity,
stale-brief decisions and publication instead.
