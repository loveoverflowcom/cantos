# Studio OIDC credential and session boundary

Implemented as an opt-in **development-host integration**, with signed mocked-provider and
local PostgreSQL evidence recorded [separately](../evidence/studio-oidc.md). Production activation
remains closed: `cantos-server` requires `CANTOS_ENV=development`, a loopback bind and local
PostgreSQL. This slice changes none of those restrictions. No Kanidm instance/client/grant/account
was provisioned. [Surface boundaries](../decisions/0015-api-surface-boundaries.md) and the
[identity registry](identity-registry.md) remain authoritative.

## Closed Studio enrollment

The [accepted account-enrollment decision](../decisions/0034-theatre-signup-and-provisioned-studio.md)
allows public self-registration for Theatre and limits Studio to one or two securely
preprovisioned administrator accounts. Studio has no public signup route or enrollment form.
These are Studio operator accounts; admission does not grant the separate `/api/v1/admin`
capabilities or override existing Work ownership, editor, rights or publication checks.

The development host requires `CANTOS_STUDIO_ADMIN_SUBJECTS` whenever Studio OIDC is enabled.
Its value is a JSON array of exactly one or two distinct, valid, **exact OIDC subjects**, scoped
to `CANTOS_STUDIO_OIDC_ISSUER`. They are not email addresses, display names, passwords, access
tokens, Theatre actor IDs or group claims. No trimming, case folding or identity linking occurs.
An absent, malformed, empty, duplicate or oversized roster prevents that host from starting;
supplying a roster without the explicit Studio issuer also fails. Library construction without
the roster remains suitable for read-only metadata checks but cannot start or complete login.

After normal signature, exact issuer/audience, nonce, expiry and access-token checks, callback
requires membership in this server-owned roster **before** actor/binding/session issuance.
Unlisted identities receive the existing unauthenticated refusal and no Studio cookie. Their
flow remains consumed, so a rejected callback never retries the provider exchange. Each private
request also checks the current process's roster against the stored exact subject before
provider introspection. Applying a changed operator roster requires restarting the host; on the
reconfigured host a removed subject's existing sessions are refused. No identity/history is
deleted, and adding a subject back does not revoke its previous unexpired sessions. For durable
revocation use the existing explicit session/actor/provider revocation controls as appropriate.

Roster entries authorize admission only after credential verification. The existing internal
actor binding may be created atomically on the first authorized login; this creates no provider
account, credentials, provider grant or application role. Theatre-issued credentials/sessions,
Theatre signup and arbitrary provider groups cannot add roster entries. The isolated development
token mode remains a local fixture/operator tool under the existing development-only host gate;
it is neither public registration nor a production account-bootstrap mechanism.

