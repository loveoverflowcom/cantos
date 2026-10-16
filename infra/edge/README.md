# Edge: DuckDNS and Caddy for the Cantos VPS

Operator-owned configuration for the public names under **`cantoss.duckdns.org`**. It is not a
`cargo xtask services` process and is not in [the manifest](../../services/manifest.toml): xtask never
starts it. The [Caddyfile](Caddyfile) is the single routing source.

| Host | Serves | Source |
| --- | --- | --- |
| `cantoss.duckdns.org` | Cantos Theatre Web (listeners; the default site) | `apps/cantos-theatre/web` Trunk build |
| `studio.cantoss.duckdns.org` | Cantos Studio Web (creators) | `apps/cantos-studio/web` Trunk build |
| `api.cantoss.duckdns.org` | Cantos API for native apps and external producers | `/api/*` to `cantos-api:8080` |

Both web apps call the API **same-origin** under `/api/` (see their `Trunk.toml` proxies), so each web
host proxies its own `/api/*`; no CORS is configured. The three hosts stay separate origins, so
Theatre and Studio share no cookie, permission or UI. The API enforces the
[surface boundaries](../../docs/decisions/0015-api-surface-boundaries.md) per route group; the edge
forwards `/api/*` unchanged and adds no authorization.

A static-only alternative for **previews** is Firebase Hosting, deployed by the explicit, manual
`cargo xtask deploy firebase` ([runbook](../../docs/deployment/firebase-hosting.md)). It serves no
API, does not replace this same-origin edge for authenticated production, and changes nothing here
until a separately reviewed origin and authentication cutover exists.

**The API is not deployed.** `cantos-server` supports `CANTOS_ENV=development`, a loopback bind and
local PostgreSQL only ([Studio OIDC boundary](../../docs/architecture/studio-oidc.md)). Every `/api/*`
route therefore answers `503 {"error":"api_unavailable"}` because nothing owns the network alias
`cantos-api`. Never publish the development host to take its place. When a reviewed production host
exists, attach it to the `cantos-edge` network with `--network-alias cantos-api` listening on 8080;
the edge needs no change. The static web apps load without it and show their unavailable states
(the apps have no mock fallback).

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

Build optimized bundles through xtask (Trunk 0.21.14 and the `wasm32` target; debug builds are
several times larger) and copy the `dist/` trees. Do not build on the 2 GB VPS.

```sh
cargo xtask build cantos-theatre --web --release
cargo xtask build cantos-studio --web --release
rsync -a --delete apps/cantos-theatre/web/dist/ root@HOST:/root/cantos-services/edge/www/theatre/
rsync -a --delete apps/cantos-studio/web/dist/  root@HOST:/root/cantos-services/edge/www/studio/
```

Trunk hashes JS/CSS/WASM names, so the Caddyfile caches those immutably and revalidates
`index.html` and unhashed files; a new release needs no cache purge or Caddy restart.
`--delete` also removes the previous release's hashed files. Never copy private data, secrets or
anything but `dist/` into `www/`.

## Run the edge

Ports 80 and 443 must be reachable (`ufw allow 80,443/tcp`); the operator makes that host firewall
change. Podman is the runtime ([ADR 0021](../../docs/decisions/0021-podman-service-runtime.md)).

```sh
install -d -m 700 /root/cantos-services/edge/www/theatre /root/cantos-services/edge/www/studio
cp infra/edge/Caddyfile /root/cantos-services/edge/Caddyfile
podman network create cantos-edge
podman volume create cantos-edge-data
podman run -d --name cantos-edge --restart=always --network cantos-edge \
  -p 80:80 -p 443:443 \
  -v /root/cantos-services/edge/Caddyfile:/etc/caddy/Caddyfile:ro \
  -v /root/cantos-services/edge/www/theatre:/srv/theatre:ro \
  -v /root/cantos-services/edge/www/studio:/srv/studio:ro \
  -v cantos-edge-data:/data \
  docker.io/library/caddy:2.11.7-alpine
```

Keep `cantos-edge-data`: it holds the issued certificates and ACME account, and losing it risks
Let's Encrypt rate limits. Reload after editing the Caddyfile with
`podman exec cantos-edge caddy reload --config /etc/caddy/Caddyfile`. `podman restart` is safe; do
not remove the volume. `--restart=always` needs `podman-restart.service` enabled on the host to
survive a reboot.

Checks: `podman logs cantos-edge`, `curl -I https://cantoss.duckdns.org/` (200, valid certificate),
the same for `studio.`, and `curl -i https://api.cantoss.duckdns.org/api/v1/theatre/x` (503 JSON
until an API is attached).

## Evidence boundary

The Caddyfile was validated with the pinned image (`caddy validate`) and its routing was exercised
against a local plain-HTTP copy with stub static trees: SPA fallback, `application/wasm`,
immutable versus revalidating cache headers, and `503` for an absent upstream on all three hosts.
Certificate issuance, internet reachability, the real Trunk bundles and routing to a real API
are **not** observed until the steps above run on the host.
