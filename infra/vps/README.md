# VPS production runtime

[#348](https://github.com/loveoverflowcom/cantos/issues/348) selects two native Rust executables
packaged as GHCR images and supervised by Podman Quadlet. The
[offline OCI contract](../../docs/deployment/native-oci-release.md) is the first release-owner
slice; publishers and the native host consumer remain pending. The assets below are retained
legacy runtime/recovery owners, not an implementation of the native target or host inventory.
Do not activate refactor HEAD, retire PG/Kanidm or change runtime principal from source evidence.

The [Firebase verification amendment](../../docs/architecture/firebase-verification.md#production-images-and-vps)
is preparation only: the native production adapter is NOT_IMPLEMENTED and provider selection
fails closed. Future activation must bind project/credentials/HTTPS actions explicitly through
operator-owned Podman secrets/read-only runtime files. Never add credentials to the image,
Actions or this directory's nonsecret templates. Retained legacy Quadlets are not a native
Identity activation mechanism; no runtime or secret configuration is changed by this PR.

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
| [`bin/cantos-vps`](bin/cantos-vps) | Root host automation: install, bootstrap, deploy, rollback, backup, restore, status, operator-run identity; source-bound `reload-browser TREE` validates/copies only Caddy and restarts edge/API without SQL, secret or image changes |
| [`bin/cantos-epoch2-cutover`](bin/cantos-epoch2-cutover), [`postgres/epoch2-empty-business-guard.sql`](postgres/epoch2-empty-business-guard.sql) | Narrow explicit legacy-883e188/28-migration cutover: sibling rehearsal, frozen-row import, off-host recovery acknowledgement, paired recovery and post-acceptance finalization |
| [`bin/cantos-oidc-reconcile`](bin/cantos-oidc-reconcile), [`bin/cantos-oidc-config.py`](bin/cantos-oidc-config.py) | Existing-client URL/callback reconciliation, secret-redacting plans, preservation checks and readback receipt; no accounts, resets, new grants or secret rotation |
| [`config.example.env`](config.example.env) | Single nonsecret origin/callback input file `/root/cantos-services/config/edge.env`; reviewed values are required |
| [`config/storage.filesystem.toml`](config/storage.filesystem.toml) | First-install **private Studio audio** target, selected locally without S3 credentials; installed once at `/root/cantos-services/config/storage.toml` |

The edge routing policy stays in [`infra/edge/Caddyfile`](../edge/Caddyfile). Changing a Quadlet
unit, the grants or the Caddyfile takes effect through `cargo xtask deploy vps`
(or `--bootstrap`), which uploads this directory from the committed revision.

The Studio API Quadlet bind-mounts `/var/lib/cantos/private` read/write and a
read-only operator-owned `storage.toml` profile. The host helper initializes the
directory as UID/GID 10001 with mode 0700 and refuses unsafe replacement; it
never rewrites a chosen profile or media bytes. `legacy-filesystem` remains the
durable target identity, even when Long Vân becomes the active write target.
First promotion prepares storage from the incoming bundle before runtime installation,
without repeating bootstrap. Existing drift is refused before runtime/recovery rotation;
new profiles are published without overwrite. Host TOML preflight uses Python 3.11+;
Rust retains provider validation. Browser-only validation is independent, while
image switches/restarts check storage when the installed Studio unit enables it.
The Theatre API has **no private media mount**. See the
[VPS media runbook](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/vps-production.md#development-private-audio-on-the-vps).

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
[migration procedure and cookie/domain gate](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/vps-production.md#direct-hosting--api-configuration).

For the [#253 Bearer migration](../../services/cantos-server/contracts/auth/bearer/v1/README.md), reconciliation derives
two exact callbacks for each existing Web client from its validated API origin:
`/api/v1/{studio|theatre}/auth/callback` and `/api/v1/{studio|theatre}/auth/bearer/callback`.
The six browser inputs are unchanged; callers cannot supply additional callback URLs. Readback
must contain exactly that pair with strict redirect matching before the receipt records both
`CANTOS_<SURFACE>_OIDC_CALLBACK` and `CANTOS_<SURFACE>_OIDC_BEARER_CALLBACK`. Deploy, rollback and
browser reload refuse a legacy-only receipt, including issuer settings in supported drop-ins.
The account-provisioning form remains unchanged and produces its original input-only receipt;
it requires separately authorized `identity --reconcile-existing` before activating this source.
Failed reconciliation restores the original client snapshots, runtime configuration and receipt.
This source support performs no live registration and does not implement or qualify the
[restricted identity gateway](../edge/identity-gateway.md).

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

Release pruning validates all retained epoch-2 journal identities/phases/metadata before any
deletion. Each journal protects its target and previous images and whole matching incoming
revision directories, including original flat and nested attempt bundles. These protections
extend the normal current/previous-image and three-newest-upload windows. Any malformed,
unreadable or unknown journal skips all release pruning with a warning. Unprotected images
need exact Cantos API title, matching revision and repository-source OCI labels; foreign/missing
labels and unknown/symlink incoming entries are left untouched. Journals and their referenced
recovery resources need a separate retention decision; ordinary deploy does not retire them.

Verified release artifacts also carry this exact source bundle. The artifact-only `promote` host
operation holds the existing operation lock across checksum/image inspection, runtime install
and the existing backup/schema/grants/switch/health protocol. It adds no bootstrap, identity or
epoch-conversion authority. See [artifact promotion](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/vps-production.md#verified-artifact-promotion).

The explicit same-epoch [prefix-upgrade lane](../../docs/deployment/postgres-prefix-upgrade.md)
restores a full epoch-2 database (`[1]` or `[1,2]`) into a sibling before applying the candidate's
exact additive registry suffix (`[1,2]` or `[1,2,3]`). It uses separate
attempt-bound journals and pending guards, preserves the original DB, and recovers the
DB/image/runtime pair before acceptance; accepted recovery only finalizes. It does not
provision media, reconcile identity or weaken ordinary schema checks. PostgreSQL dumps
exclude media bytes; Linux x86_64/17.11 host and media recovery acceptance remain separate.

The committed `bin/cantos-vps` entrypoint must keep Git mode `100755`. Source-bundle install,
browser reload and cutover/recovery execute it directly after extraction; installed bootstrap
uses `/usr/local/sbin/cantos-vps` (installed with mode `0750`). Artifact promotion likewise
executes the bundled entrypoint directly. Release packaging and extraction reject a runtime
archive without its owner execute bit before remote writes; source identity and checksum
validation still apply. The repository Python regressions check committed tree mode, real
`git archive` extraction, release repackaging/extraction and a validly hashed `100644` negative
control without running the host script or SSH. The other shell helpers are sourced, and
`cantos-oidc-config.py` is invoked through Python; they do not need executable Git modes.
