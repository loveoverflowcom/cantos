# Identity registry → Studio OIDC → Theatre identity

PR1 delivers internal mapping and explicit operator actions, governed by the
[registry contract](../architecture/identity-registry.md). It does not authenticate a provider.

## PR2: Studio OIDC

1. Introduce private verified credential evidence. Raw/deserialized claims or identity headers
   must have no path to trusted Studio authority. Verify signature/algorithm/kid, issuer,
   audience, time, state/nonce and PKCE; configure approved issuer/client pairs explicitly.
2. Use `IdentityService` after verified evidence. Keep exact key spelling and distinguish
   pairwise subjects across clients. Replay is stable; equal email never links a legacy owner.
   Linking needs authenticated old-account control and fresh new-account proof, or operator intent.
3. Design least-privilege runtime registry grants; PR1 deliberately denies HTTP-role access.
   `ActorId` is durable syntax, `ActorRecord` is a snapshot, neither is a credential. Recheck
   current active status on session issuance/requests and define disable/logout/revocation races.
4. Implement secure Studio BFF cookies, callback/JWKS rotation and mocked integration before live
   acceptance. Producer grants and Theatre/Admin authorization remain independent.
5. Live acceptance requires operator-prepared OAuth clients/grants/callbacks, credentials,
   private TLS and Kanidm runtime. PR1 authorizes no implicit Podman/certificate/account bootstrap.

Wrong issuer/audience/signature/algorithm/nonce, expiry and disabled accounts must not mint
sessions. Rotation/outages fail closed; legacy owners/history remain usable.

## PR3 and concurrent Listener library

Reuse pure ActorId/exact external keys, not Studio handlers/cookies/development tokens. PR3 owns
typed Listener authentication. Library/Progress may reference a stable actor only after that
authority exists; caller-declared IDs or registry lookup cannot authorize private library/history.
Public discovery/playback remains anonymous where product policy allows. Native login uses
system browser + PKCE and operating-system secure storage; client runtime acceptance is separate.

## Migration integration

- PR90: 0015 private storage; PR91: 0016 Publication, both open/draft on initial remote read.
- Discovery owner confirmed 0017_listener_discovery.sql, anonymous/public only.
- PR1: 0018_actor_external_identities.sql; Library/Progress uses 0019 or higher after coordination.

Reconcile `platform/postgres/migrations.rs` in increasing order when branches meet. Preserve all
original SQL checksums and run combined migration/replay/concurrency gates. PR1 applies after 0014
without the independent 0015–0017 schemas. No migration rename or old database/evidence cleanup.

The existing `cantos-operator` adds `register-identity ISSUER SUBJECT` (prints stable actor ID)
and `link-identity ACTOR ISSUER SUBJECT` (Linked/Existing or typed error). They require explicit
operator DB privilege, authenticate no provider, issue no session and grant no resource permission.
Use operator-supplied local configuration; tests always create disposable databases through
`cargo xtask verify postgres`. No new repository orchestration entrypoint is added.

See [PR1 evidence](../evidence/identity-registry.md) for local results and deferred acceptance.


## Integrated registry consumption

Use the independently reviewed registry integration at `47f5659`, retaining exact external keys
and existing actors/ownership. The integrated migration order is1–14,16–19; storage0015 remains a
separate prerequisite for its own PR. Keep all prior checksums. Credential consumers must use
fresh verified evidence and maintain separate Studio, Theatre Web/native and Admin authority.
The registry lock/disable regression and current full local acceptance are recorded in
[registry evidence](../evidence/identity-registry.md#independent-integrated-review). This changes
no production/provider/device activation gate or permission to register clients/credentials.
