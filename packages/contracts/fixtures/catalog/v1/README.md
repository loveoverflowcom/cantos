# Studio Catalog v1 fixtures

These literal synthetic Vietnamese examples freeze the JSON-only contract. They are not
production manuscripts, rights evidence, real source records or publication approvals.
The reference IDs and digests in `create-linked-chapter.json` are placeholders. The backend
independently resolves linked evidence and checks the Work owner's authorization.

[`cases.json`](cases.json) names each fixture's `$defs` type in the
[Catalog schema](../../../schema/catalog/v1.schema.json), its expected conformance, and
whether refusal belongs to shape decoding or semantic admission. The shared DTO tests decode
and round-trip accepted examples, reject unknown fields and wrong numeric types, and preserve
PATCH's absent/null/value distinction. HTTP tests own semantic refusal and actual responses;
PostgreSQL tests own transactions, revisions, retry receipts and current authorization.

Novel and story use chapters; podcast uses episodes. A podcast creation example has no source
or Script IR. Archive examples are JSON bodies for `DELETE`, which preserves history. A history
page contains immutable Work snapshots in ascending revision order, including changes caused
by child mutations. Source bytes and Script IR are referenced by stable identity and exact
checksums; this contract never rewrites their contents.

No MessagePack fixture or compatible representation is promised for these endpoints.
