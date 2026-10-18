# Theatre identity activation and enrollment handoff

Current issues: [#191 — real login activation](https://github.com/loveoverflowcom/cantos/issues/191),
[#174 — email-verified enrollment](https://github.com/loveoverflowcom/cantos/issues/174) and
[#206 — Firebase email-proof option](https://github.com/loveoverflowcom/cantos/issues/206).
The isolated local test follow-up is [#242](https://github.com/loveoverflowcom/cantos/issues/242);
its [integration/recipient tooling](../deployment/firebase-enrollment-proof.md#isolated-local-enrollment-testing)
does not substitute for the separate real Kanidm/Web journey or production activation.
The previous PR3/integration/session log is [archived](../archive/handoffs/theatre-identity-2026-10-16.md).

## Existing boundary

Studio/Theatre/Admin identity surfaces, exact actor registry and Web/native adapters exist.
[ADR 0034](../decisions/0034-theatre-signup-and-provisioned-studio.md) accepts Theatre
self-registration and one or two provisioned-only Studio administrators. The registration
state machine/contract and Kanidm onboarding direction are implemented in bounded form.
PR #196 is merged; its former Draft instruction is obsolete. Live activation is still separate.

Read the [Theatre architecture](../architecture/theatre-identity.md),
[identity contract](../../packages/contracts/theatre-identity-v1.md),
[activation evidence](../evidence/theatre-auth-activation.md) and
[Kanidm runbook](../../services/kanidm/README.md#theatre-registration-activation-runbook--approval-required-before-execution)
before changing that boundary. Do not rebuild auth from the mock screen or treat a build as login.

## #253 migration dependency

[Bearer v1](../../packages/contracts/bearer-v1.md) owns the API foundation and exact client handoff.
Implement sequentially API/contracts → Leptos Web (including private resources) → CMP lifecycle.
The owner selected cross-site opaque Bearer, RAM-only Web credentials and controlled reload/SSO;
the old cookie/same-site activation prerequisite is superseded. Browser/native/live issuer and
restricted gateway acceptance remain explicit, and enrollment keeps its independent gates.
Do not close #253 or infer cross-client account linking from a merged API foundation.

## Recommended next order

1. Verify exact nonsecret Web/native configuration and capability/error mapping using existing
   adapters. Public anonymous Home remains usable without login.
2. Complete the #174 policy/effective-permission and recovery gates before live enrollment:
   verified-email binding, operation-owned identity, safe uncertain outcomes, cooldown/budgets,
   retention and secure credential-intent delivery. No existing-account reset/retry shortcut.
3. Accept #206's real Firebase proof/Kanidm journey using the default-disabled runtime and
   [operator runbook](../deployment/firebase-enrollment-proof.md). The owner accepted
   [ADR 0036](../decisions/0036-firebase-enrollment-proof.md)'s dedicated UID, actual action
   possession and one-shot setup continuation; production adapters and durable local fences
   now exist. [Local evidence](../evidence/firebase-enrollment-runtime.md) does not establish
   effective cloud privileges, action origins, inbox/setup/OIDC or activation. Firebase proof
   must not become a session, a new issuer/password store or existing-account reset shortcut.
   The [live record](../evidence/theatre-live-auth-206.md) now shows one real journey against
   project `cantos-a348b` and a local Kanidm (mail → Cantos handler → setup → OIDC → logout, replay
   refused). It is not activation: the custom action handler was **not** accepted (Spark refused the
   action URL), the signing/creation credential was the broad default Admin SDK key rather than a
   least-privilege one, and that key, the test Firebase user and the test Kanidm person are not yet
   cleaned up. Each needs its own operator approval.
4. In the specifically authorized environment, observe actual email/action/setup → Kanidm login
   → server-confirmed session → private read → logout/rejected old session. Web and Android/iOS
   system-browser/secure-store/device acceptance are separate observations.

## Invariants and evidence

Preserve purpose-bound single use, current identity/permission checks, immutable operation
receipts and exact retry/conflict behavior. Studio has no public signup; a valid Theatre/native
identity cannot enter Studio or gain an Admin capability. Preview credentials and signup
fixtures never create a real account.

Source/core/SQL/mock evidence is distinct from configured artifact, live provider/mail,
browser and device acceptance. Keep missing configuration unavailable and record actual
blockers with #179/#180. This handoff authorizes no account/key/grant/mail/deployment operation.
