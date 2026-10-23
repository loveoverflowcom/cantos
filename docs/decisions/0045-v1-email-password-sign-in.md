# ADR 0045: v1 email/password sign-in for Theatre and Studio

- Status: Accepted as the #335 v1 scope (recorded from the project owner's task brief of 2026-10-21); native backend/session foundation and Web/CMP consumers exist; full HTTP/browser/device acceptance remains separate
- Date: 2026-10-21
- Decision owner: Cantos owner
- Related issues: #331, #334, #335, #333, #295
- Supersedes in part: the "Studio WebAuthn/passkey (D3)" requirement for v1; D3 stays the target for a later slice

## Context

The native refactor selected passkey and recovery codes for Studio. The owner closed the v1 scope
as email/password with mailbox verification, to ship a working sign-in before the heavier factors.

## Decision

1. v1 sign-in is email + password, with a mailed proof, for **Theatre and Studio**. Identity
   accepts `Purpose::Studio` with `Assurance::Password`; the passkey assurance stays admitted as a
   separate, deferred factor and is never relabelled as a password.
2. Identity keeps no Studio roster. The Server owns it (`CANTOS_IDENTITY_STUDIO_SUBJECTS`) and checks
   it at login and at every admission. Login never grants Studio, Admin, producer or membership
   authority; each keeps its own existing check.
3. Deferred, not done: passkey/MFA, recovery codes, credential recovery and change-password, phone,
   Google, Studio operator provisioning, the security-event consumer. Provider endpoints have since been removed under the source-retirement amendment.
   Their checklist items in #335 stay open.
4. No second session engine: the existing `SessionPolicy`, rotation/replay and token hashing are
   reused. Web receives an HttpOnly cookie; native an opaque Bearer.
5. Provider-free native sessions must survive provider retirement. The initial PostgreSQL
   transition is historical; current composition uses the one Server-owned Turso file. Business
   porting and full HTTP acceptance remain incomplete under #333.

## Consequences

### Owner amendment: Firebase verification (23 October 2026)

The corrected owner task selects Firebase Authentication for verification email/actions and
the Auth Emulator for local Theatre development. This supersedes source retirement only for
that external verification boundary. Native Identity/Turso keep subjects, credentials, security
state and authentication facts; Firebase verification never issues a Cantos session or links
accounts by email. [The findings and bridge proposal](../architecture/firebase-verification.md)
separate implemented local/config guards from the **NOT_IMPLEMENTED** native bridge and
production adapter. A Firebase action cannot be placed behind the existing native-secret Mailer
without a new durable operation-bound protocol. Until then, selection fails closed; no native
proof secret is placed in a Firebase continuation. PG/Kanidm/OIDC login remain retired.

A Studio account is weaker than the D3 target until a stronger factor ships; the roster, the short
Studio lifetimes (idle 300 s, absolute 600 s) and per-admission security-state reads bound that.
The wire/behavior contract is [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md).
