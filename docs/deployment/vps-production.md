# Cantos production on the VPS and Firebase Hosting

Operator runbook for [ADR 0039](../decisions/0039-vps-production-runtime.md): Cantos API,
PostgreSQL, Kanidm and the HTTPS edge on one VPS; Theatre Web and Studio Web on two Firebase
Hosting sites, served to browsers through the same-origin edge. What actually ran and what is
still open is recorded in the [deployment record](../evidence/vps-production-deployment.md).

## Topology

| Public name | Serves | Behind it |
| --- | --- | --- |
| `cantoss.duckdns.org` | Cantos (Theatre Web) | `/api/*` → `cantos-api-theatre`; other paths → Firebase site `cantoss-theatre` |
| `studio.cantoss.duckdns.org` | Cantos Studio | `/api/*` → `cantos-api-studio`; other paths → Firebase site `cantoss-studio` |
| `api.cantoss.duckdns.org` | API for native apps and producers | `/api/*` → `cantos-api-theatre`; root 404 |
| `idm.cantoss.duckdns.org` | Kanidm sign-in and OIDC | `cantos-kanidm:8443` (private CA TLS) |

```text
browser ──HTTPS──> cantos-edge (Caddy, :80/:443, Let's Encrypt)
                     ├─ /api/* ─> cantos-api-theatre | cantos-api-studio  (one image, two origins)
                     ├─ pages  ─> https://<site>.web.app  (Firebase Hosting)
                     └─ idm.   ─> cantos-kanidm
cantos-api-* ──Unix socket──> cantos-postgres  (no TCP listener; volume cantos-postgres-socket)
```

Only `cantos-edge` publishes ports. PostgreSQL has no network at all; Kanidm and the APIs are on
the private `cantos` network (bridge `cantos0`). The `*.web.app` URLs are valid static
deployments too, but their `/api/*` is Firebase's 404: use the DuckDNS names.

| Host path (root, 0700) | Owns |
| --- | --- |
| `/root/cantos-services/config/edge.env` | nonsecret hosts and Firebase site IDs ([example](../../infra/vps/config.example.env)) |
| `/root/cantos-services/config/api-*.env` | rendered API settings; `api-*-oidc.env` written by `identity` |
| `/root/cantos-services/secrets/` | generated passwords (never printed); Podman secrets mirror them |
| `/root/cantos-services/kanidm/` | `server.toml`, private-CA certificate and key |
| `/root/cantos-services/internal-ca/` | private CA for container-internal TLS |
| `/root/cantos-services/backups/` | `pg_dump -Fc` files with `.sha256`, newest 14 kept |
| `/root/cantos-services/state/` | `current`, `previous`, `history` of API revisions |
| `/etc/containers/systemd/cantos*` | Quadlet units from [`infra/vps/quadlet`](../../infra/vps/quadlet) plus drop-ins |

Named volumes: `cantos-postgres-data`, `cantos-postgres-socket`, `cantos-kanidm-data`,
`cantos-edge-data` (certificates and ACME account), `cantos-edge-config`. Never prune them.

## One-time setup

Prerequisites: DNS for `cantoss` points at the VPS ([DuckDNS updater](../../infra/edge/README.md)),
SSH as root with your own key, Podman ≥ 5.4 on the VPS, and on your machine Podman, zstd, SSH,
Rust 1.98.1, Trunk 0.21.14 and firebase-tools 15.34.0 logged in (`firebase login`).

