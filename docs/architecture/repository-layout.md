# Repository layout

> Active scope: [ADR 0018](../decisions/0018-external-audio-production.md).
> Cantos manages content, external-producer instructions and uploaded audio. TTS workers,
> synthesis job CLIs and backend mixers are removed; details below describing the original
> repository restructure are historical unless marked current.

The original application layout was implemented by
[decision 0009](../decisions/0009-monorepo-layout.md); its
[restructure evidence](../evidence/monorepo-restructure.md) records the original checks.
[#212](https://github.com/loveoverflowcom/cantos/issues/212) separates existing edge and Firebase
Hosting assets into `infra/` without changing application behavior, migrations or wire contracts.
The current map below owns paths; historical decisions/evidence retain their source-bound scope.

## Platform scope

| Product | Web | Mobile |
| --- | --- | --- |
| Cantos Studio | Supported; sole deployment target | Out of scope / not implemented |
| Cantos Theatre (listeners see **Cantos**) | Supported | Supported; Android/iOS with native playback |

Cantos Studio is web-only. Do not implement, scaffold, build or maintain a mobile client unless explicitly requested in a future roadmap.

Support here defines product scope, not implementation evidence. Theatre Web defaults to an
anonymous public discovery Home, published-unit playback and separate account/auth/Library
Save, progress, history and resume synchronization, with an explicit session-only UI mock at
`?mode=mock`. Real issuer, multi-device and deployed playback/delivery acceptance remain separate. See the
[Web implementation boundary](../../apps/cantos-theatre/web/README.md#explicit-live-and-mock-modes).
Theatre Mobile has an isolated [CMP mock](../../apps/cantos-theatre/mobile/README.md): its
production-safe `:shared` KMP module contains the UI/domain/application and ports, and the
separate JVM-only `:preview-sandbox` module contains mock external ports and Desktop Preview composition.
A production Android host composes `:shared` only. Its debug build browses the anonymous public
catalog through a production HTTP adapter when an API origin is configured (`cargo xtask
theatre-host android-debug`); release accepts an explicit HTTPS origin, and its release graph
and unsigned APK/AAB exclusion are inspected by `cargo xtask theatre-host android-release`.
The separate iOS `Cantos` project/scheme links the `CantosTheatre` production framework from
`:shared`, with no sandbox dependency.
Native playback and Library owners have source implementations; device/runtime acceptance
is separate in the [mobile guide](../../apps/cantos-theatre/mobile/README.md). Studio Mobile is only a scope notice: never a workspace
member, Gradle module, xtask target, CI lane or automatic build/test/preview consumer. Shared
contracts, tokens and libraries may serve Theatre Mobile and Studio Web. Historical decisions
and evidence retain their original scope; this matrix supersedes the Studio Mobile reservation
in [decision 0009](../decisions/0009-monorepo-layout.md).

## Map

```text
apps/
  cantos-theatre/          Cantos Theatre: the listener product (user-facing name: Cantos)
    web/                   Leptos CSR discovery/playback/Library (package cantos-theatre) LIVE CLIENT + MOCK
    mobile/                Kotlin Compose Multiplatform     (production adapters + isolated demo; acceptance pending)
      shared/              Production-safe UI/domain/application, ports and non-mock adapters
      preview-sandbox/     JVM-only Desktop Preview; shared TheatreShell and mock external ports
      androidApp/          The one Android application: :shared only RELEASE BOUNDARY; release com.cantos.theatre
                           "Cantos", debug com.cantos.theatre.dev "Cantos Dev"; no flavors, no mock variant
  cantos-studio/           Cantos Studio: the creator product
    web/                   Leptos CSR script editor         (package cantos-studio)       IMPLEMENTED
    mobile/                Scope notice only                (no implementation)          OUT OF SCOPE
services/
  cantos-forge-functions/  Producer core/local CLI outside the API (package cantos-forge) FOUNDATION
  cantos-server/           Rust + Axum + PostgreSQL modular monolith (package cantos-server)  IMPLEMENTED
  manifest.toml            Typed native/Podman/external process inventory                  CONFIGURED
  postgresql/              Official PostgreSQL17 Podman configuration; no crate            CONFIGURED
  kanidm/                  Operator-owned Podman identity configuration; no crate           CONFIG ONLY
infra/                     Deployment/runtime assets; no application domain logic
  vps/                     Production VPS image, Quadlet units, grants, host script         DEPLOYED
  edge/                    Public HTTPS edge (Caddy) and host DNS updater assets            DEPLOYED
  firebase/
    hosting/               Static Web Hosting policy and operator site-mapping example     CONFIG ONLY
    app-distribution/      Existing CLI-only operator inputs; no new config format         DOCUMENTED
packages/
  contracts/               Script IR/Studio/production schemas, fixtures, contract docs and
    api/                   the shared wire DTO crate        (package cantos-contracts)    IMPLEMENTED
  web-auth/                Shared RAM Bearer lifecycle + browser adapter for both Web apps IMPLEMENTED
  ui-system/               Shared canonical tokens/spec, licensed icons and renderer boundary
    leptos/                Web foundation                (package cantos-ui)             IMPLEMENTED
    cmp/                   Shared native controls and JVM desktop diagnostics              PREVIEW
xtask/                     cargo xtask: command orchestration (package xtask), not deployment assets
.cargo/                    the `cargo xtask` alias
docs/  templates/  scripts/  .agents/  .github/             unchanged roles; scripts/ are xtask primitives
```

- **IMPLEMENTED** has code that existed before the restructure and kept its tests.
- **LIVE CLIENT** is Theatre Web's anonymous published-work discovery/playback and scoped
  account/Library Save, progress, history and resume boundary. This is implemented client code;
  real issuer, multi-device and deployed delivery/playback acceptance remain separate. Home stays accessible without authentication; Account remains an avatar entry and
  the [two primary destinations](../../apps/cantos-theatre/web/README.md#scope-and-honest-behavior)
  are preserved.
- **MOCK** is the session-only Theatre Web mock: synthetic catalog, simulated player and in-memory
  library/queue, with evidence in its [record](../evidence/theatre-web-mock.md). It has no listener
  API, audio, account or persistence and is not evidence for any Theatre work item.
- **DEMO** is the isolated JVM Desktop Preview: it mounts the shared `TheatreShell` with mock
  external ports and the real Library store on in-memory SQLite. Android/iOS have no mock host.
  Its [current record](../evidence/theatre-desktop-preview-shared-shell-287.md) is separate from
  [historical demo evidence](../../apps/cantos-theatre/mobile/docs/mock-skeleton.md) and native acceptance.
  The [sandbox extraction record](../evidence/issue122-cmp-sandbox-isolation.md) distinguishes
  module/source checks from the production host's release artifact checks; a demo build never
  stands for them.
- **RELEASE BOUNDARY** is the production Android host (`com.cantos.theatre`): a real application
  module that composes `:shared` only and browses the real catalog when an API origin is configured.
  Its unsigned release graph and APK/AAB are inspected for sandbox exclusion; catalog integration
  does not establish native listening, a signed release or distribution.
- **OUT OF SCOPE** is a policy notice, not a reserved client or a future automatic build target.
- **CONFIG ONLY** documents and validates deployment configuration, not live deployment or
  integration acceptance. For Kanidm, the API has opt-in [Studio OIDC](studio-oidc.md) and separate
  [Theatre OIDC](theatre-identity.md) adapters. [Local Theatre sign-in/logout](../evidence/theatre-live-auth-206.md)
  has real issuer evidence; Firebase signup, Studio live acceptance and production activation
  retain their own gates.
- **FOUNDATION** is the [Cantos Forge](../../services/cantos-forge-functions/README.md) crate governed by
  accepted [decision 0022](../decisions/0022-cantos-forge-producer-cli.md): its pure, tested domain core,
  a filesystem workspace and a host-assisted local workflow (`workspace init`, `artifact add`,
  `artifact list/show`, `validate`, `handoff export`), plus attempt/recovery core, local durable
  attempt store, finite Rust worker and Node process boundary. The earlier mock provider/core
  [evidence](../evidence/forge-attempt-verification.md) retains its original scope; new boundary
  and remaining acceptance are in the [worker runbook](../deployment/forge-worker.md). It has no real provider,
  network client or API handoff, and it is not a Studio or Theatre client.
  The owner-selected [local/Functions direction](../decisions/0022-cantos-forge-producer-cli.md)
  lets Studio request operations while the same Rust producer core executes outside the API;
  private Drive app-data is planned for remote drafts. #203 relocates the existing crate to
  `services/cantos-forge-functions`; the package and local CLI remain `cantos-forge`. Source
  relocation establishes no provider, credential, Drive, runtime packaging or deployment
  acceptance.
- **PREVIEW** is the separately requested [two-renderer catalogue](../../packages/ui-system/README.md#two-diagnostic-apps).
  It owns component/token diagnostics, not a Theatre/Studio mobile app or native listening.

## Naming: technical identifiers and user-facing names

| Context | Name |
| --- | --- |
| Listener app path, package and Trunk binary | `cantos-theatre` (`apps/cantos-theatre/{web,mobile}`) |
| Listener API module, route group and DTOs | `theatre` (`/api/v1/theatre`, `cantos_contracts::theatre`) |
| Creator app path, package, API route group | `cantos-studio`, `studio` (`/api/v1/studio`) |
| Operations API module and route group | `admin` (`/api/v1/admin`) |
| Shared backend | `cantos-server` |
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
apps/*  ──HTTP──▶  services/cantos-server
apps/*  ──code/data──▶  packages/*          services/*  ──code/data──▶  packages/*
```

- `packages/` imports neither `apps/` nor `services/`. An app never imports another app, and an app
  never links `services/` code; the browser or phone talks to the API over HTTP, so no server crate
  enters a WASM dependency graph. Cantos Forge follows the same rule: a future handoff uses HTTP
  and producer credentials, and the API never spawns or waits for Forge. `cargo test -p xtask`
  checks that every workspace dependency edge points inward.
- **Studio and Theatre stay separate.** Each has its own UI, routes, session and permission
  context. A listener request must never reach a Studio route and a Studio screen must not ship in
  the Theatre bundle. Enforcement stays in the backend: private drafts, voice assets and previews
  remain unavailable to public delivery whichever client asks.
- **One authoritative modular backend.** `services/cantos-server` owns authoritative metadata and
  product APIs for both products, as decided in
  [decision 0001](../decisions/0001-modular-monolith.md). This is not a claim that Cantos can have
  only one runnable process: infrastructure and the planned Forge producer remain separate
  owners, without duplicating Catalog, rights, review or publication authority. Studio v1 is
  implemented. The API is split by surface into three route groups with separate routers, DTO modules, authentication
  and data access: `/api/v1/studio/*` (implemented), `/api/v1/theatre/*` (public Discovery/Playback
  core; actual delivery/identity/client acceptance pending) and `/api/v1/admin/*`
  (opt-in [Admin capability/revocation and audit](admin-authorization.md); live provider acceptance pending; see [decision 0015](../decisions/0015-api-surface-boundaries.md)).
  Domain rules and PostgreSQL authority stay shared; a surface never calls another surface's
  handler. Backend no longer starts TTS workers. Current contexts are Editorial, Ingestion,
  Adaptation, reference-only Production, pure Media inspection and the internal Catalog
  API, plus immutable Production Brief export and scoped development read grants; external
  audio intake and publication remain planned ([API contexts](api-contexts.md)).
  Script IR remains the shared kernel; HTTP surfaces stay separate from these contexts.
- **Firebase Hosting is configuration, not code.** `infra/firebase/hosting/` holds the tracked
  Hosting policy and a mapping example for the explicit, manual
  `cargo xtask deploy firebase` ([runbook](../deployment/firebase-hosting.md)). It serves only each
  Web app's static bundle: the API, PostgreSQL, Kanidm and storage stay outside it. The VPS
  [edge](../../infra/edge/README.md) serves those bundles and the API on one origin per app
  ([ADR 0039](../decisions/0039-vps-production-runtime.md)).
- **Infrastructure and command orchestration are separate.** [`infra/`](../../infra/README.md)
  owns deployment/runtime assets, including the edge configuration and Firebase Hosting policy.
  `xtask/` owns typed command planning, validation and explicit execution, not a second copy of
  those assets. The current service inventory and PostgreSQL/Kanidm configuration remain under
  `services/`; this mechanical boundary does not move application or producer code, provision a
  runtime or establish deployment acceptance. Add a Functions directory only with a concrete
  consumer and reviewed runtime decision under #203.
- **Kanidm is configuration, not code.** `services/kanidm/` holds deployment documentation and an
  example server config. The API implements a separate opt-in [Studio OIDC verifier](studio-oidc.md);
  nothing in this repository reimplements an identity provider.
  The [internal actor registry](identity-registry.md) now owns exact external bindings under
  `platform/identity`, preserving existing actors and development sessions; it does not verify
  provider claims, add OIDC login or grant Theatre/Admin authority.

## What is shared, and what is not

| Shared | Where | Consumers today |
| --- | --- | --- |
| Wire DTOs, JSON Schemas, serialized fixtures, MessagePack goldens, the JSON/MessagePack byte codec, contract documents | `packages/contracts` | `cantos-server`, `cantos-studio`, `cantos-theatre` |
| RAM credential lifecycle and browser authorization adapter | `packages/web-auth` | Studio Web and Theatre Web; separate purposes/instances |
| Semantic design tokens and Leptos primitives | `packages/ui-system` | Studio shell/editor/casting presentation; Theatre Web live auth/account and isolated mock; development gallery |

Not shared, deliberately:

- **Script IR rules, revisions and production instructions** remain modules of `cantos-server`
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
  Within Theatre Mobile, `:preview-sandbox` depends on `:shared`; only the JVM Desktop Preview selects both
  explicitly. Android and iOS have no mock host. `:shared` has no reverse sandbox dependency or implicit mock fallback. The
  one Android application `:androidApp` consumes `:shared` and its injected production adapters
  only, in both build types. Static source/build guards protect the declared boundary; its resolved
  debug and release classpaths and actual APK/AAB contents are inspected independently by `theatre-host`.

## Cargo workspace

The root `Cargo.toml` lists seven members and one `Cargo.lock`. Each builds on its own.

| Member | Package | Build target |
| --- | --- | --- |
| `services/cantos-server` | `cantos-server` | native; `cantos-server`, `cantos-migrate`, `cantos-operator`, validation/diff and read-only WAV inspection CLIs |
| `services/cantos-forge-functions` | `cantos-forge` | native producer-side CLI `cantos-forge`; never a dependency of another member |
| `apps/cantos-studio/web` | `cantos-studio` | `wasm32-unknown-unknown` through Trunk |
| `apps/cantos-theatre/web` | `cantos-theatre` | `wasm32-unknown-unknown` through Trunk |
| `packages/contracts/api` | `cantos-contracts` | native and WASM |
| `packages/web-auth` | `cantos-web-auth` | native core tests and WASM browser adapter |
| `packages/ui-system/leptos` | `cantos-ui` | native string-render checks and WASM CSR |
| `xtask` | `xtask` | native developer tool; never a dependency of another member |

Package names follow their directories, except `cantos-contracts`, which was renamed from
`cantos-api` so that two packages do not share a name. Binary names did not change.

## Build and verify per target

[`cargo xtask`](../../xtask/README.md) is the single entrypoint; it maps each target to Cargo or
Trunk and refuses reserved surfaces instead of simulating them.

```sh
cargo xtask build cantos-server           # service binaries (native)
cargo xtask build cantos-forge         # producer-side Forge CLI (native); built, never run by xtask
cargo xtask build cantos-studio --web  # Studio Web through Trunk
cargo xtask build cantos-theatre --web         # Theatre Web live boundary and isolated mock through Trunk
cargo xtask build cantos-theatre --mobile      # UNSUPPORTED aggregate command; use explicit theatre-host targets
cargo xtask theatre-preview build                 # shared core + preview-sandbox JVM diagnostic host
cargo xtask theatre-host android-debug --api-origin ORIGIN  # production debug APK; existing Android SDK required
cargo xtask distribute firebase cantos-theatre --channel development --api-origin https://api.example.org --groups g --dry-run # plan real debug APK distribution
cargo xtask distribute firebase cantos-theatre --channel production --api-origin https://api.example.org --groups g --dry-run # plan optimized release; operator signing required
cargo xtask deploy firebase cantos-theatre --web --project ID --channel review --dry-run  # opt-in static Web deploy to Firebase Hosting; plan only with --dry-run
cargo xtask verify rust                # workspace fmt, clippy and tests
cargo xtask verify web                 # WASM clippy and Trunk builds of both clients
```

PostgreSQL and optional Kanidm use explicit `cargo xtask services` lifecycle commands, separate
from native `run` and disposable database verification. See [services](../../services/README.md).
Configured infrastructure is not a deployed identity provider. The opt-in Studio OIDC code has
separate mocked/SQL evidence and unrun live-provider gates.

`cantos-server` serves `apps/cantos-studio/web/dist` by default for local development, through
`CANTOS_WEB_DIST`. This is a development convenience for same-origin cookies, not a statement that
the API owns the Studio frontend; Theatre Web will need its own origin or static host.

## Moved paths

Evidence and decision records written before each relevant move cite the old paths on purpose,
because they describe what was run at the time. Their links point to the current files.

| Before | After |
| --- | --- |
| `services/cantos-api/src/interfaces/http/` | `services/cantos-api/src/adapters/inbound/http/` (#327; Rust `adapters::inbound::http`, crate-root `http` alias retained) |
| `services/cantos-api/src/composition.rs` | `services/cantos-api/src/application.rs` (#327 concrete application handle; HTTP outer assembly is separate) |
| `services/edge/` | `infra/edge/` (#212 deployment assets only) |
| `services/firebase-hosting/` | `infra/firebase/hosting/` (#212 deployment assets only) |
| `apps/server/` | `services/cantos-api/` |
| `apps/web/` | `apps/cantos-studio/web/` |
| `apps/mobile/` | `apps/cantos-theatre/mobile/` |
| `contracts/` | `packages/contracts/` |
| `contracts/design/studio-tokens-0.1.0.json` | `packages/ui-system/shared/legacy/studio-tokens-0.1.0.json` (via `packages/design-tokens`) |
| package `cantos-server` | package `cantos-api` (binary `cantos-server` unchanged) |
| package `cantos-api` | package `cantos-contracts` |
| `services/cantos-api/` (package `cantos-api`) | `services/cantos-server/` (package `cantos-server`; #332, checkpoint 1/5 of #331) — the binary, routes and wire contracts are unchanged; see the [rename inventory](../../services/cantos-server/README.md#rename-from-cantos-api) for what intentionally keeps the old name |

The subsequent [UI foundation migration](ui-system-migration.md) renames the token package and
adds renderer primitives; the restructure itself did not implement these components.

The [step-2 native runtime record](../evidence/theatre-native-step2.md) supersedes earlier source-only statements that native player/account/Library/store adapters are absent. Device/iOS/codec/identity/cross-client acceptance remains pending; the isolated demo still establishes none of those capabilities.
