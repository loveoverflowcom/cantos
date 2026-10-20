# Shared packages

Only what several consumers really use. A package imports neither `apps/` nor `services/`.

| Package | Contents | Consumers |
| --- | --- | --- |
| [`contracts`](contracts/README.md) | Script IR, Studio, import, adaptation and production schemas, fixtures, contract documents, and the `cantos-contracts` Rust wire DTO crate | `services/cantos-server`, `apps/cantos-studio/web` |
| [`web-auth`](web-auth/README.md) | RAM Bearer lifecycle, single-flight refresh and browser PKCE/popup adapter | Theatre Web and Studio Web |
| [`ui-system`](ui-system/README.md) | Canonical semantic tokens, licensed outlined icons and separate renderer ownership | Studio theme/CSS and a bounded Leptos foundation preview; CMP deferred |

Add a package when a second concrete consumer exists and a review accepts its interface; not
before. See the [repository layout](../docs/architecture/repository-layout.md).
