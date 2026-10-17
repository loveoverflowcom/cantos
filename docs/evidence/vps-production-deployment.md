# VPS production deployment record — 17 October 2026

Scope: [ADR 0039](../decisions/0039-vps-production-runtime.md) on branch
`feat/vps-production-deploy`. Operator: the repository owner's machine (Ubuntu, Podman 5.7.0,
firebase-tools 15.34.0, Rust 1.98.1) against VPS `42.96.18.245` (Debian 13, 1 vCPU, 2 GB RAM,
Podman 5.4.2, ufw active) and Firebase project `cantos-a348b`. Commands are in the
[runbook](../deployment/vps-production.md). Vocabulary follows the
[engineering skill](../../.agents/skills/cantos-engineering/SKILL.md).

## Deployed

| Component | Identity | Observation |
| --- | --- | --- |
| API image | `localhost/cantos-api:883e188095227f04fb92cf4dcd29c659ac9de1fe` (current), `86b59ebc…` (previous), 124 MB | both Web-origin processes healthy, uid 10001, read-only rootfs, no capabilities |
| PostgreSQL | `postgres:17.11-bookworm`, socket-only | 28 migrations applied by `cantos_owner`; runtime grants applied to `cantos_app` |
| Kanidm | `kanidm/server:1.10.5`, domain `idm.cantoss.duckdns.org` | healthy (exec-form TLS health check); no OAuth2 client or person exists yet |
| Edge | `caddy:2.11.7-alpine` | Let's Encrypt certificates for all four names (issuer YE2, expiry 2027-01-14) |
| Theatre Web | Firebase site `cantoss-theatre`, version `6d7901291523b61d` (develop `8786bdc` incl. #230; first release `2e3a3c7af91ba380`) | xtask `deploy firebase … --channel live` PASS, HTTP smoke 8/8 |
| Studio Web | Firebase site `cantoss-studio`, version `ce75f35962351a39` (develop `8786bdc` incl. #230; first release `024706b2a5c7b810`) | xtask `deploy firebase … --channel live` PASS, HTTP smoke 10/10; the DuckDNS name serves the identical bundle |

## Evidence ledger

| Claim | Level | How |
| --- | --- | --- |
| Production host mode is a closed set; production needs explicit bind and exact HTTPS origin and refuses dev-only inputs | example-tested | `platform::host` unit tests (5) |
| A production router offers no development Studio credential and serves no static files | example-tested | `interfaces::http` tests with an unreachable pool: config 503, token login 403 (current and legacy path), dev cookie 401 before SQL, static file 404 |
| `runtime-grants.sql` alone serves real Studio create/list and anonymous Theatre discovery, is idempotent and grants nothing on Admin/registry/legacy/registration/native tables, DDL, TRUNCATE or actor/session INSERT | integration-tested | `production_grants_postgres` against PostgreSQL 17.11 in a disposable container (no host `initdb`); non-vacuity: the same request fails before the grants |
| Deploy plan builds locally before any remote write; only host-script calls remotely; unsafe `--host`/origins refused | example-tested | `deploy::vps` unit tests (6) |
| Host script | statically-checked | `bash -n`, ShellCheck 0.10.0 at warning level |
| Caddyfile | statically-checked | `caddy validate` with the pinned image |
| Quadlet units | statically-checked | `quadlet -dryrun` on the VPS (Podman 5.4.2) |
| Bootstrap, deploy, redeploy, rollback both ways | integration-tested | real runs below |
| Public HTTPS, routing, headers, closed ports | integration-tested | `curl` from the operator machine over the Internet |
| Both Web apps load from the custom names and call the real API | DOM-tested (network panel) | built-in browser: Theatre requests `/api/v1/theatre/works` → 200 (empty catalog state shown); Studio requests `/api/v1/studio/auth/configuration` → 503 and shows "Chưa thể xác nhận phiên Studio" |
| Backup restores | integration-tested | checksum verified; latest dump restored into a scratch database (28 migration rows, 94 tables), scratch database dropped |
| Workspace format, lint and tests after the change | example-tested | `cargo xtask verify rust`: rust-format, rust-lint, rust-tests PASS |
| Existing Studio session/OIDC, catalog HTTP and revision behavior unchanged by the development-session switch | integration-tested | `studio_oidc_postgres` 20, `catalog_http_postgres` 52 (restart case skipped as in the gate), `revisions_postgres` 21, all PASS on disposable PostgreSQL 17.11 |
| Repository hygiene | statically-checked with a baseline failure | `check_repository.py` reports only the 22 Leptos/CMP preview-qualification findings that `origin/develop` already has; preflight therefore stops at its first gate, and the gates above ran individually |
| CircleCI pipeline | statically-checked | `circleci config validate` and `config process`: empty target → no workflow; `api` → only API; `web` → both Web workflows |

## Runs

1. `--bootstrap` (several idempotent reruns while fixing, each fix committed):
   - `podman secret create -` refuses a redirected file → secrets are piped;
   - Kanidm's shell-form health command failed (image has no `/bin/sh`) → exec form;
   - Caddy could not reach ACME: ufw's routed policy dropped bridge traffic and its input policy
     dropped aardvark-dns → `route allow in/out on cantos0` and DNS on `cantos0` (bridge name
     pinned with `--interface-name`); the network was recreated once, before any application data;
   - a file bind mount hid the replaced Caddyfile from `caddy reload` → the edge directory is mounted.
2. First deploy `86b59ebc…`: backup (empty DB), migrations, grants, health gate, edge probe, public
   smoke `200 https://api.cantoss.duckdns.org/api/v1/theatre/works`.
3. Second deploy `883e1880…` (1 min 48 s end to end with a warm build cache): `previous` recorded,
   432 KiB pre-deploy backup, edge configuration reloaded.
4. `--rollback` to `86b59ebc…` then `--rollback` back to `883e1880…`: both health-gated, smoke 200,
   `state/history` records both.

Observed over HTTPS after the final run:

| Request | Result |
| --- | --- |
| `GET https://api.cantoss.duckdns.org/api/v1/theatre/works` | 200 `{"items":[],"next_cursor":null}`, `Cache-Control: no-store`, `Referrer-Policy: no-referrer` |
| `GET https://cantoss.duckdns.org/` and the `_bg.wasm` it names | 200 `text/html`, `no-cache`; 200 `application/wasm` (2.8 MB) |
| `GET https://studio.cantoss.duckdns.org/` | 200, `X-Frame-Options: DENY`, HSTS |
| `GET https://studio.cantoss.duckdns.org/api/v1/studio/auth/configuration` | 503 `unavailable` (no Studio OIDC yet) |
| `POST /api/v1/studio/session` with a dev token, correct Origin | 403 |
| dev cookie on `/api/v1/studio/works` | 401 |
| Studio mutation with `Origin: https://evil.example` | 403 |
| Studio-origin mutation sent to the Theatre host | 403 (origin pinned per process) |
| `GET https://idm.cantoss.duckdns.org/status` | 200 `true` |
| `http://api.cantoss.duckdns.org/` | 308 to HTTPS |
| TCP 5432, 8080, 8443, 2019 from the Internet | closed |
| `idm.cantoss.duckdns.org` from inside an API container | resolves to the edge on `cantos0`, TLS verified, 200 |

## Not accepted (NOT_RUN or blocked)

| Gate | Status | Needs |
| --- | --- | --- |
| Studio OIDC sign-in, per-request introspection, logout, roster refusal | **blocked on the owner** | `cantos-vps identity --studio-admin NAME` run by the owner, then a credential set through the printed Kanidm link; account creation and credentials are not an agent action |
| Theatre Web sign-in and Library | **blocked on the owner** | the same step with `--listener NAME` |
| Reboot persistence | NOT_RUN | units are wired into `multi-user.target` (observed); a reboot needs the owner's go-ahead |
| Automatic switch-back after a failed health gate | NOT_RUN (code path) | rollback itself ran; a failing image drill is pending |
| Off-host backup copies | NOT_RUN | owner-chosen destination |
| CircleCI pipeline run | NOT_RUN | `production` context variables, dedicated deploy key, Firebase service account |
| Firebase preview channels | NOT_RUN | not needed for this deployment |
| PostgreSQL gate (`cargo xtask verify postgres`) | UNSUPPORTED on this machine (no `initdb`); the new suite ran in a container as above | CI `postgres` job |
| Object storage, publication delivery, Theatre registration, native identity, Admin | out of scope | separate reviewed grant blocks and approvals |

Residual risks: the VPS is a single host and the browser's first hop for both Web apps; backups
share its disk until copied off; `cantos-vps identity` was exercised piecewise against a disposable
local Kanidm 1.10.5 (CLI exit codes and output shapes), not end to end on the VPS.
