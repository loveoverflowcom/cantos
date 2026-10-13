# Studio OIDC credential and session boundary

Implemented as an opt-in **development-host integration**, with signed mocked-provider and
local PostgreSQL evidence recorded [separately](../evidence/studio-oidc.md). Production activation
remains closed: `cantos-server` requires `CANTOS_ENV=development`, a loopback bind and local
PostgreSQL. This slice changes none of those restrictions. No Kanidm instance/client/grant/account
was provisioned. [Surface boundaries](../decisions/0015-api-surface-boundaries.md) and the
[identity registry](identity-registry.md) remain authoritative.

## Architecture and explicit choices (L1)

One modular backend owns this BFF; no Gateway, admin directory or new service. The concrete
consumer is Studio Web through the current Studio router. `platform/oidc/core.rs` now owns
pure signature/claims/expiry decisions and typed errors; `platform/oidc/provider.rs` owns bounded HTTP/cache;
`postgres.rs` owns durable transactions; the public application and thin Studio HTTP handlers
compose them. Pure identity mapping remains in `platform/identity`. `ring` 0.17.14 supplies actual
P-256 verification; base64/reqwest use already locked versions. No external identity headers,
email matching, client-declared actor ID or deserialized claims can mint authority.

The private `VerifiedIdentity` is neither deserializable, serializable nor cloneable; only the
crypto verifier constructs it. It contains the exact issuer/subject, expiry and token digest.
The database issuance adapter consumes that witness. A database login that can execute the narrow
issuance function is a trusted backend capability; SQL is not a second credential verifier.
A compromised backend/database capability is outside the untrusted-browser boundary.

The approved fixed confidential client is `cantos-studio-web-bff`; request only `openid`.
The issuer must be the exact `/oauth2/openid/cantos-studio-web-bff` URL. No trailing-slash alias,
userinfo, query, fragment or silently normalized URL is accepted. Web origin and exact
`/api/v1/studio/auth/callback` are operator pinned; Host/Forwarded headers choose nothing.
HTTPS uses normal certificate validation; HTTP is permitted only on literal loopback for local
tests/development. Provider metadata endpoints must retain that exact provider origin.

