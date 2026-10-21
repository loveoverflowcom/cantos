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
production adapters or move context-specific storage into platform plumbing. Media consumes Catalog domain/facts and Production admitted Briefs through owning entrypoints. The sole reverse Production→Media inquiry is `upload_token_registered`, needed to reject bearer-secret reuse across read/upload issuance under the same Catalog fence. It permits no table/adapter shortcut or general reverse dependency. `adapters/inbound/http`
owns routers and maps
errors; `platform` owns pool/session/migrations/IDs and generic incremental computation,
not business-context shortcuts. The source checker in `scripts/check_repository.py`
enforces the allowed context edges.

`src/application.rs` owns the concrete application `Store` and aggregate `StoreError`. It
joins the PostgreSQL pool with Media's private-storage registry; it is not a new context or
generic repository framework. `platform/postgres` implements connection/session plumbing and
retains compatibility reexports of those types, with no Media import. Application use cases
map their owned errors into the aggregate; pure domain decisions do not construct it.

Application shells deliberately open/commit PostgreSQL transactions and call context-local
`contexts/<context>/postgres` functions with authorization and locks in the same transaction.
This refactor introduces no generic Repository/UnitOfWork or global outbound adapter tree.

The checker places persistence by owner, not by engine. SQL naming a known table (in Rust
strings, `.sql` files or text a file includes, DDL references included) belongs in the owning
context's adapter module, `contexts/<context>/postgres` today or `contexts/<context>/turso` for
the #333 port, or in platform code for platform tables. The operator seed's exact
(file, verb, table) inserts are the only platform access to context tables, and only the
migration registry includes the whole schema. A backend name elsewhere (`application/turso/`, a
top-level `turso/`) is not an adapter. Each adapter opens only its own backend, other context
code reaches Turso only through its adapter, HTTP opens no driver, no SQL attaches another
database, and generated Identity RPC types stay in `platform/identity_auth`. The Identity crate
and workspace manifests have matching checks. These are static checks of explicit paths and
literal SQL: macros, dynamic SQL, comma-separated `FROM` lists, `#[path]`/`include!` mounts and
re-exports remain review obligations, and process/file isolation, one-owner locking and
cross-database reads still need runtime tests.

## Session policy and credential boundary

`platform/bearer/core.rs` owns immutable, checked `SessionPolicy` values and pure lifetime/
rotation decisions. It has no registration, OIDC, surface, credential syntax, HTTP or SQL
dependency. Clock, proof/provider expiry and active/spent state arrive as facts. The server's
exhaustive `bearer/client.rs` mapping chooses the existing Studio or Theatre policy, referring
to the canonical registration IDs in `oidc/config.rs`. `BearerIdentity` checks the exact
configuration/client match; its application and persistence callers resolve the policy through
that single server-owned mapping. `bearer/credential.rs` owns opaque-token routing
tags and protocol syntax. A tag is a routing hint, never admission evidence.

