# Cantos services

A service is a runnable process or independently managed infrastructure component. It need
not be a Rust crate. [ADR 0021](../docs/decisions/0021-podman-service-runtime.md) defines the
runtime boundary; [manifest.toml](manifest.toml) is the versioned process inventory consumed by
`cargo xtask`. It is not a deployment specification or a generic orchestration framework.

| Service | Runtime | Current state | Owner |
| --- | --- | --- | --- |
| `cantos-api` | native Rust/Axum | implemented development HTTP host; production deployment planned | backend maintainers |
| `postgresql` | rootless Podman OCI | local runtime configuration; backend SQL and native verification implemented | infrastructure operator |
| `kanidm` | rootless Podman OCI | opt-in evaluation configuration; OIDC and production identity unimplemented | identity operator |
| `object-storage` | external | planned; current development intake uses private files | storage operator |

The API is the one modular backend. Application/domain/API contracts do not depend on the
container runtime. `services/` holds code, infrastructure configuration and operational guides;
only `cantos-api` is a Cargo package. No TTS worker exists under the current architecture.

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

See [PostgreSQL](postgresql/README.md) and [Kanidm](kanidm/README.md) for private prerequisites,
explicit commands, storage and recovery. Configured examples grant no production acceptance.

## Implementation sequence

The process inventory and policy land first. The PostgreSQL/Kanidm guides and private environment
examples follow, then typed manifest/configuration validation and `cargo xtask services`
list/up/status/logs/down. A checked-in configuration is not evidence of a running service.
