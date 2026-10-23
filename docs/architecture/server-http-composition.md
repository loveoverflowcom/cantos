# Server HTTP composition (333.1)

The executable uses canonical inbound `assembly::server_router`: one Server Store backed by
embedded Turso and one `axum::serve` listener. Route-local state shares the owner and configured
services, with distinct trusted browser origins. Host/Forwarded/X-Forwarded never selects authority.
Business schema/SQL remains incomplete; [the checkpoint](server-turso-porting.md#checkpoint-boundary)
is separate from full HTTP and deployed acceptance.

## Route policy inventory

| Route group | Credential / authority | Origin, CSRF and admission |
| --- | --- | --- |
| `/api/v1/studio/*` and deprecated Studio aliases | Identity v1 Studio cookie and scoped native admission where the contract permits; Studio roster separately checked | Exact Studio origin; session fence, recent-auth and late transaction rechecks; no Theatre/Admin/producer authority |
| `/api/v1/theatre/{works,search,publications}` | Anonymous eligible public release discovery | Exact Theatre origin when present; no private metadata or account authority |
| `/api/v1/theatre/media/{grant}` | Scoped expiring grant with current release/withdrawal checks | Never grants private Studio/account access; transport owns byte/cache/Range policy |
| Theatre Library/progress/operations | Theatre Web cookie or native Bearer from Identity v1 | Exact browser origin/CSRF and cookie-free native rules, session fence and current actor/resource admission |
| Studio/Theatre `/identity/*` | First-party mailbox/password/fresh-fact and session lifecycle | Purpose-specific routes/cookie names, mixed/duplicate credential rejection, origin and unknown-outcome fences |
| `/api/v1/admin/*` | Dedicated capability/audit boundary; current verifier absent, fail-closed | Optional own origin never grants authority; no Studio/Theatre credential reuse |

Use [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) for exact endpoints,
wire errors, cookie/Bearer syntax and freshness. Removed `/auth/*` provider/OIDC/Firebase setup
and callback routes are not another supported sign-in path. Keep receipt/replay/revoke rules.

Middleware is applied separately before route groups merge. Each configured browser allowlist
is one exact origin, without union/wildcard. CORS preflight precedes authentication and exposes
only surface-specific methods/headers. Keep session/stale headers and native cookie-free rules
scoped to their owner. The 15-second deadline and no-store/security policy remain at boundaries;
transport-specific private no-store/no-transform bytes keep their existing policy.

## Configuration and compatibility

Production requires explicit bind and Studio/Theatre origins, exact HTTPS domain origins without
path/credentials/noncanonical port. Optional Admin origin does not configure a verifier.
`CANTOS_BROWSER_API_SURFACE` and production `CANTOS_WEB_ORIGIN` are refused; do not silently
widen a legacy host. Development keeps the validated same-origin shorthand and separate defaults.
No origin setting creates an account, roster, role, session or mailbox proof.

Legacy deployed topology is a historical observation, never a fresh host fact. Native OCI/host
activation follows [#348](../deployment/native-oci-release.md); no source cleanup changes a host,
DB/WAL, media or recovery resource.

## Local lifecycle and verification boundary

`run cantos-server` starts Server + Identity, explicitly migrates private development files and
supervises only its foreground groups. Web `run` adds Trunk on Studio 8081 or Theatre 8082;
`app` attaches a frontend to an existing API. Occupied ports refuse. `--test-cluster` is retired.
Native readiness is not a working business catalog, login, publication or browser/device gate.

`surface_policy_baseline` retains route/origin/purpose controls, with provider-era expectations
still requiring native rebinding under #333/#335. The archived PG suites are historical issued-
credential/transaction oracles, not executable native HTTP acceptance. Preserve slow-body,
duplicate/mixed credentials, current admission, cross-origin and exact cache/CORS assertions
when porting. File-backed session/RPC and model evidence never replace those HTTP assertions.
Browser/device, restore, Linux isolation, D1 Kani, footprint and deployed state remain independent.
