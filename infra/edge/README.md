# Edge: DuckDNS and Caddy for the Cantos VPS

Configuration for the public names under **`cantoss.duckdns.org`**. The [Caddyfile](Caddyfile) is
the single routing source; the production runtime runs it as the Quadlet unit `cantos-edge`,
installed and started by `cantos-vps` ([ADR 0039](../../docs/decisions/0039-vps-production-runtime.md),
[runbook](../../docs/deployment/vps-production.md)). It is not a `cargo xtask services` process.

| Host | Serves | Upstreams |
| --- | --- | --- |
| `cantoss.duckdns.org` | Cantos Theatre Web (listeners) | `/api/*` → `cantos-api-theatre:8080`; pages → Firebase site |
| `studio.cantoss.duckdns.org` | Cantos Studio Web (creators) | `/api/*` → `cantos-api-studio:8080`; pages → Firebase site |
| `api.cantoss.duckdns.org` | Cantos API for native apps and external producers | `/api/*` → `cantos-api-theatre:8080` |
| `idm.cantoss.duckdns.org` | Kanidm sign-in and OIDC | `https://cantos-kanidm:8443` (private-CA TLS) |

Host names and Firebase site IDs come from `/root/cantos-services/config/edge.env`
(`{$VAR}` placeholders), so the file names no project. Both Web apps call the API
**same-origin** under `/api/`: the edge serves each app's pages from its Firebase Hosting site and
its `/api/*` from the API process pinned to that origin. No CORS is configured and Theatre and
Studio share no host, cookie, API process or permission. The API enforces the
[surface boundaries](../../docs/decisions/0015-api-surface-boundaries.md); the edge adds no
authorization. An absent or unhealthy API answers `503 {"error":"api_unavailable"}`.

## DNS and updater

DuckDNS resolves every subdomain of `cantoss` to the same address, so one record covers all three
hosts. The DuckDNS account that owns `cantoss` must supply the token; a token from another account
returns `KO`.

1. Put the token in `/root/cantos-services/duckdns/cantoss.token` (root, mode 0600). Never commit it
   or put it on a command line that is logged.
2. Install [duck.sh.example](duck.sh.example) as `duck-cantoss.sh` (mode 0700) and add
   `*/30 * * * * /root/cantos-services/duckdns/duck-cantoss.sh >/dev/null 2>&1`.
3. Run it once. `duck-cantoss.log` reads `OK` on success, `KO` for a wrong token/domain and
   `NO_TOKEN` when the token file is empty. Then `getent hosts cantoss.duckdns.org
   studio.cantoss.duckdns.org api.cantoss.duckdns.org` must show the VPS address.

## Publish the web apps

The bundles are released to Firebase Hosting with `cargo xtask deploy firebase`
([Hosting runbook](../../docs/deployment/firebase-hosting.md)); the edge needs no change or
restart. The edge forwards Firebase's `Cache-Control: no-cache`, drops its `Alt-Svc`/HSTS for
`web.app` and adds this host's HSTS and default security headers (`?` defaults never override
the API's stricter `Referrer-Policy`). Studio additionally sends `X-Frame-Options: DENY`.

## Identity, forwarding and email actions

- `X-Forwarded-For` to the APIs is *set* to the TCP peer Caddy saw; `Forwarded`/`X-Real-IP` are
  dropped. The API honors it only for a configured trusted-proxy network
  ([runbook](../../docs/deployment/vps-production.md#theatre-registration-behind-the-edge-not-enabled)).
- The identity host proxies Kanidm's browser surface publicly and answers 404 for its management
  REST API unless the TCP peer is private (`CANTOS_INTERNAL_CIDRS` overrides the default set).
- A request carrying `oobCode` gets `no-store` and `no-referrer`, and the code is stripped from
  the upstream URL. Registration bodies are capped at 16 KB before reaching the API.

## Operate

`cantos-vps install` validates and reloads a changed Caddyfile in place
(`caddy validate` then `caddy reload`). Keep `cantos-edge-data`: it holds the certificates and
ACME account, and losing it risks Let's Encrypt rate limits. Ports 80 and 443 (TCP, and UDP for
HTTP/3) are the only published ports; `cantos-vps bootstrap` opens them in ufw together with
routing and DNS for the `cantos0` bridge, without which Caddy cannot reach ACME or Firebase.

## Evidence boundary

The Caddyfile is validated with the pinned `caddy:2.11.7-alpine` image. The
[deployment record](../../docs/evidence/vps-production-deployment.md) holds the observed
certificate issuance, routing, headers and closed internal ports on the live VPS.
