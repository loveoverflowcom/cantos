# Downloads and local state

> **Scope.** The client side of offline listening: the download lifecycle, the staging and
> verification protocol and its crash points, storage locations, account scoping, revocation,
> the local metadata store, the progress outbox and background execution limits. Use for any
> change to downloads, local persistence, progress or bookmark sync on Android or iOS.

Owners, not restated here:

- **Entitlement, offline policy, revalidation and manifest meaning:**
  [`downloads-contract.md`](../../cantos-listening/references/downloads-contract.md).
- **Progress and bookmark operations and reconciliation:**
  [`progress-sync.md`](../../cantos-listening/references/progress-sync.md).
- **Offline redistribution rights:**
  [`rights-and-provenance.md`](../../cantos-publication/references/rights-and-provenance.md).
- **Product behavior:**
  [mobile.md § Downloads and access rights](../../../../docs/architecture/mobile.md#downloads-and-access-rights),
  [§ Durable listening progress](../../../../docs/architecture/mobile.md#durable-listening-progress)
  and [business rules 27–28](../../../../docs/product/business-rules.md#listening-and-mobile).

This reference is the Kotlin technique. Code is illustrative and proposed; platform mechanisms
are candidates to verify against current documentation and the pinned versions.

## The lifecycle is a closed type

```kotlin
sealed interface DownloadState {
    data object NotDownloaded : DownloadState
    data object Authorizing : DownloadState
    data class Queued(val waitingFor: QueueCondition) : DownloadState   // unmetered network, space
    data class Transferring(val received: ByteCount, val expected: ByteCount) : DownloadState
    data object Verifying : DownloadState
    data class Available(val download: VerifiedDownload) : DownloadState
    data class Failed(val reason: DownloadFailure) : DownloadState
}

data class DownloadEntry(
    val publication: PublicationRef,
    val current: VerifiedDownload?,     // stays playable while a replacement is attempted
    val pending: DownloadState,
)
```

`current` and `pending` are separate facts so a failed replacement for a new publication
revision leaves the completed download playable, as mobile.md requires. `VerifiedDownload` is an
evidence type: its constructor is private to the verifier, so no code path can mark bytes
available without a size and checksum match
([types-as-proofs.md](../../cantos-engineering/references/types-as-proofs.md)). A pure
`reduce(entry, event)` owns every transition; the executor only moves bytes.

## Staging protocol and crash points

mobile.md fixes the order: entitlement, staging, durable resume metadata, verification, atomic
availability, recorded failure. Each step leaves a durable record from which recovery is
deterministic:

```text
1 check entitlement + offline policy (server)     → row: Requested(publication, asset,
                                                         size, checksum, format, policy)
2 obtain short-lived download authorization       → memory only; never the file's identity
3 write  staging/<account>/<publication>/<asset>.part
  persist received bytes + validator at intervals → row: Transferring(offset, validator)
4 verify size, then streaming checksum (manifest's algorithm)
5 rename into the final location on the same volume
6 one transaction: Available(path, size, checksum), staging row removed
7 on every start: reconciliation scan
```

| Crash after step | Found on restart | Recovery | Never |
|---|---|---|---|
| 1 | request row, no file | authorize and start | assume authorization is still valid |
| 3 (mid-transfer) | partial file + recorded offset | resume with a range request only if file length ≥ offset and the validator still matches and the server's delivery semantics permit it; otherwise restart | resume past bytes that were not durably written |
| 3 (complete) / 4 | full staging file, unverified | verify again | mark available because the length looks right |
| 5 | final file, row not `Available` | verify the final file against the manifest, then mark; otherwise delete the orphan | offer it as playable before the mark |
| 6 | `Available` | cheap check (exists, size) at start; full hash when the policy says | — |
| — | `Available` row, file missing (OS purge, user cleanup) | `Failed(Missing)` with re-download offer | hand a missing path to the engine |

This table is the fault-injection plan. Run the protocol over a fake file system (Okio's
`FakeFileSystem` is a candidate) with a crash hook at each named point, run recovery, and assert
the invariant: **no state offers bytes that did not pass verification**. Report it as
`fault-injected` with the crash points listed; add a property over random crash points and
transfer chunking as `property-tested`.

## Edge cases → handling → oracle

| Case (from mobile.md) | Handling | Oracle |
|---|---|---|
| storage exhaustion | preflight against the platform's allocatable/important-usage capacity; on a write failure mid-transfer, keep metadata, delete nothing completed, `Failed(InsufficientStorage)` with cleanup offer | fake file system that fails at byte N; device run with a full volume |
| metered network preference | the preference is a `QueueCondition`; the executor receives constraints, the reducer decides | reducer table; device toggle of Wi-Fi/cellular |
| authorization expiry mid-transfer | refresh authorization separately from file identity; resume with the same validator | fake server rejecting the old URL; state keeps `received` |
| process death | recovery table above | `fault-injected`; device kill during transfer |
| checksum mismatch | delete staging, `Failed(ChecksumMismatch)`; bounded retries because the server may be wrong | exact-variant test; `current` unchanged |
| changed publication | new revision downloads into its own path; `current` stays until the new one is verified; retention follows the policy | `failed_replacement_keeps_previous_download_playable` |
| logout / account switch | all rows and directories keyed by account scope; switching hides the other account's files immediately; in-flight work tagged with the account generation is cancelled and its results rejected | `account_switch_never_lists_previous_account_downloads` |
| revoked access, offline validity | store the server-stated validity window from the contract; check before offline playback; revalidate on reconnect; on revocation mark unavailable and clean up per policy; disclose that offline revocation is not immediate | reducer table over (online, offline) × (valid, expired, revoked) |
| removal / cancellation | cancel the transfer, then delete local files only; server assets are never touched | executor fake asserts cancel before delete |

No DRM guarantee is implied by any of this
([050 non-goals](../../../../docs/work-plan/050-cmp-native-listening.md#non-goals)).

## Files and storage locations

| Concern | Android (verify) | iOS (verify) |
|---|---|---|
| location | app-specific internal storage, outside the cache directory the system may clear; excluded from backup | Application Support, not Caches (purgeable); excluded from backup |
| readable while locked | — | a file-protection class that allows reads after first unlock; the strictest class makes downloaded audio unreadable on the lock screen |
| capacity preflight | the storage manager's allocatable-bytes query | the volume's available capacity for important usage |
| atomic availability | staging and final directories on the same volume, rename | the same; move the background-session temp file before the delegate returns |

Paths use opaque identifiers only: account scope ID, publication ID, asset checksum. Never
titles, emails or signed URLs; hashing an email does not anonymize it.

## Local metadata store

mobile.md requires an ADR for the local metadata store. SQLDelight and Room KMP are candidates;
evaluate target support, migration tooling and transaction semantics on both platforms. A
key-value store (DataStore KMP is a candidate) suits settings only.
[ADR 0026](../../../../docs/decisions/0026-theatre-mobile-local-storage-sandbox.md) (proposed)
fixes the test doubles per layer (never SQLite over a fake file system), the directory classes
and the adoption gates, and requires a durable `ProgressPort` to pass the shared contract suite.

- Every row is keyed by account scope. Download rows reference publication and asset checksum,
  never a URL.
- Records that settle are written once: a `VerifiedDownload` row, an outbox operation, an
  acknowledgement. Corrections append or supersede
  ([immutability.md](../../cantos-engineering/references/immutability.md)).
- A state transition and the outbox append it implies commit in one local transaction.
- Schema migrations are versioned and tested from every shipped version; a migration test over a
  fixture database is the oracle.
- Credentials never live here. The `CredentialStore` port is backed by platform secure storage
  (Keystore-backed on Android, Keychain on iOS). Choose a Keychain accessibility class that still
  permits token refresh in the background after first unlock, and check the maintenance status
  of any Android wrapper library before adopting it.

## Progress outbox

The operation shape and server behavior belong to `progress-sync.md`; the client owns durability
and scheduling.

```kotlin
data class ProgressOperation(
    val operationId: OperationId,          // idempotency key
    val deviceId: DeviceId,                // stable per install
    val sequence: Long,                    // monotonic, from the local store, never from a clock
    val accountScope: AccountScope,
    val publication: PublicationRef,
    val position: PlaybackPosition,
    val completed: Boolean,
    val baseRevision: ServerRevision?,     // last server revision this device observed
    val recordedAt: DeviceInstant,         // context only, never ordering
)
```

- **Append-only.** Record a position locally first, append an operation, flush later. An
  acknowledgement records the server revision; it does not rewrite the operation.
- **Flush** at bounded intervals, on pause or stop where lifecycle permits, on reconnect and on
  foreground; never per second. Backoff with jitter. One flusher per account, owned by the
  process scope.
- **Responses.** Accepted or duplicate → record the revision. Stale or conflicting → keep the
  local candidate, store the server's current value and revision, and project the choice the
  docs describe ("Tiếp tục tại đây" / "Dùng vị trí trên thiết bị khác"); submit the chosen value
  as a new operation against the current revision.
- **Never** reconcile by maximum position (backward seeks are valid), never order by device
  clock, never send account A's operations with account B's credentials, and coalesce pending
  operations only as `progress-sync.md` permits.
- **Bookmarks** use stable IDs and operation IDs; a deletion is a tombstone so replay cannot
  resurrect it.

Oracles: shared vectors from `cantos-listening` run in `commonTest` (`differentially-tested`
against the spec, reported per platform); a property over interleavings of local writes,
network failures, duplicate deliveries and restarts asserting that no operation is lost and no
acknowledged state regresses (`property-tested`); then `integration-tested` against the real
listener API and the two-device plus Theatre Web scenario from the
[050 review boundary](../../../../docs/work-plan/050-cmp-native-listening.md#review-boundary).

## Background execution

Neither platform guarantees immediate execution while the app is suspended; mobile.md says to
persist recoverable state and not promise otherwise. Show "queued" honestly.

| Concern | Android candidates (verify) | iOS candidates (verify) |
|---|---|---|
| download execution | WorkManager with network and storage constraints and long-running foreground info; user-initiated data transfer jobs on newer API levels | a background `URLSession` configuration; the system relaunches the app to deliver completion events |
| limits | foreground-service type rules and time limits for data sync on the targeted API levels | a user force-quit cancels background transfers; resume data on failure; expensive and constrained network flags map to the metered preference |
| signed URL expiry | refresh before resuming | a long background transfer may outlive its URL; re-authorize and resume from resume data |
| progress sync | periodic or one-off work with a network constraint | background refresh is opportunistic; flush on foreground as well |

Exactly one executor per process. The reducer decides what should run; the executor reports
facts back as events. Two executors racing the same staging file is the failure to test for.

Evidence selection for all of the above is in [cmp-testing.md](cmp-testing.md).
