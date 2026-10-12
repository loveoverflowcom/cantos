# ADR 0009: Separate listener app, Studio app, services and shared packages

- Status: accepted
- Date: 2026-10-11
- Decision owner: Cantos maintainers
- Related issue or PR: `refactor: reorganize Cantos apps and backend services` (targets `develop`)
- Supersedes / superseded by: refines the source layout in [decision 0001](0001-modular-monolith.md); N/A

> Current scope: the [platform matrix](../architecture/repository-layout.md#platform-scope)
> supersedes this decision's Studio Mobile reservation. Studio is web-only; the historical layout
> and verification record below are retained.

## Context

The repository grew as `apps/server`, `apps/web` and `apps/mobile`, with `contracts/` at the root.
`apps/web` is Cantos Studio, a creator tool, while the listener product (Cantos Theatre) and Studio
mobile had no place to live. `apps/server` is the only backend, `apps/mobile` described only the
listener app, and `contracts/` mixed interchange files, a Rust crate and design tokens. Known: the
Rust/Axum/PostgreSQL, Leptos and Compose Multiplatform choices, one modular backend and native
mobile playback are fixed by decision 0001 and `AGENTS.md`. Assumed: Theatre Web and both mobile
apps will start soon and must not be built inside Studio.

## Decision

Group by product and kind:

- `apps/cantos/{web,mobile}` for Theatre and `apps/cantos-studio/{web,mobile}` for Studio.
- `services/cantos-api` for the one backend, and `services/kanidm` for identity-provider
  deployment and configuration.
- `packages/contracts` and `packages/design-tokens` for what several consumers share.

Keep one Cargo workspace and one `Cargo.lock`. Rename packages to match: `cantos-server` becomes
`cantos-api`, and the wire DTO crate `cantos-api` becomes `cantos-contracts`. Binary names, HTTP
routes, wire formats, migrations and Script IR digests do not change. The move uses `git mv`, so
file history follows. Add only a buildable Theatre Web skeleton and READMEs for the not-yet-built
apps and Kanidm. Record in
[repository layout](../architecture/repository-layout.md) which directories are implemented,
skeletons or reserved.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Keep `apps/{server,web,mobile}` and add `apps/theatre-*` | No moves | Server stays among apps, `web` stays ambiguous, Studio mobile has no home | Rejected: the ambiguity is the problem |
| Separate Cargo workspaces per app | Independent lockfiles | Duplicated pins; shared DTO crate needs publishing or path hacks | Rejected: one lock already builds each member alone |
| Extract Script IR into `packages/` now | Looks like a clean domain package | One consumer; an interface without a reviewed second user | Rejected: extract on a real second consumer |
| Split the API into several services | Mirrors Theatre/Studio | Violates decision 0001 without measured need | Rejected |
| Gradle skeletons for both mobile apps | Looks complete | Cannot be built or verified here; implies Android/iOS support | Rejected: README guides only |
| **Product-grouped apps, one service, shared packages** | Clear ownership and the requested target layout | One-time churn in paths and docs | Selected |

## Consequences

- Product and user impact: none; no behavior, UI or API changes.
- Compatibility and migrations: SQL migrations are byte-identical. The environment variable
  `CANTOS_WEB_DIST` is unchanged but its default is now `apps/cantos-studio/web/dist`. Scripts,
  CI and the commands in `AGENTS.md` use the new package names. Older evidence keeps its original
  paths.
- Operations, storage, and production cost: no new service, process or runtime dependency.
- Implementation and recovery requirements: new apps depend on `packages/` and on the API over
  HTTP, never on another app or on service code. The listener API, when built, is a route group
  and permission context in `cantos-api`.
- Deferred questions: Kanidm integration with the API (no OIDC code exists); a Theatre token set;
  Gradle builds and any shared Kotlin module; whether a worker process justifies extracting the
  Script IR rules.

## Validation and revisit conditions

The checks and their results are in the [restructure evidence](../evidence/monorepo-restructure.md).
Revisit when a second consumer needs Script IR rules, when Theatre and Studio need a shared Kotlin
module, or when measured load or ownership justifies splitting the API.
