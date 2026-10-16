# Theatre identity activation and enrollment handoff

Current issues: [#191 — real login activation](https://github.com/loveoverflowcom/cantos/issues/191),
[#174 — email-verified enrollment](https://github.com/loveoverflowcom/cantos/issues/174) and
[#206 — Firebase email-proof option](https://github.com/loveoverflowcom/cantos/issues/206).
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

## Recommended next order

1. Verify exact nonsecret Web/native configuration and capability/error mapping using existing
   adapters. Public anonymous Home remains usable without login.
2. Complete the #174 policy/effective-permission and recovery gates before live enrollment:
   verified-email binding, operation-owned identity, safe uncertain outcomes, cooldown/budgets,
   retention and secure credential-intent delivery. No existing-account reset/retry shortcut.
3. Review #206's Firebase proof option and its interaction with accepted Kanidm onboarding.
   PR #208 is merged as a test-only prototype; [ADR 0036](../decisions/0036-firebase-enrollment-proof.md)
   remains proposed. Accept bootstrap/project/UID/action binding and any protected setup-delivery
   amendment before durable integration or live mode; use the [operator review plan](../deployment/firebase-enrollment-proof.md).
   Firebase proof must not silently become login, a new issuer/password store or replacement setup policy.
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
