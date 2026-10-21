# Identity auth v1 — email/password sessions (#335 backend slice)

Status: **implemented in the Server on PostgreSQL**, verified against the real `cantos-identity`
executable and a disposable PostgreSQL schema (see [Verification](#verification)). No frontend,
browser, device, live-SMTP or production acceptance exists. Rust DTOs:
[`cantos_contracts::identity_auth`](api/src/identity_auth.rs) plus the existing
[`bearer::{Session, TokenPair, RefreshRequest}`](api/src/bearer.rs); JSON Schema:
[identity-auth-v1.schema.json](schemas/identity-auth-v1.schema.json). Parent:
[#334](https://github.com/loveoverflowcom/cantos/issues/334)/[#335](https://github.com/loveoverflowcom/cantos/issues/335),
coordinated with #333 (Turso Server) and #295 (Web cookies).

## Owner decision recorded here (v1 scope amendment)

| Decision | Effect |
| --- | --- |
| Email + password with mailbox verification | The only sign-in method in v1, for Theatre **and** Studio. |
| Studio uses a password, with a Server roster | `Purpose::Studio` + `Assurance::Password` is accepted by Identity. The Server separately requires the exact Identity subject to be on the Studio roster. Login never grants Studio, Admin, producer or membership authority. |
| Deferred, **not done** | Passkey/WebAuthn, MFA and recovery codes (D3), credential recovery, change-password, phone, Google login, Studio operator provisioning, security-event consumer, retirement of the provider (Kanidm/Firebase) endpoints, the `cargo xtask run` two-process lane, browser/CMP/device acceptance, Kani on the D1 runner. The checklist items of #335 that these cover stay open. |
| No new session engine | Lifetimes, rotation and replay are the existing [`SessionPolicy` and rotation core](../../services/cantos-server/src/platform/bearer/core.rs); TTLs and the opaque-secret model are unchanged. |

## Who owns what

```
browser/app ──JSON──► Server (this contract)                Identity (private, no HTTP)
                       │  email/password never stored        │  password, mailbox proof, security state
                       │  session secrets (hashed)           │  identity.db (Turso), one owner
                       └── private gRPC over a Unix socket ──┘  peer uid checked, deadlines, no retry
```

Identity proves a mailbox and a password and returns an **authentication fact**; the Server turns
a *fresh* fact into a session bound to `ActorId`. The Server never reads `identity.db`; there is no
dual write, fallback database or auth-only database.

## Endpoints

All bodies are JSON (`Content-Type: application/json`, ≤ 4 KiB, unknown fields refused). Every
response is `no-store`/`no-referrer`. `{S}` is `theatre` or `studio`.

### Theatre Web (cookie) — prefix `/api/v1/theatre`

| Method and path | Request → response |
| --- | --- |
| `POST /identity/signup` | `{email}` → `202 {accepted:true, proof_ttl_seconds:900}`; identical for any address, even one already registered |
| `POST /identity/signup/complete` | `{operation_id, proof_id, proof_secret, password, explicit_confirmation:true}` → `200 {status:"completed"\|"replayed"}`; **no session, no cookie** |
| `POST /identity/login` | `{email, password}` → `200 Session` + `Set-Cookie` |
| `GET /identity/session` | cookie → `200 Session` |
| `POST /identity/session/renew` | cookie → `200 Session` + refreshed `Set-Cookie` (same value, new `Max-Age`) |
| `DELETE /identity/session` | cookie → `204` + clearing `Set-Cookie`; local and durable |

### Studio Web (cookie) — prefix `/api/v1/studio`

`POST /identity/login`, `GET /identity/session`, `POST /identity/session/renew`,
`DELETE /identity/session`: same shapes as Theatre. There is **no Studio signup**. With no roster
configured the Studio routes answer `404`.

### Theatre native (opaque Bearer) — prefix `/api/v1/theatre/native/identity`

| Method and path | Request → response |
| --- | --- |
| `POST /signup`, `POST /signup/complete` | same bodies/answers as Web |
| `POST /login` | `{email, password}` → `200 TokenPair` (`token_type:"Bearer"`, `access_token ci1_n_…`, `refresh_token ci1_r_…`, `actor_id`, `session_id`, `expires_at`, `refresh_expires_at`, `absolute_expires_at`, `authenticated_at`) |
| `POST /refresh` | `{refresh_token, session_id}` → `200` rotated `TokenPair` |
| `POST /revoke` | `{refresh_token, session_id}` → `204`; accepts a spent/expired refresh secret; idempotent |
| `GET /session` / `DELETE /session` | `Authorization: Bearer` → `200 Session` / `204` |

`Session` is `{actor_id, session_id, expires_at, absolute_expires_at, authenticated_at}` (UNIX
seconds). `session_id` is a random UUID, a fence input, never a credential. For Web `expires_at` is
the idle deadline of the cookie; for native it is the access deadline.

### Protected APIs

Existing routes accept the new credentials with no change of shape: Theatre `GET/PUT …/library/*`,
`…/progress/*`, `…/operations/*`, legacy `GET …/session`, and every Studio route behind the Studio
cookie. Replies carry `x-cantos-theatre-session` / `x-cantos-studio-session`; a request that sends
that header with another session's id gets `409` (see [Account switch](#account-switch-and-late-responses)).

## Credentials

| Channel | Carrier | Secret | Lifetime (existing policy) |
| --- | --- | --- | --- |
| Theatre Web | `__Secure-cantos_id_theatre` cookie (`cantos_id_theatre` on an `http` development origin) | `ci1_t_` + 64 hex | access 120 s · idle 600 s · absolute 900 s; cookie lives to the idle deadline |
| Studio Web | `__Secure-cantos_id_studio` (`cantos_id_studio`) | `ci1_s_` + 64 hex | access 120 s · idle 300 s · absolute 600 s |
| Theatre native | `Authorization: Bearer` + refresh body | `ci1_n_` / `ci1_r_` + 64 hex | access 120 s · idle 600 s · absolute 900 s |

* Cookies are `HttpOnly; SameSite=Strict; Path=/api/v1/{S}; Max-Age=…` and `Secure` on an `https`
  origin, **host-only** (no `Domain`), one name per surface. No script ever sees the secret; the
  body of a login reply carries none.
* Web renewal extends the idle deadline of the **same** cookie value and never passes the absolute
  deadline, so two tabs cannot revoke each other by racing a rotation. Native renewal rotates:
  replay of any spent refresh secret revokes the whole family, including the winner.
* Secrets are random 256-bit values; only SHA-256 digests are stored.
* The `ci1_` family is disjoint from the provider-bound `ca1_`/`cr1_` credentials: a legacy
  verifier cannot be handed one and the reverse. A value of the wrong surface or kind, a duplicate
  cookie, the other spelling (`Secure`/plain) of the cookie name, or cookie **and** `Authorization`
  together is refused with `401`; the server never falls back from one to the other.

## CSRF, Origin and route admission

| Request | Rule |
| --- | --- |
| Web mutation (`POST`/`DELETE`, including login, signup, renew, logout) | exactly one `Origin` equal to the configured Web origin; `Sec-Fetch-Site: cross-site` refused; JSON-only bodies. A cookie request **without** `Origin` is refused even on routes that exempt Bearer writes. `403` |
| Web read | `Origin`, if present, must match; no `Authorization` header |
| Native | **no** `Origin`, `Cookie` or `Sec-Fetch-*` header at all (they mark a browser); anonymous routes carry no `Authorization`; session routes exactly one Bearer of the native kind. `401` |

CORS stays exact-origin with credentials (existing `with_browser_cors`); native clients need no
forged `Origin`.

## Email link and signup

The mail carries `<CANTOS_IDENTITY_SIGNUP_LINK_BASE>#operation=<32 hex>&proof=<32 hex>&secret=<64 hex>`.
The secret is in the fragment, so a `GET`, prefetch or link scanner never sends it to a server. The
front-end page reads the fragment and, only on the owner's explicit action, `POST`s
`signup/complete` with the password **the mailbox owner chose**. The initiator of a signup cannot
set a password, and polling or a receipt never grants a login.

* A lost completion response is retried as the **identical** request: the Server derives the
  command id from `(operation_id, proof_id)`, Identity answers `replayed` only if the password also
  matches, and the account→`ActorId` binding is re-applied idempotently. Another password or proof
  is refused (`401`). `503` on completion means *retry the identical request*, never "nothing
  happened".
* Completion creates an account and **no session**: the user then logs in with the password.
* Existing address: the start answers identically and sends no mail; nothing is reset or linked.

## Errors

Failures are generic: the body is the existing `ApiError` (`unauthenticated`, `forbidden`,
`invalid_request`, `not_found`, `unavailable`). No response says which of password, account, proof,
roster or security state failed.

| Status | Meaning |
| --- | --- |
| `400` | malformed/unknown-field JSON, bad hex, password length policy, missing confirmation |
| `401` | wrong or unknown credentials, expired/revoked/spent/mixed/wrong-kind credential, or a security change observed at admission |
| `403` | wrong or missing `Origin`; **Studio roster denial after the password verified** (the one place a distinct answer is given: the caller already proved the credential) |
| `404` | Identity not configured, or Studio login with no roster |
| `409` | session fence mismatch; `x-cantos-{theatre,studio}-session` carries the current session id |
| `429` | Identity budget spent; `Retry-After` seconds (code `unavailable`) |
| `503` | Identity unreachable, slow, busy or answered outside the contract; unknown effect. Protected requests fail closed. |

## Studio roster

`CANTOS_IDENTITY_STUDIO_SUBJECTS` is a JSON array (1–8) of exact Identity subject ids (32 lowercase
hex). It is the Server's own operator list: never a role claimed by a client, Identity or the
password. Login and **every admission** require membership, so removing a subject closes Studio at
its next request. Membership grants nothing inside Studio: script, catalog, producer and delegation
permissions keep their own existing checks. The subject → `ActorId` binding is the same exact
`(issuer, subject)` registry binding, so one person has one actor on Web, native and Studio.

## Freshness and revocation (quantified)

| Question | Contract |
| --- | --- |
| Fact age at login | an authentication fact is used only if `0 ≤ now − authenticated_at ≤ 30 s` (5 s tolerated ahead); otherwise `503` |
| Identity re-read | `GetSecurityState` at **every** login, renewal and protected admission; no cache. Deadline 3 s |
| Admission rule | admit only if the subject is `Active`, `instance_epoch` equals the session's and `security_version` equals the session's. Any difference (disabled, advanced, **lower** — a restored older file —, other epoch/subject) revokes the family **durably**; it is never resurrected when a later read matches again |
| Maximum revocation latency | a change committed in Identity is seen by the next admission. A request already admitted keeps its lock until it ends, bounded by the host's 15 s request deadline. A change that lands while a request is running is therefore effective for the *next* request |
| Identity unavailable | protected requests, login, signup and renewal fail closed (`503`) **without** revoking: an outage is not a security change. Logout and revocation are local writes and still work |
| Login | exactly one attempt, never retried transparently; deadline 12 s; a deadline is an *unknown effect*, not a rollback |
| No DB lock across RPC | Identity is consulted before and after, never inside a transaction |

Peer checks: the socket's peer uid must equal `CANTOS_IDENTITY_PEER_UID` on every connection;
messages ≤ 8 KiB; at most 32 calls in flight (the 33rd fails closed). Statuses and logs never carry
request content.

### Account switch and late responses

A session id is nonsecret. Clients send `x-cantos-theatre-session` (Studio:
`x-cantos-studio-session`) with the session they believe they hold; a mismatch is `409` with the
current id and **no effect** (this also covers logout and renew). A new Web login with the previous
cookie present replaces the cookie and retires the previous family, so a late request of the old
account cannot be dispatched as the new one.

## Configuration

| Variable | Meaning |
| --- | --- |
| `CANTOS_IDENTITY_SOCKET` | absolute socket path (≤ 100 bytes). Presence enables the feature; every other value below is then mandatory except the roster |
| `CANTOS_IDENTITY_PEER_UID` | uid the Identity process must run as |
| `CANTOS_IDENTITY_PEER_POLICY` | `direct_peer`, or `trusted_proxy` with `CANTOS_IDENTITY_TRUSTED_PROXIES` (CIDR list, no `/0`). The client address Identity budgets on is the socket peer, or the single hop a trusted proxy appended |
| `CANTOS_IDENTITY_STUDIO_SUBJECTS` | optional roster, above |

An orphan variable without the socket is a startup error. `Secure` cookies follow the scheme of the
configured Web origin.

## Storage invariants and #333 hand-off

Migration [`0004_identity_sessions.sql`](../../services/cantos-server/schema/0004_identity_sessions.sql)
(additive; registry `[1,2,3]` → `[1,2,3,4]`, prefix lane required in production) adds
`identity_sessions`, `identity_session_tokens` and `identity_session_register()`. They are **server
business data**, classified `server` in the
[ownership manifest](../../services/cantos-server/schema/native-table-ownership.json), and must
**not** be dropped with the provider-auth tables.

1. `issuer` is the constant `cantos-identity`; `subject` is 32 hex; FK to
   `actor_external_identities(issuer,subject)`; **no** `provider_access`, flow, PKCE or Kanidm column.
2. A session stores `observed_security_version`, `observed_instance_epoch`, `assurance='password'`,
   `authenticated_at`, absolute/idle deadlines, `channel ∈ {theatre-web, studio-web, theatre-native}`.
3. Revocation is permanent (`revoked ⇔ revoked_reason`); consumption of a refresh secret is
   permanent; bindings and the absolute deadline are immutable; the idle deadline never decreases.
4. Only digests of secrets are stored; no secret appears in any log or reply except at issue.
5. Actor binding is idempotent per exact subject, serialized with the operator registry lock;
   email is never an identity key.
6. Conformance for #333 is the PostgreSQL suites `identity_password_postgres`,
   `identity_password_facts_postgres` and `schema_postgres`; a Turso port must keep each assertion.

## Verification

See the PR description for the exact commands and the NOT_RUN list. In short: pure decisions,
credential syntax, DTOs and gates are unit/example-tested; the journeys are
`integration-tested` against the real Identity executable, real Turso file, real Argon2id, a
run-owned SMTP sink and a disposable PostgreSQL database; the fact-boundary suite uses a scripted
Identity and is `mock-integration-tested` only.
