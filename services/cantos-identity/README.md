# Cantos Identity

This service contains the default dependency-free decision core, an opt-in `persistence` shell and,
behind the opt-in `service` feature, the **`cantos-identity` executable**: Theatre signup with a
mailbox proof, Argon2id password credentials and `AuthenticatePassword` over a private
authenticated Unix-socket RPC. It is part of [#334](https://github.com/loveoverflowcom/cantos/issues/334)
on `refactor/cantos-native-turso`; it is **not** the whole of #334 and it is not a Server session.

The [owner's v1 amendment](../../docs/architecture/native-turso-refactor.md#owner-amendment-v1-authentication-and-parallel-integration)
and [ADR 0045](../../docs/decisions/0045-v1-email-password-sign-in.md) supersede the older
factor/dependency target. The sections below describe actual source, including the Studio
password path that ADR 0045 enabled.

* Implemented and run against the real engine: signup start, explicit mailbox-confirmed setup,
  password authentication, restart, replay/unknown-outcome reconciliation, disable race, the
  RPC boundary and the executable (see [Verification](#verification) for exactly what ran).
* Not implemented: Studio provisioning, passkey and recovery codes; credential recovery,
  change-password, second factors, disable/re-enable RPC; restore epoch rotation; the security-event
  consumer; network mode. First-party Web/CMP consumers exist, with browser/device acceptance
  separate. The Server consumer (private client, fresh security-state reads, sessions and ActorId
  binding on Turso) is the
  [Identity auth v1 contract](../cantos-server/contracts/auth/identity-auth/v1/README.md).
* The end point is an **authentication fact** of Identity. It is neither an application session nor
  authorization of any resource. See the [RPC contract](contracts/identity/v1/README.md).

## Implemented boundary

| Kernel | Invariant | Ordinary oracle |
| --- | --- | --- |
| `SubjectId`, `InstanceEpoch`, `OperationId` | Distinct nonzero 128-bit namespaces, private representation; construction is shape validation only | Zero refusal, every single-bit position and maximum round-trip; compile-fail namespace/constructor examples |
| `SecurityVersion` | Positive value fitting a Turso INTEGER; successor refuses exhaustion without wrapping or mutation | Exact invalid/limit errors and wider-arithmetic boundary cases |
| `check_completion` | Started and current subject/epoch/version must agree; both accounts active; operation and surface identical; factor satisfies the selected login path | Literal refusals plus all 7,680 finite state/binding/factor combinations, including disable/re-enable and restored epochs |
| `MailboxProof` transition plans | Only explicit, currently bound, unexpired confirmation plans consumption; cancellation/consumption are terminal; replacement changes the locator after an explicit cooldown | Exact expiry/clock/storage edges, all binding changes, terminal/replacement refusals and 1,296 four-step traces on production symbols |

`Purpose` has separate Theatre and Studio cases. `Assurance` is categorical, with no ordering
or arithmetic. The Theatre password path admits `Password`. Studio admits `Password` too (the
[#335 v1 decision](../cantos-server/contracts/auth/identity-auth/v1/README.md#owner-decision-recorded-here-v1-scope-amendment);
Identity keeps no Studio roster, which is the Server's) and the deferred D3 `UserVerifiedPasskey`
path. A passkey without user verification, mailbox proof or one recovery code cannot complete
either login path. A future additional path requires an explicit policy change;
these labels do not implement or verify any factor.

`check_completion` returns only a decision (`Result<(), CompletionDenied>`), never an
AuthenticationFact, setup capability or session. All inputs are facts that a future owner must
establish. Internal refusal reasons must become generic denial at the RPC/public boundary.
The default core accepts no password, secret token, email or credential. The optional shell
below accepts bounded proof secrets and persists only their digests; it logs no secret.

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
not give initiating browser A a setup capability. The A-starts-for-B journey is now exercised at
the application layer and through the executable (see [Signup and authentication](#signup-and-authentication));
the HTTP/browser leg belongs to #335.

Cancellation ends this challenge's operation. A consumed/cancelled snapshot cannot confirm,
cancel again or resend; a later flow needs a new operation. An expired pending challenge can
be replaced after the owner-supplied positive cooldown, including its exact boundary. The
replacement keeps the binding, starts at the supplied `now`, has a strictly later expiry and
requires a different locator. Old-locator completion/cancellation/replacement is refused.
TTL, rate/send/account/IP budgets and retention remain explicit future owner policy.

The persistence owner enforces fresh secret/digest generation and lifetime locator
uniqueness, validates stored terminal rows (never hydrating everything with `issue`), and compares
the entire expected snapshot under the shared #332 transaction guard. Replacing that record,
consuming the real proof and recording its operation receipt must be atomic with the effect
they authorize. Two copies of a plan are **not** two admissible commits. Reconcile lost/unknown
commit outcomes through receipts; never treat a timeout as rollback or reopen Pending.
The sequenced pure-state tests do not demonstrate real races, durable consume-once or recovery.

## Historical mailbox-proof base and remaining shell obligations

The base for the mailbox-proof slice is integration `26d957b37f6d6de57d86a75ee4614d4491611464`,
containing #337 (rename), #338 (pre-composition regressions) and #339 (security-state core). It lacks #330's
reconciliation, #332's shared Turso owner/migrations, Identity RPC draft and qualified dependency
pins. The shared Turso owner is now accepted and the optional security-state shell below
consumes it. The mailbox-proof core itself supplies no substitute for the remaining prerequisites.

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

Signup, Argon2id, the credential schema, the immutable mailbox binding, the mail port and the private
authenticated UDS RPC are implemented in the `service` feature below. Credential recovery,
change-password, Studio passkey ceremonies and one-use recovery codes, restore rotation, the
mail-port relocation out of the Server and footprint measurement remain later #334 slices.
Actor/session/HTTP/frontend integration belongs to #335. No server core or mail implementation
is copied or moved before its replacement is ready.

## File-backed security state

The optional `persistence` feature supplies `IdentityStore` over the existing
[`cantos-store-turso`](../../packages/store-turso/README.md) `OwnedDatabase`. It uses
its service-owned `IDENTITY_APPLICATION_ID` (`CNTI`, `0x434E5449`), its own checksummed epoch-1
migrations and the same exclusive file lease, serialized writer, trusted clock and commit-outcome types; it adds no engine wrapper,
cross-service file access or second database owner.

The [storage contract](docs/security-store-v1.md) owns the exact
scope and compatibility rules. The schema has instance metadata, checked subject/security state,
ordered events and immutable operation receipts. `Intent::Migrate` may initialize a fresh random
instance epoch only when every service table is empty; `Serve` refuses missing metadata and
ordinary reopen retains the existing epoch. A partial first initialization is resumed only by
explicit migrate. Missing metadata alongside data is corruption, never a fresh installation.

The two internal owner-authorized mutations are subject creation and Active→Disabled. They
require the expected instance epoch. Disable also requires the observed security version and
uses the existing checked successor. Subject state, version, next event and bound receipt commit
in one transaction. New operation IDs cannot resurrect a disabled subject, transfer a subject or
reuse an ID with a different kind/subject/expected version. A repeated operation returns its
original receipt, which is deliberately not a current Active snapshot or authentication fact.
Reading security state is separate from reading an old receipt.

These library methods do not authorize their caller, provision a live account, verify any
credential, create an ActorId or issue Studio/Admin/resource authority. No public transport calls
them. The default core and Kani commands leave `persistence` off; file-backed test fixtures are
test-only. The storage feature is verified explicitly through the existing Turso gate and Rust CI.

`CommitFault::Unknown` is preserved. Reconcile the exact operation/epoch/binding through the
durable receipt before interpreting or retrying an uncertain effect. Real tests discard a
successful post-commit response, reopen the file and replay without another event; that is not
an injected engine COMMIT-Unknown result. Engine uncertainty classification remains the shared
owner's separately tested boundary. Trigger-injected event failure proves partial subject writes
roll back. Initialization commit uncertainty has its own typed outcome and requires reopening
instance metadata, since this startup step has no subject-operation receipt. Concurrent CAS/replay,
immutable guards, bounded event pages, integer exhaustion,
wrong-epoch/foreign-owner refusal and reopen have real pinned-engine tests.

The signup extension below persists the credential. This shell does not implement general
recovery/re-enable/delete policy, rotate an epoch on restore, provide authenticated RPC, consume events in Server
or establish revocation propagation/login. A copied old file cannot identify its own restore;
the explicit restore rotation and Server resync remain #334/#335/#336 acceptance, as do crash,
true ENOSPC, browser/device, footprint and actual D1 Kani proof evidence.

## File-backed mailbox proofs

The [internal mailbox storage contract](docs/mailbox-store-v1.md)
adds migration 2 to the same epoch-1 registry and shared owner. It retains migration 1 unchanged.
Its immutable issue history preserves locator/digest uniqueness; a workflow stores its complete
original binding/current proof, and a separate command namespace binds immutable receipts.
Checked hydration retains Pending, Consumed and Cancelled states and refuses malformed rows.

`ProofMaterial::generate` creates an OS-random locator and 32-byte secret. The secret is
non-cloneable, redacted and zeroized on drop, with explicit exposure for a future mail adapter.
The file stores only a domain-separated digest. Issue/replace accept retained material so
unknown-outcome reconciliation never silently generates a different secret. No durable mail
outbox, delivery or secret recovery is supplied; a crash can lose undispatched secret material.

The owner checks current locator/digest, explicit action, trusted transaction time, original
epoch and recovery subject/version before a new transition. Existing pure plans feed one
conditional update plus receipt transaction. Exact retries return historical receipts, with
all payload fields bound; they grant no current verified status, setup capability or session.
Proof confirmation alone is a storage primitive, not a signup. `complete_signup` composes it with
the account, email and credential in the same transaction through the transaction-local
primitive; nothing grants from a separately committed row or receipt.

The existing feature-enabled Turso gate includes real-file proof tests. The default pure core
and Kani feature selection are unchanged. No live account, mail, legacy DB or runtime is changed.

## Signup and authentication

Migration 3 (`signup.sql`, append-only; migrations 1 and 2 are untouched) adds `signup_workflows`
(the immutable mailbox binding of an operation), `identity_emails` (the verified address of a
subject), `password_credentials` (an Argon2id PHC string and a credential version) and
`signup_completions`. All four reject UPDATE and DELETE by trigger.

```
StartTheatreSignup(op, email)  ──►  pending signup + proof digest (one tx)  ──►  mail adapter
                                                                                  │ (proof secret exists only here)
mailbox owner opens the link, chooses a password, confirms explicitly            ◄┘
CompleteTheatreSignup  ──►  hash (bounded pool, no tx)  ──►  ONE tx: re-read proof/binding/email,
                            consume proof + subject + verified email + credential + event + receipt
AuthenticatePassword   ──►  read record ──► verify (bounded pool, no tx) ──► final check under the
                            writer lock (state, version, epoch, credential version) ──► fact
```

* **The initiator cannot set the password.** `StartTheatreSignup` accepts no password and returns
  no proof; the proof goes only to the bound mailbox. The completing party supplies their own
  password. The email is read from the stored workflow inside the completing transaction, never
  from the request.
* **Hashing is outside every transaction and every async executor thread** and runs through
  `KdfPool`: a fixed number of workers, a bounded waiting line with a deadline, and the permit is
  moved *into* the blocking closure, so a timed-out or cancelled request cannot free capacity for
  work that is still running. Stored hashes are checked against the policy floor and ceiling
  before any KDF allocation. Passwords are never trimmed, truncated or normalized.
* **Receipts are history.** Replaying `CompleteTheatreSignup` with the identical command returns
  the original subject only if the supplied password verifies against the credential that
  command created; a changed password, proof or operation is refused. No receipt, poll or status
  read grants a credential, a session or an authentication fact. No fast password digest is stored.
* **Start and resend are one decision.** Without a mail transport every start is refused before
  any read or write. Otherwise the registered-address check, the operation's immutable email
  binding, its current proof and the resend cooldown are read and applied inside one writer
  transaction (`start_or_resend_signup`, deciding with the pure `domain::signup_start`), so
  concurrent starts of one operation produce one issued or replaced proof and never a conflict.
  Mail is dispatched only for that committed `Issued`/`Replaced` outcome.
* **Mail is separate from consumption.** The proof is written (digest only) before mail is sent,
  outside the transaction. A definite relay refusal and an uncertain answer both leave the proof
  pending (a refused one is unreachable because nobody holds its secret) and are answered like
  every other start; SMTP acceptance never asserts delivery. There is **no durable mail outbox**: a crash after
  the write loses the unsent secret, and recovery is an explicit re-start of the same operation after
  the resend cooldown (which replaces the proof and retires the old one) or a new operation. "Exactly
  once delivery" is not promised.
* **Existing email** is never reset, linked or adopted: a start for a registered address sends no
  mail and answers identically (status and body; timing is not equalized, see the contract), and a completion that races onto an address already registered
  rolls back whole and is denied.
* **Abuse budgets** (in-memory, bounded, reset on restart) key on salted hashes of the email and of
  the client address the authenticated Server asserted. Login failures throttle per (account,
  address) and per account across addresses. There is no permanent lock, but a distributed
  attacker rotating addresses (an IPv6 /64 each) can keep one account throttled for as long as it
  keeps failing, and a third party can likewise delay signup for a known address for up to an hour
  (the same budget caps mail volume). The account-wide ceiling is therefore a **review item**.
  Budget keys come from the client address the authenticated Server asserts: Identity trusts that
  value completely. The table evicts the oldest window with an O(n) scan under one mutex when full.
* Unknown email, wrong password, disabled account, stale credential and an unusable stored hash
  are one `Unauthenticated` answer; an unknown email still pays one real KDF verification against
  a dummy hash. **Equal timing is not claimed.**

## Executable

`cantos-identity migrate` creates or upgrades `identity.db` and exits; `cantos-identity serve` never
creates or migrates a database and refuses a missing, foreign, corrupt or older-schema file without
touching it. Configuration is entirely environment variables, each required value explicit:

| Variable | Meaning |
| --- | --- |
| `CANTOS_IDENTITY_DATABASE` | absolute path of the database file (its directory must be private to the owner) |
| `CANTOS_IDENTITY_SOCKET` | absolute path of the Unix socket (its directory must be owned by this user, no access for others, no group write) |
| `CANTOS_IDENTITY_SOCKET_MODE` | `0600` (default) or `0660` |
| `CANTOS_IDENTITY_SERVER_UIDS` | comma list of 1–8 uids allowed to call (the Theatre Server) |
| `CANTOS_IDENTITY_SIGNUP_LINK_BASE` | `https` front-end address the mailed link opens |
| `CANTOS_IDENTITY_SMTP_HOST/PORT/TLS/FROM[/USERNAME/PASSWORD]` | optional; `TLS` is `starttls`, `implicit` or `loopback-plaintext` (literal loopback address, no authentication) |

`starttls`/`implicit` relay TLS depends on a `rustls` chain that carries open advisories on the base
branch (see the [qualification record](../../docs/evidence/identity-rpc-dependency-qualification.md#advisory-scan));
do not accept those modes for production until the lock-only bump lands. Without SMTP configuration the process starts, reports `"mail":"missing"` in its single readiness
line on stdout and answers every `StartTheatreSignup` with `Unavailable` and records nothing: it fails closed. Stdout/stderr
carry only fixed event strings. SIGTERM/SIGINT stops accepting and drains in-flight requests for at
most ten seconds, then removes only the socket inode this run created. An existing socket path is
replaced only when it is a socket that nobody answers on. One owner per database file is enforced by
the shared store.

## Policy values and decisions

| Value | Setting | Why / test | Review |
| --- | --- | --- | --- |
| Proof lifetime | 900 s | Limits a leaked link; exact expiry second tested | **NEW**, owner review |
| Resend cooldown | 60 s after issue | Replaces (and retires) the proof; boundary tested | **NEW** |
| Argon2id | m=19456 KiB, t=2, p=1, 32 B out, 16 B salt | OWASP minimum; RFC 9106 library vector; floor/ceiling refusal tests | accepted by #334 text |
| KDF concurrency / queue / wait | 2 / 6 / 5 s (nominal peak about 38 MiB; a planted hash is admitted up to 64 MiB × 2 workers) | Burst, cancel and panic tests | **NEW** |
| Password length | 8–128 scalars, ≤512 bytes, no composition rules | NIST-style length-only policy | **NEW** |
| Email | ASCII, no trimming, no quoted local part; domain lowercased; local part case preserved | The #334 canonical rule; two spellings are two addresses | issue text; EAI refused |
| Budgets | start 3/h per email, 20/h per address; complete 20/15 min per address, 10/15 min per (operation, address); login failures 5/15 min per (account, address), 40/15 min per account; 60/15 min per address | `application.rs` throttle tests | **NEW** |
| RPC | 8 KiB messages, 20 s deadline, 16 connections × 4 streams | oversize and burst process tests | **NEW** |

Decisions recorded for review (none is marked "owner approved"):

| Decision | IF | Chosen | Test | Status |
| --- | --- | --- | --- | --- |
| Setup shape | no consumer fixed two steps | one `CompleteTheatreSignup` command | no-capability journey | contract draft, #335 to accept |
| Mail adapter | Server's adapter is typed to its registration domain | Identity-owned adapter on the same pinned `lettre`; the Server copy stays until #335 retires its Kanidm path | mail adapter tests | owner to confirm the interim duplicate |
| Durable mail outbox | none approved | none; explicit resend | restart/cooldown tests | **OWNER_DECISION_REQUIRED** for an encrypted outbox |
| Loopback plaintext SMTP | the journey needs a run-owned sink | `loopback-plaintext` accepted only for a literal loopback host without credentials | config tests | owner to confirm it may ship |
| Socket group access | a Server under another uid | `0660` option plus uid allowlist | process denial test | configuration only |

## Acceptance annotations

Searching `ACCEPTANCE-PENDING` in the source lists every place whose real-infrastructure check is
still open: restore epoch rotation, the security-event consumer (#335), the front-end link and
HTTP leg (#335), the D1 Kani proof, production SMTP delivery, and footprint measurement.

## Verification

Normal workspace `cargo xtask verify rust` includes this library. Preflight discovers its
package through Cargo metadata; an Identity-only source diff selects its format/lint/tests.
Shared Cargo/tooling changes still broaden preflight normally.

Focused development checks use the same Cargo primitives:

```sh
cargo test --locked -p cantos-identity
cargo clippy --locked -p cantos-identity --all-targets -- -D warnings
cargo test --locked -p cantos-identity --features persistence
cargo test --locked -p cantos-identity --features service
cargo clippy --locked -p cantos-identity --features service --all-targets -- -D warnings
cargo run --locked --manifest-path tools/proto-codegen/Cargo.toml -- --check
cargo xtask verify turso
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
adapter tests do not establish `bounded-model-checked`. This work changes no Kani kernel (only
shared Cargo/module files that select them); the persistence, credential and RPC layers have
separate real Turso, Argon2id and process tests, reported independently of Kani.
