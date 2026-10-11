# Repository layout

Status: implemented by [decision 0009](../decisions/0009-monorepo-layout.md). The layout moves
existing code without changing behavior, migrations or wire contracts; the
[restructure evidence](../evidence/monorepo-restructure.md) records what was run and what was not.

## Map

```text
apps/
  cantos/                  Cantos Theatre: the listener product
    web/                   Leptos CSR skeleton              (package cantos-theatre)      SKELETON
    mobile/                Kotlin Compose Multiplatform     (guide only, no Gradle)       RESERVED
  cantos-studio/           Cantos Studio: the creator product
    web/                   Leptos CSR script editor         (package cantos-studio)       IMPLEMENTED
    mobile/                Kotlin Compose Multiplatform     (guide only, no Gradle)       RESERVED
services/
  cantos-api/              Rust + Axum + PostgreSQL modular monolith (package cantos-api)  IMPLEMENTED
  kanidm/                  Identity provider deployment/config only; no code              CONFIG ONLY
packages/
  contracts/               Script IR/Studio/production schemas, fixtures, contract docs and
    api/                   the shared wire DTO crate        (package cantos-contracts)    IMPLEMENTED
  ui-system/               Shared canonical tokens/spec, licensed icons and renderer boundary
    leptos/                Web foundation                (package cantos-ui)             IMPLEMENTED
    cmp/                   Native mapping contract        (documentation only)            DEFERRED
docs/  templates/  scripts/  .agents/  .github/             unchanged roles
```

- **IMPLEMENTED** has code that existed before the restructure and kept its tests.
- **SKELETON** builds and renders a statement that nothing exists yet. It has no catalog, account,
  player or fake data, and it is not evidence for any Theatre work item.
- **RESERVED** is a README that fixes the location and the rules from the mobile architecture. It
  has no Gradle project, so nothing about Android or iOS is built or claimed.
- **CONFIG ONLY** documents and validates deployment configuration. Cantos does not yet
  authenticate through Kanidm: the API still uses operator-issued development tokens.

## Dependency direction

```text
apps/*  ──HTTP──▶  services/cantos-api
apps/*  ──code/data──▶  packages/*          services/*  ──code/data──▶  packages/*
```

- `packages/` imports neither `apps/` nor `services/`. An app never imports another app, and an app
  never links `services/` code; the browser or phone talks to the API over HTTP, so no server crate
  enters a WASM dependency graph.
- **Studio and Theatre stay separate.** Each has its own UI, routes, session and permission
  context. A listener request must never reach a Studio route and a Studio screen must not ship in
  the Theatre bundle. Enforcement stays in the backend: private drafts, voice assets and previews
  remain unavailable to public delivery whichever client asks.
- **One backend.** `services/cantos-api` is the single deployable service for both products, as
  decided in [decision 0001](../decisions/0001-modular-monolith.md). Studio v1 is implemented. The
  listener API is planned; when it arrives it is a separate route group and permission context in
  this service, with its own DTO module in `packages/contracts`. A worker may run as a second
  process of the same package.
- **Kanidm is configuration, not code.** `services/kanidm/` holds deployment documentation and an
  example server config. The API will validate tokens issued by it once identity work starts;
  nothing in this repository reimplements an identity provider.

## What is shared, and what is not

| Shared | Where | Consumers today |
| --- | --- | --- |
| Wire DTOs, JSON Schemas, serialized fixtures, contract documents | `packages/contracts` | `cantos-api`, `cantos-studio` |
| Semantic design tokens and Leptos primitives | `packages/ui-system` | Studio shell/editor/casting presentation; Theatre skeleton; development gallery |

Not shared, deliberately:

- **Script IR rules, revisions and production-input rules** remain modules of `cantos-api`
  (`script_ir`, `revisions`, `production`). They have one consumer; the Studio editor validates
  through the API. Extract a crate under `packages/` when a second consumer such as a worker binary
  in another package needs it and a review accepts the interface. Domain code must stay free of
  HTTP, UI and provider types so that move stays mechanical.
- **UI renderers.** Leptos and Compose evolve separately. Web apps may share renderer primitives
  from `packages/ui-system/leptos`, while application patterns/routes/state stay app-owned.
  [UI system](../design/ui-system.md) owns the per-surface intensity: Theatre calm and media-first,
  Studio denser and task-first. Both products use one canonical palette/semantic contract;
  density and layout vary by task. No separate app token copy or Kotlin runtime is introduced.
- **Kotlin code.** No shared Kotlin module exists. Theatre and Studio mobile start as separate
  Gradle builds; introduce `packages/` Kotlin modules only for code both apps really use.

## Cargo workspace

The root `Cargo.toml` lists five members and one `Cargo.lock`. Each builds on its own.

| Member | Package | Build target |
| --- | --- | --- |
| `services/cantos-api` | `cantos-api` | native; binaries `cantos-server`, `cantos-migrate`, `cantos-operator` and the inspection CLIs |
| `apps/cantos-studio/web` | `cantos-studio` | `wasm32-unknown-unknown` through Trunk |
| `apps/cantos/web` | `cantos-theatre` | `wasm32-unknown-unknown` through Trunk |
| `packages/contracts/api` | `cantos-contracts` | native and WASM |
| `packages/ui-system/leptos` | `cantos-ui` | native string-render checks and WASM CSR |

Package names follow their directories, except `cantos-contracts`, which was renamed from
`cantos-api` so that two packages do not share a name. Binary names did not change.

## Build and verify per target

```sh
# service and shared contracts (native)
cargo test -p cantos-api -p cantos-contracts --locked

# Studio web
cargo clippy -p cantos-studio --target wasm32-unknown-unknown --locked -- -D warnings
(cd apps/cantos-studio/web && NO_COLOR=true trunk build --locked)

# Theatre web skeleton
cargo clippy -p cantos-theatre --target wasm32-unknown-unknown --locked -- -D warnings
(cd apps/cantos/web && NO_COLOR=true trunk build --locked)

# Kanidm example config (container image pinned by the deployer)
see services/kanidm/README.md
```

`cantos-server` serves `apps/cantos-studio/web/dist` by default for local development, through
`CANTOS_WEB_DIST`. This is a development convenience for same-origin cookies, not a statement that
the API owns the Studio frontend; Theatre Web will need its own origin or static host.

## Moved paths

Evidence and decision records written before this change cite the old paths on purpose, because
they describe what was run at the time. Their links point to the current files.

| Before | After |
| --- | --- |
| `apps/server/` | `services/cantos-api/` |
| `apps/web/` | `apps/cantos-studio/web/` |
| `apps/mobile/` | `apps/cantos/mobile/` |
| `contracts/` | `packages/contracts/` |
| `contracts/design/studio-tokens-0.1.0.json` | `packages/ui-system/shared/legacy/studio-tokens-0.1.0.json` (via `packages/design-tokens`) |
| package `cantos-server` | package `cantos-api` (binary `cantos-server` unchanged) |
| package `cantos-api` | package `cantos-contracts` |

The subsequent [UI foundation migration](ui-system-migration.md) renames the token package and
adds renderer primitives; the restructure itself did not implement these components.