The [Bearer lifetime contract](../../packages/contracts/bearer-v1.md#lifetime-rotation-and-unknown-outcomes)
keeps the exact TTLs, refusal/replay precedence and original absolute cap. The application and
PostgreSQL shell retain roster checks, hash/client/issuer/family bindings, actor/family locks,
provider I/O outside transactions and final-time rereads. This boundary does not implement
#261's typed admission result or #295's Web-cookie migration.

## HTTP assembly and transport owners

The canonical Rust path is `adapters::inbound::http`. The former `interfaces/http` implementation
and namespace were moved in #327; the crate-root `cantos_server::http` alias remains for compatibility.
Historical source hashes and commit-pinned links retain their original paths. Naming this
concrete Axum adapter does not claim that the former name violated Hexagonal Architecture.

`adapters/inbound/http/assembly.rs` is the trusted outer composition owner: it constructs one
concrete capability bundle and selects identity, artwork and static-serving capabilities.
`router.rs` consumes that bundle to mount/layer the same Studio, Theatre, Admin and deprecated
Studio routes. `state.rs` owns request state; `errors.rs` owns JSON error projection; `policy.rs`
owns cross-surface Origin, deadline and cache/security headers. Studio credential selection,
session/query/header fencing and the admission lease around the handler live in
`studio/admission.rs`. Cookies, CORS, codecs and binary transport remain HTTP-owned.

Dependency arrows (not response/data flow) point from outer assembly and HTTP to application
capabilities. Inner contexts never import this assembly or HTTP DTO compatibility methods.
Moving the concrete handle from the former `composition.rs` to `application.rs` makes that
distinction explicit without pulling Media storage ownership into PostgreSQL plumbing.

| Existing host entrypoint | Identity/artwork selection | Static fallback |
| --- | --- | --- |
| `router` | Configured Theatre identity, otherwise deny; artwork only with private storage | Development dist only without private storage/artwork |
| `api_router` | Same capability selection | Never; production host also disables development Studio credentials before assembly |
| `router_with_publication_delivery`, `router_with_theatre_delivery`, `router_with_origin_playback` | Same selection; override only the named delivery capability | Same development guard |
| `router_with_public_media` | Same selection; caller supplies publication service plus matching origin playback | Never |
| `router_with_listener_identity` | Explicit authenticator; same artwork selection | Same development guard |
| `router_with_private_artwork` | Explicit artwork and denied Library authenticator, preserving this seam even with configured identity routes | Never |

Without delivery configuration, publication/playback remains unavailable. The host-settings
module remains a pure configuration decision; it is not an Axum runtime container. This source
reorganization changes no URL, DTO, issuer, token, transaction, deployment or #295 auth policy.

Media owns the private read facade used by Studio HTTP. Its use cases resolve owner-authorized
exact receipt/rendition locations, perform object I/O outside transactions and reauthorize the
same location before delivery. The HTTP shell consumes reviewed scope/range/stream types from
that facade; it never imports the storage registry, selects a provider or resolves arbitrary
object keys. See [private storage](object-storage.md) and
[ADR 0037](../decisions/0037-provider-independent-private-storage.md).

## Editorial application and document boundaries

`script_ir` owns only read/validation, canonical encoding and diff. Editorial owns revision
storage, current permissions, append/replay/CAS and editorial reviews. Its PostgreSQL adapter
checks accepted sequences, exact canonical exports and both `sir-c1`/`sir-e1` digests before
returning `RevisionFacts`/`ReviewFacts`; shared `append_revision` takes a `RevisionOperation`
and admitted `ScriptContent`, never an HTTP save request. Facts confer no current access:
callers still lock and authorize their resource in the existing transaction.

Catalog and Media use those internal facts for exact owner/revision/digest binding. Production
and Adaptation explicitly project facts through `editorial::document` when freezing the existing
versioned revision document. That document retains its historical `RevisionResponse` Rust name,
field order, exact canonical text and timestamp precision; changing an internal Rust type does
not rewrite Briefs, frozen inputs or submission/acceptance receipts. HTTP's Studio Editorial
adapter owns the compatibility `Store::load/save/history/review` facade and response projections.
No context calls that HTTP facade internally.

`editorial::domain::diagnostics` owns one safe classification of reader errors into stable
code/path facts, discarding private payloads. Adaptation returns `AdaptationRejection` with these
safe diagnostics. HTTP maps them to `FieldIssue`, and the Adaptation document adapter freezes
them into the unchanged versioned `AdaptationProblem` envelope at receipt creation. Neither
pure diagnostic path constructs a wire error or imports HTTP.

## API surfaces

| Prefix | Who uses it | State |
| --- | --- | --- |
| `/api/v1/studio/*` | Studio Web catalog/editorial/import/adaptation/planning; private intake, exact owner reads and publication authority | Partially implemented; opt-in Studio OIDC sessions |
| `/api/v1/theatre/*` | Listener discovery/playback/progress; separate Web/native identity | Public Discovery/Playback and Library/Progress cores plus opt-in identity implemented; default playback unavailable, unconfigured authentication denied; production acceptance pending |
| `/api/v1/admin/*` | Separate platform authorization and audit | Opt-in [capability/revocation slice](admin-authorization.md); live provider acceptance pending |
| Studio producer instructions | Exact read-only external bearer grant | [Brief v1](../../packages/contracts/production-brief-v1.md) implemented for development |
| Studio private audio lifecycle | Exact owner/upload scopes | [Audio Lifecycle v1](../../packages/contracts/audio-lifecycle-v1.md) implemented in development; separate [Publication v1](../../packages/contracts/publication-v1.md) core with default unavailable delivery |
| Studio private audio delivery | Current owner session; exact intent/rendition | [Private content reads v1](../../packages/contracts/private-content-reads-v1.md): protected GET/HEAD/Range and refreshable access; no listener access |
| Studio Work delegation | Owner session issues/revokes; separate `cantos_dlg_` bearer for one Work and closed Catalog actions | [Delegation v1](../../packages/contracts/delegation-v1.md) / [ADR 0028](../decisions/0028-work-scoped-delegation.md) and bounded Studio owner panel implemented; live acceptance separate |

Catalog has pure domain, transactional application and PostgreSQL modules plus thin JSON HTTP
adapters exclusively under `/api/v1/studio`. Its [contract](../../packages/contracts/catalog-v1.md)
defines work/content-unit mutations, deterministic pages and immutable Work history.
[Content Identity](../../packages/contracts/content-v1.md) adds separate immutable content,
plain text/recording support and exact owned source/Script IR bindings in the same context.
Work-scoped [delegation](../../packages/contracts/delegation-v1.md) also lives in Catalog: a pure
grant decision, an application lane that never turns a bearer into a session actor, and an
append-only SQL adapter. Delegated writes reuse the Content append core with the delegate as
recorded actor and no rights authority.

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
  retains intake safeguards; [ADR 0037](../decisions/0037-provider-independent-private-storage.md)
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
