# Kanidm opt-in evaluation runtime

Kanidm remains a **proposed production identity provider**. The API now implements an opt-in
[Studio OIDC BFF](../../docs/architecture/studio-oidc.md), with signed mocked-provider/local-SQL
evidence. Live account/group synchronization, client registration, Kanidm login and production
deployment remain NOT_RUN. The separate
[actor registry](../../docs/architecture/identity-registry.md) stores explicit external bindings
without authenticating Kanidm. The API retains development sessions when OIDC is disabled;
this infrastructure change grants no Theatre/Admin authentication capability.
[ADR 0021](../../docs/decisions/0021-podman-service-runtime.md) standardizes the container runtime,
not the identity decision. [ADR 0009](../../docs/decisions/0009-monorepo-layout.md) fixes its location.

The [manifest](../manifest.toml) pins official `docker.io/kanidm/server:1.10.5`, matching the
previous example's chosen release. A historical throwaway `configtest` was reported for that
release; it is not fresh runtime evidence for this PR. The current layout separates read-only
`/config` from persistent `/data`. No Rust crate or custom Containerfile is needed.

## Operator prerequisites

Use an already installed rootless Podman connection (Linux or running macOS Podman Machine).
Prepare a private directory **outside Git**, caller-owned mode 0700, containing only the intended
`server.toml`, `chain.pem` and `key.pem` (regular caller-owned files, no group/other permissions).
xtask never creates certificates, recovers admin accounts or changes trust/security settings.
Private mounts do not relabel files; enforcing SELinux needs preapproved readable input labels.
On macOS the directory must already be shared with the Machine. Export the
[host environment example](env.example); xtask does not load dotenv files.

Copy [server.toml.example](server.toml.example) and set domain/origin deliberately before first
start. Origin must use HTTPS, match the domain and chosen host port. Keep the fixed internal
bind, database and TLS paths. This small evaluation contract rejects extra configuration keys;
production configuration needs its own reviewed runtime rather than this command.

Supply a valid leaf certificate plus issuing chain in `chain.pem` and its matching private key.
For this local health contract the leaf needs both the chosen identity hostname and
`localhost` in its DNS SANs. The pinned `kanidmd scripting healthcheck --verify-tls` connects
to `localhost:8443`; upstream loads issuing certificates after the leaf as trust roots.
A lone self-signed leaf is insufficient for that verified health probe. Certificate issuance,
trust installation and key permissions are operator work, not an implicit task action.
Never commit key/cert contents or identity data.

## Explicit lifecycle and validation

```sh
export CANTOS_KANIDM_CONFIG_DIR=/absolute/private/cantos/kanidm
export CANTOS_KANIDM_PORT=8443
cargo xtask services up kanidm
cargo xtask services status
cargo xtask services logs kanidm
cargo xtask services down kanidm
cargo xtask services up kanidm
```

`up --all` excludes optional Kanidm;
`down --all` may stop an already-owned Kanidm container. Startup first statically validates the
private configuration, then runs `/sbin/kanidmd configtest -c /config/server.toml` in a newly
created labeled, network-disabled throwaway container with tmpfs `/data`. It never validates
against an existing identity database. `configtest` may initialize temporary data; its success
proves upstream configuration acceptance, not TLS serving, identity integration or backup safety.

The actual server uses `/sbin/kanidmd server -c /config/server.toml`, a named
`cantos-<instance>-kanidm-data` volume, and a host mapping only on `127.0.0.1`. Health uses the
pinned scripting healthcheck with TLS verification. Upstream 1.10.5 establishes an HTTPS
connection to `/status` but does not check the returned status/body; report that bounded
readiness, never end-to-end identity acceptance. Podman bounds each health probe and xtask
bounds startup. Failed startup preserves the server container/volume for explicit inspection.

`down` preserves container/volume. Restart refuses image/port/config drift; it never changes an
existing domain, silently upgrades data or deletes identity state. Server initialization may run
Kanidm's own first-start internal migrations on **its new volume**; Cantos never calls domain
rename, raise, remigrate, vacuum or restore. Backups, safe upgrades, MFA/passkeys, Studio/Theatre
OAuth2 scopes and production recovery need a separate ADR/work item.

