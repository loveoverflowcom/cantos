# Persistence: embedded Turso and legacy PostgreSQL

> **Scope.** Design and verify durable ownership, transactions, constraints, optimistic
> concurrency, immutable history, migrations and real-database tests. The owner-selected
> [native Turso target](../../../../docs/architecture/native-turso-refactor.md) governs epic
> #331. The Server composes a Turso owner; business SQL/schema porting remains incomplete.
> Retained PostgreSQL source is a migration oracle, never native acceptance.

Read the owning context, current adapter, schema registry and contract before changing storage.
For #331, the selected engine is local in-process Turso through the pinned Rust `turso` crate,
with one owner process/file per service. Do not substitute libSQL remote, Turso Cloud, SQLite
or redb. Media bytes remain in approved private/object storage. Other tasks must follow their
actual implemented storage/decision rather than assuming this refactor has already landed.

The Server registry is `services/cantos-server/src/platform/turso/mod.rs`; Identity has its own
registry. Retained `services/cantos-server/schema` SQL and `legacy-pg-tests` are historical porting
inputs. The removed PG migrator is not a current owner. Do not rewrite registered SQL/checksums,
silently adopt an existing file/database or reset data to obtain a pass.
External upload recovery belongs to the [production owner](../../cantos-production-pipeline/references/durable-jobs.md);
backend synthesis/provider jobs remain retired under ADR 0018.

For reads or writes feeding a read model, load [Turso read performance](turso-read-performance.md).
That reference owns baseline/optimization/evaluation evidence, not transactional correctness.

## The database is the second barrier, not the first

Domain types reject invalid values first; constraints and guarded writes catch concurrency,
adapter bugs and operator mistakes. Do not assume a SQLite-compatible engine implements a
particular constraint, trigger, isolation level or SQL function correctly for this workload.
Qualify the selected Turso version/features with real file-backed tests before relying on it.
The pinned engine's measured behavior (what is absent, what is silently open, how to back up) is
recorded in the [capability report](../../../../docs/evidence/turso-capability-report.md) and
enforced by `cargo xtask verify turso`; read it before designing a constraint, guard or backup.

| Invariant | Mechanism to implement and verify |
| --- | --- |
| stable IDs are unique | primary/unique keys, no natural-key reuse |
| one accepted value for a scoped operation | tested uniqueness/conditional mutation in one transaction |
| revisions are unique and monotonic | composite uniqueness, checked increment and compare-and-set |
| references remain valid | actual supported foreign keys/constraints, validated restoration |
| money and sizes stay in range | domain bounds plus tested database constraints and integer conversion |
| a retry cannot duplicate an accepted effect | actor/purpose/operation-bound idempotency receipt |

Constraint errors map to typed domain/conflict/storage outcomes; never leak SQL, swallow an
error or report success from zero affected rows. Lack of required engine support is a named
blocker, not permission to weaken the invariant or silently choose another database.

## Transactions follow decisions

```text
read coherent facts
  -> pure decision
  -> one guarded transaction: decision + durable receipt + required outbox changes
  -> bounded external effects after commit
```

Keep transactions short and free of RPC, password hashing, mail or object-storage waits.
Recheck authority, revisions, security epochs and final expiry at the correct commit boundary.
A failed conditional update is a typed conflict/lost-lease result. A lost commit acknowledgement
is uncertain until reconciled, not proof that no effect occurred. Retries must respect operation
identity and re-evaluate facts where the contract requires it.

Map every replaced PostgreSQL row/advisory lock, `SECURITY DEFINER` function, grant, snapshot
and clock assumption to a concrete Turso mechanism and test. A global in-memory mutex alone
is not a demonstrated substitute for durable concurrency/receipt guarantees. File permissions
and application admission replace part of the trust boundary, not PostgreSQL's role model.
Do not preserve unsupported SQL or remove constraints just to complete a mechanical port.

## Ownership across processes

Each service alone opens its database. Do not use cross-service file reads, ATTACH, joins or
foreign keys. Multiple connections inside one owner are distinct from two processes opening
the file. The selected default excludes experimental multiprocess support; migration, backup
and deployment tooling must cooperate with the owner or run after it is stopped.

