# Identity, authentication and application-session boundaries

> **Scope.** Working method for the owner-selected
> [Server/Identity refactor](../../../../docs/architecture/native-turso-refactor.md) and later
> changes to credentials, verification, recovery, private authentication RPC or session
> admission. Product scope remains in that architecture and the owning contracts, not here.

Start with the [v1 owner amendment](../../../../docs/architecture/native-turso-refactor.md#owner-amendment-v1-authentication-and-parallel-integration)
for selected factors, deferred work, transitional DB composition and security-state strategy.
Then inspect the [implemented Identity/RPC boundary](../../../../services/cantos-identity/README.md)
the Server consumer in [Identity auth v1](../../../../services/cantos-server/contracts/auth/identity-auth/v1/README.md)
(PostgreSQL sessions; Studio password under [ADR 0045](../../../../docs/decisions/0045-v1-email-password-sign-in.md))
and actual client consumers; the Web clients consume the cookie endpoints from source and CMP the
native Bearer endpoints, while browser/device acceptance and Studio operator provisioning are pending.

## Read and model before changing authority

Trace the actual frontend, public handler, private RPC, credential verifier, account/actor
binding, session issuance and resource admission. Inventory each purpose and trust boundary.
Reuse existing typed IDs, lifecycle cores and error vocabulary before introducing a new port
or service framework. A decoded DTO, email match, known subject or stored actor row is not
proof of authentication; a snapshot fact is not a permit usable after its lease expires.

Keep Identity subjects/credentials/security state separate from Server ActorId/application
sessions/resource permissions. New local identity must not pretend to be an old OIDC issuer.
Studio login-only admission, Listener authority, dedicated Admin and work-scoped producer
capabilities remain distinct. Preserve frontend cookie/native transport decisions and session
policy unless their owner explicitly changes them. Do not restore old Web Bearer-only guidance.

## Library selection is not a security proof

Use maintained, reviewed libraries rather than implementing cryptographic primitives. Audit
exact versions/features, advisories, licensing, target/MSRV compatibility and dependency cost.
Use a password KDF with unique salts and upgradeable parameters; high-entropy one-time tokens
have different storage/verification needs from passwords. Never trade weak password hashing
for a smaller memory benchmark. Bound expensive verification concurrency and queues, execute
CPU-intensive work outside asynchronous I/O workers and database transactions, then recheck
account/security state before issuing success.

Secret wrappers/redaction and zeroization are defense in depth, not proof that no copies remain.
Test Debug/error/log/tracing paths with synthetic secrets. Do not persist or echo passwords,
provider tokens, private keys, reset/setup links or MFA secrets as ordinary diagnostics.
Encryption/recovery keys must be outside their protected database and have a restore policy.

## Registration, recovery and MFA

Model initiator A, mailbox owner B, scanners/prefetchers, concurrent requests and lost responses.
A entering B's email must not obtain B's setup capability or session after B verifies. Only
possession and explicit consumption of the correctly bound proof may advance setup. GET,
status polling, redirects and a client `verified` flag do not supply that proof.

Proofs need purpose/operation binding, entropy, expiry, one-winner consumption, replay/cancel/
resend semantics and safe unknown-outcome recovery. Existing account collisions must not become
permission to adopt/reset/link them. Generic outward messages and bounded attempt/send budgets
must be evaluated for enumeration, guessing and denial of service, including attacker-induced
account lockout. Use trusted time and explicit clock-skew assumptions.

For a selected higher-assurance operation, preserve its required factor; do not infer a new MFA
requirement for v1 from the legacy provider. When deferred factors/recovery enter scope, define
secure enrollment/re-enrollment, secret storage, replay windows and recovery for the chosen method.
A mailbox challenge or password match alone must not satisfy a higher-assurance operation.

Sending, mailbox delivery and consumption are separate facts. Production still needs an
approved mail transport; local tests use a run-owned private capture inbox. Durable outbox
storage of challenge material requires an explicit encrypted/TTL-bound design rather than
plaintext logging for retry. No generic SMTP server or cloud account is implicitly provisioned.
The [Firebase verification amendment](../../../../docs/architecture/firebase-verification.md)
changes the selected external email-action provider, not native authority. Inspect the distinct
capabilities before adapting a transport; keep native proof secrets out of continuation URLs and
require durable operation-bound provider-consumption evidence before native credential effects.
The implemented emulator lifecycle is separate from the still-unimplemented native proof bridge.

## Private RPC and separated databases

Treat generated protobuf fields as untrusted until the authenticated channel, method/purpose,
request binding, shape, freshness, subject and security version have been checked. Bound payload,
connect/request deadlines, concurrency and retries; restrict reflection/diagnostic surfaces.
UDS permissions/peer checks or network mTLS establish a caller boundary, not business authority.
Authentication and authorization for sensitive RPC methods both need negative tests.

A timeout/cancel does not establish remote rollback. Do not automatically replay a password
attempt, single-use proof or non-idempotent mutation. Receipt/operation identity and typed
unknown outcomes must support reconciliation without restoring a consumed capability.

Do not read the other service's database or claim cross-file atomicity. Provisioning/binding
must be idempotent and recoverable after either commit. Disable/security changes commit durable
monotonic security state. Under the owning v1 decision, direct authoritative checks may establish
freshness without an event consumer. Define outage/unknown-state refusal and the time between
check and admission. If an event path is used, commit its event with the state change; the session
owner atomically records its inbox/watermark and revocation before acknowledgement. Specify
deduplication, ordering, retention, catch-up and resync. A numeric field alone proves no protocol.

State the maximum revocation lag and freshness checks at issuance, renewal, protected and
sensitive operations. Unhealthy propagation cannot silently retain authority indefinitely.
Do not infer instant revocation from an asynchronous stream; default admission must not weaken
existing security without an explicit owning decision. Test disable against in-flight hashing,
RPC and session issue, late account-switch responses, direct-check outages/staleness, event
duplication when applicable and each restart order.
Restoring an older database must not silently reset security epochs and revive revoked sessions.

## Evidence and Kani

Start with pure production decisions over typed facts and independently stated outcomes.
Select critical kernels for bounded Kani proofs: proof purpose/expiry/consume-once, state
transitions, security/revision exhaustion, assurance admission and session/replay decisions.
The [formal method](mutation-and-formal.md#bounded-model-checking) owns assumptions, bounds,
production reach, cover goals, non-vacuity, unwinding and actual execution reporting. Extend
existing xtask registries; do not add a verifier runtime dependency or second proof runner.

Kani does not establish cryptographic correctness, library safety, concurrent database I/O,
transport authentication or browser behavior. Pair it with actual library vectors/verification,
real file-backed Turso races/crash/restart, private RPC negative tests, real API/session flows
and separately reported browser/native-device acceptance. Ordinary tests/compilation or the
presence of a harness is not `bounded-model-checked`.

Keep deterministic mock adapters, test inboxes, fixture keys and bypass routes outside production
artifacts. A full-flow test must receive a real API-confirmed session; seeding a session or
mocking `/session` only tests a double. Preserve A/B, cross-surface, replay, expiry, outage,
log-leak and backup/recovery regressions when replacing providers.

Primary references: [OWASP password storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html),
[recovery](https://cheatsheetseries.owasp.org/cheatsheets/Forgot_Password_Cheat_Sheet.html),
[Kani limitations](https://model-checking.github.io/kani/tool-comparison.html).
