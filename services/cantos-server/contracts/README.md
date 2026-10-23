# Cantos Server contracts

The Server owns the canonical contracts of every HTTP surface it serves
([ADR 0046](../../../docs/decisions/0046-service-owned-contracts.md)). Each module keeps one
authority per version under `<surface>/<module>/<version>/`: its `README.md`, its
`<module>.schema.json` and/or `<module>.proto`, and `fixtures/`. Clients read these files as
build and test inputs and keep their own representations; nothing here is a runtime dependency
or a shared SDK. Identity's private RPC is owned by
[Identity](../../cantos-identity/contracts/README.md); Theatre client conformance corpora are
owned by [Theatre](../../../apps/cantos-theatre/conformance/README.md).

| Surface | Modules (version) | Server representation | Client consumers |
| --- | --- | --- | --- |
| `studio/` (`/api/v1/studio`) | [editorial](studio/editorial/v1/README.md) (v1, JSON + [Protobuf pilot](studio/editorial/v1/protobuf.md)), [script-ir](studio/script-ir/0.1.0/README.md) (0.1.0), [catalog](studio/catalog/v1/README.md), [content](studio/content/v1/README.md), [artwork](studio/artwork/v1/README.md) (JSON + Protobuf), [delegation](studio/delegation/v1/README.md), [briefs](studio/briefs/v1/README.md), [audio-intake](studio/audio-intake/v1/README.md), [audio-lifecycle](studio/audio-lifecycle/v1/README.md), [publication](studio/publication/v1/README.md), [production](studio/production/v1/README.md), [manuscript](studio/manuscript/v1/README.md), [adaptation](studio/adaptation/v1/README.md), [private content reads](studio/private-content-reads/v1/README.md) | `contract::studio` | Studio Web `contract`; Theatre Mobile pilot/artwork vectors (test-only) |
| `theatre/` (`/api/v1/theatre`) | [discovery](theatre/discovery/v1/README.md), [library](theatre/library/v1/README.md), [registration](theatre/registration/v1/fixtures/README.md), [identity](theatre/identity/v1/README.md) (retired) | `contract::theatre` | Theatre Web `contract`; Theatre Mobile (Kotlin) |
| `auth/` (Studio and Theatre) | [identity-auth](auth/identity-auth/v1/README.md), [bearer](auth/bearer/v1/README.md) | `contract::auth` | `cantos-web-auth` `wire`; Theatre Web signup; Theatre Mobile |
| `admin/` (`/api/v1/admin`) | [access](admin/access/v1/README.md) | `contract::admin` | none |
| `producer/` | [handoff](producer/handoff/v1/README.md) (proposed, not implemented) | none | none |
| `historical/` | [TTS receipt](historical/tts-receipt-v1.md), [durable jobs](historical/durable-jobs-v1.md), [clip reuse](historical/clip-reuse-v1.md) | none (retired) | none |

## Consumer representations and code generation

| Consumer | Representation | Generation and drift evidence |
| --- | --- | --- |
| Server (`src/contract`) | hand-written Serde DTOs per surface, `codec`, `routes`; a wire leaf that names no domain or adapter | `tests/contract_conformance` validates fixtures against the schemas and round-trips them; Studio pilot/artwork prost bindings from `tools/proto-codegen` |
| Studio Web (`src/contract`) | only the Studio DTOs, codec and editorial pilot binding it uses | `contract::conformance_tests` round-trips the canonical fixtures; its prost binding is a separate `tools/proto-codegen` output |
| Theatre Web (`src/contract`) | discovery, library, signup and error-envelope DTOs it uses | `contract::conformance_tests` round-trips fixtures and compares signup DTOs with the schema |
| `cantos-web-auth` (`wire`, `address`) | the cookie-session `Session`/`Login` shapes; browser API addressing (client policy) | `wire` tests check a schema-valid v1 session and the legacy fixture |
| Theatre Mobile (Kotlin) | generated library/identity wire from the schemas; hand-written discovery DTOs | Gradle generators read these schemas/fixtures; `PublicCatalogWireDriftTest` and vector tests |

