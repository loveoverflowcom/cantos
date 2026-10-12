# External submission durability (replaces backend synthesis jobs)

> [ADR 0018](../../../../docs/decisions/0018-external-audio-production.md) retires
> server-side TTS leases, worker claims and vendor reservation state. No Cantos synthesis
> worker exists. This reference now defines the **upload-intent lifecycle** for future code.

## Proposed state machine — not implemented

```text
pending_upload -> uploaded_unverified -> accepted_private -> pending_review
                 \-> rejected                 \-> approved -> published
       \-> abandoned           \-> rejected
```

Represent states as typed enums with closed transitions in a pure domain, validated by
the committing PostgreSQL transaction. Bind intent to actor, work, content unit,
brief digest, rendition profile and idempotency key. Never use a generic job state whose
meaning could be confused with a TTS invocation.

## Recovery invariants

- Prepare retries with the same key and exact scope produce the same intent. Conflicting
  scope/key requests fail with a typed conflict.
- A private object uploaded before finalize is not an accepted rendition.
- Finalize requires server-observed object bytes/digest/size and an authorized creator/
  producer scope; a caller string is never sufficient.
- The response may be lost after commit; retry must replay the immutable receipt.
- Object store and PostgreSQL transactions do not share an atomic commit; explicitly
  retry/reconcile orphaned private objects with limited lifetime.
- Revocation, stale brief, object corruption, failed human review or stale rights cannot
  result in public publication.
- A newly approved release cannot mutate or delete the previous published release.

## Tests

Run adverse sequencing, concurrent duplicate finalize, expired grants, loss after object PUT,
loss after DB commit, wrong-object substitution, cross-actor access and failed publication
replacement against real PostgreSQL plus a controlled object store. Mock external producer
execution; **never implement it inside the Cantos API**.
