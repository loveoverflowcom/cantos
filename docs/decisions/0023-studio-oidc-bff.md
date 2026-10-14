# ADR 0023: A Studio-only OIDC BFF in the existing API

- Status: proposed for production adoption; implemented for the authorized bounded PR2 slice
- Decision owner: Cantos maintainers
- Related dependency: [PR92](https://github.com/loveoverflowcom/cantos/pull/92), exact published head `277e28c1ab3a06e3c2fae6093e3278bdece1a0bb`

## Context

Studio already has hashed PostgreSQL sessions and resource ownership. Internal external-key
mapping alone does not authenticate a credential. The requested sequence adds Studio OIDC before
separate Theatre Web/CMP identity and Admin authorization. Production provider/client/TLS gates
remain unrun; credentials and live bootstrap were not authorized in this implementation session.

## Decision

Use a separate confidential `cantos-studio-web-bff` client, Authorization Code + S256, a private
verified-identity witness and the existing backend/session authority. Keep the host restricted
to development/loopback; opt-in OIDC disables development-token login. Use actual `ring` P-256
crypto, exact issuer/audience/nonce/time rules, bounded same-origin discovery/JWKS refresh and
per-request authenticated access-token introspection. The [architecture](../architecture/studio-oidc.md)
records the complete threat model, session policy and recovery limits.

Migration0020 owns durable callback consume, replay receipts and narrowed issuance. The atomic
SQL finish reuses PR1's exact types, primary key and identical framed lock, instead of granting
the HTTP role the general operator registry's table privileges. This is a deliberate orchestration
choice: registration, session insertion and rotation must share one final transaction after the
private verifier witness. SQL retains current-state barriers; pure credential and expiry decisions
remain outside I/O. General linking stays with `IdentityService`/operator. No automatic legacy
account linking or resource grants are introduced.

`cargo xtask kanidm reconcile --dry-run` prints an operator plan only; `kanidm verify` explicitly
fetches read-only metadata. Neither installs tools, changes security permissions, starts Podman,
recovers admin, creates a secret or mutates provider configuration. The user owns live operator steps.

## Alternatives

- Copy VOT Gateway/admin/bootstrap: rejected; unrelated services/privilege and implicit credentials.
- Trust browser identity headers or mapping DTOs: rejected; no credential proof.
- Accept offline ID-token validity until local expiry: rejected for ongoing disable/revocation;
  introspection intentionally makes provider availability an authorization dependency.
- Give HTTP general identity/actor writes: rejected; narrowed atomic issuance is sufficient.
- Activate production based on mocked tests: rejected; deployment/provider gates remain closed.

## Consequences and validation

New actor bindings preserve existing ownership/FKs and immutable history. Studio session cookie
scope and exact host policy grant no Theatre/Admin permission; producer grants remain separate.
No audio, publication, AI invocation or billing behavior changes. Rollback stops opt-in OIDC and
retains the additive schema/receipts; stored OIDC sessions are refused with OIDC disabled.

Tests cover signed crypto, deterministic clocks, PKCE/redirect binding, mocked HTTP rotation,
callback failures/replay, actual PostgreSQL callback/rotation/disable locks and cookie/CSRF routes.
[Evidence](../evidence/studio-oidc.md) states actual results, baseline failures and all live gates.
Production requires operator-prepared Kanidm/TLS/client/grants, observed login/disable/rotation/
logout and browser behavior, reviewed bearer protection, retention/rate limits and pending
revocation recovery. Introspection is not atomic with a later remote disable event; provider SSO
logout is distinct from local/access-token logout. Revisit for PR3/PR4 and production deployment.
