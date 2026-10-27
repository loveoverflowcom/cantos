# Admin authorization and audit

**Current source: native policy/audit core, no composed production credential verifier.**
`platform/admin` exposes an `AdminVerifier` port; the former Kanidm/OIDC implementation is
retired. The production Server leaves `Store.admin` unset and Admin routes return unavailable.
Studio/Theatre sessions, Identity password facts, registry lookup and producer grants never
supply Admin authority. There is no Admin UI/login client, account disable/delete, role grant,
publication override or generic permission framework.

## Ownership and trust

[`platform/admin/domain.rs`](../../services/cantos-server/src/platform/admin/domain.rs) owns
closed roles, action decisions and current `RoleEvidence`. A verifier must authenticate a
credential for the dedicated Admin purpose and observe its role at verification time before
constructing that evidence. [`AdminService`](../../services/cantos-server/src/platform/admin/mod.rs)
coordinates the verifier and native adapter; HTTP parses syntax and projects typed errors.

The actor is the existing exact `(issuer, subject) → actors.id` registry binding, never an email,
username or trusted request header. Admin creates/links no identity. A future native verifier
needs its own reviewed credential/freshness contract; a configured origin or roster grants nothing.
The old issuer/JWKS/introspection setup is retained only in
[pinned provider-era history](https://github.com/loveoverflowcom/cantos/blob/2ee9360817a207eb89a8f05ee9ed42102d714229/docs/architecture/admin-authorization.md).

## Credential and role contract

| Current role | Capabilities | Revoke Admin access |
| --- | --- | --- |
| `auditor` | Allow | Deny |
| `identity_operator` | Allow | Allow |
| Missing, unknown, stale, revoked or inactive | Deny | Deny |

`map_groups` retains exact legacy group mappings as pure policy, not a configured provider.
No positive role cache exists. Admission and the final pre-commit check reject stale evidence
and reverse-clock observations. Remote verification happens outside SQL; its observation is
never atomic with local revocation. No current production verifier or live key/group acceptance
is established by synthetic test evidence.

## Routes and audit policy

The [Admin v1 contract](../../services/cantos-server/contracts/admin/access/v1/README.md) owns wire shapes/errors.

| Route | Effect |
| --- | --- |
| `GET /api/v1/admin/capabilities` | Current actor/role/source/deadline and committed audit ID |
| `POST /api/v1/admin/actors/{actor}/revoke-access` | Permanent local Admin block, operation receipt and allowed audit atomically |

Revoke requests carry one non-nil UUID operation ID. Exact current-authorized replay returns the
original receipt with a replay audit; changed targets under the same operation conflict.
Different operations on an already blocked actor retain distinct audit receipts. Unknown commit
acknowledgement is reconciled by the same operation, never a fresh key or blind replay.

Audit includes only bounded internal actor/target/action/outcome, trusted time and correlation ID;
no credential, provider subject/email, request body or private error text. Required audit failure
never permits success. An effect failure rolls back the effect/receipt/allowed audit, then tries
an unavailable audit; this can itself fail without authorizing the effect. Transport parsing,
origin rejection and disabled host configuration precede application decisions.

## Local and remote revocation boundaries

Immutable local Admin blocks affect all current/future Admin credentials for the actor. They
change no actor active flag, Studio/Theatre session, producer permission, resource right or release.
No unblock endpoint exists. The serialized writer orders local admission against revocation;
queued writes reread active/block facts, then recheck evidence time before commit. An earlier
admitted request may complete first. There is no remote-group/SQL atomicity or global SSO logout.

## Persistence and read inventory

Native migration 22 owns audit, operation and revocation rows in the single `server.db`.
Schema guards protect append-only records, including replacement/upsert attempts; application
checks and private process/file ownership replace PG role/function grants. Do not apply legacy
SECURITY DEFINER setup to Turso or rewrite retained migration checksums.

Actual native tests are [`admin_turso.rs`](../../services/cantos-server/tests/admin_turso.rs)
and [`platform/admin/turso_tests.rs`](../../services/cantos-server/src/platform/admin/turso_tests.rs),
mapped in the [retirement ledger](server-turso/remaining-pg-retirement.md). They use explicit
synthetic role observations; they do not qualify a real Admin verifier. Reads are indexed actor,
binding/block and operation lookups; actual load/latency remains unmeasured. Production activation
requires a separately reviewed and qualified verifier, not re-enabling the retired provider.
