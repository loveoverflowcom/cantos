# Native persistence readiness

## Current source boundary

The [Server source boundary](server-turso-porting.md#checkpoint-boundary) and actual registry
own current implementation. `services/manifest.toml` registers two native processes, each with
one Turso file. The Server composes `platform/turso` through `platform/store`; Identity owns
`identity.db` and private authenticated UDS RPC. The Server reaches Identity only through RPC.

| Layer | Implemented source | Separate acceptance |
| --- | --- | --- |
| Shared store | [Owned database](../../packages/store-turso/README.md), checksummed registry, serialized writer, pinned readers and verified backup | Engine qualification does not establish every business/HTTP journey |
| Server foundation | [Turso owner](../../services/cantos-server/src/platform/turso/mod.rs), [facade](../../services/cantos-server/src/platform/store/mod.rs), actor registry, native and development sessions | Production process, contention and recovery qualification |
| Server business contexts | Migrations 12–23 cover all 69 Server-owned tables in the legacy ownership manifest; v24 adds cover disclosures. Catalog, Editorial/Ingestion, Adaptation, Production, Admin, Media/Publication and Listening use native adapters | [Replacement ledger](server-turso/remaining-pg-retirement.md) maps actual native tests; remaining parity/data-conversion and production acceptance stay explicit |
| Identity | [Service](../../services/cantos-identity/README.md), password/mailbox/security-state persistence and private RPC | Production provisioning, recovery/restore and browser/device qualification |
| Auth consumers | [Identity auth v1](../../services/cantos-server/contracts/auth/identity-auth/v1/README.md), Web cookie and CMP Bearer adapters | [23 October loopback integration](../evidence/post-merge-integration-20261023.md) is bounded evidence, not HTTPS/deployed or all-device acceptance |
| Tooling | [Native commands](../../xtask/README.md), explicit migrations, run-owned directories and socket readiness; opt-in [prepared sample](../deployment/theatre-seeded-catalog.md) | Plain seed refusal, sample-only private assets, public release policy and host activation are distinct boundaries |

## Porting and retirement

Preserve the legacy [PG schema inventory](../../services/cantos-server/schema/README.md),
checksums, invariant mappings and [ported/retired oracle record](server-turso/remaining-pg-retirement.md).
The former 23-file/251-test PG tree has been retired; original assertions remain traceable in
commit-pinned records and the replacement ledger. Do not recreate old test files or run the
retired `verify postgres` gate. Registered placeholder versions 4–11 must not be filled in.

Keep short atomic write units, late authorization rechecks, receipts and unknown-commit handling.
The Server facade bounds writer acquisition to five seconds; this alone is not a load/latency
qualification. Do not create an auth-only second Server DB, read Identity's file or claim
cross-DB atomicity. Read performance follows [#384](https://github.com/loveoverflowcom/cantos/issues/384):
measure Turso SQL/index first; then consider ArcSwap snapshots or bounded Quick Cache entries.
Moka is excluded and incremental engines are deferred by that owner decision.

No database, WAL, backup, media, account or installed service is retired by source cleanup.
Without production inventory and owner decision, data conversion remains Tier B. Native OCI
qualification/admission, same-schema NATIVE_READY upgrades and first-native/legacy cutover have
separate boundaries in [#348's release contract](../deployment/native-oci-release.md).
