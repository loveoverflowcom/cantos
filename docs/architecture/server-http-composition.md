# Server HTTP composition (333.1)

The executable uses the canonical inbound `assembly::server_router`: one `Store::new`, one
PostgreSQL pool, one `axum::serve` listener. Route-local `AppState` values clone the same pool
and configured services. They carry distinct trusted browser origins; a request cannot select
authority using Host, Forwarded or X-Forwarded headers. PostgreSQL remains the only persistence
authority. This source change does not migrate or activate the deployed VPS composition.

## Route policy inventory

Each configured browser allowlist is a singleton exact serialized origin. No global union or
wildcard is constructed. Requests carrying Origin must match their route's origin, including
reads and deprecated aliases. Native requests omit Origin and retain existing admission.

| Route group | Surface / browser origin | Credential purpose and owner | CSRF / admission | Error / cache |
| --- | --- | --- | --- | --- |
| `/api/v1/studio/*` | Studio / `CANTOS_STUDIO_WEB_ORIGIN` | `studio::admission`, Studio OIDC cookie or checked `ca1_s_` Bearer | Exact Studio Origin for cookie mutations; existing cookie-free Bearer exceptions, fences, recent-auth and transaction rechecks | API `code/current_revision/issues`; no-store except transport-owned private no-store/no-transform bytes |
| `/api/v1/{scripts,validation,imports,adaptations,production,session}` and their existing subpaths | Deprecated Studio aliases / same Studio origin | Same supported Studio handlers and admission; no aliases added for newer Catalog/intake routes | Same Studio boundary; existing deprecation/cookie-path handling | Existing API errors, deprecation headers; no-store |
| `/api/v1/theatre/{works,search,publications}/*`, `/works`, `/search` | Theatre / `CANTOS_THEATRE_WEB_ORIGIN` | `theatre`: anonymous discovery and public-release lookup; playback POST remains narrowly anonymous | Exact Origin when present; only existing playback-authorization POST exemption | Theatre public `code/retryable`; no-store |
| `/api/v1/theatre/media/{grant}` | Theatre / same Theatre origin | `theatre` + audio transport: scoped, expiring grant and current-release/retraction checks | Grant never grants account/private Studio authority | Existing Theatre/audio errors; transport-owned private no-store/no-transform policy |
| `/api/v1/theatre/{library,progress,operations}/*` | Theatre / same Theatre origin | `theatre_auth::bff_identity`, listener authenticator and Library: distinct Web cookie, `ca1_t_` or native `ca1_n_` admission | Existing bearer/native exceptions, session fence, account/resource and transaction rechecks; never public | Library `code/retryable`, existing auth errors; no-store |
| Studio/Theatre `/auth/*`, `/session`, `/auth/bearer/*`; Theatre `/native/auth/*` | Matched surface / its own configured origin | Existing flow, callback, cookie/Bearer, enrollment and native-purpose handlers | Exact Origin if present; cookie CSRF, duplicate/mixed credential refusal, existing callback/state/PKCE and no-Origin native rules | Existing auth DTO/status, redirects and cookies; no-store |
| `/api/v1/admin/*` | Admin / optional `CANTOS_ADMIN_WEB_ORIGIN` | Dedicated `AdminService` verified resource JWT, role/revocation check and immutable audit; never Studio/Theatre session | Browser disabled without its own origin. Existing mutation Origin requirement retained; duplicate Authorization and any Cookie refused | API `code/current_revision/issues`; no-store; absent verifier stays unavailable |

The shared middleware implementation is applied separately to each route group, before groups
merge. CORS preflight runs before credential-requiring admission. Studio/Theatre retain their
existing GET/HEAD/POST/PUT/DELETE CORS method set and only their own session/stale headers,
plus accept/content-type/authorization/range. Admin allows GET/HEAD/POST and
accept/content-type/authorization; it gains no session header or Range permission. PATCH
preflight remains refused under the existing browser transport contract.
The shared 15-second deadline and cache/security response policy remain at each boundary.
No token format, TTL, SessionPolicy, revocation or final admission lease is changed.

## Configuration and compatibility

Production requires explicit `CANTOS_BIND_ADDRESS`, `CANTOS_STUDIO_WEB_ORIGIN` and
`CANTOS_THEATRE_WEB_ORIGIN`. Optional `CANTOS_ADMIN_WEB_ORIGIN` admits that browser origin only;
it neither configures a verifier nor grants Admin authority. All production origins must be
exact HTTPS domain origins, without paths, credentials, trailing slashes or noncanonical ports.
Studio OIDC uses the Studio origin; Theatre OIDC/registration uses the Theatre origin.
`CANTOS_API_ORIGIN` retains the explicit shared callback destination contract.

`CANTOS_BROWSER_API_SURFACE` is rejected at startup. Production also rejects the old
`CANTOS_WEB_ORIGIN`: replace it with the explicit per-surface inputs rather than silently
expanding a legacy host's authority. Development retains `CANTOS_WEB_ORIGIN` only as a
same-origin shorthand; explicit surface inputs override it independently and are validated.
Production continues to refuse static files, development sessions/audio roots and private
issuer CA configuration. Missing provider configuration remains disabled/fail-closed.

The deployed origin-pinned Quadlet/systemd assets and current runtime are unchanged. This
candidate requires an explicit future configuration migration and #336 activation/recovery
acceptance; building or merging it is not live cutover. It does not remove PostgreSQL/Kanidm,
publish OCI images, or change database/media/backups.

## Local lifecycle and verification boundary

`cargo xtask run cantos-server` configures Studio on `http://127.0.0.1:8081` and Theatre on
`http://127.0.0.1:8082` in one owned Server. `run <app> --web` starts that same composition plus
one frontend; use `app <other-app> --web` to attach another frontend to the existing Server.
`--web-origin` on the standalone Server overrides Studio's development origin. Per-app use
still validates its actual Trunk origin. The retained-test-cluster primitive execs this same
binary; it does not add a backend. Runner port ownership, readiness timeouts and bounded
process-group drain/cleanup remain with the existing supervisor.

`surface_policy_baseline` retains the old pinned-host controls and adds the executable's
combined-router matrix. Unissued sentinels prove parsing/refusal only. `bearer_postgres` uses
the existing signed loopback issuer and real disposable PostgreSQL through the combined router,
including two issued Web purposes on one shared pool. `admin_postgres` runs its dedicated
verified/audited admission through the combined assembly. These are fixture evidence, not
live Kanidm acceptance. The process regression in `bearer_postgres` starts the actual production
executable in production mode, checks exactly one owned Server child, uses one listener for
all surfaces, writes through an issued fixture credential, gracefully stops/restarts and checks
committed data and exact operation replay. It belongs to the existing PostgreSQL gate.
The existing Theatre media seed now uses the same canonical combined assembly with distinct
browser origins and checks actual promoted public bytes, private owner Range reads, their exact
cache/CORS headers and cross-origin/anonymous refusals. The combined-router slow-body regression
reaches the production JSON extractor and its unmodified 15-second surface deadline.

Browser/device journeys, real provider acceptance, Kani, Turso serving, native no-infrastructure
acceptance, footprint and VPS cutover are separate gates. Test results and exact source SHAs
belong in the PR evidence rather than becoming a second execution-status board here.

For 333.2–333.8, reuse this composition and one application owner while building the complete
Server Turso schema/adapters; 333.9 owns the atomic persistence flip. Preserve the checked
SessionPolicy/credential owners and the [persistence prerequisite map](native-persistence-readiness.md).
