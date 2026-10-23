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

## Structure

| Module | Owns | Runs on |
| --- | --- | --- |
| `core` | pure decisions: admission, intent `Ticket`, use/renew/re-read edges, monotonic `Clock`, `RateGate`, status projection, `credentialed_target` | host + WASM |
| `owner` | the session owner (`Client`, `Lease`) written against a small `Port` (clock, one HTTP attempt, a cross-tab lock, a view event, a cross-tab signal) | host + WASM |
| `serial` | FIFO async mutex that orders this document's cookie-changing exchanges | host + WASM |
| `refresh` | single-flight renewal coordinator | host + WASM |
| `browser` | the browser `Port` (`fetch`, Web Lock, `BroadcastChannel`, window event) and the `client()` registry | WASM only |

Host tests therefore drive the **production owner** through a scripted `Port`; only the browser
adapter itself is untested on the host.

## Implemented client boundary

- The credential is an API-issued `HttpOnly` cookie (`__Secure-cantos_id_{theatre,studio}`,
  `SameSite=Strict`, host-only, `Path=/api/v1/{surface}`). No script reads, stores, forwards or
  logs it, and no body, URL or media URL carries it. This package keeps only **nonsecret session
  facts** in document RAM: actor id, session id (a fence, never a credential), idle and absolute
  deadlines, authentication time.
- Every user-session request is `credentials: include` with redirects refused, and carries
  `x-cantos-{theatre,studio}-session` with the session the client believes it holds. The browser
  adds `Origin` itself; application code never forges it. `Lease::apply` is the single place a
  protected `fetch` receives that policy and `Lease::receive` the single place its answer is
  judged; XHR callers take `Lease::fence`, set `withCredentials` themselves and pass the status,
  `Retry-After` and session echo to `Lease::response`. `core::credentialed_target` is the only way an arbitrary URL
  (artwork upload) may receive the ambient cookie: exact API origin, this surface's path only.
- Public catalog/media, producer capability Bearer and signup requests are **not** user-session
  requests and never use this policy.
- `Client::login(email, password, expected_actor)` is one attempt, never retried. A definite
  refusal (400/401/403/404/429) leaves the previous local state; an unknown outcome (the cookie
  may have been replaced) drops it, so only an authoritative `read_session` can re-admit a
  session. A login answered for another actor than `expected_actor` (Studio re-authentication as
  the same owner) is never adopted and its fresh cookie is revoked. The authorization intent is
  fixed **before the first await** and re-checked after old cleanup, after the cross-tab lock and
  immediately before the POST: a login cancelled while a failed logout's cleanup (or another tab's
  exchange) was pending sends nothing and returns `Changed`. Once the POST left, its effect is
  unknown: a late response of a cancelled/logged-out intent is revoked (exactly the session the API
  reported), never adopted, and an abort is never treated as a rollback.
- `Client::read_session()` (`GET identity/session`, no fence) answers "who am I now?" and is the
  startup/reload path: empty RAM never means signed out; only a 401 does. A different session
  than the local one switches intent and fences every older lease, so a delayed A response can
  never update B.
- `Client::access()` admits one protected request. It uses the session, renews the **idle**
  deadline when `RENEW_MARGIN` seconds remain (a single shared flight; the cookie value is
  unchanged, so an unknown renewal outcome is safe to retry later), and re-reads the session when
  the local idle/absolute deadline has passed instead of concluding signed-out from a clock. The
  absolute deadline cannot be extended: after it the API answers 401 and the user logs in again.
  A renewal reply is validated against the snapshot it was **dispatched** with (same actor,
  session, authentication time and absolute deadline; idle never below that snapshot nor above
  absolute) and then merged into the current family keeping the largest idle the API confirmed. A
  read confirming idle 800 followed by a renewal answering 700 therefore keeps 800 and the
  session; an answer of 500, a foreign family or a stale intent is never published.
- Local time is the wall clock with a monotonic floor (`core::Clock`), so a device clock moving
  backwards never extends a session; deadlines remain the server's.
