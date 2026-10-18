# Theatre identity and account isolation

> **#253 Bearer amendment (2026-10-18):** the target API credential is a Cantos-issued opaque
> Bearer with rotating refresh. The [Bearer v1 contract](../../packages/contracts/bearer-v1.md) owns new routes/lifecycle,
> JSON-only identity wire, fresh-auth evidence, RAM/F5 and compatibility cutover. Cookie-only
> statements below describe the retained **legacy compatibility** server flow;
> they no longer impose a same-site domain or cookie prerequisite on the target architecture.
> Web and native consumers use Bearer. Legacy server issuance cutover and real
> gateway/issuer/browser/native acceptance remain separate.

Implemented PR3 boundary for #12/#35; issue #34 has no accepted guest-import policy.
Public Discovery/Playback stays guest where its existing policy permits. Persistent Saved,
History and Continue/Progress requires a verified account. The identity boundary introduces no
Library domain, progress store, guest merge, native audio player or gateway. The separately scoped
[#129 Web presentation follow-up](../evidence/theatre-web-auth-account.md) mounts public Home and
contextual auth/account over the existing identity owner; its runtime evidence remains distinct
from live-provider and native acceptance.

## Native Bearer consumer (#253)

The production Android/iOS composition uses the [native lifecycle and secure journal](../../apps/cantos-theatre/mobile/README.md#opt-in-native-identity).
One process owner coordinates exchange, API-confirmed restore, single-flight refresh and
family revoke. An interrupted exchange/rotation is cleanup-only; the environment-bound encrypted
record is replaced and read back before authority is published. Logout invalidates local intent
before IO, and private HTTP/SQLite effects check account, family and rotation leases. Same-family
rotation preserves the existing full production listener UI, player and durable operation IDs.
The current public native media path uses narrow release capabilities, not account-token URLs.

Original proof/provider expiry remains the ceiling. Issuer-specific account links are not
inferred from email or display name. Exact new Web callback reconciliation is implemented
locally; rollout requires a separately authorized operator reconciliation. The default-deny
gateway source and real browser/issuer/Android/iOS acceptance remain open. Legacy issuance is
retained only for the explicit server cutover; no target client falls back to it.

## Accepted enrollment policy and current capability

[ADR 0034](../decisions/0034-theatre-signup-and-provisioned-studio.md) accepts listener
self-registration and limits Studio to one or two securely provisioned administrator accounts.
The former self-signup/invitation/defer product choice is resolved. Theatre identity, account
creation and Studio application admission remain separate facts; a Theatre account never gains
Studio authority through signup or an existing registry binding.

The [Theatre configuration contract](../../packages/contracts/theatre-identity-v1.md) reports
login readiness separately from registration capability. Kanidm 1.10.5 has authenticated person
creation and credential-update-intent APIs, although its public UI has no integrated signup.
On 16 October the product owner approved a Theatre email-verification/onboarding module around
those APIs, keeping the issuer and its credential UI. Missing integration configuration remains
an unavailable capability; local adapter construction never establishes deployed authority or
successful signup. The source audit, accepted direction and live-operation boundary are in
ADR 0034. OIDC discovery alone cannot enable registration.

The delivery order is pure registration rules and contract, deterministic adapters/HTTP/Web,
disposable PostgreSQL, then separately authorized issuer/mail and real Web acceptance. No real
provider account, service credential, standing grant, SMTP setup or security-setting change is
authorized by code/test approval. Native signup wiring and device acceptance remain separate.
See the
[handoff](../work-plan/theatre-identity-handoff.md#accepted-enrollment-policy-and-web-first-handoff).

### Email onboarding owner and effect boundaries

[#206](https://github.com/loveoverflowcom/cantos/issues/206)'s
[ADR0036](../decisions/0036-firebase-enrollment-proof.md) accepts Firebase mailbox proof with
retained Kanidm credentials/OIDC and one-shot setup delivery to the confirming browser.
The selected default-disabled mode extends the existing registration owner with bounded
Firebase bootstrap/action adapters, additive migration0028, thin HTTP contracts and Web
confirmation. The SMTP mode below preserves its existing setup-mail sequence; Firebase never
implements its generic Mailer or becomes an automatic fallback. The
[runtime evidence](../evidence/firebase-enrollment-runtime.md) separates local tests and
emulator/SQL/browser results from still-unaccepted live activation. Review the
[operator runbook](../deployment/firebase-enrollment-proof.md) before any activation.

Firebase enrollments permanently bind project/operation/UID/email/deadline; cancelled/expired
operations never reopen. Resend uses the same live UID and absolute deadline, without claiming
old-code invalidation. Each mutating effect has a durable phase, including a fresh actual-mail
fence after token exchange. Same-action inspection/consumption precedes every Kanidm effect.
The initiating browser has no proof/status/setup endpoint. Explicit cancellation also requires
actual action possession and consumes it without provisioning. A one-shot setup response is
never a session, and losing it does not permit replay. Existing Firebase-email conflicts and
unknown effects require reconciliation, with no adoption/reset/automatic deletion.

`platform::theatre_registration` owns this workflow in the existing API. Its pure values and
phase rules depend on neither HTTP, SQL, Kanidm nor SMTP. Application ports have PostgreSQL,
Kanidm and vendor-neutral SMTP adapters plus isolated doubles; the Web layer consumes the
versioned contract and does not construct authority. Registration never writes the actor
registry, issues a session or changes Studio's administrator roster.

The sequence is request → mailbox proof → positively acknowledged **new** provider person →
credential-setup link sent to that mailbox → issuer credential UI → existing OIDC login/session
confirmation. Registration accepts no password, provider person ID, issuer, role, arbitrary
return URL or separate delivery recipient from the client. Verification links carry a random
32-byte token in a fragment; only its purpose-separated hash is persisted. Opening the link
shows a confirmation step; an explicit same-origin POST claims it before provider effects.
GET/prefetch alone must have no account-creation effect.

Migration0027 records the exact configured issuer namespace, immutable operation/email binding,
hashed challenge, expiry, phase and acknowledged provider person ID. Budget receipts record
accepted verification-mail starts, including later failed/uncertain delivery; raw verification
or issuer setup secrets are not retained. SQL serializes the budget decision and token rotation,
then atomically consumes a valid unexpired proof. A consumed, cancelled, replaced or expired
proof cannot acquire another create lease. Locks are followed by a fresh expiry check.
The current migration retains registration and append-only mail-budget receipts and refuses
DELETE/TRUNCATE. It implements no age-based PII cleanup; accepting a retention period requires
a reviewed cleanup/migration design rather than assuming expired proofs remove those rows.

Before each non-idempotent create/intent/mail effect, the operation advances durably. The
proposed provider name and UUID derive from an operation UUID, not email or caller input. Only a
positive create acknowledgement yields the witness needed by the intent adapter. Conflict,
lost response, malformed success, failed receipt commit or process loss cannot trigger a
lookup-and-reset fallback. Once provisioning starts, public resend/retry cannot roll it back to
a retryable proof. A rejected or uncertain effect blocks automatic recovery; no exactly-once
email, background recovery or cleanup procedure is claimed. An operator must reconcile the
exact operation before any specifically authorized recovery; existing people are never deleted
or reset to repair a signup attempt.

Mailbox proof does not resolve the policy for an address already present outside these
registration receipts. The current adapter does not establish provider-wide email uniqueness;
it must not claim to deny all existing-email signup or to link identities by mailbox. That
policy, bounded ASCII/case-insensitive deduplication and operation-derived account naming remain
review items before activation. Changing mailbox requires a separate proof; there is no
client-controlled mutation of an in-flight binding.

SMTP acceptance, inbox delivery, verified mailbox, setup-link delivery, completed credentials
and authenticated Theatre session are distinct facts. UI copy and evidence must keep them
separate. Public request handling returns a generic acknowledgement before bounded mail work;
suppressed duplicates, cooldown/quota decisions and existing local receipts do not expose an
account-existence response. This does not constitute a measured network timing proof or solve
Internet abuse. Every registration POST requires socket peer metadata and process-local rolling
global/peer attempt admission plus bounded concurrent effects. IPv4-mapped IPv6 is normalized
before a private-key HMAC partitions the client address: the socket peer under `direct_peer`,
or, under `trusted_proxy`, the `X-Forwarded-For` value set by a configured proxy network (the
header is ignored from every other peer). These
attempt counters reset on restart and do not coordinate across hosts. Durable accepted-mail
budgets/cooldown remain a separate PostgreSQL boundary. Deployment-wide ingress limiting,
personal quota-metadata retention and cleanup need explicit review. `trusted_proxy` is
implemented and unit/HTTP-tested; the VPS edge's address and live behavior remain operator
acceptance ([runbook](../deployment/vps-production.md#theatre-registration-behind-the-edge-not-enabled)).

Host composition defaults off. `platform::theatre_registration::config` requires an existing
Theatre Web identity adapter, explicit provider bearer/HMAC/SMTP inputs, all bounded lifetime,
budget and attempt values, and `PEER_POLICY=direct_peer` or `trusted_proxy` with
`TRUSTED_PROXIES` (the list is refused with `direct_peer`). Missing or standalone false `ENABLED`
disables only when no other registration/SMTP inputs are present; partial/orphan setup fails
startup. `POLICY_APPROVED=true` and `ISSUER_ACCESS_APPROVED=true` under the
`CANTOS_THEATRE_REGISTRATION_` prefix are required operator declarations. They cannot establish
effective provider privileges or live delivery. Construction performs no discovery, network
probe, account/grant/credential creation or mail send. The
[runbook input table](../../services/kanidm/README.md#explicit-host-inputs) owns the exact names
and constructor bounds; no fixture value is selected as a production default.

Activation also requires effective Kanidm target protection. `idm_people_on_boarding` can
reset existing people; neither the group name nor Cantos's new-person witness makes a stolen
bearer safe. A separate operator/security review must establish issuer-side protection for
existing Theatre/Studio people or approve a concrete alternative and residual blast radius.
Studio's one/two-subject server admission remains required but is not a provider credential
protection mechanism. Keep registration disabled while that gate and the unapproved live mail,
credential-delivery, retention and recovery policies remain unresolved.

## Decisions and critique of the original proposal

The October13 proposal described an old develop snapshot. Current develop has merged Publication,
Discovery and Library; PR92 registry/PR98 Studio remain published dependencies. A registry lookup,
actor ID or Studio cookie is not authentication. The existing sealed Library seam is the authority,
not a new generic ActorContext/header system. Exact issuer/subject is mapped only after signature,
audience, time and nonce verification; email/name does not link accounts. Pairwise subjects across
clients may map to different actors until an explicit linking action is authorized. Equal subjects
under different client-specific issuers also map independently: issuer is never normalized across
clients. A shared Web/native Library needs an explicitly authorized registry link after both
identity proofs; no automatic cross-client account correspondence is inferred.

Web uses a separate BFF client `cantos-theatre-web-bff`; provider tokens stay off the JS boundary.
Native uses a separate public `cantos-theatre-native` protocol and OS storage. Web session tokens
are not native bearer tokens. Shared `platform/oidc` contains crypto, bounded transport, config and
errors; Studio owns its original session shell/SQL and Theatre owns its separate shell/SQL. This
extracts real repeated plumbing without changing Studio client/scopes/callback/cookies. Admin may
reuse plumbing later, but gets no permission/session admission from either existing surface.

Native reference audit read the current local VOT Workspace at
`3de94ae93790550d1edcb4d86fce17d005fbbd29`: Votex PKCE/callback/nonce and keyring adapter, portable
auth core and the gateway's private proof boundary. No gateway/header login, certificate bypass,
password flow or fallback file storage is copied. Protocol/platform references are
[RFC8252](https://www.rfc-editor.org/rfc/rfc8252),
[Android Keystore](https://developer.android.com/privacy-and-security/keystore),
[Apple Keychain](https://developer.apple.com/documentation/security/keychain-services).
Reading these is not native/provider runtime evidence.

## Implemented backend boundary

Theatre issuer is exact `/oauth2/openid/cantos-theatre-web-bff`, independent of Studio issuer.
Config pins origin/redirect and requests only `openid`. Shared ES256 verifier checks kid/key usage,
signature, exact issuer/audience/azp, exp/iat/nbf, nonce and subject; bounded JWKS refresh and injected
clock behavior retain PR98's tests. Redirects/TLS bypass are refused; HTTP is literal loopback only.

Migration **0021_theatre_identity.sql** adds separate flow and session tables and narrow trusted
issuance. It preserves all older SQL/checksums; registry0018 remains mapping authority and0019
remains Library. Issuance uses the exact registry lock, current actor locks and fresh time after
lock waits, before registration/session mutation. Consume commits before the single token exchange;
lost responses/crashes never authorize replay. Proof hash is unique; expiry is bounded by proof,
access introspection and15minutes. Same previous session has one rotation winner.

Cookies use independent names/path: HTTPS `__Secure-cantos_theatre` Strict and
`__Secure-cantos_theatre_flow` Lax; both HttpOnly/Secure/host-only/Path `/api/v1/theatre`.
Loopback names are `cantos_theatre` and `cantos_theatre_flow`. Alternate namespaces and duplicates
are refused. Logout requires exact Origin, clears local/session/flow immediately and records remote
revocation pending on outage. Provider SSO/global logout is not promised.

The actual `TheatreListenerAuthenticator` checks opaque local session, introspects current provider
subject/audience/expiry, rereads local active/revoked/expiry, then mints PR95's sealed witness.
Normal host remains default deny without Theatre config. Enabled BFF account middleware refuses
direct Authorization headers, verifies the cookie through this adapter and takes current session
and actor SHARE locks before dispatching Library. A sealed request extension avoids repeating
network under locks. Logout/disable waits for already admitted requests; a request admitted before
expiry may finish. Network happens before SQL admission lock; no provider call is made inside it.
The outer15second deadline bounds the request/lock lease. Remote disable is observed at
introspection and cannot be atomic with local SQL; previously admitted work can finish.

Provider bearer remains private text for test/development introspection/revocation, as in PR98.
Production must accept at-rest/backup/role/secret/rate/retention and pending-revocation recovery
before activation. Only existing development/loopback host is configurable here; no clients,
grants, credentials, TLS, network, container or production activation is provisioned.

## Web and CMP lifecycle

Theatre Web mounts its same-origin BFF account owner in the default application shell. Anonymous
Home reads published work summaries independently with credentials omitted; session lookup,
readonly Library and explicit Login/Account routes retain separate ownership. Account/session
generation fences success and failure callbacks and clears private projections immediately on
logout or session invalidation. Cleanup acknowledgement, stale-only cleanup and expected-session
mismatch remain the login barrier. No durable offline outbox, Library write or implicit guest
merge is claimed. The listening mock and auth-state diagnostic are explicitly selected modes;
the auth diagnostic injects fixtures into the same Home/Auth/Account composition with live effects
disabled. See the [Web implementation boundary](../../apps/cantos-theatre/web/README.md#explicit-live-and-mock-modes)
and [#129 evidence](../evidence/theatre-web-auth-account.md). Native acceptance is recorded separately
in the campaign section and the
[14 October evidence appendix](../evidence/theatre-identity.md#resumed-native-identity-acceptance--14-october-2026).

The Web shell stores only canonical internal Login return context and a random 32-character
lowercase-hex nonsecret UI intent, plus bounded nonsecret scroll/focus restoration data, in session
storage. `/auth/login?intent=…` accepts that bounded intent; the backend binds it to the exact OAuth
state with a separate state-named HttpOnly/SameSite=Lax companion cookie under
`/api/v1/theatre/auth`, using the Secure namespace on HTTPS. This does not replace or extend the
private browser binder, state/PKCE/nonce validation or flow expiry. Provider tokens remain off the
browser boundary. Registration has a distinct hashed mailbox proof and never upgrades that
proof into a session; setup-mail acknowledgement directs the listener back to the existing
login/session owner. No password form or fabricated account completion is introduced.

Successful callbacks emit a fixed `/#auth-return?intent=…`; bound provider cancellation emits
`/#auth-cancelled?intent=…`. Provider error text and caller redirect URLs are not reflected. A
legacy callback without an intent has a fixed hash and no contextual completion authority. The
pure parser accepts only the exact matching intent and canonical internal Login route: old A's
callback cannot consume newer B's context. A matching completion is presentation intent only;
the mounted owner must confirm the actual current `/session` before returning to the origin.
Back, history navigation, cancel and logout synchronously invalidate old presentation epochs.

Provider cancellation is durable and exact to its private state/browser binder, issuer, client,
redirect and unexpired flow. It marks that flow consumed/cancelled and locks the flow before any
session issued from it. A paused exchange then fails final issuance; if issuance committed first,
only that flow's session is locally revoked. Fresh time is checked after lock/write waits. Local
cancellation/revocation commits before bounded best-effort provider cleanup, retaining pending
remote revocation on failure. Another flow/account B is not selected, and only A's nonsecret
companion return cookie is expired; no shared session-cookie expiry or global logout is emitted.
The [new callback/SQL scenarios](../evidence/theatre-web-auth-account.md#exact-provider-return-and-cancellation)
cover exact correlation, forged binders, late issuance and A cancellation preserving B; signed
provider mocks remain distinct from live Kanidm acceptance.

CMP common code has equivalent account fences and a native login state machine. One owner uses
system browser, fresh32byte entropy and S256; callback validates exact redirect/state before a
single exchange effect. Nonce accompanies that effect for future token verification. Cancel or
new login fences old results; duplicate callback cannot dispatch again or invalidate the active
exchange. Pending attempts are deliberately not restored after process death. No secret or login
WebView is embedded. The common core never constructs an authenticated account from a code.

Android adapter uses ACTION_VIEW, exact Uri parser, SecureRandom/SHA256 and nonexportable
AndroidKeyStore AES-GCM; ciphertext lives in AtomicFile under noBackupFilesDir, bounded/redacted
with no plaintext fallback. A cancelled external browser may remain open; protocol state rejects
its callback. iOS adapter source uses ASWebAuthenticationSession/cancel correlation and Keychain
AfterFirstUnlockThisDeviceOnly, no synchronization/file fallback. Swift adapters are included in
the Xcode Sources phase and mounted through the Kotlin/Swift bridge and `IosNativeOwner`; the
native exchange/resource transport is implemented. The canonical Kotlin/Native 2.2.20 simulator
framework build is UNSUPPORTED here. OS lifecycle and secure-store persistence acceptance remain
NOT_RUN; source mounting or compilation does not establish simulator/device behavior.

## Read assessment

Session/flow reads are indexed hash point reads; issuer/subject uses registry's C-collated PK.
Authentication is per request, strongly current local admission and provider observation; no cache
or aggregate consumer warrants DBSP (**reject for this scope**). Concurrency/grant/read semantics
need disposable PG tests. Target workload, not measurement:1000active sessions,20concurrent account
requests, local point-read p95≤20ms and no cross-account results; introspection end-to-end latency,
throughput, EXPLAIN/BUFFERS and production rate budgets remain unmeasured/deferred. No speedup claim.

Final adapter location is `contexts/listening/theatre_identity.rs`, exported through Listening auth;
technical persistence owns the lease and HTTP dispatches only. Three bounded admission slots leave
dispatch capacity in the existing pool. Expiry is read after actor wait. Separate native code-response
verification in `platform/theatre_identity/native.rs` has no principal/session conversion or mounted
resource authority by itself. The native consumer below supplies trusted transport, durable
exchange, current revocation observation and registry/session composition.

## Native integration added by independent campaign review

`NativeIdentity` uses a closed public-client authentication mode: no secret can be supplied to
native exchange/introspection/revocation. Exact issuer/client/redirect and bounded same-origin
discovery/JWKS are pinned. The device creates state/nonce/PKCE and uses the OS browser; the API
durably consumes before its one public exchange. It verifies ES256 OIDC nonce and RFC9068
`at+jwt` issuer/audience/client_id/time/scope, exact same subject and current introspection.
Only private verified evidence reaches exact registry issuance. ID/access token purposes cannot
substitute for each other.

The [pinned Kanidm 1.10.5 source](https://github.com/kanidm/kanidm/blob/v1.10.5/server/lib/src/idm/oauth2.rs)
implements public exchange with form `client_id` and no secret, and token-possession introspection/
revocation without client authentication. Its discovery advertises `none` for the latter two
while token discovery lists Basic/Post even for public clients. Native config explicitly pins
public-client scope; a confidential registration fails exchange. Actual provider configuration,
TLS and live acceptance remain unperformed.

Migration 0024 adds distinct native flows/sessions. Native credentials are opaque random bearers;
provider tokens remain private. Each resource request verifies the provider JWT, introspects,
rereads the active local actor/session and takes bounded admission locks. Native credentials do
not authenticate Studio, Admin or Web `/session`. Mixed Cookie/Authorization is denied. Local
logout commits before remote revocation; outages retain durable pending evidence. Provider
observation cannot be atomic with local SQL; already admitted work may finish. No refresh
credential or offline authorization is introduced.

Both native issuance and the replaced Web issuance function recheck flow/proof validity after
INSERT/trigger/index waits. Final expiry raises a rollback exception, preserving no new actor,
binding or session. Historical 0021 bytes stay immutable. Resource/session fence headers reject
old-account intents before dispatch; guarded Web logout cannot revoke a newly valid cookie
during stale-only cleanup or an expected-session mismatch.

Guarded/stale-only Web logout responses emit no shared cookie expiry headers. A response can
arrive after a newer login even when the old request was correctly fenced at dispatch. Local
revocation leaves the HttpOnly cookie inert until replacement/expiry; begin accepts it as
anonymous only after a positive exact-issuer/client local revoked/expired query. Unknown
credentials and provider uncertainty remain closed. Callback responses likewise omit global
flow-cookie expiry; durable consumed/completed state enforces replay refusal and binders retain
their 300-second expiry. Each flow has an immutable state-named HttpOnly binder, so another
begin cannot overwrite evidence of a pending callback. Callback parsing is exact and refuses
legacy mutable cookies, duplicate/malformed pairs, wrong Secure names and inventories over
eight. Begin refuses allocation at eight. Concurrent initial requests can exceed the observed
cap; overflow is never truncated and requires binder expiry before retry.

Migration 0024 associates each Web session with its source flow. An active expected-session
logout validates inventory pairs, locks all flow rows in state-hash order (including its own
source), then session rows in token-hash order, and refreshes the current clock afterward.
It atomically cancels pending inventory flows and revokes already-issued siblings before remote
effects. Issuance uses the same flow-before-session order; a callback either commits first and
is revoked, or observes cancellation and issues nothing. Stale-only cleanup and locally
revoked/expired A retries cannot cancel B's newly intended anonymous flows. Rotation flows and
sessions propagate an immutable root, acquiring its advisory lock before flow locks during
begin, issuance and logout. This only serializes related rotations. Inert expected-A cancellation
still needs the captured exact B/C private binder and source receipt, with a matching root and
server-side expiry rechecks after locks and writes. Root identity or retired A/UUID alone cannot
revoke observed B/C; expired binders are not standing cancellation capabilities. A-bound
rotations also fail the locked `previous_hash` revocation check. Unguarded HTTP DELETE is refused without effects;
no actor-wide, issuer-wide or cross-device logout policy is introduced.
The session's durable revocation-batch attribution keeps sibling remote acknowledgements tied
to the original logout. Provider effects share a five-second overall deadline; timeout leaves
local revocation committed and the response pending. An inert-A retry can recover already-revoked
pending siblings without old binder cookies and without cancelling fresh B. No recovery daemon
or production-provider acknowledgement is represented as implemented acceptance.

The common client/controller has strict DTO decoding, fenced callbacks, unknown-code-outcome
refusal and explicit restore only after server validation. Keystore/Keychain hold only the
native credential; a nonsecret durable marker prevents restoration after uncertain logout.
iOS mounts the Swift system browser/URLSession/Keychain bridge with both pinned HTTPS config
fields. The user-authorized Android INTERNET/callback/Application/MainActivity source is now
mounted and the canonical mock Android build passed. Its packaged manifest confirms the callback
routing declaration and `singleTask`; actual OS dispatch remains unverified. Missing/invalid
operator configuration retains the isolated mock. No live origins, app installation, cleartext/TLS
override or deployed/OS network setting was introduced.

Android build PASS is separate from OS storage/browser/cancellation/crash/restart, device,
accessibility and provider acceptance, which remain NOT_RUN. The canonical iOS framework gate
remains UNSUPPORTED. A pending native denial marker with lost/corrupt credentials stays closed;
controlled recovery is unassigned and no acknowledgement/reset is invented. The API host remains
development/loopback-only; absent `CANTOS_THEATRE_NATIVE_ISSUER` keeps native routes disabled.