Sources: [upstream configuration](https://kanidm.github.io/kanidm/stable/server_configuration.html),
[pinned CLI options](https://github.com/kanidm/kanidm/blob/v1.10.5/server/daemon/src/opt.rs),
[pinned health implementation](https://github.com/kanidm/kanidm/blob/v1.10.5/server/daemon/src/main.rs).

## Studio OIDC operator boundary

[ADR0023](../../docs/decisions/0023-studio-oidc-bff.md) and the
[credential/session architecture](../../docs/architecture/studio-oidc.md) record exact policy.
This directory still contains no identity-provider code. The API integration never starts Kanidm,
recovers admin, creates grants/credentials, persists a secret or changes TLS/network permissions.

```sh
cargo xtask kanidm reconcile --issuer https://idm.example/oauth2/openid/cantos-studio-web-bff \
  --web-origin https://studio.example --group cantos_creators --dry-run
cargo xtask kanidm verify --issuer https://idm.example/oauth2/openid/cantos-studio-web-bff \
  --web-origin https://studio.example --dry-run
```

Reconcile prints argument arrays for operator review only, always `NOT_RUN` mutation. It does
not execute a shell or fetch a credential. Compare current provider configuration before using
that plan; it does not automatically repair an existing client or prove exact redirects/scopes.
The optional executed `verify` explicitly reads discovery/JWKS through the real API adapter,
without client authentication/user data. Success proves metadata shape/keys only; login, disable,
logout and production remain NOT_RUN. It uses normal TLS validation and refuses cross-origin
endpoints, redirects, remote HTTP and credentials embedded in URLs.

Before any live activation the operator must separately authorize and prepare the confidential
client, exactly one reviewed Studio callback, an appropriate creator group/openid scope mapping,
S256 enforcement, ES256 signing, trusted TLS and server-side secret delivery. Never reuse this
client for Theatre or Admin, disable PKCE, enable legacy signing, bypass certificate warnings or
silently enroll users. See [official Kanidm OAuth2 policy](https://kanidm.github.io/kanidm/stable/integrations/oauth2.html).

Opt-in local server configuration is `CANTOS_STUDIO_OIDC_ISSUER`, operator-delivered
`CANTOS_STUDIO_OIDC_SECRET` and exact `CANTOS_WEB_ORIGIN`. Secrets never belong in Git, command
arguments, browser storage or logs. The host still requires development/loopback/local PostgreSQL;
there is no production activation flag. Visit `/api/v1/studio/auth/login`; the existing Studio
current-session check can discover the cookie after the callback. The token-entry UI is unchanged;
in OIDC mode development-token POST login is forbidden. If an old/expired cookie blocks login,
use same-origin DELETE `/api/v1/studio/session` before starting again.

After migration0020, the operator may review/apply these narrow database grants to the existing
HTTP role. No role/user is provisioned by migration or tooling:

```sql
GRANT SELECT ON actors,sessions,studio_oidc_sessions TO cantos_app;
GRANT UPDATE ON sessions TO cantos_app;
GRANT SELECT,INSERT,UPDATE ON studio_oidc_flows TO cantos_app;
GRANT UPDATE(revocation_pending) ON studio_oidc_sessions TO cantos_app;
GRANT EXECUTE ON FUNCTION
  studio_oidc_finish(bytea,text,text,text,text,bytea,bytea,text,bigint),
  production_lock_actor(text) TO cantos_app;
```

No direct registry read/write, actors INSERT/UPDATE, sessions INSERT or DDL is granted.
The callback's private verifier witness is the credential gate; the trusted runtime SQL capability
is not exposed to a browser. Private provider access tokens are retained for introspection and
revoke; this local slice stores them as text. Review encryption/backup permissions, retention,
rate limits and pending-revocation recovery before any production deployment.

Logout commits local revocation before attempting provider revocation, then clears current and
known legacy Studio cookies in the returned response. Inspect
`Cantos-Provider-Revocation: completed|pending` and the retained pending row; an unavailable provider
never restores local authority. Provider SSO/global logout is not implemented. A controlled retry
with the same token can finish pending access revocation; abandoned rotation/logout records need
an operator recovery policy before production. Do not clear evidence to make a run appear clean.

Live acceptance checklist (all currently NOT_RUN): real login and exact client redirects; wrong
issuer/audience/nonce/alg/signature; key rotation/outage; account disable and token revocation;
local logout versus provider SSO; HTTPS browser cookie scope/namespace/CSRF; production secret/TLS
and revocation recovery policy. Mocked results must never be relabeled Kanidm-live evidence.