Reference audit: fresh VOT develop `f3fea641a5fd226b883c4e32224ad8c97b8323f0`, particularly Console
proof and typed Kanidm reconciliation. Cantos retains private verified evidence and bounded
configuration, without copying VOT's Gateway headers, admin grants, microservices, credential
recovery or automatic bootstrap. Primary protocol sources:
[Kanidm OAuth2](https://kanidm.github.io/kanidm/stable/integrations/oauth2.html),
[Kanidm 1.10.5 protocol DTOs](https://github.com/kanidm/kanidm/blob/v1.10.5/proto/src/oauth2.rs),
[OIDC validation](https://openid.net/specs/openid-connect-core-1_0.html#IDTokenValidation),
[PKCE](https://www.rfc-editor.org/rfc/rfc7636). This source audit is not provider-live evidence.

## Invariants / threat and evidence ledger

| Invariant | Failure class | Owner / cheapest oracle |
| --- | --- | --- |
| Exact client/redirect and S256 | Intercepted code, redirect substitution or client mix-up | Pinned config + real HTTP mock validating code verifier and redirect; RFC7636 vector |
| Browser-bound state and nonce | Login CSRF or attacker callback | Random 256-bit state/binder/nonce; hashed durable flow and signed nonce; concurrent SQL consume |
| Private verified witness | Raw DTO/header/registry lookup becomes authentication | Constructor/privacy review + actual P-256 signed rejection partitions |
| ES256/kid/key/signature/claims | Alg confusion, forged identity or stale proof | Bounded pure verifier + injected clock and exact error examples |
| One code exchange attempt | Lost response or concurrent callbacks redispatch code | Commit consumed before HTTP; discard-ack/failing-exchange/race tests |
| Issuance and exact identity | Orphan actor, automatic linking or account isolation failure | Same PR1 length-framed lock/PK + narrow function atomic actor/binding/session/receipt |
| Current active/revoked facts | Disable/logout/rotation races mint stale authority | Session and actor locks + final time checks; explicit PostgreSQL lock-wait test |
| Cookie source/host scope | Session fixation, insecure namespace fallback, CSRF | Exact configured cookie name; duplicates/mixed names rejected; Origin guard and HTTPS policy tests |
| Remote provider active | Disabled/revoked provider credential continues indefinitely | Authenticated introspection per request; outage/expiry/disabled refusal |
| Surface separation | Studio grants Listener/Admin or producer authority | Current Studio OIDC routes only; reserved-route HTTP tests; producer grants unchanged |
| Production stays closed | Mock evidence enables deployment | Development/loopback/local DB host checks remain; live gates NOT_RUN |

## Callback and recovery

`GET /api/v1/studio/auth/login` redirects to the pinned provider. Cross-site Fetch Metadata or a
mismatched Origin is refused. A valid previous Studio session is remembered for rotation.
An expired/revoked cookie, or a browser carrying an old development cookie when OIDC is enabled,
must call same-origin `DELETE /api/v1/studio/session` first, then begin login again.

State/binder hashes, nonce, PKCE verifier, issuer, client, redirect and previous session hash are
stored with a 300-second expiry. The binder cookie is HttpOnly/SameSite=Lax because a top-level
provider callback must send it. The callback consumes the matching flow **before** exchanging the
code. It never automatically retries the token endpoint, including after timeout, cancellation,
worker/process failure or a lost response. A failed or lost callback requires a fresh login.
A lost successful acknowledgement may leave an inaccessible bounded session; replay reveals no
cookie and creates no second session. Completed receipts are retained; no history is deleted.
Maintenance/rate limits/retention budgets must be reviewed before deployment.

Discovery/JWKS HTTP has 2-second connect/5-second total deadlines, no redirects, 64 KiB bodies and
at most 16 keys. Cache lifetime is 300 seconds. Unknown kid or signature failure can refresh once
per 10 seconds, serialized, including failed refreshes; key refresh also handles rotation with a
reused kid. Expired metadata is invalidated before I/O, with bounded failed-load backoff. No outage
extends expired metadata. Crypto accepts only ES256/P-256, bounded canonical base64url, one matching
kid, signature key usage, exact issuer/single audience/optional azp, exp/iat/nbf and nonce. There is
no leeway; operator clocks must be synchronized. The shell reads fresh clock facts after token,
cache/JWKS waits and introspection; unit and HTTP tests inject deterministic clocks.

## Studio sessions, disable and logout

Additive [0020](../../services/cantos-api/migrations/0020_studio_oidc.sql) follows inherited 0018;
0019 belongs to Library/Progress. Original SQL checksums are unchanged. Existing sessions gain
`auth_kind` defaulting to `development`. New OIDC sessions reuse 32-byte SHA-256 hashes and actor
FKs; expiry is the minimum of ID-token expiry, introspected access expiry and 15 minutes, with no
refresh token. SQL checks expiry/flow binding again under the final locks. The receipt's unique
proof hash blocks a duplicate ID-token admission. New identities create an actor/binding
atomically; existing identities reuse the active actor. This function cannot link to an arbitrary
legacy actor, grant resource permission or reactivate an account. Explicit linking remains an
operator action. Runtime has no direct registry/actor/session INSERT privilege.

Provider bearer credentials stay in the backend's private OIDC session table. They are retained
for introspection/revocation, never returned to browsers or logged. The test/local slice stores
these private values as text; production must review encryption/secret delivery, backup access,
retention and limited runtime-role grants before activation. The only browser credential is a
random 256-bit opaque cookie. HTTPS uses `__Secure-cantos_studio` with Secure/HttpOnly/SameSite=Strict,
no Domain and Path `/api/v1/studio`. The flow cookie similarly uses `__Secure-cantos_oidc`, Lax.
Loopback HTTP uses unprefixed names. `__Host-` cannot be used with the required Studio Path.
Authentication rejects alternate names, duplicates and mixed cookies. OIDC-enabled token-login
POSTs (including the old alias) are forbidden; OIDC-disabled hosts reject stored OIDC sessions.

Every authorized HTTP request introspects the stored provider bearer through the pinned,
authenticated client endpoint and checks subject/audience/optional issuer+client/expiry. Current
local session and active actor are rechecked; OIDC transactions lock both session and actor.
Local disable/revocation serializes with issuance and transaction authorization. A provider event
cannot be atomic with PostgreSQL: a request may linearize on active introspection just before
remote disable; already accepted work is not retrospectively cancelled. Provider outage refuses
new work. Existing resource ownership/reader/editor rules and scoped producer Bearer grants stay.

`DELETE /api/v1/studio/session` requires exact Origin. It cancels unfinished flows for the binder,
commits local revocation first, then attempts provider access revocation and clears session/flow
and both known legacy Studio cookie paths in the returned response, reporting `Cantos-Provider-Revocation: completed|pending`. An outage preserves the
pending row; repeating logout with the same bearer from the controlled operator context can retry
it. There is no background recovery daemon or implicit federated/global logout. The provider's
SSO session may remain; signing in again can establish a fresh Cantos session. Rotation also marks
the old provider credential pending, while rejecting the old Cantos token immediately. Operator
recovery for abandoned pending rows is a remaining live/operational gate.

## Affected read inventory

| Read | Frequency / semantics | PostgreSQL path / assessment |
| --- | --- | --- |
| Consume/finalize flow | One matching callback; strong single-use/current expiry | state_hash PK; browser index for explicit logout cancellation |
| Authenticate OIDC session | Per request; current active/revoked/expiry | sessions and OIDC-session hash PKs, actor PK; no cached authorization |
| Register issuer/subject | Per successful callback; exact identity/active state | PR1 C-collated composite PK and identical lock digest |
| Pending logout fact | Per explicit logout/retry; retained observation | token_hash PK; no background aggregate consumer |

No repeated join/aggregate consumer earns DBSP; authentication must retain current transactional
facts. No extra read cache/index or speedup is claimed. The real PostgreSQL gate exercises these
point reads; PR1's retained indexed lookup is the registration baseline. Per-request provider
introspection latency and production cardinality/load remain unmeasured; provider availability
is an intentional authorization dependency.

## Successor boundary

Theatre Web/CMP must not consume `StudioOidc`, its cookies, handlers, development token or private
session table as Listener authority. Reuse only ActorId/exact keys and audited verification ideas.
A trusted Listener adapter may map verified issuer/sub through PR92, then construct its own sealed
witness. Library/Progress's proposed seam is recorded in the
[handoff](../work-plan/identity-authentication-handoff.md); inspect its published code before use.
Native system-browser PKCE and OS-secure credentials remain PR3 work. Admin stays reserved.
