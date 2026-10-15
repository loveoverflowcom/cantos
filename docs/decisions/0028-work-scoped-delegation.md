# ADR 0028: Work-scoped delegation with central opaque grants (first slice)

- Status: accepted for the first slice; implemented (L1–L5 local evidence below); Studio Web UI,
  further actions and production role provisioning are not part of it
- Date: 2026-10-15
- Decision owner: Cantos repository owner (selections recorded during the #113 implementation
  session); Catalog context maintainers
- Related issue or PR: [#113](https://github.com/loveoverflowcom/cantos/issues/113),
  [PR #136](https://github.com/loveoverflowcom/cantos/pull/136),
  [RFC](../architecture/work-scoped-delegation-rfc.md), [contract](../../packages/contracts/delegation-v1.md)
- Supersedes / superseded by: N/A. It narrows, not replaces, the RFC: every RFC question this
  ADR does not answer stays open.

## Context

The [RFC](../architecture/work-scoped-delegation-rfc.md) compared central opaque grants (A),
short-lived signed scope (B), attenuable capabilities (C) and hybrids without selecting one.
Known facts at the source baseline: Cantos runs one Axum API over PostgreSQL; Catalog already
serializes every Work/content effect on one `catalog_clock` row and authorizes inside the
transaction; producer read/upload grants already use domain-separated SHA-256 lookups of
256-bit bearer secrets; Studio session authentication returns a plain actor string, so
substituting a grantor's actor for a delegate would silently inherit owner power.

The owner selected, for the first backend slice: **direction A**, the **Catalog actions**
`work:read`, `content:read`, `content:write` on one Work, attribution to a **registered
delegate actor**, and **server-minted issuance through the Studio owner API**.

## Decision

1. **Credential.** The server mints 32 bytes from the operating-system CSPRNG and reveals
   `cantos_dlg_<64 lowercase hex>` exactly once. The prefix exists for secret scanning; it is
   not a checksum or MAC. PostgreSQL stores only
   `SHA-256("cantos/work-delegation/v1\n" ‖ secret bytes)`, which cannot collide with the
   Studio session hash (bare token text) or producer grants (another domain and input). Any
   other spelling, including a 64-hex session or producer token, is not a delegation credential.
2. **Scope.** One grant names exactly one Work, a non-empty set of the three closed actions,
   one registered delegate actor (an existing active `actors` row distinct from the grantor)
   and `expires_at` at most 30 days after the server's issuance time. Review, publication,
   rights, membership, archive, grant issuance, Script/source/brief/audio/artwork actions have
   no representation. Unit/revision pins, quotas and child grants are not part of this slice.
3. **Issuance.** Only the current Work owner, through an Origin-checked Studio session, issues,
   lists and revokes. Issuance is idempotent per `(grantor, operation_id)`: a retry with the
   same canonical request returns the grant with `token: null`; a changed request conflicts. A
   lost secret is never redisplayed: the owner revokes and issues again. Archived Works cannot
   receive new grants. Grants are append-only: no edit, widen or extend.
4. **Current-rights contract.** Each delegated request takes the Catalog clock (share for reads,
   update for writes), resolves the grant by lookup, share-locks the grantor and delegate actor
   rows, reads the database clock and asks the pure core for a permit: not revoked, issued ≤ now
   < expires, both actors active, resource's authoritative Work equals the granted Work, the
   grantor still owns it, the Work is not archived, and the action is granted. The same facts
   are re-resolved before commit. There is no positive cache.
5. **Revocation linearization.** Revocation takes the Catalog clock for update. A request
   admitted before the revocation commits may finish; every request that acquires the clock
   after it sees the revocation. Revocation never retracts a committed receipt or bytes already
   returned. Deactivating either actor waits on the share locks and denies later requests.
6. **Attribution.** A delegated write is recorded by the delegate actor (`recorded_by`, Work
   revision and operation receipt), never by the grantor. `catalog_delegated_operations` links
   the receipt to its grant and therefore to the grantor. The grant ID is part of the replay
   fingerprint, so another grant cannot replay the receipt.
7. **Delegated writes** may carry only text or recording bodies: Source and Script links belong
   to Ingestion/Editorial rights that a Catalog grant does not convey. Rights references keep
   Catalog's existing owner-only change rule.
8. **Transport.** Delegates use `/api/v1/studio/delegated/*` with exactly one
   `Authorization: Bearer` header. A cookie or Studio session fence on the same request is a
   refusal, and the bearer is refused on every cookie route. Only the exact delegated append
   path is exempt from the browser Origin barrier, because it cannot carry a cookie. Lifecycle
   denials map to 401, a foreign or missing resource to 404, a missing action or a forbidden
   link/rights change to 403.

The pure core is `contexts/catalog/domain/delegation.rs`; the shell is
`contexts/catalog/application/delegation.rs` with its SQL adapter; HTTP only extracts the
bearer and maps outcomes. Migration `0026_catalog_delegation.sql` is additive.

## Alternatives

| Option | Benefits | Costs and limitations | Reason selected or rejected |
| --- | --- | --- | --- |
| A: central opaque grant (this ADR) | revocation fenced in the effect transaction; reuses Catalog clock, SQL and producer-grant patterns; no new dependency or key custody | per-request lookup and two actor share locks; revocation briefly serializes with all Catalog requests; bearer is transferable | selected by the owner for the first slice |
| B: PASETO/JWT signed scope | proof verifiable without a lookup | current rights, revocation and archive still need the same online facts; signing-key custody and rotation; no distributed verifier exists | rejected for now; revisit with a real second verifier |
| C: Biscuit/Macaroon attenuation | holders derive narrower children offline | verifier/parser budget, chain revocation semantics, new dependency; no sub-agent consumer exists | rejected for now; revisit when Forge coordinates sub-agents |
| Hybrid (grant + existing one-submission upload capability) | keeps upload's exact pins | needs `audio:prepare` semantics and Media review | deferred; producer grants are unchanged |
| Authenticate the bearer as the grantor's session actor | no new authorization code | every owner-only path and receipt would treat the delegate as owner | rejected; the failure mode the RFC names |
| Mutable grants (edit actions/expiry) | fewer grants to manage | silent broadening and ambiguous attribution | rejected; revoke and reissue |

## Consequences

- Product and user impact: an owner can give an AI editor or collaborator bounded Catalog
  read/content-write access to one Work without sharing a Studio cookie. Studio Web has no
  management UI yet; the API is the only issuance surface.
- Compatibility and migrations: additive migration 0026 (three append-only tables, immutability
  triggers, `REVOKE ALL FROM PUBLIC`). Existing sessions, producer read/upload grants, Admin
  and Catalog routes keep their behavior; the shared append core is a pure extraction.
- Operations, storage, and production cost: production role provisioning must grant the HTTP
  role `SELECT, INSERT` on `catalog_delegation_grants`, `catalog_delegation_revocations` and
  `catalog_delegated_operations` (deployment provisioning remains separate, as for other
  slices). Per delegated request: one unique-index lookup, one revocation join, two actor share
  locks. No last-used column, denial audit or rate limit exists.
- Implementation and recovery requirements: rotate a leaked credential by revocation; a lost
  issuance response by revoke + reissue. Rolling back the application leaves the tables unused;
  rows are retained as history.
- Deferred questions: unit/revision pins, further actions (`script:*`, `brief:read`,
  `audio:prepare/upload`, `artwork:upload`), sub-agent attenuation, quotas, last-used/denial
  audit retention, Studio Web UX, Forge client support and the Quint capability for this
  contract.

## Validation and revisit conditions

Local evidence for this slice is recorded in the
[implementation evidence](../evidence/work-scoped-delegation-implementation.md): the pure core
is `example-tested` with bounded exhaustive admission/action-subset loops and an independent
hash oracle; the PostgreSQL/HTTP lane is `integration-tested` against a disposable PostgreSQL
17.11 cluster, including two lock-interleaving revocation tests; hand mutations record which
assertions kill which defects. Production identity/role provisioning and a Studio Web UI are
not accepted here. Revisit when a second verifier, a sub-agent workflow, an action outside
Catalog or a measured latency problem appears.
