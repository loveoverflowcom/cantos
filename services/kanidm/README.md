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

[ADR0023](../../docs/decisions/0024-studio-oidc-bff.md) and the
[credential/session architecture](../../docs/architecture/studio-oidc.md) record exact policy.
This directory still contains no identity-provider code. The API integration never starts Kanidm,
recovers admin, creates grants/credentials, persists a secret or changes TLS/network permissions.

```sh
cargo xtask kanidm reconcile --issuer https://idm.example/oauth2/openid/cantos-studio-web-bff \
  --web-origin https://studio.example --group REVIEWED_STUDIO_ADMIN_GROUP --dry-run
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
client, exactly one reviewed Studio callback, a dedicated preprovisioned Studio-admin group/openid
scope mapping,
S256 enforcement, ES256 signing, trusted TLS and server-side secret delivery. Never reuse this
client for Theatre or Admin, disable PKCE, enable legacy signing, bypass certificate warnings or
silently enroll users. See [official Kanidm OAuth2 policy](https://kanidm.github.io/kanidm/stable/integrations/oauth2.html).

Opt-in local server configuration is `CANTOS_STUDIO_OIDC_ISSUER`, operator-delivered
`CANTOS_STUDIO_OIDC_SECRET`, exact `CANTOS_WEB_ORIGIN` and required nonsecret
`CANTOS_STUDIO_ADMIN_SUBJECTS` (a JSON array of one or two exact, distinct Studio OIDC subjects).
Secrets never belong in Git, command arguments, browser storage or logs. The host still requires
development/loopback/local PostgreSQL;
there is no production activation flag. Studio reads the bounded current-mount
`/api/v1/studio/auth/configuration`, confirms the actual cookie with GET `/api/v1/studio/session`,
and shows the provider sign-in control in OIDC mode. That control performs same-origin DELETE
`/api/v1/studio/session` before navigating, so expired/legacy cookies can recover. Development
mode retains the token-entry form; OIDC mode forbids development-token POST login. The configuration
DTO contains only the mode and cannot mint authority. Direct operators may still visit the login
route after explicit local logout. Migration0025 repairs final expiry checks after SQL lock waits and adds scoped browser revocation
attribution without changing0020's checksum; the narrow column grant below is required.

After migrations0020 and0025, the operator may review/apply these narrow database grants to the existing
HTTP role. No role/user is provisioned by migration or tooling:

```sql
GRANT SELECT ON actors,sessions,studio_oidc_sessions TO cantos_app;
GRANT UPDATE ON sessions TO cantos_app;
GRANT SELECT,INSERT,UPDATE ON studio_oidc_flows TO cantos_app;
GRANT UPDATE(revocation_pending,revoked_by_token_hash) ON studio_oidc_sessions TO cantos_app;
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

## Studio administrator provisioning contract

[ADR 0032](../../docs/decisions/0032-theatre-signup-and-provisioned-studio.md) fixes Studio to
one or two administrator accounts provisioned by an authorized operator, with no public Studio
registration. The accounts administer Studio content within existing resource permissions;
they receive no implicit `/api/v1/admin` capability, provider administration or cross-owner access.
Theatre self-registration grants none of those permissions.

Provisioning is a separate, explicitly authorized operation. Before it can run, record the
target environment/issuer, one or two intended account identities, the reviewed Studio group
and requested access scope. Never request a password in chat. Do not reuse credentials from
conversation history, hardcode a password/token or expose provider administration in a public
signup handler. Use the provider's existing approved secure enrollment/recovery channel and
server secret-delivery channel; their live availability must be verified by the operator.

The bootstrap sequence is an operator contract, not an executed setup script:

1. Verify the existing issuer/client and inspect current account/group configuration read-only.
2. Obtain specific approval before creating accounts, credentials, group membership, client
   grants or permanent access. The uppercase group in the dry-run example is a placeholder;
   replace it with the actual reviewed group rather than creating a guessed group.
3. Securely provision the approved one or two accounts and restrict the dedicated Studio
   client's `openid` mapping to their reviewed group. Keep the existing confidential client,
   exact redirect, S256, signing and TLS requirements. Do not grant Theatre's signup group
   access to the Studio client.
4. Read the accounts' exact subjects for the pinned **Studio** issuer through an authorized
   provider channel. Configure `CANTOS_STUDIO_ADMIN_SUBJECTS` as a JSON array containing only
   those subjects, then restart the development host with the existing separately delivered
   Studio client secret. This roster is not a password, actor ID, email match or role claim.
5. Accept activation only after real login for each listed account, denial for an unlisted
   Theatre account, per-request refusal after roster removal, provider disable, logout and
   the existing cookie/CSRF/recovery checks. Restore only explicitly approved configuration.

The API validates roster count, distinctness and identity syntax at startup. It checks exact
issuer/subject admission before session issuance and on each authenticated request. Missing
roster denies login; malformed/partial host configuration fails startup. Reconfiguring the
roster does not delete old identity/session evidence or durably revoke removed credentials;
explicit existing revocation controls own that operation. The first admitted callback may
create the internal actor binding atomically, but cannot provision a provider account or grant.
No live step above has run as part of the implementation. The development-token fixture path
remains local-only under the current host restrictions; it is not an account-provisioning path.

