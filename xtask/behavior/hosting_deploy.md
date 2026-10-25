# Draft: `cantos.tooling.hosting-deploy` (#199)

Module-owned inventory of [`cantos.tooling.hosting-deploy`](../../specs/behavior/catalog.json). The [root inventory](../../specs/behavior/inventory.md) keeps the shared authority framing; the registry owns the stable ID and lifecycle.

| Classification | Authority / fact | Model consequence |
| --- | --- | --- |
| Reviewed narrow request | [ADR 0012](../../docs/decisions/0012-xtask-command-surface.md#decision) and the [Hosting command contract](../../docs/deployment/firebase-hosting.md) require explicit one-app intent, a validated bundle, durable intent before one never-retried remote write and honest uncertain outcomes. | The lifecycle stays `draft`; this model does not accept production Hosting or any new origin/auth choice. |
| Implemented, independently of this model | [Coordinator](../src/deploy/flow.rs) and [receipt](../src/deploy/receipt.rs) preserve release outcome separately from smoke and local persistence; native mocked coordinator tests remain the implementation oracle. | No Rust observer is registered; model PASS is not implementation conformance. |
| Finite assumption | One run and one selected app/bundle; preparation and persistence are abstract booleans, and remote outcomes are nondeterministic facts. | Covers intent-before-effect, no repeat write and confirmed-effect retention after failed smoke/local receipt. It does not parse config/URLs, hash bytes, execute Firebase or prove permissions, transport, crash durability or actual deployment. |
