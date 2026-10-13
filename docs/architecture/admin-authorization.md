# Admin authorization and audit

This bounded backend slice implements dedicated Admin credential verification, capability
inspection and revocation of **Admin access only**. It is default unavailable without explicit
operator configuration and database function grants. Kanidm/live client acceptance is not implied.
No Admin UI, login client, OAuth registration, account disable/delete, role grant, resource ACL,
publication override or generic permission framework is introduced.

## Ownership and trust

`platform/admin/domain` owns closed platform roles and action decisions, using immutable exact
identity values. `credential` owns a private signed-credential witness; `provider` verifies the
operator-pinned issuer's HTTP metadata/keys and observes current credentials/groups. `AdminService`
composes these with the PostgreSQL adapter. HTTP extracts raw syntax and projects typed errors;
it executes neither SQL nor role decisions. Only signature plumbing is shared with Studio/Theatre.

The internal actor is the **existing** registry binding `(issuer, subject) → actors.id`. Admin
never creates or links an account, matches an email/username or accepts identity headers. A new
Admin issuer/client's pairwise subject needs explicit operator binding before use. Actor syntax,
registry lookup, Studio/Theatre sessions and producer grants confer no Admin authority.

Reference audit: VOT Workspace origin/develop `f3fea641a5fd226b883c4e32224ad8c97b8323f0`,
Console proof and `vot-directory` identity/registry types. Its proof-before-role and stable internal
identity principles are retained; no Gateway, UUID-sub assumption, directory framework or trusted
client header is copied. This source audit is not live-provider evidence.

## Credential and role contract

One exact operator issuer must end in `/oauth2/openid/cantos-admin-api`. The audience and
`client_id` are both the fixed `cantos-admin-api`; user-supplied values choose neither. Admission
requires an ES256 signature from the bounded issuer JWKS, unique matching `kid`, compatible
P-256 signing key, explicit `typ=at+jwt`, exact issuer/single audience/client, valid subject,
`iat <= now < min(exp, iat+300)` and nonfuture `nbf`. Unknown algorithms/keys, invalid signature,
missing/unknown role, expiry and provider outage fail closed. There is no ID-token or session
fallback. Tokens are used only for the request; no credential is stored in Admin tables or logs.

The dedicated `cantos_admin_groups` claim is an array with at most two allowlisted exact system
group values. Missing/empty or any unknown value refuses admission; generic `admin`, provider
`idm_admins`, email, username and arbitrary claims cannot grant a role.

| Verified issuer group | Platform role | Capabilities | Revoke Admin access |
| --- | --- | --- | --- |
| `cantos_system_auditors` | `auditor` | Allow | Deny |
| `cantos_identity_operators` | `identity_operator` | Allow | Allow |
| Both | `identity_operator` | Allow | Allow |
| Missing, unknown, stale, revoked or inactive | None | Deny | Deny |

Authenticated introspection is required on **every** request, with exact active/sub/issuer/audience/
client/expiry and the current `cantos_admin_groups` claim. Its mapped role must match the signed
role; group removal/demotion refuses the request. The resulting witness expires at the earliest
JWT/access expiry or **five seconds after observation**, and is checked against PostgreSQL's fresh
clock after admission locks and again after all SQL waits immediately before commit. A reverse clock jump also refuses it. No positive role cache exists.
Metadata/JWKS has a 300-second maximum cache, no expired-cache fallback; loading retries are
bounded to one per ten seconds. New keys fail closed until the next fresh metadata load.

