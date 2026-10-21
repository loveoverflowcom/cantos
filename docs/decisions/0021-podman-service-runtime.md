# ADR 0021 — Podman-first service runtime

- Status: accepted architecture; command/configuration tooling is implemented.
  Historical commit sequencing below does not establish live infrastructure acceptance.
- Date: 2026-10-13.
- Owner: Cantos maintainers.
- Refines: [ADR 0001](0001-modular-monolith.md), [ADR 0009](0009-monorepo-layout.md),
  [ADR 0012](0012-xtask-command-surface.md).

## Context

Scoped supersession: the [native refactor](../architecture/native-turso-refactor.md) removes
container dependencies from native development and acceptance. The owner's
[#348 production amendment](https://github.com/loveoverflowcom/cantos/issues/348) retains
Podman Quadlet for two native Rust executables, distributed through GHCR by registry digest.
The [release boundary](../deployment/native-oci-release.md) separates source implementation
from pending publish/host acceptance. The historical decision below does not authorize
changing an existing host's rootful/rootless principal or retiring legacy data/runtime.

One modular Rust/Axum backend depends on PostgreSQL. Kanidm currently has an example
configuration but no OIDC integration. Treating `services/` as Rust packages hides infrastructure
ownership and encourages competing run scripts. The Mac mini needs a container path that does
not introduce Docker Engine/Desktop/daemon or silently provision a VM.

## Decision

A service represents a process or independently managed infrastructure, not necessarily a
source package. Keep one application backend and a small typed process inventory in
[services/manifest.toml](../../services/manifest.toml): native, Podman and external runtimes,
required/optional dependencies, current implementation state, owner, environments, ports,
health, persistence, lifecycle and development/production distinctions.

Use rootless Podman for local OCI infrastructure, official patch-pinned PostgreSQL 17 and
Kanidm images, loopback-only configurable host ports and instance-scoped named volumes.
`docker.io` is a registry, not an engine dependency. No Containerfile is needed. Linux and an
already configured rootless macOS Podman Machine connection share the same CLI contract.
No command installs Podman, attaches/changes a connection, initializes or starts a machine.
Those environment changes require separate explicit consent.

`cargo xtask services` owns explicit list/up/status/logs/down. `up --all` means the required
local container infrastructure (PostgreSQL); optional Kanidm requires its own named `up` and
operator-supplied TLS/configuration. Native processes stay under `cargo xtask run`; external
services are inventory only. Status must report observed container state, never desired state.
Only containers/volumes with the exact Cantos manager, project, instance and service labels may
be used; operations target inspected immutable IDs to avoid name-replacement races. Refuse
foreign names, unlabelled volumes and configuration drift. `down` stops only and never removes
containers or volumes. Recovery after timeout is inspection/logs and explicit retry; retain
partial state and data. No automatic cleanup, migration, identity bootstrap or upgrade.

Build/test/preflight never auto-start infrastructure. Keep disposable native PostgreSQL
verification and native `run` unchanged. Repository/CI verification validates the manifest and
examples without requiring Podman, TLS secrets or a live service. The parser/planner and
lifecycle shell receive deterministic regression and mocked boundary tests. Actual lifecycle,
readiness and persistence evidence needs a real supported environment.

## Consequences and deferred work

This does not choose production identity, implement OIDC, change domain/API contracts or deploy
anything. Production backup/restore, network policy, secret delivery, public TLS, Linux Quadlet,
HA and upgrades need separate reviewed work. No generic orchestrator, broker, service framework,
new crate or fake infrastructure package is introduced.

Patch tags are explicit version pins, not immutable supply-chain digest guarantees. Revisit
digest locking before production adoption; review image changes and never reattach a populated
volume to an incompatible image automatically. Named volumes live in the selected Podman
connection (inside the Mac VM), so machine deletion/reset can lose data; lifecycle commands
never perform those operations. Operators own tested backups outside that VM.

## Sources and acceptance

Consult the official [Podman run contract](https://docs.podman.io/en/latest/markdown/podman-run.1.html),
[Podman Machine](https://docs.podman.io/en/latest/markdown/podman-machine.1.html),
[PostgreSQL image documentation](https://github.com/docker-library/docs/blob/master/postgres/README.md)
and [Kanidm configuration](https://kanidm.github.io/kanidm/stable/server_configuration.html).
Acceptance distinguishes documented/configured, deterministic/mocked checks and actual runtime
observations. A missing Podman/connection is `UNSUPPORTED`, never a live pass.
