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
clients may map to different actors until an explicit linking action is authorized.

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
