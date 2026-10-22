# Cantos Bearer v1 — shared lifecycle and native boundary

Current first-party sign-in/session routes and credential syntax belong to
[Identity auth v1](identity-auth-v1.md): **Web cookie, CMP/native opaque Bearer**. The `ci1_`
Identity session family replaced provider-bound `ca1_`/`cr1_` issuance. Provider OIDC/PKCE,
callback and two-leg exchange routes are removed, never an alternate supported login workflow.
Browser/device and production HTTP acceptance remain separate.

## Authority and wire

Opaque secrets are random 256-bit values, stored only as digests. Prefixes are routing tags,
never authority. Identity proves authentication over private RPC; the Server binds exact subject,
ActorId, purpose/channel and session, checking current security state and resource permission.
No email-based linking, cross-purpose cookie/token reuse or client-selected actor/role/issuer.

[Identity v1](identity-auth-v1.md#endpoints) owns exact JSON routes, session fences, cookie names,
CSRF/origin rules and errors. Existing [Rust DTOs](api/src/bearer.rs),
[response schema](schemas/bearer-v1.schema.json) and [synthetic fixtures](fixtures/bearer/v1/token-pair.json)
remain compatibility inputs; their provider-era example prefixes do not authorize those removed
credentials. No wire schema is changed by this documentation cleanup.

## Routes and two code legs

The old provider flow/callback/exchange legs are retired.
[Original protocol](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/packages/contracts/bearer-v1.md#routes-and-two-code-legs) preserves
historical API/PKCE decisions, never current commands. Native login/refresh/revoke and Web cookie
renew/logout use the separate Identity v1 routes. Registration/proof confirmation grants no
Studio/Admin/resource authority; password authentication alone does not select a Studio roster.

## Lifetime, rotation and unknown outcomes

Reuse the immutable `SessionPolicy` and pure rotation/replay decisions in
`services/cantos-server/src/platform/bearer/core.rs`; do not create another session engine.
Current exact profiles are [Identity v1 lifetimes](identity-auth-v1.md#credentials):
access at most 120 s; Studio idle 300 s / absolute 600 s; Theatre idle 600 s / absolute 900 s.
`0 < access <= idle <= absolute` is checked, and requests cannot override profiles. A refresh
never changes family/actor, original authentication time or absolute deadline.

Native refresh consumes its secret once and commits replacement atomically. Replaying a spent
refresh revokes the family including a concurrent winner. Single-flight per family is required;
there is no recoverable plaintext token receipt. An uncertain refresh or secure-store replacement
requires hiding credentials, cleanup with the exact old refresh/session, then fresh login; never
replay a spent/unknown refresh or infer rollback from timeout/unavailable. Business operation IDs
and receipts remain separate. Web renewal instead extends the same cookie's idle deadline within
its fixed absolute cap, using the dispatch snapshot/session fence.

Logout/revocation is local and permanent; no removed provider revocation header or background
provider retry is implied. Calls never hold a database writer across private RPC/external I/O.
Keep late authority/admission rechecks and exact cookie/native credential separation.

## Fresh authentication and account identity

Identity's verified fact supplies authentication time; token issue/refresh never substitutes a
fresh proof. Studio roster and recent-auth/resource policy are separate current checks. The exact
freshness/version/epoch/outage rules stay in [Identity v1](identity-auth-v1.md#freshness-and-revocation-quantified).
An unavailable Identity fails closed without inventing revocation or successful work. Exact
immutable subject binding preserves actor/ownership history; no email merge or automatic linking.

## Migration and client handoff

Web uses the shared cookie owner with exact Origin/CSRF and no Bearer fallback. Native retains
the secure journal, single-flight refresh, persist-before-admission, unknown-outcome cleanup,
restart confirmation and late actor/intent fencing. The executable
[bearer lifecycle model](../../specs/behavior/inventory.md#cantos-bearer-lifecycle-253) remains draft
with all properties/controls; model PASS never proves native storage, process or device acceptance.
Historical provider credentials/setup/callback records are not migrated by restoring old routes.

## Persistence, read cost and gates

Native sessions use the Server's one embedded Turso owner, not PostgreSQL/provider state or a
second auth-only DB. [Foundation mapping](../../docs/architecture/server-turso-schema.md#identity-session-mapping)
records constraints, transaction/replay and remaining HTTP/restore gaps. Retained PG test sources
and the [original evidence](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/packages/contracts/bearer-v1.md#persistence-read-cost-and-gates)
are historical porting oracles. `verify postgres` is retired; native acceptance is scoped real-
file/RPC/HTTP evidence through current Rust/Turso owners. No cache or performance gain is claimed.
