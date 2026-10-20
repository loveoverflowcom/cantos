# Cantos Identity security core

This is the dependency-free library slice of [#334](https://github.com/loveoverflowcom/cantos/issues/334),
on `refactor/cantos-native-turso`. It has no executable, listener, database, credential verifier
or production consumer. It does not complete 334.1 or provide native authentication.

## Implemented boundary

| Kernel | Invariant | Ordinary oracle |
| --- | --- | --- |
| `SubjectId`, `InstanceEpoch`, `OperationId` | Distinct nonzero 128-bit namespaces, private representation; construction is shape validation only | Zero refusal, every single-bit position and maximum round-trip; compile-fail namespace/constructor examples |
| `SecurityVersion` | Positive value fitting a Turso INTEGER; successor refuses exhaustion without wrapping or mutation | Exact invalid/limit errors and wider-arithmetic boundary cases |
| `check_completion` | Started and current subject/epoch/version must agree; both accounts active; operation and surface identical; factor satisfies the selected login path | Literal refusals plus all 7,680 finite state/binding/factor combinations, including disable/re-enable and restored epochs |
| `MailboxProof` transition plans | Only explicit, currently bound, unexpired confirmation plans consumption; cancellation/consumption are terminal; replacement changes the locator after an explicit cooldown | Exact expiry/clock/storage edges, all binding changes, terminal/replacement refusals and 1,296 four-step traces on production symbols |

`Purpose` has separate Theatre and Studio cases. `Assurance` is categorical, with no ordering
or arithmetic. The Theatre password path admits `Password`; the Studio D3 path admits only
`UserVerifiedPasskey`. A passkey without user verification, mailbox proof or one recovery code
cannot complete either login path. A future additional path requires an explicit policy change;
these labels do not implement or verify any factor.

`check_completion` returns only a decision (`Result<(), CompletionDenied>`), never an
AuthenticationFact, setup capability or session. All inputs are facts that a future owner must
establish. Internal refusal reasons must become generic denial at the RPC/public boundary.
No password, token, email or credential is accepted, stored, serialized or logged by this crate.

## Mailbox-proof decision boundary

`ProofId` is a nonzero **public locator**, never a secret proof or digest. `ProofTime` is
nonnegative Unix seconds fitting an INTEGER; a constructor validates shape, not clock trust.
Issuance requires `issued_at < expires_at`. Confirmation admits `issued_at <= now < expires_at`.
The binding retains the operation, database instance epoch and intent. Theatre signup is
distinct from credential recovery, whose binding also retains subject, security version and
Theatre/Studio purpose. There is no Studio public-signup variant.

`plan_confirmation`, `plan_cancellation` and `plan_replacement` return `ProofTransition`
with the entire expected snapshot and successor. They commit nothing and issue no setup
capability, credential, AuthenticationFact or session. The candidate locator must come from a
real secret verifier; the current binding must come from authoritative workflow/account state,
not a decoded request or an email match. Recovery account eligibility and the later credential
change require their own current-state check. A mailbox confirmation does not satisfy Studio
passkey assurance or authorize a password reset by itself.
The pending workflow must keep its delivery mailbox immutable for that operation; changing
the address requires a new operation and proof. Email parsing/canonicalization is not supplied
by a public locator or by this binding type.

GET/prefetch/polling/provider-status observation maps to `Inspect`, which cannot plan
consumption. Only an adapter-established explicit confirmation action may advance it;
the enum is not authenticated HTTP evidence. In particular, learning that B confirmed does
not give initiating browser A a setup capability. The full A-starts-for-B journey remains an
unrun application/transport acceptance test.

Cancellation ends this challenge's operation. A consumed/cancelled snapshot cannot confirm,
cancel again or resend; a later flow needs a new operation. An expired pending challenge can
be replaced after the owner-supplied positive cooldown, including its exact boundary. The
replacement keeps the binding, starts at the supplied `now`, has a strictly later expiry and
requires a different locator. Old-locator completion/cancellation/replacement is refused.
TTL, rate/send/account/IP budgets and retention remain explicit future owner policy.

The persistence owner must enforce fresh secret/digest generation and lifetime locator
uniqueness, validate stored terminal rows (never hydrate everything with `issue`), and compare
the entire expected snapshot under the shared #332 transaction guard. Replacing that record,
consuming the real proof and recording its operation receipt must be atomic with the effect
they authorize. Two copies of a plan are **not** two admissible commits. Reconcile lost/unknown
commit outcomes through receipts; never treat a timeout as rollback or reopen Pending.
The sequenced pure-state tests do not demonstrate real races, durable consume-once or recovery.

## Shell obligations and prerequisites

The base for the mailbox-proof slice is integration `26d957b37f6d6de57d86a75ee4614d4491611464`,
containing #337 (rename), #338 (pre-composition regressions) and #339 (security-state core). It lacks #330's
reconciliation, #332's shared Turso owner/migrations, Identity RPC draft and qualified dependency
pins. This slice supplies no substitute for those prerequisites.

After #332, the Identity owner must:

- generate subject/operation IDs and a fresh `instance_epoch` on DB create/restore, and read
  snapshots from authoritative storage; arbitrary decoded/client values are not authority;
- verify the real credential/factor, trusted caller/method, purpose, expiry and request binding;
- hash outside transactions/async executors with bounded concurrency, then obtain current
  security state and serialize the final check with security-changing writes;
- atomically commit credential/state changes, version increments and security events through
  the shared Turso transaction owner; the pure successor establishes no durable event;
- enforce one-use proof/operation receipts and reconcile unknown outcomes before replay;
  this stateless check cannot provide consume-once, a freshness lease or race-free session issue;
- preserve Server ActorId/bindings/permissions; no email auto-link or credential export.

Email/password/secret-proof checked types, actual mailbox verification/signup/recovery, Argon2id, Studio passkey
ceremonies and one-use recovery codes, events/receipts/schema, mail-port relocation, private
gRPC/UDS authentication, footprint and end-to-end acceptance remain later #334 slices.
Actor/session/HTTP/frontend integration belongs to #335. No server core or mail implementation
is copied or moved before its replacement is ready.

## Verification

Normal workspace `cargo xtask verify rust` includes this library. Preflight discovers its
package through Cargo metadata; an Identity-only source diff selects its format/lint/tests.
Shared Cargo/tooling changes still broaden preflight normally.

Focused development checks use the same Cargo primitives:

```sh
cargo test --locked -p cantos-identity
cargo clippy --locked -p cantos-identity --all-targets -- -D warnings
cargo xtask preflight --changed-only --base origin/refactor/cantos-native-turso
```

Three optional registered Kani kernels call production symbols:

```sh
cargo xtask verify kani --harness identity-security-version --harness identity-completion --harness identity-mailbox-proof
```

The version model uses full raw `u64` and a wider `u128` oracle with no assumptions. The
completion model covers full nonzero 128-bit IDs, valid storage versions and all state/surface/
factor cases; its assumptions are exactly the checked constructors' domains. Both use unwind 2,
no stubs and explicit accepted/refused/boundary covers (6 and 10). A version/ID change selects
all three kernels. Existing Kani/compiler/solver/target pins and proof checks are retained.

The mailbox-proof harness calls production constructors/planners with a full raw `u64`
clock (including invalid storage values), full nonzero IDs, valid security versions and a
nonempty storage-bounded window. Constructor-domain assumptions, unwind 2, no stubs and 16
covers are recorded in [the ledger](../../docs/assurance/kani.md#native-identity-kernels-registered-not-yet-run).
It checks exact binding/intent/time admission, sequenced terminal refusals and replacement
of an old locator. Its proposition excludes possession, cryptography, HTTP actions, clock trust,
concurrent commits or durable replay. Shared ID/version inputs select all three Identity
kernels; admission's `Purpose` also selects mailbox-proof; mailbox sources select that kernel.

Kani evidence is **NOT_RUN** until an actual pinned Ubuntu x86_64 D1 workflow produces a run
URL, exact source SHA and complete assertion/unwinding/cover output. macOS tests and registry/
adapter tests do not establish `bounded-model-checked`. Real Turso, cryptographic and RPC
evidence is separate and not supplied by this library slice.
