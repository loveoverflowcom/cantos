# Publication/access recovery and rollout boundary (#658)

This is the operator prerequisite/runbook for [#634](https://github.com/loveoverflowcom/cantos/issues/634),
not an executable restore or deployment lane. [ADR 0054](../decisions/0054-publication-generation-and-access.md)
and [ADR 0055](../decisions/0055-development-phase-compatibility.md) own the accepted rules.
The [S12 evidence ledger](../evidence/publication-access-readiness-658.md) owns the source-bound
observations. Production migration, restore, credential/security changes and activation require
their own authorized task and reviewed implementation.

## Current decision: NO_GO

**An old Server/Identity pair is not safe to expose.** The disposable S12 drill observes:

| Restored cut | Original viewer's session | Restricted metadata/media |
| --- | --- | --- |
| Both snapshots taken after logout, grant revoke and Identity disable | 401 | 401; no audio |
| Server before those changes, Identity after disable | 401 from the fresh Identity check | 401; no audio |
| Both snapshots before those changes | **200: the logged-out session is resurrected** | 404 under the default closed composition; no audio |

These are actual production-router requests and a real Identity process/RPC on disposable Turso
files. They are not production recovery acceptance. `IdentityStore::open` retains the restored
instance epoch; no restore epoch rotation, Server resynchronization or external revocation floor
is implemented. A file that predates logout/revoke cannot discover that lost history by itself.
Default restricted OFF does not protect every authenticated surface from a resurrected session.
Keep a stale pair offline, including authenticated/public application exposure, until the
security recovery protocol and its tests are accepted. Do not infer safety from matching hashes,
an intact schema or successful startup.

Ordinary Server assembly never enables restricted serving. Its trusted test/composition opt-in
has no persistent switch and no operator activation command; starting a fresh process loses the
activation and its epoch invalidates old private grants. Do not add an activation switch to work
around this recovery gap. Private Web Play remains locked after teardown failure; native Play
remains locked without safe credentials on each open/seek/refresh. Their locks are separate from
the recovery fence.

## Backup boundary

Server owns `server.db`; Identity owns `identity.db`. Neither service reads the other's file.
`OwnedDatabase::backup_to` serializes its writer, refreshes its engine snapshot, stages
`VACUUM INTO`, verifies integrity/current registry/table row counts, fsyncs and returns SHA-256
and length. It publishes without overwriting a destination. It is a **single-file** operation.
Two receipts do not establish an atomic cross-file cut.

For a future reviewed production checkpoint:

1. Resolve the exact binary/image pair, complete schema registries, configuration/storage target
   identities and current deployment admission record. The older OCI examples naming Server v24
   do not admit this v35/Identity v4 source.
2. A reviewed owner protocol must quiesce HTTP mutations, in-flight authentication/signup/security
   transitions, Server writes and Identity writes. Specify drain, timeout, interruption and
   uncertain-outcome reconciliation. This host coordination is **not implemented by S12**.
3. Each owner performs its own verified `backup_to` to a fresh private destination. Record the
   two receipts, cut identifiers/times, schema/checksum history and Identity epoch/security-state
   observation. Never copy an open main file or drop WAL/owner locks to obtain a snapshot.
4. Record immutable media manifests and the retained original-target object set separately.
   Database snapshots do not back up media objects, encryption/recovery keys, configuration or
   rights documentation. Reconcile missing objects and preserve their hashes/provenance.
5. Reconcile any unknown snapshot publication before reuse. A final destination or `.backup`
   reservation is evidence to inspect, never permission to overwrite/delete it.
6. Qualify an exact disposable recovery copy with the intended binary pair and all current
   security gates. Keep production files and original snapshots immutable during the rehearsal.

The S12 test coordinates a quiescent harness, stops Identity before its independent backup
owner opens the file, and performs no concurrent mutations during the two snapshots. It does
not establish the future protocol in step 2, process-crash durability, a consistent live pair,
offsite/encrypted recovery or a media-object restore.

## Recovery prerequisites and sequence

All production steps below remain pending; this document grants no operation permission.

| Order | Prerequisite / owner boundary | Acceptance before proceeding |
| --- | --- | --- |
| 1 | Identity + Server security owners, currently unassigned | Implement/review epoch rotation and durable Server session revocation/resync, including stale **both-file** rollback, lost reply and restart. Reconcile post-cut grant/group/withdraw changes from an independently retained authority or keep affected policies disabled. |
| 2 | Publication/Media + storage operator, currently unassigned | A reviewed recovery cut identifies exact schema/binary pair, immutable release/rights/approval hashes and original-target media objects; unknown/missing/draft/restricted rows never become public. |
| 3 | Operator on disposable copies | Verify the two digests/lengths/schema histories; create new private destinations; open through each service's `Intent::Serve`. Refuse missing, foreign, diverged, newer or pending schema rather than adopt it. Preserve original backup bytes. |
| 4 | Security owners + test owner | The original revoked session, refresh family, disabled account, revoked grant/group membership, expired share, withdrawn Work/Unit and old media/cursor all remain denied. Test current, stale, mixed and interrupted cuts, then each restart order. Any revival is FAIL. |
| 5 | Publication/Media owner | Re-confirm current policies and rights from accepted authority after rotation/resync; qualify every actual route/byte path. A historical receipt or owner statement alone does not implement an activation protocol. |
| 6 | Web + native transport owners | Resolve Web teardown and native per-fetch credential gaps; qualify trusted HTTPS cookies, actual HTMLAudioElement, Android and iOS source loaders, seek/Range/refresh, account cleanup and late callbacks. |
| 7 | Release/operator owners | Obtain an admitted current image pair, separate production migration/activation authorization, validated ingress/cookies/CORS/origins, backup retention, restore/cutover receipts and monitoring/rollback decision. |

No numbered step is a host command supplied by S12. Current reproducible local commands are
in the [evidence ledger](../evidence/publication-access-readiness-658.md#commands-and-local-results).
The existing [OCI release contract](native-oci-release.md) remains authoritative for its own
supported same-schema host/binary upgrade lane; that lane explicitly never restores database
bytes. Do not reinterpret binary rollback as this two-file security recovery.

## Migration and rollback

ADR 0054/0055 supersede old epic prose requiring compatibility lanes and backfill. There is no
new backfill/conversion command. Existing data is not publication authority: only an explicitly
published generation can be disclosed through current readers. Preserve rights/consent and
immutable records for anything retained. Append registered migrations; never edit their SQL.

The S12 refusal test opens a verified v35 snapshot with the actual pre-epic v30 registry prefix.
Both Serve and Migrate refuse `NewerThanBinary { applied: 35, known: 30 }` without changing
snapshot bytes; Identity's registry also refuses it as a foreign service file. This qualifies
the source's refusal, not execution of an old binary, old-client support or production rollback.
Reverting a binary cannot drop ACL/generation gates. Prefer a forward fix; a matching older
data/binary restoration still requires the security protocol above. Never restore only one
file as a shortcut.

## Proposed recovery objectives — pending owner acceptance

These are proposals for a small development/test deployment, **not accepted targets, automatic
retention actions, measured recovery performance or a production SLA**:

| Objective | Proposal for review | Evidence/decision still required |
| --- | --- | --- |
| Content/metadata RPO | At most 24 hours, plus an explicit verified checkpoint before a schema/cutover operation | Owner accepts tolerable content loss; inventory and copy all immutable media; qualify a consistent pair. This says nothing about losing revocations. |
| Security state | No restoration of revoked authority (existing invariant) | Epoch rotation/resync and a trustworthy post-cut revocation/policy reconciliation mechanism; currently unsupported. No nonzero content RPO relaxes this invariant. |
| RTO | At most 4 hours after an operator declares recovery | Measure a complete admitted disposable cut: files, media, rotation/resync, policies, client qualification and ingress. The five-second local harness is not an RTO measurement. |
| Retention | 14 daily checkpoints plus 4 weekly checkpoints, and both sides of any migration until that recovery window is accepted | Owner accepts storage/privacy/rights obligations, encrypted/offsite policy, key custody and deletion authority. No purge follows from this proposal. |

S11's performance budgets remain proposed, independently pending. Reuse its
[actual plans/measurements](../evidence/publication-access-performance-657.md); it adopted no
runtime index/cache. RPO/RTO/retention and performance acceptance belong in their owner records,
not as new constants, gate baselines or fabricated PASS thresholds.

## Ordered review and handoff

The single [#676 ledger](https://github.com/loveoverflowcom/cantos/issues/676) indexes unresolved
#652–#658 acceptance. #658 supplies a local delta for its parent to publish serially. Priorities:

1. Restore rotation/resync and stale-pair session revival; no restored exposure or production GO.
2. Private Web teardown and Android/iOS per-fetch credential transport; keep Play closed.
3. The inherited 66 UI qualification findings, ten Kani obligations, missing generation/listening
   model decisions/conformance and client lifecycle observer. No waiver/hash edit/N/A is supplied.
4. Group affected-Work impact, share status history and exact historical Work-text contract;
   private cover/Library restore remain unavailable.
5. Browser mutation/account-switch/a11y and named Android/iOS/device/AT qualification. The prior
   denied browser operations are not retried by this runbook or a substitute API fixture.
6. Accepted workload/performance and recovery objectives, then current packaging, media restore,
   production migration/admission/activation and independent deployed acceptance.

No merge closes the epic, accepts these missing layers or starts deployment.
