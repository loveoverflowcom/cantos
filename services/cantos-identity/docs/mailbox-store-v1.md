# Identity mailbox proof store v1 (internal)

This is the challenge-lifecycle storage slice of #334.1/#334.2, implemented by the optional
[`IdentityStore`](../README.md). It persists real mailbox-proof
state in the existing Identity-owned Turso file. It supplies no mail delivery, immutable email
workflow, signup/setup command, credential, verified authentication fact or public/private RPC.

## Ownership and compatibility

[`proofs.sql`](../src/persistence/proofs.sql) is additive migration 2
of Identity schema epoch 1. Accepted migration 1 remains byte-identical. Explicit `Migrate`
upgrades a compatible file; `Serve` refuses an unapplied suffix. The shared `OwnedDatabase`
remains the only file, migration, writer, clock and commit owner. Missing instance metadata
alongside any security or proof table's data is corruption, never a fresh instance.

The proof binding persists its original instance epoch, workflow operation, intent and, for
recovery, exact subject/security version and Theatre/Studio purpose. It is never reconstructed
with a newer instance epoch. A new recovery mutation rechecks the current subject's Active
state and version under the writer. Epoch mismatch refuses even replay before writer acquisition.

| Relation | Stored invariant |
| --- | --- |
| `mailbox_proof_issues` | Immutable issued locators, digests and time windows; lifetime locator/digest uniqueness, including after replacement |
| `mailbox_proof_workflows` | Immutable full binding and current locator/state; composite foreign key to its exact issue; only Pending may become terminal or be replaced |
| `mailbox_proof_receipts` | Immutable command/request binding and historical result; exact issue/workflow association and action/state relationship |

`ProofCommandId` is a separate nonzero 128-bit namespace from the existing security mutation
`OperationId`. The binding's `OperationId` identifies the immutable workflow, not a proof command.
These distinct types and receipt tables deliberately permit identical raw bytes across namespaces.

## Secrets and atomic transitions

The shell generates a 16-byte nonzero locator and a 32-byte secret with the existing pinned OS
randomness library. The locator is public and proves nothing. `ProofSecret` has redacted Debug,
no Clone/Copy/serialization, explicit byte exposure and zeroization on drop. That limits ordinary
accidental disclosure; it cannot erase caller-made copies, CPU registers, swap or mail contents.
The DB receives only a domain-separated SHA-256 digest. Fixed-size digest verification uses
the pinned constant-time comparison library; it is not a whole-system timing proof.

Issuance and replacement take caller-retained `ProofMaterial`. This makes an exact retry possible
without minting a different secret after an unknown outcome. Receipts never recover or return
that secret. A crash may lose an undispatched secret; no plaintext outbox or automatic mail
retry is supplied. A future mail/workflow owner must bind its immutable address, delivery,
rate/send/account/IP budgets and any encrypted outbox/retention policy before activation.

Each mutation loads authoritative current state under the shared serialized write transaction,
calls the existing pure planner, conditionally updates the complete expected snapshot, and
records the bound receipt in that same transaction. Confirmation checks the actual secret and
an adapter-established explicit action. Inspect/prefetch/polling cannot consume a proof.
Replacement requires a fresh locator and secret digest, retains historical uniqueness, and
refuses completion with the former locator/secret. Consumed and Cancelled remain terminal.

Trusted transaction microseconds are converted to Unix seconds for the existing proof core.
The exact expiry second is excluded; a clock before issuance refuses. Expiry and positive
cooldown are explicit owner inputs; this storage slice selects no product TTL or quota.

## Receipts, unknown outcomes and future setup

The canonical request fingerprint binds the command action, complete workflow binding,
locators, secret digest and applicable expiry/cooldown/action fields. Trusted execution time is
excluded so an identical request can reconcile later. Reusing a command for a different payload
refuses. Statement/precondition failure rolls back; `CommitFault::Rejected` and `Unknown`
remain distinct. Reconcile the exact receipt after an uncertain effect before changing intent.

A receipt is an immutable historical shape-valid snapshot, distinct from the current workflow.
Replaying issuance after consumption returns its original Pending snapshot. It proves no current
mailbox ownership and grants no setup, password reset, assurance, ActorId, session or permission.

The transaction-local confirmation primitive exists for future composition. **CompleteSetup
must commit proof consumption and the authorized credential/setup effect in the same owner
transaction.** It must not call the separately committing lifecycle method and then grant from
a `Consumed` row, status lookup or receipt in a later transaction. Until that composition and
the mailbox-holder capability design exist, these methods are internal storage primitives and
the A-starts-for-B signup journey remains unaccepted. They expose no transport endpoint.

## Dependency and evidence boundary

The optional persistence feature reuses exact already-locked pins: `sha2 =0.10.9` (MIT/Apache-2.0),
`subtle =2.6.1` (BSD-3-Clause, no default features), and `zeroize =1.9.1` (MIT/Apache-2.0, no
default features, MSRV 1.85). The default/Kani core gains none of these dependencies. No password
KDF, Tonic or WebAuthn qualification is claimed.

Manual dependency review on 2026-10-21 used local pinned manifests/source and RustSec database
revision [`e6f94f37`](https://github.com/RustSec/advisory-db/commit/e6f94f3760d5e6c54c1b05f204002263bc025524).
The complete advisory tree listed no entries for subtle/zeroize or the reused digest,
crypto-common, block-buffer, cpufeatures/getrandom crates. Its sha2 advisory
[RUSTSEC-2021-0100](https://rustsec.org/advisories/RUSTSEC-2021-0100.html) affects the old 0.9.7
backend, not this pin. The historical zeroize_derive advisory is outside the enabled features.
This is a scoped manual review, not cargo-audit, a workspace advisory clearance or a crypto audit.
See the libraries' [hash API](https://docs.rs/sha2/0.10.9/sha2/),
[comparison contract](https://docs.rs/subtle/2.6.1/subtle/trait.ConstantTimeEq.html) and
[zeroization limits](https://docs.rs/zeroize/1.9.1/zeroize/).

The existing `cargo xtask verify turso` selects the feature-enabled service tests, including
run-owned file-backed lifecycle, rollback, concurrency, reopen and negative SQL/decoder evidence.
Pure planner tests remain separate. No mutable DB/WAL, secret or production fixture is committed.
Kani requires its actual D1 runner; these SQL/crypto tests do not establish it. Mail, login,
authenticated RPC, two-process E2E, restore rotation, real ENOSPC and legacy retirement remain
separate acceptance. Point lookups use authoritative indexed SQL; no cache/DBSP or performance
improvement is claimed, and representative runtime latency/footprint measurement is not supplied.