`cargo run --locked --manifest-path tools/proto-codegen/Cargo.toml -- --check` (inside
`cargo xtask verify contracts`) compares every checked-in Protobuf binding with a fresh
generation from its owner's `.proto`. Run it without `--check` after a reviewed `.proto` change.
Compatibility: field numbers and names are frozen; a removed number and name are reserved; new
JSON fields, enum values or units need a new version or the module's documented additive rule.


[Identity auth v1](auth/identity-auth/v1/README.md) owns current email/password sign-in, Web HttpOnly
cookies, CMP/native Bearer, exact freshness/revocation and Studio roster. Its private transport is
[Identity RPC](../../cantos-identity/contracts/identity/v1/README.md). [Bearer lifecycle](auth/bearer/v1/README.md) preserves shared SessionPolicy,
rotation/replay and native journal invariants; provider flow/callback/exchange routes are retired.
The Server composes embedded Turso foundation/sessions; business SQL/schema porting and full
HTTP/browser/device/restore acceptance remain incomplete. Old PG evidence below is historical,
not today's native acceptance or a runnable PG/provider workflow.

The [internal Identity security store v1](../../cantos-identity/docs/security-store-v1.md) records the partial
service-owned file schema, epoch/CAS/event/receipt behavior and real-file fixtures. It supplies
no credential, public API/RPC or authentication-fact contract.

The [internal Identity mailbox store v1](../../cantos-identity/docs/mailbox-store-v1.md) adds challenge issuance,
secret-digest verification, consume/cancel/replace and bound receipts in the same file. It is
not a signup/setup, mail-delivery or authentication contract.

> Current [ADR 0018](../../../docs/decisions/0018-external-audio-production.md):
> [external-audio-handoff-v1](producer/handoff/v1/README.md) is a **proposed**, not implemented,
> API contract with its [Production Brief v1](studio/briefs/v1/README.md) subset implemented. Durable TTS/receipt/speech-reuse contracts below document historical experiments;
> they are **not live server capabilities** after the 2026-10-12 refactor.

The separate [Studio owner audio intake v1](studio/audio-intake/v1/README.md) is implemented for
private development script-revision-bound intents, explicit four-profile Opus inspection and
immutable receipts. It is not the general external-producer proposal or a publication API;
required Rust/PostgreSQL execution is **NOT_RUN** in its implementation environment.

