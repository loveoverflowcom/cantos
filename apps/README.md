# Applications

Two products: Cantos Theatre supports Web and Mobile; Cantos Studio supports Web only. They share contracts and design tokens from
[`packages/`](../packages/README.md) and talk to [`services/cantos-api`](../services/cantos-api/README.md)
over HTTP; they never import each other. See the
[repository layout](../docs/architecture/repository-layout.md).

| Product | Audience | Web (Leptos) | Mobile (Kotlin Compose Multiplatform) |
| --- | --- | --- | --- |
| Cantos Theatre, `cantos-theatre/` (users see **Cantos**) | Listeners | [`cantos-theatre/web`](cantos-theatre/web/README.md): session-only mock; no API/audio | [`cantos-theatre/mobile`](cantos-theatre/mobile/README.md): isolated CMP mock; native audio/API absent |
| Cantos Studio, `cantos-studio/` | Creators | [`cantos-studio/web`](cantos-studio/web/README.md): implemented editor | [`cantos-studio/mobile`](cantos-studio/mobile/README.md): out of scope / not implemented |

[`cantos-forge`](cantos-forge/README.md) is not a client of either product: it is the proposed
producer-side CLI for AI hosts and producers ([ADR 0022](../docs/decisions/0022-cantos-forge-producer-cli.md)).
It runs outside the API runtime, never imports `services/` or another app and, in this
foundation, implements only its verified pure core and `capabilities`.