Real provisioning requires a separate approval naming the environment, account identity and
intended Studio access. An operator must securely provision the accounts on the existing issuer,
restrict the dedicated Studio client to those accounts through a reviewed provider group, read
their exact subject identifiers through an authorized channel, then deliver the confidential
client secret through the existing server secret channel. Do not create or request passwords in
chat or source control. No account creation, live group grant, client configuration or credential
change is executed by this slice. The [operator contract](../../services/kanidm/README.md#studio-administrator-provisioning-contract)
keeps these activation steps separate from code and synthetic tests.

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

`GET /api/v1/studio/auth/configuration` returns only the closed `development | oidc` presentation
mode, without credentials, provider locations, identity or a session cookie. Studio Web reads it,
then confirms the actual browser cookie with `GET /api/v1/studio/session` on mount, explicit
refresh and idle browser-history restoration. Callback navigation alone establishes no UI
authority. Epoch-fenced replies cannot restore a logged-out or superseded account. Local draft
ownership and ambiguous saved intent remain attached to the original actor.

In OIDC mode Studio renders provider sign-in and local logout instead of development token entry.
Provider sign-in first performs same-origin local logout, revoking the old local session, then
navigates to the login route. Existing browser-unload protection covers unsaved edits. Development
mode retains token exchange plus cookie confirmation. Config/client tests are separate from the
required real-browser interaction and accessibility observations.

`GET /api/v1/studio/auth/login` redirects to the pinned provider. Cross-site Fetch Metadata or a
mismatched Origin is refused. A valid previous Studio session is remembered for rotation.
A positively known locally revoked/expired current-issuer OIDC cookie can begin anonymously;
unknown cookies, active disabled actors and provider outages remain closed. A browser carrying
an old development cookie when OIDC is enabled must use explicit legacy-cookie retirement.

Additive 0025 assigns every session a public noncredential UUID. The current-session response
echoes it in `X-Cantos-Studio-Session`; the UI freezes UUID plus observation epoch synchronously
for each private request. Protected responses echo the actual UUID, and an expected mismatch
returns 409 before dispatch without changing cookies or authority. Downloads and browser login
navigation carry the same UUID as `studio_session_id`, while the cookie remains mandatory.
Only a fresh ticketed actual-cookie observation adopts another UUID; a 409 header cannot do so.
Legacy explicit clients may omit the fence; the current UI refuses private requests without it.

A matching 401/409 invalidates private presentation across all Studio workspaces. A delayed A
denial cannot invalidate B or a newer observation of A. Successful responses missing/mismatching
the expected UUID are discarded as unknown outcomes: private presentation is hidden but the
original operation snapshot remains available for receipt reconciliation. Dirty authoring,
content and import drafts remain bound to their owner through pending/failure/logout/account
switch; another actor cannot see or dispatch them. Clean acknowledged imports reset on a new
actor. Explicit refresh preserves dirty ownership and can restore its owner after A/B/A;
idle restoration defers while drafts or operations need preservation. Native composition/file
activity also counts as retained private ownership. Clean adoption resets all old source IDs,
revision metadata and validation/review state.

State/binder hashes, nonce, PKCE verifier, issuer, client, redirect and previous session hash are
stored with a 300-second expiry. Each flow has an immutable state-derived cookie name under the
Studio binder prefix; a new flow never overwrites another binder. Begin admits fewer than eight
existing inventory entries; callback/logout reject duplicate, malformed or overflowing inventory
without truncation or partial acknowledgement. Concurrent initial responses can exceed that
limit; the resulting inventory fails closed until cookies expire. The binder cookie is
HttpOnly/SameSite=Lax because a top-level
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
All three confidential-client endpoints form-encode client ID and secret before HTTP Basic,
as required by [RFC6749§2.3.1](https://www.rfc-editor.org/rfc/rfc6749.html#section-2.3.1).
The existing pinned `url` serializer owns this encoding; secrets remain backend-only.

## Studio sessions, disable and logout

Additive [0020](../../services/cantos-api/migrations/0020_studio_oidc.sql) follows inherited 0018;
0019 belongs to Library/Progress. Original SQL checksums are unchanged. Existing sessions gain
`auth_kind` defaulting to `development`. New OIDC sessions reuse 32-byte SHA-256 hashes and actor
FKs; expiry is the minimum of ID-token expiry, introspected access expiry and 15 minutes, with no
refresh token. Additive [0025](../../services/cantos-api/migrations/0025_studio_oidc_final_time.sql)
replaces the issuance function while preserving 0020 and its grants. SQL rechecks flow, proof/access
and previous-session expiry after registry/actor waits, then after every issuance effect and
constraint lock wait. Expiry at the final fence raises a private exception and rolls back the
entire function subtransaction (actor/binding/session/receipt, old revocation and flow completion)
before returning the typed refusal; consumed-before-exchange remains committed. Receipts record
their source flow and immutable rotation lineage root. Lineage advisory locks serialize rotation
and logout before sorted flow/session/receipt row locks; the root never grants cancellation
authority. The receipt's unique
proof hash blocks a duplicate ID-token admission. New identities create an actor/binding
atomically; existing identities reuse the active actor. This function cannot link to an arbitrary
legacy actor, grant resource permission or reactivate an account. Explicit linking remains an
operator action. Runtime has no direct registry/actor/session INSERT privilege.

Provider bearer credentials stay in the backend's private OIDC session table. They are retained
for introspection/revocation, never returned to browsers or logged. The test/local slice stores
these private values as text; production must review encryption/secret delivery, backup access,
retention and limited runtime-role grants before activation. The only browser credential is a
random 256-bit opaque cookie. HTTPS uses `__Secure-cantos_studio` with Secure/HttpOnly/SameSite=Strict,
no Domain and Path `/api/v1/studio`. Flow cookies use `__Secure-cantos_oidc_<state>`, Lax.
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

`DELETE /api/v1/studio/session` requires exact Origin and an expected session UUID or explicit
stale-only cleanup mode. An active expected session cancels its validated bounded inventory and
revokes its issued sessions, including a callback committed before cookie acknowledgement.
An inert A cannot cancel a fresh anonymous B flow. Cancelling a pending/issued rotation from A
requires that captured private binder, rechecked against source/lineage and server expiry after
every lock wait. A final clock check after all cancellation effects rolls back the transaction
if a supplied binder or the active session authorizing inventory cancellation expired; the
rollback completes before a definitive refusal. Missing or expired binders leave observed B
active. Attributed retries include
only already revoked pending rows, never newly active B. The operation commits local revocation
first, then attempts the current provider credential, reporting
`Cantos-Provider-Revocation: completed|pending`. Guarded current-UI requests emit no global cookie
expiry headers: a late A response cannot delete B's newer session or flow. The revoked cookie is
inert and a subsequent login overwrites it. Automatic cleanup uses `X-Cantos-Studio-Stale-Only: 1`;
a currently valid cookie returns 409 and a provider outage returns 503 without any revocation.
Unsigned cleanup preserves pending guest flows; explicit UI logout is enabled only after actual
cookie confirmation. Unfenced OIDC logout is refused. Safe guarded legacy retirement expires
old unprefixed HTTPS names or the deprecated HTTP `/api/v1` path, never the current credential
name/path. Duplicate HTTP names return 409 for fresh observation without revoking an ambiguous
token. A positively known inert local development token can begin anonymously after cleanup;
an unknown token cannot. Callback
completion also leaves the already-consumed 300-second flow binder to expire naturally, so it
cannot expire a newer tab's flow. An outage preserves the
pending row. Captured siblings remain durably attributed and pending for controlled operator
recovery. `completed` requires success for the current credential and no attributed pending
sibling, including inert retries without expired binders. An old-issuer credential is never
sent to the newly configured issuer's revocation endpoint;
local revocation commits and that old receipt remains pending. There is no background recovery
daemon or implicit federated/global logout. The provider's
SSO session may remain; signing in again can establish a fresh Cantos session. Rotation also marks
the old provider credential pending, while rejecting the old Cantos token immediately. Operator
recovery for abandoned pending rows is a remaining live/operational gate.

## Affected read inventory

| Read | Frequency / semantics | PostgreSQL path / assessment |
| --- | --- | --- |
| Consume/finalize flow | One matching callback; strong single-use/current expiry | state_hash PK; exact bounded inventory and source-state association |
| Authenticate OIDC session | Per request; current active/revoked/expiry | sessions and OIDC-session hash PKs, actor PK; no cached authorization |
| Register issuer/subject | Per successful callback; exact identity/active state | PR1 C-collated composite PK and identical lock digest |
| Pending logout fact | Per explicit logout/retry; retained observation | token_hash PK and revoked-by pending index; no background aggregate consumer |

No repeated join/aggregate consumer earns DBSP; authentication must retain current transactional
facts. Source/lineage/revoked-by indexes support the concrete scoped cancellation reads; no read
cache or speedup is claimed. The real PostgreSQL gate exercises these
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
