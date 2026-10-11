# Shared packages

Only what several consumers really use. A package imports neither `apps/` nor `services/`.

| Package | Contents | Consumers |
| --- | --- | --- |
| [`contracts`](contracts/README.md) | Script IR, Studio, import, adaptation and production schemas, fixtures, contract documents, and the `cantos-contracts` Rust wire DTO crate | `services/cantos-api`, `apps/cantos-studio/web` |
| [`design-tokens`](design-tokens/README.md) | Versioned semantic token source | Studio CSS through `scripts/studio_tokens.py` |

Add a package when a second concrete consumer exists and a review accepts its interface; not
before. See the [repository layout](../docs/architecture/repository-layout.md).
