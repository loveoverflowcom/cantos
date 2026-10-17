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
Secrets never belong in Git, command arguments, browser storage or logs. A production host
(`CANTOS_ENV=production`, [ADR 0039](../../docs/decisions/0039-vps-production-runtime.md)) takes the
same inputs from its OIDC drop-in, Podman secret and env file written by the operator-run
`cantos-vps identity`, which also creates the clients, groups and people described below; the
development host keeps loopback/local PostgreSQL. Studio reads the bounded current-mount
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

[ADR 0034](../../docs/decisions/0034-theatre-signup-and-provisioned-studio.md) fixes Studio to
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

The accepted [Theatre self-registration policy and onboarding direction](../../docs/decisions/0034-theatre-signup-and-provisioned-studio.md)
retain Kanidm. The pinned1.10.5 public UI has no integrated signup, but authenticated person
creation and credential-update-intent APIs exist. The approved implementation wraps those
operations in a Theatre-owned email-verification workflow; it never collects passwords or
infers an anonymous signup URL from OIDC. The public Web
[`GET /api/v1/theatre/auth/configuration`](../../packages/contracts/theatre-identity-v1.md#registration-policy-and-implemented-capability)
reports mounted login and registration capability independently. Missing registration setup
remains unavailable; an enabled local adapter is not live issuer/mail acceptance. No live
issuer configuration/account/grant was inspected or changed by implementation/tests.

The opt-in development host accepts `CANTOS_THEATRE_OIDC_ISSUER` and
`CANTOS_THEATRE_OIDC_SECRET` only as an explicit pair, independent of Studio. Issuer must end in
`/oauth2/openid/cantos-theatre-web-bff`; scope is only `openid` and callback is the exact configured
Web origin plus `/api/v1/theatre/auth/callback`. An HTTPS issuer with a private authority (a local
acceptance Kanidm) additionally needs `CANTOS_THEATRE_OIDC_CA_FILE`: an absolute, regular PEM file of
at most 64 KiB, not group/other writable, holding only `CERTIFICATE` blocks (one to eight). Those
anchors **replace** the public roots for this issuer's OIDC and registration/onboarding clients only;
no other client gains them, validation is never disabled, a plain-HTTP issuer or a file without an
issuer is refused, and nothing is installed into a system trust store. No live registration, grant, secret, account,
container, certificate or network change is performed by implementation/tests. The current xtask
Kanidm reconciliation plan remains Studio-specific; no Theatre live provisioning is implied.

### Theatre registration activation runbook — approval required before execution

This is a review/runbook contract, not an executed bootstrap. The product owner's approval
covers code and isolated tests only. Do not create a service principal, account, bearer,
credential/key, group/ACP/client grant, sender or SMTP setting from these instructions without
specific approval for the concrete environment, identities and authority.

1. Identify the exact environment and pinned Theatre Web issuer/client, operator/security and
   acceptance owner, allowed test mailbox/person scope, permitted mail volume/cost and rollback
   owner. Inspect existing configuration read-only. Keep the Studio confidential client and
   exact one/two-subject roster separate; a Theatre signup must not change either.
2. Review the real Kanidm1.10.5 permission plan. The adapter needs person creation with a
   preselected random UUID/name/mail and credential-update intent for that newly acknowledged
   UUID. It does not require existing-person discovery/reset, arbitrary person modification,
   deletion, group management or Studio/Admin grants. However, the built-in
   `idm_people_on_boarding` role also permits resets of existing non-high-privilege people.
   **Do not grant it merely because its name sounds narrow.** Validate the actual ACP/target
   rules and protection of every pre-existing Theatre/Studio account. Do not assume a custom
   target filter is supported. If the issuer cannot enforce the reviewed boundary, document
   residual bearer-compromise blast radius and obtain a concrete decision before activation.
3. Review the remaining deployment policies: external existing-email behavior; bounded ASCII
   parsing/case-insensitive deduplication and operation-derived names; verification/setup TTLs,
   recipient cooldown/global/network budgets; direct-peer or trusted-proxy behavior; PII/quota
   receipt retention, backup/access protection and cleanup; stalled-operation recovery. Existing
   mailbox equality never authorizes a reset or cross-client identity link. Constructor ranges
   and fixture values are implementation bounds, not accepted production policy.
4. Present the exact principal/grants, secret storage/rotation/revocation and mail sender/domain/
   transport/recipient plan for specific user approval. Use secure secret delivery, not chat,
   issue, command arguments or Git. No provider/email vendor or paid service is selected here.
   Deliver the provider bearer only server-side; registration must remain off until the plan
   and activation gates are satisfied.
5. After approval, provision only the authorized resources and configure the explicit host
   inputs. Validate migration0027 and narrow DB grants on a disposable environment first.
   Runtime requires SELECT/INSERT/UPDATE on `theatre_registrations` and SELECT/INSERT on
   `theatre_registration_mail_starts`; no DELETE/TRUNCATE, registry, Studio or provider authority
   follows from those grants. Current triggers retain the rows; there is no age-based cleanup
   implementation. A reviewed retention/cleanup policy may require an additional migration.
   The migration itself provisions no role or provider identity.
6. Exercise the approved test scope against real Kanidm: positive new-person acknowledgement,
   conflict/403/timeout/malformed response, intent expiry and protection of existing/Studio
   people. A lost create or intent acknowledgement is uncertain, not permission to retry or
   search by email and reset. Inspect the exact durable operation/provider state before any
   separately authorized recovery. Do not clear/delete/reset an old person to repair signup.
7. Accept mail in separate stages: real SMTP response, inbox arrival, mailbox confirmation,
   setup-link mail arrival, user-established credential, existing OIDC callback and API-confirmed
   Theatre session. Include expiry/replay/cancel/resend/outage, reload/logout and direct Studio
   denial. A successful SMTP DATA response is not inbox delivery or completed registration.
   Record Web evidence separately from unrun native/device acceptance; keep #174/#191 open
   until their own acceptance gates are met.

#### Explicit host inputs

[`config::from_lookup`](../cantos-api/src/platform/theatre_registration/config.rs) composes
registration without discovery, a provider call, SMTP connection or bootstrap. By default all
registration/SMTP inputs are absent and the module is off. An explicit `ENABLED=false` is also
off **only when every other registration/SMTP input is absent**. Partial/orphan configuration,
an unsupported declaration or a missing Theatre Web identity adapter fails startup. A disabled
module reports `registration_unconfigured` with Web OIDC configured, or
`identity_unconfigured` without it; its POST routes return 404.

All suffixes in this table use the exact `CANTOS_THEATRE_REGISTRATION_` prefix. Every listed
field is required when `ENABLED=true`; there are no policy-number, bearer or HMAC-key defaults.
The accepted ranges below are constructor bounds, **not recommended production settings**.

| Suffix | Required meaning / accepted bound |
| --- | --- |
| `ENABLED` | Exact `true` to request composition; absent or standalone `false` leaves it off. |
| `POLICY_APPROVED` | Exact `true` operator declaration that the candidate email/naming/abuse/retention/recovery policies were reviewed for this environment. |
| `ISSUER_ACCESS_APPROVED` | Exact `true` operator declaration after reviewing the concrete principal, grants and existing-account target protection. This is not proof of effective issuer permissions. |
| `PEER_POLICY` | Exact `direct_peer`; no forwarded-header or trusted-proxy mode is implemented. |
| `TOKEN` | Separately delivered server-only Kanidm service bearer; never a Theatre user credential, CLI argument or repository fixture. |
| `NETWORK_KEY` | Separately delivered nonzero 32-byte HMAC key encoded as 64 lowercase hexadecimal characters; protect and review rotation because it partitions personal quota metadata. |
| `VERIFICATION_SECONDS` | Explicit mailbox-proof TTL, 60–3600 seconds. |
| `RESEND_SECONDS` | Per-registration cooldown, 30 seconds through the configured proof TTL. |
| `WINDOW_SECONDS` | Durable accepted-verification-mail budget window, 30–86400 seconds. |
| `MAX_GLOBAL_REQUESTS`, `MAX_NETWORK_REQUESTS` | Durable budget limits: global 1–10000; per peer 1 through the global value. Failed/uncertain initial mail still consumes an accepted start. |
| `SETUP_SECONDS` | Explicit Kanidm credential-intent lifetime, 300–86400 seconds; the adapter refuses a value the pinned issuer would silently extend. |
| `MAX_INFLIGHT` | Per-process simultaneous registration operations/mail work, 1–32. |
| `ATTEMPT_WINDOW_SECONDS` | Process-local rolling admission window, 1–86400 seconds. |
| `ATTEMPT_MAX_GLOBAL`, `ATTEMPT_MAX_NETWORK` | Process-local admission limits: global 1–10000; per peer 1 through the global value. These are separate from durable mail-start budgets. |

The host must already have its separate `CANTOS_THEATRE_OIDC_ISSUER`/
`CANTOS_THEATRE_OIDC_SECRET` Web BFF pair. Registration requires HTTPS and the exact existing
`/oauth2/openid/cantos-theatre-web-bff` issuer path; it derives provider endpoints from that
origin rather than accepting another admin URL. `CANTOS_WEB_ORIGIN` supplies the fixed Web
origin and verification-link destination. The SMTP fields below are also required for enabled
composition. Neither approval declaration creates authorization or proves issuer-side least
privilege, SMTP delivery, inbox access or production readiness. Do not set them merely because
isolated tests pass; retain the independently reviewed operational record and specific approval.

All three registration POSTs require socket `ConnectInfo`; a missing peer fails closed with
503. Only the actual peer address, with IPv4-mapped IPv6 normalized, selects the HMAC partition.
`Forwarded`/`X-Forwarded-For` never select it. A reverse proxy is therefore the peer for this
implementation; end-user ingress/proxy policy must be reviewed before deployment. Rolling
attempt counters and concurrency slots are process-local, reset on restart and are not shared
across hosts. PostgreSQL mail-start budgets persist and serialize across connections/processes
sharing the issuer namespace. Neither layer establishes deployment-wide ingress protection.

The vendor-neutral SMTP adapter uses explicit
`CANTOS_THEATRE_REGISTRATION_SMTP_HOST`, `_PORT`, `_TLS` and `_FROM` inputs under the shared
`CANTOS_THEATRE_REGISTRATION_SMTP_` prefix. Optional `_USERNAME` and `_PASSWORD` must be supplied
as a pair. `_FROM` is a bare sender address, not a display/header string. TLS is `starttls`
(required upgrade, never plaintext fallback) or `implicit`, with normal certificate validation.
`loopback` is a development-only fixture transport restricted to literal loopback IPs and no
authentication; it is not a remote SMTP option. Absence disables the adapter; partial/invalid
configuration is refused. Constructing it neither connects nor sends a test mail. Secrets,
recipient addresses and bearer links are redacted from adapter debug/errors.

Registration stores only hashes for mailbox proofs and sends issuer setup links without
persisting their raw token/body. It records effect phases before network calls and never
automatically repeats provisioning after a potentially accepted operation. A dropped initial
verification delivery can be requested again after the configured cooldown, rotating the old
proof; a provisioning/setup/delivery uncertainty needs manual reconciliation. There is no
background outbox, blanket reset, auto-cleanup or exactly-once-mail guarantee.

Sources: [pinned onboarding documentation](https://github.com/kanidm/kanidm/blob/v1.10.5/book/src/accounts/authentication_and_credentials.md),
[create and intent HTTP routes](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/v1.rs),
[built-in access profiles](https://github.com/kanidm/kanidm/blob/v1.10.5/server/lib/src/migration_data/dl14/access.rs#L1335-L1482),
[explicit UUID duplicate protection](https://github.com/kanidm/kanidm/blob/v1.10.5/server/lib/src/plugins/base.rs#L37-L133).
Pinned source supports the adapter contract; deployed policy/authority still requires the real
acceptance above. The [current evidence ledger](../../docs/evidence/theatre-web-auth-mac.md) owns
actual local/live results.

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
