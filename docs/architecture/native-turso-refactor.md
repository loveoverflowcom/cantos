# Native Server and Identity with embedded Turso

## Decision and evidence boundary

The product owner selected this target on 20 October 2026. [Epic #331](https://github.com/loveoverflowcom/cantos/issues/331)
owns execution and acceptance; its five checkpoints are [foundation #332](https://github.com/loveoverflowcom/cantos/issues/332),
[Server persistence #333](https://github.com/loveoverflowcom/cantos/issues/333),
[Identity #334](https://github.com/loveoverflowcom/cantos/issues/334),
[integration #335](https://github.com/loveoverflowcom/cantos/issues/335) and
[qualification/cutover #336](https://github.com/loveoverflowcom/cantos/issues/336).

The selected direction is now partially implemented: Server and Identity are native processes,
each owning a separate Turso file. The shared store, actor/session foundation, private RPC,
first-party Web cookie and CMP/native Bearer consumers exist. The
[porting checkpoint](server-turso-porting.md#checkpoint-boundary) owns the remaining business
schema/SQL gap. A successful file open/build does not establish HTTP parity or native acceptance.

The original baseline `develop@cccb547c08585e1266e26b2b07ead3a2c887697e` used `services/cantos-api`,
PostgreSQL, Kanidm and two API processes. That is history, not current source or fresh deployed
observation. Current canonical source is `services/cantos-server`; retained PG schema, checksum
history and the [ported/retired oracle record](server-turso/remaining-pg-retirement.md) of the former
`legacy-pg-tests` supply porting oracles. Registered migrations remain immutable.

[`packages/store-turso`](../../packages/store-turso/README.md) supplies exclusive ownership,
checksummed registries, serialized writers, pinned readers, typed commit uncertainty and verified
backup primitives. Its [capability report](../evidence/turso-capability-report.md) is source-bound
engine evidence, separate from business/runtime/restore/footprint acceptance. Use `coalesce` for
regex CHECKs; qualify the pinned engine rather than assume SQLite behavior. No second owner,
DB daemon or cross-service file read is introduced.

[#348](../deployment/native-oci-release.md) selects GHCR digests and Podman/Quadlet for production
packaging only; native development/E2E/Kani remains container-independent. Candidate producer,
offline admission, real pair execution, host activation and observed deployment are separate.
The source checkpoint authorizes no legacy service/data retirement.

## Owner amendment: v1 authentication and parallel integration

The owner selected this v1 scope on 21 October 2026 for #332/#334/#335/#336, coordinated with
#295. It supersedes the earlier Studio D3/passkey requirement and blanket Server-Turso/event-
consumer prerequisites **for v1**, without accepting unfinished implementation or closing issues.
[ADR 0045](../decisions/0045-v1-email-password-sign-in.md) records the accepted email/password
decision and [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md) its backend contract;
this section owns the parallel-integration and security-state strategy around them.

| Selected v1 decision | Boundary |
| --- | --- |
| Email/password with email verification, including Studio password login | Identity proves identity; Server still checks Studio roster/admission, ActorId, roles and resource permissions. Studio stays provisioned/login-only, with no public signup or email-based auto-link. |
| Reuse the existing session engine | Preserve SessionPolicy, TTL/replay/revocation and purpose fences; do not create a separate mobile session system. Web uses HttpOnly/Secure cookies with exact-origin CORS/CSRF; CMP uses opaque Bearer with its existing coordinator and secure storage. |
| Auth and frontends proceed alongside Server PostgreSQL → Turso migration | This permitted the historical transitional PG slice; the later source-retirement amendment supersedes it. Current sessions use the one Server Turso owner; no dual write, partial second DB or cross-DB atomicity. |
| Direct authoritative security-state checks are allowed | A durable event consumer is not a v1 prerequisite. Define freshness bounds and fail-closed issuance, renewal and protected/sensitive admission; recheck epoch/version/current state and reject late or stale facts. Events remain a possible later optimization, with their own recovery contract. |
| Verify email in Web, then return to CMP | This is a valid v1 journey, with operation/purpose binding and explicit mailbox confirmation. Polling, a redirect or a client flag alone never authenticates the app. |

Passkey/MFA, recovery/change-password, phone and Google login are **deferred**, not completed
or prerequisites for v1. Their proof/recovery/assurance obligations still apply when introduced.
Existing legacy provider paths retain their guards until their replacements are implemented and
accepted; a documentation amendment does not authorize a runtime bypass.

| Implementation observed in source | Verification boundary |
| --- | --- |
| Identity `service` implements Theatre mailbox-confirmed signup, Argon2id password authentication (Theatre, and Studio under [ADR 0045](../decisions/0045-v1-email-password-sign-in.md)) and `GetSecurityState` over private UDS RPC | The [service guide](../../services/cantos-identity/README.md#verification) records source-bound prior runs; this amendment runs no authentication tests. |
| Studio password is implemented in Identity: [the core](../../services/cantos-identity/src/domain/admission.rs) admits `(Studio, Password)` beside the deferred `UserVerifiedPasskey`, and the Server's roster decides Studio entry | Prior runs are recorded in [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md#verification); this amendment runs none. Studio provisioning remains absent; a password result is never relabelled as passkey assurance. |
| [Server foundation](native-persistence-readiness.md) composes Turso sessions/ActorId bindings; business port remains incomplete | Historical transitional PG E2E never qualifies native HTTP parity. |
| [Shared Web auth](../../packages/web-auth/README.md) now consumes the Server's v1 Web cookie endpoints (source only; no Bearer fallback); [CMP](../../apps/cantos-theatre/mobile/README.md#opt-in-native-identity) has first-party Identity v1 Bearer coordination/secure storage | Web cookie/fetch/XHR/session-restore wiring exists in source; real first-party v1 browser journeys remain pending, with separate browser/device acceptance. |

The #335 backend slice consumes the authenticated RPC implementation and draft contract;
completion of every Server Turso adapter was not a prerequisite. Final #336 native acceptance still needs the
two production binaries, two file-backed Turso owners, real RPC/auth/revocation/restore journeys
and separate browser/device/Kani evidence without PostgreSQL or container dependencies.
[#348 packaging](../deployment/native-oci-release.md) selects production GHCR/Podman only; it
does not make native development or tests container-dependent.

## Owner amendment: source retirement on the integration branch

On 22 October 2026 the owner decided that PostgreSQL and Kanidm leave the **source** of the
integration branch `refactor/cantos-native-turso` now, without waiting for full end-to-end, Kani,
browser/device or live-host acceptance. This supersedes only the earlier condition that kept
legacy source "while the default serving path consumes it" and "only alongside the accepted
replacement and its real tests" (the retirement paragraph of
[native persistence readiness](native-persistence-readiness.md#porting-and-retirement)); it does
not lower any invariant.

| Decided | Unchanged |
| --- | --- |
| `cantos-server` serves every use case from its one embedded Turso `server.db`; `cantos-identity` owns `identity.db`. PostgreSQL, Kanidm, Firebase Authentication enrollment and every provider session/token path are removed from implementation, composition, tooling and active operating configuration in this repository. Retired SQL and tooling are recoverable only through Git history and source-bound evidence, never as a compiled or default dependency. | Rust Turso embedded at the pinned version (no SQLite/libSQL remote/Turso Cloud); Identity v1 email/password with mailbox verification; HttpOnly/Secure Web cookies and opaque Bearer for native; `SessionPolicy`, rotation/replay, session fences and revocation; ActorId, immutable subject bindings, Studio roster and resource authorization; immutable revisions, hashes and receipts; production packaging through GHCR images and Podman/Quadlet. |
| A partially integrated or not-yet-accepted intermediate state may be delivered as a Draft PR if every gap is stated precisely. | No fallback to PostgreSQL, Kanidm or Firebase Authentication, no runtime mock and no claim that a missing backend task succeeded. Admin has no native credential verifier yet and fails closed. |

Four conclusions stay separate in every handoff: **source retirement** (legacy source and tooling are
gone), **buildability** (the workspace compiles and the selected checks run), **integration
acceptance** (two production binaries on two files, real RPC/auth/revocation/restore journeys) and
**production readiness** (live host, data migration, cutover). This amendment grants none of the
last two and authorizes no deployment, publication, data conversion, deletion of a database, volume,
backup or media on any host, nor any change to an installed service. A database file, WAL, backup,
account or secret on a real machine stays out of reach of source retirement.

## Owner amendment: Firebase verification provider

The corrected owner task of 23 October 2026 narrows the source-retirement decision above:
Firebase Authentication is selected again only for verification email/actions, with a supervised
Auth Emulator in local Theatre `xtask run`. Native Identity + Turso remain authoritative and
Firebase remains outside login/session issuance. PG/Kanidm/OIDC/provider login are not restored.
Read the [implemented operation-bound protocol and recovery limits](firebase-verification.md).
Identity uses encrypted native proof custody and durable provider-effect fences. The production
adapter is source only; expired enrollment recovery and broader browser/device/deployed acceptance remain
open. Emulator readiness alone establishes no authentication acceptance.

## Single-process HTTP review boundary

The [HTTP composition owner](server-http-composition.md) describes the implemented one-listener
Server, route-specific origins/state and native session consumers. Reuse the shared SessionPolicy,
credential parsing, actor binding and late admission checks; no duplicate engine or global origin
allowlist. Provider-only routes are removed; Admin fails closed without its verifier.

Review exact own/sibling/foreign/null/lookalike origins, purposes, duplicate/mixed credentials,
session fences and no-store behavior through actual HTTP production handlers. Preserve archived
PG assertions as porting oracles, including slow-body deadline and transactional revocation races.
Their historical PASS is not native parity. Native session/file/RPC tests qualify only their named
scope; two-production-binary HTTP, browser/device, restore, isolation, D1 and footprint remain
separate #333/#335/#336 acceptance. Readiness and opening the owner cannot close those gaps.

Current source owners are `platform/store`, `platform/turso`, `platform/identity`,
`platform/identity_auth` and `adapters/inbound/http`. Business context SQL/schema remains unported.
No second DB, provider fallback, weakened admission or mock success may bridge the gap.

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
| Cantos Identity | One native Rust process, private RPC, local in-process Turso `identity.db` | Immutable subjects, v1 credentials/mailbox proof and security state; deferred factors/recovery follow the amendment |
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

Native offline tests, Turso tests and Kani must not require Podman, Docker, PostgreSQL, Kanidm,
Firebase Auth Emulator, Redis, a message broker or a Feldera service. The explicit local Theatre
`xtask run` now owns an Auth Emulator under the verification-provider amendment above; ordinary
build/test/preflight does not start it. Its installed tooling is an explicit local-run prerequisite.
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

Disabling and security changes require durable security-version state and a demonstrated
admission/revocation protocol. Design idempotent provisioning/binding; direct current-state
checks are allowed by the v1 amendment. If events are used, design an atomic Identity outbox
plus Server inbox/checkpoint, with deduplication and recovery. State the
revocation freshness bound and fail-closed behavior during unknown state, lag or outages. A delayed
login result cannot revive a disabled account; an asynchronous event alone does not establish
instantaneous global revocation. No distributed transaction or exactly-once effect is implied.

## Product and frontend continuity

Keep Theatre Web/CMP, web-only Studio, public API namespaces/domains, player ownership,
Library/Progress/Bookmark semantics and native SQLDelight storage. Keep Script IR/revision
bytes, accepted receipts, publication/rendition identities, hashes, rights and retraction
semantics. Theatre is read-heavy, not write-free; Studio and listener/session mutations remain.

One Server replaces the historical two API processes using **route-scoped** origin, CSRF,
credential and capability policies. Do not union Studio and Theatre origins into a global
allowlist or relax exact-origin checks. Preserve separate Admin/producer authority.

FE changes are limited to justified adapters/configuration/controllers and v1 credential/mailbox
controls under the amendment; deferred MFA is not a v1 UI prerequisite. Inventory exceptions;
do not claim zero FE changes while removing a required login path. Preserve design, navigation,
localization and error/retry behavior. No duplicate auth UI or production mock fallback.

## Identity scope

Implement only the account lifecycle needed by Cantos, not a general OIDC/LDAP/SCIM server.
Use reviewed/pinned libraries for password KDFs, cryptography, randomness and mail; deferred
factors need their own later qualification. Candidates include RustCrypto Argon2id, existing getrandom/lettre, secrecy/zeroize and
reviewed TOTP/WebAuthn libraries. Dependency selection requires compatibility, advisory,
license and feature review; a named candidate is not a dependency pin or security audit.

The mailbox holder, not an initiating browser polling status, controls credential setup.
GET/prefetch does not activate an account. Proofs are purpose-bound, expiring and single-use;
existing identities cannot be adopted, reset or linked by matching email. Studio remains
provisioned/login-only; its v1 password factor is recorded in ADR 0045 and the amendment, and
provisioning remains absent.

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

The current [#384 decision](https://github.com/loveoverflowcom/cantos/issues/384) supersedes
DBSP experimentation as a prerequisite: **Turso SQL/index first, selective measured cache later;
incremental engines DEFER**. The incremental module, feature and gate are retired; do not infer
an incompatibility between the technologies. The old PostgreSQL `xid8`/`xmin` feed is historical,
not a Turso commit-order contract. Preserve its unique reasoning through [ADR 0017](../decisions/0017-experimental-dbsp-job-summary.md).

Follow the [Turso read method](../../.agents/skills/cantos-engineering/references/turso-read-performance.md).
Measure the actual consumer/representation on a ready native module. Keep authority, late
admission/revocation checks, coherent reads, receipts and unknown-outcome handling. No cache,
engine or unmeasured performance claim follows from source retirement.

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
