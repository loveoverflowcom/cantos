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
| [`bin/cantos-epoch2-cutover`](bin/cantos-epoch2-cutover), [`postgres/epoch2-empty-business-guard.sql`](postgres/epoch2-empty-business-guard.sql) | Narrow explicit legacy-883e188/28-migration cutover: sibling rehearsal, frozen-row import, off-host recovery acknowledgement, paired recovery and post-acceptance finalization |
| [`bin/cantos-oidc-reconcile`](bin/cantos-oidc-reconcile), [`bin/cantos-oidc-config.py`](bin/cantos-oidc-config.py) | Existing-client URL/callback reconciliation, secret-redacting plans, preservation checks and readback receipt; no accounts, resets, new grants or secret rotation |
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

The account-provisioning `identity` form uses those same Web origins and API callbacks.
The separate `identity --reconcile-existing` form preserves accounts, roster, groups, scopes,
passwords and secrets while reconciling exact landing/callback URLs and strict matching. Its
provider readback, visible-attribute/group/roster checks and `show-basic-secret` digest comparison
precede the nonsecret `config/oidc-callbacks.env` receipt. Existing generated OIDC configuration
needs a matching receipt before deploy/rollback. This establishes observed configuration,
not a successful browser session or equality of provider-redacted signing-key bytes. See the
[migration procedure and cookie/domain gate](../../docs/deployment/vps-production.md#direct-hosting--api-configuration).

The current image declares schema epoch 2. Ordinary deploy/rollback rejects legacy databases
and images from another epoch; it does not convert or reset a deployed database. PostgreSQL
stays pinned to 17.11. Read the [schema cutover plan](../../docs/deployment/postgres-schema-cutover.md)
before deploying the current source to an installation using historical migrations.
`--cutover-epoch2` is the only implemented cross-epoch lane: it requires that exact legacy
source/schema/checksum/ownership and zero business/session rows, preserving one clock row and
two Studio/three Theatre OIDC-flow rows. It retains the old database/image/runtime and refuses
ordinary mutations while recovery is pending. A general populated-data transformer remains
unimplemented. Live acceptance belongs to attributable private run receipts and the owning PR;
the presence of a command or an earlier run does not describe today's installed state.

The verified off-host cutover archive excludes OCI images, Podman secrets, the Kanidm admin
password and private CA. The exact old image stays on the host; the archive is not complete
disaster recovery. Keep independent protected image/secret/identity/media/CA recovery. Use the
same target SHA, persisted 32-hex attempt ID and exact incoming attempt tree for the runbook's
`cutover-recover` invocation. A revision alone cannot select another attempt's recovery. Recovery
before `accepted` restores the matching installation; after `accepted` it finalizes only.
