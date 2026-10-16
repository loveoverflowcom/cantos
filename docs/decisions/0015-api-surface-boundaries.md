# ADR 0015: Name the listener app `cantos-theatre` and split the API into surface groups

- Status: proposed policy record; the `cantos-theatre` name and separate Studio/Theatre/Admin
  route groups are implemented. Per-surface production acceptance remains separate.
- Date: 2026-10-12
- Decision owner: Cantos maintainers
- Related issue or PR: `refactor: rename Cantos Theatre and separate API boundaries` (targets `develop`)
- Supersedes / superseded by: refines [decision 0009](0009-monorepo-layout.md) (app directory name) and [decision 0001](0001-modular-monolith.md) (internal boundaries); N/A

## Context

Known: the listener app lived at `apps/cantos`, a name that matches the whole product and the
backend (`cantos-api`) and hides which client it is. All implemented HTTP routes were Studio
routes mounted flat at `/api/v1/*`, in one router module with one cookie scope, although the
listener API ([listening](../../.agents/skills/cantos-listening/SKILL.md)) and an operations
surface are planned and must not share permissions with Studio. Decision 0001 requires one
modular monolith. Assumed: listener and administration endpoints will start soon.

## Decision

1. Rename `apps/cantos` to `apps/cantos-theatre` (package `cantos-theatre` is unchanged). The
   `cargo xtask` target becomes `cantos-theatre`; `cantos` stays an accepted alias.
2. Keep one deployable `services/cantos-api` and split its HTTP host into three route groups:
   `/api/v1/studio/*` (implemented), `/api/v1/theatre/*` and `/api/v1/admin/*` (reserved: empty
   routers answering `404 not_found`, no fake or mock data). Each group has its own router module
   (`src/http/{studio,theatre,admin}.rs`), DTO module in `cantos-contracts`, authentication and
   data-access methods; shared code is middleware, error mapping, domain modules and the one
   PostgreSQL authority. Base paths live in `cantos_contracts::routes`.
3. Keep the pre-split `/api/v1/*` Studio paths as a deprecated alias served by the same handlers,
   flagged with `Deprecation: true`. It receives no new routes. A session cookie is scoped to the
   mount that issued it (`Path=/api/v1/studio` or the alias's `Path=/api/v1`), so a Studio
   session issued under the new mount is not sent to Theatre or Admin paths.
4. Technical identifiers use `theatre`/`cantos-theatre`; user-facing text for listeners is
   **Cantos** and for creators **Cantos Studio**. The Theatre Web `<title>` and heading changed
   from "Cantos Theatre" to "Cantos".

Wire DTOs, error codes, migrations, persisted identifiers and Script IR digests do not change.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Separate `theatre-api` and `admin-api` services | Hard isolation | Violates 0001; three deployments before a second consumer exists | Rejected |
| Keep flat `/api/v1/*` and add listener routes beside Studio | No move | One cookie scope, easy to reuse a Studio handler for listeners | Rejected |
| Remove the old paths immediately | No alias to maintain | Breaks existing clients, scripts and evidence harnesses | Rejected; alias is deprecated, not removed |
| Move Studio DTOs into a `studio` module | Symmetric names | Churn in every consumer for no behavior change | Deferred; root holds Studio v1 plus shared types |

## Consequences

- Product and user impact: listeners see **Cantos**; none see the technical name.
- Compatibility and migrations: no migration. The old `/api/v1/*` Studio paths and their cookies
  keep working. Clients in this repository call `/api/v1/studio/*`.
- Operations: cookies issued by the alias still reach every `/api/v1` path; they authenticate
  nothing on Theatre or Admin because those groups serve no routes.
- Implementation requirements: a first Theatre or Admin endpoint needs real behavior, its own
  authentication/authorization and DTOs, and extends `tests/surface_routes.rs`.
- Deferred questions: removal date of the alias; the Theatre and Admin authentication model;
  whether Studio DTOs move to a module; production-grade administrator authorization.

## Validation and revisit conditions

`services/cantos-api/tests/surface_routes.rs` (no database), the PostgreSQL compatibility test in
`revisions_postgres.rs` and `cargo xtask` gates; see [evidence](../evidence/api-surface-split.md).
Revisit when the first listener endpoint is designed or when the alias has no remaining callers.
