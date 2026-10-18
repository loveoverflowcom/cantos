# Theatre Identity v1

> **#253 Bearer amendment (2026-10-18):** the target API credential is a Cantos-issued opaque
> Bearer with rotating refresh. The [Bearer v1 contract](bearer-v1.md) owns new routes/lifecycle,
> JSON-only identity wire, fresh-auth evidence, RAM/F5 and compatibility cutover. Cookie-only
> statements below describe the retained **legacy compatibility** flow until the Web migration;
> they no longer impose a same-site domain or cookie prerequisite on the target architecture.
> Client migration and real gateway/issuer/browser/native acceptance remain separate.

PR3 implements separate opt-in Web BFF and native public-client identity on the development host.
Native exchange/resource admission and OS adapter mounting are code; provider/device acceptance
and production activation remain separate gates.
See the [architecture](../../docs/architecture/theatre-identity.md) for the trust boundary.

| Route | Admission / response |
| --- | --- |
| GET `/api/v1/theatre/auth/configuration` | Public read-only capability, always mounted; no account/session/provider I/O or credential fields |
| GET `/api/v1/theatre/auth/login` | Same-origin navigation; redirects to exact Theatre Web issuer, `openid`, code + S256 |
| GET `/api/v1/theatre/auth/callback` | Exact browser/state/nonce/redirect; consumed before one exchange; opaque cookie only |
| POST `/api/v1/theatre/auth/registration` | Exact Origin; email-only request; generic 202 acknowledgement, no session or provider person before proof |
| POST `/api/v1/theatre/auth/registration/verify` | Exact Origin; explicit token-only confirmation; 200 `setup_link_sent` or `unavailable`, never account/session completion |
| POST `/api/v1/theatre/auth/registration/firebase/confirm` | Exact Origin; explicit `{operation_id,oob_code}` action-code confirmation; one-shot `setup_ready` private delivery or `unavailable`, never a session |
| POST `/api/v1/theatre/auth/registration/cancel` | Exact Origin; token-only cancellation of an unconsumed proof; generic 202 acknowledgement |
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

[ADR0034](../../docs/decisions/0034-theatre-signup-and-provisioned-studio.md) accepts
public self-registration for Theatre and separately provisioned Studio administrators. Theatre
registration never creates Studio authority or changes a Studio admission list. Product approval
does not establish provider support or authorize live account/provider configuration changes.

The Web capability DTO is `theatre_identity::Configuration`, with the strict
[configuration schema](schemas/theatre-auth-configuration-v1.schema.json). Login and signup
availability are separate values. A configured registration module reports:

```json
{"login":"oidc","registration":{"status":"email_verification"}}
```

`login` is `unconfigured` without a Theatre Web BFF adapter, including native-only configuration;
registration is then `unavailable` with `identity_unconfigured`. A missing registration module
reports `registration_unconfigured`; `provider_unsupported` remains an explicit unavailable
reason and does not imply that Kanidm lacks onboarding APIs. `email_verification` means the
reviewed module/adapters have been mounted from explicit configuration, not that the issuer or
mail transport was probed or a real account created. Metadata responses are public, `no-store`
and `no-referrer`; they read no cookie, return no provider/client secret, set no session cookie
and allocate no registration flow. Only GET/HEAD is accepted. Other surfaces and the deprecated
Studio alias do not inherit this route. Malformed/unknown metadata remains a client failure.

