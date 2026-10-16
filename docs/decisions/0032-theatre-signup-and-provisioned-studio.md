# ADR 0032: Theatre self-registration and provisioned Studio administrators

- Status: accepted product policy; issuer capability and live activation remain separate
- Date: 2026-10-16
- Decision owner: Cantos product owner, through the explicit user decision
- Related issue or PR: [#174](https://github.com/loveoverflowcom/cantos/issues/174), [#191](https://github.com/loveoverflowcom/cantos/issues/191), [Draft PR #196](https://github.com/loveoverflowcom/cantos/pull/196)
- Supersedes / superseded by: supersedes the open self-signup/invitation/defer product choice in #174; preserves [ADR 0015](0015-api-surface-boundaries.md) and [ADR 0024](0024-studio-oidc-bff.md)

## Context

The product owner selected public self-registration for Cantos listeners and only one or two
securely provisioned administrator accounts for Cantos Studio. Earlier #174/#191 text left the
enrollment policy open. That choice is now settled; support in the selected issuer is a separate
technical requirement, and a registration screen or working existing-account login proves neither
account creation nor Studio authorization.

The existing application has separate Studio, Theatre Web, Theatre native and Admin clients,
credentials and request admission. An internal actor mapping is shared identity metadata, not
permission to cross those boundaries. This change continues Theatre Web Leptos first; native
implementation and OS acceptance retain their existing scope.

## Decision

1. **Theatre supports listener self-registration.** Registration must use a verified issuer
   capability and its accepted return contract. Keep public discovery available to guests.
   Cantos does not collect or store a new account password, build a second credential database,
   guess a provider enrollment URL or report success from a local fixture. A return URL alone
   is not account proof; authenticated presentation requires the existing server-confirmed
   Theatre session and its state, nonce, PKCE, cancellation and account fences.
2. **Studio has no public registration.** Only one or two administrator accounts provisioned
   through an explicitly authorized operator action may enter Studio. Provisioning is an
   identity-provider/account operation plus a separate server admission configuration; it is
   never a password/token compiled into the app, a UI-only role or a shared Theatre session.
   Existing Studio ownership and action authorization continue to apply after admission.
3. **Studio admission is enforced by the server.** The bounded Studio configuration identifies
   one or two exact subjects under the pinned Studio issuer. It is checked before callback
   registry/session issuance and on every protected request. Subject/email/name supplied by a
   client, a new Theatre account, or an actor already in the registry cannot add itself to that
   configuration. Removing admission must refuse subsequent Studio requests. See the
   [Studio boundary](../architecture/studio-oidc.md) for the implemented configuration and tests.
4. **Identity purposes remain separate.** A Studio administrator here means an admitted Studio
   operator; it does not confer the dedicated `/api/v1/admin/*` capabilities. Theatre signup
   grants neither Studio/Admin admission nor producer grants, paid membership or implicit
   guest-data import. Exact issuer/subject linking retains its separate authorized procedure.
5. **Live provisioning requires a concrete authorized operation.** Before creating a real
   account, credential/key or standing grant, identify the environment, one or two intended
   account identities and exact authority, then obtain the user's specific authorization.
   Deliver enrollment/recovery material through the provider or an approved secure handoff;
   do not request passwords in chat or reuse credentials from conversation history. Code and
   isolated test fixtures do not perform that operation.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Theatre self-registration; provisioned Studio operators | Open listener onboarding with a small, explicit editorial authority | Requires an actual issuer enrollment capability and separate Studio admission | Selected by the product owner |
| Theatre invitation-only or indefinitely deferred signup | Avoids an unsupported enrollment adapter | Does not deliver the selected public listener journey | Not the accepted product policy; an unavailable state may only describe the current blocker |
| Public Studio signup or automatic admission after Theatre signup | Fewer operator steps | Expands editorial/private-content authority beyond the selected accounts | Rejected |
| New Cantos password store or hardcoded administrator credential | Would bypass the current issuer limitation | Adds a different credential/security boundary without approval | Rejected |

## Consequences

- Product and user impact: listeners can create their own accounts once the issuer capability
  is implemented and accepted; Studio remains login-only for its provisioned operators.
- Compatibility and migrations: preserve existing identity, cookie/session, ownership and
  immutable migration contracts. This ADR does not authorize an account merge, provider switch
  or credential-format change. Development-token tooling remains local development tooling,
  not the production Studio onboarding path.
- Operations, storage, and production cost: no live account, client, grant, secret, key, TLS,
  network or production activation is created by this decision record.
- Implementation and recovery requirements: keep unavailable capability explicit and typed;
  retain existing login/logout/unknown-outcome recovery and prevent Theatre-to-Studio escalation.
- Deferred questions: the actual enrollment-capable issuer mechanism and return/verification
  semantics; selected real environments/accounts and their secure provisioning; authentication
  factors/recovery and production operational acceptance. These are not a reopening of the
  self-registration product choice.

## Validation and revisit conditions

The repository pins Kanidm 1.10.5. Its [UI router](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/views/mod.rs)
does not expose public signup. Its [`/ui/enrol` handler](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/views/enrol.rs)
requires an authenticated user and prepares that user's credential-update intent; its
[`POST /v1/person`](https://github.com/kanidm/kanidm/blob/v1.10.5/server/core/src/https/v1.rs)
requires authenticated client information. Source review therefore does not establish a usable
public enrollment mechanism. No live issuer configuration was inspected or changed by this ADR.

The [Theatre contract](../../packages/contracts/theatre-identity-v1.md) distinguishes the accepted
policy from current capability. Missing identity configuration and unsupported provider signup
remain unavailable; a supported route may be added only with an audited mechanism and real
return/security semantics. Existing-account login can be accepted independently. A real issuer,
client and authorized test account are required for live login; actual self-registration needs
separate enrollment evidence. Local signed-provider/browser fixtures must remain labeled.

Validate Studio with allowed and denied signed identities, denied Theatre credentials, denied
unlisted subjects, post-session admission removal and direct private API requests. Validate the
Web guest/login/register/cancel/retry/history/reload/logout flow independently from native and
provider acceptance. Keep Draft PR #196 while its required acceptance is incomplete. Revisit this
ADR only when the product owner changes the audience/access policy; report an issuer limitation
as an implementation blocker rather than silently changing that policy.
