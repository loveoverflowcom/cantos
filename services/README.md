# Cantos services

[manifest.toml](manifest.toml) is the typed process inventory used by `cargo xtask`.
It describes source/configuration, never observed deployment or complete runtime acceptance.

| Service | Ownership | Source boundary |
| --- | --- | --- |
| [cantos-server](cantos-server/README.md) | Studio/Theatre/Admin HTTP, actors, application sessions and business authority in `server.db` | One native listener and embedded owner; business SQL/schema port is incomplete |
| [cantos-identity](cantos-identity/README.md) | Password/mailbox/security state in `identity.db`; private authenticated UDS RPC | Native executable; facts never grant Server sessions or resource authorization |
| Object storage | Private filesystem or explicitly configured S3 media | Externally managed; no lifecycle/deletion command |
| [Forge](cantos-forge-functions/README.md) | External producer CLI/finite worker | Separate from serving/login; no registered mandatory process or accepted live Functions/provider execution |

Each native process owns one file, separate migration and backup lifecycle. The Server never
opens Identity's DB. Shared owner primitives live in `packages/store-turso`; context SQL stays
with the context. See the [porting boundary](../docs/architecture/server-turso-porting.md#checkpoint-boundary)
and [Identity v1](../packages/contracts/identity-auth-v1.md).

## Runtime policy

`cargo xtask run` owns the two foreground native processes, private development files and socket.
Migrations are explicit; serve refuses missing/unmigrated files. `services list|validate` inspects
the inventory. Container lifecycle, Kanidm and PostgreSQL verification commands are retired
refusals, not setup instructions. Native dev/E2E/Kani has no Podman dependency.

Production packaging follows [#348](../docs/deployment/native-oci-release.md): GHCR digests and
Podman/Quadlet. Image production, admission, host activation and live acceptance are separate;
no command activates a native host yet. [infra](../infra/README.md) owns deployment assets.
Historical PG/provider assets and deployed-state records are recovery/migration inputs only;
source cleanup authorizes no service, data, WAL, volume, media or account deletion. Firebase
Hosting, Functions, App Distribution, Drive and Forge retain their separate owners.
