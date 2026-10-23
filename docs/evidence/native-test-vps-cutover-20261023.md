# Native test-VPS cutover and Pages publication (23 October 2026)

Source-bound observation of one **owner-authorized test** run. It is not the production runbook
([vps-production.md](../deployment/vps-production.md)), not admission of the image pair and not
browser, device or end-to-end acceptance. An older PASS here is not the status of later code.

## Scope and inputs

| Item | Value |
| --- | --- |
| Source | PR [#409](https://github.com/loveoverflowcom/cantos/pull/409) head `d069c3895d03f47894c8a69a7259c284cc417518` (open, not merged at the time) |
| Producer | `native-oci-publish` workflow, run `39950065009`, `linux/amd64`, unadmitted candidate (`release_eligible: false`) |
| Server image | `ghcr.io/loveoverflowcom/cantos-server@sha256:f8beede6e55d1a0a7d9cec3f71a551cd4d3875a98609a02eeeeae78d6d719c17`, UID 10001 |
| Identity image | `ghcr.io/loveoverflowcom/cantos-identity@sha256:966c35a06cb4fb1fcfd2c83a138cc42fe59e8d184c35f96a5dfaea3a5e1d0ce6`, UID 10002 |
| Web artifact | `release-build theatre-web --api-origin https://api.cantos.cc` at the same commit, config digest `daf7ec459c65…` |
| Host | one Debian 13 x86_64 VPS (1 vCPU, 1.9 GiB), Podman 5.4.2 |

## What was observed

| Layer | Result | Evidence and limit |
| --- | --- | --- |
| Build and push | PASS | GHCR push and registry manifest/config readback in the Actions run; layer and pair gates stay `NOT_RUN` |
| Pull on host | PASS | exact digests pulled; image IDs equal the recorded config digests; non-root user, exact entrypoint, no `Cmd` |
| Isolated smoke | PASS (development mode) | both migrations, Identity `ready`, Server over the Unix socket; bad login returned `401 unauthenticated` |
| Native production start | PASS | both Quadlet units active; Server `production`, `server.db epoch 1, version 23`; edge reached `cantos-server:8080` |
| Public API | PASS | `https://api.cantos.cc` valid TLS; `/api/v1/theatre/identity/session` `401`; CORS preflight echoes `https://cantos.cc` |
| Pages publish | PASS | `cargo xtask deploy cloudflare cantos-theatre --web … --environment production`: deployment `fbad22a4-5fd6-4811-b734-f1e73a9d81e9`, readback and 9-request smoke |
| DNS | PASS | zone created, records mirrored, DNSSEC disabled by the owner, nameservers changed at the registrar, zone `active`, `cantos.cc` served by Cloudflare |

## Deviations and findings

- The legacy stack (API ×2, Kanidm, PostgreSQL, backup timer) was **stopped and disabled, not
  deleted**. A fresh logical backup was taken first; backups and the previous edge files
  (`Caddyfile.legacy`, `edge.env.legacy`) were retained. No legacy data was migrated.
- Server production refuses `CANTOS_AUDIO_ROOT`; the filesystem target must be named
  `legacy-filesystem` and the TOML must be readable by UID 10001 (mode 0644, no secrets). R2 was
  not enabled, so private storage is a host directory (mode 0700, owner 10001).
- The Identity image has no `Cmd`: its unit needs `Exec=serve`. The socket directory must be
  `2750`, owner 10002, group 10001, with socket mode `0660`, so only the Server can connect.
- Behind Caddy the Server uses `CANTOS_IDENTITY_PEER_POLICY=trusted_proxy` with the Podman
  network CIDR; Caddy replaces `X-Forwarded-For`.
- A fresh worktree needs `npm ci` in `infra/cloudflare/wrangler` before the Pages publisher runs.
- Spaceship DNSSEC had to be disabled before the nameserver change; it is still disabled.

## Reproducible host shape (no secrets)

Directories `/root/cantos-native/{identity-db,server-db,storage,run,config}`; units
`cantos-identity.container` (`Network=none`, `Exec=serve`, `ExecStartPre` migrate) and
`cantos-server.container` (network `cantos.network`, `ExecStartPre` `cantos-migrate`) pinned to the
digests above; Caddy serves only `api.cantos.cc` and proxies `/api/*` to `cantos-server:8080`.
Server environment: `CANTOS_ENV=production`, `CANTOS_BIND_ADDRESS`, `CANTOS_SERVER_DATABASE`,
`CANTOS_STORAGE_CONFIG`, per-surface HTTPS origins (`https://cantos.cc`, `https://studio.cantos.cc`),
`CANTOS_IDENTITY_SOCKET`, `CANTOS_IDENTITY_PEER_UID`, `CANTOS_IDENTITY_PEER_POLICY`,
`CANTOS_IDENTITY_TRUSTED_PROXIES`. Identity: database, socket, mode, server UIDs and signup link base.
The Quadlet renderer and activation consumer remain the unimplemented PR B of #348; these hand-written
units are a test-host observation, not a supported lane.

## Not done

Browser, cookie and sign-in journeys; mail provider (`mail: missing`, so signup is unavailable);
Studio roster, so no Studio sign-in; Studio Pages publication (`studio.cantos.cc` still proxies
Firebase); image-layer and image-pair gates; restore and reboot rehearsal; DNSSEC re-enable;
Pages custom-domain status was still `pending` after four minutes although the site served.
Credentials shared in chat during the run (Cloudflare token, GHCR PAT) must be rotated.
