# Cantos Bearer v1 — API foundation for #253

Status: implemented API/SQL foundation and Web/CMP consumer migration. Legacy server issuance
cutover, restricted identity gateway and real issuer/browser/device acceptance remain separate.
The 2026-10-19 [#295 owner decision](https://github.com/loveoverflowcom/cantos/issues/295)
supersedes #253's mandatory RAM-only **Web** target with API-issued HttpOnly/Secure cookies on
the custom domains; migration remains pending. **Native retains this Bearer contract.**
[ADR 0041](../../docs/decisions/0041-direct-hosting-api-boundary.md#owner-selected-web-cookies-native-bearer-retained-295)
owns the selected split. Web routes and phases below describe implemented Bearer behavior, not
the future cookie wire or approval to retire/reuse compatibility handlers without review.

## Authority and wire

Cantos issues opaque random 256-bit secrets, SHA-256 lookup at the API; these are not JWTs.
`ca1_s_`, `ca1_t_`, `ca1_n_` are routing tags for Studio Web, Theatre Web and Theatre native.
Tags confer no authority: exact configured client, issuer, active actor, expiry and permanent
family revocation are checked in PostgreSQL and the provider is introspected on each admission.
Refresh uses a separate `cr1_` random secret. Provider artifacts/secrets stay server-side.
The existing verifier, registry and provider adapter are reused. No client-supplied issuer,
client ID, return URL, actor, role or confidential secret is accepted.

Identity endpoints are **JSON-only** (including errors); this does not extend the business
Protobuf route inventory. [Rust DTOs](api/src/bearer.rs), [response schema](schemas/bearer-v1.schema.json)
and [synthetic fixtures](fixtures/bearer/v1/token-pair.json) own exact shapes. Token responses are
`no-store` with no cookie. Session views contain only account/fence/lifetime facts.

## Routes and two code legs

Prefixes are `/api/v1/studio/auth/bearer`, `/api/v1/theatre/auth/bearer`, and
`/api/v1/theatre/native/auth/bearer`. A prefix selects its fixed configured client. An absent
configuration fails closed; writing this implementation never activates an issuer or grants access.

| Method / suffix | Request / response |
| --- | --- |
| `POST /flows` | `{state, nonce, code_challenge, reauthenticate?: false}` → `{flow_id, authorization_url}` |
| `GET /callback` (Web only) | Provider `state` + authorization `code` → fixed Web `/auth/bearer/complete?flow_id=…&state=…&code=…` |
| `POST /flows/{flow_id}/exchange` | `{state, code, code_verifier}` → `TokenPair` |
| `POST /flows/{flow_id}/cancel` | `{state, code_verifier}` → 204, cancels even a completed/lost-response family |
| `POST /refresh` | `{refresh_token, session_id}` → rotated `TokenPair` |
| `POST /revoke` | `{refresh_token, session_id}` → 204; accepts spent/expired refresh to clean up unknown outcomes |
| `GET /session` | access Bearer → noncredential `Session` |
| `DELETE /session` | access Bearer → 204 after durable local family revoke |

All POST protocol requests omit Authorization and cookies. Session endpoints accept exactly one
`Authorization: Bearer <access_token>` and no Cookie header. Duplicate/ambiguous credentials
are refused. Browser Origin, if supplied, must be the surface's exact configured origin;
native clients need no forged Origin. OPTIONS remains CORS-owned and requires no token.
Business Studio and Theatre `/session` and Library/owner endpoints also admit these access tokens.
General account tokens are never URL parameters, navigation/download links or CDN credentials.

Web uses two PKCE legs. Client state/nonce/verifier are fresh protocol entropy (43–128 unreserved
ASCII; challenge exactly 43 base64url characters, S256). Cantos independently generates its
provider state and provider verifier; state/nonce/exact issuer/audience/callback are verified by
the existing OIDC boundary. Callback consumes the durable flow **before** the one provider
exchange. It stores verified proof server-side and returns only a random, hashed, one-use
handoff, bound to the original client state/challenge/flow/client. Exchange consumes that handoff
under the flow lock. The 300-second flow deadline bounds both code legs; no caller return target.
Provider refusal/cancel consumes and cancels that bound flow and returns only the fixed Web
completion path with `error=authorization_failed`, flow ID and client state; provider descriptions
are never reflected. Optional callback `iss` must match exactly. Malformed/ambiguous callback,
network uncertainty or consumed code requires a new flow. A flow pins its callback and Web return
origin durably, so configuration changes cannot retarget an in-flight authorization. No provider code or handoff is blindly replayed after unknown outcomes.

Native uses the same lifecycle with the existing public native client, exact registered system
browser redirect and client PKCE/nonce/state. Its exchange consumes the durable flow before the
sole provider call. Native `FlowRequest` may carry the closed `callback_profile` (`production`
when omitted, or `development`); it names one operator-admitted exact callback, never a URI, and
the flow durably pins it, so exchange sends the callback bound at begin. Other clients reject
the field; an unadmitted profile is refused without fallback
([runbook](../../docs/deployment/native-callback-profiles.md)). The original native endpoints are legacy server compatibility only; migrated CMP consumers
never fall back. Their final server-side cutover remains an explicit acceptance obligation.

## Lifetime, rotation and unknown outcomes

| Policy | Studio | Theatre Web/native |
| --- | --- | --- |
| Access token | at most 120 s | at most 120 s |
| Refresh idle deadline | at most 300 s since issuance/refresh | at most 600 s |
| Absolute family deadline | at most 600 s from verified callback | at most 900 s |

The server selects these fixed profiles in `platform/bearer/client.rs`, using the registration
IDs owned by `platform/oidc/config.rs`. The immutable `SessionPolicy` in `bearer/core.rs` checks
`0 < access_ttl <= idle_ttl <= absolute_ttl` at construction. Its issue/rotation decisions take
only that policy and time/state facts; credential prefixes and PKCE/state syntax belong to
`bearer/credential.rs`. Requests cannot select or override a policy. This separation changes
neither the wire contract nor admission authority.

Every deadline is additionally capped by the original verified identity proof **and** provider
access expiry. This foundation does **not** use provider refresh tokens, request offline access,
assume Kanidm refresh support or extend original authority. A new authorization is required at
that boundary. Longer sessions require separately qualified upstream renewal semantics.

Each refresh locks its family, checks active actor and current deadline using the database clock,
consumes the hash once, inserts a new access/refresh pair and advances only the idle deadline in
one transaction. Family ID/session fence, actor, absolute deadline and signed authentication time
remain unchanged. The old access token stops admitting new work when rotation commits.
Admission holds a family and actor share lock through dispatch; logout/refresh may linearize
after already admitted work. Provider calls happen outside SQL transactions. Admission/refresh
recheck local facts after provider calls and all lock/trigger waits.

Replay of **any** spent refresh revokes the whole family, including its winner's new access token.
This also contains concurrent refreshes; clients must single-flight refresh per family. There is
no replay receipt containing recoverable plaintext token material. After timeout/network loss or
failed secure-store replacement, abandon local credentials, explicitly revoke with the old
refresh + session ID (or cancel the flow for uncertain exchange), and reauthorize. Retry cleanup
safely; never replay the consumed exchange/refresh, and never replay an unknown business mutation.
Keep its operation ID/receipt separately. A 401 requires reauthorization; 403 is permission/recent
proof refusal; 503 is unavailable/unknown outcome and never evidence that an operation did not commit.

Logout commits permanent local family revoke before best-effort provider revocation. The
`cantos-provider-revocation: pending|completed` response distinguishes remote acceptance;
`revocation_pending` remains durable on provider outage or flow-cancellation cleanup. This slice
adds no background retry worker. Requests after local revoke fail even during a provider outage.
Expired/spent access can still revoke its own family. The refresh cleanup endpoint cannot revoke
another session ID/client. A late A cleanup never targets a B flow/family.

## Fresh authentication and account identity

Only a verified signed issuer `auth_time` is authentication time; missing is `null`. The verifier
refuses future `auth_time` or one later than signed `iat`. Refresh never changes this value.
Bearer Studio delegation mutations, producer-grant issuance, publication approval/publish/withdraw
require an actual authentication time less than 300 seconds old. `reauthenticate: true` asks
`prompt=login&max_age=0`; the deployed issuer must still demonstrate correct signed evidence.
An issuer omitting `auth_time` cannot authorize these operations in this lane. Issuer qualification
is pending; a fresh token/SSO callback/dialog is never substituted for proof.

Registry identity remains exact `(issuer, subject)`. Web/native client-specific issuers or subjects
are **not** auto-linked by email, display name or transport migration. Existing explicitly linked
keys resolve to the same active actor; otherwise they remain separate accounts. Operator-reviewed
proof-based linking/issuer migration is a separate prerequisite to claiming cross-client account
parity. No account merge or new live linking operation is included.

## Migration and client handoff

Phase 1 deliberately keeps legacy cookie login/session and original native routes available for
the unmodified clients. This is a **temporary staged compatibility window**. Phases 2 (Web) and 3 (CMP) have migrated
the target consumers; ending legacy server issuance remains required before #253 acceptance
or production cutover, alongside approved gateway/issuer/native activation. One request never mixes cookie
and Bearer. Do not deploy just this PR and claim the cookie requirement is removed from clients.

Phase 2 implements the new prefixes and cookies-omitted Web effects through the shared
[Web owner](../web-auth/README.md): RAM-only credentials, a single refresh flight, local intent/
rotation plus API-session fences and explicit revoke/cancel recovery. A reserved popup carries
TTL/consume-once protocol state through full-page issuer redirects; the opener exchanges its
one-use handoff and keeps drafts/return context in RAM. Reload/new tab requires an explicit new
authorization; no private draft/token is reconstructed from display state. The Web inventory
covers JSON/Protobuf, XHR artwork, private previews and all three original-source downloads.
Production Web audio/Range consumers are absent, not accepted from the isolated mock player.
The old development-cookie form has no Web fallback; legacy server issuance remains staged
compatibility until the explicit server cutover. Local mock/browser evidence is separate from
real gateway/issuer/Hosting acceptance.

Phase 3 implements native exchange/refresh/cancel/revoke and API-confirmed restore through one
process owner. Its full-pair secure journal is bound to API origin/issuer, persisted and read
back atomically in Keystore-encrypted AtomicFile or Keychain. Flow is written before exchange;
Refreshing with the old family is written before refresh. Interrupted/unknown consuming effects
are cleanup-only after restart. Even failed persistence readback attempts known flow/family
cleanup while retaining the denial barrier. Cleared is durable before removing that barrier.
Unbound old single-token records are discarded locally; they are never disclosed to a changed
origin or restored as Bearer v1, and their original server authority expires within 900 seconds.

All private native requests borrow current authority from the same single-flight coordinator.
Access/session/intent/rotation fences reject late bodies and account A effects after B. Ordinary
same-family rotation preserves playback, account-scoped SQLite and exact outbox operations.
Rejected authority is quarantined synchronously; a separate admission epoch forces a paused
restore even if StateFlow coalesces immediate reconfirmation. The current public media consumer
uses narrow release capabilities, not account tokens in URLs. Native secure-store/process-death,
API/Library and actual Android/iOS decoder/Range acceptance still require named targets.

## Persistence, read cost and gates

[Migration 0002](../../services/cantos-server/schema/0002_bearer.sql) is additive to exact epoch-2
version 1; no baseline rewrite, epoch-1 conversion or live schema operation. The app receives
narrow bearer-table grants plus the registry definer, no actor/registry INSERT grant. Runtime
schema verification refuses the pre-expansion registry; migration checks every known checksum,
then atomically applies the exact missing suffix. Rolling back to a version-1 binary after the
expansion fails schema verification; use a forward fix or operator-reviewed restore, not row deletion.

Read inventory: indexed unique access/refresh hash → one family/actor; callback state hash → one
flow; cancellation → exact flow/family; family index supports retained generations. Frequency/load
is unknown; target is bounded point lookup with existing 5s statement/3s lock deadlines and no
extra provider calls under lock. DBSP: reject for this authoritative security point-lookup workload;
lag/cached authority would violate revoke semantics. No latency improvement claim; PostgreSQL
EXPLAIN and synthetic timings belong to test evidence, not a production SLO.

[Behavior catalog](../../specs/behavior/catalog.json) registers finite draft expectations with
native test mappings in the [inventory](../../specs/behavior/inventory.md). Model and real SQL/API
evidence remain independent. Required live gateway, issuer, browser third-party-cookie refusal,
Web effects, secure storage and native media gates remain open under #191/#179/#180/#253.

Security references: [RFC 9700 §4.14](https://www.rfc-editor.org/rfc/rfc9700.html#section-4.14),
[OpenID Connect authentication time/max_age](https://openid.net/specs/openid-connect-core-1_0.html#AuthRequest).

The restricted default-deny public identity gateway remains **unimplemented**. The existing
edge management denylist is not gateway acceptance. Exact dual legacy/Bearer callbacks are now
implemented in the existing-client reconciliation source and readback guards. Existing old
receipts fail closed until a separately authorized `identity --reconcile-existing` registers
and verifies `/api/v1/{studio|theatre}/auth/bearer/callback` at each existing confidential client.
No account provisioning or live reconciliation occurred. Gateway route/method/asset/issuer
qualification and rollout remain successor work; no provider token/management endpoint may be
exposed to bypass this prerequisite.