[Kanidm's OAuth documentation](https://kanidm.github.io/kanidm/stable/integrations/oauth2.html)
describes separate clients, ES256 and JWT access tokens/introspection, but does **not** establish
this custom claim's current-role semantics. That exact provider contract is an unresolved live
prerequisite. A default Kanidm deployment lacking it is unavailable/denied; this PR creates no
client, group, secret, claim map or live right. The narrow access-token shape uses the purpose
separation in [RFC 9068](https://www.rfc-editor.org/rfc/rfc9068.html); full RFC interoperability
(including algorithm negotiation) is not claimed. A future authorized live verifier must prove
fresh group removal and key rotation before activation, rather than replaying token-time groups.

## Routes and audit policy

| Route | Purpose | Decision/audit policy |
| --- | --- | --- |
| `GET /api/v1/admin/capabilities` | Read current actor, role, source, admission expiry | Verify current proof and local policy; required `allowed` audit before success |
| `POST /api/v1/admin/actors/{actor}/revoke-access` | Append a local block on target actor's Admin access | Identity operator only; existing target; atomic block + operation receipt + required `allowed` audit |

POST requires `{ "operation_id": "<non-nil UUID>" }`. Exact actor+operation replay returns the
original receipt after **current** authorization and appends a `replayed` decision. Same operation
with a different target is a `409 operation_reused` with `conflict` audit. Separate UUIDs targeting
an already blocked actor have no further privilege effect but get distinct receipts/audits.
Bearer replay within its short validity is ordinary request authentication, not a one-shot grant.

Every decision reaching the Admin application requires audit: unauthenticated/unmapped actor is
NULL; a mapped local-policy denial records its internal actor. Targets are bounded ActorIds,
actions/outcomes are closed values, and timestamps/correlation UUIDs are server supplied. Audit
contains no credential, digest, subject, issuer, email, username, body, provider error or extra PII.
Malformed transport/JSON, unsupported routes/methods, origin middleware rejection, disabled host
configuration and cancellation before a decision are outside this decision ledger. They perform
no Admin mutation. No success is returned when required audit cannot commit: safe `503 unavailable`.

Audit is append-only: constraints, no runtime table grants and UPDATE/DELETE/TRUNCATE rejection
triggers. Audit insertion and mutation share one transaction. An effect error rolls back any block,
receipt and successful audit, then attempts a separate `unavailable` audit; failure stays `503`.
A crash/cancel before commit rolls back; lost commit acknowledgement returns unavailable and the
same operation recovers the original receipt. Audit outage may mean no durable deny record, but
never authorizes an effect. No exactly-once HTTP or external-generation claim is made.

## Local and remote revocation boundaries

Permanent local Admin blocks are immutable records; there is no unblock or grant endpoint.
Restoration is a future reviewed recovery capability, not deleting evidence. Blocks apply to all
current/future Admin tokens for the actor and leave actor active state, Studio/Theatre sessions,
producer grants, resource rights and historical audio/publication unchanged.

The adapter takes sorted actor/target SHARE locks plus a namespaced advisory lock per actor,
reads current active/block facts and time after waits, decides, then commits. Local disable and
Admin revocation serialize with admitted requests; a request admitted before revocation may
finish first. Concurrent duplicate operations for an actor serialize. Locks/timeouts are bounded
by the existing 5s statement / 3s lock / 15s HTTP deadlines. Provider I/O is outside SQL locks.

Remote group removal is an **introspection observation**, not atomic with PostgreSQL. Removal
before observation refuses; removal after observation can affect an already admitted request for
at most the five-second witness window until the final decision check (also bounded by token
expiry). Waiting beyond that window rolls back speculative effects and records denial. An admitted
request may finish its commit acknowledgement/response after that check; response completion is
not bounded to five seconds. Operators needing an immediate local fence use the Admin block. Do not promise atomic
remote-group/SQL revocation or global SSO logout. A platform role never enters Studio, owner or
resource ACL policy; this endpoint changes only the separate Admin block table.

## Persistence and read inventory

Additive migration `0022_admin_authorization_audit.sql`; historical SQL/checksums remain unchanged.
Combined registry has1–14 plus16–22; independent Storage0015 remains reserved to its PR.
Runtime receives no grants automatically. Narrow SECURITY DEFINER functions have fixed
`pg_catalog` search paths and fully qualified tables; operator opts in only their EXECUTE grants.
The application/verifier remains the credential trust boundary, not arbitrary SQL callers.

Reads are existing exact identity+actor point lookup, actor/block admission and actor+operation
receipt lookup. PKs cover exact filters; actions produce no list, private-resource join or aggregate.
Frequency/production load is unknown. Target is correctness under the stated deadlines; no latency
SLO or performance gain is claimed. DBSP is rejected for this indexed point-lookup/transaction
workload: asynchronous rights would weaken current admission. Synthetic PG plans/timings are
recorded in [evidence](../evidence/admin-authorization.md); performance/load acceptance remains
separate. See [wire contract](../../packages/contracts/admin-v1.md) and
[combined-review handoff](../work-plan/admin-authorization-handoff.md).
