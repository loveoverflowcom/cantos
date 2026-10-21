# Shared packages

Only what several consumers really use. A package imports neither `apps/` nor `services/`.

| Package | Contents | Consumers |
| --- | --- | --- |
| [`contracts`](contracts/README.md) | Script IR, Studio, import, adaptation and production schemas, fixtures, contract documents, and the `cantos-contracts` Rust wire DTO crate | `services/cantos-server`, `apps/cantos-studio/web` |
| [`web-auth`](web-auth/README.md) | RAM Bearer lifecycle, single-flight refresh and browser PKCE/popup adapter | Theatre Web and Studio Web |
| [`store-turso`](store-turso/README.md) | Single-owner embedded Turso foundation: owner lock, checksummed schema registry, serialized `BEGIN IMMEDIATE` writer, typed commit outcomes, verified backup and an engine capability report | [`services/cantos-identity` opt-in persistence](../services/cantos-identity/README.md#file-backed-security-state) (`identity.db`, #334); Server consumer remains planned (`server.db`, #333), no production composition |
| [`contracts/identity-rpc`](contracts/identity-rpc-v1.md) | Draft `cantos.identity.v1` private RPC: proto, checked-in prost/tonic code, fact validation and wire goldens (#332 prerequisite slice) | [`services/cantos-identity` `service` feature](../services/cantos-identity/README.md#executable); no Server consumer yet (#335) |
| [`ui-system`](ui-system/README.md) | Canonical semantic tokens, licensed outlined icons and separate renderer ownership | Studio theme/CSS and a bounded Leptos foundation preview; CMP deferred |

Add a package when a second concrete consumer exists and a review accepts its interface; not
before. See the [repository layout](../docs/architecture/repository-layout.md).
