# ADR 0041: Direct Firebase Hosting and the Cantos API browser boundary

- Status: accepted owner direction; API addressing/CORS/callback support implemented in this slice;
  domain selection, minimal identity gateway and production acceptance remain pending
- Date: 2026-10-17
- Decision owner: Cantos product owner (production-audit request and API-mediated Web clarification)
- Related: [0034](0034-theatre-signup-and-provisioned-studio.md),
  [0036](0036-firebase-enrollment-proof.md), [0040](0040-firebase-email-proof-alternatives.md)
- Supersedes: [0039](0039-vps-production-runtime.md), only its Web proxy/public identity topology;
  its Podman, Unix-socket PostgreSQL, backup and production-host rules remain applicable
- Operations: [current production topology and runbook](../deployment/vps-production.md)

## Context

The owner selected direct Firebase Hosting for both Web apps and asked that their clients talk to
Cantos API. The existing deployment is transitional: static Hosting works, but relative `/api`
requests there return Firebase 404. The existing API processes pin different legacy Web origins;
merge state does not identify their deployed image. Neither static delivery nor a reachable OIDC
login page establishes a completed user journey.

## Decision

Theatre Web and Studio Web load directly from separate Firebase Hosting sites. Cantos API remains
on the VPS behind its API ingress (`https://api.cantoss.duckdns.org`). PostgreSQL stays on a private
Unix socket and Kanidm stays on the private VPS network. Caddy owns only necessary API/TLS and
identity gateway ingress in the target topology. The old Web reverse proxy is transitional source
and installed configuration, not the default architecture to extend.

Both Web clients resolve only their own `/api/v1/theatre/*` or `/api/v1/studio/*` routes against
an explicit, nonsecret build input `CANTOS_API_ORIGIN`; absence preserves the local Trunk proxy.
An invalid origin or cross-surface path fails closed. The implementation reuses locked `url`
2.5.8 to refuse silently normalized origins, userinfo, paths, queries and fragments. Fetch/XHR and
login navigation go to Cantos API, never to a client-configured identity provider. Anonymous and
producer-bearer requests omit cookies; owner/session requests retain their fences and use Fetch
`credentials: include`. This changes no authorization rule or token-storage policy.

Each API process may opt into `CANTOS_BROWSER_API_SURFACE=theatre|studio`, with its one exact
`CANTOS_WEB_ORIGIN` as the CORS allowlist. Preflight accepts only the declared surface, methods and
headers, returns no wildcard, and reaches no authentication or database effect. Actual responses
expose only their surface's session-fence/representation headers and vary on Origin. Preflight
and refusals are no-store; real responses retain their transport-owned private cache policy. The API ingress
routes Studio paths to the Studio process and Theatre paths to the Theatre process.

For split-origin OIDC, the API runtime's explicit `CANTOS_API_ORIGIN` pins the callback on the API
host; completion returns only to the fixed configured Web origin. It never reflects a caller's
return URL. Confidential clients, exact issuer/audience, state, nonce, PKCE S256, consumed-flow
fences, roster checks, introspection and session expiry remain authoritative. The existing
cross-site login refusal and host-only HttpOnly/Secure cookies remain: sessions use SameSite=Strict,
flow cookies Lax; flow lifetime is 300 seconds, session lifetime at most 900 seconds/provider expiry.
There is no Domain cookie, browser-held provider token or localStorage credential.

### Cookie and domain gate

`*.web.app` and the DuckDNS API are cross-site. CORS and `credentials: include` cannot override
SameSite=Strict or browser third-party-cookie policy. This slice therefore does **not** establish
login/session persistence on those hostnames. Do not change cookies to None, remove CSRF checks,
or move credentials into localStorage to make the audit pass.

The proposed stable solution is operator-owned DNS: separate Firebase custom domains for Cantos
and Cantos Studio, and a browser API alias under the **same registrable domain**, with an exact
allowlist per surface. The DuckDNS API may remain for native/producer compatibility. No owned
domain or alias has been selected, created or verified here. Firebase's domain/TLS setup and a
reviewed issuer/callback migration are prerequisites to activation, not implied by this ADR.

### Minimal public identity gateway (design pending; not implemented)

Cantos API orchestrates identity and enforces application authorization; Kanidm owns passwords,
MFA, credential setup and OIDC. A browser must still reach Kanidm's authentication UI through a
restricted public gateway during OIDC. An API call alone cannot complete that browser interaction.
Do not expose `idm.cantoss.duckdns.org` as a general Kanidm service in the target topology.

Before deployment, qualify a default-deny gateway against pinned Kanidm 1.10.5. The route/method
inventory must include only required UI/static assets, browser authentication and credential-setup
operations and the exact OIDC client endpoints. Token, introspection, revocation and onboarding
management operations should use private ingress. Person/group/client/system/schema/raw/recycle
management must never be publicly routed, including new or alternate API paths. The current
management-prefix denylist is an interim source hardening, not this allowlist design.

Choose the gateway authority and confirm Kanidm's issuer, origin, CSP, asset and redirect behavior
before writing routing rules. Do not assume Kanidm works under an arbitrary `/identity` prefix;
renaming its initialized domain also affects credentials/WebAuthn and existing issuer bindings.
The exact gateway/domain migration requires a separate reviewed implementation and explicit
operator deployment authority. No security or identity configuration is deployed by this slice.

## Alternatives

| Option | Assessment |
| --- | --- |
| Continue the Web → VPS → Firebase proxy | Existing compatibility path; superseded as the target by the owner's direct-Hosting direction |
| Direct web.app → DuckDNS with relaxed cookies | Public anonymous reads can use CORS; reliable authenticated sessions are not accepted under third-party-cookie restrictions |
| Same-site custom domains and exact CORS | Recommended next domain decision; requires operator-owned DNS and full browser/issuer qualification |
| Firebase Functions/Cloud Run proxy | Adds another runtime/cost/credential boundary; not selected or provisioned |

## Validation and revisit conditions

Verify URL refusal partitions and CORS allow/deny boundaries locally, then inspect real browser
requests after explicitly authorized deployment. Accept auth only with real callback, reload,
logout/revocation, roster refusal and Theatre-to-Studio denial, including restrictive cookie
settings. Registration stays disabled under its independent ADR 0036/0040 gates. Native production
acceptance remains separate. Revisit this decision if no same-site browser domain can be supplied;
that requires an explicit alternative identity/session design, not a CORS workaround.

Primary references: [Firebase custom domains](https://firebase.google.com/docs/hosting/custom-domain),
[Hosting rewrite targets](https://firebase.google.com/docs/hosting/full-config),
[Fetch credentials and SameSite](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API/Using_Fetch#including_credentials),
[CORS and third-party cookie policy](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS#requests_with_credentials),
[Kanidm origin/domain configuration](https://kanidm.github.io/kanidm/stable/server_configuration.html).
These support the design constraints, not deployed acceptance.
