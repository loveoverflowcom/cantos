# ADR 0041: Direct Firebase Hosting and the Cantos API browser boundary

- Status: accepted owner direction; API addressing/CORS/callback support implemented in this slice;
  Bearer API implemented; #295 Web cookies consumed by both Web apps from source (Identity auth v1,
  browser/E2E acceptance pending); native Bearer retained; minimal identity gateway source
  implemented; production acceptance remains pending
- Date: 2026-10-17; Bearer amendment 2026-10-18 (#253); Web-cookie amendment 2026-10-19 (#295)
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
login navigation go to Cantos API, never to a client-configured identity provider. Current
implemented native consumer uses [Bearer v1](../../services/cantos-server/contracts/auth/bearer/v1/README.md); the Web apps
no longer do. Both Web apps send `credentials: include` with the session fence to the
[Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) cookie endpoints
(#295, [web-auth](../../packages/web-auth/README.md)); browser and real API acceptance are pending.
Provider artifacts stay in the API; mixed credentials remain refused by the server.

Each API process may opt into `CANTOS_BROWSER_API_SURFACE=theatre|studio`, with its one exact
`CANTOS_WEB_ORIGIN` as the CORS allowlist. Preflight accepts only the declared surface, methods and
headers, returns no wildcard, and reaches no authentication or database effect. Actual responses
expose only their surface's session-fence/representation headers plus `Retry-After` and vary on Origin. Preflight
and refusals are no-store; real responses retain their transport-owned private cache policy. The API ingress
routes Studio paths to the Studio process and Theatre paths to the Theatre process.

For split-origin OIDC, the API runtime's explicit `CANTOS_API_ORIGIN` pins the callback on the API
host; completion returns only to the fixed configured Web origin. It never reflects a caller's
return URL. Confidential clients, exact issuer/audience, state, nonce, PKCE S256, consumed-flow fences, roster checks,
introspection and original proof/provider authority remain authoritative. The implemented Bearer Web flow uses
independent client/provider PKCE legs; its API callback returns only a one-use handoff code to the
fixed Web origin. Exchange returns access/refresh via no-store JSON, never URLs. Native retains
its distinct public OIDC client and exact system-browser redirect.

### Owner-selected Web cookies; native Bearer retained (#295)

On 2026-10-19 the [owner selected #295](https://github.com/loveoverflowcom/cantos/issues/295):
Theatre Web at `https://cantos.cc` and Studio Web at `https://studio.cantos.cc` use API-issued
HttpOnly/Secure cookies with `https://api.cantos.cc`. This supersedes only #253's mandatory
RAM-only Web Bearer direction. The API cookie policy is [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md)
and the Web client migration is implemented in source; browser acceptance is pending. This decision
does not report deployed cookies or undo existing session authority, revocation and account
fences. CMP/native retains Bearer and secure storage.

The migration must define narrow cookie scope, SameSite, path and expiry, preferring host-only
API cookies where suitable; it does not authorize `Domain=.cantos.cc`. Include credentials only
for the exact API/surface, including private resource flows. Preserve exact-origin credentialed
CORS and mutation CSRF checks against foreign origins and same-site siblings. Distinct origin
and distinct site are different conditions. Preview `web.app`/`pages.dev` and local topologies
need separate declared qualification; production acceptance uses the custom HTTPS domains.
No live DNS, issuer, credentials or database operation is authorized by this document.

### Implemented Bearer transport and reload policy (#253; Web target superseded)

The following records the **retired** pre-#295 Web behavior (removed from both Web apps) and the
retained native contract. It is not the current Web cookie/reload policy.

The owner selected Cantos-issued opaque Bearer for Theatre Web/CMP and Studio Web on 2026-10-18.
For that Bearer implementation, a same-site custom domain or third-party session cookie is not
a prerequisite. Firebase Hosting and API may be cross-site. Exact per-surface CORS,
restricted interactive identity ingress and browser acceptance remain required; CORS itself
never authenticates. Existing custom domains remain valid optional addressing decisions.

Web access and optional refresh credentials live only in RAM. In-tab refresh single-flights;
F5/new tab loses RAM and requires controlled authorization/SSO or login. The Web implementation uses a user-reserved popup with top-level issuer navigation so the opener retains drafts; blocked windows fail with explicit retry. Temporary
PKCE/state with TTL/consume-once may survive navigation, never access/refresh credentials.
Native uses its secure storage/cleanup barriers. Refresh rotation, reuse-family revocation,
unknown-response reauthorization and actual signed recent-authentication proof are specified
in Bearer v1. This initial implementation never extends the original provider/proof lifetime.

The API foundation, Web RAM/transport migration and native secure-journal/refresh consumers
are implemented. Exact dual legacy/Bearer callback reconciliation source is implemented;
operator rollout, removal of legacy server issuance and real restrictive-browser/issuer/device
acceptance remain unfinished. Legacy cookies retain
Strict/Lax, Secure/HttpOnly and their CSRF rules during that explicit staging window. Relaxing
cookies or persisting Web tokens in localStorage is not part of this decision.

### Minimal public identity gateway (source implemented; live qualification pending)

Cantos API orchestrates identity and enforces application authorization; Kanidm owns passwords,
MFA, credential setup and OIDC. A browser must still reach Kanidm's authentication UI through a
restricted public gateway during OIDC. An API call alone cannot complete that browser interaction.
Do not expose `idm.cantoss.duckdns.org` as a general Kanidm service in the target topology.

Before deployment, qualify a default-deny gateway against pinned Kanidm 1.10.5. The route/method
inventory must include only required UI/static assets, browser authentication and credential-setup
operations and the exact OIDC client endpoints. Token, introspection, revocation and onboarding
management operations should use private ingress. Person/group/client/system/schema/raw/recycle
management must never be publicly routed, including new or alternate API paths. The [gateway source](../../infra/edge/identity-gateway.md) implements explicit method/path
admission and private TCP-peer back-channel access, with pinned-Caddy local routing tests.
Those tests use a recording upstream and do not establish real Kanidm/browser acceptance.

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
| Cantos opaque Bearer and exact CORS | Implemented Web transport and retained native target; #295 supersedes the Web target; issuer/device acceptance remains separate |
| Custom-domain Web cookies | Owner-selected Web target under #295; API/client migration, narrow cookie policy, exact-origin CORS/CSRF and real browser acceptance pending |
| Firebase Functions/Cloud Run proxy | Adds another runtime/cost/credential boundary; not selected or provisioned |

## Validation and revisit conditions

Verify URL refusal partitions and CORS allow/deny boundaries locally, then inspect real browser
requests after explicitly authorized deployment. Accept auth only with real callback, reload,
logout/revocation, roster refusal and Theatre-to-Studio denial, including restrictive cookie
settings. Registration stays disabled under its independent ADR 0036/0040 gates. Native production
acceptance remains separate. #295 Web acceptance additionally inspects Set-Cookie, credentialed
requests, scope and CSRF on the custom domains; historical Bearer acceptance cannot substitute
for that migration or establish native regression coverage.

Primary references: [Firebase custom domains](https://firebase.google.com/docs/hosting/custom-domain),
[Hosting rewrite targets](https://firebase.google.com/docs/hosting/full-config),
[Fetch credentials and SameSite](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API/Using_Fetch#including_credentials),
[CORS and third-party cookie policy](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS#requests_with_credentials),
[Kanidm origin/domain configuration](https://kanidm.github.io/kanidm/stable/server_configuration.html).
These support the design constraints, not deployed acceptance.
