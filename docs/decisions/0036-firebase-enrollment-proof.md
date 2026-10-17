# ADR 0036: Firebase mailbox proof for retained Kanidm enrollment

- Status: accepted code/test direction A and one-shot setup continuation; live activation unaccepted
- Date: 2026-10-16
- Decision owner: Cantos product owner, with identity/security and deployment operators
- Related issue or PR: [#206](https://github.com/loveoverflowcom/cantos/issues/206), [#174](https://github.com/loveoverflowcom/cantos/issues/174), [#191](https://github.com/loveoverflowcom/cantos/issues/191)
- Supersedes / superseded by: narrowly amends the Firebase setup-delivery path in [ADR 0034](0034-theatre-signup-and-provisioned-studio.md)

## Context

The owner prefers Firebase's built-in verification mail without another mail vendor. The
existing registration application owns durable challenges, new-person provenance, Kanidm
setup intents and SMTP delivery. PR #196 has merged into this baseline; its merge does not
accept outstanding qualification, policy, PostgreSQL or live gates. Firebase is a proof
provider, not an implementation of the existing generic `Mailer`.

ADR0034's SMTP path sends the Kanidm setup link to the verified mailbox. Firebase cannot
send that arbitrary message. The product owner explicitly approved Firebase mailbox proof,
a dedicated UID for each enrollment, and a one-shot setup link returned only to the browser
which explicitly confirms the actual email action. Kanidm retains credentials and OIDC.
The approval covers code and isolated tests, with the mode disabled until operator acceptance;
it grants no live account, key, grant, mail, template, legal-term or security-setting action.
The owner named project `cantos` for read-only CLI inspection. An initial expired CLI login
failed; after the operator reauthenticated, the unique display name resolved to ACTIVE project
`cantos-a348b` and default Hosting URL `https://cantos-a348b.web.app`. The inventory shows an
Android demo app, no Web app. That metadata accepts neither action hosting nor API/Kanidm
origins, credentials, mail or authentication.

## Decision

**A is selected for code and isolated tests:** Firebase proves the enrollment mailbox;
Kanidm retains credentials and genuine OIDC authentication. A dedicated password-free,
unverified Firebase UID is derived from each fresh operation, never adopted from an existing
identity. Setup is delivered once in the confirming HTTP response, kept only in RAM and never
stored for recovery. Losing that response requires reconciliation; it cannot reissue an intent.
This replaces the second setup-email requirement only for explicitly selected Firebase mode.
The SMTP mode retains ADR0034's email sequence and is never an automatic fallback.

The first boundary, PR #208, was test-only. The subsequent implementation compiles the
bounded REST adapters, durable operation fences, selected host composition, HTTP contract and
Web confirmation in the existing registration owner. It remains off by default. Operator
approval declarations do not prove effective provider privileges, project binding, safe action
hosting, configured origins or a completed live signup.

### Supported contracts and limits

| Step | Exact official contract | Limit / required decision |
| --- | --- | --- |
| Password-free user bootstrap | Admin `createUser` accepts explicit UID/email, `emailVerified:false`, and no password; duplicates fail. [Manage users](https://firebase.google.com/docs/auth/admin/manage-users#create_a_user). | Selected: one dedicated never-reused UID per enrollment. The adapter requires a positive exact create acknowledgment. Effective permissions, retention and operator reconciliation remain activation gates. Never adopt/reset/delete a discovered identity. |
| Send credential | Server signs a custom RS256 token for that UID; `accounts:signInWithCustomToken` exchanges it for ID/refresh tokens. [Custom tokens](https://firebase.google.com/docs/auth/admin/create-custom-tokens#create_custom_tokens_using_a_third-party_jwt_library), [exchange](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/signInWithCustomToken). | Custom sign-in alone may create a UID without the intended email attribute. No bootstrap token goes to an anonymous caller. No runtime developer-CLI login. Signed synthetic fixtures do not establish real IAM/signing acceptance. |
| Built-in verification mail | `accounts:sendOobCode` with `VERIFY_EMAIL` and ID token returns email. [REST guide](https://firebase.google.com/docs/reference/rest/auth#section-send-email-verification). | No arbitrary body/OTP/setup mail. The general [send schema](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/sendOobCode) also labels email required where the dedicated guide omits it; resolve in separately authorized acceptance. |
| Non-consuming action check | `accounts:resetPassword` with only `oobCode` checks purpose without mutation/consumption; response has `requestType`, email and optional newEmail. [Check](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/resetPassword). | Require `VERIFY_EMAIL` and pending email before consume. No UID/project/operation ID/issue time is returned. Never pass password fields. |
| Consume the SAME code | `accounts:update` with `oobCode`; [response](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/SetAccountInfoResponse) has `localId` and email. [Update](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/update). | Require exact dedicated UID/email after success. Update accepts other purposes, so success alone is insufficient. The guide's confirmation example omits `emailVerified`; a present false value is refused, without inventing mandatory presence. Lost/malformed success stays uncertain. |
| Project/operation binding | Fixed provider origin and server-selected key; standard sending returns no action code/per-send nonce. [Send response](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/GetOobCodeResponse). | Responses do not prove key-to-project association or echo the local operation. Dedicated UID exclusivity and independently verified configuration are prerequisites. Matching email, `continueUrl` and historical verified state cannot supply binding. |

`returnOobLink:true` / Admin link generation sends **no mail**; it cannot solve built-in
delivery. Normal mail offers no pre-send code digest to persist. Reviewed contracts do not
guarantee latest-resend invalidation: local epochs do not prove old Firebase codes invalid.
Never delete/recreate users to evade quota. The implementation allows a resend for the same live UID under its unchanged absolute
operation deadline; it does not claim latest-code invalidation. Cancellation consumes the actual
purpose/UID-bound action and durably terminates that operation without creating a Kanidm person.
Expiry/cancellation never reopens it. A fresh successor has a new UID, but retained Firebase
email uniqueness may reject it with EMAIL_EXISTS. That stays blocked for operator reconciliation;
there is no adoption, reset or automatic deletion to make retries succeed.

If a future design admits ID tokens, add the separate Firebase RS256/rotating-key/exact
issuer/audience/subject/time verifier and explicit disabled/revoked-user policy. Do not broaden
Kanidm's ES256 verifier. No ID-token admission/verifier is implemented here.
[Official verification](https://firebase.google.com/docs/auth/admin/verify-id-tokens).

### Selected one-shot setup continuation and recovery

Only possession and explicit consumption of the actual email action may open a continuation.
The initiating browser, polling receipt, `emailVerified:true`, refreshed tokens, `auth_time`,
redirect state and matching email provide no authority. Same-device and cross-device completion
must use the actual action, independently of the initiating cookie. Public status and
`continueUrl` never return setup material or sessions.

After a durable one-winner proof fence and fresh validity recheck, reuse the
positively acknowledged new-person/intent flow and deliver setup only to the confirming
browser. The setup capability is short-lived and inaccessible to initiator polling; no raw or encrypted recovery copy is persisted. Pin the Kanidm destination; require no-store,
no-referrer, safe history handling and explicit navigation. Only the confirming response receives the RAM value. A durable HandedOff phase precedes that response; duplicate or lost-response retries cannot retrieve the link.

Reserve before provider consumption, person creation, intent creation and continuation
release. A consumed code with lost response/failed local commit stays uncertain; lookup
cannot recover action possession. A lost setup response requires a specifically authorized operator reconciliation, never blind intent/reset replay. Migration0028 implements the new phases independently of SMTP migration0027; real SQL acceptance is recorded separately from fixtures.

Firebase URLs contain query codes, unlike current fragment challenges. A custom handler must
show explicit confirmation and let the server consume; the default handler applying the code
before redirect is incompatible. Review CDN/ingress/proxy logs, caches, referrers and analytics
before the first request, not only JS scrubbing. GET/scanner prefetch must not provision.
Origins/CSRF, action leakage and cross-device continuation remain separate gates.
[Custom handlers](https://firebase.google.com/docs/auth/custom-email-handler).

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| A: Firebase proof + Kanidm + protected continuation | Preserves accepted issuer and vendor preference; potentially avoids second mail | Two identity records; bootstrap, binding/resend, continuation/recovery and delivery amendment need review | Selected code/test direction; live acceptance remains separate |
| B: Firebase primary Theatre issuer | One Firebase authentication lifecycle | Explicit session/issuer migration, linking and revocation decisions | Outside this slice; no silent migration |
| C: existing SMTP enrollment | Implemented verification/setup-mail sequence | Supported delivery remains necessary; does not meet requested vendor outcome | Preserved baseline; no fallback/new vendor selected |

## Consequences

- Runtime behavior: an explicit `firebase` provider selection can compose the new registration
  flow; absent configuration stays disabled and partial/orphan/mixed SMTP/Firebase inputs fail.
  Firebase evidence never writes identity registry/session state or gains Studio/Admin access.
- Compatibility: additive migration0028 and capability `firebase_email_verification`; existing
  SMTP contract and historical migration bytes remain intact. Web is the first consumer;
  native consumption and device acceptance are not inferred.
- Operations: an operator must supply service-account signing material securely. The API mints
  and renews its own short-lived `identitytoolkit`-scoped bearer from it in RAM (JWT-bearer
  exchange at the fixed Google token origin; single-flight, expiry margin, fail closed) and
  never uses Firebase CLI login as server authority. The earlier operator-supplied bearer input
  is retired, not kept as a mode; the code creates no account, role or key. A dedicated
  one-permission account and key lifecycle are proposed in the
  [service-account runbook](../deployment/firebase-enrollment-service-account.md), not executed.
- Storage: business state, effect fences, budgets and audit remain in PostgreSQL. Firebase has
  necessary Auth UID/email records. No action code, provider token or setup secret is persisted.
  Current receipts are retained; age-based cleanup requires a separate reviewed procedure.
- Recovery: rejected/unknown effects stop automatic replay. A process killed while inspecting
  conservatively strands that operation; provider reads cannot restore action possession.
- Live observation (16 October 2026): one real run on project `cantos-a348b` with a local Kanidm
  reached mail → Cantos handler → one-shot setup → OIDC login → logout and refused a replayed code
  ([record](../evidence/theatre-live-auth-206.md)). It also showed that action inspection needs the
  Email/Password provider enabled (reported as a 503 configuration fault). It does not accept
  activation, and these gates remain **open**: a custom action handler (the Spark project refused the
  action URL, so the default hosted handler still receives the link), a least-privilege
  signing/creation credential (the broad default Admin SDK key was used; the narrow account is
  designed but not created, and the old key not retired) and cleanup of that key,
  the test user and the test person.
- Unaccepted activation gates: exact project/API-key association, effective Auth and Kanidm
  target protection, existing-email behavior, numeric policy/retention, safe action ingress,
  recipient/mail budget, origins and the real proof→setup→OIDC→session journey.

## Amendment note (2026-10-17)

[ADR 0040](0040-firebase-email-proof-alternatives.md) (proposed) re-examines the
default-handler question against the #206 result: it finds the default hosted handler unable to
provide this proof and lists the owner decisions that could unblock the custom-handler gate. It
changes nothing accepted here.

## Validation and revisit conditions

The [runtime evidence ledger](../evidence/firebase-enrollment-runtime.md) and [earlier prototype ledger](../evidence/firebase-enrollment-proof.md) separates pure/fixture results
from emulator, signed-token, PostgreSQL, browser and live acceptance. The
[operator review plan](../deployment/firebase-enrollment-proof.md) prepares those gates; it is
not activation instructions. The registered registration Quint model remains draft/model-only;
abstract proof epochs prove neither Firebase binding nor implementation conformance.

The recorded user decision accepts the code direction; neither a test nor a PR merge accepts live activation. Keep the mode disabled while effective bootstrap, exclusive project/UID binding, action ingress or protected delivery has no live evidence. Do not silently switch issuers, hide SMTP behind Firebase or close
#174/#191 from this code/test slice.
