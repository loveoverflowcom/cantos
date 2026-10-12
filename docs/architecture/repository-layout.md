# Repository layout

> Active scope: [ADR 0018](../decisions/0018-external-audio-production.md).
> Cantos manages content, external-producer instructions and uploaded audio. TTS workers,
> synthesis job CLIs and backend mixers are removed; details below describing the original
> repository restructure are historical unless marked current.

Status: implemented by [decision 0009](../decisions/0009-monorepo-layout.md). The layout moves
existing code without changing behavior, migrations or wire contracts; the
[restructure evidence](../evidence/monorepo-restructure.md) records what was run and what was not.

## Platform scope

| Product | Web | Mobile |
| --- | --- | --- |
| Cantos Studio | Supported; sole deployment target | Out of scope / not implemented |
| Cantos Theatre (listeners see **Cantos**) | Supported | Supported; Android/iOS with native playback |

Cantos Studio is web-only. Do not implement, scaffold, build or maintain a mobile client unless explicitly requested in a future roadmap.

Support here defines product scope, not implementation evidence. Theatre Web is a session-only
UI mock with no API/audio integration and
Theatre Mobile has an isolated [CMP mock](../../apps/cantos-theatre/mobile/README.md), without native audio or API. Studio Mobile is only a scope notice: never a workspace
member, Gradle module, xtask target, CI lane or automatic build/test/preview consumer. Shared
contracts, tokens and libraries may serve Theatre Mobile and Studio Web. Historical decisions
and evidence retain their original scope; this matrix supersedes the Studio Mobile reservation
in [decision 0009](../decisions/0009-monorepo-layout.md).

## Map

```text
apps/
  cantos-theatre/          Cantos Theatre: the listener product (user-facing name: Cantos)
    web/                   Leptos CSR session-only mock     (package cantos-theatre)      MOCK
    mobile/                Kotlin Compose Multiplatform     (isolated mock, no audio/API) DEMO
  cantos-studio/           Cantos Studio: the creator product
    web/                   Leptos CSR script editor         (package cantos-studio)       IMPLEMENTED
    mobile/                Scope notice only                (no implementation)          OUT OF SCOPE
services/
  cantos-api/              Rust + Axum + PostgreSQL modular monolith (package cantos-api)  IMPLEMENTED
  kanidm/                  Identity provider deployment/config only; no code              CONFIG ONLY
packages/
  contracts/               Script IR/Studio/production schemas, fixtures, contract docs and
    api/                   the shared wire DTO crate        (package cantos-contracts)    IMPLEMENTED
  ui-system/               Shared canonical tokens/spec, licensed icons and renderer boundary
    leptos/                Web foundation                (package cantos-ui)             IMPLEMENTED
    cmp/                   Shared native controls and JVM desktop diagnostics              PREVIEW
xtask/                     cargo xtask: build, run and verification command surface (package xtask)
.cargo/                    the `cargo xtask` alias
docs/  templates/  scripts/  .agents/  .github/             unchanged roles; scripts/ are xtask primitives
```

- **IMPLEMENTED** has code that existed before the restructure and kept its tests.
- **MOCK** is the session-only Theatre Web mock: synthetic catalog, simulated player and in-memory
  library/queue, with evidence in its [record](../evidence/theatre-web-mock.md). It has no listener
  API, audio, account or persistence and is not evidence for any Theatre work item.
- **DEMO** is the isolated Theatre CMP mock: shared Kotlin screens and Android/iOS host wiring,
  with target evidence in its [record](../../apps/cantos-theatre/mobile/docs/mock-skeleton.md).
  It implements no native audio, listener API, background playback, downloads or durable sync.
- **OUT OF SCOPE** is a policy notice, not a reserved client or a future automatic build target.
- **CONFIG ONLY** documents and validates deployment configuration. Cantos does not yet
  authenticate through Kanidm: the API still uses operator-issued development tokens.