[Script IR 0.1.0](studio/script-ir/0.1.0/README.md) owns the versioned script document, its
read/write policy, canonical bytes and digest. The [Studio v1 contract](studio/editorial/v1/README.md)
serves the Axum store and the Leptos consumer (native business port incomplete). Each consumer
keeps its own Rust DTOs and checks them against the same JSON Schema and fixtures; see
[representations](#consumer-representations-and-code-generation).

The [Studio Catalog v1 contract](studio/catalog/v1/README.md) defines the new JSON-only Work and ContentUnit
hierarchy, revisioned metadata, explicit PATCH omission/null semantics, soft archive, bounded
pages, atomic reorder and retry receipts. Its [schema](studio/catalog/v1/catalog.schema.json) and
[synthetic Vietnamese fixtures](studio/catalog/v1/fixtures/README.md) are shared with the HTTP tests.
Catalog visibility grants no public delivery; optional immutable source/Script IR links grant
neither rights clearance nor publication. The deprecated alias and reserved surface groups gain
no Catalog routes, and Studio Web Catalog UX remains a separate acceptance slice.

The [private Work artwork v1 contract](studio/artwork/v1/README.md) adds raw metadata DTOs, JSON Schema,
strict opt-in Protobuf presence and independent fixtures for one private PNG/JPEG Work cover.
Its metadata/byte/application acceptance stays separate from public delivery and client UI;
existing Catalog and pilot contracts retain their exact representation.

The [Content Identity v1 contract](studio/content/v1/README.md) adds separate immutable content revisions,
normalized text or exact source/Script IR bindings, recording-only units, full revision digests,
owner-bound evidence, atomic retries and bounded history. Metadata/content counters are distinct;
legacy Catalog links do not move managed content. It adds no brief, accepted audio or publication.

The [Production Brief v1 contract](studio/briefs/v1/README.md) exports immutable Catalog content,
rights/guidance/profile pins and independent expiring read permissions. It produces no audio
and grants no upload/review/publication authority.

The [Work-scoped delegation v1 contract](studio/delegation/v1/README.md) implements the
[ADR 0028](../../../docs/decisions/0028-work-scoped-delegation.md) first slice: owner-issued,
show-once `cantos_dlg_` bearer grants for `work:read`, `content:read` and `content:write` on one
Work, recorded under a registered delegate actor and revocable behind the Catalog clock. Its
[schema](studio/delegation/v1/delegation.schema.json) and [fixtures](studio/delegation/v1/fixtures/README.md) are
shared with the wire and PostgreSQL HTTP tests. It grants no Script, Brief, audio, review or
publication authority and replaces no producer grant.

The [Publication v1 contract](studio/publication/v1/README.md) adds current-owner distribution clearance,
exact approval, complete readiness blockers, immutable releases, atomic active pointers and
withdrawal receipts over private candidates. Its JSON schema/fixtures and Studio routes are
implemented; [evidence](../../../docs/evidence/publication-core.md) records verified core and pending
PostgreSQL/public delivery acceptance. Default delivery is unavailable. Audit manifests stay
Studio-only; no public Theatre projection or staging delivery is provided.

The [manuscript import v1 contract](studio/manuscript/v1/README.md) adds a private, immutable source intake
receipt for TXT, Markdown, DOCX and validated Script IR. It uses the existing source registry,
separates exact original bytes from normalized extraction, and never saves a script revision or
grants publication permission automatically. Its hand-authored Schema and independent literal
wire examples are checked by the shared consumer and backend contract tests.

The [adaptation v1 contract](studio/adaptation/v1/README.md) adds private revision-pinned caller contexts, a closed
untrusted proposal format and explicit reviewed acceptance. Server-created IDs/evidence and the
existing Script IR reader form the admission boundary. Proposal citation coverage and findings
stay outside Script IR and require source comparison. The [tool schemas](studio/adaptation/v1/tools.json) expose source-bound context/read/submit/review
operations over the existing authenticated HTTP service and CLI. Caller provenance is declared,
unverified evidence; no inference integration or live model result is implied.

The [production input v1 contract](studio/production/v1/README.md) shares versioned casting, script-scoped
rights/budget declarations, immutable frozen candidates and exact owner approval between Axum
and Studio. Its [schema](studio/production/v1/production.schema.json) and
[synthetic fixtures](studio/production/v1/fixtures/README.md) describe a reference-only capability
catalog. Current eligibility is computed from authoritative facts; freezing is preservation,
not permission to dispatch. Those reference fixtures establish no real voice availability,
legal verification, provider price, audio, reservation or actual billing receipt. Historical
reference snapshots keep their exact identities and output contracts. The catalog now also
declares the separately implemented local eSpeak inspection entry and the Google Cloud TTS
vi-VN entries; their availability evidence belongs to the bounded local TTS and Google adapter
slices.

The [offline TTS receipt inspection contract](historical/tts-receipt-v1.md) is a local server CLI format
over those exact frozen documents. It checks synthetic and actual local WAV bytes plus
caller-recorded attempt/cost facts without changing authority. A user-invoked eSpeak NG 1.52.0
probe saves private frozen-input/intent/receipt/output records with an explicit native 22,050 Hz
PCM profile. Three Vietnamese voices have live synthesis/byte-validation evidence; listening
quality is **NOT_RUN**. This offline format supplies no shared synthesis API, current production
authorization, durable job or audio-asset acceptance. Commercial selection and its live sample remain deferred;
the local slice does not close #6.

The [durable jobs v1 development contract](historical/durable-jobs-v1.md) describes the same-package
authenticated CLI, PostgreSQL run/job/step/attempt records and separate local/fake worker.
It reuses production-p1 frozen documents and receipt-v1 admission; it adds no shared HTTP DTO,
new wire schema or provider abstraction. `durable-input-v1` is an exact resolved-input linkage
fingerprint, not the future reusable speech-cache key. Google commercial dispatch is disabled
and VieNeu remains research-only. Full #7 acceptance, clip reuse and delivery assets are not
claimed by this bounded foundation.

The [clip reuse v1 contract](historical/clip-reuse-v1.md) defines the position-free `speech-v1` fingerprint, the
verified-clip registry, the attempt-less reuse selection and the read-only regeneration plan that
extend the durable foundation. Its independent Python oracle writes
[`speech-vectors.json`](https://github.com/loveoverflowcom/cantos/blob/2de8c69/packages/contracts/fixtures/clip-reuse/v1/speech-vectors.json), Rust must reproduce them, and the
plan report has a [JSON Schema](https://github.com/loveoverflowcom/cantos/blob/2de8c69/packages/contracts/schema/clip-reuse/v1.schema.json) and a literal
[fixture](https://github.com/loveoverflowcom/cantos/blob/2de8c69/packages/contracts/fixtures/clip-reuse/v1/plan-edit-one-line.json). It adds no shared HTTP DTO, Studio route,
object storage, live provider or publication path.

The requester-selected [Theatre mock playback v1 corpus](../../../apps/cantos-theatre/conformance/playback/v1/vectors/README.md)
shares exact state/event/effect vocabulary and hand-authored regression vectors between independent
Rust and Kotlin presentation cores. It is mock-only test data, not a listener HTTP DTO, publication
manifest, production authorization or audio acceptance. Each platform reports its own test gaps.

## Byte representations

The Server's [`contract::codec`](../src/contract/codec.rs) is where its DTOs become JSON or
MessagePack bytes; Studio Web keeps its own [`contract::codec`](../../../apps/cantos-studio/web/src/contract/codec.rs)
for the representations it sends. MessagePack is a named map, decoded from a buffered slice with a
nesting bound, a top-level map and no trailing bytes. Which routes negotiate it is owned by the
[wire codec](../../../docs/architecture/wire-codec.md) and the [Studio contract](studio/editorial/v1/README.md#representations).
[`msgpack-goldens.json`](studio/editorial/v1/fixtures/msgpack-goldens.json) holds the
MessagePack bytes of each Studio fixture, written by the independent standard-library oracle
`scripts/reference_msgpack.py`; Server tests require `rmp-serde` to produce the same bytes and
decode them to the JSON fixture values. After a deliberate fixture change, regenerate with
`python3 scripts/reference_msgpack.py --write-goldens`, review the diff and never edit hex by hand.

See [contract evidence](../../../docs/evidence/script-ir-contract.md) and
[revision evidence](../../../docs/evidence/script-revision-persistence.md). JSON DTOs are
hand-written in each consumer and checked against the same Schema/fixtures; they are not generated
OpenAPI. A generated binding names its source and generation command.

[Private Audio Lifecycle v1](studio/audio-lifecycle/v1/README.md) implements Content/Brief-bound admission, independently scoped upload, append-only review and private candidate readiness. Public publication remains unavailable.

## Theatre Discovery/Playback

[Discovery/Playback v1](theatre/discovery/v1/README.md) adds real anonymous public-release queries,
opaque keysets and a fail-closed playback authorization port. Shared DTOs and strict fixtures
are implemented; real PostgreSQL/public delivery and drift-checked Web/CMP adoption remain
acceptance dependencies. No library/progress or Studio-cookie reuse is introduced.

[Admin v1](admin/access/v1/README.md) defines the dedicated backend capability/revocation responses and
append-only-audit policy boundary. It has shared Rust DTOs, schema and synthetic fixtures; no
Admin UI/client or live Kanidm acceptance is implied.

## Studio Protobuf wire pilot

[Versioned policy and schema](studio/editorial/v1/protobuf.md) own the four actual Studio
revision/validation wire roots. Existing Serde DTOs, exact Script IR text and JSON errors remain
unchanged. `cargo xtask verify contracts` checks independent Python goldens, exact generated
prost drift of every consumer's binding and production Rust codec tests; schema/fixture changes
also select common/JVM Kotlin tests in preflight. [The migration ledger](../../../docs/evidence/protobuf-wire-migration.md)
records deferred browser/SQL/Native acceptance, measured costs and JSON rollback.

## Theatre Identity

[Theatre provider identity v1](theatre/identity/v1/README.md) is retired history. Current mailbox proof,
password and sessions follow [Identity v1](auth/identity-auth/v1/README.md). Actor/purpose/origin isolation,
explicit confirmation, replay/revocation and immutable receipts remain required; no retired
provider model supplies native acceptance. Web/device/secure-storage journeys remain separate.

The [Theatre client error presentation v1](../../../apps/cantos-theatre/conformance/client-error-presentation-v1.md) owns default origin,
client taxonomy, modal lifetime and safe recovery semantics shared by Web/CMP.