- 401 → signed out (session/protected calls) or `InvalidCredentials` (login). 409 → another
  tab/intent changed the account: drop the local session and re-read. The API stamps every
  admitted answer (and its fence refusal) with the session the cookie holds; an echo naming
  another session, for any status, drops the local session inside `Lease::response` without a
  sign-out event, so a stale lease cannot keep acting and no caller has to remember the rule. A
  business 409 echoes the caller's own session and keeps its meaning. 429 → `RateLimited(n)` with
  a bounded `Retry-After`. 503/network → `Unavailable`, never signed-out. 403 → `Forbidden`: a
  wrong Origin **or** a Studio login whose password verified but has no roster seat; the response
  does not say which, so no consumer may claim the password was right. 404 → `NotFound`
  (Identity not configured, no Studio roster): a clear error, never a fallback.
- **429 gate.** Every user-session request goes through it: `Lease::receive(&response)` (or,
  for XHR, `Lease::response(status, retry_after, echo)`) must be called with each answer before
  its body is used. A 429 arms one surface-wide hold (a later shorter answer never shortens it), even for a
  stale lease: it is a fact about the API, not the account. While the hold lasts `Client::access`
  (and login, read, renew, cleanup) refuse with `RateLimited(remaining)` before any dispatch,
  including for a session that is still valid, so view refresh, manual retry and durable-journal
  replay in this document all keep the deadline. It is document RAM: another tab or a reload
  starts with an empty gate. The Server's CORS layer exposes `Retry-After` to the configured Web
  origin of its surface, so the hold is the wait the API asked for; an answer without a readable
  header (an intermediary that drops it, an HTTP-date) holds for the conservative 30 s default.
- **Cookie-changing exchanges are ordered.** Login, renew and logout DELETE leave one at a time:
  inside the document through `serial`, across tabs through a Web Lock named for the surface and
  API origin (`navigator.locks`, reached by reflection because `web-sys` marks it unstable). The
  browser applies `Set-Cookie` in dispatch order, so a late `renew(A)` cannot overwrite the cookie
  of `login(B)` and a late `logout(A)` cannot clear it. A browser without Web Locks keeps only the
  in-document ordering. A lock another tab never releases within 15 s is `Unavailable`. A waiter
  dropped before the grant aborts its request; a grant that lands after the waiter was dropped is
  released at once, so a cancelled login or renewal cannot hold the lock for the document's life.
- **Cross-tab signal.** After this document changes the cookie (login adopted, a refused login
  revoked, a logout DELETE acknowledged) it posts on a per-surface `BroadcastChannel`. A receiving
  tab only withdraws its local authority (`Client::external_change`: fence every lease, fire the
  existing invalidation event) so its views re-read `GET identity/session`. The message proves
  nothing about who the cookie names and is never trusted as authentication.
- Logout invalidates the local intent first (`begin_logout`), then `DELETE identity/session`
  fenced to the closed session. 204, 401 and 409 are terminal; any other outcome stays queued in
  RAM and blocks new admission/login until an acknowledged retry. Cleanup is pinned to the
  original session, so a late A cleanup cannot close B. Browser termination can lose the queue;
  the server's idle/absolute deadlines remain the bound.

## Deliberate coupling

These are reviewed choices, not leftovers; revisit one only when its condition changes.

- **Two named surfaces, not a generic scope config.** `Surface` is the contract's `WebSurface`, and
  the fence header, invalidation event, lock and channel names are fixed per surface. Studio and
  Theatre are the only consumers; a configurable scope would turn cookie path, header and lock
  isolation into caller-supplied strings that must then be validated. Revisit with a third real
  Web consumer.
- **Contract DTOs, not a private wire copy.** The package implements Identity auth v1, so it reads
  `Session`/`Login` and `api_url` from `cantos-contracts`; a duplicate DTO could drift from the
  Server silently. Apps project `SessionView` into their own account types.
