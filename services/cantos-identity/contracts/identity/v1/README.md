# Identity private RPC v1 (draft)

Authority: [identity.proto](identity.proto), package `cantos.identity.v1`, owned by Identity
([ADR 0046](../../../../../docs/decisions/0046-service-owned-contracts.md)). Each side generates
its own binding: Identity its server ([`rpc/generated.rs`](../../../src/rpc/generated.rs), behind
[`rpc.rs`](../../../src/rpc.rs)), the Server its client
([`identity_auth/rpc`](../../../../cantos-server/src/platform/identity_auth/rpc.rs), which also owns
`validate_fact`). Neither imports the other's crate. This is the #332 prerequisite slice
delivered with [#334](https://github.com/loveoverflowcom/cantos/issues/334). It is a **draft**. The
Server's client ([`identity_auth`](../../../../cantos-server/src/platform/identity_auth/client.rs),
[contract](../../../../cantos-server/contracts/auth/identity-auth/v1/README.md)) consumes it: field numbers are frozen and any removed number
**and name** must be reserved.

Transport is gRPC on a Unix domain socket owned by the Identity process. The caller is
authenticated from the OS peer credential (see [Caller authorization](#caller-authorization));
nothing in a message names or elevates a caller. There is no TCP, no reflection and no health
service that exposes the schema. A future network mode must use mTLS and is not part of this draft.

## Methods

| Method | Purpose | Response carries |
| --- | --- | --- |
| `StartTheatreSignup` | Record a pending signup bound to the exact mailbox and mail it a proof | `accepted` (always true for a well-formed request) and the fixed `proof_ttl_seconds`; **never** the proof, a setup capability or an account fact |
| `CompleteTheatreSignup` | The mailbox owner presents the mailed proof, the password they chose and an explicit confirmation | `subject_id`, `instance_epoch`, `COMPLETED` or `REPLAYED` |
| `AuthenticatePassword` | Verify email + password for `PURPOSE_THEATRE` or `PURPOSE_STUDIO` | an `AuthenticationFact` |
| `GetSecurityState` | Read a subject's state, security version and instance epoch | state, `security_version`, `instance_epoch` |

There is no provisioning, recovery, change-password or operator method: none is declared, so none
can be called. **#335 v1 amendment** ([ADR 0045](../../../../../docs/decisions/0045-v1-email-password-sign-in.md)): `PURPOSE_STUDIO` is accepted with `ASSURANCE_PASSWORD` for
`AuthenticatePassword`; the fact echoes the purpose it was asked for. Identity keeps no Studio
roster: the Server's roster decides who may enter Studio, and a Studio fact grants nothing by
itself. A passkey remains a separate, deferred assurance.

### One-command setup (decision)

No consumer had fixed a two-step `ConfirmMailbox → CompleteSetup` shape, so the contract uses the
single command preferred by the requirement: `CompleteTheatreSignup` consumes the mailbox proof
and creates the account, verified email and password credential in **one** storage transaction.
There is therefore no setup capability to leak, and the password is chosen by the party that
presents the proof, never by the initiator of the signup. A receipt, a status poll or a `REPLAYED`
answer is history, not authority, and is not accepted anywhere as proof of a current account.

### Retry, replay and unknown outcomes

`command_id` names one `CompleteTheatreSignup`. After a lost response, retry the **identical**
command: the original subject comes back as `REPLAYED` with no new effect, but only if the supplied
password verifies against the credential that command created. The same `command_id` with a
different proof, operation or password is refused. A `Code::Unknown` status means the commit
outcome is unknown: reconcile with the identical command, never with a new one. `AuthenticatePassword`
and `StartTheatreSignup` have no automatic retry; each creates no durable effect that needs one
(a start may be repeated, see the cooldown below).

## Authentication fact

`AuthenticationFact` carries `subject_id`, `purpose`, `assurance`, `authenticated_at_unix_micros`,
`security_version`, `instance_epoch` and the caller's `operation_id`. It carries no password
hash, proof, ActorId or role.

* It is **not** a session and grants no resource access. A decoded fact is untrusted input:
  [`validate_fact`](../../../../cantos-server/src/platform/identity_auth/rpc.rs) checks shape, operation and purpose binding, and only
  then yields a `ValidatedFact`. #335 owns the freshness window, subject-to-actor binding and
  session issue.
* `authenticated_at_unix_micros` is the trusted Identity clock at the **final state check**: under
  the single serialized writer, after the password verification, Identity re-read the account state,
  security version, instance epoch and credential version and found them unchanged. It makes no
  promise about any later instant: a disable that commits afterwards is visible to #335 only through
  `GetSecurityState` or the (future) security-event consumer. Before issuing a session, #335 must
  compare the fact's `security_version` and `instance_epoch` with a fresh `GetSecurityState`.
* The same `operation_id` may be answered twice; consuming a fact once is the Server's job.

## Errors

Messages are fixed strings; no status ever interpolates request content or an internal reason.

| Code | Meaning |
| --- | --- |
| `INVALID_ARGUMENT` | Malformed id, address, purpose, password length, or missing explicit confirmation |
| `UNAUTHENTICATED` | `AuthenticatePassword` denial: unknown email, wrong password, disabled account, stale credential or unusable stored hash are indistinguishable |
| `PERMISSION_DENIED` | Signup completion denial (wrong/expired/consumed/cancelled/replaced proof, unknown operation, email already registered, all indistinguishable) **or** an unauthorized caller |
| `RESOURCE_EXHAUSTED` | A rate budget is spent; `retry-after-seconds` metadata |
| `UNAVAILABLE` | Storage unavailable, **no mail transport configured**, or KDF capacity exhausted; nothing was changed (without a transport a `StartTheatreSignup` records nothing at all) |
| `UNKNOWN` | Commit outcome unknown: retry the identical command |
| `ABORTED` | A `CompleteTheatreSignup` command id was reused with different content. `StartTheatreSignup` never returns it, including under concurrent starts of one operation: an operation bound to another address or already concluded gets the generic answer |
| `UNIMPLEMENTED` | A method this build does not declare |
| `NOT_FOUND` | `GetSecurityState` for an unknown subject (a Server-only call) |

`StartTheatreSignup` returns the same status and body whether the address already has an account,
whether the operation was reused for another address or is already concluded, whether mail was
attempted and whether the relay accepted, refused or lost it. The only differing status is
`UNAVAILABLE` when **no** mail transport is configured, which is a deployment-wide condition and
not a per-address signal: it is checked before any lookup or write, so a registered and an
unregistered address, a fresh and a reused operation all get that same refusal and nothing durable
is recorded (no workflow, proof or receipt). **Timing is not equalized:** an unregistered address costs two writes and
an SMTP exchange that a registered one does not, so a timing observer can still tell them apart;
the per-address and per-client budgets bound how often it can be asked. A relay-refused message
leaves its proof pending but unreachable (nobody holds the secret); the same operation may be
re-started after the 60 s cooldown, which replaces the proof. The registered-address check, the
operation's immutable email binding, its current proof and the cooldown are decided together in one
store transaction, so concurrent starts of one operation yield one issued (or replaced) proof and
one message, never a conflict; mail is sent only after that transaction commits.

## Limits

Each message is bounded at 8 KiB (decode and encode). Fields: ids 16 bytes (nonzero), proof secret
32 bytes, email at most 254 bytes ASCII, password 8–128 Unicode scalars and at most 512 bytes for a
new credential (a login password may be shorter but not longer), client address at most 45 bytes.
The server also applies a per-request deadline, per-connection stream and connection ceilings, a
bounded KDF worker pool with a bounded waiting line, and the rate budgets in the
[policy table](../../../README.md#policy-values-and-decisions).

## Caller authorization

The server reads the peer credential (`SO_PEERCRED`/`LOCAL_PEERCRED`) of each Unix connection and
compares its uid with the configured allowlist of Theatre Server uids. A connection from any other
uid is closed before a byte is read; every handler checks again and refuses a request that carries
no credential. There is a single role in this build (Theatre Server); every declared method belongs
to it. `client.client_ip` is an assertion by that authenticated caller under its own trusted-proxy
policy: Identity uses it only to key abuse budgets and never reads a header or any other client
claim. A host without peer credentials is unsupported (compile error), never a bare-TCP fallback.

## Mailed link (draft, #335 consumes it)

The mail carries `<SIGNUP_LINK_BASE>#operation=<hex>&proof=<hex>&secret=<hex>`. The secret rides
in the fragment so a GET, prefetch or link scanner never transmits it to any server; only the
front-end page reads it and, on the owner's explicit action, sends `CompleteTheatreSignup` through
the Server. Opening the link consumes nothing. This format is not yet implemented by any front end.

## Regeneration

Every binding is checked in and owned by its consumer: Identity's server binding, the Server's
client binding, and each side's test peer (Identity's process-test client, the Server's scripted
Identity server). Regenerate or drift-check all of them with the pinned, workspace-isolated
generator (it runs a vendored protoc; no normal build does). `cargo xtask verify contracts`
runs the check inside `protobuf-reference`:

```sh
cd tools/proto-codegen
cargo run --locked            # rewrite every consumer's binding from identity.proto
cargo run --locked -- --check # fail if any binding differs from a fresh generation
```

[Wire goldens](fixtures/wire-goldens.json) were produced by an independent byte-level encoder
written from the protobuf encoding specification, not by prost. Identity's
[`tests/rpc_wire.rs`](../../../tests/rpc_wire.rs) and the Server's client tests both encode and
decode them with their own bindings.
