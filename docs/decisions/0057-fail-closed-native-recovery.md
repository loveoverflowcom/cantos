# ADR 0057: Fail-closed native recovery

- Status: accepted recovery policy; implementation and qualification pending
- Date: 2026-11-03
- Decision owner: project owner, conversation authorization `Sentinel_d48a035a20808191b77a16be99fd5eeb`
- Related: [#658](https://github.com/loveoverflowcom/cantos/issues/658), [#676](https://github.com/loveoverflowcom/cantos/issues/676)
- Extends ADR 0054 D08d and the native Turso restore invariant; no authentication-factor change

## Context

Exact-image run [41545871906](https://github.com/loveoverflowcom/cantos/actions/runs/41545871906)
restored a pre-logout Server/Identity pair and admitted the revoked session (200, expected 401).
The epoch, security version and Server revocation bit all rolled back together. Migration cannot
recover facts absent from both files. Epoch rotation alone does not preserve disabled accounts.

## Decision

The owner accepted this policy: after restore, keep access closed until reconciliation against
current security authority retained outside the restored database pair. Missing authority keeps
access closed; never reset credentials, re-enable accounts or infer that lost revocations vanished.
This acceptance authorizes specification/implementation/tests, not qualification or deployment.

The bounded implementation uses the existing single Turso owner and its serialized writer:

1. Each enrolled owner stores an instance/revision marker in its own database and a private
   durable fence outside all database snapshot/restore roots. The owner holds both file leases.
   The OS owner/operator is trusted; a hostile root or rollback of the entire host/fence is
   outside this protocol. No service reads the other service's database.
2. Before each write transaction, publish/fsync a pending fence. Commit the next revision with
   the transaction, then publish/fsync the committed fence before reporting success. Reads and
   reopen refuse a missing, untrusted, malformed, pending or mismatching fence. Explicit known
   rollback may clear its own pending marker. In the same owner lifetime, a cancellation with
   no COMMIT attempt may also be settled after acknowledged rollback and a previous-marker
   check. Process loss destroys that in-memory evidence; unknown COMMIT never uses this path.
3. This fence is a freshness floor, not a journal. It contains no passwords, session tokens,
   account lists or permission data. It cannot reconstruct absent security facts.
4. The first supported reconciliation source is a complete **current** private pair, captured
   with both owners stopped and whose markers match the independently retained committed
   fences. Its full current state preserves logout/refresh, disabled accounts, grants, group
   membership and publication policy without an unreviewed table-by-table security merge.
   Older or mixed sources are refused. Backup-only recovery after losing this current source
   remains unsupported and closed; a historical hash/receipt is not a freshness proof.
5. A supported recovery operation preserves original bytes, records durable intent outside
   restore roots, operates on fresh destinations, explicitly rotates Identity epoch and
   durably retires old session/refresh families. Reopening requires both owners reconciled and
   policy checks complete. Lost replies, partial progress and restart cannot silently reopen.
   Restricted delivery remains disabled pending its separate owner confirmation/qualification.

Ordinary restart does not rotate Identity epoch or revoke valid sessions. Existing admission,
TTL, replay, purpose, ActorId and roster semantics remain. Initial fence enrollment is an
explicit stopped-owner operation, never a silent Serve/Migrate side effect. An enrolled file
cannot be reopened without its fence by omitting configuration. Registered SQL is never edited.

## Alternatives

| Option | Decision |
| --- | --- |
| Rotate an epoch only inside the restored pair | Insufficient: loses post-cut disabled-account and policy facts |
| Trust an old manifest, timestamp or watermark restored with the files | Insufficient: it rolls back with the facts it purports to prove |
| Independent full security event journal | Deferred: not necessary for the bounded current-authority recovery lane; requires its own acknowledgement, retention and catch-up protocol |
| Weaken the restore oracle or activate the failed candidate | Rejected: violates existing non-revival requirements |

## Consequences and validation

Every fenced mutation adds durable local filesystem work. Measure overhead and retain explicit
unknown outcomes; availability is sacrificed when freshness cannot be established. Existing
private storage is reused; no new service, paid provider, credential or access grant is implicit.
Production directory ownership/mount provisioning is a separate concrete operator action.

Required evidence includes real Turso transactions, stale/mixed pair refusal, logout and refresh
revocation, disabled account login, missing/old/corrupt/untrusted source, fence loss, crash at
each durability boundary, exact retry and restart ordering. A positive unaffected login after
completed reconciliation must accompany the negative tests. A blanket outage is not recovery
acceptance. Preserve exact-image, host executor and assurance gates; keep NO_GO until they pass.
