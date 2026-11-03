# v27 → v36 forward qualification

Status: source, initial-host executor and disposable qualification tooling. The new executor
has no Linux/systemd acceptance until its separate attributed rehearsal actually passes.
No host activation, admission or recovery acceptance is implied. The independent [same-schema](native-oci-release.md) and
[v24-to-v27 candidate](native-forward-v27.md) retain their original bounds.

## Existing SQL versus release policy

The historical suffix 28–35 remains byte-for-byte registered. Server v36 appends owner recovery
metadata; Identity v5–v6 append that metadata and the explicit restore-epoch receipt/guard.
Identity v7 preserves proof epoch history: it replaces the proof workflow's FK to the mutable
current epoch with an immutable epoch ledger, copying the dependent signup/provider graph
inside one owner migration. Old proofs/receipts retain their original bindings; new workflows
must use the current epoch. Registered v1–v6 SQL is unchanged. A populated v6 fixture checks
all retained records, disabled state, migration replay and rotation; DDL conflicts must roll
back every new copy and leave the v6 prefix servable.
The initial v4 baseline is unenrolled. An already-enrolled older registry remains refused
unchanged by the owner's existing startup guard; this slice adds no fenced-schema-upgrade
executor and never drops/recreates its authority to get past that refusal.
The declaration matches those actual registries, with the new fenced security-state contract;
the producer's mismatch guard remains unchanged. The checked-in
v27 image baseline is the observed source `89c5964c89cdbb0c50397f7d1263af34a5c3e517` and exact
public OCI/configuration digests, not an independently admitted old release or a live DB query.

Real-engine tests construct the exact v27 registry prefix. The test-only checksum pin is
derived from that historical source and checks all 27 version/name/SQL identities before
using the prefix. The populated fixture covers all pre-upgrade table values, opaque Unicode
and BLOB records, Work/member/operation relationships, PNG artwork and disclosure history,
active/disabled actors and live/revoked sessions. It checks the v36 suffix, repeated migration,
integrity, immutable/revocation guards, refused pending serving and refused old-image serving.
Separate DDL conflicts check both an unchanged v27 refusal and a partially committed v31
prefix: failure must never claim v36 or permit restarting the v27 binary on changed data.
These tests use actual Turso, not SQLite; executing them is separate from their source.

## Exact-image forward rehearsal

On a disposable native Linux builder with the matching public images and pinned source:

```sh
cargo xtask release native-rehearse \
  --transition v27-v36 \
  --manifest PRIVATE_MANIFEST --manifest-digest SHA256 \
  --source-sha FULL_SHA --configuration-digest SHA256 --platform linux/amd64 \
  --baseline infra/oci/historical-v27.json --baseline-digest SHA256 \
  --output FRESH_OUTPUT_DIRECTORY
```

The source-pinned command uses the existing isolated OCI fixture and HTTP mailbox/signup
driver. It creates data with the old exact pair, verifies a pre-cut revoked cookie, stops both
owners, copies the complete current pair, verifies that new serving/check refuses pending
migrations without modifying bytes, migrates only disposable copies twice, checks both owners,
explicitly enrolls the independent floors, and observes preserved authentication and post-upgrade
revocation after restart. Ordinary migration never silently enrolls or rotates an epoch. The real
Podman process/private-bridge control remains separate from the four ordinary OCI gates.

The report always retains `activation`, `admission`, `executor_transition`, `live_readiness`
and `historical_snapshot_restore` as `NOT_RUN`; recovery remains `NO_GO_REMAINS_SEPARATE`.
A file/runtime-double result is never Linux/exact-image evidence. Any failed runtime check
leaves FAIL and an empty completed-check list. Existing output cannot be overwritten.

The manual publisher selects this rehearsal for a v36 candidate and retains the v24-to-v27
lanes for v27 candidates. This independent step can collect forward-upgrade evidence after
a failed ordinary qualification step, but it neither clears that failure nor makes the job
successful. It does not run on the VPS or change production units/databases. Build/publish
credentials stay confined to the existing manual publisher; no new credentials are created.

## Recovery must not be inferred from a successful upgrade

The previous OCI restore oracle revoked a session after taking a backup, restored that older
pair and then required the restored session to work. That inverted the revocation property.
The corrected `migration-restore` gate requires the post-cut revoked session to remain denied.
Without a qualified reconciliation mechanism, a real fixture that revives it fails the gate;
it is never waived because both files are consistent or the migration passed.

The real-engine stale-copy counterexample shows the separate limit: applying every new DDL
to an older internally consistent directory cannot reconstruct a revocation absent at its cut.
A successful counterexample test demonstrates that unsafe state; it is not recovery PASS.
The separate [ADR0058 current-authority protocol](../decisions/0058-fail-closed-native-recovery.md)
adds the approved fail-closed recovery policy and owner primitives. Its exact-image recovery
matrix, host executor and the existing #676/#658 NO_GO remain independent of this forward
rehearsal. The session-admission kernel is unchanged; the new storage/recovery protocol is not
covered by its pure model. No old backup is restored into production.

## Production boundary and next required evidence

`cargo xtask release native-migrate-v36` is a separate initial-host lane; the old
`native-migrate-v27` still refuses v27-to-v36. Before activation there must be an
operator-reviewed current inventory/schema check, passing exact target-pair gates, a reviewed
v27-to-v36 host executor with failure/replay and forward-recovery evidence, an ingress/write
fence and fresh source/image/configuration-bound operator inputs. No image-only rollback may
serve a partially or fully migrated file. Never restore an old snapshot after accepted writes.

The new Server also consumes additive Identity mailbox RPC methods absent from the old
running source. Identity schema v4 alone proves no RPC/image-pair compatibility. Updating that
image requires its own authorized scope and exact-pair tests; never infer it from API deploy
permission. Keep existing Studio roster, data paths, peer policy and restricted-serving locks.

Closing the exposure barrier before stopping a running pair can make graceful shutdown retain
systemd's `failed` state. The initial v36 adapter accepts that state as stopped only after both
named containers are absent and the existing process/DB-FD exclusion check passes. It retains
the observed unit states in the private attempt before resetting only those proven stopped
failed units for an explicit start. This is neither a health PASS nor a database/floor reset;
the original same-schema and v27 adapters keep their bounds. The disposable rehearsal must
reach this blocked-barrier case and preserve a session accepted before the coordinator crash.

## Behavior impact and verification ledger

`cantos.identity.native-session-admission` is draft; the approved auth contract owns permanent
revocation. This changes a release oracle and adds migration evidence, not the authentication
transition rules. Classification: code defect in the release oracle; no model/lifecycle change.
There is no registered v27-to-v36 host-executor model or implementation acceptance in this
slice. Existing model/conformance/assurance obligations remain, never auto-N/A.

| Claim | Oracle | Limit |
| --- | --- | --- |
| Release declaration matches real source | Production compatibility owner with real files in Git fixture; negative drift regression retained | Not migration/activation acceptance |
| Exact v27 data/guards survive v36 and replay | Real-engine populated prefix tests and DDL failures | Synthetic files, not current production data |
| Stale copies lose post-cut revocation | Real-engine counterexample and restore-gate regression | NO_GO evidence, not a recovery mechanism |
| Rehearsal binds the exact suffix/pair and refuses false success | Transition/receipt tests plus actual disposable-image run when available | Doubles are not image/HTTP evidence |
| Current CLI reaches the new owner | xtask parser/dispatch regression | Not executor or host qualification |

Report actual commands/results and source identities per run. Unrun exact-image, Linux,
executor, model, browser/device and production gates remain explicit gaps.
