# ADR 0036: Firebase mailbox proof for retained Kanidm enrollment

- Status: proposed; isolated prototype only, no provider or setup-delivery choice accepted
- Date: 2026-10-16
- Decision owner: Cantos product owner, with identity/security and deployment operators
- Related issue or PR: [#206](https://github.com/loveoverflowcom/cantos/issues/206), [#174](https://github.com/loveoverflowcom/cantos/issues/174), [#191](https://github.com/loveoverflowcom/cantos/issues/191)
- Supersedes / superseded by: none; proposes a narrow amendment to [ADR 0034](0034-theatre-signup-and-provisioned-studio.md)

## Context

The owner prefers Firebase's built-in verification mail without another mail vendor. The
existing registration application owns durable challenges, new-person provenance, Kanidm
setup intents and SMTP delivery. PR #196 has merged into this baseline; its merge does not
accept outstanding qualification, policy, PostgreSQL or live gates. Firebase is a proof
provider, not an implementation of the existing generic `Mailer`.

ADR0034 requires sending the Kanidm setup link to the verified mailbox. Firebase's verification
sender cannot send that arbitrary message. A protected continuation could avoid the second
email, but requires an explicit delivery decision. This proposal does not change the accepted
rule. No Firebase mode is mounted, configured or advertised by this slice.

## Decision

**Proposed A:** Firebase proves enrollment mailbox control; Kanidm retains credentials and
genuine OIDC authentication; evaluate a protected setup continuation. Keep Firebase unavailable
until bootstrap, binding, delivery and operational gates are accepted. Existing registration
composition and capability remain unchanged.

The first review boundary is a test-only REST contract prototype inside the existing
registration owner: pure binding/fence decisions and HTTP fixtures, no new signup application,
service, trait, dependency, DTO, SQL or UI. Its candidate proof cannot construct the current
application challenge or an authenticated principal and is excluded from non-test builds.

### Supported contracts and limits

| Step | Exact official contract | Limit / required decision |
| --- | --- | --- |
| Password-free user bootstrap | Admin `createUser` accepts explicit UID/email, `emailVerified:false`, and no password; duplicates fail. [Manage users](https://firebase.google.com/docs/auth/admin/manage-users#create_a_user). | Candidate: one dedicated never-reused UID per enrollment. Privileged creation, conflicts, effective permissions, retention and reconciliation remain unimplemented. Never adopt/reset/delete a discovered identity. |
| Send credential | Server signs a custom RS256 token for that UID; `accounts:signInWithCustomToken` exchanges it for ID/refresh tokens. [Custom tokens](https://firebase.google.com/docs/auth/admin/create-custom-tokens#create_custom_tokens_using_a_third-party_jwt_library), [exchange](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/signInWithCustomToken). | Custom sign-in alone may create a UID without the intended email attribute. No bootstrap token goes to an anonymous caller. No runtime developer-CLI login. Not exercised here. |
| Built-in verification mail | `accounts:sendOobCode` with `VERIFY_EMAIL` and ID token returns email. [REST guide](https://firebase.google.com/docs/reference/rest/auth#section-send-email-verification). | No arbitrary body/OTP/setup mail. The general [send schema](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/sendOobCode) also labels email required where the dedicated guide omits it; resolve in separately authorized acceptance. |
| Non-consuming action check | `accounts:resetPassword` with only `oobCode` checks purpose without mutation/consumption; response has `requestType`, email and optional newEmail. [Check](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/resetPassword). | Require `VERIFY_EMAIL` and pending email before consume. No UID/project/operation ID/issue time is returned. Never pass password fields. |
| Consume the SAME code | `accounts:update` with `oobCode`; [response](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/SetAccountInfoResponse) has `localId` and email. [Update](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/accounts/update). | Require exact dedicated UID/email after success. Update accepts other purposes, so success alone is insufficient. The guide's confirmation example omits `emailVerified`; a present false value is refused, without inventing mandatory presence. Lost/malformed success stays uncertain. |
| Project/operation binding | Fixed provider origin and server-selected key; standard sending returns no action code/per-send nonce. [Send response](https://docs.cloud.google.com/identity-platform/docs/reference/rest/v1/GetOobCodeResponse). | Responses do not prove key-to-project association or echo the local operation. Dedicated UID exclusivity and independently verified configuration are prerequisites. Matching email, `continueUrl` and historical verified state cannot supply binding. |

`returnOobLink:true` / Admin link generation sends **no mail**; it cannot solve built-in
delivery. Normal mail offers no pre-send code digest to persist. Reviewed contracts do not
guarantee latest-resend invalidation: local epochs do not prove old Firebase codes invalid.
Never delete/recreate users to evade quota. The prototype only tests supplied local facts.

If a future design admits ID tokens, add the separate Firebase RS256/rotating-key/exact
issuer/audience/subject/time verifier and explicit disabled/revoked-user policy. Do not broaden
Kanidm's ES256 verifier. No ID-token admission/verifier is implemented here.
[Official verification](https://firebase.google.com/docs/auth/admin/verify-id-tokens).

### Proposed setup continuation and recovery

Only possession and explicit consumption of the actual email action may open a continuation.
The initiating browser, polling receipt, `emailVerified:true`, refreshed tokens, `auth_time`,
redirect state and matching email provide no authority. Same-device and cross-device completion
must use the actual action, independently of the initiating cookie. Public status and
`continueUrl` never return setup material or sessions.

Candidate: after a durable one-winner proof fence and fresh validity recheck, reuse the
positively acknowledged new-person/intent flow and deliver setup only to the confirming
browser. Any continuation capability must be purpose-separated, hashed at rest, short-lived
and inaccessible to initiator polling. Pin the Kanidm destination; require no-store,
no-referrer, safe history handling and explicit navigation. Decide one-shot RAM delivery
versus reviewed encrypted recovery storage before implementation. No raw bearer persistence
is authorized by this proposal.

Reserve before provider consumption, person creation, intent creation and continuation
release. A consumed code with lost response/failed local commit stays uncertain; lookup
cannot recover action possession. A lost setup response needs an agreed consumption/recovery
protocol, never blind intent/reset replay. Existing SQL fences do not prove these new claims.

Firebase URLs contain query codes, unlike current fragment challenges. A custom handler must
show explicit confirmation and let the server consume; the default handler applying the code
before redirect is incompatible. Review CDN/ingress/proxy logs, caches, referrers and analytics
before the first request, not only JS scrubbing. GET/scanner prefetch must not provision.
Origins/CSRF, action leakage and cross-device continuation remain separate gates.
[Custom handlers](https://firebase.google.com/docs/auth/custom-email-handler).

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| A: Firebase proof + Kanidm + protected continuation | Preserves accepted issuer and vendor preference; potentially avoids second mail | Two identity records; bootstrap, binding/resend, continuation/recovery and delivery amendment need review | Recommended prototype direction, not accepted integration |
| B: Firebase primary Theatre issuer | One Firebase authentication lifecycle | Explicit session/issuer migration, linking and revocation decisions | Outside this slice; no silent migration |
| C: existing SMTP enrollment | Implemented verification/setup-mail sequence | Supported delivery remains necessary; does not meet requested vendor outcome | Preserved baseline; no fallback/new vendor selected |

## Consequences

- Product/user impact: no runtime behavior change. Verification, credential setup, login,
  authorization and linking remain distinct. Studio/Admin retain independent admission.
- Compatibility/migrations: no wire, SQL, cookie, actor registry, historical migration or
  application-port change. Prototype is test-only.
- Operations/storage/cost: zero live calls/mail/configuration. If adopted, Firebase retains
  necessary Auth records; business data remains in PostgreSQL on the VPS. No default
  Firestore, Functions or extra mail vendor.
- Recovery: accepted design must precede durable claims, quotas, retention and reconciliation.
- Deferred questions: exact project/tenant/privilege, existing-email handling, naming,
  UID exclusivity/resend lifecycle, protected delivery, origins and retention.

## Validation and revisit conditions

The [evidence ledger](../evidence/firebase-enrollment-proof.md) separates pure/fixture results
from emulator, signed-token, PostgreSQL, browser and live acceptance. The
[operator review plan](../deployment/firebase-enrollment-proof.md) prepares those gates; it is
not activation instructions. The registered registration Quint model remains draft/model-only;
abstract proof epochs prove neither Firebase binding nor implementation conformance.

Acceptance requires an explicit owner decision, not a passing test or PR merge. If bootstrap,
exclusive binding or protected delivery cannot be demonstrated, retain unavailable Firebase
and name the blocker. Do not silently switch issuers, hide SMTP behind Firebase or close
#174/#191 from this prototype.
