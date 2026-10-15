# Work-scoped delegation fixtures

Synthetic transport examples for [delegation v1](../../../delegation-v1.md). The token in
`issue-created.json` is a repeating fixture pattern, never an issued credential; no fixture
contains a real actor, Work, secret or lookup hash. The shared wire test checks raw round trips,
closed fields and the closed action vocabulary against the
[schema](../../../schema/delegation/v1.schema.json). HTTP/PostgreSQL tests validate live
responses against the same schema; pure domain tests own lifetime, scope and decision rules.

| File | Shape |
| --- | --- |
| `issue-request.json` | Owner issuance request (`IssueDelegationRequest`) |
| `issue-created.json` | First response, the only one that carries `token` |
| `issue-replay.json` | Same-operation retry: identical grant, `token: null` |
| `grant-page.json` | Owner listing with one active and one revoked grant |