Kanidm1.10.5 lacks an integrated public signup UI but supports authenticated
[person creation and credential-update intents](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/v1.rs).
Upstream documents [credential links for onboarding](https://github.com/kanidm/kanidm/blob/v1.10.5/book/src/accounts/authentication_and_credentials.md).
The approved Theatre module supplies mailbox proof and durable new-person ownership around
those privileged operations. There is no password-create handler, arbitrary enrollment URL,
client-selected issuer/person ID/role or automatic account linking.

### Email registration facts and receipts

The strict [`theatre_registration` DTOs](api/src/theatre_registration.rs) and
[registration schema](schemas/theatre-registration-v1.schema.json) accept an email-only request
or a token-only confirmation/cancellation request. Unknown fields, password, person ID, issuer,
recipient override and caller role are refused. A receipt contains only a closed status; no
email, raw proof, provider setup token/URL, account ID or session credential is returned.

These three POST routes accept JSON up to 2KiB and require the exact configured Web `Origin`;
`Sec-Fetch-Site: cross-site` is refused. They do not use a Studio/Theatre session as onboarding
authority. Missing module returns 404; malformed/unknown-field JSON or invalid email returns
400, wrong Origin returns 403, and infrastructure/unavailable execution returns 503. Every route
requires the real socket peer for its HMAC quota partition; missing peer returns 503. IPv4-mapped
IPv6 is normalized, and forwarded headers never select the quota peer. All responses inherit
the Theatre `no-store`/`no-referrer` boundary. GET/HEAD cannot verify, cancel or create a
registration. Responses never set an authentication cookie.

All three routes use bounded process-local rolling global/peer attempt admission and an
in-flight semaphore. Saturated start requests retain the generic 202 acknowledgement without
dispatch; verify/cancel saturation returns 503 without consuming proof. These counters reset
on process restart and are not shared by multiple hosts. Durable PostgreSQL accepted-mail
budgets and recipient cooldown independently gate new verification sends; those receipts
survive restart. This is not a claim of deployment-wide rate limiting, trusted proxy support or
measured enumeration resistance.

- `{"status":"accepted"}` acknowledges an eligible public request without revealing whether
  a duplicate/cooldown/quota decision suppressed mail. It does not promise a sent or delivered
  message, verified mailbox or provider account.
- `{"status":"setup_link_sent"}` means the workflow positively acknowledged a new person,
  requested an intent for that exact person and received SMTP acceptance for the setup email.
  It proves neither inbox delivery nor completed credentials or login.
- `{"status":"unavailable"}` refuses unusable proof or stopped/uncertain onboarding. It is
  never an instruction to reset an existing account or blindly replay create/intent/send.

The application sends a verification link at the pinned Web origin with the proof in
`#register-verify?token=…`. Opening it presents confirmation; it must not automatically perform
an effectful request. Explicit confirmation posts the proof in the body, preventing ordinary
link scanners/prefetch from provisioning. The Web owner removes the fragment, keeps raw proof
only in current memory and fences delayed completion after cancel/history/logout. No proof
belongs in persistent browser storage, referrer, log or shared cache. The server retains only a
purpose-separated SHA256 hash and atomically claims an unexpired token once. Resend rotates the
challenge only before provisioning; a cancelled, expired, replaced or consumed proof cannot
create a second person. Reload requiring a new proof must say so rather than fabricate success.

The durable operation pins the exact issuer namespace, verified email, generated UUID/name and
new-person acknowledgement. No existing-account search by email/name grants intent authority.
Conflict, timeout/lost response, malformed acknowledgement, receipt failure and uncertain mail
stop automatic replay and retain a blocked/stalled operation for reconciliation. Cancellation
invalidates an unconsumed proof only; it cannot reset/revoke an account/session or recall an
already dispatched provider operation. Setup secrets are not persisted for automatic retry.

Current candidate policy uses bounded ASCII addresses, case-insensitive local deduplication,
operation-derived provider names and explicit bounded lifetime/cooldown/global/network settings.
These, external-existing-email handling, retention/cleanup, proxy/network partition policy and
manual recovery need review before activation. The built-in Kanidm onboarding group can reset
existing people: live acceptance must verify effective issuer target protection separately from
the application's new-person guard. No live principal, credential, grant, SMTP setup or account
is authorized by the code/test approval. See the
[operator runbook](../../services/kanidm/README.md#theatre-registration-activation-runbook--approval-required-before-execution).
The host defaults off and requires every explicit policy, adapter and secret input before
mounting this capability. `POLICY_APPROVED` and `ISSUER_ACCESS_APPROVED` are operator declarations
under the `CANTOS_THEATRE_REGISTRATION_` prefix; they do not inspect actual grants or replace
the live acceptance record. Partial/orphan configuration fails startup, rather than silently
enabling a reduced workflow.

After setup, listeners use the existing OIDC flow and server-confirmed session. A registration
receipt, redirect or verified mailbox cannot authenticate the account, admit Studio/Admin,
grant Membership/producer authority or merge guest data. Isolated fixtures, SMTP peer tests and
Web browser doubles remain separate from real issuer/inbox/session and native acceptance.

### Firebase verification and browser setup delivery

The Firebase mode implements the separately approved mailbox-proof amendment for [#206](https://github.com/loveoverflowcom/cantos/issues/206).
It remains opt-in and disabled without its full server configuration; implementation and isolated
fixtures do not establish live provider, inbox, ingress, issuer-target or completed-login acceptance.
Kanidm remains the credential and OIDC issuer. The capability is additive:

```json
{"login":"oidc","registration":{"status":"firebase_email_verification"}}
```

The email-only start route retains its generic 202 response. It returns no operation identifier,
Firebase UID, account name, proof, setup link or session. Each server enrollment owns a dedicated
Firebase UID. The email action uses a fixed server-selected Theatre action origin; its
`continueUrl` is exactly that origin's `/?firebaseEnrollment=<operation UUID>`. The browser
accepts only the standard `mode=verifyEmail`, `oobCode`, `continueUrl`, optional `apiKey` and
optional `lang` fields, with no duplicate or unknown parameters. The continuation must match
the current origin and fixed `/` path, contain only the canonical operation UUID and have no
userinfo or fragment. The link's API key is never trusted as server configuration. Code and
binding validation remain server obligations; parsing a link is no mailbox proof.

Before constructing navigation or making application I/O, the Web owner captures the action
code into RAM and replaces the entire query with the safe registration fragment. A failed scrub
blocks application I/O. Opening or prefetching the URL never posts confirmation. The action
page declares `no-referrer` before assets; the authorized deployment must additionally prove
no-store and query-free access/error logging at every ingress, CDN and Web host. Client-side
scrubbing cannot retroactively protect the first GET request or an external email/link scanner.

Only explicit confirmation posts the strict `FirebaseConfirmRequest` body to
`/api/v1/theatre/auth/registration/firebase/confirm`. Callers cannot choose email, UID, person,
issuer, role, return URL or setup recipient. The private response is separate from the legacy
SMTP receipt:

```json
{"status":"setup_ready","login":"listener_fixture","setup_url":"https://idm.example.invalid/ui/reset?token=fixture-only"}
```

`setup_ready` means that the server acknowledged provisioning and returned this one browser's
credential setup capability. It does not mean credentials are complete, a session exists or
inbox delivery succeeded. The other closed outcome is `{"status":"unavailable"}`. Unknown
fields, missing fields and account/session claims are refused by DTO and schema. These
responses inherit no-store/no-referrer and never set a session cookie. The server pins the exact
Kanidm origin/path; the Web adapter additionally refuses non-HTTPS URLs, userinfo, fragments,
control characters and malformed/oversized replies.

The Web retains neither code nor setup link in persistent storage, history, logs or DOM hrefs.
The explicit setup button takes the capability from RAM before navigation and cannot replay it.
Navigation, logout and restored page history clear private state and fence old success and
failure callbacks. Only explicit Cancel dispatches `POST /auth/registration/firebase/cancel`
with the same strict action request. It inspects/consumes the actual purpose/UID-bound code,
then durably cancels without Kanidm provisioning; an operation UUID alone cannot cancel.
It cannot undo a winning confirmation, and a generic202 does not prove cancellation won.
Lifecycle cleanup merely forgets the browser action without provider mutation. A lost confirmation/setup response, failed navigation,
reload or interruption needs reconciliation; the UI never retrieves the capability or blindly
retries confirmation. A separate subsequent OIDC login and server-confirmed `/session` are
required to show an authenticated account. Firebase action proof never becomes a Kanidm or
Cantos authentication session and grants no Studio/Admin/Membership authority.

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
