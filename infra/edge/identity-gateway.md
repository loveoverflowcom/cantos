# Kanidm browser gateway qualification

Status: source inventory and interim management-path hardening only. The reviewed default-deny
gateway, authority/domain migration and live browser acceptance required by
[ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md) remain pending. No replacement
gateway is enabled by this document; the [production runbook](../../docs/deployment/vps-production.md)
owns deployment and operator authorization. Keep registration disabled under its independent gates.

## Pinned source boundaries

Inspect the official `v1.10.5` source matching the image pin in
[`services/manifest.toml`](../../services/manifest.toml). A path existing upstream does not make it
necessary for Cantos or sufficient for an allowlist.

| Owner | Observed routes and review implication |
| --- | --- |
| [HTTPS composition](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/mod.rs) | The HTML router is nested at `/ui`; assets at `/pkg`; images and manifest are separate routes. `/ui/*` therefore cannot be admitted as one harmless static tree. |
| [HTML/HTMX router](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/views/mod.rs) | Authentication, OAuth consent and credential updates share this router with management. It nests independent management routers at `/ui/admin` and `/ui/api/admin`. The interim edge filter covers each root and subtree for every method, before its reverse proxy, for nonprivate TCP peers. |
| [OIDC router](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/oauth2.rs) | Discovery, keys, authorization, userinfo, token, introspection and revocation have distinct routes. Enumerate actual client IDs and methods; a public `/oauth2/*` wildcard also admits back-channel operations. |

The management filter deliberately retains browser `/ui/login`, `/ui/oauth2`, `/ui/reset` and
credential-update `/ui/api` routes. It is still a denylist: unlisted or future management paths
remain a risk. Static regression checks prove the checked-in entries and ordering, not Caddy
matching/normalization, upstream authorization or the installed edge policy. Narrow the current
`CANTOS_INTERNAL_CIDRS` to reviewed Cantos peers; forwarded headers never grant private ingress.

## Required evidence before an allowlist replaces the transitional proxy

1. Select the gateway authority with the same-site Web/API domain plan. Preserve or explicitly
   migrate the initialized Kanidm origin, issuer, exact callbacks and WebAuthn relying-party
   domain. Do not place Kanidm beneath a guessed path prefix.
2. Capture sanitized requests from pinned Kanidm 1.10.5 for actual Cantos OIDC login, every
   enabled credential mechanism, consent/refusal, logout and operator-issued credential setup.
   Start with the exact source routes below, then reconcile templates, JavaScript, images,
   manifest and redirects against the browser trace. Do not save credential/reset tokens.

   | Candidate route family | Methods to qualify from the pinned router |
   | --- | --- |
   | `/ui/login`, `/ui/login/oauth2_landing` | GET |
   | `/ui/login/{begin,mech_choose,pw,totp,backup_code,seckey,passkey}` | Explicit GET redirects and POST handlers; select only enabled mechanisms |
   | `/ui/oauth2`, `/ui/oauth2/resume`, `/ui/oauth2/consent` | GET, GET, POST respectively |
   | `/ui/enrol`, `/ui/reset`, `/ui/recover`, `/ui/revoke`, `/ui/update_credentials` | Distinct entry/recovery flows; review each method and capability rather than admit `/ui/*` |
   | `/ui/reset/*`, `/ui/api/*` | Credential forms and commit/cancel endpoints require individual names; never wildcard because management shares `/ui/api` |

3. Pin exact OIDC clients for Web, native and any approved operator consumer. Decide which
   discovery/key routes need public access; route API token exchange, introspection, revocation
   and onboarding management through private ingress while preserving issuer/TLS validation.
4. Build a method/path allowlist ending in an unconditional edge refusal. Test management and
   unknown routes across all methods, including HTML/HTMX admin, REST, SCIM, alternate discovery
   paths, encoded separators and normalized dot segments. A Kanidm 401 is upstream routing,
   not edge refusal. Verify private API/CLI access separately, with spoofed forwarding headers
   unable to become a private TCP peer.
5. Adapt/validate with the deployed pinned Caddy, then run real browser acceptance on the
   selected authorities: callback, reload/session, logout/revocation, roster refusal and
   Theatre-to-Studio denial with restrictive browser cookie settings. Inspect CSP, asset MIME
   types, redirects and WebAuthn. Source tests and reachable login HTML do not satisfy this gate.

No production domain is selected here. Without that decision and the sanitized pinned browser
trace, completing or deploying an allowlist would invent missing acceptance; the bounded source
hardening must not be reported as a completed gateway or authentication rollout.
