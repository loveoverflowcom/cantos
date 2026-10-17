# VPS production runtime

Tracked, nonsecret VPS runtime assets. [ADR 0039](../../docs/decisions/0039-vps-production-runtime.md)
retains the runtime foundation; [ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md)
selects direct Hosting. Generated legacy origins/public identity proxy remain transition work;
source changes do not describe installed state.
The operator procedure is the [runbook](../../docs/deployment/vps-production.md); the command
contract is [`cargo xtask deploy vps`](../../xtask/README.md#deploy-vps). Nothing here contains a
host secret: passwords, the private CA, client secrets and the Kanidm admin credential are
generated on the VPS and kept root-only there.

| Path | Owns |
| --- | --- |
| [`image/Containerfile`](image/Containerfile) | Digest-pinned release image: `cantos-server` + `cantos-migrate`, non-root, `CANTOS_ENV=production`; built off the VPS from a `git archive` |
| [`quadlet/`](quadlet) | Podman Quadlet units: private network `cantos` (bridge `cantos0`), socket-only PostgreSQL, Kanidm, two single-origin API processes, the edge |
| [`postgres/roles.sql`](postgres/roles.sql) | `cantos_owner` (owns database, migrates) and `cantos_app` (runtime login) |
| [`postgres/runtime-grants.sql`](postgres/runtime-grants.sql) | Least-privilege runtime grants, checked by `production_grants_postgres` |
| [`kanidm/server.toml.template`](kanidm/server.toml.template) | Kanidm configuration rendered with the identity host name |
| [`systemd/`](systemd) | Daily logical backup timer |
| [`bin/cantos-vps`](bin/cantos-vps) | Root host automation: install, bootstrap, deploy, rollback, backup, restore, status, operator-run identity |
| [`config.example.env`](config.example.env) | Shape of `/root/cantos-services/config/edge.env` |

The edge routing policy stays in [`infra/edge/Caddyfile`](../edge/Caddyfile). Changing a Quadlet
unit, the grants or the Caddyfile takes effect through `cargo xtask deploy vps`
(or `--bootstrap`), which uploads this directory from the committed revision.

Invariants: PostgreSQL never listens on TCP; only `cantos-edge` publishes ports; each API process
pins exactly one HTTPS Web origin; production refuses development Studio credentials; images are
built off-host from committed source; every deploy takes a backup before migrating and switches
back automatically when the health gate fails.
