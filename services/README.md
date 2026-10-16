# Cantos services

A service is a runnable process or independently managed infrastructure component. It need
not be a Rust crate. [ADR 0021](../docs/decisions/0021-podman-service-runtime.md) defines the
runtime boundary; [manifest.toml](manifest.toml) is the versioned process inventory consumed by
`cargo xtask`. It is not a deployment specification or a generic orchestration framework.

| Service | Runtime | Current state | Owner |
| --- | --- | --- | --- |
| `cantos-api` | native Rust/Axum | implemented development HTTP host; production deployment planned | backend maintainers |
| `postgresql` | rootless Podman OCI | local runtime configuration; backend SQL and native verification implemented | infrastructure operator |
| `kanidm` | rootless Podman OCI | operator configuration; API Studio/Theatre OIDC adapters implemented; local Theatre sign-in/logout accepted, production activation gated | identity operator |
| `object-storage` | external | filesystem and generic S3-compatible adapters implemented; real-provider/deployment acceptance separate | storage operator |

The API is the authoritative modular content backend, not a restriction to one possible service
process. Application/domain/API contracts do not depend on the container runtime. `services/`
holds service implementations and their canonical local definitions/guides; only `cantos-api`
is currently a Cargo package here. #203 owns the planned Forge relocation/runtime in
`services/cantos-forge-functions`; the existing local crate remains in `apps/cantos-forge` today.
The API never synthesizes audio or calls Gemini.

Cross-environment deployment configuration lives in [`infra/`](../infra/README.md): the public
edge and static Firebase Hosting are not registered xtask services. Their directory move does
not change live resources, start lifecycle commands or establish deployment acceptance.

## Runtime policy

Podman is the standard container runtime. Linux uses rootless Podman; a Mac mini uses an
operator-installed, already running rootless Podman Machine connection. Docker Engine, Docker
Desktop, a Docker daemon and Compose are not dependencies. `docker.io` is an allowed OCI
registry. Use official images pinned to an explicit patch/distribution tag (or reviewed digest),
never `latest`; an upgrade requires compatibility and recovery evidence.

Development containers bind only `127.0.0.1`, use configurable high ports and instance-scoped
persistent named volumes. The manifest declares required and optional dependencies, environments,
ports, health, persistence and lifecycle for each process. Optional identity/storage does not
imply implemented integration. Production topology, public ingress, backups, TLS issuance,
accounts, application-role grants and migration approval remain operator-owned separate work.

Only explicit lifecycle commands may start infrastructure. Build, tests, `run` and preflight
never start PostgreSQL or Kanidm, install Podman or initialize/start a machine. Native
`cargo xtask run` and the disposable `initdb`/`pg_ctl`/`psql` verification remain independent.
Secrets, certificates, identity databases and persistent data stay outside Git. Stop/restart
must preserve volumes; no lifecycle command performs migrations, pruning or data deletion.

## Local guides

See [PostgreSQL](postgresql/README.md), [Kanidm](kanidm/README.md) and the [public edge](../infra/edge/README.md) for private prerequisites,
explicit commands, storage and recovery. Configured examples grant no production acceptance.

## Implemented command boundary

`cargo xtask services list|validate|up|status|logs|down` now implements the bounded local
contract described in [xtask](../xtask/README.md#explicit-local-service-infrastructure).
`up --all` starts required PostgreSQL only; optional Kanidm requires named `up`.
Static validation and deterministic/mocked boundary tests require no Podman. Actual
container/TLS/readiness/persistence evidence is separate; see the
[runtime evidence](../docs/evidence/podman-service-runtime.md).

The server must support named-volume `nocreate` (older unsupported versions fail without an
automatic update). Private mounts never relabel host files; enforcing SELinux requires
preapproved readable input labels. Do not concurrently modify connection definitions or force
replace labeled resources during lifecycle operations. No production acceptance is implied.
