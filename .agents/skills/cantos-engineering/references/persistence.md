# PostgreSQL persistence

> **Scope.** Design and verify Cantos PostgreSQL usage: what the database owns, transaction
> boundaries, constraints as a second barrier, optimistic concurrency on drafts and progress,
> append-only history, migrations and real-database tests. Use when adding a table, migration,
> query, transaction, uniqueness rule or concurrency control. External upload recovery is
> detailed in [`cantos-production-pipeline`](../../cantos-production-pipeline/references/durable-jobs.md);
> backend synthesis jobs and provider leases are retired under ADR 0018.

PostgreSQL owns metadata, revision history, producer submission receipts, approvals,
publication manifests and listener progress; media bytes live in object storage
([architecture § Data and delivery](../../../../docs/architecture/overview.md#data-and-delivery)).
The API uses `tokio-postgres` with `deadpool-postgres` and a checked migration registry in
`services/cantos-api/src/platform/postgres/migrations.rs`. Inspect its current schema and
adapters before adding persistence; historical provider-job/cost tables remain compatibility
data, not current synthesis runtime state.

For queries or writes feeding a read model, apply the foundation's
[read-performance workflow](postgresql-read-performance.md). It owns PostgreSQL baselines,
index/query comparisons, conditional DBSP evaluation and maintenance/recovery evidence; this
reference continues to own transaction and persistence correctness.

## The database is the second barrier, not the first

Domain types are the first barrier. Constraints catch what a bug, a concurrent writer or a manual
fix would otherwise persist:

| Invariant | Constraint |
|---|---|
| stable IDs are unique | primary keys; no natural-key reuse |
| one accepted artifact per production step | partial unique index on `(step_id) WHERE accepted` |
| a revision number is unique per script | `UNIQUE (script_id, revision)` |
| a provider attempt references a reservation | foreign key, `NOT NULL` |
| money is non-negative minor units | `CHECK (amount_minor >= 0)` with an integer type |
| an idempotency key is used once per actor and operation | `UNIQUE (actor_id, operation, idempotency_key)` |

A constraint failure maps to a typed domain or conflict error in the adapter; it never leaks as a
raw SQL error to a client, and it is never swallowed.

## Transactions follow decisions

```text
read facts (consistent snapshot or row locks)
  → pure decision
  → one transaction: write the decision + outbox rows, guarded by the facts it relied on
```

- Keep transactions short and free of network calls to providers or object storage.
- Revalidate what the decision assumed at commit: `UPDATE … WHERE revision = $expected`,
  `WHERE lease_token = $token`, or `SELECT … FOR UPDATE` for the rows the decision depends on.
- Zero rows affected by a guarded update is a typed outcome (conflict, lost lease), never success.
- External side effects that must follow a commit go through a transactional outbox, executed
  after commit by a consumer that tolerates repeats.

## Optimistic concurrency

Draft saves and listener progress carry the revision the client last saw. The server accepts
atomically or returns the current state and revision. It never silently overwrites a competing
edit, and it never orders writes by device clocks. Test the conditional update against a real
database with two concurrent writers.

## Append-only history

Settled records are inserted, never updated in content ([`immutability.md`](immutability.md)).
Where a mutable pointer exists (draft head, active release, lease owner), the pointer change is
a guarded transition and the pointed-at records stay immutable. Consider revoking `UPDATE` on
settled tables for the application role through reviewed migrations; prove it with an
integration test, not by reading the migration.

## Migrations

- Each migration is forward-only, reviewed, and paired with fixtures of the data it transforms.
- Separate expand (add nullable column, backfill) from contract (enforce, drop) when a running
  worker or client might still use the old shape.
- Data migrations that change meaning record what they changed; they never mutate settled
  approvals, manifests or artifacts' content.
- A migration's test seeds the **old** schema with representative rows, runs the migration and
  validates every row through current domain constructors.

## Testing against a real database

An in-memory fake is useful for application tests, but it cannot prove isolation levels, locking,
partial unique indexes or `SKIP LOCKED` behavior. Claims about concurrency, constraints,
idempotency and migrations need `integration-tested` evidence against PostgreSQL:

- one database (or schema) per test, created and dropped by the harness;
- deterministic seed data, fixed clocks and IDs injected by the shell;
- two connections for concurrency tests, with explicit interleaving points rather than sleeps;
- assertions on rows, not only on returned values.

Report the PostgreSQL version and how it was provisioned. A test that ran against a fake is
`example-tested` against a double.

## Review questions

1. Which invariant does this table or index protect, and which domain type protects it first?
2. Can two concurrent requests both pass the decision and both commit? What guard prevents it?
3. Does any transaction wait on a network call?
4. Does a rejected operation leave rows unchanged, and is that asserted?
5. Can the migration run while the previous application version is still serving requests, and is a rollback path stated??
