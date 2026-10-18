# Cantos edge assets

The [Caddyfile](Caddyfile) is tracked routing policy for `cantos-edge` on the VPS. Current
production topology and all operator steps live in the [production runbook](../../docs/deployment/vps-production.md).
[ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md) selects direct Firebase Hosting
and API ingress. This source still includes the transitional ADR 0039 Web/identity proxies;
editing it neither deploys it nor qualifies the minimal Kanidm gateway against a real issuer.

The API host routes Studio paths to `cantos-api-studio` and Theatre paths to
`cantos-api-theatre`; each process pins its own Web origin/CORS surface. The edge adds no
application authorization. Preserve replaced `X-Forwarded-For`, dropped `Forwarded`/`X-Real-IP`,
registration body limits and first-response email-code protections. These protections on a
legacy Web proxy do not automatically apply to direct Firebase Hosting.

Current source admits only named Kanidm browser paths/methods and static assets, ending in
an edge404 for everything else. Management and OAuth back-channel routes require explicitly
inventoried TCP-peer CIDRs; forwarding headers cannot grant private access. The
[identity gateway qualification boundary](identity-gateway.md) records pinned upstream owners,
local Caddy routing tests and remaining live issuer/browser acceptance. Source support does not
identify or qualify the installed policy; no identity-domain migration or credential is created.

[`duck.sh.example`](duck.sh.example) is an operator-owned DNS updater primitive. Keep its token
outside Git under root-only permissions; the token must belong to the named DuckDNS account.
A failed DNS lookup is not authorization to change records or credentials. Caddy validates and
reloads only through an explicitly authorized VPS operation. Preserve `cantos-edge-data` and
`cantos-edge-config`; certificate/ACME state is persistent data, not disposable build output.

Transitional static upstreams keep Theatre and Studio on distinct verified Firebase Site IDs.
Follow the [controlled site transition](../../docs/deployment/firebase-hosting.md#shorter-site-names-controlled-transition-237)
before changing their environment; an edge reload alone does not pick up EnvironmentFile changes.
This does not change ADR 0041 or establish authenticated sessions on cross-site domains.
