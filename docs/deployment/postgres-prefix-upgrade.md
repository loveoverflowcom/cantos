# Legacy PostgreSQL prefix upgrade — historical

This guide describes the retired PostgreSQL/Kanidm/two-API source path. Its executor is disabled;
`deploy vps` and legacy `api` releases refuse before effects. The
[native OCI contract](native-oci-release.md) owns #348's current packaging and activation gaps.
The [native checkpoint](../architecture/server-turso-porting.md#checkpoint-boundary) is incomplete
and must not be deployed over an existing host/database. No native activation command exists.

[Original runbook at `b2611c3d`](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/deployment/postgres-prefix-upgrade.md) preserves exact recovery, topology, SQL-registry,
accepted-write, off-host backup and cutover constraints for authorized legacy investigation.
Historical deployed-state observations are not a fresh host inventory. Keep separate current/
previous releases and verified DB/media recovery; unknown writes or migration outcomes require
reconciliation before retry. Existing epoch/checksum history is immutable; a new engine never
replays PG migration SQL. Default user-data migration remains Tier B until inventory and owner
decision establish otherwise.

Do not stop/remove legacy services, accounts, packages, credentials, DB/WAL, volumes, backups or
media because their source path retired. Such actions require a separately authorized operator
scope, fresh inventory and proven restore/cutover/retention. Static Hosting/Pages, Firebase
Functions/App Distribution, Drive and Forge have independent owners.
