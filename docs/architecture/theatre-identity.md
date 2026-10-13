# Theatre identity and account isolation

Implemented PR3 boundary for #12/#35; issue #34 has no accepted guest-import policy.
Public Discovery/Playback stays guest where its existing policy permits. Persistent Saved,
History and Continue/Progress requires a verified account. No Library domain, progress store,
guest merge, native audio player, UI redesign or gateway is introduced.

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

Theatre Web has an unmounted same-origin BFF port and an account lifecycle core. Account/session
generation fences success and failure callbacks, clears projection immediately on leaving and
retains immutable account-owned intents. Once dispatched, unknown outcomes require receipt lookup
first. Reauthentication asA can recover A's original intent/body;B cannot dispatch or acknowledge
it. Storage is memory only in this helper: no durable offline outbox or actual Library UI sync is
claimed. Existing anonymous/mock UI is preserved until separately integrated against proven API.

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
AfterFirstUnlockThisDeviceOnly, no synchronization/file fallback. Swift source is not mounted in
the mock Xcode host; Kotlin/Swift bridge, lifecycle and native exchange/resource transport remain
deferred. Compile evidence is not simulator/device/secure-store persistence acceptance.

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
fields. Android manifest activation awaits a source-only human exception after automatic
approval review rejected the INTERNET/callback source change; no cleartext/TLS override or
deployed network setting was changed.

Native builds, OS storage/browser/cancellation/crash/restart, devices, accessibility and provider
acceptance remain separate unrun gates. The host remains development/loopback-only; absent
`CANTOS_THEATRE_NATIVE_ISSUER` keeps native routes disabled.
