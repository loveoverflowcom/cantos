# PostgreSQL local Podman runtime

Configured local infrastructure, not a production deployment. The API already uses PostgreSQL;
this adds a separate optional development runtime. Native `cargo xtask verify postgres` still
creates a fresh disposable cluster with `initdb`, `pg_ctl` and `psql` and never uses this database.

The [process manifest](../manifest.toml) pins the official
`docker.io/library/postgres:17.11-bookworm` image (ARM64 and amd64). This is an explicit patch
and distribution tag, not an immutable digest. No custom image, Docker daemon or Compose is needed.
The named `cantos-<instance>-postgresql-data` volume mounts `/var/lib/postgresql/data`, the
PostgreSQL 17 data location. Both container and volume have Cantos ownership labels.

## Explicit development lifecycle

Supply an existing private password file outside the repository (regular, caller-owned, mode
0600), then export the [example variables](env.example). xtask neither creates a password nor
reads/prints its contents. The file mounts read-only with a private SELinux label where
supported and the official entrypoint uses `POSTGRES_PASSWORD_FILE`. On macOS the source path
must already be shared into the running Podman Machine; xtask never changes VM mounts.

```sh
export CANTOS_POSTGRES_PASSWORD_FILE=/absolute/private/cantos/postgres-password
export CANTOS_POSTGRES_PORT=55432
cargo xtask services --instance local up postgresql
cargo xtask services --instance local status
cargo xtask services --instance local logs postgresql
cargo xtask services --instance local down postgresql
cargo xtask services --instance local up postgresql
```

Commands become available in the lifecycle commit of this PR. Host binding is always
`127.0.0.1`, with a configurable unprivileged port; a occupied port fails without stopping its
owner. `pg_isready` checks container loopback TCP, which avoids a false ready result from the
image's temporary initialization server (Unix socket only). It proves database readiness,
not application schema, role grants, password validity or API readiness. Startup is bounded;
failed state remains available for operator inspection.

The image initializes a fresh empty volume with its `postgres` administrative role/database
and password authentication (`scram-sha-256`). No app-role grants, SQL initialization scripts
or Cantos migrations run automatically. The API must use a separately provisioned least-privilege
app role and explicitly approved migrations; do not point the API at the administrative role.
`cargo xtask run cantos-api` retains its explicit `DATABASE_URL`/test-cluster requirements.

## Persistence and recovery

`down` stops the inspected container ID, preserving its container and named volume. Repeated
`up` starts the same compatible container; it never replaces a foreign or drifted resource.
Changing ports, mount paths or image is configuration drift and requires an operator-reviewed
recreation/upgrade. No command removes a volume, prunes resources or attaches an unlabelled
volume. Changing the password file does not rotate an existing database password: image
initialization variables apply only to an empty data directory.

Use a distinct `--instance <name>` and unused port for disposable fixtures or another checkout.
Defaults are shared across checkouts on the selected Podman connection. Do not reuse populated
volumes for tests or upgrades. Operators own backups/restore; a Podman Machine reset/deletion
can destroy its internal volumes even though `services down` cannot. Production secrets,
backups, roles, monitoring and upgrades remain separately planned.

Sources: [official image tags](https://github.com/docker-library/official-images/blob/master/library/postgres),
[official image environment/storage](https://github.com/docker-library/docs/blob/master/postgres/README.md),
[PostgreSQL readiness](https://www.postgresql.org/docs/17/app-pg-isready.html).