- **PREVIEW** is the separately requested [two-renderer catalogue](../../packages/ui-system/README.md#two-diagnostic-apps).
  It owns component/token diagnostics, not a Theatre/Studio mobile app or native listening.

## Naming: technical identifiers and user-facing names

| Context | Name |
| --- | --- |
| Listener app path, package and Trunk binary | `cantos-theatre` (`apps/cantos-theatre/{web,mobile}`) |
| Listener API module, route group and DTOs | `theatre` (`/api/v1/theatre`, `cantos_contracts::theatre`) |
| Creator app path, package, API route group | `cantos-studio`, `studio` (`/api/v1/studio`) |
| Operations API module and route group | `admin` (`/api/v1/admin`) |
| Shared backend | `cantos-api` |
| Listener-facing title, navigation, splash, manifest, metadata, localization | **Cantos** |
| Creator-facing product title | **Cantos Studio** |

`cantos-theatre` and "Cantos Theatre" are technical and documentation names. A listener never sees
"Cantos Theatre": the Web `<title>`, headings and any PWA/mobile label say **Cantos**. Shared
domain nouns (Work, Episode, Script IR, Publication) keep their names and belong to no surface.
The listener app moved from `apps/cantos` to `apps/cantos-theatre` by
[decision 0015](../decisions/0015-api-surface-boundaries.md); `cargo xtask` still accepts `cantos`
as an alias of `cantos-theatre`.

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
  API is split by surface into three route groups with separate routers, DTO modules, authentication
  and data access: `/api/v1/studio/*` (implemented), `/api/v1/theatre/*` and `/api/v1/admin/*`
  (reserved: empty routers answering `404 not_found`, no fake data; see [decision 0015](../decisions/0015-api-surface-boundaries.md)).
  Domain rules and PostgreSQL authority stay shared; a surface never calls another surface's
  handler. Backend no longer starts TTS workers. Current contexts are Editorial, Ingestion,
  Adaptation, reference-only Production, pure Media inspection and the internal Catalog
  API; external audio intake and publication remain planned ([API contexts](api-contexts.md)).
  Script IR remains the shared kernel; HTTP surfaces stay separate from these contexts.
- **Kanidm is configuration, not code.** `services/kanidm/` holds deployment documentation and an
  example server config. The API will validate tokens issued by it once identity work starts;
  nothing in this repository reimplements an identity provider.

## What is shared, and what is not

| Shared | Where | Consumers today |
| --- | --- | --- |
| Wire DTOs, JSON Schemas, serialized fixtures, MessagePack goldens, the JSON/MessagePack byte codec, contract documents | `packages/contracts` | `cantos-api`, `cantos-studio` |
| Semantic design tokens and Leptos primitives | `packages/ui-system` | Studio shell/editor/casting presentation; Theatre Web mock; development gallery |

Not shared, deliberately:

- **Script IR rules, revisions and production instructions** remain modules of `cantos-api`
  (`script_ir` and the domain modules under `contexts/`). The Studio editor validates through
  the API. There is **no** synthesis worker or provider client to share that package.
  Domain code stays free of HTTP, UI and provider runtime types.
- **UI renderers.** Leptos and Compose evolve separately. Web apps may share renderer primitives
  from `packages/ui-system/leptos`, while application patterns/routes/state stay app-owned.
  [UI system](../design/ui-system.md) owns the per-surface intensity: Theatre calm and media-first,
  Studio denser and task-first. Both products use one canonical palette/semantic contract;
  density and layout vary by task. No separate app token copy or Kotlin runtime is introduced.
- **Kotlin code.** Theatre is the only mobile product. The shared `packages/ui-system/cmp`
  Gradle package and its Android preview host are component diagnostics, not Studio Mobile.
  Shared contracts and semantic tokens may serve Theatre Mobile and Studio Web; extract further
  libraries only for concrete consumers.

## Cargo workspace

The root `Cargo.toml` lists six members and one `Cargo.lock`. Each builds on its own.

| Member | Package | Build target |
| --- | --- | --- |
| `services/cantos-api` | `cantos-api` | native; `cantos-server`, `cantos-migrate`, `cantos-operator`, validation/diff and read-only WAV inspection CLIs |
| `apps/cantos-studio/web` | `cantos-studio` | `wasm32-unknown-unknown` through Trunk |
| `apps/cantos-theatre/web` | `cantos-theatre` | `wasm32-unknown-unknown` through Trunk |
| `packages/contracts/api` | `cantos-contracts` | native and WASM |
| `packages/ui-system/leptos` | `cantos-ui` | native string-render checks and WASM CSR |
| `xtask` | `xtask` | native developer tool; never a dependency of another member |

Package names follow their directories, except `cantos-contracts`, which was renamed from
`cantos-api` so that two packages do not share a name. Binary names did not change.

## Build and verify per target

[`cargo xtask`](../../xtask/README.md) is the single entrypoint; it maps each target to Cargo or
Trunk and refuses reserved surfaces instead of simulating them.

```sh
cargo xtask build cantos-api           # service binaries (native)
cargo xtask build cantos-studio --web  # Studio Web through Trunk
cargo xtask build cantos-theatre --web         # Theatre Web session-only mock through Trunk
cargo xtask build cantos-theatre --mobile      # UNSUPPORTED: production native listening absent
cargo xtask theatre-demo build                 # shared mock + JVM diagnostic host
cargo xtask theatre-demo android-build         # mock debug APK; existing Android SDK required
cargo xtask verify rust                # workspace fmt, clippy and tests
cargo xtask verify web                 # WASM clippy and Trunk builds of both clients
```

Kanidm has no build or run command: see services/kanidm/README.md for its example config.

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
| `apps/mobile/` | `apps/cantos-theatre/mobile/` |
| `contracts/` | `packages/contracts/` |
| `contracts/design/studio-tokens-0.1.0.json` | `packages/ui-system/shared/legacy/studio-tokens-0.1.0.json` (via `packages/design-tokens`) |
| package `cantos-server` | package `cantos-api` (binary `cantos-server` unchanged) |
| package `cantos-api` | package `cantos-contracts` |

The subsequent [UI foundation migration](ui-system-migration.md) renames the token package and
adds renderer primitives; the restructure itself did not implement these components.
