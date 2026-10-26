# Shared packages

Only what several consumers really use. A package imports neither `apps/` nor `services/`.

| Package | Contents | Consumers |
| --- | --- | --- |
| [`localization`](localization/README.md) | Accepted versioned semantic catalog, app-owned Web projections and bounded ICU4X formatter core; native bindings remain #507 | Studio Web and Theatre Web |
| [`web-auth`](web-auth/README.md) | Identity v1 cookie-session lifecycle, single-flight idle renewal, its own session wire shapes and browser API addressing | Theatre Web and Studio Web |
| [`store-turso`](store-turso/README.md) | Single-owner embedded Turso foundation: owner lock, checksummed schema registry, serialized `BEGIN IMMEDIATE` writer, typed commit outcomes, verified backup and an engine capability report | [`services/cantos-identity` opt-in persistence](../services/cantos-identity/README.md#file-backed-security-state) (`identity.db`, #334); composed Server [foundation](../docs/architecture/server-turso-schema.md) (`server.db` actor/session foundation, business port incomplete; #333) |
| [`ui-system`](ui-system/README.md) | Canonical semantic tokens, licensed outlined icons and separate renderer ownership | Studio theme/CSS and a bounded Leptos foundation preview; CMP deferred |

Add a package when a second concrete consumer exists and a review accepts its interface; not
before. Contracts are not packages: each service owns its canonical schemas, protos and fixtures
([Server](../services/cantos-server/contracts/README.md), [Identity](../services/cantos-identity/contracts/README.md))
and each consumer keeps its own representation ([ADR 0046](../docs/decisions/0046-service-owned-contracts.md)). See the [repository layout](../docs/architecture/repository-layout.md).
