# Cantos edge assets

The [Caddyfile](Caddyfile) is tracked routing policy for `cantos-edge` on the VPS. Current
production topology and all operator steps live in the [production runbook](../../docs/deployment/vps-production.md).
[ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md) selects direct Firebase Hosting
and API ingress. This source still includes the transitional ADR 0039 Web/identity proxies;
editing it neither deploys it nor qualifies the pending minimal Kanidm gateway.

The API host routes Studio paths to `cantos-api-studio` and Theatre paths to
`cantos-api-theatre`; each process pins its own Web origin/CORS surface. The edge adds no
application authorization. Preserve replaced `X-Forwarded-For`, dropped `Forwarded`/`X-Real-IP`,
registration body limits and first-response email-code protections. These protections on a
legacy Web proxy do not automatically apply to direct Firebase Hosting.

Current source denies listed Kanidm management prefixes to public peers, including the separate
HTML and HTMX administration trees `/ui/admin` and `/ui/api/admin` in pinned Kanidm 1.10.5.
This is interim hardening, not a default-deny browser-route allowlist and not evidence about the
installed file. The [identity gateway qualification boundary](identity-gateway.md) records pinned
upstream route owners and the remaining acceptance. Qualify it before replacing the public
identity proxy; never expose management operations as a workaround for OIDC.

[`duck.sh.example`](duck.sh.example) is an operator-owned DNS updater primitive. Keep its token
outside Git under root-only permissions; the token must belong to the named DuckDNS account.
A failed DNS lookup is not authorization to change records or credentials. Caddy validates and
reloads only through an explicitly authorized VPS operation. Preserve `cantos-edge-data` and
`cantos-edge-config`; certificate/ACME state is persistent data, not disposable build output.

Transitional static upstreams keep Theatre and Studio on distinct verified Firebase Site IDs.
Follow the [controlled site transition](../../docs/deployment/firebase-hosting.md#shorter-site-names-controlled-transition-237)
before changing their environment; an edge reload alone does not pick up EnvironmentFile changes.
This does not change ADR 0041 or establish authenticated sessions on cross-site domains.
