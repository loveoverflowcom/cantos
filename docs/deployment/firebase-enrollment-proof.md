# Firebase enrollment proof: operator review plan

Preparation for [proposed ADR0036](../decisions/0036-firebase-enrollment-proof.md) and
[#206](https://github.com/loveoverflowcom/cantos/issues/206). There is no runtime Firebase
registration mode, environment-variable contract or activation command. The test-only
prototype uses synthetic HTTP fixtures and sends no mail. Existing Kanidm/registration gates
continue to apply.

## Records required before implementation/activation

| Record | Required content / acceptance owner |
| --- | --- |
| Product/security | Accept option A and the exact replacement for ADR0034's setup-mail rule, or retain that rule and name the separate delivery dependency. Agree existing-email handling, UID exclusivity, resend/cancel, TTL/quotas/retention and uncertain-outcome recovery. |
| Environment | Operator names exact Firebase project/tenant/plan, action/API/Kanidm origins/client and intended test identities/recipients. Inventory existing users and template consumers before any change. No project is selected here. |
| Credentials | Review effective Auth create/custom-signing and Kanidm target restrictions, secret custody and rotation. No admin keys in Git/Web/CMP/argv/receipts/logs; no developer CLI login as server credential. Current fixtures require none. |
| Action handler | Pin HTTPS origins; explicit confirmation; query redaction before ingress logging; no analytics/referrer/cache leakage; scanner-safe GET. Project action-URL changes affect other templates. Use supported current links, no obsolete mobile-link integration. |
| Auth lifecycle | Exact dedicated unverified/password-free user creation, conflicting/disabled/deleted identities, minimal UID/project/email/operation retention/cleanup. No adoption/reset, automatic delete/recreate or quota evasion. |
| Mail/abuse | Actual plan limits, durable recipient/network/operation/global budgets and cooldown; generic quota refusal/uncertain send. No automatic SMTP fallback or inbox-delivery claim. |
| Continuation | One-winner durable claim, fresh expiry after waits, action-bound same/cross-device completion, bounded protected delivery, consumption and lost-response recovery. Initiating/polling browsers receive no setup material/session. |

Firebase documents different Spark budgets for address-verification mail (1,000/day) and
email-link sign-in (five/day). Effective limits and abuse controls can change; inspect the
selected project before activation and include resend costs. Quota is not a promise of
successful inbox delivery or completed registrations.
[Official limits](https://firebase.google.com/docs/auth/limits).

## Independent acceptance sequence

1. Review exact contracts and the proposed decision. Run the isolated prototype with
   `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo test --locked -p cantos-api --lib theatre_registration::firebase_prototype`.
   This uses local HTTP doubles, no Firebase account/CLI/SDK/mail. Final repository handoff
   uses `cargo xtask preflight --changed-only`; retain every actual gate status.
2. After design acceptance, implement the bounded provider mode and durable PostgreSQL
   claims/unique bindings/expiry/quotas/retention, contracts/composition and Web continuation.
   Run `cargo xtask verify postgres` for actual disposable migration/race/restart/lock-wait
   checks; never use an inherited/live database.
3. With separately supplied authorized tooling, run real Authentication Emulator bootstrap
   and action tests. It delivers no real mail and emits unsigned tokens. Production must
   reject emulator configuration/unsigned evidence. No emulator integration is supplied here.
   [Emulator guide](https://firebase.google.com/docs/emulator-suite/connect_auth).
4. If the accepted design uses ID tokens, independently add signed rotating-key verifier
   fixtures for issuer/audience/subject/time, disabled/revoked credentials and key/project
   mismatch. Ordinary verification does not check revocation or prove action possession.
5. Only after exact operator authorization: named recipient receives Firebase mail → explicit
   action → exact enrollment proof → approved Kanidm setup delivery → user-chosen credentials
   → genuine OIDC → API `/session` → reload/logout, plus direct Studio/Admin denial and deployed
   origin/BFF checks. Accounts/grants/keys/mail/templates/Hosting remain outside code approval.

No live environment is inspected or changed here. Hosting #199 does not establish authentication
or the same-origin BFF cookie contract. No wildcard credentialed CORS, broad cookie Domain,
hidden proxy service or browser-security bypass is selected.
