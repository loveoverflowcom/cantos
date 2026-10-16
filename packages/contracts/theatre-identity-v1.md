# Theatre Identity v1

PR3 implements separate opt-in Web BFF and native public-client identity on the development host.
Native exchange/resource admission and OS adapter mounting are code; provider/device acceptance
and production activation remain separate gates.
See the [architecture](../../docs/architecture/theatre-identity.md) for the trust boundary.

| Route | Admission / response |
| --- | --- |
| GET `/api/v1/theatre/auth/configuration` | Public read-only capability, always mounted; no account/session/provider I/O or credential fields |
| GET `/api/v1/theatre/auth/login` | Same-origin navigation; redirects to exact Theatre Web issuer, `openid`, code + S256 |
| GET `/api/v1/theatre/auth/callback` | Exact browser/state/nonce/redirect; consumed before one exchange; opaque cookie only |
| GET `/api/v1/theatre/session` | Theatre cookie; `{actor_id, session_id}` as nonsecret display/fencing facts |
| DELETE `/api/v1/theatre/session` | Exact Origin; works for expired sessions; local revoke/cancel before remote revoke; guarded UI 204 without shared cookie expiry |
| Library / Progress / operation receipts | Existing [Library contract](listener-library-v1.md), sealed Listener principal; Web cookie admission |

Session DTO is [`cantos_contracts::theatre_identity::Session`](api/src/theatre_identity.rs), with
required strings and no unknown fields. `actor_id` is an opaque internal actor; `session_id` is a
random UUID unrelated to the credential. Neither is proof. No token, provider claim or refresh
credential appears in the response. Auth-route failures use the existing `ApiError`; Library
admission failures keep the Library error contract. All responses use no-store/no-referrer.

Provider credentials remain server-side. Native resources use a separate opaque native bearer,
never a Web cookie. Cookie/bearer mixtures are refused. Web `/session` accepts only its Web cookie;
`/native/session` accepts only the native bearer.
Absent Theatre configuration retains default-deny account admission and existing guest routes.
No automatic guest import/merge. Login/logout never emits history, moves playback or imports
mock state. Client intent IDs/bodies are stable; A's pending intents cannot be submitted asB.

## Registration policy and implemented capability

[ADR0032](../../docs/decisions/0032-theatre-signup-and-provisioned-studio.md) accepts
public self-registration for Theatre and separately provisioned Studio administrators. Theatre
registration never creates Studio authority or changes a Studio admission list. Product approval
does not establish provider support or authorize live account/provider configuration changes.

The current Web capability DTO is `theatre_identity::Configuration`, with the strict
[configuration schema](schemas/theatre-auth-configuration-v1.schema.json) and literal
[unconfigured](fixtures/theatre-identity/v1/configuration-unconfigured.json) /
[OIDC](fixtures/theatre-identity/v1/configuration-oidc.json) fixtures:

```json
{"login":"oidc","registration":{"status":"unavailable","reason":"provider_unsupported"}}
```

`login` is `unconfigured` without a Theatre Web BFF adapter, including native-only configuration;
that mount returns registration reason `identity_unconfigured`. With the current Theatre Web
adapter, login is `oidc` and registration remains `provider_unsupported`. This describes the
mounted implementation, never live issuer reachability or a successful authentication. Both
responses are public, `no-store` and `no-referrer`; neither reads cookies, returns provider/client
secrets, sets a session cookie or allocates a flow. Only GET/HEAD is accepted. Other surfaces and
the deprecated Studio alias do not inherit this route.

The pinned Kanidm1.10.5 adapter has no accepted public self-registration operation. Its
[UI router](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/views/mod.rs)
does not expose signup; its
[`/ui/enrol`](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/views/enrol.rs)
requires an authenticated user and creates a credential-update intent for that existing account.
The [person-creation API](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/v1.rs)
is an authenticated administrative operation, not an anonymous registration contract. This source
inspection establishes no supported entrypoint for the current adapter; no deployed issuer was
probed and no provider account/grant/security setting was changed.

There is consequently no `/auth/register`, `/auth/signup`, password-create handler, arbitrary
registration URL setting or supported-capability variant. OIDC discovery/login, a credential
enrollment page and a client-supplied URL cannot enable registration. Web must show the precise
unavailable reason and allow metadata retry/Back/continued guest discovery without submitting
credentials. Transport/malformed/unknown metadata is a separate client failure, not a supported
registration state. No provider outcomes such as email-verification-pending or registration
success are claimed or synthesized.

Actual registration remains blocked on an approved, capability-backed issuer enrollment
integration: exact provider/version/configuration, accepted route/method and return correlation,
verification/abuse/recovery policy and isolated Theatre-only authority. Any provider replacement,
privileged provisioning bridge or live security change needs its own explicit decision/action
authorization. A future implementation must verify callback/session using the existing OIDC
authority before showing account success; it must preserve cancellation/replay/account fences
and must not merge guest data implicitly. Local fixtures do not satisfy that live acceptance.