Across databases, use explicit idempotent transitions and durable outbox/inbox/checkpoints or
an equivalent verified recovery protocol. Document the observable intermediate states, event
ordering, acknowledgement, retention and recovery. Do not claim atomic distributed commit or
exactly-once delivery from two successful local transactions. Security propagation additionally
follows [identity/session boundaries](identity-and-session-boundaries.md) and the owning v1
amendment's direct-state-check option; do not require an event consumer merely because two
databases exist. Transitional integration uses the current authoritative Server owner.

## Optimistic concurrency

Drafts and progress carry the revision observed by the client. Accept atomically or return the
current state/conflict; do not silently overwrite a competitor or order writes by device time.
Test two actual connections using deterministic interleaving points, including rollback,
commit acknowledgement loss and conflict retry. Record busy/timeout/cancel behavior and
bounded backpressure; success must not depend on an unbounded writer queue.

## Append-only history

Settled revisions, approvals, manifests and receipts are immutable under
[the immutability method](immutability.md). A mutable pointer uses a guarded transition while
its target remains unchanged. Preserve exact accepted bytes/digests during migration.

Use supported database enforcement where appropriate and prove attempted violations fail.
For legacy PostgreSQL, narrow grants/definer functions remain obligations of the old path.
For embedded files, record the changed OS/process trust boundary and compensating validation;
do not claim that a service user with file access has an equivalent table-level privilege model.

## Migrations

- Version and checksum each service's schema/engine format. Startup checks rather than silently
  rewrites a nonempty incompatible database. Define explicit migration ownership and exclusion.
- Preserve PostgreSQL epoch/checksum history and source-bound evidence. A new engine's fresh
  schema is not a license to reinterpret or drop legacy data.
- Import versioned logical records through current domain validation; compare IDs, references,
  counts, immutable hashes, receipts, revisions and authorization state. Include partial/import
  retry/cancellation and unknown outcomes, not just an empty-database happy path.
- A live-file backup must use a tested engine-consistent snapshot/checkpoint procedure, including
  WAL and required key material. Copying the main file alone is not assumed sufficient.
- Rehearse restore, crash/disk-full and compatibility refusal. Coordinate cross-database restore
  cuts/security epochs; restoring one file must not revive a revoked credential/session.
- Separate data rollback from binary rollback. Accepted new writes require reconciliation or
  forward recovery before returning to an old database. State downtime/quiescence constraints.
- Existing production data, volumes and keys require explicit operator authority for any change
  or deletion. Never count deleting media/recovery copies as a footprint optimization.

## Testing against a real database

An in-memory repository double does not establish SQL constraints, isolation, commit durability
or locking. For Turso acceptance use the actual pinned engine and **run-owned durable files**:

- controlled clocks/IDs and independent expected results;
- multiple connections within the owner, explicit interleavings and assertions on stored rows;
- duplicate/competing writes, replay, abort, process restart/crash and acknowledgement loss;
- schema/prepare invalidation, integer/time/NULL/collation boundaries and corrupt-row refusal;
- backup/restore, disk exhaustion and a second-process/file-ownership negative test.

Report version/features, filesystem, durability/checkpoint settings and source/build provenance.
An actual engine in memory may test SQL behavior but does not prove file durability/recovery.
Kani verifies selected pure decisions, not Turso internals or concurrent I/O. Preserve separate
layer results using the [foundation vocabulary](../SKILL.md#6-never-collapse-evidence-into-the-word-verified).

The native target tests require no database/container daemon, cloud or live mailbox. A legacy
PostgreSQL exporter/equivalence lane is explicit and isolated from default workflows; it may
use authorized existing sources or sanitized fixtures. Missing old-engine execution remains
NOT_RUN, not proof that a fixture exercised the actual exporter.

## Review questions

1. Which domain and storage mechanism protect each invariant, and what actually exercises both?
2. Can competing requests commit incompatible decisions? Which persisted guard prevents this?
3. Does a transaction wait on a network or password-hashing effect?
4. Do refusal and unknown outcome have distinct semantics and stored-row assertions?
5. Can this exact engine/schema be backed up, migrated, restored and rolled back without losing
   accepted data or resurrecting revoked authority?