1. Firebase sites (already done for project `cantos-a348b`: `cantoss-theatre`, `cantoss-studio`)
   and the ignored mapping `infra/firebase/hosting/.firebaserc`
   ([Hosting runbook](firebase-hosting.md#one-time-setup)).
2. On the VPS, write `/root/cantos-services/config/edge.env` (mode 0600) from
   [`config.example.env`](../../infra/vps/config.example.env) with the real site IDs.
3. Bootstrap (idempotent; safe to rerun after editing `edge.env`):

   ```bash
   cargo xtask deploy vps cantos-api --host root@42.96.18.245 --bootstrap --dry-run
   cargo xtask deploy vps cantos-api --host root@42.96.18.245 --bootstrap
   ```

   It installs the units, generates the PostgreSQL superuser/owner/app passwords and the private
   CA, provisions roles and the `cantos` database, renders Kanidm's configuration, opens ufw for
   80/443 plus routing and DNS on `cantos0`, starts PostgreSQL, Kanidm and the edge and enables
   the daily backup timer. Caddy then obtains certificates for all four names.

## Deploy the API

```bash
cargo xtask deploy vps cantos-api --host root@42.96.18.245 \
  --smoke-origin https://api.cantoss.duckdns.org --dry-run      # review: starts nothing
cargo xtask deploy vps cantos-api --host root@42.96.18.245 \
  --smoke-origin https://api.cantoss.duckdns.org                # REMOTE WRITE
```

xtask archives the committed `HEAD` (uncommitted edits are never deployed), builds the
digest-pinned [image](../../infra/vps/image/Containerfile) locally with Podman (about 3 minutes
cold, cached afterwards), uploads it with the host bundle and runs `cantos-vps deploy SHA`:

1. a logical backup `pre-<sha>`;
2. `cantos-migrate` as `cantos_owner` (checksummed, transactional);
3. [`runtime-grants.sql`](../../infra/vps/postgres/runtime-grants.sql) for `cantos_app`;
4. retag `localhost/cantos-api:current`, restart both API units; `Notify=healthy` makes each
   restart wait for its health check (`GET /api/v1/theatre/works`, a real database read);
5. a probe of all three API hosts through the edge;
6. on any failure, switch back to the previous image (the migrated schema stays; the backup
   path is printed). Old images and uploads beyond current/previous are pruned on success.

A new migration that adds a table read or written at runtime **must** extend the grants file;
`production_grants_postgres` in `cargo xtask verify postgres` proves the file alone serves real
Studio and Theatre requests and grants nothing on excluded surfaces.

## Deploy the Web apps

```bash
cargo xtask deploy firebase cantos-theatre --web --project cantos-a348b --channel live
cargo xtask deploy firebase cantos-studio  --web --project cantos-a348b --channel live
```

Nothing on the VPS changes: the edge fetches the new bytes from Firebase on the next request
(`Cache-Control: no-cache`). Firebase console rollback applies to the DuckDNS names as well.

## Identity: Studio administrator and Theatre sign-in (operator-run)

Account creation and credentials belong to the owner. Run this yourself in an interactive
terminal; it prints one-time credential links only there:

```bash
ssh -t root@42.96.18.245 cantos-vps identity --studio-admin YOUR_ACCOUNT_NAME \
  --display "Your Name" [--listener A_LISTENER_ACCOUNT]
```

It recovers `idm_admin` into a root-only file (never printed), creates the confidential OAuth2
clients `cantos-studio-web-bff` and `cantos-theatre-web-bff` with their exact callbacks, the
groups `cantos_studio_admins` and `cantos_theatre_listeners` with `openid` scope maps, the named
people, stores both client secrets as Podman secrets, writes the issuers and the one-entry Studio
roster (the person's UUID, the OIDC `sub`), adds OIDC drop-ins to both API units and restarts
them. Open the printed link (valid 24 hours) to set a passkey or password, then sign in at
`https://studio.cantoss.duckdns.org`. Rerun with another `--listener` to add listeners; Theatre
self-registration stays disabled ([ADR 0034](../decisions/0034-theatre-signup-and-provisioned-studio.md)).

## Rollback, backup and restore

```bash
cargo xtask deploy vps cantos-api --host root@42.96.18.245 --rollback   # previous API image
cargo xtask deploy vps cantos-api --host root@42.96.18.245 --status     # read-only
ssh root@42.96.18.245 cantos-vps backup manual                          # on demand
ssh root@42.96.18.245 cantos-vps restore /root/cantos-services/backups/FILE.dump --yes-replace-database
```

Rollback swaps the API image only. Restore verifies the checksum, takes a `pre-restore` backup,
stops both APIs, restores in one transaction, reapplies grants and restarts them. Copy backups
off the VPS regularly (`scp root@…:/root/cantos-services/backups/*.dump …`); they live on the
same disk. Kanidm keeps its own daily online backup (7 versions) in `cantos-kanidm-data`.
Configuration rollback: the previous bundle stays at `/root/cantos-services/runtime.previous`.

## CircleCI (manual only)

[`.circleci/config.yml`](../../.circleci/config.yml) follows the VOT pattern: a pipeline
triggered by hand on `develop` with `target` (branch or SHA already merged into
`origin/develop`) and `component` (`api`, `theatre-web`, `studio-web`, `web`, `all`). An empty
target runs nothing. Jobs call the same xtask commands. Context `production` needs:

| Variable | Value |
| --- | --- |
| `VPS_HOST`, `VPS_USER` | `42.96.18.245`, `root` |
| `VPS_SSH_PRIVATE_KEY` | a **dedicated** deploy key; append its public key to `/root/.ssh/authorized_keys` |
| `VPS_KNOWN_HOSTS` | `42.96.18.245 ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIECs09HO2QgBOWXpAVbqQcDp3Oap4wqVmjINB16Ul78r` (verify on the host with `ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`) |
| `CANTOS_API_ORIGIN` | `https://api.cantoss.duckdns.org` |
| `FIREBASE_PROJECT`, `FIREBASE_THEATRE_SITE`, `FIREBASE_STUDIO_SITE` | `cantos-a348b`, `cantoss-theatre`, `cantoss-studio` |
| `FIREBASE_SERVICE_ACCOUNT_JSON` | key of a service account limited to *Firebase Hosting Admin* on that project |

The service-account key exists only for CI; never commit it or use it locally (local deploys use
`firebase login`). Without it, deploy the Web apps from your machine.

## Verify

```bash
curl -sS https://api.cantoss.duckdns.org/api/v1/theatre/works          # 200 JSON (real catalog)
curl -sS -o /dev/null -w '%{http_code}\n' https://studio.cantoss.duckdns.org/api/v1/studio/auth/configuration
                                                                        # 503 before identity, 200 after
curl -sS https://idm.cantoss.duckdns.org/status                        # true
curl -sSI https://cantoss.duckdns.org/ | grep -i cache-control          # no-cache (Firebase bytes)
for p in 5432 8080 8443 2019; do timeout 5 bash -c "</dev/tcp/42.96.18.245/$p" && echo "$p OPEN"; done
ssh root@42.96.18.245 cantos-vps status
```

## Troubleshooting

| Symptom | Cause and fix |
| --- | --- |
| Certificates never issue; edge log `lookup … on 10.89.0.1:53: i/o timeout` | ufw input/route deny for the bridge; rerun `--bootstrap` (it adds `route allow in/out on cantos0` and DNS on `cantos0`) |
| A unit stays `activating`, then times out | its health check never passes; `podman inspect NAME --format '{{json .State.Health}}'`. Shell-form health commands fail in images without `/bin/sh` (Kanidm uses exec form) |
| `deploy` rolled back | `journalctl -u cantos-api-theatre -u cantos-api-studio`; the new image stays loaded; fix and redeploy |
| Studio shows "cannot confirm session" | identity not provisioned yet (configuration answers 503) or Kanidm unreachable |
| Changed the `cantos` network definition | stop the units, `podman network rm cantos`, rerun `--bootstrap` |

## Acceptance status

See the [deployment record](../evidence/vps-production-deployment.md) for observed results.
Not accepted yet: real Studio/Theatre sign-in and logout against this Kanidm (needs the
operator-run identity step and a human credential), a reboot test, off-host backup copies,
an automatic-rollback drill, CircleCI runs (needs the context), object storage, publication
delivery, Theatre registration, native identity and Admin.
