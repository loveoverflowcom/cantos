# Current production topology and operations

This is the single current production topology/runbook. [ADR 0041](../decisions/0041-direct-hosting-api-boundary.md)
selects direct Firebase Hosting and API-mediated browser requests; [ADR 0039](../decisions/0039-vps-production-runtime.md)
retains the VPS runtime/durability rules. Installed configuration is transitional. Recheck the
remote status and Hosting releases before every operation; this document is not a deployment receipt.

**Schema transition:** current source requires epoch 2. Ordinary deploy/rollback refuses the
previous epoch-1 database and images; it cannot convert the deployed installation. Read the
[controlled cutover runbook](postgres-schema-cutover.md) before packaging this source for a
live host. Inventory, backup/restore and approved data conversion remain separate from source
verification. PostgreSQL 17.11 stays pinned until a separate major-upgrade acceptance.

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

## Direct Hosting → API configuration

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
in `config/oidc-callbacks.env`. Only separately authorized `cantos-vps identity` writes that input
receipt after configuring the clients with the Web origins and API callbacks above. Existing
installations without it fail closed. Review the existing client origin, issuer, allowed redirects
and obsolete callbacks before reconciliation: identity's add-redirect operation does not delete
old provider redirects or update an existing client's display/origin metadata. The receipt records
configured inputs, not live discovery/login or exclusive provider allowlists. Do not hand-write
it as a shortcut around provider review.

Public anonymous reads can use web.app → DuckDNS after deployment. Authentication cannot be
accepted on those cross-site names with the retained Strict cookies. Supply reviewed same-site
custom domains/API alias and a minimal identity gateway as specified by ADR 0041 before activating
split-origin login. No real custom domain is configured by these instructions. A source change
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

## Identity: Studio administrator and Theatre sign-in (operator-run)

Existing accounts only for acceptance. `cantos-vps identity` changes accounts, clients, groups,
secrets, roster and API units: it requires separate explicit delegation naming host/accounts.
Deployment authorization does not grant it. Password/passkey/MFA setup belongs to the operator's
interactive browser. Never put credentials or reset links in chat, Git, screenshots or shared logs.
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
Copy each finalized backup off the VPS regularly, keeping **all three siblings** together:
`FILE.dump`, `FILE.dump.schema-epoch` and `FILE.dump.sha256`. In the destination directory,
verify `sha256sum -c FILE.dump.sha256` before the separate restore drill. Copying only the dump
loses the metadata required by ordinary restore. The originals live on the same VPS disk.
Failed dump/checksum creation or unreadable schema metadata stops before retention or deployment.
An unknown-epoch archive is retained with its metadata for investigation and returns failure;
it never establishes a usable recovery point. `.partial` files are incomplete evidence,
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
