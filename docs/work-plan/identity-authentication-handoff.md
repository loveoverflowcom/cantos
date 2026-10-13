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

## Historical PR2 implementation → PR3 / Library handoff

This section preserves the original PR2 dependency snapshot. Current integration inherits the
merged registry and develop Publication/Discovery/Library originals with unchanged checksums;
Studio adds migration0025 after0020 for independently reviewed issuance and browser cancellation.

PR2 implements [Studio OIDC](../architecture/studio-oidc.md), with exact client
`cantos-studio-web-bff`, current-only login/callback routes, private verifier witness, durable
consume-before-exchange and hashed PostgreSQL sessions. It does not enable a production host.
[Evidence](../evidence/studio-oidc.md) and [operator policy](../../services/kanidm/README.md) distinguish
mocked HTTP/actual crypto and PostgreSQL from NOT_RUN live Kanidm/browser/TLS gates. New migration
is **0020_studio_oidc.sql**; Library/Progress retains0019. Discovery PR93 @
`b07ab14e5ec52ebc711ea272dfa2bcb61c09fdfd` owns0017 and is not inherited by this branch.
StoragePR90/PublicationPR91 are not inherited either. Integrate their original SQL/checksums and
increasing migration registry only at the coordinated combined-tree review.

PR3 must define separate Listener credentials/verifier/session semantics. Reuse ActorId/exact
external keys and registry mapping after verified credential evidence; never Studio cookie,
StudioOidc, raw actor ID, operator bearer or caller identity headers. Kanidm pairwise subjects
and issuer/client paths may differ across clients; linking requires explicit authenticated intent,
not equal email. No Studio session grants private Library/Progress or Admin access.