Native client is fixed `cantos-theatre-native`, scope `openid`, callback
`com.cantos.theatre:/oauth2/callback`. System-browser adapters parse exact URI, reject fragments,
unknown/duplicate parameters and correlate state before the single code exchange effect. The
nonce/verifier stay in the pending in-memory attempt. Cancellation/process restart needs a new
login. A code callback is not an authenticated account.

| Native route | Contract |
| --- | --- |
| POST `/api/v1/theatre/native/auth/flows` | `{state,nonce,code_challenge}` → `{flow_id,authorization_url}`; fixed public client, no credentials/cookies |
| POST `/api/v1/theatre/native/auth/flows/{flow_id}/exchange` | `{state,code,code_verifier}`; durable one-attempt consume before one public exchange; `{actor_id,session_id,access_token,expires_at}` |
| GET `/api/v1/theatre/native/session` | Native bearer → `{actor_id,session_id,expires_at}` after resource signature, current introspection and local admission |
| DELETE `/api/v1/theatre/native/session` | Native bearer; local revoke commits first; 204 with completed/pending remote revocation; identical-token retries are safe |

`access_token` in the exchange response is a random 64-character lowercase-hex **Cantos native
session credential**, independent of provider access tokens and Web cookies. Provider tokens
stay private in PostgreSQL. Native clients persist only this credential in OS secure storage.
All native routes reject Cookie headers, including an empty header. Auth JSON bodies are bounded
at 16KiB and provider responses at 64KiB. [Native schema](schemas/theatre-native-identity-v1.schema.json)
and the shared Rust DTOs own this wire contract.

Private Library/Progress/operation responses carry `X-Cantos-Theatre-Session`, the admitted
nonsecret session UUID. An optional request header with that name declares which session owns
the intent; mismatch returns 409 before resource dispatch and stamps the actual session. The
header grants no authority. Clients reject mismatched/missing response fences, hide old account
projections and revalidate identity; they do not log out a switched account on 409.

Web DELETE accepts the expected-session header too. Automatic 401 cookie cleanup additionally
sends `X-Cantos-Theatre-Stale-Only: 1`; a now-valid cookie returns 409 with the actual UUID and no
revocation or cookie clearing. Unknown provider status fails closed with 503 and no local revoke.
Guarded/stale-only 204 responses emit no `Set-Cookie`: a delayed response must not erase a newer
account cookie or flow created by another tab. Local revocation makes the retained HttpOnly
cookie inert until replacement or expiry. Login treats only a positively known local revoked or
expired credential as anonymous; unknown credentials and live provider outages remain closed.
Web flows use distinct immutable HttpOnly cookie names derived from their exact random state,
with a separate random binder value. The callback accepts only its own state/binder pair; mutable
legacy binder cookies are refused. Login admits fewer than eight inventory cookies; malformed,
duplicate state/binder, mixed Secure names and overflow fail closed without truncation or flow
allocation. Concurrent anonymous starts can exceed the cap before any responses are observed;
the browser must wait for the 300-second binder expiry before retrying if this happens.
Active expected-A logout locks the validated inventory and A's own source flow, then sessions
in deterministic order. It cancels pending flows and revokes already-issued sibling sessions
before remote effects; callbacks linearizing later cannot issue. A stale-only or already locally
revoked/expired A cleanup cannot cancel newly intended anonymous B flows. An expected-A cleanup
may still cancel a rotation receipt only with its captured exact private binder, unexpired after
all lock/write waits, and a matching immutable lineage root/source session association. The root
serializes A→B→C rotations before flow locks; it grants no cancellation authority by itself.
Old A plus UUID alone, or an expired/missing B binder, preserves active observed B/C. Rotations
bound by SQL `previous_hash` cannot finish after the previous session is revoked. Session association remains local to the
proven browser inventory; this is not an actor-wide or cross-device logout policy.
Captured revocations carry durable attribution to the triggering session. The remote batch has
an overall five-second deadline; outage/cancellation leaves acknowledged local revocation with
pending evidence. A retry using A's original credential can finish only its already-revoked
pending siblings even after old binder cookies expire, and cannot cancel fresh B. No background
recovery worker or global provider logout is claimed.
Callbacks issue the new session cookie but never expire inventory cookies: SQL consumption/
completion enforces one-time use. DELETE without an expected-session or stale-only guard is
refused without effects; current Web clients always use a guard.
Native logout uses its own bearer and never clears Web cookies.

No automatic guest import or credential restore occurs. A nonsecret durable native logout marker
precedes cleanup; uncertain revocation retries retain the same credential. New login waits for
OS deletion/marker acknowledgement. Provider/device acceptance, native runtime, secure-store
crash behavior and production activation remain NOT_RUN until actually exercised.
