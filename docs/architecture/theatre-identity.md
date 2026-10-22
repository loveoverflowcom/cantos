# Theatre provider identity — retired

Provider-only guidance is superseded by [Identity auth v1](../../packages/contracts/identity-auth-v1.md) and [private RPC v1](../../packages/contracts/identity-rpc-v1.md).
Source retirement removed the OIDC/Firebase enrollment handlers, provider setup and callback
exchange. The old commands/configuration are not supported native workflows. Original source-bound
requirements and observations are retained in [pinned history](https://github.com/loveoverflowcom/cantos/blob/b2611c3db3896ce9733280a319008924ee8bf974/docs/architecture/theatre-identity.md). They are not current
implementation or native/browser/device acceptance.

Preserve the live invariants at their current owners: exact ActorId binding without email
matching, purpose/surface isolation, exact Origin/CSRF, explicit mailbox confirmation, replay/
unknown-outcome reconciliation, revocation, immutable operation receipts and no registration-issued
Studio/Admin/resource authority. Web uses cookies; CMP/native uses opaque Bearer. Password
verification returns an Identity fact, never a Server session. Retired provider models supply
no acceptance for that flow.

Provider retirement authorizes no account, key, database/WAL, media or deployed-service deletion.
Firebase Hosting, Functions and App Distribution are independent products with their own owners.
