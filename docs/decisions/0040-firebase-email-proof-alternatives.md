# ADR 0040: Firebase email-proof alternatives under the default action handler

- Status: proposed; no authentication behavior changes with this record
- Date: 2026-10-17
- Decision owner: Cantos product owner, with identity/security and deployment operators
- Related issue or PR: [#206](https://github.com/loveoverflowcom/cantos/issues/206), [#174](https://github.com/loveoverflowcom/cantos/issues/174), [#191](https://github.com/loveoverflowcom/cantos/issues/191)
- Supersedes / superseded by: does not supersede [ADR 0036](0036-firebase-enrollment-proof.md); records the
  evidence an amendment would need. ADR 0036 stays the accepted code/test direction.

## Context

The owner wants Firebase Authentication to do exactly one job (deliver Firebase's stock
verification mail and give evidence that the mailbox is controlled), with Cantos API as the sole
orchestrator and Kanidm keeping credentials, MFA and OIDC. The owner also prefers no custom
template, brand or SMTP. Live run #206 found that the Spark project `cantos-a348b` refused to set
a custom action URL (`EMAIL_TEMPLATE_UPDATE_NOT_ALLOWED`; the console failed too), so the mail
kept Firebase's default hosted handler. This record answers whether that default can supply the
proof ADR 0036 requires, and what remains if it cannot.

Known facts (documents are linked; observations are from the [#206 record](../evidence/theatre-live-auth-206.md)):

- A custom **action handler** (where the link points) and a custom **template or sender domain**
  are separate Firebase features. Cantos needs only the first; ADR 0036 asks for no custom body.
  [Custom handlers](https://firebase.google.com/docs/auth/custom-email-handler),
  [custom domain](https://firebase.google.com/docs/auth/email-custom-domain).
- With the default handler, Firebase's hosted page applies the code itself and the user may
  continue to the `continueUrl`. The retrieved pages do not say that anything provable about
  *who* applied the code reaches `continueUrl`, and they do not describe the hosted handler's
  behavior for `signIn` links at all.
  [Passing state](https://firebase.google.com/docs/auth/web/passing-state-in-email-actions).
- In #206 the Firebase user already had `emailVerified=true` before Cantos consumed anything
  (cause undetermined; a mail or link scanner on the default handler is consistent with it).

Assumption to test, not a fact: whether any plan change removes the action-URL restriction.
No retrieved document states the rule behind `EMAIL_TEMPLATE_UPDATE_NOT_ALLOWED`.

## Threat model

Roles: **A** types B's address and keeps the initiating browser; **B** owns that mailbox and
opens the mail; **S** is a link scanner or prefetcher that opens mail links on behalf of B.
The binding to prove is
`Firebase project → verification event → UID → email → registration operation → mailbox owner`.
Insufficient alone: `emailVerified=true`, matching email, the registration UUID, `continueUrl`,
the initiator's cookie, a Firebase redirect, or an unverified ID token.
Required outcome: **A never receives B's credential-setup capability**, and S cannot create a person.

## Options

| Option | What it proves to the API | Verdict |
| --- | --- | --- |
| 1. Default hosted handler; API reads `emailVerified` or the user record (polling) | Only that *someone* applied *a* code for the UID. A, polling from the initiating browser, would then be handed B's setup link; S can flip the flag. | **Rejected: unsafe.** This is the exact A-enters-B scenario. |
| 2. Default hosted handler; user lands on `continueUrl` | Nothing: the page can be opened by A directly, carries no code or token, and the code is already consumed. | **Rejected: unsafe.** |
| 3. Custom action handler on the Cantos origin, stock mail (ADR 0036) | Possession of the actual `oobCode`: a non-consuming purpose check, then the API consumes the same code and requires the exact dedicated UID and email. Initiator cookie, polling and tokens grant nothing. | **Safe, and already implemented**, but **blocked** on the Spark project. Unknown whether a plan change lifts it. |
| 4. Email-link sign-in (`EMAIL_SIGNIN`) with a per-send `continueUrl`; the application origin receives the link and a server calls `accounts:signInWithEmailLink` | Possession of the emailed link, consumed server-side; the response includes `localId`, `email` and tokens. The action URL is chosen per request, not as a template setting. | **Observed in the controlled follow-up:** two forwarded codes redeemed successfully, each replay refused. Admin precreation retained the dedicated UID; sending to an absent user was refused with client signup disabled. No non-consuming inspection is known. Needs an explicit ADR 0036 amendment and implementation evidence before activation. |
| 5. Cantos mints a custom token for the initiator who then proves the email | The initiator, not the mailbox owner, holds the token. | **Forbidden** by the task and unsafe. |
| 6. Another mail path (SMTP) | Proof by delivery of Cantos' own message | Outside the owner's constraint (no second provider); remains the existing ADR 0034 mode. |

If any ID token ever becomes evidence (option 4), it needs the separate exact Firebase RS256
verifier ADR 0036 already requires: signature with rotating keys, issuer, audience, subject,
time window, project, revocation or disabled state, and the operation binding.
[Verify ID tokens](https://firebase.google.com/docs/auth/admin/verify-id-tokens).

## Conclusion

1. **The default `VERIFY_EMAIL` handler with polling or continuation cannot provide a safe proof** (options 1
   and 2). No design that relies on polling `emailVerified`, `continueUrl` or a redirect can
   keep A from B's capability. No such workaround is implemented or proposed.
2. The safe path is possession of a code or link that only the mailbox owner holds, **consumed
   by Cantos** (options 3 or 4). Option 3 is built and tested but needs a Firebase project that
   accepts a custom action URL; option 4 now has positive live code-consumption evidence with
   admin precreation, but still needs an explicit design amendment and implementation verification.
3. Proven here: the threat model, why options 1, 2 and 5 fail, that option 3 satisfies the
   binding in fixtures, emulator and one live run with the action URL copied by hand, and (probe B)
   that the default handler forwards an `EMAIL_SIGNIN` code to the continue origin. The controlled
   follow-up additionally observed two successful consumptions and refused replays, retaining the
   exact admin-created UID. The earlier inconclusive attempt is not reclassified.
   Not proven: production delivery/operation binding, all scanner behavior, what a plan change
   does to the action-URL restriction, and independent project/key association.

## Requested decisions (none taken by this record)

- **A.** Authorize a bounded probe of whether `cantos-a348b` accepts the action URL after the
  owner's chosen plan or project change, using at most the approved recipient and mail budget.
- **B.** *(Original budget spent/inconclusive; fresh two-mail follow-up completed positively.)*
  The owner delegated the follow-up and selected a fresh mailbox. Direct send to an absent user
  was refused, so the authorized probe used explicit admin precreation without enabling client
  signup. Two forwarded codes were accepted; the measured second handler-open-to-success interval
  was 10.551 seconds. Both replays were refused. This opens a design amendment for option 4; it
  does not accept or implement that amendment.
- **C.** If neither yields a safe proof, keep Firebase mode disabled and use the existing
  SMTP enrollment (ADR 0034) when a mail relay is acceptable.

## Probe A result (2026-10-17)

With the owner's authorization to re-set only the action URL (no plan, billing, authorized-domain
or mail change), one `PATCH` of `notification.sendEmail.callbackUri` to `https://cantoss.duckdns.org/`
on project `cantos-a348b` (Identity Toolkit admin v2, through the signed-in Firebase CLI's own
client) was rejected: HTTP 400 `EMAIL_TEMPLATE_UPDATE_NOT_ALLOWED`, the same as in #206. A read-back
shows the project unchanged (`https://cantos-a348b.firebaseapp.com/__/auth/action`). The plan was
not re-inspected, so the cause stays unestablished; no mail was sent. Option 3 therefore remains
blocked on this project, and the remaining choices are a different project/plan the owner names,
probe B, or C.

## Probe B result (2026-10-17)

With the owner's authorization (one recipient, at most two mails, email-link enabled temporarily,
no plan or authorized-domain change), one `EMAIL_SIGNIN` mail was sent through the admin
`accounts:sendOobCode` endpoint with `continueUrl=https://cantos-a348b.web.app/?probe=1`
(an already-authorized domain) and `canHandleCodeInApp:true`. The owner then opened the mail's
link and reported the browser's final URL. Observed (the code itself is deliberately not recorded):

- the send succeeded on this project, so a **per-request `continueUrl` is accepted where the
  project-wide action URL is refused** (probe A);
- the browser ended on the `continueUrl` origin, carrying `probe=1`, `apiKey`, `mode=signIn`,
  `oobCode` and `lang`: the default hosted handler **forwarded the code to the application instead
  of keeping it**;
- `signIn.email.passwordRequired` was restored to `true` by the owner right after (output seen).

Not shown: that the forwarded code was still **unconsumed**. The documented client flow
(`isSignInWithEmailLink` then `signInWithEmailLink` on the redirected page) implies it, but
proving it means redeeming a code, which would sign in or create a Firebase user and was not
authorized. Also unobserved: behavior for a recipient without a Firebase user while client
sign-up is disabled, the server-side `accounts:signInWithEmailLink` response shape, and
whether any non-consuming inspection exists. One of the two approved mails remains unused.

Redemption attempt (second mail, same day): the owner first ran an unrelated cleanup that deleted
the leftover #206 test Firebase user for the approved recipient, and then, some minutes after the
click, re-enabled email-link and redeemed the forwarded code once. Firebase answered
`400 INVALID_OOB_CODE` for the first redemption and for the replay. This is **inconclusive**: the
code may have been consumed by the hosted handler or a scanner, invalidated by the user deletion,
or expired, and the run cannot tell these apart. It also resembles #206's first mail, whose code was
invalid for an undetermined reason, so it is a reason for caution, not proof of either outcome.
Both approved mails are spent; both times email-link was disabled again (output seen). Both mails
went to an email that already had a Firebase user, so the no-user case is unobserved.

Effect at that checkpoint: option 4 had partial observed support on the current
project (send accepted, code forwarded) but its decisive property is **unproven**; it is also a
**new design** (UID is assigned by Firebase, the code is redeemed rather than
inspected first, dedicated-UID exclusivity and the operation binding must be redone) and needs its
own ADR and owner decision before any code. These were the limits of the original probe; the
controlled follow-up below supplies the later evidence and narrows the UID concern.

## Controlled follow-up: fresh mailbox (2026-10-17)

The owner delegated the next approach and named a fresh mailbox, with a budget of at most two
new mails. A direct admin lookup found no user for it. The project read-back showed email/password
enabled, `passwordRequired=true`, `disabledUserSignup=true`, and `localhost` already authorized.
No billing, IAM, authorized-domain, action-template, Kanidm or production-deployment change was
made for this probe.

With email-link temporarily enabled, the project-scoped admin `accounts:sendOobCode` call for
`EMAIL_SIGNIN` to that absent user returned **400 `ADMIN_ONLY_OPERATION`**, before any successful
mail acknowledgement. The configuration was restored and read back as `passwordRequired=true`.
This is an observed rejection for this project/configuration, not a result about code forwarding,
consumption, or Firebase email-link support in other configurations. Client signup was not enabled
to get around it.

The bounded follow-up instead starts from a fresh mailbox and explicitly precreates one
password-free, unverified, dedicated test UID through the project-scoped Admin create endpoint,
as the existing Cantos bootstrap does in purpose (the API uses a service-account bearer, while
this diagnostic uses the operator's Firebase CLI OAuth authority). Lookup confirmed the exact UID
and email before sending. One stock `EMAIL_SIGNIN` mail was accepted at **07:50:20 UTC**, with a
per-send continuation on the existing authorized `localhost` domain. `disabledUserSignup` remains
true. No user deletion or configuration change is allowed between that send and redemption.

The temporary loopback receiver records arrival and redemption timestamps, requires an explicit
POST within 60 seconds of arrival, and never consumes a code on GET. It keeps the code and any
returned tokens in RAM, omits them from its event logs, and restores email-link to disabled after
the outcome or a ten-minute timeout. The Gmail connector retrieved both actual stock mails from
Spam; a real browser opened their original links through Firebase's default hosted handler.
The diagnostic controller then posted to the local receiver to redeem the forwarded code.
The first browser-form POST was refused locally before dispatch; this is not a production UI or
CSRF test. No Cantos registration route, Kanidm operation or setup capability was exercised.

| Observation (UTC) | First mail | Second mail, same test UID |
| --- | --- | --- |
| Mail acknowledged | 07:50:20.257 | 07:58:37.107 |
| Browser handler opening recorded | Not timestamped | 07:59:09.053 |
| Forwarded code reached loopback | 07:56:18.887 | 07:59:10.723 |
| Redemption dispatched | 07:57:00.394 (41.51 s after arrival) | 07:59:18.744 (8.02 s after arrival) |
| First redemption response | 07:57:01.364, **200** | 07:59:19.604, **200** (10.551 s after handler opening) |
| Exact email and admin-precreated UID | Match; `isNewUser=false` | Match; `isNewUser=false` |
| Replay | **400 `INVALID_OOB_CODE`** | **400 `INVALID_OOB_CODE`** |
| Admin lookup after consumption | Same UID/email, `emailVerified=true` | Same UID/email, `emailVerified=true` |
| Email-link disabled, read-back | 07:57:02.893 | 07:59:21.243 |

There was no deletion or configuration change between either successful send and its first
redemption. Both successful responses included `idToken`, `refreshToken`, `expiresIn`, `localId`,
`email`, `isNewUser` and `kind`; tokens were discarded, not admitted as Cantos identity evidence
or independently signature-verified. The second run meets the requested one-minute interval from
handler opening; the first only measures arrival-to-dispatch and does not establish click time.

**Result:** the default handler did not consume either `EMAIL_SIGNIN` code in these two journeys.
Option 4's decisive property is now observed on this project with a dedicated admin-precreated
user and client signup disabled. Firebase need not assign a new UID in that variant, so abandoning
ADR 0036's dedicated-UID rule is not justified by these results. Non-consuming inspection,
purpose checking through consumption, durable operation fences, mismatches, lost responses,
revocation and confirmation-only setup release still need an explicit design and production-code
tests. This is not acceptance of those invariants or of production activation.

Cleanup deleted only this run's exact temporary UID after both replay checks. Direct read-back
confirmed zero users for the trial mailbox, `passwordRequired=true`, `disabledUserSignup=true`,
and unchanged authorized domains/action URL. Both new mails are spent. No #206 key, Kanidm person
or unrelated account was changed. Local receivers were stopped after evidence capture.

The earlier statement that the no-user case was unobserved refers to probe B's original two
mails. This follow-up observes a fresh-mailbox send rejection and a separate admin-precreated
case; it does not claim automatic creation by `signInWithEmailLink` with client signup disabled.

## Consequences

- Product and user impact: none today; Firebase-mode signup stays disabled in production.
- Compatibility and migrations: none. No contract, migration or capability changes.
- Operations: production must not enable `CANTOS_THEATRE_REGISTRATION_ENABLED` for Firebase mode
  until the action handler gate is accepted.
- Deferred questions: the plan/handler rule, option 4's explicit design/implementation acceptance,
  least-privilege signing account and production delivery qualification.

## Validation and revisit conditions

Revisit when decision A or B yields live evidence, or when Firebase documents the hosted
handler's `signIn` behavior. Evidence for the surrounding hardening that *did* ship (trusted
edge, registration grants, edge routing) is in the
[enrollment audit](../evidence/theatre-enrollment-audit-20261017.md).
