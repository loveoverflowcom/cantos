# Viewer access: bounded S06 expectation

Lifecycle: draft. Authority: [ADR 0054](../../../../docs/decisions/0054-publication-generation-and-access.md) D04/D05.

| Approved requirement | Implementation fact | Finite assumption | Unmodeled / next owner |
| --- | --- | --- | --- |
| Work AND Unit; INHERIT changes audience only | Pure Catalog evaluator and common S07/S10 serving admission | One Work/Unit, current rights/owner summarized in gates | Native DB/router/bytes and client evidence remain separate |
| Claim binds a stable actor and mailbox reuse cannot transfer it | Identity own-store fact and Server immutable claim binding | Two actors, one verified mailbox represented as an external fact | Syntax, crypto, caller authentication, schema, races and restart require native tests |
| Revocation/replay cannot resurrect access | Immutable receipts and revoke tombstones | One grant; no later owner-authorized new grant | Cross-file disable/restore and production acceptance remain independent |
| Flat groups remain owner scoped | Typed group + owner, membership expiry/removal | One group, membership and same-owner flags | Bounded listing and actual foreign keys require Turso tests |

The manifest pairs each property with a named run, reachable witness and typechecking negative
control. Model results are sampled expectations, not implementation conformance or Kani evidence.
The existing native mailbox proof capability remains unchanged: S06 reads proof-created email
rows and introduces no new proof-consumption transition. The new RPC fact validator has ordinary
binding/freshness tests; no formal whole-auth or I/O claim is made.


S07 shared discovery supplies current actor/group facts to this same Work AND Unit evaluator
before projection and byte admission. S10 persists INHERIT or restriction within parent and applies it at the common serving seam.
Real file-backed Turso tests bind independent Work/Unit actor/group intersection, replacement
CAS/replay/reopen, expiry, owner isolation, pre-page HIDE and Library behavior. The real Identity
HTTP journey tests old public and wrong-actor private GET/HEAD/Range/conditional denials. The principal-first candidate
indexes are not authority. Opaque cursors and permits also carry the process/session and current
publication/policy/group/expiry/lifecycle version fence; native integration tests exercise these boundaries.
No new domain transition, claim rule or lifecycle is selected, so this model remains unchanged.
No model or synthetic-shell result establishes production HTTP or database conformance.

S10 keeps the same approved Work AND Unit transition rather than creating per-client models.
The draft model's finite flags do not model normalized SQL, coherent multi-entry generation CAS,
client invalid-cursor purge or decoder/restore behavior. The metadata-generation capability is
still draft/model-null and listening.progress still needs-decision/model-null. Client lifecycle
has no registered conformance observer. Those are unaccepted assurance gaps, not waived by the
unchanged sampled model or native tests, and are carried to #676/#658.
