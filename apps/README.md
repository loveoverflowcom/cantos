# Applications

Two products, each with a web and a mobile client. They share contracts and design tokens from
[`packages/`](../packages/README.md) and talk to [`services/cantos-api`](../services/cantos-api/README.md)
over HTTP; they never import each other. See the
[repository layout](../docs/architecture/repository-layout.md).

| Product | Audience | Web (Leptos) | Mobile (Kotlin Compose Multiplatform) |
| --- | --- | --- | --- |
| Cantos Theatre, `cantos-theatre/` (users see **Cantos**) | Listeners | [`cantos-theatre/web`](cantos-theatre/web/README.md): skeleton | [`cantos-theatre/mobile`](cantos-theatre/mobile/README.md): guide only |
| Cantos Studio, `cantos-studio/` | Creators | [`cantos-studio/web`](cantos-studio/web/README.md): implemented editor | [`cantos-studio/mobile`](cantos-studio/mobile/README.md): reserved |
