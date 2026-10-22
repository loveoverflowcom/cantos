# Cantos Web session owner (Identity v1 cookies)

`cantos-web-auth` is consumed by **both** Leptos Web apps. It imports contracts, never app or
service code. Studio and Theatre keep separate document-scoped owners, API prefixes and cookies.
[Identity auth v1](../contracts/identity-auth-v1.md) owns the server wire, lifetimes and error
semantics; [ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md) and
[ADR 0045](../../docs/decisions/0045-v1-email-password-sign-in.md) own the topology and factor
scope. This package selects no issuer and grants no permission: a session proves a login, never
a Studio, producer, membership or Admin capability.

The old OIDC popup, PKCE, `ca1_`/`cr1_` token pair and RAM Bearer engine are **removed**. There
is no Bearer or provider fallback. Native Bearer belongs to CMP, not to this package.

## Implemented client boundary

- The credential is an API-issued `HttpOnly` cookie (`__Secure-cantos_id_{theatre,studio}`,
  `SameSite=Strict`, host-only, `Path=/api/v1/{surface}`). No script reads, stores, forwards or
  logs it, and no body, URL or media URL carries it. This package keeps only **nonsecret session
  facts** in document RAM: actor id, session id (a fence, never a credential), idle and absolute
  deadlines, authentication time.
- Every user-session request is `credentials: include` with redirects refused, and carries
  `x-cantos-{theatre,studio}-session` with the session the client believes it holds. The browser
  adds `Origin` itself; application code never forges it. `Lease::apply` is the single place a
  protected `fetch` receives that policy; XHR callers take `Lease::fence` and set
  `withCredentials` themselves. `core::credentialed_target` is the only way an arbitrary URL
  (artwork upload) may receive the ambient cookie: exact API origin, this surface's path only.
- Public catalog/media, producer capability Bearer and signup requests are **not** user-session
  requests and never use this policy.
- `Client::login(email, password, expected_actor)` is one attempt, never retried. A definite
  refusal (400/401/403/404/429) leaves the previous local state; an unknown outcome (the cookie
  may have been replaced) drops it, so only an authoritative `read_session` can re-admit a
  session. A login answered for another actor than `expected_actor` (Studio re-authentication as
  the same owner) is never adopted and its fresh cookie is revoked. A late login response of a
  cancelled/logged-out intent is revoked, never adopted.
- `Client::read_session()` (`GET identity/session`, no fence) answers "who am I now?" and is the
  startup/reload path: empty RAM never means signed out; only a 401 does. A different session
  than the local one switches intent and fences every older lease, so a delayed A response can
  never update B.
- `Client::access()` admits one protected request. It uses the session, renews the **idle**
  deadline when `RENEW_MARGIN` seconds remain (a single shared flight; the cookie value is
  unchanged, so an unknown renewal outcome is safe to retry later), and re-reads the session when
  the local idle/absolute deadline has passed instead of concluding signed-out from a clock. The
  absolute deadline cannot be extended: after it the API answers 401 and the user logs in again.
- Local time is the wall clock with a monotonic floor (`core::Clock`), so a device clock moving
  backwards never extends a session; deadlines remain the server's.
- 401 → signed out (session/protected calls) or `InvalidCredentials` (login). 409 → another
  tab/intent changed the account: drop the local session and re-read. 429 → `RateLimited(n)` with
  a bounded `Retry-After` and a local gate against dense retries. 503/network → `Unavailable`,
  never signed-out. 403 → `Forbidden` (Studio: verified password without a roster seat). 404 →
  `NotFound` (Identity not configured, no Studio roster): a clear error, never a fallback.
- Logout invalidates the local intent first (`begin_logout`), then `DELETE identity/session`
  fenced to the closed session. 204, 401 and 409 are terminal; any other outcome stays queued in
  RAM and blocks new admission/login until an acknowledged retry. Cleanup is pinned to the
  original session, so a late A cleanup cannot close B. Browser termination can lose the queue;
  the server's idle/absolute deadlines remain the bound.

## Signup is not here

Theatre signup (`identity/signup`, the mailed `#operation=…&proof=…&secret=…` fragment and
`identity/signup/complete`) belongs to the Theatre app because it authenticates nobody: completion
creates an account and **no session**. See the [Theatre Web README](../../apps/cantos-theatre/web/README.md).

## Protected resource inventory

| Consumer | Transport / boundary |
| --- | --- |
| Theatre session, Continue, Saved, History, progress, bookmarks | `Lease::apply`, expected session fence, response/body/intent checks, JSON only |
| Studio revision head/history/save/validation | `Lease::apply` plus the UUID fence echo; existing per-route JSON/Protobuf codec and ambiguous-write receipts retained |
| Studio catalog/content/brief/audio metadata/delegations | owner session; producer status/upload keeps its distinct work-capability Bearer with cookies omitted |
| Artwork upload | XHR `withCredentials=true` + fence header to a `credentialed_target`; fetch fallback via `Lease::apply` |
| Private artwork preview, original-source download | authenticated fetch via `Lease::apply`, exact identity/hash/format checks, owned revocable object URL / ephemeral Blob |
| Public catalog, public media, signup | credentials omitted; never cookie-bearing |

## Verification and activation

Host tests: `cargo test -p cantos-web-auth` (admission, intent/ticket fences, use/renew/re-read
edges, clock floor, status projection, Retry-After, credentialed-target admission, single-flight
renewal). WASM lint: `cargo clippy -p cantos-web-auth --target wasm32-unknown-unknown --all-targets
-- -D warnings`. Ordinary gates are `cargo xtask verify rust`, `cargo xtask verify web`, then
final-diff preflight.

**Not verified:** no browser, real Identity/Server, cross-site cookie, CORS/CSRF header, Studio
roster, renewal-over-time or multi-tab acceptance exists. The former mocked-issuer
`verify web-bearer` fixture served only the removed provider flow and was retired; it is not
replaced by a mock that would pretend the real cookie wire works. Real acceptance needs the
native Server + Identity pair and an installed browser.
