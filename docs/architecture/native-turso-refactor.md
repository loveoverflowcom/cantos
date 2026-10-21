# Native Server and Identity with embedded Turso

## Decision and evidence boundary

The product owner selected this target on 20 October 2026. [Epic #331](https://github.com/loveoverflowcom/cantos/issues/331)
owns execution and acceptance; its five checkpoints are [foundation #332](https://github.com/loveoverflowcom/cantos/issues/332),
[Server persistence #333](https://github.com/loveoverflowcom/cantos/issues/333),
[Identity #334](https://github.com/loveoverflowcom/cantos/issues/334),
[integration #335](https://github.com/loveoverflowcom/cantos/issues/335) and
[qualification/cutover #336](https://github.com/loveoverflowcom/cantos/issues/336).

This is an **owner-selected architecture direction, not implemented-runtime evidence**.
The reviewed baseline is `develop@cccb547c08585e1266e26b2b07ead3a2c887697e`.
At that baseline the service is still `services/cantos-api`, PostgreSQL is authoritative,
Kanidm supplies authentication, and the VPS uses container-oriented assets with separate
Studio and Theatre API processes. The HTTP adapter is already `adapters/inbound/http` after
PR #329. Re-read actual source before implementation; do not restore obsolete HTTP ownership.

**Implemented by #332 (checkpoint 1/5):** the crate, directory, workspace entry, lockfile
entry, xtask selectors, build scripts, guards and active documents now say `cantos-server`
(`services/cantos-server`). The deployed legacy VPS runtime keeps its operational names
(Quadlet units, container and image names, release bundle file name, Caddy snippet) until the
operator-authorized cutover of #336; see the
[rename inventory](../../services/cantos-server/README.md#rename-from-cantos-api). Everything
else in this record remains a target, except the bounded library slice below.

**Implemented by the #332.4 foundation slice:** [`packages/store-turso`](../../packages/store-turso/README.md)
supplies the single-owner lock, the checksummed schema registry with a pure startup decision, a
serialized `BEGIN IMMEDIATE` writer with typed commit outcomes, an injected clock and a verified
`VACUUM INTO` backup, plus [probes of the pinned engine](../evidence/turso-capability-report.md)
(`cargo xtask verify turso`). Its foundation is `integration-tested` on Linux x86_64 (`tmpfs` and `ext4`)
and macOS arm64. Identity now has an opt-in [security-state persistence consumer](../../services/cantos-identity/README.md#file-backed-security-state)
with real-file tests; Server still has no consumer and no Server table has been ported.
PostgreSQL stays authoritative for serving. A true full-disk, footprint and the Server schema mapping
remain `NOT_RUN`. The probes corrected several planning assumptions (see the report): recursive CTEs
work, unknown collations are refused, a broken `REGEXP` `CHECK` is silently open unless wrapped in
`coalesce`, and `VACUUM INTO` is stale unless run on the writer after a refreshing read.

The [Identity security-core library](../../services/cantos-identity/README.md) supplies
checked, distinct subject/operation/instance IDs, storage-bounded security versions and a pure
completion recheck for active/current state, binding, surface and assurance. Its mailbox-proof
planners add bound explicit-confirmation/expiry, terminal cancel/consume and replacement
decisions with expected/successor snapshots; secret verification and atomic persistence remain
shell obligations. It introduces no
binary, crypto verifier, RPC, authentication fact or session. The separate opt-in persistence
shell supplies an Identity schema and atomic disable/version/event/receipt storage; runtime
composition and credential/RPC prerequisites still belong to later #334 slices. Pure decisions do not establish durable revocation,
restore safety, atomic consume-once or auth E2E acceptance. Kani harness registration is not
evidence that a model checker ran.

The integration branch is `refactor/cantos-native-turso`, with reviewable commits/checkpoints
and a final integrated PR to `develop`. Do not merge an incompatible intermediate rename,
schema or authentication cutover. Preserve other worktrees and reconcile concurrent work.
The documentation preparation changes no executable, installed runtime, database or account.

## Single-process HTTP review boundary

[#333.1](https://github.com/loveoverflowcom/cantos/issues/333) first composes the existing
Studio/Theatre/Admin HTTP surfaces in one `cantos-server`, retaining PostgreSQL. Its prerequisite
is #332 PR-A, including the #332.0 reconciliation of the latest `develop` and
[#330's checked SessionPolicy and credential split](https://github.com/loveoverflowcom/cantos/pull/330)
into the renamed tree. A mechanical rename alone does not establish that reconciliation.
Preserve the single session-policy implementation, credential prefixes/bindings,
replay/revocation precedence and deadline semantics when resolving the renamed paths.

The renamed Server now includes that reconciliation from `0611c41a` (PR #330): checked
`SessionPolicy` and lifetime/rotation remain in `platform/bearer/core.rs`, registration mapping
in `bearer/client.rs`, and opaque credential/protocol parsing in `bearer/credential.rs`.
Application, PostgreSQL and inbound admission callers use those same owners. This is reuse of
the existing implementation, with unchanged TTLs and native token format, not a second session
engine or the native issuer cutover.

For [#335](https://github.com/loveoverflowcom/cantos/issues/335), the real Identity binary and
authenticated UDS server of #334.7 are prerequisites of the RPC client; the #333 PR-B Turso
composition is a prerequisite of sessions, binding and atomic event checkpoints. The current
Identity library supplies neither executable nor credential/RPC adapter, and Server still owns
PostgreSQL persistence. Reconciled policy tests and the pre-composition controls below cannot
accept any of those missing integration layers. Keep the old provider until its real replacement
is available; public first-party forms and Studio passkeys retain the settled D4/D3 decisions.

The [pre-composition regressions](../../services/cantos-server/tests/surface_policy_baseline.rs)
exercise the current production API router and origin-pinned browser wrapper. They add no
multi-origin configuration or new host composition. Prefix-shaped sentinels are deliberately
unissued; an own-purpose sentinel reaches an unavailable verifier as a routing/parsing control,
while a wrong-purpose or ambiguous credential is refused before that verifier. That control
is not successful authentication or auth E2E evidence. No database, issuer, test session,
network listener or provider is started by these tests.

| Preserved boundary | Pre-composition oracle | Remaining #333.1 acceptance |
| --- | --- | --- |
| Exact browser origin and route | Studio/Theatre host × current/sibling/Admin/legacy route × own/sibling/foreign/null/lookalike origin × all three Bearer prefixes | One process with separate configured allowlists for Studio, Theatre and Admin, without unioning them |
| Credential purpose and ambiguity | Own-purpose unavailable control versus wrong-prefix, duplicate Authorization, Cookie-only and mixed Cookie/Bearer refusals on real Bearer session routes | Issued credentials across protected resources, accounts and all surfaces; real cookie admission, duplicate-cookie parsing, expiry, fencing, replay and revocation |
| Preflight | Exact methods and own session headers admitted; other-surface headers, duplicate Origin/method and sibling origin refused | The same matrix through the combined route composition, including Admin policy |
| Authority source | Spoofed Host/Forwarded/X-Forwarded headers cannot admit a sibling browser Origin | Combined-host authority comes only from trusted configuration and the matched route |
| Error and cache policy | Exact Studio/legacy and Admin API errors, separate Theatre Library error shape, refusal/preflight `no-store`, no cookies minted | Authenticated/public/private-media success, timeout errors and transport-owned cache policy through the combined host |

Do not concatenate two origin-pinned wrappers around the whole router: each would reject the
other surface. Apply each surface's CORS/CSRF, authentication, deadline, error and cache policy
at its own route boundary. Keep deprecated Studio aliases under Studio authority. Admin's
dedicated verifier and audit admission cannot derive authority from Studio/Theatre sessions.
Native cookie-free requests retain their existing purpose-specific admission; development
credentials remain outside production composition.

The runtime PR must also demonstrate one owned Server process in `cargo xtask run` without
provisioning infrastructure. Live Quadlet/systemd changes belong to #336. Subsequent persistence
slices 333.2–333.8 target `refactor/333-server-turso-persistence` without changing composition;
333.9 owns the Turso flip. The origin-pinned baseline tests do not satisfy any of those runtime,
Turso, Kani, browser/device or footprint acceptance gates.

## Selected target

The [per-table persistence ownership and runtime prerequisite map](native-persistence-readiness.md)
connects this selected target to current source owners. Its checked inventory is preparatory;
it does not establish a Turso service consumer, native authentication or legacy retirement.

```text
cantos/
  apps/
    cantos-theatre/{web,mobile}/
    cantos-studio/web/
  services/
    cantos-server/
    cantos-identity/
    cantos-forge-functions/
  packages/
```

| Owner | Runtime and storage | Responsibility |
| --- | --- | --- |
| Cantos Server | One native Rust process, local in-process Turso `server.db` | Public Studio/Theatre/Admin APIs, ActorId, application sessions, resource authorization and business data |
| Cantos Identity | One native Rust process, private RPC, local in-process Turso `identity.db` | Immutable subjects, credentials, mailbox proof, recovery, required MFA and security state |
| Cantos Forge | Existing separate producer boundary | Audio production and private producer workflows; not a daemon required for login or ordinary serving |

The database choice is **Turso Database through the Rust `turso` crate, local-only**.
The request's spelling `tursor` refers to Turso. It does not select Turso Cloud, the `libsql`
remote-replica client, a PostgreSQL-compatible network frontend, a `tursodb` daemon, or an
implicit fallback to SQLite/redb. Pin and qualify the actual crate/version/features before
schema adoption. SQLite compatibility is not evidence of equivalent transaction semantics.

Each database file has one process owner. Multiple connections inside that owner may be
used only with tested transaction behavior. There is no cross-service file access, ATTACH,
SQL JOIN, foreign key or atomic transaction across the two databases. Identity data is
protected by separate OS ownership and narrowly scoped RPC, not by shared credentials.
Do not enable experimental multiprocess access merely to permit overlapping deployments.

Native development, offline E2E, Turso tests and Kani must not require or start Podman, Docker,
PostgreSQL, Kanidm, Firebase Auth Emulator, Redis, a message broker or a Feldera service.
Native development targets macOS and Linux. [#348](https://github.com/loveoverflowcom/cantos/issues/348)
supersedes only the earlier production no-container clause: the two Rust executables are
packaged as OCI images, distributed through GHCR registry digests and supervised by Podman
Quadlet. Neither image contains its database, media or secrets. The
[release contract](../deployment/native-oci-release.md) distinguishes the implemented offline
verifier from pending publishers and native host activation. This does not accept a native
runtime or retire legacy resources. Two backend processes do not mean that TLS ingress,
media storage, browsers, build tools or the entire machine have two processes.
Forge, Firebase Functions/App Distribution, Drive and approved media infrastructure are not
removed by retiring Firebase Authentication or Kanidm.

## Communication and authority

The internal baseline is gRPC plus Protocol Buffers using Tonic/Prost: Unix domain sockets
on one host, authenticated mTLS when crossing a network. FE continues using public HTTP
contracts. A different internal protocol needs a reviewed compatibility/security contract
and an equal-workload benchmark, not an unsupported claim of being faster.

Identity authentication facts must be bound to the authenticated caller/request, subject,
purpose, authentication time/assurance and security version. Deserializing a proto or knowing
an email/subject does not create authority. Server owns actor binding, session issuance,
Studio admission, dedicated Admin authority and resource/action authorization. Authentication
does not grant membership, producer privileges or a shared cross-surface session.

Preserve the existing session core where semantics remain valid, coordinating #261 and #326.
Web uses HttpOnly/Secure cookies under #295; native uses opaque Bearer credentials and secure
storage. Do not silently change TTLs, token formats or cross-purpose acceptance. Any needed
authentication-contract amendment is explicit, with matching consumer migration.

Credential reset, disabling and security-method changes require durable security-version and
revocation propagation. Design idempotent provisioning/binding and an atomic Identity event
outbox plus Server inbox/checkpoint, or an equivalent demonstrated protocol. State the
revocation freshness bound and fail-closed behavior during unknown lag or outages. A delayed
login result cannot revive a disabled account; an asynchronous event alone does not establish
instantaneous global revocation. No distributed transaction or exactly-once effect is implied.

## Product and frontend continuity

Keep Theatre Web/CMP, web-only Studio, public API namespaces/domains, player ownership,
Library/Progress/Bookmark semantics and native SQLDelight storage. Keep Script IR/revision
bytes, accepted receipts, publication/rendition identities, hashes, rights and retraction
semantics. Theatre is read-heavy, not write-free; Studio and listener/session mutations remain.

One Server must replace the current two API processes using **route-scoped** origin, CSRF,
credential and capability policies. Do not union Studio and Theatre origins into a global
allowlist or relax exact-origin checks. Preserve separate Admin/producer authority.

FE changes are limited to justified adapters/configuration/controllers and the credential,
setup or MFA controls that replacing hosted Kanidm actually requires. Inventory exceptions;
do not claim zero FE changes while removing a required login path. Preserve design, navigation,
localization and error/retry behavior. No duplicate auth UI or production mock fallback.

## Identity scope

Implement only the account lifecycle needed by Cantos, not a general OIDC/LDAP/SCIM server.
Use reviewed/pinned libraries for password KDFs, cryptography, randomness, mail and required
MFA. Candidates include RustCrypto Argon2id, existing getrandom/lettre, secrecy/zeroize and
reviewed TOTP/WebAuthn libraries. Dependency selection requires compatibility, advisory,
license and feature review; a named candidate is not a dependency pin or security audit.

The mailbox holder, not an initiating browser polling status, controls credential setup.
GET/prefetch does not activate an account. Proofs are purpose-bound, expiring and single-use;
existing identities cannot be adopted, reset or linked by matching email. Studio remains
provisioned/login-only; replacing Kanidm must not silently remove its required second factor.

Production mail still needs an explicitly configured, approved delivery path. Tests use a
private run-owned inbox. No new SMTP server, live mailbox, provider login or cloud resource is
required to run local tests. Account/password/passkey migration needs secure proof or explicit
re-enrollment; it is not a presumed database copy.

## Persistence and DBSP

PostgreSQL locks, functions, grants, clock semantics and checksummed migrations protect real
invariants. Port their guarantees deliberately to the pinned Turso engine; do not merely
translate SQL or remove constraints. Keep context-owned adapters, short transactions,
conditional mutations, immutable history and replay-safe receipts. File access control is a
changed security boundary, not an equivalent replacement for PostgreSQL roles.

DBSP is an optional in-process derived read model, never identity or authorization authority.
The current sample has signed deltas but a PostgreSQL `xid8`/`xmin` freshness contract. There is
no demonstrated Turso adapter at the reviewed baseline. Reuse appropriate computation, not
PostgreSQL snapshot assumptions. A candidate needs an atomic committed change feed,
snapshot/watermark alignment, whole-transaction publication, replay/deduplication, rebuild
and bounded fallback to authoritative Turso SQL. Row IDs are not presumed commit ordering.

Evaluate one concrete hot read against improved Turso SQL/index/cache baselines. Record
`adopt | reject | defer`, including binary/RAM and write-maintenance cost. Reject/defer is
valid; absent adoption evidence, DBSP stays out of the default release. No Feldera/broker
process is added. No unmeasured throughput or footprint gain is claimed.

## Assurance and native acceptance

Use canonical engineering/behavior/domain/UI skills and existing xtask orchestration.
Mandatory targeted Kani work covers selected high-risk pure production kernels: proof
lifecycle, security epochs, session/replay decisions, revision exhaustion and, if adopted,
feed/freshness decisions. Record assumptions, symbolic bounds, production reach and cover
outcomes. Kani is not a proof of concurrent I/O, cryptographic libraries or database durability.

Separate pure/property/model evidence from real Turso file-backed race/crash tests, actual
crypto and gRPC, two-process API journeys, browser interactions and native device acceptance.
Use owned temporary databases, deterministic clocks/IDs and a test-only inbox. Mocks do not
satisfy real login/session evidence and must not enter production artifacts.

Compare baseline/candidate on equivalent hardware, workloads, security and durability:
stripped binaries plus dynamic dependencies, installed footprint, all-component idle/peak
RSS/PSS, startup, build/test time, p50/p95/p99, errors, throughput and DB/WAL/backup bytes.
Set budgets before tuning. Distinguish steady-state after retirement from migration peak
holding recovery copies. Lower security or deleting user/media data is not optimization.

## Migration and scoped supersession

Provide a versioned, checksummed logical export/import and rehearse restore with stable IDs,
permissions, immutable hashes and receipts. Existing PostgreSQL data is not assumed empty.
Preserve historical SQL/checksums. Legacy export tooling is explicit migration work, never a
default runtime/test dependency or permission to start containers.

Coordinate the two databases' restore boundary and security epochs so restoring a snapshot
cannot revive revoked sessions. A new owner cannot overlap an old owner on the same default
Turso file. Once new writes are accepted, pointing back to an old database is not a no-loss
rollback: define quiescence, reconciliation or forward recovery and test the failure points.

Within this refactor, this decision supersedes the old PostgreSQL/Kanidm selection and local/test
container dependencies in ADR 0001/0021/0034/0036/0041, not their unrelated domain/security
obligations. #348 restores OCI/Quadlet only as the production packaging/supervision boundary.
#332 owns
reconciliation of active AGENTS/layout/contract/runbook references as code changes; historical
records remain source-bound. Coordinate #174/#191/#242/#261/#295/#299/#326 and #277–#280,
without closing them or creating competing implementations merely because a checkpoint merged.

Review, issue logging and this documentation do not authorize deployment, DNS/IAM changes,
real mail, account provisioning, database/media deletion or automatic merge. Live cutover and
legacy retirement require separately authorized operator steps after restore acceptance.

## Primary references

- [Turso local Rust engine](https://docs.turso.tech/sdk/rust/quickstart),
  [compatibility](https://docs.turso.tech/sql-reference/compatibility),
  [transactions](https://docs.turso.tech/sql-reference/statements/transactions),
  [single-process default](https://docs.turso.tech/sql-reference/multiprocess-access).
- [Tonic](https://docs.rs/tonic/latest/tonic/),
  [DBSP inputs](https://docs.rs/dbsp/latest/dbsp/operator/input/index.html),
  [Kani limitations](https://model-checking.github.io/kani/tool-comparison.html).

Documentation is research/decision evidence only. Runtime tests, Kani, benchmarks and live
migration are not established by this record; GitHub owns their execution status.
