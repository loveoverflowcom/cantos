# Current production topology and operations

**Web provider transition:** #278/#280 request static Cloudflare hosting for the existing
`cantos.cc` / `studio.cantos.cc` targets. The
[rollout and recovery runbook](cloudflare-rollout.md) owns provider qualification, domain
cutover and Firebase Hosting retirement; the Firebase destinations below describe the retained
current/recovery lane, not evidence of a completed migration. Recheck actual installed state.
The API/PostgreSQL/Kanidm stay here. Compatible API runtime needs no redeploy solely for a Web
host change; image, browser-config, identity and database authority remain separate.

> **#253 staging:** [Bearer v1](../../packages/contracts/bearer-v1.md) adds a new exact API callback
> `/api/v1/{studio|theatre}/auth/bearer/callback` for the existing per-surface confidential client.
> Registering/qualifying it, updating the existing-client reconciliation receipt and completing
> the restricted identity gateway need their own reviewed source/operator rollout. This API
> foundation performs no such live write. Current cookie handlers are compatibility source;
> [#295](https://github.com/loveoverflowcom/cantos/issues/295) now selects Web cookies on the custom
> domains, with migration pending. Review callback/session reuse under that decision; native
> retains Bearer. Do not retire cookie support as a consequence of the superseded Web target.

This is the single current production topology/runbook. [ADR 0041](../decisions/0041-direct-hosting-api-boundary.md)
selects direct Firebase Hosting and API-mediated browser requests; [ADR 0039](../decisions/0039-vps-production-runtime.md)
retains the VPS runtime/durability rules. Installed configuration is transitional. Recheck the
remote status and Hosting releases before every operation; this document is not a deployment receipt.

**Schema transition:** current source requires epoch 2. Ordinary deploy/rollback refuses the
previous epoch-1 database and images; it cannot convert the deployed installation. Read the
[controlled cutover runbook](postgres-schema-cutover.md) before packaging this source for a
live host. The separate `--cutover-epoch2` lane supports only the checked legacy
`883e188`/28-migration installation with empty business/session tables and the specified clock
and OIDC-flow rows. It rehearses a sibling restore/import and verifies an off-host recovery
copy before switching. A general populated-data transformer remains unimplemented. Attribute
each live operation to its private source-bound receipts and owning PR; command support alone
does not establish deployment acceptance. PostgreSQL 17.11 stays pinned.

## Destinations and remaining transition

| Component | Destination | Acceptance boundary |
| --- | --- | --- |
| Cantos Web | [cantoss-theatre.web.app](https://cantoss-theatre.web.app) on Firebase project `cantos-a348b`, site `cantoss-theatre` | Static delivery exists; API-origin bundle and server CORS need separately authorized deployment |
| Cantos Studio | [cantoss-studio.web.app](https://cantoss-studio.web.app), site `cantoss-studio` in the same project | Static delivery exists; direct-origin authentication is not accepted |
| Cantos API | [api.cantoss.duckdns.org](https://api.cantoss.duckdns.org) on VPS `42.96.18.245` | Two origin-pinned processes, one image; new Studio ingress routing must be deployed with the reviewed settings |
| Kanidm | Private `cantos` network, `cantos-kanidm:8443`, private-CA TLS | Minimal browser auth gateway/domain migration pending; management stays private |
| PostgreSQL | `cantos-postgres`, shared Unix socket volume, SCRAM | No TCP listener; owner migration role and least-privilege runtime role |
| Caddy | VPS API/TLS and required auth gateway ingress | Installed legacy Web/identity proxies remain transition work, not the target topology |

The old Web hosts (`cantoss.duckdns.org`, `studio.cantoss.duckdns.org`) still proxy Firebase and
same-origin API requests. They may be inspected diagnostically but must not be presented as the
selected direct-Hosting solution. The legacy public identity host is also still reachable. A
401 from a management path means it reached Kanidm, not that the edge blocked it; require an edge
refusal before upstream routing. Current source's management-prefix filter is not proof that the
installed edge has it. Keep authentication unavailable rather than disabling browser checks.

## Read-only inventory

```bash
cargo xtask deploy vps cantos-api --host root@42.96.18.245 --status
firebase hosting:sites:list --project cantos-a348b --json
firebase hosting:channel:list --site cantoss-theatre --project cantos-a348b \
  --config /ABS/PATH/infra/firebase/hosting/firebase.json --json
firebase hosting:channel:list --site cantoss-studio --project cantos-a348b \
  --config /ABS/PATH/infra/firebase/hosting/firebase.json --json
```

Run Firebase diagnostics sequentially from separate private ignored working directories; the CLI
may race on `firebase-debug.log`. It can include operator emails, so sanitize before sharing.
Confirm each running container's image revision label against `state/current`; do not substitute
`origin/develop`. For Web, record the release version/message and match served file SHA-256 values
to its local receipt. A `+dirty` source cannot honestly be called an exact clean deployed SHA.
Keep observations in ignored artifacts and the review PR, not a growing deployment-session diary.
Receipts retain the commit actually built/deployed. Later documentation-only commits do not
change that identity; report the deployed revision and final PR revision separately.

## Bearer rollout preconditions

Before deploying the Bearer clients, establish the installed API image revision and current
schema registry through trusted read-only access. Merge state and a 403 admission refusal do
not establish a deployed route. In particular, use the actual methods in
[Bearer v1](../../packages/contracts/bearer-v1.md); a GET of the POST-only flow path is only a
routing diagnostic. Do not run a consuming login/exchange/refresh merely as a presence probe.

The new API requires epoch 2 migrations 0001 and 0002. `preflight_schema` currently invokes the
strict target `--check` before backup/migration: an existing 0001-only registry is refused by
ordinary deployment. Do not bypass that guard, edit registry rows or run a production migration
by hand to force a rollout. Use only the explicit [paired prefix-upgrade lane](postgres-prefix-upgrade.md)
after its operator prerequisites and rehearsal when the live inventory shows that prefix.
It migrates a fully restored sibling, retains the source and pairs database/runtime recovery;
ordinary deployment remains strict. The old 0001-only binary rejects the expanded registry;
its pre-migration health does not establish a usable image-only rollback after 0002. Preserve
checked backups and verified data-preserving recovery; never promise ordinary binary rollback
across this boundary or automatically restore/drop production data.

The [restricted gateway source](../../infra/edge/identity-gateway.md) retains the existing
initialized identity authority and private CA. Inventory explicit existing Cantos TCP peers for
`CANTOS_INTERNAL_CIDRS` before activation; there is no broad private-network default. Validate
with the pinned deployed Caddy and separately qualify actual login/setup/consent and API
back-channel behavior. Callback reconciliation is limited to the existing clients' exact
legacy/Bearer URLs and preservation checks. No provider account, grant, secret or domain change
is implied by API, gateway or static Hosting deployment.

## Direct Hosting → API configuration

### Selected custom domains

The owner selected `https://cantos.cc` for Theatre, `https://studio.cantos.cc` for Studio and
`https://api.cantos.cc` for the API. The Hosting sites remain `cantoss-theatre` and
`cantoss-studio` in project `cantos-a348b`. Register each custom domain on its own site and use
the DNS records returned by Firebase; certificate issuance and real browser acceptance are
separate observations. The VPS API A record uses the inventoried `42.96.18.245`; add AAAA only
after observing a usable public IPv6 address and end-to-end IPv6 ingress.

For the first HTTPS stage keep `CANTOS_API_HOST=api.cantoss.duckdns.org` and add
`CANTOS_API_ALIAS_HOST=api.cantos.cc`. Both hosts use the same surface-separated API routes;
the optional alias does not add a CORS origin or change callbacks. After new TLS is observed,
set the primary host to `api.cantos.cc`, retain the old host as the alias, and reconcile:

```text
CANTOS_THEATRE_WEB_ORIGIN=https://cantos.cc
CANTOS_STUDIO_WEB_ORIGIN=https://studio.cantos.cc
CANTOS_THEATRE_API_ORIGIN=https://api.cantos.cc
CANTOS_STUDIO_API_ORIGIN=https://api.cantos.cc
CANTOS_THEATRE_OIDC_CALLBACK=https://api.cantos.cc/api/v1/theatre/auth/callback
CANTOS_STUDIO_OIDC_CALLBACK=https://api.cantos.cc/api/v1/studio/auth/callback
```

Keep a root-only copy of the previous edge/API/callback files. The existing authorized
`cantos-vps identity --reconcile-existing` updates only existing clients, preserves accounts,
groups, roster and credential digests, and renders API env files. Apply the reviewed config
with `cargo xtask deploy vps cantos-api --host root@42.96.18.245 --reload-browser`; this lane
avoids bootstrap, SQL, migrations, secret changes and image replacement. Build/deploy both Web
apps with `CANTOS_API_ORIGIN=https://api.cantos.cc` through their existing explicit Hosting
commands. Preserve host-only Secure/HttpOnly Strict sessions, Lax flow cookies and CSRF guards.

Do not add `idm.cantos.cc`, rename Kanidm's initialized domain, or change WebAuthn credentials.
The existing identity ingress remains transitional; the ADR 0041 default-deny gateway and
real login/logout acceptance remain separate work. Domain selection alone does not accept it.

Build/deploy each app with explicit nonsecret `CANTOS_API_ORIGIN`. Invalid input fails closed in
the client; unset keeps local Trunk paths. The Hosting receipt records this input in `build.api_origin` and the explicit build command.

```bash
CANTOS_API_ORIGIN=https://api.cantoss.duckdns.org \
  cargo xtask build cantos-theatre --web --release
CANTOS_API_ORIGIN=https://api.cantoss.duckdns.org \
  cargo xtask build cantos-studio --web --release
```

The host's single nonsecret source is `/root/cantos-services/config/edge.env`, shaped by
[`config.example.env`](../../infra/vps/config.example.env). Supply all six reviewed browser
inputs explicitly; legacy `CANTOS_THEATRE_HOST`/`CANTOS_STUDIO_HOST` continue to name transitional
edge proxies and never fill in missing Web origins.

| Input per surface (`THEATRE` or `STUDIO`) | Required value |
| --- | --- |
| `CANTOS_<SURFACE>_WEB_ORIGIN` | Exact reviewed HTTPS Web origin; the two surfaces must differ |
| `CANTOS_<SURFACE>_API_ORIGIN` | `https://` plus the configured `CANTOS_API_HOST`, for the existing default-port ingress |
| `CANTOS_<SURFACE>_OIDC_CALLBACK` | That API origin plus `/api/v1/theatre/auth/callback` or `/api/v1/studio/auth/callback`, respectively |

Origins use lowercase DNS names with no port, path, trailing slash, credentials, query or fragment.
An explicitly authorized bootstrap validates the complete input set before replacing generated
configuration. It renders `api-theatre.env` and `api-studio.env`, each with
`CANTOS_BROWSER_API_SURFACE=theatre` or `studio`, the matching exact `CANTOS_WEB_ORIGIN` and
`CANTOS_API_ORIGIN`. The API origin sets its exact OIDC callback host separately from the fixed
Web return origin. Do not permit both surfaces through one origin wildcard.
The API ingress must route `/api/v1/studio/*` to Studio and `/api/v1/theatre/*` to Theatre. Preserve
host-only HttpOnly/Secure cookies, CSRF Origin checks, PKCE/state/nonce and current authorization.

Deploy and rollback compare the generated API env files to those inputs before backup, migration
or image switching. Missing, duplicate, invalid or stale settings refuse the operation; they do
not rewrite running origin policy. Reconcile a legacy install through a separately authorized
bootstrap/configuration change first. API Quadlet `*.container.d/*.conf` supplemental files may
not redefine `CANTOS_ENV`, `CANTOS_BIND_ADDRESS`, `CANTOS_WEB_ORIGIN`, `CANTOS_API_ORIGIN` or
`CANTOS_BROWSER_API_SURFACE`. Supported env syntax is one `EnvironmentFile=/absolute/path`
per line (no optional-file prefix, quoting or specifiers), or one plain uppercase-key
`Environment=KEY=VALUE` with a simple value containing letters, digits, `_./:,-`. Move complex
registration values into a supplemental env file. Each nonempty, noncomment supplemental line
must be a plain `KEY=VALUE` assignment with an unquoted identifier and no whitespace before or
inside the key; its same-line value may contain spaces or JSON. Bare names, wildcards, `export`
and multiline values are refused, so Podman cannot forward host environment by an unassigned
name. Registration remains disabled unless its
independent enablement gates are met. The generated OIDC Secret entry is allowed. Ordinary
tracked Unit/Service, mount, network and health directives are supported; unknown directives,
environment forwarding, custom commands and raw Podman args fail closed pending review.
Inspect global Quadlet drop-ins and generated systemd service overrides independently; this
validation covers the per-surface Quadlet/env-file composition, not arbitrary operator
replacement of the service.

When generated `api-<surface>-oidc.env` exists, deploy/rollback also require matching callbacks
in `config/oidc-callbacks.env`. Existing installations without it fail closed. For the two
existing clients, `cantos-vps identity --reconcile-existing` updates landing URLs, replaces
obsolete callbacks with the exact reviewed callback set and enables strict redirect matching.
It checks provider readback, protected client attributes, group membership, the unchanged
Studio roster file and the basic-secret digest before writing the receipt. The epoch cutover
uses the same reconciler with the six staged inputs described in the
[cutover procedure](postgres-schema-cutover.md#implemented-narrow-epoch-1-cutover).
The receipt establishes those observed configuration inputs, not live discovery/login or
underlying bytes hidden by provider redaction. Never hand-write it as a shortcut.

Public anonymous reads can use web.app → DuckDNS after deployment. The current Web clients
implement [Bearer v1](../../packages/contracts/bearer-v1.md), but #295 selects API-issued
HttpOnly/Secure Web cookies on `cantos.cc` / `studio.cantos.cc` with `api.cantos.cc`; migration
and real browser acceptance are pending. Native retains Bearer. Qualify exact credentialed CORS,
CSRF, callback/session scope and the restricted identity gateway; raw Hosting previews need a
separate declared topology. Existing cookie handlers require review before reuse. A source change
does not rewrite the installed legacy inputs, issuer redirects or custom drop-ins; reconcile them
together in the authorized cutover and retain the reviewed previous configuration for recovery.
Image rollback leaves origin and identity configuration unchanged; restore those separately when
rolling back a domain/issuer migration, before restarting the previous image.

## Explicit remote operations

Use the [xtask command contracts](../../xtask/README.md#deploy-vps) and the
[Hosting tooling guide](firebase-hosting.md). Each write requires a user request naming its
host, or Firebase project/channel/app. An audit/PR request is not deployment permission.

```bash
cargo xtask deploy vps cantos-api --host root@42.96.18.245 --dry-run
cargo xtask deploy vps cantos-api --host root@42.96.18.245 --bootstrap --dry-run
cargo xtask deploy vps cantos-api --host root@42.96.18.245 --cutover-epoch2 --dry-run
CANTOS_API_ORIGIN=https://api.cantoss.duckdns.org \
  cargo xtask deploy firebase cantos-theatre --web --project cantos-a348b --channel live --dry-run
CANTOS_API_ORIGIN=https://api.cantoss.duckdns.org \
  cargo xtask deploy firebase cantos-studio --web --project cantos-a348b --channel live --dry-run
```

Removing `--dry-run` writes production. API deployment builds committed HEAD locally with Podman,
backs up, migrates as `cantos_owner`, reapplies runtime grants, switches both API processes,
health-gates and switches back on failure. Migrations remain forward-only. Unknown outcomes require
status/console reconciliation before replay. Never initialize/start/change a local Podman machine
as an audit side effect. CircleCI remains manual; CI is not user acceptance.
The narrow cutover is a separate operation: preserve its pending journal, incoming source tree
and private local off-host archive. Its [state-aware recovery command](postgres-schema-cutover.md#implemented-narrow-epoch-1-cutover)
uses the same full revision, persisted attempt ID and exact incoming attempt tree. An accepted
cutover can only be finalized, because returning to legacy data could discard new writes.

## Identity: Studio administrator and Theatre sign-in (operator-run)

Existing accounts only for acceptance. The account-provisioning form
`cantos-vps identity --studio-admin NAME [--display TEXT] [--listener NAME]` changes accounts,
clients, groups, secrets, roster and API units; its delegation names the host/accounts.
`cantos-vps identity --reconcile-existing` instead preserves those accounts, roster, group
membership, scopes, passwords and existing secrets, and has no reset/account-creation path.
It requires the existing administrator credential and both confidential clients/Podman secrets.
Raw `show-basic-secret` output stays in a transient root-only file; only digests survive the
preservation checks. Failure restores the captured provider/API configuration or retains the
private snapshot for reconciliation if protected state changed concurrently. It does not
restart APIs by itself; the coordinated deployment applies the rendered API inputs.
Deployment authorization alone does not include identity changes. Password/passkey/MFA setup
belongs to the operator's interactive browser. Never put credentials or reset links in chat,
Git, screenshots or shared logs.
Theatre registration stays disabled until [Firebase enrollment](firebase-enrollment-proof.md),
[least privilege](firebase-enrollment-service-account.md) and ADR 0040 gates are independently met.
An authorized test email alone neither enables signup nor creates a Studio administrator.

## Theatre registration behind the edge (not enabled)

Preserve the existing 16 KB edge body cap, explicit trusted-proxy peer policy, replaced
`X-Forwarded-For` and dropped `Forwarded`/`X-Real-IP`. Restrict trusted CIDRs to reviewed Cantos
peers; process-local quotas do not coordinate across hosts. SQL budgets persist separately.
`oobCode` ingress needs no-store/no-referrer, query scrubbing before static upstreams, and no query
logging. Moving direct to Firebase requires equivalent first-response protection there before
email action URLs can target it; the old VPS-only protection does not follow the bundle.
Service-account signing files need dedicated least privilege and an explicit protected mount;
the base Quadlet has none. Enable only reviewed registration grants; keep revocation idempotent.
Kanidm browser-route allowlisting must be qualified separately from management access.

## Development private audio on the VPS

The first Studio audio uploads use the **existing** filesystem driver, not Long Vân.
The VPS bundle installs `infra/vps/config/storage.filesystem.toml` once as
`/root/cantos-services/config/storage.toml`; existing operator content is **never
overwritten**. Only the Studio API Quadlet mounts that nonsecret TOML as
`/etc/cantos-storage.toml`, sets `CANTOS_STORAGE_CONFIG`, and grants access to
the bind-mounted host directory `/var/lib/cantos/private`. Container rootfs remains
read-only. The host directory is UID/GID `10001:10001` with mode `0700`; config
is root-owned mode `0644` under a root-only config directory. Startup refuses
missing, symlinked or incorrectly permissioned paths. This deployment is an
operator-facing configuration **option**, not proof that today's VPS has activated it.

The default installed profile is:

```toml
version = 1
[storage]
active_write_target = "legacy-filesystem"

[storage.targets.legacy-filesystem]
driver = "filesystem"
private_root = "/var/lib/cantos/private"
```

No S3 credential is necessary. The API admits only inspected immutable media
through its existing authenticated Studio intake and serves approved private reads
through the API; never expose the root through Caddy, static Hosting, public URLs,
an additional writable mount or Theatre. The Theatre API Quadlet has **no private
storage mount**. A release played by Theatre uses the separate
`CANTOS_PUBLICATION_ROOT` and playback-key configuration (#170), which this
change deliberately does **not** enable.

When Long Vân is ready, add `[storage.targets.longvan-hn1]` from the
[Long Vân example](../../services/cantos-api/config/storage.longvan.example.toml)
to that same TOML and choose `active_write_target = "longvan-hn1"`. **Retain**
`legacy-filesystem` with its original root for every historical audio receipt.
External secrets are supplied by operator-managed API EnvironmentFile/Podman
secrets, never in TOML or Git. The API validates **all** configured targets and
requires their secret inputs even if not active; do not add Long Vân until these
inputs and its real S3 capability acceptance are ready. There is **no fallback,
implicit migration, or duplicate upload to providers** when a target fails.

After the explicitly approved VPS deployment, verify the tracked Studio container
mounts and exact filesystem target, upload a test Ogg/Opus file through a real
authorized Studio session, read it back with byte/hash and Range checks, and confirm
a second user and anonymous users cannot fetch it. Repeat the read after a Studio
container restart and image redeploy/rollback without changing the original
`storage_target_id`. No provider credentials should be needed for this default.
These runtime checks are **NOT_RUN** until an actual VPS test receipt exists.

**Backup requirement:** the existing scheduled PostgreSQL `pg_dump` does **not**
include `/var/lib/cantos/private`. Back it up independently **off-VPS**, preserve
immutable file contents and ownership/modes on restore, and drill a receipt-to-byte
restore with the matching PostgreSQL snapshot. Do not delete or prune this directory
during deploy, rollback or image cleanup. Provision enough disk capacity and monitor
free space before large audio batches. Publication assets, when activated, need
a different root and independent backup policy. Tracking: [#309](https://github.com/loveoverflowcom/cantos/issues/309).

## Runtime assets, recovery and checks

Root-only `/root/cantos-services/` holds `config`, `secrets`, `internal-ca`, `kanidm`, `backups`,
`state` and installed `runtime`/`edge`. Quadlets live under `/etc/containers/systemd`; source assets
are in [infra/vps](../../infra/vps/README.md). Preserve volumes `cantos-postgres-data`,
`cantos-postgres-socket`, `cantos-kanidm-data`, `cantos-edge-data` and `cantos-edge-config`.
The edge publishes 80/443; host SSH is separately operator managed. Never prune persistent data.

API rollback: `cargo xtask deploy vps cantos-api --host root@42.96.18.245 --rollback` (remote write).
Rollback swaps the API image only after its epoch label and read-only schema check pass.
Restore requires exactly the dump and its `.schema-epoch` sidecar in the checksum manifest,
epoch 2 and a compatible current image/database, takes a `pre-restore` backup, stops both APIs
and restores in one transaction.
It checks the restored schema before applying grants/restarting; rejection leaves the APIs stopped.
Historical backups require their matching legacy bundle and a separately reviewed recovery plan,
not fabricated epoch metadata. Cross-epoch recovery follows the [cutover plan](postgres-schema-cutover.md).
For the narrow cutover, the old database, image/runtime and recovery journal remain retained;
before `accepted`, recovery restores them as a pair. After `accepted`, automatic recovery only
finalizes the new installation. Its off-host tar excludes OCI image bytes, Podman secrets,
the Kanidm administrator password and private CA. Retaining an old image on the VPS and copying
that tar does not replace independent image/secret/identity/storage/CA disaster recovery.
Ordinary deploy pruning validates every retained epoch-2 journal before any image/upload
deletion. It preserves each journal's target and previous images and whole incoming revision
directories, in addition to current/previous images and the three newest valid uploads.
This includes original flat and nested-attempt bundles. A malformed, unreadable or unknown
journal skips all release pruning with a warning. Image deletion additionally requires the
exact Cantos API title, matching revision and repository-source OCI labels; foreign/missing
labels and unknown/symlink incoming entries are left untouched. Retain the journals and their
dependencies until a separate retention decision.
Copy each finalized backup off the VPS regularly, keeping **all three siblings** together:
`FILE.dump`, `FILE.dump.schema-epoch` and `FILE.dump.sha256`. In the destination directory,
verify `sha256sum -c FILE.dump.sha256` before the separate restore drill. Copying only the dump
loses the metadata required by ordinary restore. The originals live on the same VPS disk.
Failed dump/checksum creation or unreadable schema metadata stops before retention or deployment.
`FILE.dump.sha256` is finalized only after recognized schema metadata and a successful checksum.
An unknown-epoch archive is retained with its metadata and partial checksum for investigation
and returns failure; it never establishes a usable recovery point. Retention counts only finalized
checksum manifests, so failed artifacts cannot displace good backups on a later run.
`.partial` files are incomplete evidence,
never usable backups. Kanidm keeps its own daily online backup
(7 versions) in `cantos-kanidm-data`.
Configuration rollback: the previous bundle stays at `/root/cantos-services/runtime.previous`.
Hosting rollback uses the selected site's console release history. A scheduled same-disk
backup is not disaster-recovery acceptance.

After an authorized change, open both direct Hosting URLs in a fresh browser session. Inspect
static assets, actual API request origins/status/body, loading/empty/error/retry and guest denial.
With provisioned test accounts, drive OIDC → API callback → session → reload → logout → rejected
old session; inspect Studio roster and Theatre-to-Studio denial. Draft creation needs explicit
production-data permission. Use real published releases for detail/playback; never seed production.
Reboot persistence, failed-health switch-back and native playback each need their own acceptance.

## Verified artifact promotion

`cargo xtask deploy vps cantos-api --host USER@HOST --smoke-origin https://api.example.org
--release-manifest PATH --release-digest SHA256 --release-sha FULL_SHA` consumes an already-built
manifest v2 release. Add `--release-request PATH` inside its trusted CircleCI workflow, or
`--local-artifact` outside CI after independently qualifying producer/source/provenance.
The [release contract](circleci-builds.md) owns expiry, byte/format limits, exact public inputs,
source/config identity and same-workflow trust; v1 and arbitrary dist inputs are refused.
`--dry-run` validates metadata and prints the operation without staging or remote calls.

Before a remote write, the client snapshots/rechecks actual bytes, validates the Docker archive
config ID, Linux amd64/source/schema label and layer diff IDs, verifies the matching runtime tar
and checks host architecture. It uploads only those bytes into a fresh incoming SHA/attempt
path, with transfer checksums. The artifact's own host script receives
`promote REVISION TREE IMAGE_ID` and holds one operation lock across checksum verification,
image load/identity checks, matching runtime install and the existing deploy function.
The host never compiles Rust. Pending cutovers, schema/browser drift and missing recovery
prerequisites retain their fail-closed behavior; no implicit bootstrap/identity/restore occurs.

Per-run private receipts are beneath `artifacts/vps-release/ATTEMPT/receipt.json`; they bind the
requested SHA, manifest digest, host/incoming path, image ID, reached stages, activation and
public smoke separately. A failed SSH response after activate begins is unknown, even if the
host finished. Inspect `--status`, retained incoming tree, state/history and health before a
new request; do not replay automatically. A public smoke failure after host success retains
known activation and a separate smoke gap. In a multi-component release it blocks Web.

Image switch-back does not undo DB migrations. Preserve forward-only migrations, epoch guards,
current/previous images, journals, backups and volumes. Existing additive-schema incompatibility
and controlled epoch-cutover procedures above still gate production acceptance. A matching
artifact is no proof that today's installed database or identity is compatible. This source
change performs no deployment; account/CI/build/health/browser acceptance needs named evidence.
