# VPS production runtime

Tracked, nonsecret VPS runtime assets. [ADR 0039](../../docs/decisions/0039-vps-production-runtime.md)
retains the runtime foundation; [ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md)
selects direct Hosting. Explicit per-surface origins/callbacks are validated by the host script;
installed legacy configuration/public identity proxy remain transition work;
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
| [`config.example.env`](config.example.env) | Single nonsecret origin/callback input file `/root/cantos-services/config/edge.env`; reviewed values are required |

The edge routing policy stays in [`infra/edge/Caddyfile`](../edge/Caddyfile). Changing a Quadlet
unit, the grants or the Caddyfile takes effect through `cargo xtask deploy vps`
(or `--bootstrap`), which uploads this directory from the committed revision.

Invariants: PostgreSQL never listens on TCP; only `cantos-edge` publishes ports; each API process
pins exactly one HTTPS Web origin; production refuses development Studio credentials; images are
built off-host from committed source; every deploy takes a backup before migrating and switches
back automatically when the health gate fails.

`bootstrap` validates both surfaces before rendering their API env files: each pins its Web
origin, `CANTOS_BROWSER_API_SURFACE` and explicit API origin. The API origin must match the
configured HTTPS API ingress; each OIDC callback must use that origin and its exact surface path.
Legacy Web proxy host names never supply a missing browser origin. Deploy/rollback compare the
rendered env files with this input before backup, migration or image switching and refuse drift;
they never rewrite origin policy during an image switch. Supplemental Quadlet env files and
simple direct env assignments cannot replace those protected keys. Unsupported override syntax
fails closed; see the runbook for supported drop-ins. This check covers the tracked
Quadlet/env-file composition; generated systemd service overrides still need separate inspection.

Authorized `identity` uses those same Web origins and API callbacks, and writes the nonsecret
`config/oidc-callbacks.env` input receipt. An existing generated OIDC configuration needs a matching
receipt before deploy/rollback. The receipt does not prove live provider state or remove obsolete
redirects: an operator must review those and accept the callback/session flow separately. See the
[migration procedure and cookie/domain gate](../../docs/deployment/vps-production.md#direct-hosting--api-configuration).

The current image declares schema epoch 2. Ordinary deploy/rollback rejects legacy databases
and images from another epoch; it does not convert or reset a deployed database. PostgreSQL
stays pinned to 17.11. Read the [schema cutover plan](../../docs/deployment/postgres-schema-cutover.md)
before deploying the current source to an installation using historical migrations.
