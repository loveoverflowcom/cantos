# Theatre viewer claim v1

`POST /api/v1/theatre/shared/claim`, JSON `{ "operation_id": "canonical-nonzero-uuid" }`.
[Schema](viewer-access.schema.json), [request](fixtures/claim.json), [receipt](fixtures/claimed.json).
There are no actor, email, groups or subject request fields; extra/unknown fields are refused.
The caller uses the existing Theatre cookie with exact Origin or native Bearer and optional
`x-cantos-theatre-session` fence. Studio/development sessions, mixed credentials and a trusted
listener override cannot claim. No new session is issued.

The Server requests a fresh subject/version/epoch/operation-bound verified mailbox over the
private Identity RPC before opening its writer. Settlement re-admits the same live session and
actor and checks the mailbox fact's age (at most 15 seconds old, at most 5 seconds future skew,
as on the same host). Missing, disabled, mismatched, stale or unavailable facts fail closed.
It atomically claims at most 100 live pending rows in ID order and records exact actor/subject/
epoch/version bindings, grants and a durable operation receipt. More pending rows require a
new explicit operation. Pending expiry is a claim deadline; a newly created claim grant has no
expiry. A pre-existing actor grant keeps its existing expiry.

The receipt is historical: the same operation returns its original grants with `replayed:true`
after revoke/restart and writes no authority. Querying another account by mailbox is impossible
on this route. All responses are `no-store`. No invitation mail is sent. This is a control plane;
restricted catalog/media serving remains disabled until S07. Error classes follow the
[Studio control plane](../../../studio/viewer-access/v1/README.md).
