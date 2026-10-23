# Identity security store v1 (internal)

This is the partial #334.1/#334.5 storage contract implemented by the optional
[`cantos-identity` persistence shell](../README.md#file-backed-security-state).
It is not a public HTTP/RPC, credential or authentication-fact contract. Server continues to use
PostgreSQL; only the Identity-owned library consumer opens this new file through the shared owner.

## Schema and trust boundary

[`schema.sql`](../src/persistence/schema.sql) is migration 1 of
Identity schema epoch 1, using Identity's service-owned `IDENTITY_APPLICATION_ID`
(`CNTI`, `0x434E5449`) marker. The shared owner supplies its own immutable migration history. No PostgreSQL table, registry checksum,
provider access token, ActorId or email binding is copied into this schema.

| Relation | Stored invariant |
| --- | --- |
| `identity_instance` | One nonzero 16-byte instance epoch; ordinary API writes cannot alter/delete it |
| `identity_subjects` | Nonzero 16-byte subject namespace, positive INTEGER security version, Active or Disabled state; subject identity/delete guarded |
| `security_events` | Positive explicitly allocated sequence, subject/version/kind/state, trusted microsecond timestamp; append-only with one event per subject version |
| `operation_receipts` | Nonzero operation namespace bound to subject, change kind, expected/resulting version and the exact event; immutable and unique |

Domain constructors and checked row decoding remain the first barrier. SQL checks, foreign keys,
uniqueness and named `cantos:identity.*` triggers are the second barrier on the pinned engine.
The OS/process owner can still alter its own file outside this API; these are not PostgreSQL
role-level permissions or protection against a hostile filesystem owner.

Time is `UnixMicros` from the shared transaction clock. Versions/sequences are positive and at
most `i64::MAX`; neither wraps. The sequence is allocated under the shared serialized writer,
not inferred from a row ID, clock or PostgreSQL transaction ID. No event retention/deletion or
ACK protocol is supplied by this slice.

## Startup and mutations

`Intent::Serve` never creates schema or instance metadata. Explicit `Migrate` creates/checks the
static migration first, then initializes an OS-random epoch in a guarded transaction only when
all service tables are empty. A crash between those stages leaves an uninitialized file which
Serve refuses and explicit Migrate may finish. Ordinary reopen preserves the epoch; data without
the metadata is corruption. A restore cannot be detected from that file alone. Epoch rotation
and Server full resync are still required future acceptance, not implicit startup behavior.

All mutations require the expected instance epoch before writer acquisition or receipt replay:

- `create_subject(operation, subject, expected_epoch)` creates version 1 / Active, a Created
  event and receipt atomically. This is an internal caller-authorized storage primitive with
  no credential, email, ActorId or privilege side effect. A different operation cannot recreate
  an existing subject.
- `disable_subject(operation, subject, expected_version, expected_epoch)` admits only an Active
  subject at that version. It atomically writes Disabled / checked successor version, a Disabled
  event and receipt. Re-enable, delete, password/factor change and generic state transitions are
  intentionally unimplemented.
- Exact retries return the same immutable receipt. Reusing an operation with another subject,
  kind or expected version is refused. Epoch mismatch is refused even for a replayed operation.

`OperationReceipt` describes an accepted historical effect. It is distinct from `StoredSubject`
and contains no current-active assertion. Creation replay after disable can return version 1's
receipt while a fresh subject read returns version 2 / Disabled. Neither type authenticates a
caller or becomes an `AuthenticationFact`.

Precondition or statement failure explicitly rolls back. COMMIT outcomes preserve the shared
`Rejected` versus `Unknown` distinction; a timeout/lost acknowledgement is never rollback proof.
Reconcile using the exact epoch/operation/binding receipt and current state before retrying or
reporting absence. New receipt/event rows and subject state are not separate commits.
The initialization transaction has no subject-operation receipt: its separately typed unknown
outcome is reconciled by reopening/checking instance metadata, never by inventing a receipt.

`events(after_seq, limit)` returns ordered checked event records with an exclusive cursor and
`1..=128` batch bound. It has no transport authorization, Server checkpoint, freshness promise,
global revocation claim or restore handshake. Those belong to #334.7/#335.

## Compatibility and evidence

This is a new opt-in service schema, with no populated production migration supplied. Its
checksummed SQL must not be edited after acceptance/application; later credential/proof/state
work needs additive reviewed migrations. A newer file is refused by an older registry. Mutable
DB/WAL/sidecar files and test output stay outside Git in run-owned private directories.

The fixtures in [`file_tests.rs`](../src/persistence/file_tests.rs)
use the production consumer, shared `TestRun`/`FixedClock` and actual pinned Turso files. They
separate real SQL, rollback/concurrency/reopen and lost-response tests from pure domain tests.
No model, mock, empty database or proof harness is presented as login/RPC/restore acceptance.

The existing `cargo xtask verify turso` gate includes Identity persistence lint/tests before
the shared foundation/probe suite and runs in existing Rust CI; Identity source changes select
it in preflight. Focused primitive: `cargo test --locked -p cantos-identity --features persistence`.
Kani runs the unchanged default pure kernels; actual D1 runner evidence remains separate.
