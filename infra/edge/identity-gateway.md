# Kanidm browser gateway qualification — historical

> **Retained legacy ingress/recovery record.** Native authentication uses
> [Identity v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md);
> the Server's Kanidm/OIDC adapters are retired. The policy/evidence below does not configure
> native signup/login or authorize a live ingress change. Preserve installed legacy resources
> until separately accepted cutover/recovery.

Historical status: default-deny method/path gateway source implemented for pinned Kanidm 1.10.5,
with local Caddy 2.11.7 routing evidence. Live deployment, sanitized real browser traces,
issuer/callback and credential-mechanism acceptance remain separate gates. No running
configuration changes when this file or the Caddyfile changes. The
[production runbook](../../docs/deployment/vps-production.md) owns rollout authority.
Registration remains disabled under its independent gates.

## Authority and upstream boundary

Preserve the existing `CANTOS_IDM_HOST`, initialized Kanidm origin/domain, issuer and
WebAuthn relying party. No new hostname, path prefix, provider client, account, credential,
role or grant is selected by this gateway. Do not rename the initialized identity domain.
The source policy narrows the existing general identity proxy; it does not provision identity.

Only `(cantos_kanidm_upstream)` in the [Caddyfile](Caddyfile) proxies to Kanidm. It retains
private-CA validation and `tls_server_name cantos-kanidm`; there is no TLS bypass.
An explicitly configured `CANTOS_INTERNAL_CIDRS` identifies the inventoried Cantos TCP peers,
not forwarded headers. No RFC1918-wide default is supplied. Missing ranges admit no private
management/back-channel requests, so inventory the actual network/peer scope before activation.
Never set this variable to a public/default route or broaden it merely to make login pass.

## Public admission

The exact paths and methods live once in the Caddyfile's named matchers:

| Matcher | Admitted scope |
| --- | --- |
| `kanidm_browser_get` | GET/HEAD of named login/landing, consent/resume, existing setup/recovery/logout UI, domain/two existing Web-client images, manifest/robots and individually named bundled static assets |
| `kanidm_browser_post` | POST of named password/MFA/passkey login, consent, existing recovery/revoke and credential-update form/commit/cancel handlers |
| Fallback | Every other path or method receives edge 404/no-store before upstream routing |

There is no public `/ui/*`, `/ui/api/*`, `/pkg/*`, `/oauth2/*`, REST or SCIM wildcard.
The shared OAuth login/consent UI still delegates client/state/nonce/PKCE, CSRF and user
credential checks to Kanidm; path admission itself authenticates nobody. Discovery/keys,
exchange/userinfo/introspection/revocation and all management stay on private ingress.
Unix/SSH credential management, RADIUS, profile editing and provider administration are
not public Cantos browser requirements and remain refused.
No native direct-to-provider back-channel is introduced; Cantos API retains those effects.
The existing two Web image names do not create a native client or invent its live issuer.

Public percent-encoded path spellings, repeated separators, dot segments and backslashes
are refused before upstream routing. Query encoding required for OIDC state/PKCE remains
available. Refused requests have no-store; the identity authority uses no-referrer. No
access logging of authorization URLs, reset codes or credentials is enabled by this policy.

## Pinned source inventory

These are primary source owners, not evidence about the installed server:

- [HTTPS composition 1.10.5](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/mod.rs):
  HTML at `/ui`, independently mounted static directory, domain/client images and manifest.
- [HTML/HTMX router 1.10.5](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/views/mod.rs):
  named login/setup routes coexist with `/ui/admin` and `/ui/api/admin`; wildcard UI admission
  would expose management routing.
- [OIDC router 1.10.5](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/oauth2.rs):
  browser and back-channel operations are separate; a public OAuth wildcard is unnecessary.
- [Base template](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/templates/base.html)
  and [icons](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/templates/base_icons.html):
  static asset names; query cache-busters do not require wildcard paths.

## Verification and remaining rollout gates

The repository gate discovers `scripts/test_edge_gateway.py` and `test_edge_cache.py`.
With an already installed binary supplied as `CANTOS_CADDY_BIN`, the optional tests require
Caddy 2.11.7 (matching the Quadlet pin) and execute actual Caddy request matching against a
recording loopback HTTP double. They cover public unknown/management/back-channel paths
across eight methods, spoofed forwarding headers, encoded/dot/separator paths, named allowed
methods, empty private ranges and private-peer access. No test downloads tools or starts
Podman/Kanidm. Without the binary, the optional runtime tests skip explicitly.

That is real edge routing with a mocked upstream, not Kanidm authentication, TLS, browser
or device acceptance. Before live activation:

1. Verify SSH host identity independently and inspect current edge/API/source/schema/config
   and operation ownership. Preserve existing configuration and recovery artifacts.
2. Inventory the actual Cantos TCP peers and existing initialized authority. Validate/adapt
   the candidate with the pinned deployed Caddy and private CA; an empty peer list breaks
   API back-channel access. Do not use broad ranges or spoofed headers as a workaround.
3. Reconcile only the existing clients' exact legacy/Bearer callbacks through the guarded
   owner, preserving roster, grants and secret digests. Native issuer/client registration
   must be verified separately; source names are not live registration evidence.
4. Capture sanitized actual login, consent/refusal, enabled credential setup/MFA, redirect,
   logout and popup/opener traffic. Reconcile every required asset and method with the
   pinned inventory. Never retain passwords, provider codes or reset tokens in evidence.
5. Confirm edge refusal of management/unknown/alternate paths and private back-channel
   access, then actual callback/reload/logout, signed recent-auth proof, surface isolation
   and restrictive-browser behavior. Kanidm 401 or reachable HTML is not edge refusal or
   authenticated acceptance. Missing routes must be reviewed individually, never replaced
   by a permissive proxy fallback.
