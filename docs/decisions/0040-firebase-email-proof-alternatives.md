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
| 4. Email-link sign-in (`EMAIL_SIGNIN`) with a per-send `continueUrl`; the Cantos origin receives the link and the API calls `accounts:signInWithEmailLink` | Possession of the emailed link, consumed server-side; the response carries the ID token whose `email` is the recipient. The action URL is chosen per request, not as a template setting, so the Spark restriction may not apply. | **Unverified.** Not shown to avoid the default handler consuming the code first; no non-consuming inspection is known; Firebase assigns the UID, so ADR 0036's dedicated-UID exclusivity must be redesigned; needs Email-link sign-in enabled. Candidate only. |
| 5. Cantos mints a custom token for the initiator who then proves the email | The initiator, not the mailbox owner, holds the token. | **Forbidden** by the task and unsafe. |
| 6. Another mail path (SMTP) | Proof by delivery of Cantos' own message | Outside the owner's constraint (no second provider); remains the existing ADR 0034 mode. |

If any ID token ever becomes evidence (option 4), it needs the separate exact Firebase RS256
verifier ADR 0036 already requires: signature with rotating keys, issuer, audience, subject,
time window, project, revocation or disabled state, and the operation binding.
[Verify ID tokens](https://firebase.google.com/docs/auth/admin/verify-id-tokens).

## Conclusion

1. **The default Firebase handler cannot provide a safe proof for this architecture** (options 1
   and 2). No design that relies on polling `emailVerified`, `continueUrl` or a redirect can
   keep A from B's capability. No such workaround is implemented or proposed.
2. The safe path is possession of a code or link that only the mailbox owner holds, **consumed
   by Cantos** (options 3 or 4). Option 3 is built and tested but needs a Firebase project that
   accepts a custom action URL; option 4 needs an owner-authorized experiment and a new design.
3. Proven here: the threat model, why options 1, 2 and 5 fail, and that option 3 satisfies the
   binding in fixtures, emulator and one live run with the action URL copied by hand.
   Not proven: that the default hosted handler never consumes an `EMAIL_SIGNIN` code before
   redirecting, what a plan change does to the action-URL restriction, and project/key
   association.

## Requested decisions (none taken by this record)

- **A.** Authorize a bounded probe of whether `cantos-a348b` accepts the action URL after the
  owner's chosen plan or project change, using at most the approved recipient and mail budget.
- **B.** Or authorize a probe (at most two mails to the approved recipient) of option 4: does
  the default handler redirect an `EMAIL_SIGNIN` link to the Cantos origin with the code
  unconsumed? A positive result opens a new ADR for that design; a negative result closes it.
- **C.** If neither yields a safe proof, keep Firebase mode disabled and use the existing
  SMTP enrollment (ADR 0034) when a mail relay is acceptable.

## Consequences

- Product and user impact: none today; Firebase-mode signup stays disabled in production.
- Compatibility and migrations: none. No contract, migration or capability changes.
- Operations: production must not enable `CANTOS_THEATRE_REGISTRATION_ENABLED` for Firebase mode
  until the action handler gate is accepted.
- Deferred questions: the plan/handler rule, option 4 semantics, least-privilege signing account.

## Validation and revisit conditions

Revisit when decision A or B yields live evidence, or when Firebase documents the hosted
handler's `signIn` behavior. Evidence for the surrounding hardening that *did* ship (trusted
edge, registration grants, edge routing) is in the
[enrollment audit](../evidence/theatre-enrollment-audit-20261017.md).