## Theatre identity PR3 (operator preparation remains separate)

The accepted [Theatre self-registration policy](../../docs/decisions/0032-theatre-signup-and-provisioned-studio.md)
does not enable an issuer operation. The pinned1.10.5 source review finds no supported public
enrollment entrypoint for the current adapter: `/ui/enrol` is authenticated device enrollment,
and person creation requires administrative authorization. The public Web
[`GET /api/v1/theatre/auth/configuration`](../../packages/contracts/theatre-identity-v1.md#registration-policy-and-implemented-capability)
therefore returns `registration.status=unavailable`, with `identity_unconfigured` before Web BFF
configuration or `provider_unsupported` after it. No registration URL or password/provisioning
bridge is inferred from OIDC. Actual self-registration remains blocked on an approved supported
integration; no live issuer configuration/account/grant was inspected or changed by these tests.

The opt-in development host accepts `CANTOS_THEATRE_OIDC_ISSUER` and
`CANTOS_THEATRE_OIDC_SECRET` only as an explicit pair, independent of Studio. Issuer must end in
`/oauth2/openid/cantos-theatre-web-bff`; scope is only `openid` and callback is the exact configured
Web origin plus `/api/v1/theatre/auth/callback`. No live registration, grant, secret, account,
container, certificate or network change is performed by implementation/tests. The current xtask
Kanidm reconciliation plan remains Studio-specific; no Theatre live provisioning is implied.

For an operator-reviewed Theatre deployment, runtime needs SELECT on `actors`,
`theatre_identity_sessions`; SELECT/INSERT/UPDATE on `theatre_identity_flows`;
UPDATE(revoked,revocation_pending,revoked_by_token_hash) on `theatre_identity_sessions`; EXECUTE on
`theatre_identity_finish(bytea,text,text,text,text,bytea,bytea,text,bigint)`,
`listener_actor_active(text)` and `listener_lock_actor(text)`, plus the existing explicitly reviewed
Library/Progress grants. It receives no registry SELECT or actor/session INSERT privilege.
These are documentation/test grants, not an executed persistent role configuration.

Native `cantos-theatre-native` has an opt-in public-client code path with exact callback
`com.cantos.theatre:/oauth2/callback`; no Studio/Web secret enters the app. Bounded token exchange,
access-token signature/purpose verification, current introspection, exact registry issuance and
separate opaque native sessions are implemented; scoped local evidence is recorded in the
[resumed Theatre ledger](../../docs/evidence/theatre-identity.md#resumed-native-identity-acceptance--14-october-2026).
Runtime needs SELECT on `actors`/`theatre_native_sessions`, UPDATE(revoked,revocation_pending) on native
sessions, SELECT/INSERT/UPDATE on native flows, and EXECUTE on
`theatre_native_finish(uuid,text,text,text,bytea,bytea,text,bigint,uuid)` plus the existing Listener
lock/Library grants. These are reviewed deployment requirements, not live role changes.
iOS host code remains opt-in; the canonical local iOS framework gate is UNSUPPORTED without the
required installed Kotlin/Native 2.2.20 compiler. The user-authorized Android manifest and
Application/MainActivity source is mounted and its canonical mock Android build passed. This
source integration installed no app and changed no OS/device network or TLS setting. Provider
public-client registration/scopes, OS storage/system-browser/device/lifecycle acceptance,
live TLS/credentials and production activation remain NOT_RUN/closed. Do not issue Web BFF
sessions as native bearer tokens. See
[Theatre architecture](../../docs/architecture/theatre-identity.md) and
[handoff](../../docs/work-plan/theatre-identity-handoff.md).

## Admin resource contract (opt-in; no live acceptance)

Admin uses a **separate** fixed `cantos-admin-api` resource client and exact issuer, not Studio,
Theatre, `idm_admins`, an operator token or a producer grant. The
[policy](../../docs/architecture/admin-authorization.md) requires both a signed dedicated
`cantos_admin_groups` claim and a current authenticated introspection observation of that same
claim. Allowed values map only to `auditor` and `identity_operator`. Missing/unknown/stale/current
role removal fails closed; local Admin block is authoritative regardless of the provider.

A default provider configuration is **not sufficient**: no live evidence establishes custom
claim freshness in Kanidm introspection. This PR implements the actual verifier and refusal path,
not an OAuth client/claim/grant bootstrap or end-to-end login. Do not enable it by merely copying a
sample secret/claim map or treating token-time groups as current membership. An authorized provider
acceptance task must establish real current-group removal, disable, expiry, wrong issuer/audience/
keys and rotation before activation; otherwise leave Admin unavailable. No live credentials or
security settings are created by this slice.

For an operator-prepared local test deployment only, migration0022 requires explicit EXECUTE on
`admin_identity_facts(text,text,text)`, `admin_append_audit(uuid,text,text,text,text,uuid)`,
`admin_operation(text,uuid)` and `admin_revoke_access(text,uuid,text,uuid)` to the runtime role.
Do not grant direct INSERT/UPDATE/DELETE/TRUNCATE or SELECT on Admin tables or the registry. These
functions are privileged effects, not an authentication mechanism. Audit outage denies success;
there is no local block deletion/unblock routine. This documentation grants no account rights.
