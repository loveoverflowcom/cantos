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
| `studio/` (`/api/v1/studio`) | [editorial](studio/editorial/v1/README.md) (v1, JSON + [Protobuf pilot](studio/editorial/v1/protobuf.md)), [script-ir](studio/script-ir/0.1.0/README.md) (0.1.0), [catalog](studio/catalog/v1/README.md), [content](studio/content/v1/README.md), [artwork](studio/artwork/v1/README.md) (JSON + Protobuf), [cover disclosure](studio/cover-disclosure/v1/README.md) (JSON), [metadata publication](studio/metadata-publication/v1/README.md) (JSON), [preview](studio/preview/v1/README.md) (JSON), [delegation](studio/delegation/v1/README.md), [briefs](studio/briefs/v1/README.md), [audio-intake](studio/audio-intake/v1/README.md), [audio-lifecycle](studio/audio-lifecycle/v1/README.md), [publication](studio/publication/v1/README.md), [production](studio/production/v1/README.md), [manuscript](studio/manuscript/v1/README.md), [adaptation](studio/adaptation/v1/README.md), [private content reads](studio/private-content-reads/v1/README.md) | `contract::studio` | Studio Web `contract`; Theatre Mobile pilot/artwork vectors (test-only) |
| `theatre/` (`/api/v1/theatre`) | [discovery](theatre/discovery/v1/README.md), [account presentation](theatre/account-presentation/v1/README.md), [library](theatre/library/v1/README.md), [registration](theatre/registration/v1/fixtures/README.md), [identity](theatre/identity/v1/README.md) (retired) | `contract::theatre` | Theatre Web `contract`; Theatre Mobile (Kotlin) |
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
Compatibility: Protobuf field numbers and names are never reused; a removed number and name are
reserved. During development a breaking change replaces the current shape in the same delivery
that updates and tests every current consumer, with no version lane or alias kept for older
builds ([ADR 0055](../../../docs/decisions/0055-development-phase-compatibility.md)).


## Current runtime and persistence

[Identity auth v1](auth/identity-auth/v1/README.md) owns email/password, Web HttpOnly cookies,
CMP/native Bearer, freshness/revocation and the Studio roster. Identity owns its
[private RPC](../../cantos-identity/contracts/identity/v1/README.md),
[security store](../../cantos-identity/docs/security-store-v1.md) and
[mailbox store](../../cantos-identity/docs/mailbox-store-v1.md). Stored proof/state is not a session.

Server actors, sessions and business contexts use one embedded `server.db`; native mappings
and test dispositions are in the [source boundary](../../../docs/architecture/server-turso-porting.md#checkpoint-boundary)
and [retirement ledger](../../../docs/architecture/server-turso/remaining-pg-retirement.md).
Retained PG migrations/checksums are historical recovery inputs, not native schema instructions.
Wire fields, IDs, limits, digests, operation receipts and version compatibility survive the port.

Studio Catalog/Content, Brief, private audio/artwork and Publication have native implementations.
Private admission never publishes; publication requires current rights, review/approval and a
configured verified delivery adapter. Public covers additionally require an exact eligible
release pin and owner disclosure. See the individual module contracts in the table above.
Theatre Discovery/Playback and Library/Progress have Web/CMP consumers; release-bound bookmarks
have a CMP consumer. Browser/device, real storage/delivery and deployed acceptance stay separate
from DTO/file/loopback evidence.
Admin has native audit/revocation persistence but no composed production credential verifier;
all production Admin routes fail closed. The general producer handoff remains a proposal.

## Byte representations

The Server's [`contract::codec`](../src/contract/codec.rs) owns its wire bytes; Studio Web keeps
its own [`contract::codec`](../../../apps/cantos-studio/web/src/contract/codec.rs).
JSON is the default. Negotiated MessagePack uses named maps, bounded buffered decoding and no
trailing bytes; the [wire codec policy](../../../docs/architecture/wire-codec.md) and
[Editorial representations](studio/editorial/v1/README.md#representations) own exact route scope.
[`msgpack-goldens.json`](studio/editorial/v1/fixtures/msgpack-goldens.json) is generated by the
independent `scripts/reference_msgpack.py --write-goldens` oracle, never edited as hex by hand.

## Studio Protobuf wire pilot

The [pilot policy/schema](studio/editorial/v1/protobuf.md) owns the four Studio revision/validation
roots. Existing Serde DTOs, exact Script IR text and JSON errors stay unchanged. The
[artwork contract](studio/artwork/v1/README.md) separately owns its Protobuf presence rules.
`cargo xtask verify contracts` checks independent goldens and every consumer's generated binding;
browser/SQL/native activation and performance evidence remain separate.

## Historical and client-only records

Retired [TTS receipt](historical/tts-receipt-v1.md), [durable jobs](historical/durable-jobs-v1.md)
and [clip reuse](historical/clip-reuse-v1.md) preserve experiment history. Their old commands,
provider integrations and PG/DBSP records are not live Server capabilities after ADR 0018.
The retired [Theatre provider identity](theatre/identity/v1/README.md) grants no native acceptance.

The Theatre-owned [mock playback corpus](../../../apps/cantos-theatre/conformance/playback/v1/vectors/README.md)
is test-only, not a public API or audio witness. [Client error presentation](../../../apps/cantos-theatre/conformance/client-error-presentation-v1.md)
owns client defaults, error taxonomy and recovery semantics independently of Server DTOs.
