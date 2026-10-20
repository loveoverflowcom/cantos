# Applications

Two products: Cantos Theatre supports Web and Mobile; Cantos Studio supports Web only. They share contracts and design tokens from
[`packages/`](../packages/README.md) and talk to [`services/cantos-server`](../services/cantos-server/README.md)
over HTTP; they never import each other. See the
[repository layout](../docs/architecture/repository-layout.md).

| Product | Audience | Web (Leptos) | Mobile (Kotlin Compose Multiplatform) |
| --- | --- | --- | --- |
| Cantos Theatre, `cantos-theatre/` (users see **Cantos**) | Listeners | [`cantos-theatre/web`](cantos-theatre/web/README.md): session-only mock; no API/audio | [`cantos-theatre/mobile`](cantos-theatre/mobile/README.md): isolated CMP mock; native audio/API absent |
| Cantos Studio, `cantos-studio/` | Creators | [`cantos-studio/web`](cantos-studio/web/README.md): implemented editor | [`cantos-studio/mobile`](cantos-studio/mobile/README.md): out of scope / not implemented |

[Cantos Forge](../services/cantos-forge-functions/README.md) lives under `services/` and is
not a client of either product. It preserves the `cantos-forge` crate/CLI identity and runs
outside the API runtime ([ADR 0022](../docs/decisions/0022-cantos-forge-producer-cli.md)).
Its host-assisted local artifact workflow and producer runtime share one core; Forge never
imports Cantos API implementation code or a product client.
