# Native persistence readiness

## Current source boundary

The [Server porting guide](server-turso-porting.md#checkpoint-boundary) owns the incomplete
checkpoint. `services/manifest.toml` registers two native processes, each with one Turso file.
The Server composes `platform/turso` through `platform/store`; Identity owns `identity.db` and
private authenticated UDS RPC. The Server reaches Identity only through the RPC client.

| Layer | Implemented source | Remaining acceptance |
| --- | --- | --- |
| Shared store | [Owned database](../../packages/store-turso/README.md), migration registry, serialized writer and pinned readers | Engine qualification is separate from business/HTTP acceptance |
| Server foundation | [Turso owner](../../services/cantos-server/src/platform/turso/mod.rs), [facade](../../services/cantos-server/src/platform/store/mod.rs), actor registry, native sessions and development sessions | Business migration slots remain incomplete; context SQL/test ports under #333 |
| Server business ports | [CMP read/Library closure](server-turso/cmp-business-api.md) and [Editorial/Ingestion](server-turso/editorial-ingestion.md), appended migrations 12–16 | Remaining Catalog/Adaptation/Production/Admin/Media write ports and full non-auth HTTP parity remain #333 work |
| Identity | [Service](../../services/cantos-identity/README.md), password/mailbox/security-state persistence and private RPC | Provisioning/recovery/restore and named acceptance under #334/#336 |
| Auth consumers | [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md), Web cookie and CMP Bearer adapters | Production two-binary HTTP, browser/device, restore and D1 evidence remain separately owned by #335/#336 |
| Tooling | [Native commands](../../xtask/README.md), explicit migration, run-owned development directories and socket readiness | Seeding/catalog HTTP and host activation are not accepted merely from startup |

## Porting and retirement

Preserve legacy [PG schema inventory](../../services/cantos-server/schema/README.md), invariant
mappings and `legacy-pg-tests` as migration oracles. PostgreSQL-shaped source names do not
establish a PostgreSQL runtime, nor does opening Turso establish completed business schema.
The ownership scanner still inventories historical PG declarations, including its missing legacy
registry source; repairing that input without changing its meaning is a #333 follow-up.

Keep short atomic write units, late authorization rechecks, receipts and unknown-commit handling.
Do not create an auth-only second Server DB, read Identity's file or claim cross-DB atomicity.
Read performance follows [#384](https://github.com/loveoverflowcom/cantos/issues/384): SQL/index
first, measured selective cache later; incremental engines DEFER.

No database, WAL, backup, media, account or installed service is retired by source cleanup.
Without completed production inventory and owner decision, migration remains Tier B. Native OCI
packaging/activation is separately gated by [#348](../deployment/native-oci-release.md).
