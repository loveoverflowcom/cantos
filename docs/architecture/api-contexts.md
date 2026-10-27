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
errors; `platform` owns the Turso facade/registry, sessions and IDs,
not business-context shortcuts. The source checker in `scripts/check_repository.py`
enforces the allowed context edges.

`src/application.rs` owns the concrete application `Store` and aggregate `StoreError`. It
joins the one embedded Turso owner with Media's private-storage registry; it is not a new context
or generic repository framework. `platform/store` implements statement/session plumbing and
retains compatibility reexports of those types, with no Media import. Application use cases
map their owned errors into the aggregate; pure domain decisions do not construct it.

Application shells open/commit Turso units and call context-local `turso` adapters with
current authorization in the same serialized writer unit. Read-only use cases use pinned snapshots.
RPC, media I/O and provider observations stay outside writer units, followed by final rechecks.
This refactor introduces no generic Repository/UnitOfWork or global outbound adapter tree.

The checker places persistence by owner, not by engine. SQL naming a known table (in Rust
strings, `.sql` files or text a file includes, DDL references included) belongs in the owning
context's `turso` adapter module (file/directory layout varies), or in platform code for platform tables. The operator seed's exact
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
dependency. Clock, authenticated-fact expiry and active/spent state arrive as facts.
`platform/bearer/client.rs` maps the three stable registration names to checked Studio/Theatre
policies; those names no longer identify an external OIDC client. Identity v1 credential syntax
lives in `platform/identity_auth/credential.rs`; routing tags are never admission evidence.

[Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) owns
current email/password, Web-cookie and native-Bearer issuance/admission. The shared
[SessionPolicy/lifetime contract](../../services/cantos-server/contracts/auth/bearer/v1/README.md#lifetime-rotation-and-unknown-outcomes)
retains TTLs, replay refusal and the original absolute cap. Private RPC supplies fresh Identity
facts; the Server binds actor, purpose, origin and session in `server.db`. The former provider
Bearer/OIDC flow is retired. Neither a Studio session nor an Identity fact grants Admin authority.

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
distinction explicit without pulling Media storage ownership into generic persistence plumbing.

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
storage, current permissions, append/replay/CAS and editorial reviews. Its Turso adapter
checks accepted sequences, exact canonical exports and both `sir-c1`/`sir-e1` digests before
returning `RevisionFacts`/`ReviewFacts`; shared `append_revision` takes a `RevisionOperation`
and admitted `ScriptContent`, never an HTTP save request. Facts confer no current access:
callers still authorize their resource in the current transaction.

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
| `/api/v1/studio/*` | Studio Web catalog/editorial/import/adaptation/planning; private intake, exact owner reads and publication authority | Implemented native cookie sessions; roster and exact origin required; production acceptance separate |
| `/api/v1/theatre/*` | Listener discovery/playback/progress; separate Web/native identity | Public Discovery/Playback and Library/Progress cores plus opt-in identity implemented; default playback unavailable, unconfigured authentication denied; production acceptance pending |
| `/api/v1/admin/*` | Separate platform authorization and audit | [Capability/revocation and audit core](admin-authorization.md); no production verifier composed, so fail closed |
| Studio producer instructions | Exact read-only external bearer grant | [Brief v1](../../services/cantos-server/contracts/studio/briefs/v1/README.md) implemented for development |
| Studio private audio lifecycle | Exact owner/upload scopes | [Audio Lifecycle v1](../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md) implemented in development; separate [Publication v1](../../services/cantos-server/contracts/studio/publication/v1/README.md) core with default unavailable delivery |
| Studio private audio delivery | Current owner session; exact intent/rendition | [Private content reads v1](../../services/cantos-server/contracts/studio/private-content-reads/v1/README.md): protected GET/HEAD/Range and refreshable access; no listener access |
| Studio Work delegation | Owner session issues/revokes; separate `cantos_dlg_` bearer for one Work and closed Catalog actions | [Delegation v1](../../services/cantos-server/contracts/studio/delegation/v1/README.md) / [ADR 0028](../decisions/0028-work-scoped-delegation.md) and bounded Studio owner panel implemented; live acceptance separate |

Catalog has pure domain, transactional application and Turso modules plus thin JSON HTTP
adapters exclusively under `/api/v1/studio`. Its [contract](../../services/cantos-server/contracts/studio/catalog/v1/README.md)
defines work/content-unit mutations, deterministic pages and immutable Work history.
[Content Identity](../../services/cantos-server/contracts/studio/content/v1/README.md) adds separate immutable content,
plain text/recording support and exact owned source/Script IR bindings in the same context.
Work-scoped [delegation](../../services/cantos-server/contracts/studio/delegation/v1/README.md) also lives in Catalog: a pure
grant decision, an application lane that never turns a bearer into a session actor, and an
append-only SQL adapter. Delegated writes reuse the Content append core with the delegate as
recorded actor and no rights authority.

The legacy alias `/api/v1/*` retains preceding Studio handlers. New private byte/read-access
routes exist only under Studio; they are not added to the deprecated alias. Do not duplicate
handlers or invent implemented route claims from proposal documents. Shared DTOs remain in
`services/cantos-server/src/contract`; binary delivery has its own versioned HTTP contract.

## Current contexts

- `editorial`: accepted Script IR revisions, review and authorization.
- `ingestion`: original manuscript bytes and extraction receipts.
- `adaptation`: caller-owned proposal/acceptance, no model inference.
- `catalog`: work/content-unit identities, metadata, authorization and immutable revisions through
  application use cases, Turso persistence and the authenticated Studio HTTP API.
- `production`: declarative settings/rights/casting, immutable snapshots and approvals, plus
  Catalog-bound [Production Briefs](../../services/cantos-server/contracts/studio/briefs/v1/README.md) and scoped
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
publication and playback delivery. Its [v1 contract](../../services/cantos-server/contracts/theatre/library/v1/README.md)
and [consumer handoff](../work-plan/listener-library-progress-handoff.md) define separate Theatre authority (Web cookie, native Bearer) with default-deny composition and Server-owned Turso authority. The separate
[Platform identity registry](identity-registry.md) binds external identities but supplies no
listener credential verification or authority from a Studio session.
Catalog/Media exports provide owned transaction fences and bounded eligibility batches; Listening
never reads their private tables directly. HTTP can compose the service/auth port, validate channel-specific credential
syntax and map typed errors, but cannot mint principals or run domain reconciliation.

The architecture checker names this context/dependency direction. Current native regressions
reuse canonical publication fixtures and test-only principals without a production mock-auth feature;
[the retirement ledger](server-turso/remaining-pg-retirement.md) preserves old PG assertions.
This narrow test seam does not admit interface calls from production Listening adapters.

## Next contexts

Complete broader Studio Web, provider delivery and Theatre identity/native acceptance
separately. The bounded [Audio Lifecycle](../../services/cantos-server/contracts/studio/audio-lifecycle/v1/README.md) links
Catalog-bound Briefs and private admission/review; Media's
[Publication core](../../services/cantos-server/contracts/studio/publication/v1/README.md) owns approvals and atomic manifest
pointers, and Listening owns account read models. Favor typed IDs, pure state
machines and immutable versioned records. No TTS worker queue, local neural runtime,
commercial speech-provider adapter, synthesis retry system or speech-clip registry belongs here.

## Transactions and external effects

Creator authorization, upload intent, finalize idempotency, observed storage bytes,
review permissions and publication must not race across owners or revisions. Serialized writer units re-read scoped authoritative facts; object uploads complete outside them and
are verified before a separate finalized commit. All media stays private until approval.

The historical `durable_*` and `speech_clips` SQL objects remain in retained PostgreSQL
migrations for upgrade integrity; they are not Server Turso business tables or live generation jobs. A future schema
migration should add content/asset tables, not rewrite old migration checksums.

## Compatibility and verification

Keep surviving `revisions`, `production`, `store`, `http` and read-only `wav_inspection`
aliases temporarily. Deleted provider/`tts`/`worker` legacy library reexports intentionally
have no replacement: callers should integrate the external-audio API, not vendor SDKs.

Use `cargo xtask verify repository`, `verify rust`, `verify contracts`,
`verify turso` and final-diff `preflight --changed-only --base "$BASE_SHA"`. Tests of old TTS dispatch are removed
with the retired runtime; write new tests for upload replay, ownership, byte integrity,
stale-brief decisions and publication instead.
