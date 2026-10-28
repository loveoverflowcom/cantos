# Sample metadata operator

This is a stopped-database operator capability for only the six Works and twelve units
identified by ADR 0051 or the separately pinned fictional replacement. It does not issue a login or grant a release. The pinned source
digest is a scope check, not proof of rights; the operator must obtain a real source and
metadata-rights confirmation from the rights holder before a publication attempt. A
synthetic test attestation must never be used for production.

Before use, complete the normal image qualification, admission, backup and migration
procedure. Stop the Server and acquire exclusive ownership of its current migrated
database. A migration to v27 requires its own approved backup and recovery plan. This
document does not authorize a production command or replace that admission.

Supply a private JSON request file with these exact keys:

```json
{
  "operation_id": "<new canonical UUID>",
  "action": "publish",
  "source_sha256": "d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212",
  "operator_reference": "<operator-provided reference>",
  "evidence_reference": "<reference to the verified source and rights evidence>",
  "rights_statement": "<the actual metadata rights assertion>",
  "confirm_source": true,
  "confirm_metadata_rights": true
}
```

For the separately generated fictional replacement, use source SHA-256
`0ad007cb18d46b58df25dbc2e8ea98bf58bce752c9e6d640fc4a78a90ef779d6`
and the exact bytes in `services/cantos-server/tests/fixtures/fictional-sample-metadata.json`.
Its provenance is recorded beside the source. The product owner authorized this original
fictional metadata for this task; the production request must reference the actual provenance
and state that authorization truthfully. Do not use a synthetic test claim or assert that the
older source has been cleared. The two source hashes select distinct fixed principals, records,
and withdrawal scopes; they do not let an operator choose arbitrary works or owners.

The operator reference is self-declared audit context, not an authenticated Studio identity
or cryptographic signature. The exact source file is supplied separately; it remains private
and is never projected to listeners. The request names no owner or arbitrary Work. The
transaction refuses an occupied reserved actor ID or a conflicting sample Work, and commits
all admitted metadata and the action receipt together. Running the same operation again with
the identical request is a replay; changing its request is rejected. After qualified admission,
the stopped-file operator command is `cantos-operator sample-metadata REQUEST_JSON SOURCE_JSON`.

To withdraw, submit another request with a new operation ID and `action` set to `withdraw`,
then run `cantos-operator withdraw-sample-metadata REQUEST_JSON`. The references and statement
must describe the withdrawal; the two confirmation fields need not be true for withdrawal.
The action and per-subject decisions are append-only. This first bounded implementation does
not support a new publication after withdrawal; replaying the original publication cannot
undo a later withdrawal. Both commands require `CANTOS_SERVER_DATABASE` to name the stopped
Server's database. Never put secrets or a proposed password in either request.

The API runtime image deliberately does not contain `cantos-operator`. The OCI workflow can
build a separate Linux operator artifact from the same pinned Server build stage and qualify
that exact binary against the exact image pair using disposable synthetic data. Review the
artifact's source, build-record, binary hash, qualified-image manifest and the separate sample
qualification report before any use. A build alone does not establish qualification, rights,
admission or authority to operate on the production database. The binary retains other operator
commands and must be kept in an operator-controlled private location; do not mount it into the
long-running Server service or make it available through HTTP. Run the reviewed sample command
with a stopped database and explicit least-privilege, network-disabled execution, then retain its
receipt and re-check the API. A separate operator execution decision remains required.
