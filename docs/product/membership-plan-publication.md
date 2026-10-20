# Feature: Read-only Membership plan publication

- Status: draft — proposal only; no source or wire contract accepted
- Owner: product decision owner in [#34](https://github.com/loveoverflowcom/cantos/issues/34);
  implementation/review owner unassigned
- Issue: [#176](https://github.com/loveoverflowcom/cantos/issues/176)
- Related decisions: [Membership interest](membership-interest.md),
  [API surface boundaries](../decisions/0015-api-surface-boundaries.md)

## User outcome and review boundary

Listeners can optionally inspect approved Membership terms from Home/introduction, Account,
account-registration completion or an interest receipt, without buying anything or acquiring
access. Until an approved source exists, the current presentation says **Chưa công bố**.
The [existing interaction contract](membership-interest.md#interaction-contract-from-issue-128)
owns Back destinations and independence from registration, receipt and playback state.

This proposal prepares the product/source decision and implementation acceptance matrix for
#176. It introduces no endpoint, plan DTO, schema, database, catalog entries or live client
adapter. It does not complete #176's implementation acceptance. No price, benefit, trial,
discount, plan identity or commercial eligibility has been approved by this document.

## Inspected baseline and consumers

Baseline: `develop@6818642a7c62cb35c9da04ed6d7ac2daeb1008b1`.
Issue #176 and product owner #34 contain no decision comments at the inspected snapshot.
Closure of #128 or #129 does not accept commercial terms or backend publication semantics.

| Consumer / owner | Inspected implementation | Gap before adoption |
| --- | --- | --- |
| Theatre Web, #128/#129 | [Membership preview](../../apps/cantos-theatre/web/src/membership_preview.rs) renders `Unpublished`, `Loading`, `Empty`, `Unavailable`, `Failed`; its Monthly/Yearly cards contain unpublished copy | Preview states/cards are not public catalog records; no `published` projection reader exists here |
| Theatre CMP, #128/#130 | [MembershipInterest.kt](../../apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/domain/membership/MembershipInterest.kt) has the same five plan states and independent `PlansOrigin` | Presentation/domain preview state is not a wire DTO or paid access authority |
| Anonymous API | [Theatre router](../../services/cantos-api/src/adapters/inbound/http/theatre.rs) serves discovery/manifest/playback routes; [composition](../../services/cantos-api/src/adapters/inbound/http/mod.rs) mounts separate Theatre identity/Library routers | No plan publication endpoint is implemented; preserve separate Theatre routing and never reuse a Studio handler/session |
| Membership contact / identity | [Four separate facts](membership-interest.md#four-separate-facts), #128/#129/#130 | This read-only projection needs neither an interest receipt nor contact/profile data; signup/provider acceptance remains separate |

## Source alternatives — owner selection required

No option is selected. The owner must record a selection and reasons before a storage or wire
shape becomes authoritative. A future implementation keeps deterministic validation/projection
inside the existing modular monolith and source I/O at its edge; this proposal adds no context,
crate, service, dependency or generic catalog abstraction.

| Option | Suitable when / benefits | Costs, failure and recovery questions | Evidence needed to select |
| --- | --- | --- | --- |
| A. Versioned checked-in public manifest/configuration | Terms change infrequently; code review can approve a complete public snapshot | Public Git history cannot erase withdrawn terms. Define approval identity, validation, rollout consistency across API instances, reload rejection and withdrawal speed. Keep private drafts outside public Git | Product accepts public historical retention and deploy/reload latency; demonstrate atomic snapshot activation and rollback |
| B. PostgreSQL draft → approved → published catalog | Named operators need changes independent of deployment and private drafts | Adds authoring authority, immutable approval binding, transactions, concurrent publication fences and retention/backup obligations. A public reader must project only the active approved snapshot | Product names operators and workflow; review authorization, draft isolation, upgrade/rollback and concurrent publication before schema |
| C. External commercial catalog projection | An existing accepted source actually owns terms | Provider credentials, latency, privacy, availability and upstream revisions cannot be assumed. Remote calls must not occur during playback or supply access authority | Separate provider/policy/provenance and latency/privacy review; deterministic snapshot import, failure/withdrawal and cost boundaries accepted first |

## Decision matrix before a versioned contract

These are approval questions, not default values. #34's product owner resolves product choices;
the #176 implementation owner and Web/CMP consumers review the resulting contract together.
Unassigned roles require assignment before acceptance.

| Decision | Options / trade-off to resolve | Required recorded outcome |
| --- | --- | --- |
| Locale and currency | One explicit launch locale/currency vs a bounded supported set; fallback vs explicit unavailable locale | Supported combinations, formatting ownership, fallback behavior; no inferred VND or translated commercial terms |
| Terms and eligibility | Informational conditions only vs later personalized eligibility outside this endpoint | Approved title/benefits/conditions, stable identity and display order; whether price includes taxes, exact period/unit/rounding if prices are approved; no price-derived entitlement |
| Effective time | Immediate activation vs scheduled windows; server time, timezone and boundary inclusivity | Exact authoritative clock and before/at/after behavior, overlap/gap policy and expiry behavior |
| Version identity | Immutable content revision vs active publication generation | Distinguish wire schema version, immutable terms revision and current publication identity; same terms after withdrawal must not silently reuse an obsolete active validator |
| Bounded projection | Single bounded snapshot vs another justified bounded representation | Limits on plans/text/locales/bytes, deterministic ordering and validation rejection; no unbounded public draft enumeration |
| Empty and unknown | Approved empty snapshot vs invalid publication; known absence vs inaccessible configuration | Define whether empty can be published; unknown/inaccessible must not be silently reported as known unpublished |
| Consistency and rollback | Single authoritative switch vs explicitly bounded eventual visibility across instances/caches | Atomic complete snapshot, concurrent-update conflict policy, visibility bound, restart/reload behavior; rollback republishes accepted content under a current generation |
| Cache and withdrawal | Revalidation-only vs finite freshness; stale/offline terms permitted or forbidden | Exact freshness and withdrawal bound across API, browser/CDN and CMP; validator semantics, failure caching, stale responses and no private/error body caching |
| Old versions | No public historical reads vs explicit retained public history | What a withdrawn/version-pinned request returns and retention obligations; distinguish public historical terms from private never-published drafts |
| HTTP and compatibility | Method/path/status/error mapping, content negotiation and schema evolution | Theatre-only anonymous read contract, bounded responses and exact failures; justify codec scope under existing transport policy, fixtures for old/unknown versions before DTOs |

## Candidate flow and failure matrix

The three public semantic states requested by #176 are `not_published`, `published` and
`temporarily_unavailable`. The following candidate behavior needs owner approval with the
matrix above; it defines no JSON field names, route, status codes, database transitions or TTL.
Loading/cancellation/transport failures remain client states. They are not proof of a source's
publication state.

| Source / event | Candidate public result | Required failure oracle for later implementation |
| --- | --- | --- |
| Authoritatively configured absence or withdrawal | `not_published`, current publication generation, no terms | Fresh read/reload shows **Chưa công bố**; no previous terms/private draft body attached |
| Complete approved and currently effective snapshot | `published`, exact revision/current generation and approved bounded terms | Readers observe one complete snapshot with stable ordering; no mixed old/new terms |
| No source configured | Explicitly provision known absence or treat configuration as unavailable, per owner decision | Missing configuration cannot silently manufacture an approved empty catalog or fake plans |
| Source inaccessible, malformed or conflicting | `temporarily_unavailable` unless an explicitly approved still-valid snapshot policy applies | Distinct from known absence; no private source path, credential or draft leakage; retry policy is bounded |
| Approved snapshot contains zero plans | Resolve through empty-publication policy first | Client renders an intentional empty state or source rejects publication; never an invented price/card |
| Invalid/unapproved/future/expired candidate on reload | Never activate the candidate; previous-snapshot availability depends on accepted effective-time policy | Atomic rejection; verify prior valid state remains intact and expired/withdrawn terms cannot reappear |
| Publish races withdrawal or two publishers conflict | One authoritative transition under selected source consistency policy | No partial activation; loser/retry cannot overwrite a newer generation |
| Read/cache revalidation races withdrawal | New generation invalidates old active validators within accepted bound | Old validator cannot yield a misleading unchanged response; late client response cannot replace a newer generation |
| Rollback, process restart or cross-instance read | Resolve authoritative current generation and accepted revision | Historical content is not mutated; withdrawn generation never becomes current just through restart or cache restoration |
| Open/close/retry from any allowed origin | Receipt, account, entitlement and playback unchanged | Back follows closed internal origin values; deep link falls back to Account; no checkout/subscription/write side effect |

## Contracts, production impact and delivery order

1. Product/source review: resolve every decision row, name approver and implementation owner,
   and record accepted policy in owning docs/ADR. A proposed document or merged planning PR
   alone is not acceptance.
2. Contract review: publish the selected versioned contract with shared accepted fixtures,
   exact errors, compatibility and cache/withdrawal rules, tied to these real Web/CMP consumers.
   Only then add plan types/storage. Do not mint real commercial fixtures from design cards.
3. Backend: pure validation/publication decision tests, then source adapter and anonymous
   Theatre read projection. Add real disposable PostgreSQL tests only if option B is selected;
   source/reload/concurrency acceptance still applies to the selected alternative.
4. Client adoption: Web and CMP separately consume the accepted projection, preserve Back,
   fence late loads and retain receipt/account state. Observe relevant renderer interaction,
   text scaling, keyboard/focus and accessibility; previews do not accept live behavior.

The interest-receipt backend remains independent; neither its delivery nor work on #177 may be
used to select plan source, terms or eligibility. No provider/identity configuration, payment,
subscription, paid/expired/trial state, entitlement engine or Studio Mobile is introduced.
No audio/release revision, rights approval, media cache or migration changes in this proposal.
Future plan cache changes must use the separately accepted policy above. No paid call or new
infrastructure is necessary for this review boundary.

## Acceptance and verification ledger

| Claim / issue criterion | Failure mode / cheapest oracle | Evidence held in this proposal | Deferred acceptance |
| --- | --- | --- | --- |
| Product decision + contract approved before DTO/DB | Proposal mistaken for policy; review status, unresolved matrix and docs-only diff | `documented`: no selection, contract/schema/code added | Named product acceptance, then consumer-backed versioned contract; #176 criterion remains unmet |
| No approved source → **Chưa công bố** without fake plans | Prototype cards treated as production data; trace current consumers above | `documented`: existing unavailable/preview boundary retained | Backend/client fresh-load and cache tests; no runtime result claimed |
| Atomic publication, withdrawal, version/cache/reload | Stale terms reappear or drafts leak; independent expected-state sequences above | `documented`: test obligations only | Pure tests followed by selected source concurrency/restart/API tests |
| Read-only display cannot charge or grant access | Plan navigation creates subscription or paid status; inspect effects/row deltas in later flow | `documented`: explicit scope and unchanged source | Handler allowed-method and zero-write tests; Web/CMP origin/receipt/playback assertions |

Docs-only handoff uses `RUSTUP_TOOLCHAIN=1.98.1 TMPDIR=/private/tmp cargo xtask preflight
--changed-only`. Its repository/format result establishes hygiene, not runtime behavior.
L2/L4/L5 implementation, publication/cache/version tests, real DB/provider/device and renderer
acceptance are NOT_RUN in this boundary. Later implementations select the affected
`cargo xtask verify rust`, `cargo xtask verify contracts`, `cargo xtask verify postgres`,
`cargo xtask verify web` and `cargo xtask theatre-demo verify` checks through the existing
[verification cadence](../../.agents/skills/cantos-engineering/references/verification-cadence.md).
Exact focused test selectors depend on the accepted implementation; none exist for #176 yet.

Remaining blocker: the product/source/version/cache decisions and accepted versioned contract.
The next owner is #34's product decision owner, followed by an unassigned #176 implementation
owner and Web/CMP reviewers. Runtime production activation stays unavailable until those
decisions and the selected source/consumer failure matrix have acceptance evidence.
