# ADR 0039: VPS production runtime and same-origin Firebase Web edge

- Status: accepted runtime foundation; Web/public identity topology superseded by
  [ADR 0041](0041-direct-hosting-api-boundary.md). The original decision below is historical
  rationale, not current deployment or sign-in status.
- Date: 2026-10-17
- Decision owner: Cantos product owner, with the deployment operator
- Related: [ADR 0015](0015-api-surface-boundaries.md), [ADR 0021](0021-podman-service-runtime.md),
  [ADR 0024](0024-studio-oidc-bff.md), [ADR 0034](0034-theatre-signup-and-provisioned-studio.md),
  [Firebase Hosting runbook](../deployment/firebase-hosting.md)
- Runbook: [current production topology and operations](../deployment/vps-production.md)

## Context

The owner asked for Cantos API on one small VPS (Debian 13, 1 vCPU, 2 GB RAM, `cantoss.duckdns.org`
names) and both Leptos Web apps on two Firebase Hosting sites, without GitHub Actions, without
bypassing development-only restrictions and without mock data.

Three facts constrained the design:

1. `cantos-server` refused anything but `CANTOS_ENV=development`, a loopback bind and local
   PostgreSQL. Each process pins exactly **one** Web origin: Origin/CSRF checks, the Studio and
   Theatre OIDC callbacks and cookie security derive from it.
2. Both Web clients call relative `/api/v1/...` with same-origin credentials. The Studio session
   cookie is `__Secure-cantos_studio`, `SameSite=Strict`, host-only, scoped to `/api/v1/studio`;
   OIDC callbacks land on the Web origin (BFF). A cross-site API origin would need a client API
   setting, credentialed CORS and `SameSite=None` cookies subject to third-party-cookie policy.
3. Firebase Hosting rewrites reach Cloud Functions/Cloud Run only, not an external server, and
   DuckDNS resolves every subdomain of `cantoss` to one address, so a Firebase custom domain
   cannot share the registrable site `cantoss.duckdns.org` with the VPS.

## Decision

**Production host mode.** `CANTOS_ENV=production` is a closed, validated mode
(`platform::host`): explicit bind, one exact `https` Web origin (no path, no IP literal), no
static files (API-only router), no development Studio session or token login, no development
audio root and public issuer trust only. PostgreSQL stays local: the existing loopback/Unix-socket
rule is kept and production uses a Unix socket. SIGTERM shuts the server down gracefully.

**Two API processes, one image.** `cantos-api-theatre` pins `https://cantoss.duckdns.org` and
`cantos-api-studio` pins `https://studio.cantoss.duckdns.org`. This keeps the single-origin
security middleware unchanged and separates Studio and Theatre processes (ADR 0015).
`api.cantoss.duckdns.org` routes to the Theatre process for native/producer clients.

**Same-origin edge in front of Firebase.** Caddy on the VPS terminates TLS for the Web hosts,
serves `/api/*` from the matching API process and every other path from that app's Firebase
Hosting site (`https://<site>.web.app`, `Host` rewritten to the upstream). Firebase hosts and
versions the static bytes; browsers see one origin per app, so cookies, OIDC callbacks, CSRF
Origin checks and the existing clients work unchanged, with no CORS and no client API setting.
The raw `*.web.app` URLs remain valid static deployments whose `/api/*` answers 404
(honest unavailable states), never a mock. Firebase never proxies to the VPS.

**Runtime.** Rootful Podman with Quadlet units started at boot (single-tenant root-administered
VPS; ADR 0021's rootless default covers developer machines). PostgreSQL 17 has no TCP listener;
clients use a shared socket volume with SCRAM authentication. Roles: `postgres` (bootstrap,
backup), `cantos_owner` (owns the database, runs migrations), `cantos_app` (runtime, only the
grants in `infra/vps/postgres/runtime-grants.sql`, verified by a PostgreSQL test). Kanidm runs
privately with a host-CA certificate and is published only through the edge at
`idm.cantoss.duckdns.org`; APIs reach it through a private-network alias of the edge. Only the
edge publishes ports (80/443); ufw routes and resolves DNS only for the `cantos0` bridge.
Secrets are generated on the host and handed to containers as Podman secrets.

**Delivery.** `cargo xtask deploy vps cantos-api` builds a digest-pinned image from a
`git archive` of `HEAD` on the developer machine or CI, uploads it with the host bundle and runs
`cantos-vps deploy`: logical backup, migrations as owner, grants, image switch, `Notify=healthy`
gates, a public probe through the edge and automatic switch-back on failure. Rollback, status
and bootstrap are explicit subcommands; CircleCI only offers a manual pipeline that calls them.

## Consequences

- The VPS is in the Web path: Firebase still stores, versions and serves the bundles, but its CDN
  is not the browser's first hop. A second registrable domain would allow Firebase custom
  domains, at the price of the cross-site API design rejected above.
- Studio sign-in requires an operator-run `cantos-vps identity` (OAuth2 clients, one Studio
  administrator, its exact subject roster) and a credential set by the owner through Kanidm's
  reset link. Until then the Studio host reports authentication unavailable.
- Theatre registration, Theatre native identity, Admin and object storage stay unconfigured.
  Only Firebase-mode registration has a reviewed, optional grant block (applied when it is
  enabled); the others' blocks are absent and must be reviewed before enabling them. The edge
  also restricts Kanidm's management API to the private network and sets `X-Forwarded-For`
  for a trusted-proxy peer policy.
- Migrations are forward-only. An automatic rollback switches the image back over an already
  migrated schema; restoring a backup is an explicit, confirmed operator action.
- Changing `idm.cantoss.duckdns.org` after initialization is a Kanidm domain migration.

## Rejected alternatives

- **Cross-site API from `*.web.app` with CORS**: needs `SameSite=None` cookies and an API-origin
  client setting; unreliable under third-party-cookie policy and a security redesign.
- **Firebase custom domains**: impossible under one DuckDNS name; a different site again implies
  the cross-site design.
- **Cloud Run/Functions proxy**: adds a paid service and a new runtime for a pure reverse proxy.
- **Per-surface Web origins inside one process**: would change the shared Origin middleware and
  OIDC composition; two processes need no change to that security code.
- **Building on the VPS**: a 1 vCPU/2 GB host cannot build the workspace in reasonable time.