The Library seam is now published in [PR95](https://github.com/loveoverflowcom/cantos/pull/95),
exact head `dc45b46f732c19948f01947039ec4b09fa8a7741`, tree
`b53ccafff81d1d98f4df368cbbaf41f9b5338dca`. Its
[official handoff](https://github.com/loveoverflowcom/cantos/blob/dc45b46f732c19948f01947039ec4b09fa8a7741/docs/work-plan/listener-library-progress-handoff.md)
and exact auth/application/router code were read for this handoff only; PR95 is not inherited
or merged into PR2. The implemented seam is
`contexts::listening::auth::{ListenerAuthenticator, ListenerCredentials, AuthenticatedListener,
AuthenticationError}`. `ListenerCredentials` is bearer-only, bounded and redacted; cookies never
enter the port. `authenticate(credentials)` returns a boxed Send + 'static future containing a
Result of the sealed witness; `AuthenticatedListener::from_verified_actor(OwnerId)` is
crate-visible for trusted adapters only, without Deserialize/public actor parsing. Default
`DenyListenerAuthentication` refuses private account operations before PostgreSQL. LibraryService
derives ListenerId ownership from the witness; implemented trusted HTTP composition is
`router_with_listener_identity(..., Arc<dyn ListenerAuthenticator>)`, preserving AppState/Studio
composition. Normal host composition still uses default deny. PR3 must use that **exact published
Library code**, rather than invent a parallel Listener authority. Migration0019 belongs to PR95;
its original SQL/checksum must join 0017/0018/0020 only at coordinated integration. The Library
handoff's own PostgreSQL/client acceptance remains separate; reading source proves neither.

Account/session fencing also belongs to PR3 acceptance: clear credentials/projections at logout,
never relabel account A outbox entries as B, ignore delayed responses across the account/session
boundary, and do not emit history merely from login/logout. Account-switch and provider
disable/revocation tests must exercise the actual Listener adapter and private routes. Issue34
has no accepted guest handoff policy; keep account-only persistence and no implicit guest merge.
Native system-browser PKCE/OS secure storage and Web/CMP runtime evidence remain PR3.

Session policy to preserve: current active/revoked local facts inside transaction, no failed-provider
success fallback, explicit expiry and logout linearization, cookie source matched to configured host.
Provider disable cannot be atomic with local writes; document its introspection observation boundary.
Live production adoption must additionally resolve bearer at-rest protection, pending remote
revocation recovery, retention/rate limits and provider SSO versus local/access logout semantics.

## PR2 independent integration handoff

The reviewed Studio source `850a820e7db224c41e2f4fd534301e83c0e1d5bf` incorporates merged
registry/develop `007ae8f0a2d82ead84c5c0b98eecc3b5dece36b3` and unchanged originals through0020,
with additive0025. All17 full local preflight gates report PASS, including actual disposable
PostgreSQL/HTTP, signed ES256 mocks, both WASM builds and9/9 mutation kills. Actual Studio browser
checks report11 PASS/0 FAIL, with explicit inconclusive text scaling and deferred accessibility,
provider and unobserved race gates. See the current
[integrated evidence](../evidence/studio-oidc.md#independent-integrated-acceptance-13-october-2026)
for exact source/tree, raw log hashes, fail-before regressions and production limitations.

PR3 must merge this actual branch/develop code and preserve both shared HTTP contract negotiation
and Studio UUID request/response fencing. The shared OIDC transport remains confidential for
Studio; native public-client transport cannot weaken Studio/Admin credential purpose. Immutable
per-flow private browser binders and lineage serialization do not authorize actor-wide logout.
Keep Theatre Web/native credentials, resource ownership and cancellation scopes independent.
The work queue continues PR3 then PR4; linked PRs own execution status. No production client,
credential, grant or network configuration was created by this integration.

## PR3 Theatre implementation → Admin

[Theatre identity handoff](theatre-identity-handoff.md) owns PR3's separate BFF/session adapter,
Web/CMP integration and exact implemented/deferred matrix. This source integration inherits the
reviewed Studio/develop branch above, with Publication/Discovery/Library originals and Library's
actual sealed authority seam. The PR3 migration union is1–14,16–21,24,25 (22 entries);0020/0021 and
all historical checksums remain unchanged. Native0024 and Studio0025 are additive. Admin0022
belongs to the succeeding PR4; artwork0023 remains reserved and absent here.

Shared `platform/oidc` reuses crypto/transport while Studio/Theatre session policies and SQL
remain independent. Studio/Theatre Web use confidential clients; the exact native public client
does not weaken their verification or Admin purpose/admission/role+audit decisions. Neither
Studio nor Theatre cookies/sessions, native bearers or producer grants confer Admin authority.
Preserve the current Studio UUID fences, JSON default/opt-in Protobuf and legacy MessagePack
contract when resolving shared code.

Current combined Theatre verification and independent acceptance are pending. Android INTERNET,
callback registration and host mounting remain unapplied after automatic approval review
rejected the source activation; the source-only proposal awaits a direct human exception.
Live provider/client/grants/credentials/TLS and actual Web/native device/secure-store acceptance
remain explicit gates. The parent review continues PR3 then PR4; this handoff claims neither
completion of the four-PR campaign nor production activation.

## PR4 current local source integration

[Admin authorization](../architecture/admin-authorization.md) adds its dedicated exact
`cantos-admin-api` access-token purpose, confidential introspection/current-group evidence,
existing registry actor, local policy and required append-only audit. Its revocation endpoint
changes only Admin access. Shared ES256 signature/Basic encoding plumbing supplies no role,
session or resource authority to another surface.

The Admin branch normally merges the reviewed Theatre/develop source above and retains each
independent host opt-in, Studio request/response UUID fence and negotiated wire contract. Its
migration union is1–14,16–22,24,25 (23 entries); published0022 bytes are unchanged and artwork0023
remains absent. Existing Admin local/mock evidence precedes this dependency integration; fresh
combined gates remain pending in the parent's resource lane. Keep the PR draft while Theatre's
Android source exception and required integration checks are unresolved. No push, remote merge,
production activation or live provider/account configuration follows from this local merge.
