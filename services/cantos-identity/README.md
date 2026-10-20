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

`Purpose` has separate Theatre and Studio cases. `Assurance` is categorical, with no ordering
or arithmetic. The Theatre password path admits `Password`; the Studio D3 path admits only
`UserVerifiedPasskey`. A passkey without user verification, mailbox proof or one recovery code
cannot complete either login path. A future additional path requires an explicit policy change;
these labels do not implement or verify any factor.

`check_completion` returns only a decision (`Result<(), CompletionDenied>`), never an
AuthenticationFact, setup capability or session. All inputs are facts that a future owner must
establish. Internal refusal reasons must become generic denial at the RPC/public boundary.
No password, token, email or credential is accepted, stored, serialized or logged by this crate.

## Shell obligations and prerequisites

The fresh base for this slice is integration `5e5a526b6517dfd6c8e82b52430d1d5591c0d1f0`,
containing merged #337 (rename) and #338 (pre-composition regressions). It still lacks #330's
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

Email/password/proof checked types, mailbox ownership/signup/recovery, Argon2id, Studio passkey
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

Two optional registered Kani kernels call production symbols:

```sh
cargo xtask verify kani --harness identity-security-version --harness identity-completion
```

The version model uses full raw `u64` and a wider `u128` oracle with no assumptions. The
completion model covers full nonzero 128-bit IDs, valid storage versions and all state/surface/
factor cases; its assumptions are exactly the checked constructors' domains. Both use unwind 2,
no stubs and explicit accepted/refused/boundary covers (6 and 10). A version/ID change selects
both dependent kernels. Existing Kani/compiler/solver/target pins and proof checks are retained.

Kani evidence is **NOT_RUN** until an actual pinned Ubuntu x86_64 D1 workflow produces a run
URL, exact source SHA and complete assertion/unwinding/cover output. macOS tests and registry/
adapter tests do not establish `bounded-model-checked`. Real Turso, cryptographic and RPC
evidence is separate and not supplied by this library slice.
