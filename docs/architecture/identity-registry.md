# Actor identity registry

The Server owns the provider-neutral actor registry under
`services/cantos-server/src/platform/identity`: pure values/decisions, `IdentityRegistry`,
`IdentityService` and `TursoIdentityRegistry`. It uses the same `server.db` serialized owner as
native sessions. Registry lookup/linking never authenticates or creates resource authority;
[Identity v1](../../packages/contracts/identity-auth-v1.md) owns fresh authentication facts and
session issuance over private RPC. No Server read of `identity.db` or cross-DB transaction.

## Identity contract

| Value / operation | Rule |
| --- | --- |
| ActorId | Preserve opaque existing IDs, ownership and history; validate constructor/row spelling |
| External identity key | Exact case-sensitive `(issuer,subject)`, never email/username matching or normalization; validated bounds/control refusal are metadata, never proof |
| Registration | Atomically bind active actor + key, exact-key replay returns winner; inactive binding or occupied candidate refuses with no losing actor |
| Linking | Explicit active target only; same pair idempotent, foreign owner refuses; no transfer/merge/unlink |
| Lookup | Optional active actor snapshot grants no session, role or authorization |
| Errors | Typed rejection, unavailable including unknown commit acknowledgement, corrupt-row refusal; no leaked SQL/secrets |

Constructors and serde/SQL hydration share validation. Context table guards keep registry SQL in
its owner; a business context cannot read the table directly. Native issuance composes the
transaction-aware registry seam inside its write unit. Conditional writes, current active-state
rechecks and immutable binding/receipt guards remain required.

## Persistence and read boundary

The [foundation mapping](server-turso-schema.md#invariant-mapping) records exact constraints,
serialized ordering, race/replay/reopen and unknown-ack tests. One `BEGIN IMMEDIATE` replaces PG
advisory/share-lock mechanics; a lost acknowledgement is unavailable and reconciles by exact key.
The fixture/engine result is distinct from production HTTP/browser/restore acceptance.

[Original PG registry design](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/architecture/identity-registry.md) and
[source-bound evidence](../evidence/identity-registry.md) retain the narrow grants, Unicode bounds,
query plan and 20 warm synthetic lookup measurement. These are historical, not current Turso
performance. [#384](https://github.com/loveoverflowcom/cantos/issues/384) requires native SQL/index
measurement before selective cache; incremental engines DEFER.

## Reference audit and successor boundaries

The original VOT/Cantos comparison informed exact-key metadata, typed proof and admin separation;
it did not authorize copied gateway/provider framework or live bootstrap. Provider OIDC adapters
are retired. Preserve account-control and privileged-intent proof for any future operator linking,
never match email. Provisioning/recovery, Admin verifier, native HTTP, migration/restore and
browser/device acceptance remain separate #334/#335/#336 work. No data/account/service deletion
follows from source retirement.
