# Service runtime work

Read [services](../../../../services/README.md), [infra](../../../../infra/README.md), the
[current layout](../../../../docs/architecture/repository-layout.md) and the owning decision
before editing runtime code. A service is a process/infrastructure boundary, not necessarily
a crate. Use typed inputs, ownership checks and argv values, not interpolated shell commands.
Keep deployment assets separate from xtask planning/execution. A concrete consumer and reviewed
runtime decision precede scaffolding; documentation does not establish an installed runtime.

## Native target and legacy boundary

For [epic #331](../../../../docs/architecture/native-turso-refactor.md), the selected target is
two native long-lived backend processes, Server and Identity, each owning a local in-process
Turso file. Forge is separate and not started for ordinary login/serving. No Podman/Docker,
PostgreSQL/Kanidm, Redis, broker, Feldera or Turso server is a hidden prerequisite of native
dev/E2E/Turso/Kani. The [23 October verification amendment](../../../../docs/architecture/firebase-verification.md)
selects an explicitly installed Auth Emulator for local Theatre `xtask run` only; no ordinary
build/test/preflight starts it and readiness is not native auth evidence. Never install or upgrade
it implicitly. [#348](https://github.com/loveoverflowcom/cantos/issues/348) supersedes only
the production no-container selection: GHCR digest distribution and Podman Quadlet supervise
the two executables. Read the [OCI release boundary](../../../../docs/deployment/native-oci-release.md)
before production work. The offline verifier is plan-only; publishers/activation remain pending.
Existing deployed resources are not thereby authorized to stop or delete; legacy migration
work remains explicit and separately scoped.

The historical [ADR 0021](../../../../docs/decisions/0021-podman-service-runtime.md) still describes
the old runtime; #348 selects OCI/Quadlet production packaging while native test isolation stays
independent. The legacy operations below do not accept a native rollout. Missing prerequisites are UNSUPPORTED or
NOT_RUN as appropriate, never permission to install/bootstrap tools or weaken isolation.

## Native lifecycle

- Reuse typed xtask supervision, manifests, release promotion and reporting. Do not invent
  executable commands until implemented or add competing runners for individual agents.
- Give each database a single process owner and separate OS permissions. Multiple threads and
  connections inside the owner are allowed only with tested semantics. Do not open a live file
  with an overlapping server, migration CLI or backup process by assuming SQLite behavior.
- A deployment drains/stops the old owner or uses a tested owner-mediated handoff. Experimental
  multiprocess access is not a default workaround. Readiness does not imply migration, auth or
  publication acceptance; distinguish liveness, schema readiness and actual journeys.
- UDS paths need owned parent directories, restricted permissions, peer policy and safe stale
  socket handling. Network RPC needs authenticated peers/mTLS and explicit method admission.
  Do not choose authority from untrusted forwarded headers or filesystem paths.
- Native releases bind source/config/architecture, executable bits, hashes and dynamic
  dependencies. Build once, promote verified bytes. #348 production releases bind both registry
  image digests and their source-bound host bundle; native developer/test supervisors remain
  container-independent. Do not run a helper from a different source revision.
- Run-owned test files, directories, sockets and child IDs are tracked precisely. Cancellation,
  readiness and shutdown have bounded budgets; clean up only those exact owned resources.
  Never kill all processes by name/port or prune shared files/volumes.
- Build and pure tests never auto-start infrastructure. Integration lanes explicitly start
  only their owned native fixture processes and use test-only mail capture, not real mail.
  Sanitized environments refuse inherited production DB/provider credentials and unsafe proxies.
- Measure the complete process tree and runtime footprint, not one binary. Exclude ingress,
  media assets and toolchains only when the report names those exclusions. Two backend owners
  is not a claim that browsers, CI workers or the entire host have only two processes.

## Legacy container operations

When a separately authorized task still inspects or migrates the legacy Podman deployment,
observe the existing rootless connection before effects; never configure a connection/machine
implicitly. Preserve host security labels. Check exact stopped-container ID, actual mount and
volume ownership before starting it; `nocreate` avoids an unintended empty replacement.

Cancellation/readiness budgets cover the whole invocation. Timeouts may leave ambiguous
retained state; reconcile before retrying. Review recreation, upgrades and retirement
separately. Labels do not protect against an administrator forging ownership or deleting a
resource concurrently. Preserve old databases, media and recovery evidence until an explicitly
authorized cutover/restore/retention decision permits retirement; source cleanup is not deletion.

## Verification

Exercise foreign-resource refusal, file locks, overlapping owners, drift, timeout, cancellation,
restart, secret redaction and recovery with focused tests and actual native process acceptance.
A fixture mock proves planning/effect-order only, not host behavior. Report macOS/Linux/runtime
acceptance separately. Follow [persistence](persistence.md) for file durability and cross-database
restore, and [local execution](local-execution.md) for authority and evidence status.
