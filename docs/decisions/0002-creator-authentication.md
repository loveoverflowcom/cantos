# ADR 0002 — Database-backed creator bearer credentials

- Status: proposed for review; selected for the script-revisions implementation.
- Date: 2026-10-09.
- Decision owner: Cantos maintainers.
- Related issue or PR: [#1 — Authenticated script persistence and contracts](https://github.com/loveoverflowcom/cantos/issues/1).
- Supersedes / superseded by: N/A.

## Context

The first Cantos Studio backend must identify a creator, isolate private scripts and
attribute every saved revision. There is no browser login flow or selected identity
provider. The slice needs real authentication without adding a paid service or an
account-registration product. [Business rules](../product/business-rules.md#rights-and-access)
permit one authorized creator to hold several roles, while requiring backend permission
checks and recorded actors.

## Decision

Use operator-provisioned, per-creator opaque bearer credentials backed by PostgreSQL.
An operator associates each credential with a stable creator ID using the server's
administrative CLI. Provisioning generates 32 bytes from the operating system's
cryptographic random generator and displays the lowercase hexadecimal credential once.
Only the SHA-256 digest of its exact 64-character text is persisted, together with the
creator ID, an explicit expiry and nullable revocation time. A credential is neither a
creator ID nor a shared environment secret.

The API accepts exactly one `Authorization: Bearer <credential>` header. The scheme
is case-insensitive; credential text is exactly 64 lowercase hexadecimal characters.
Query parameters, cookies and request bodies cannot supply identity. Each request
resolves the credential digest in PostgreSQL and requires an enabled creator, an
unexpired credential and no revocation. Missing, malformed, unknown, expired, revoked
and disabled credentials all return the same unauthenticated outcome. Database failure
fails closed and maps to a service error, never anonymous access.

The authentication module alone constructs `Actor`. It has a private ID field and no
deserializer or public constructor. Application operations authorize the actor against
the stored resource owner, including scripts, revisions, history and referenced private
source/rights records. Client-supplied actor or owner fields cannot establish permission.
The first slice supports owner access only; sharing and transferable ownership are
outside this decision. An inaccessible script is indistinguishable from a missing script
in API responses. Protected responses use `Cache-Control: no-store`.

The mutation transaction records the authenticated actor, server time, operation ID,
action, script and resulting revision, including the prior revision where applicable.
The history and revision content are append-only. An idempotency replay still requires
authentication and resource authorization; operation keys are scoped to actor and action.
Accepting a script is an editorial revision transition, not permission to produce or
publish it. Source and rights references remain attached to the exact revision.

Use HTTPS for all remote credential transport. The local development server binds to
loopback by default; remote deployment requires TLS termination and a protected backend
connection. Do not put bearer credentials in logs, URLs, committed fixtures, browser
storage or error messages. Revocation and creator disabling take effect on subsequent
authentication lookups; an already authenticated request may finish. There is no
cross-request credential cache.

The high-entropy random credential and one-way verifier follow
[OWASP session-token storage guidance](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#server-side-session-token-storage).
This choice is a bounded backend credential mechanism; it does not establish a browser
session or account-recovery policy.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| PostgreSQL-backed opaque credential | Small authentication boundary; immediate revocation on the next request; no external service | Operator provisioning; possession grants access until expiry/revocation; a lookup per request | Selected for this API slice |
| Self-issued JWT | Local signature verification | Adds signing-key management and stale-claim/revocation policy without a current consumer | Defer |
| OIDC and browser session | Established identity lifecycle and browser login | Requires provider and deployment configuration outside this slice | Revisit when Studio login is implemented |
| Local passwords | No external identity provider | Adds password hashing, login throttling, recovery and credential lifecycle | Defer |
| Claimed creator ID or shared bearer secret | Minimal configuration | Cannot securely distinguish creators or attribute independent actions | Reject |

## Consequences

- Product and user impact: an operator provisions each creator and securely transfers their
  credential. There is no public signup, password login, recovery, sharing or administrative
  permission bypass in the HTTP API.
- Compatibility and migrations: creators have stable IDs independent of credentials.
  Multiple credentials can identify one creator. A later login/session adapter can produce
  the same application actor without changing script ownership or history.
- Operations and storage: PostgreSQL holds credential digests, expiry and revocation state.
  Provision, rotate and revoke credentials through the administrative CLI; disable a creator
  to prevent all of their credentials from authenticating. Rotation creates a new credential
  and explicitly revokes the old one. A restart does not reset credentials or ownership.
- Implementation and recovery: private routes authenticate before resolving or returning
  private data. Resource authorization applies on every operation, including retries and
  revision/history lookup. SQL and internal errors remain server-side.
- Deferred questions: browser cookie sessions with CSRF protection, account recovery,
  MFA/OIDC, collaborative roles, public deployment rate limits and operator identity audit.

## Validation and revisit conditions

The authentication module includes deterministic tests for canonical credential syntax,
case-insensitive schemes and cross-creator owner checks. Real PostgreSQL API integration
tests must demonstrate valid authentication; unknown, expired, revoked and disabled
credentials; denial across every script/revision/history read and write; actor spoofing;
protected references; and persistence after backend restart. They must inspect durable
actor history and verify denied requests leave script state unchanged. Test execution
evidence belongs in the slice completion report; this ADR alone proves no runtime behavior.

Revisit before browser login, public self-service onboarding, delegated editing or remote
production deployment. Document the selected identity and session lifecycle, credential
storage in clients, CSRF rules if cookies are introduced, and rate limits at that boundary.
