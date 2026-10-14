# ADR 0026: Theatre Mobile local storage boundaries and layered test sandbox

- Status: proposed; the adapter-agnostic `ProgressPort` contract suite is implemented. No storage
  library, directory abstraction or durable adapter is adopted.
- Date: 2026-10-14
- Decision owner: Cantos maintainers
- Related: [mobile architecture](../architecture/mobile.md#implementation-decisions-to-record),
  [downloads and local state](../../.agents/skills/cantos-cmp-mobile/references/downloads-and-local-state.md),
  [fake storage simulator](../../apps/cantos-theatre/mobile/README.md#fake-storage-simulator),
  [#16 offline downloads](https://github.com/loveoverflowcom/cantos/issues/16),
  [#17 progress sync](https://github.com/loveoverflowcom/cantos/issues/17),
  [#34 guest/sign-in policy](https://github.com/loveoverflowcom/cantos/issues/34)
- Supersedes / superseded by: N/A

## Context

A review proposal asked for one pull request that standardizes app-private directories and a
single, centrally configured test sandbox for Theatre Mobile: an `AppDirectories` value, Okio
file-system adapters with `FakeFileSystem` in tests, an SQLDelight-backed `ProgressPort` with
in-memory and temporary-file SQLite tests, and a shared contract suite for the fake and SQLite
adapters. [mobile.md](../architecture/mobile.md#implementation-decisions-to-record) requires an
ADR for the local metadata store before implementation, so this record decides what the
proposal establishes now and what waits for a consumer.

Known facts, from the code and owning documents at `develop` `e226735`:

| Fact | Source |
| --- | --- |
| The CMP app is a memory-only mock. `FakeStorageDatabase` serializes every `FakePersistentProgressPort`, injects latency and write faults, and fences writes with a reset epoch. | [fake storage simulator](../../apps/cantos-theatre/mobile/README.md#fake-storage-simulator) |
| `ProgressPort` loads and stores one whole `ListeningSnapshot`. That model is memory-only, has no account scope and is not release-pinned. | `ports/ProgressPort.kt`, `domain/listening/ListeningRecords.kt` |
| Durable progress is scoped to account, episode and immutable publication, written locally first and synchronized through an append-only outbox. Never reconcile by maximum position or order by device clock. | [mobile.md § Durable listening progress](../architecture/mobile.md#durable-listening-progress), [progress sync](../../.agents/skills/cantos-listening/references/progress-sync.md) |
| #34 owns guest/sign-in policy. The Library/Progress core requires an account for durable state and adds no guest import. The mock's account screen tells listeners that saves and positions last only for this session. | [mobile.md](../architecture/mobile.md), [mock README](../../apps/cantos-theatre/mobile/README.md#screens-and-behavior) |
| No common code performs file I/O. Native identity adapters write their own platform files outside `commonMain`: an Android `AtomicFile` under `noBackupFilesDir` and an iOS backup-excluded marker. | `androidMain/.../adapters/identity`, `iosApp` |
| Adding a dependency requires a concrete consumer and executed compile, link and test evidence on every claimed target. Kotlin/Native 2.2.20 is absent on the current host, so iOS framework linking is `UNSUPPORTED`. | [CMP tooling](../../.agents/skills/cantos-cmp-mobile/references/cmp-tooling.md), [mock README](../../apps/cantos-theatre/mobile/README.md#local-commands) |

The proposal's technical observations, checked against these facts:

| Proposal claim | Finding |
| --- | --- |
| Okio `FakeFileSystem` cannot back SQLite | Correct. SQLite performs file I/O, locking and journaling through its own [VFS layer](https://www.sqlite.org/vfs.html), not through `okio.FileSystem`. A fake file system would silently test nothing about the database. |
| Do not write a fake SQLite; use the real engine in memory or in a temporary file | Correct. A JDBC in-memory database disappears when its connection closes, so close/reopen, migration and WAL tests need a file-backed database. |
| Keep `FakeStorageDatabase` for concurrency and fault injection | Correct. An SQLite driver cannot express virtual-time latency, request-order fencing or a reset epoch. |
| Domain and application code never see directories | Correct. Paths are adapter configuration chosen at the composition root. |
| Separate the database, verified offline audio and partial downloads by backup and purge policy | Correct as a requirement. The proposed placement still needs choices: Android Auto Backup includes the database directory by default, iOS backs up Application Support unless a file is excluded, and the sketch did not set the exclusion it described. |
| A shared contract suite binds the fake and a future durable adapter | Correct, adopted below. Its sample case, "monotonic progress update ignores stale timestamp", contradicts progress sync: there is no timestamp on the port, a rewind is valid, and device clocks never order updates. |
| Add a durable `ProgressPort`, `FileStoragePort`, `AppDirectories`, Okio and SQLDelight now | Rejected for now. There is no consumer, a guest-scoped durable store would contradict the account rule and the session-only copy, and a whole-snapshot table would fix a schema before the listener progress contract and outbox. |

## Decision

1. **Substitute each I/O layer with the test double native to its own abstraction.** The test
   sandbox is a centrally configured set of per-layer adapters, not one fake file system that
   every library is assumed to use.

   | Layer | Production adapter (when adopted) | Test double | Never |
   | --- | --- | --- | --- |
   | Listening behavior through `ProgressPort` | durable account-scoped adapter (#17) | `FakeStorageDatabase` / `FakePersistentProgressPort`: latency, faults, reset fencing, request order | replace it with SQLite in memory |
   | File bytes: download staging, verification, atomic move (#16) | Okio `FileSystem.SYSTEM`, a candidate | Okio `FakeFileSystem`, a candidate, with a crash hook at each staging point | route SQLite through it |
   | Local metadata store | SQL engine, with SQLDelight and Room KMP as candidates | the same engine: in memory for queries and mapping; a file in a per-test temporary directory for close/reopen, migration and WAL | a hand-written fake SQL engine |
   | Platform directories | resolved by the host and passed to the adapters it constructs | test temporary or fake roots | a path inside `domain`, `ports` or `application` |

2. **Contract first.** An adapter-agnostic `ProgressPortContract` lives in `commonTest` and is
   bound today to the sandbox fake, with and without latency. It requires that a new store loads
   exactly its seed; a stored snapshot round-trips exact text, `Long` and list order; a store
   replaces the whole snapshot, so unsaved works, cleared history and rewinds never merge back;
   reopened connections share one store; separate stores stay isolated; and a caller cancelled
   before storing commits nothing. A future durable adapter must pass the same suite on each
   target it claims. Fake-only controls stay in the fake's own tests, and durable failures
   (storage full, I/O error, cancellation inside a transaction) need adapter-level tests.

3. **Directory classes, verified against current platform documentation at adoption.**
   Identifiers in paths are opaque: account scope, publication and asset checksum.

   | Class | Android candidate | iOS candidate | Unresolved at adoption |
   | --- | --- | --- | --- |
   | Metadata database | app database directory | Application Support | backup inclusion of account-scoped rows; both platforms back it up by default |
   | Verified offline audio | app-specific internal storage excluded from backup | Application Support, excluded from backup, with a file-protection class readable after first unlock | retention per publication revision |
   | Partial downloads (`.part`) | same volume as offline audio, outside backup | same volume as offline audio, outside backup | purgeable cache (purge forces a restart) or managed storage (orphan cleanup required) |
   | Credentials | platform secure storage only (existing native identity adapters) | Keychain (existing) | none here |

4. **Adoption gates.** Okio, `okio-fakefilesystem`, SQLDelight or Room KMP, an `AppDirectories`
   value, a file-storage port and a durable `ProgressPort` enter the build only with the slice
   that consumes them. Each needs a named consumer and gap, exact versions checked against Kotlin
   2.2.20, Compose 1.9.0, AGP 8.11.1 and Gradle 8.14.3, and executed compile, link and tests on
   desktop JVM, Android and iOS simulator (including system SQLite linkage for a native driver).
   The metadata store additionally needs account-scoped keys, the outbox and its transaction
   boundary, a migration test from every shipped schema version and this suite passing. That
   adoption updates this ADR or a successor.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| Per-layer doubles behind ports, contract suite now, libraries with their consumer | each test exercises the layer it claims; no unused dependency; the fake keeps deterministic faults | the durable adapter and file adapters remain future work | selected |
| One fake file system for every library | one sandbox object | SQLite bypasses it through its VFS; database tests would pass without touching the fake | rejected |
| Hand-written fake SQLite | fast and controllable | duplicates engine semantics and diverges from transactions, locking and migrations | rejected |
| Replace `FakeStorageDatabase` with in-memory SQLite | one fewer fake | loses virtual-time latency, request-order fencing and injected faults; drivers block threads | rejected |
| Adopt Okio, SQLDelight, `AppDirectories` and a SQLite `ProgressPort` in one PR now | early integration practice | no consumer; a guest-scoped durable store contradicts the account rule and session-only copy; the whole-snapshot schema predates the progress contract; iOS link evidence unavailable | deferred to #16 and #17 |
| Copy the same cases into each adapter's tests | no shared test base | the copies drift; a missing case goes unnoticed | rejected |

## Consequences

- Product and user impact: none. The mock stays memory-only and its session-only copy is unchanged.
- Compatibility and migrations: none. No dependency, schema, migration or port signature changes.
- Operations, storage, and production cost: none until a slice adopts a store; then backup
  inclusion, purge and file protection become visible configuration that must be tested on devices.
- Implementation and recovery requirements: #16 runs its staging protocol over the file-system
  double with the crash points from the downloads reference; #17 binds its durable adapter to
  `ProgressPortContract` plus file-backed reopen and migration tests. A whole-snapshot store is
  not the target model: durable records follow the Library/Progress contract and outbox.
- Deferred questions: metadata store library; whether the outbox shares its database; backup
  inclusion of account-scoped rows; purgeable partial downloads; guest-to-account handling (#34).

## Validation and revisit conditions

Evidence for the implemented part: `cargo xtask theatre-demo verify` executes the contract suite
on desktop JVM through two fake bindings. Sensitivity was checked with seven temporary adapters
that were not committed: sorted lists, a maximum-position merge, a set union of saved works,
per-connection stores, one store shared across stores, a store ignoring cancellation, and a
floating-point `Long` encoding. Each failed at least one case, and both real bindings passed.
Android and iOS execution of the suite is `NOT_RUN`.

Revisit when #16 starts download staging, when #17 schedules durable progress after the listener
progress contract and #34, when an installed Kotlin/Native 2.2.20 makes iOS linking possible, or
when a platform storage, backup or file-protection rule changes.
