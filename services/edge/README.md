# Edge: DuckDNS and Caddy for the Cantos VPS

Operator-owned configuration for the public name **`cantoss.duckdns.org`**. It is not a
`cargo xtask services` process and is not in [the manifest](../manifest.toml): xtask never
starts it. It routes to no Cantos service yet; `cantos-api` has no accepted production
deployment, so the Caddyfile answers `503` rather than proxying a development host.

| Piece | Where | Notes |
| --- | --- | --- |
| DNS name | DuckDNS account that owns `cantoss` | A token from another account returns `KO` |
| Updater | `/root/cantos-services/duckdns/` | [duck.sh.example](duck.sh.example), cron every 30 minutes |
| TLS and routing | `cantos-edge` Podman container | [Caddyfile](Caddyfile), automatic Let's Encrypt |

## One-time DNS and updater

1. Put the DuckDNS token in `/root/cantos-services/duckdns/cantoss.token` (root, mode 0600).
   Never commit it or paste it into chat or a command line.
2. Install the script as `duck-cantoss.sh` (mode 0700) and add
   `*/30 * * * * /root/cantos-services/duckdns/duck-cantoss.sh >/dev/null 2>&1`.
3. Run it once. `duck-cantoss.log` reads `OK` on success, `KO` for a wrong token/domain and
   `NO_TOKEN` when the token file is empty. `getent hosts cantoss.duckdns.org` must then show
   the VPS address. A manual IP change on the DuckDNS site is equivalent for a fixed address.

## Edge container

Ports 80 and 443 must be reachable (`ufw allow 80,443/tcp`); this is a deliberate host
firewall change that the operator makes. Podman is the runtime ([ADR 0021](../../docs/decisions/0021-podman-service-runtime.md)).

```sh
install -d -m 700 /root/cantos-services/edge
cp Caddyfile /root/cantos-services/edge/Caddyfile
podman volume create cantos-edge-data
podman run -d --name cantos-edge --restart=always \
  -p 80:80 -p 443:443 \
  -v /root/cantos-services/edge/Caddyfile:/etc/caddy/Caddyfile:ro \
  -v cantos-edge-data:/data \
  docker.io/library/caddy:2.11.7-alpine
```

Keep `cantos-edge-data`: it holds the issued certificate and ACME account, and losing it
risks Let's Encrypt rate limits. Check with `podman logs cantos-edge` and
`curl -i https://cantoss.duckdns.org/` (expects `503` and a valid certificate). Reload after
editing the Caddyfile with `podman exec cantos-edge caddy reload --config /etc/caddy/Caddyfile`.
`podman restart` is safe; do not remove the volume.

## Evidence boundary

This documents the configuration. Certificate issuance, reachability and routing to
`cantos-api` are observed only when the checks above are run on the host.
