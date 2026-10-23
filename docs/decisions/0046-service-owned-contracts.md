# ADR 0046: Service-owned contracts and consumer-owned representations

- Status: Accepted (requested by the project owner, 2026-10-23); source migration implemented; wire, browser, device and deployed acceptance unchanged and still governed by each contract
- Date: 2026-10-23
- Decision owner: Cantos owner
- Related: [ADR 0009](0009-monorepo-layout.md), [ADR 0015](0015-api-surface-boundaries.md), [ADR 0023](0023-versioned-protobuf-wire.md), [native refactor](../architecture/native-turso-refactor.md)
- Supersedes in part: ADR 0009's single `packages/contracts` location and ADR 0023's `prost = 0.13.5` pin

## Context

`packages/contracts` held every interchange concern in one place: hand-written Rust DTOs for the
Studio, Theatre, Admin and authentication surfaces (`cantos-contracts`), the private Identity RPC
proto with its generated client *and* server (`cantos-identity-rpc`), JSON Schemas, `.proto`
files, fixtures, contract documents, a JSON/MessagePack/Protobuf codec, URL constants and
browser API addressing. The Server, Studio Web, Theatre Web and `cantos-web-auth` all imported the
one crate; Identity and the Server shared one generated RPC crate. The consequences were:

- the crate was the Server's wire layer and every client's SDK at once, so a Server-side DTO
  edit recompiled and re-linted every Web client and nothing showed which consumer needed a type;
- Server and Identity shared generated client *and* server glue, so neither side owned its half;
- deployment addressing (`web_api`) and transport policy lived in the contract crate;
- Studio Web linked the artwork Protobuf codec and every Studio module it never calls;
- the Identity codegen drift check was manual and not part of any gate.

## Options

| | A: central `packages/contracts` | B: contracts per service | C: per service and module |
| --- | --- | --- | --- |
| Schema ownership | one shared owner | defining service | defining service, per surface/module/version |
| Coupling / build edges | every client depends on one crate | clients depend on no crate; read service schemas | same as B, narrower change impact |
| Independent client generation | no; one Rust SDK | yes | yes |
| Versioning | one tree, mixed versions | per service | per module (`catalog/v1`, `script-ir/0.1.0`) |
| Multi-language maintenance | Rust DTOs shared, Kotlin separate | each consumer its own | each consumer its own |
| Generated-code drift | one copy | one copy per consumer, drift-checked | same as B |
| Repository ergonomics | one place to look | per-service index | per-service index plus module folders |
| Migration effort | none | moderate | moderate (same moves, finer folders) |

## Decision

Option C. Each service owns the canonical contract of the communication it provides; consumers
own their language representation.

1. **Canonical authority.** `services/cantos-server/contracts/<surface>/<module>/<version>/` holds
   the Server's published HTTP contracts: `studio/`, `theatre/`, `admin/`, `auth/` (public sign-in
   served on Studio and Theatre), the proposed `producer/` handoff and retired `historical/`
   records. A version folder holds `README.md` (the contract), `<module>.schema.json` and/or
   `<module>.proto`, and `fixtures/`. `services/cantos-identity/contracts/identity/v1/` holds the
   private `cantos.identity.v1` proto and its wire goldens. JSON-only APIs keep their versioned
   JSON Schema as the single authority; nothing is switched to Protobuf and no parallel `.proto`
   is invented for them. The four Studio Protobuf pilot roots and artwork keep their one `.proto`
   beside the JSON Schema, as ADR 0023 already specifies.
2. **Consumer representations.** No crate is shared for contracts:
   - the Server owns `services/cantos-server/src/contract` (`studio`, `theatre`, `admin`, `auth`,
     `codec`, `routes`), a wire leaf that names no domain, application, adapter or platform code;
   - Studio Web owns `apps/cantos-studio/web/src/contract` with only the Studio DTOs, codec and
     editorial Protobuf pilot binding it uses;
   - Theatre Web owns `apps/cantos-theatre/web/src/contract` (discovery, library, signup, error
     envelope);
   - `cantos-web-auth` owns its two session shapes (`wire`) and the browser API addressing
     (`address`), which is client transport policy, not a contract;
   - Theatre Mobile keeps its existing Kotlin generators and hand-written DTOs, now reading the
     service-owned schemas and fixtures as Gradle build inputs.
3. **Code generation per consumer.** JSON DTOs stay hand-written in each Rust consumer: the
   schemas encode PATCH missing/null/value, safe-integer bounds, `deny_unknown_fields` and
   redacted `Debug` that no available Rust JSON-Schema generator reproduces without a new
   framework. Each consumer proves its copy against the canonical fixtures (Server: schema
   validation plus round trip; clients: round trip and schema property sets). Protobuf bindings
   are generated: the pinned, isolated `tools/proto-codegen` writes one checked-in binding per
   consumer and side (Server Studio pilot and artwork, Studio Web pilot, Identity RPC server,
   Server Identity RPC client, and each side's test peer) and `--check` compares them with a fresh
   generation inside `protobuf-reference`. Generated files carry wire semantics only; adapters,
   validation (`validate_fact`), redaction and domain mapping stay hand-written in the consumer.
4. **One prost version.** A crate cannot hold two prost majors, and the Server now owns both its
   Studio pilot codec and its Identity client, so the Studio pilot moves from prost 0.13.5 to
   0.14.4 (the Identity RPC pin). The generated messages differ only by added `Eq, Hash` derives;
   field numbers, presence and the independent Python/Kotlin goldens are unchanged.
5. **Fixtures.** Fixtures live beside the schema of the service that owns the contract and are test
   evidence only. Client-only conformance corpora shared by Theatre Web and Theatre Mobile (mock
   playback vectors, client-boundary probes, client error presentation) are product-owned and move
   to `apps/cantos-theatre/conformance`. Identity's RPC goldens move from Rust literals to the
   Identity-owned `fixtures/wire-goldens.json`, read by both sides. Internal Identity store
   documents are implementation docs (`services/cantos-identity/docs`), not contracts.
6. **Boundaries enforced.** `packages/contracts` is retired and the repository gate refuses its
   return. Apps and packages still never depend on a service crate; a service reaches another
   only through its own generated client. Inside the Server, generated Identity RPC types and
   tonic stay in the platform Identity adapter, prost only there and in the Studio Protobuf codec.

## Consequences

- A change to a Server DTO selects only the Server; a change to a canonical schema or fixture
  selects the Server and that surface's consumers (preflight rules follow the folders).
- Each client carries a copy of the DTOs it uses; drift is caught by its own fixture tests, the
  Server's schema tests and the Protobuf drift check, not by sharing memory layout.
- Historical evidence keeps its original commands and paths; links were updated.
- The Server's pure domains still use some of its own wire DTOs as persisted canonical documents,
  as before. Separating those is domain work outside this decision.

## Remaining ledger

- No deployed, browser or device behavior was re-accepted by this change; each contract keeps
  its own acceptance status.
- `services/cantos-server/legacy-pg-tests` (outside Cargo) still named `cantos_contracts` and
  `packages/contracts` paths when this decision landed. They were since ported or retired, and no
  file remains ([retirement record](../architecture/server-turso/remaining-pg-retirement.md)).
