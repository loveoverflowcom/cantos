# ADR 0007: Versioned casting and scoped production-input authorization

- Status: proposed for production adoption; bounded local implementation under review.
  Narrowed by [ADR 0018](0018-external-audio-production.md): a snapshot is guidance for an external
  producer, and any dispatch, reservation or provider-adapter statement below is superseded
- Date: 2026-10-10
- Decision owner: Cantos maintainers
- Related issue: [#5](https://github.com/loveoverflowcom/cantos/issues/5)
- Extends: [ADR 0003](0003-script-revision-persistence.md) and
  [ADR 0006](0006-ai-script-adaptation.md)

## Context

The existing modular Rust/Axum service owns authenticated actors, source/evidence links,
immutable accepted Script IR exports, revision history and explicit owner editorial review.
The Leptos Studio consumes that authority. Casting is external to Script IR. Its c1 content
digest deliberately excludes rights/provenance; a content digest alone cannot authorize
production.

The current scope is settings, rights/budget declarations and immutable production candidates.
The user controls Gemini / ChatGPT / Codex and invokes Cantos tools. No Ollama/model installation,
local inference, TTS call, real provider credentials, terms acceptance or payment is authorized
by this slice. The former local/open-source generation prerequisite is superseded, not passed.
Provider selection and actual adapter availability remain decisions for explicitly authorized
later work. A synthetic reference capability demonstrates control validation without making
claims about a provider's real voices, price, language support or audio quality.

## Decision

Extend the existing server package and shared `cantos-api` contract, rather than adding a
parallel revision, rights or accounting system. Store append-only versioned settings and
script-scoped rights declarations in PostgreSQL. The existing backend access table remains
authoritative: editors can record settings; owners make rights declarations, freeze candidates
and explicitly approve exact candidates. Reads apply current script access.

Resolve one accepted canonical Script IR revision into ordered per-dialogue inputs. Version
voice/provider/model assignment, permitted rate/pitch/emotion/intensity controls, pronunciation,
language, budget policy and caller-recorded rate. An absent performance override preserves
accepted Script IR delivery. Unsupported controls, unknown voice/model identity and invalid
pronunciation fail explicitly; no neutral voice or alternative model is silently substituted.
Pin the catalog, adapter and output-contract versions, including the reference-only/no-audio
boundary. This is neither a speech cache key nor a generated asset manifest.

Create an immutable `FrozenProductionDocument` with its owner, complete accepted revision
export/c1/e1 metadata, complete versioned settings, selected rights-declaration versions,
resolved dialogue inputs, estimate and `production-p1` digest. The digest covers the complete
serialized document except `input_digest`, including record identities/actors/times. It uses
compact UTF-8 JSON with recursively lexicographic object keys and a domain-separated SHA-256
preimage. It excludes transient evaluation findings and the evaluation clock. New digest
semantics require a new label; do not rewrite stored candidates.

Freezing records a reviewable candidate even when rights, cost or approval findings block
eligibility. It makes no provider call and grants no spend permission. A separate immutable
owner approval binds the snapshot ID and exact input digest. Eligibility is computed from
current facts rather than a persisted `approved`/`stale` boolean: script/settings/rights scope,
owner editorial review, rights terms, actor authority and approval must still match. Readiness
is an explanation; mutations re-read the relevant facts inside their transaction.

Rights declarations are private, script-scoped actor assertions for `production_synthesis`.
They identify evidence or one exact provider/model/voice subject, holder/reference, language,
territory, permitted scope, attribution/restrictions, status and validity interval. The current
territory is explicitly `private-planning`; these records do not grant public distribution,
another creator's reuse or a legal clearance finding. `pending`, missing, revoked, not-yet-valid,
expired or mismatched coverage blocks the relevant input. A null expiry is an explicit
open-ended term, never an absent permission. Re-evaluate time even without a new database write;
expiry is exclusive at its boundary. Append a new declaration/version to correct or revoke
one; frozen candidates preserve the earlier version while current eligibility fails closed.

Budget policy uses integer minor units (USD cents or VND dong), an exact nonempty scope and
explicit territory. No rate means an unavailable estimate, not zero. A caller-recorded rate is
a planning assertion, never a provider quote or actual receipt. Count Unicode scalar values
of each dialogue's resolved effective spoken text and round each rational line estimate upward
in integer arithmetic, then sum with checked bounds. This is an explicit planning policy because
dialogue is the future send unit, not a verified provider billing rule. An estimate equal to the
limit is within budget; excess blocks.
The domain rejects overflow and ambiguous numeric values. No reservation, provider attempt,
settlement or actual charge is created. A future adapter must reserve budget transactionally
before dispatch and settle against real receipt/reconciliation evidence; it cannot reinterpret
this estimate as a settled charge.

All writes carry caller-retained operation IDs. Exact actor/script/request retries reconcile
the original immutable receipt; a changed request under the same operation ID conflicts.
Settings, claim versions, revision and preview digest use explicit compare-and-swap guards.
Serialize concurrent decisions on the script, recheck current actor/membership/session facts,
then append facts and receipt together. Rollback leaves neither a partial candidate nor approval.
Process/database restart must preserve exact records and replay semantics. This local transport
and persistence decision does not settle production identity, deployment or provider behavior.

## Alternatives

| Option | Benefit | Cost or limitation | Decision |
| --- | --- | --- | --- |
| Existing revision/evidence authority plus append-only production records | One permission and revision boundary; concrete Studio consumer | Requires current-state reevaluation of immutable history | Selected |
| Casting/rights state embedded into Script IR | One file | Couples editorial interchange to production policy and provider controls | Rejected |
| Approve content digest only | Small identity | Excludes rights/provenance and ignores casting/cost scope | Rejected |
| Persist eligibility or silently substitute unsupported controls | Simple display or happy path | Approval drift, expiry/revocation bypass or a voice nobody selected | Rejected |
| Configure a real provider or accounting ledger now | Can generate audio | Requires provider/rights/spend decisions outside #5 | Deferred to the owning adapter/execution slices |
| DBSP read model | May maintain large repeated aggregates | No such measured consumer exists for bounded point/history reads; stale authority is unsafe | Deferred |

## Consequences and revisit conditions

Migration 0006 adds immutable settings, rights claims, snapshots and approvals to the same
PostgreSQL authority. Script IR `0.1.0`, source registry, c1/e1 and previous migrations remain
compatible. Snapshot history is bounded; known historical IDs remain inspectable under current
access. No media bytes, private manuscript fixtures, credentials or generated audio enter Git.

The [production v1 contract](../../packages/contracts/production-v1.md) owns wire names and bounds.
The [execution evidence](../evidence/casting-production-inputs.md) names the actual pure,
HTTP/PostgreSQL, crash/retry, independent oracle and Studio observations; an unrun gate is not
production adoption. No reference catalog entry can make `billable_dispatch_available` true.

Revisit before real dispatch, distribution/publication rights, a concrete provider catalog/rate,
reservation/receipt settlement, cross-script rights sharing, larger history workloads or a
production identity deployment. #6 follows only after #5 acceptance and explicit authorization
for its provider scope. Publication must recheck its broader source/voice/asset rights and
exact render approvals; a production-input approval never substitutes for those gates.
