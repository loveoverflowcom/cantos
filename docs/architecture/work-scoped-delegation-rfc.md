# Work-scoped agent delegation — decision-pending RFC

- Status: Research / decision pending. **No mechanism, algorithm, token format, schema,
  endpoint, TTL or deployment is selected.** This is not an implementation contract or ADR.
- Issue: [#113](https://github.com/loveoverflowcom/cantos/issues/113).
- Source baseline: `develop` at `b6066acc659b6d4cd030badb9c1abdad08b6dcf2`, fetched
  14 October 2026. This supersedes the issue's older `3668cb509ff` observation for this review.
- Scope: decision-neutral comparison, actual boundary inventory, proposed typed policy
  responsibilities and disposable synthetic experiments. No production source, migrations,
  credentials, configuration or existing security defaults change.

## Question to decide

Which objective justifies additional machinery at Cantos's current scale: centrally revocable
delegation, distributed short-lived verification, or offline attenuation by sub-agents?
The product is one primary creator/operator, not an implicit global root account.

An external editor/producer may receive only an explicitly granted Work UUID, optionally an
exact unit/revision/Brief/submission, selected operations and bounded time. Studio cookies and
platform Admin credentials must never enter Forge or an untrusted producer environment.
The [research evidence](../evidence/work-scoped-delegation-research.md) supports comparison,
not selection; [the handoff](../work-plan/work-scoped-delegation-handoff.md) orders the next gates.

## Existing authorities are different

| Authority | Implemented boundary | What it does not establish |
| --- | --- | --- |
| Human Studio | [Studio OIDC BFF](studio-oidc.md), current session/actor, DB revocation and provider observations | Admin, producer or arbitrary Work-grant authority |
| Platform Admin | [Admin API](admin-authorization.md), dedicated ES256 resource token, live introspection, narrow local Admin block and required audit | Work ownership, general ACL, root token or global logout |
| Catalog | [Catalog application](../../services/cantos-api/src/contexts/catalog/application/mod.rs), owner/member policy and transaction-time fences | Independent Editorial or Ingestion rights |
| Editorial / Ingestion | [Script authorization](../../services/cantos-api/src/contexts/editorial/application.rs) and [source access](../../services/cantos-api/src/contexts/ingestion/application.rs) | A Catalog link alone does not reveal unrelated private script/source bytes |
| Producer read | [Brief contract](../../packages/contracts/production-brief-v1.md), exact immutable read-only Brief, at most 24h, domain-separated lookup | Upload, owner, reviewer or publisher |
| Producer upload | [Audio Lifecycle](../../packages/contracts/audio-lifecycle-v1.md), exact prepared submission, at most 24h, revocable and actual-byte inspected | QC/release, private owner preview or another submission |
| Media object access | [Private storage](object-storage.md), exact recorded target/key/hash/size through authorized metadata | Work permission from a key prefix; direct presigned writes |

Current producer contracts require caller-supplied 32 random bytes represented as 64 lowercase
hex characters; issuance checks syntax/reuse, not the caller's entropy source. Only purpose-separated
SHA-256 lookups persist. They are not server-minted
one-time-reveal Work PATs. Their narrow guarantees are comparison baselines, not selected
formats for this RFC. Historical migration counts in older contracts are not the current registry.

### Actual call-site inventory at the source baseline

These are migration/review boundaries, **not endpoints authorized for delegation by this RFC**.
Names identify current owning functions; family links cover associated queries and mutations.

| Family / source | Current access/effect boundary | Required delegation review |
| --- | --- | --- |
| [HTTP Studio assembly](../../services/cantos-api/src/interfaces/http/studio/mod.rs) and [middleware](../../services/cantos-api/src/interfaces/http/mod.rs) | Owner routes use cookie/session middleware; producer routes are separately merged; old script routes retain deprecated aliases | Explicit credential purpose and route mapping, duplicate/mixed credential refusal, no cookie fallback or new legacy aliases |
| [Catalog commands](../../services/cantos-api/src/contexts/catalog/application/commands.rs) `catalog_execute` | Work/unit create/edit/patch/archive/reorder and internal `SetMember`; session, Catalog clock, actor/Work facts, operation receipt, final authentication | Closed operation mapping; no create-Work/membership/rights mutation inferred from content write; actual caller and grant lineage in replay identity |
| [Catalog queries](../../services/cantos-api/src/contexts/catalog/application/queries.rs) | Work/unit/list/history; actor/query/epoch-bound cursors | Restrict list joins, history and cursor identity to granted resources; a Work A token must not enumerate other memberships |
| [Content](../../services/cantos-api/src/contexts/catalog/application/content.rs) `append_content`, revision/history | Unit resolves authoritative Work; separate source checksum, exact owned Script revision/export and rights evidence | Same Work ancestry plus explicit content operation; no cross-Work private source/export through a supplied link |
| [Owner Work facade](../../services/cantos-api/src/contexts/catalog/application/owner_work.rs) | `owner_work`, `advance_owner_work`, `recheck_owner_work` are owner-only Media consumers | Do not fabricate `OwnerWork` for a delegate; add a reviewed, action-specific consumer seam if needed |
| [Editorial](../../services/cantos-api/src/contexts/editorial/application.rs) `load/save/history/review` | Script owner/member `Read/Write/Review`; review owner-only | Grant must bind an authoritative Work/unit → exact ScriptRef plus independent Editorial rights; unlinked standalone script stays inaccessible |
| [Ingestion](../../services/cantos-api/src/contexts/ingestion/application.rs) | Import operation belongs to actor; original/import reads owner-scoped; `source` requires authorized script link | Work scope cannot authorize owner-wide imports or originals without a new reviewed exact-resource relation |
| [Adaptation](../../services/cantos-api/src/contexts/adaptation/application/mod.rs) and accept/context/submit/legacy modules | Actor-scoped frozen import/run, proposal, accept/cancel and exact source checks | No Work relation is sufficient by itself; classify separately or leave unavailable to delegates |
| [Production](../../services/cantos-api/src/contexts/production/application/mod.rs), casting/freeze/approve | Script authorization, settings/rights, frozen snapshots and owner approval | Separate planning edits from rights declarations, approval and freezing; do not inherit all owner actions |
| [Briefs](../../services/cantos-api/src/contexts/production/application/briefs.rs) | Owner create/read/issue/revoke; `producer_instructions` checks exact scope/current Brief and clock | Proposed `brief:read` is not existing `read_instructions`; decide whether grant issuance is human-only |
| [Owner audio intent](../../services/cantos-api/src/contexts/media/application.rs) | Prepare/status/upload; inspect/store outside transaction, then owner/session/exact revision recheck | A Work grant must not directly authorize this owner path by substituting grantor actor |
| [Audio Lifecycle](../../services/cantos-api/src/contexts/media/lifecycle_application.rs) | Prepare/status/revoke/review/candidate; independent `scoped` producer PUT/status; final recheck after I/O | Decide `audio:prepare` → existing exact submission capability; keep review/candidate/publication independently gated |
| [Private reads](../../services/cantos-api/src/contexts/media/read_application.rs) | Resolve exact owner object; `open_verified`/`temporary_read`; re-resolve after object I/O | Recheck all grant lineage/current facts before releasing a stream/URL; cannot recall bytes already released |
| [Artwork](../../services/cantos-api/src/contexts/media/image_application.rs) `prepare/begin_upload/admit/set_artwork/preview` | Catalog owner facade, inspector, private storage, final reauthorization and Work revision CAS | Upload and selection are different operations; no delegate preview or metadata selection inferred from upload |
| [Publication](../../services/cantos-api/src/contexts/media/publication_application.rs) | Rights, approval, readiness, publish/active/withdraw with current owner and immutable receipts | `publication:stage` is a proposed label, not publish permission; retain human-only decision hypothesis |
| [Storage port](../../services/cantos-api/src/contexts/media/storage/port.rs), [S3](../../services/cantos-api/src/contexts/media/storage/s3.rs) | Verified payload, conditional content-addressed write/readback, verified reads, optional temporary reads | No caller-selected target/key, ACL mutation, object deletion or direct-upload API |

The common danger is [session `authenticate`](../../services/cantos-api/src/platform/postgres/session.rs)
returning a plain actor string. `authenticate(delegated_token) → grantor_actor` would cause
existing owner comparisons, owner-wide lists and receipt attribution to grant unintended power.
An identity lookup, arbitrary signed `sub`, email or caller header cannot construct authority.

## Threat model and comparison

Trust the configured verifier/issuer, owning-context fact resolvers and metadata transaction;
do not trust token holders, raw DTOs, arbitrary token facts, object keys, titles/slugs or provider
success metadata. Database/service compromise and owner account takeover need their own recovery
controls; this RFC does not claim to solve them. All bearer options remain transferable when
stolen. A token identifies a credential/grant, not necessarily the human or process presenting it.

| Dimension | A: central opaque PAT | B: short-lived signed scope | C: attenuable capabilities | Viable hybrid to evaluate |
| --- | --- | --- | --- | --- |
| Proof | Random secret → purpose-separated digest lookup; public ID/prefix optional | PASETO v4.public or purpose-separated EdDSA JWT; exact verifier keys/type/claims | Biscuit signed blocks or Macaroon chained caveats; verifier must enforce every inherited restriction | A root grant plus exact existing upload capability; alternatively ephemeral B/C child |
| Current Work rights | Online grant/grantor/delegate/resource facts | Signature cannot establish current rights; online facts still needed unless owner explicitly accepts stale authorization | Offline attenuation does not establish current parent trust, ownership or lifecycle | Same online fact requirements; root and child both matter |
| Revocation | Local DB fence can serialize admission against revoke; no positive cache in strongest hypothesis | Pure offline verifier can accept until expiry; online revoke restores A-like dependency | Parent/root revoke must reject every derived chain; child revoke needs explicit policy/state | Child cannot outlive root revoke in API policy; URL already issued has its separate lifetime |
| TTL | Product-selected bounded lifetime; 7 days in issue journey is a scenario, not default | Short proof lifetime plus separately reviewed issuance/renewal; no automatic refresh scope | Effective interval intersects every ancestor/caveat | Minimum of root/child/domain capability; existing upload maximum stays 24h |
| Offline delegation | No; central issuer mints narrower children if approved | No holder attenuation for ordinary signatures; central minting | Yes, chief distinguishing benefit; holders can still retain/reuse the original parent | Added machinery justified only by demonstrated sub-agent need |
| SQL footprint | Digest/grant lookup + current actors/ownership/ancestry/revoke facts; final recheck | May remove secret lookup, not current facts; online `jti`/lineage checks may restore a lookup | Current root/parent-revoke facts + ancestry; chain verification and rule evaluation | Extra root/child joins and replay/audit identity; measure before adoption |
| Latency / outage | DB outage denies; benchmark real locked queries, not hash speed alone | Proof can validate without DB; mutation still denies if required current facts unavailable | Public-key Biscuit proof can verify remotely; Macaroon verifier shares secret custody; required state outage still denies | More failure surfaces, not assumed faster or more available |
| Size / resource limits | 32-byte secret; encoded length depends on chosen UX envelope | Claims + signature; payload is readable for public signed formats; cap encoded length/JSON depth | Size grows with caveats/blocks; cap bytes, depth, facts, rules, iterations and verifier time | Bound every component and total Authorization header; no compression shortcut |
| Keys / rotation | SHA lookup has no server lookup key; HMAC requires purpose-separated key IDs and dual-key migration; BLAKE3 choice still open | Trusted service/KMS custody, exact `kid`, overlap/end-of-trust and emergency revoke; never export private key to Forge | Biscuit root signing-key rotation and chain protocol; Macaroon root shared-secret custody/rotation | Independent purposes/keys; extra rotation runbooks and stale-child handling |
| Operator / UX | List public IDs, one-time secret reveal, revoke/replace, safe last-used metadata; handling lost issuance response needs a policy | Same user-level controls plus issuer availability, key inventory and renewal | Show lineage and effective restrictions, not raw rule editing by default; origin/delegate identity needs independent evidence | One root view plus exact upload status; do not hide descendant rights |
| Audit / incident | Public grant ID, verified caller if known, grantor, exact resource/action/outcome; no secret/hash | Same plus trusted key/credential-purpose identity; `sub` is not automatically a registered actor | Include verified chain/revocation identifiers and reliable attribution; holder-appended name is not authentication | Link parent → exact child → operation/receipt; revoke root without deleting history |
| Rollback / migration | Additive reviewed tables/functions only after ADR; replace/reissue safer than in-place broadening hypothesis | Versioned readers, dual trusted-key window, child expiry/revoke before retiring a verifier | Versioned block/caveat grammar; deny unsupported semantics, finite parser/evaluator budget | Drain/expire/revoke children before disabling root path; preserve current producer grants |

These cost/fit conclusions are architectural **inferences**, not measured production performance
or selections. SHA-256/HMAC/BLAKE3 speed does not rank cryptographic security. A 256-bit uniform
secret differs from a human password; password-hashing assumptions are not automatically relevant.
CRC32, if retained for entry/scanning UX, detects accidents and is not authentication or a MAC.
An HMAC lookup key may limit use of a stolen lookup database, but is another secret to rotate;
the primary defense against guessing an opaque secret is adequate entropy and safe handling.

### Protocol and Rust feasibility evidence

- [RFC 8725](https://www.rfc-editor.org/rfc/rfc8725.html) motivates algorithm allowlisting,
  explicit purpose/type and mutually exclusive validation. [RFC 8037](https://www.rfc-editor.org/rfc/rfc8037.html)
  defines EdDSA JOSE plumbing; neither is a Work-authorization contract.
- [PASETO v4 specification](https://github.com/paseto-standard/paseto-spec/blob/master/docs/01-Protocol-Versions/Version4.md)
  fixes the public protocol's Ed25519/pre-authentication encoding. Existing JWT transport is
  not PASETO interoperability evidence. [pasetors docs, observed 0.8.1](https://docs.rs/pasetors/latest/pasetors/)
  expose typed public/version APIs and note that custom claims require application validation.
- [biscuit-auth 6.0.0](https://docs.rs/biscuit-auth/6.0.0/biscuit_auth/) documents offline
  attenuation, public-key verification and external revocation state. [Its revocation guide](https://www.biscuitsec.org/docs/guides/revocation/)
  describes inherited identifiers. No library alone supplies Cantos's atomic revoke/commit fence.
- [macaroon 0.3.0](https://docs.rs/macaroon/0.3.0/macaroon/) documents conjunctive first-party
  caveats/discharges and depends on `sodiumoxide`; evaluate native dependency and parser/caveat
  interoperability, rather than treating it as an interchangeable Biscuit verifier.
- Cantos already locks `getrandom 0.3.4`, `sha2 0.10.9`, `hmac 0.12.1`, `ring 0.17.14`
  and `base64 0.22.1` in [the API manifest](../../services/cantos-api/Cargo.toml).
  [`ring` Ed25519 verification](https://docs.rs/ring/0.17.14/ring/signature/static.ED25519.html)
  is available as a primitive, **not** an existing purpose-separated Work JWT issuer/parser.

Version/documentation availability is not a maturity/security audit. Before selecting a new
crate: pin version and features in a disposable crate, compile with Rust 1.98.1 on the API and
Forge targets, run official vectors/hostile inputs, review maintenance/security advisories,
licenses, transitive dependencies and limits, and record migration/version interoperability.
No such Rust compatibility campaign ran here; no dependency or lockfile changed.

## Proposed typed principal/policy contract, not wire DTOs

The invariant to review is: **credential proof AND current delegate/grantor authority AND exact
resource ancestry AND allowed action AND time/revocation AND existing domain invariants**.
Any unresolved or unavailable conjunct refuses the operation.

| Conceptual value | Construction / responsibility | Must not permit |
| --- | --- | --- |
| `AuthenticatedPrincipal` | Closed, surface-specific HumanStudio or Delegated alternatives; only the corresponding verifier/resolver can construct proof | Raw deserialization, unknown `sub`, or producer/Admin proof becoming Studio owner |
| `DelegatedAuthority` | Private witness with public grant ID, credential purpose, grantor, independently verified delegate identity **or explicitly bearer-only caller**, scope, issued/not-before/expiry bounds and lineage | Converting grantor into real caller; claiming known agent identity merely from token text |
| `ResourceFacts` | Owning contexts resolve canonical Work/unit/revision/Script/source/Brief/submission relations and current authority | Trusting request Work/slug/key prefix, holder-added facts, or Catalog links as independent rights |
| `DelegationDecision` | Pure function over admitted proof, immutable scope, fresh facts, requested typed action and explicit clock | SQL/HTTP/provider calls, global owner/admin role shortcut, unknown action wildcard |
| `AuthorizedAction` | Private request-bound permit: principal, exact resource/action/pins, validity and observed/fenced current-fact identity | Serializing as a reusable credential; consuming after an I/O gap without required recheck |
| Typed refusal | Owned closed outcomes for invalid proof/purpose, missing authority/ancestry, action/time/revoke, stale/corrupt facts and unavailable I/O | String errors, silent defaults, stale-proof fallback, or unexpected fault becoming success |

This taxonomy is a design sketch: names/layout, constructors and error precedence require review,
not a new generic policy framework. Keep domain errors exact internally and use existing safe
HTTP projection rules. Distinguish invalid caller fields from corrupt persisted accepted facts.
This design sketch does not establish type-enforced or compiled authority barriers.

Scope must carry an exact canonical WorkId; optional unit/revision/Brief/submission pins narrow,
never widen it. Treat child restrictions conjunctively with every inherited restriction, never
union child rights into parent rights. Unknown action, scope version, fact provenance or policy
grammar fails closed. A bounded chain must preserve grantor/purpose and cannot extend expiry or
restore a removed action. Cryptographic attenuation does not destroy the holder's original
parent credential; operational separation still requires delivering only the intended child.

Kanidm authenticates humans; Admin retains platform policy. Catalog owns Work/membership and
unit ancestry; Editorial owns Script/source links and its access rules; Ingestion owns imported
source privacy; Production owns exact Brief/freshness; Media owns actual-byte admission and exact
storage location; Publication owns approvals/releases. A credential verifier only proves its
credential purpose. Application/PostgreSQL resolve/lock current facts; the pure core decides;
HTTP extracts bounded raw syntax. Forge is an external consumer, never the issuer's key custodian.

Proposed closed actions from #113 remain review candidates: `work:read`, `content:read/write`,
`script:read/write`, `brief:read`, `audio:prepare/upload`, `artwork:upload`, `publication:stage`.
They are **not** current API scopes. Require a per-operation map before exposing one. Review,
publish/withdraw, rights/membership changes, grant issuance, owner transfer/recovery and platform
administration remain human-owner/operator-only as a **hypothesis for the owner to accept**.

## Revocation, time and effect boundary to choose

The strongest online candidate serializes local revoke and effect admission through a reviewed
shared DB fence. Acquire grant lineage/actor/resource locks in one documented order compatible
with Catalog's current clock lane; resolve current owner/delegate/parent state and fresh time
**after every blocking wait and at the final admission point**. Do not invent a second lock order.
An operation admitted before revoke obtains the conflicting fence may finish first. Revoke is
effective for later admissions; it cannot retract an earlier committed receipt or delivered bytes.

Object inspection/PUT/GET happens outside metadata transactions. After it, reacquire facts,
verify unchanged exact pins/target and reauthorize before a new receipt, stream or URL is released.
Failure may leave a private unreferenced/shared object, never publication or false success. Do not
delete content-addressed bytes on rollback. A reauthorization snapshot before I/O is insufficient.
For prepared upload children, choose whether parent revoke continues to fence them; detaching a
child must be an explicit independently approved grant, not an accidental default.

Remote identity observation cannot be made atomic with a local DB commit. Specify its accepted
observation age, final-clock/clock-regression behavior and outage denial separately. Admin's
five-second witness is **not** automatically the Work contract. Reads already streaming and
presigned URLs already issued need their own leakage/revocation bounds, not an "instant revoke"
claim. Hard call/byte/upload quotas need transactional reservation/settlement and replay rules;
rate limiting and unverified optional fields cannot masquerade as those quotas.

Audit durability needs an explicit policy decision too. The mutation hypothesis is one atomic
effect + immutable operation receipt + required audit, with no success on audit failure. A denied
request whose audit cannot persist must still deny; an ambiguous commit requires original-operation
reconciliation, not another grant or effect. Decide read/denial/last-used audit scope and retention
cost separately. Preserve actual caller (or explicitly bearer-only identity), grantor and lineage
without logging full credentials, secret hashes, private manuscript text or presigned URLs.

## Issuance and incident UX alternatives

| Option still open | Benefit | Review gate |
| --- | --- | --- |
| Authenticated Studio owner UI | Resource context, one-time reveal, revoke/list/last-used alongside the Work | Current owner/session/re-auth policy, CSRF, browser secret handling, lost-response recovery |
| Trusted operator CLI | Small automation surface and auditable explicit intent | Human/operator identity plus exact Work ownership, secure delivery; no global root bypass |
| Separate Admin-authorized endpoint | Operational separation if product requires it | Admin alone is insufficient: explicit independent Work-grantor policy and recovery authority |
| Combination | Different owner/operator workflows | Shared core/receipt rules, no less restrictive fallback or duplicate issuers |

Public IDs and safe labels can be listed; full secrets should be revealed once and excluded from
URLs, logs, receipts, diagnostics and manuscripts. Issuance retries/lost responses must not
silently mint another credential or redisplay a persisted plaintext secret. Choose secure
replacement/reissue or a reviewed bounded delivery mechanism. Grant rotation/revocation preserves
immutable operation attribution. Last-used update cost, audit retention, compromised-key/grant
response and owner recovery/bootstrap are independent operator decisions, not account creation here.

## Presigned writes are a separate optional optimization

Today [Media storage](object-storage.md) exposes verified API-proxied writes and optional reads,
not PUT/POST grants. [AWS's presigned URL contract](https://docs.aws.amazon.com/AmazonS3/latest/userguide/using-presigned-url.html)
describes transferable, time-limited access; successful signing is not actual provider acceptance.
No Long Vân/S3 credentials, endpoints or live requests were used in this research.

Compare API proxy with provider SigV4 PUT and constrained POST only after an authorized disposable
provider test. Verify actual supported conditional-write/checksum/size/content-type/method/key
constraints, signed headers, CORS if a browser is a consumer, short TTL, replay, cancellation,
multipart behavior and cleanup. A custom HMAC URL is not S3 interoperability. A checksum or PUT
success does not replace server readback/hash/length/profile inspection before metadata admission.
Provider overwrite/shared-key risk makes direct writes to settled content-addressed keys unsafe
without an established immutable-write guarantee; a staged exact-intent key is an alternative to
evaluate, not a selected schema. Uploaded orphans stay private and cleanup must preserve references.
An already issued URL can remain usable until provider expiry independently of application revoke.
The issue's 5–15 minute TTL is a test hypothesis, not a shipped policy.

## Decision gates

1. Owner chooses the optimization objective and explicit allowed operations/issuance surface;
   determine bearer-only versus independently authenticated delegate attribution.
2. Compare A/B/C and hybrids under the **same** accepted current-rights, revoke, TTL, outage and
   parent-child contract; do not compare weaker offline authorization with stronger online A.
3. Record actual Rust compatibility, token sizes/limits and locked-query/read/write/audit costs;
   current mechanics experiments cannot supply production latency or crate selection.
4. After semantic review, add a `needs-decision`/draft Quint capability using the existing catalog,
   with named runs, reachable witnesses and sensitive controls; acceptance is a separate step.
5. Final ADR explicitly selects/rejects alternatives and records migration, rollout/rollback,
   operator/audit/recovery and deferred acceptance. Only then create bounded implementation work.

Until those gates pass, keep all existing credentials, APIs, private storage and production
activation restrictions unchanged. This RFC neither closes #113 nor enables general delegation.
