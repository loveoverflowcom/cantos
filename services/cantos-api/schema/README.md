# Current PostgreSQL schema

`0001_current.sql` is the canonical **epoch 2, version 1** fresh-install DDL. It materializes
current constraints/functions/triggers/indexes without replaying retired provider migrations.
The metadata registries are created by `platform/postgres/migrations.rs` in the same transaction.
`0002_bearer.sql` is the additive epoch-2 version 2 expansion. The migrator validates every
applied checksum and applies only the exact missing suffix, atomically. There is no executable
epoch-1 upgrade chain or parallel SQL bootstrap command. Runtime verification requires both versions.
Use `cantos-migrate` through the owning deployment/test commands; server startup is read-only.

An applied version's SHA-256 is immutable within its epoch. Future changes need a separately
reviewed forward migration or explicit epoch decision; do not edit an installed baseline to
make its checksum match. New versions and unexpected registry entries must not be accepted by
an older binary. The schema marker does not certify the validity of imported application data.

The fresh schema was materialized from current authoritative entities at
`develop@3e6c183d3d616e8ec4a782ba788b4b70d0c6e49a`, with caller-only adaptation constraints.
Its catalog-clock seed is explicit. Functions preceding relation definitions use local
`check_function_bodies=false`, as in a PostgreSQL schema dump; real SQL gates execute the
functions and their current consumers. Runtime authority remains in narrow operator-applied
grants, never automatic grants to a role guessed from a name.

`legacy-epoch-1-manifest.json` records all original migration names/versions/SHA-256 values and
an immutable source revision. It is an audit/recognition artifact, **not executable SQL** or a
compatibility engine. Original bytes are retrievable from Git; no deployed row or migration
checksum is changed by retiring source files. The new migrator refuses any unmarked nonempty
database, including an old registry whose rows happen to look like version 1 of epoch 2.

See the [cutover runbook](../../../docs/deployment/postgres-schema-cutover.md) for private live
inventory, backup/restore validation, a reviewed one-time data mapping and explicit operator
approval. No production conversion tool is supplied. The generic optional DBSP library is
unrelated to schema authority; its old TTS capture tables are absent here.
