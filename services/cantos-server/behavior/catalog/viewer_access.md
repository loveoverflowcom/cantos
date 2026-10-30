# Viewer access: bounded S06 expectation

Lifecycle: draft. Authority: [ADR 0054](../../../../docs/decisions/0054-publication-generation-and-access.md) D04/D05.

| Approved requirement | Implementation fact | Finite assumption | Unmodeled / next owner |
| --- | --- | --- | --- |
| Work AND Unit; INHERIT changes audience only | Pure Catalog evaluator; S06 does not route restricted reads | One Work/Unit, current rights/owner summarized in gates | S07 route/bytes composition; S10 Unit policy UI |
| Claim binds a stable actor and mailbox reuse cannot transfer it | Identity own-store fact and Server immutable claim binding | Two actors, one verified mailbox represented as an external fact | Syntax, crypto, caller authentication, schema, races and restart require native tests |
| Revocation/replay cannot resurrect access | Immutable receipts and revoke tombstones | One grant; no later owner-authorized new grant | Cross-file disable/restore and production acceptance remain independent |
| Flat groups remain owner scoped | Typed group + owner, membership expiry/removal | One group, membership and same-owner flags | Bounded listing and actual foreign keys require Turso tests |

The manifest pairs each property with a named run, reachable witness and typechecking negative
control. Model results are sampled expectations, not implementation conformance or Kani evidence.
The existing native mailbox proof capability remains unchanged: S06 reads proof-created email
rows and introduces no new proof-consumption transition. The new RPC fact validator has ordinary
binding/freshness tests; no formal whole-auth or I/O claim is made.