- **One owner per surface and document.** `browser::client` keeps a document-lifetime owner per
  surface: the serial queue, cross-tab lock, 429 gate, renewal flight and cleanup queue are only
  correct when every request of that surface shares them. Applications never construct a second
  browser owner; host tests construct `owner::Client` with a scripted `Port`.
- **Consumers keep their resource rules.** Business codecs, receipts, required-echo confirmation of
  writes and user-visible errors stay in the apps; this package owns only session admission,
  credential policy and the answer checks every protected request shares.

## Signup is not here

Theatre signup (`identity/signup`, the mailed `#operation=…&proof=…&secret=…` fragment and
`identity/signup/complete`) belongs to the Theatre app because it authenticates nobody: completion
creates an account and **no session**. See the [Theatre Web README](../../apps/cantos-theatre/web/README.md).

## Protected resource inventory

| Consumer | Transport / boundary |
| --- | --- |
| Theatre session, Continue, Saved, History, progress, bookmarks, operation receipts | `Lease::apply`, expected session fence, `Lease::receive`, body/intent checks, JSON only |
| Studio revision head/history/save/validation | `Lease::apply` plus the UUID fence echo and `Lease::receive`; existing per-route JSON/Protobuf codec and ambiguous-write receipts retained |
| Studio catalog/content/brief/audio metadata/delegations | owner session; producer status/upload keeps its distinct work-capability Bearer with cookies omitted |
| Artwork upload | XHR `withCredentials=true` + fence header to a `credentialed_target`, `Lease::response` with the XHR's `Retry-After` and echo; fetch fallback via `Lease::apply` + `Lease::receive` |
| Private artwork preview, original-source download | authenticated fetch via `Lease::apply` + `Lease::receive`, exact identity/hash/format checks, owned revocable object URL / ephemeral Blob |
| Public catalog, public media, signup | credentials omitted; never cookie-bearing |

## Verification and activation

Host tests: `cargo test -p cantos-web-auth`.

- `core`: admission, intent/ticket fences, use/renew/re-read edges, clock floor, status
  projection, Retry-After, `credentialed_target`, renewal against the dispatch snapshot
  (600 → read 800 → reply 700 keeps 800; 500 and a foreign family refused), `RateGate`.
- `owner` (the production owner through a scripted `Port`): cancel while a failed logout's
  cleanup is retried, while another tab holds the cookie lock and after the POST left (zero /
  zero / revoke-the-late-session); definite refusal versus unknown effect; an old read, renewal or
  401 arriving after a new intent changes nothing; the 429 hold across `access`, `read_session`
  and a stale lease; an echo of another session dropping the local session for any status; one cookie-changing exchange at a time in dispatch order; the cross-tab
  signal; malformed or oversized session bodies. Each guard was mutation-checked once (removing
  it fails the named test).
- `serial`, `refresh`: FIFO and cancelled-waiter behavior; single-flight renewal.

WASM lint: `cargo clippy -p cantos-web-auth --target wasm32-unknown-unknown --all-targets --
-D warnings`. Ordinary gates are `cargo xtask verify rust`, `cargo xtask verify web`, then
final-diff preflight.

**Not verified** (`NOT_RUN`): `browser.rs` is compiled and linted for `wasm32` but has no
browser execution: the Web Lock (including the grant-after-drop release), `BroadcastChannel`,
`fetch` deadline, `Lease::receive` header reads and view event are untested in a real browser. No browser, real Identity/Server, cross-site cookie, CORS/CSRF
header, Studio roster, renewal-over-time or **multi-tab cookie-jar** acceptance exists, so the
ordering above is design plus host-tested sequencing, not observed browser behavior. The former
mocked-issuer `verify web-bearer` fixture served only the removed provider flow and was retired;
it is not replaced by a mock that would pretend the real cookie wire works. Real acceptance needs
the native Server + Identity pair and an installed browser (`wasm-bindgen-test` and a webdriver
are not part of this repository).
