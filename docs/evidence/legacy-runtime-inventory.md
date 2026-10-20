# Retired PostgreSQL/provider runtime inventory

Source audit: `develop@3e6c183d3d616e8ec4a782ba788b4b70d0c6e49a`, 2026-10-17.
Implementation scope: [#238](https://github.com/loveoverflowcom/cantos/issues/238), coordinated
with the distinct fresh-schema/cutover work in [#239](https://github.com/loveoverflowcom/cantos/issues/239).
The owner update in [#67](https://github.com/loveoverflowcom/cantos/issues/67) removes the
requirement to maintain retired provider execution compatibility. [ADR 0018](../decisions/0018-external-audio-production.md)
continues to prohibit inference/workers in the API and keeps PostgreSQL authoritative.
[ADR 0006](../decisions/0006-ai-script-adaptation.md) remains proposed for production adoption;
its old compatibility paragraph does not override the newer owner decision.

This record inventories source/callers. It does not claim a live database survey, deployment,
model quality or production acceptance. Historical immutable records and evidence are not deleted
by runtime removal. Production database replacement needs inventory and the separate controlled
cutover gates.

## Caller and ownership inventory

| Classification | Boundary and caller | SQL/runtime dependency | Decision |
| --- | --- | --- | --- |
| Active | [Studio adaptation review](../../apps/cantos-studio/web/src/adaptation.rs), [HTTP client](../../apps/cantos-studio/web/src/api.rs), [CLI tools](../../scripts/cantos_adaptation_tool.py) | `c1` frozen `adaptation_runs`; `adaptation_submissions`, `adaptation_proposals`, `adaptation_cancellations`, `adaptation_acceptances`; ingestion source and editorial revision/evidence entrypoints | Keep context/create/read/review, caller proposal submit, context cancel and explicit acceptance. All run reads are owner-bound point lookups; operation replay is owner/operation-bound. |
| Active | [Caller adaptation application](../../services/cantos-server/src/contexts/adaptation/application/context.rs), [submit](../../services/cantos-server/src/contexts/adaptation/application/submit.rs), [accept](../../services/cantos-server/src/contexts/adaptation/application/accept.rs) | Transaction-time authentication, per-actor advisory and run row locks, exact frozen context digest/source/base, immutable submission and acceptance receipts | Preserve exact replay, source/head immutability, session recheck, source ownership, stale head rejection and review gates. Statuses are `awaiting_proposal`, `succeeded`, `cancelled`, `accepted`. Invalid domain output is a submission result, not a provider-run state. |
| Retired, with one client branch at audit | `GET /adaptations/{run}/review` previously serialized `workflow: legacy`; Studio rendered `LegacyRunDetails` and historical provider configuration | `a1` frozen-input decoder, retired run receipts, provider observations and optional accepted revision | Remove both ends of this dependency together. `/review` retains its existing `workflow: caller` shape and rejects legacy workflows in client/contracts. Acceptance loads a caller context only. |
| Retired, unused by current transport | `GET /adaptations/provider`, `POST /adaptations`, `GET /adaptations/{run}`, `POST /adaptations/{run}/cancel` | Disabled start, `load_adaptation`, `cancel_adaptation`, expiry/observation recovery and dispatch deadlines | Remove handlers and Store methods entirely. No unavailable/null shim remains; requests get the ordinary route 404 across all surface prefixes. |
| Retired | Old shared start/provider/config/run DTOs and `Legacy` review variant | `StartAdaptationRequest`, `AdaptationConfig`, `AdaptationProviderMetadata`, `AdaptationProviderResponse`, `AdaptationRunResponse`, `FrozenAdaptation`, `INPUT_A1_DIGEST`, `Observation` | Remove DTOs/decoders/schema definitions and legacy-only fixtures. Retain caller generation declarations and explicit cost-basis rejection; a provider name in caller metadata is inert provenance. |
| Retired | Old PostgreSQL provider dispatch/read recovery | `adaptation_attempts`, `adaptation_observations`, `adaptation_runs.problem`, `dispatch_deadline` | Current adapters neither query nor write them. Physical old tables/rows have no runtime compatibility consumer; schema replacement is a separate operation. |
| Retired before this change | TTS worker, JobSummary/summary-view maintainer and speech reuse runtime | `durable_jobs`, `durable_steps`, attempts/outbox and `speech_clips` | No source consumer in the current API/Studio/Theatre/Forge was found. The schema work retires its old feed-specific verification rather than reviving workers. |
| Still-live separate compatibility | Deprecated `/api/v1/*` Studio HTTP alias | [Studio router mount/cookie paths](../../services/cantos-server/src/adapters/inbound/http/studio/mod.rs), [route tests](../../services/cantos-server/tests/surface_routes.rs), [Catalog HTTP tests](../../services/cantos-server/tests/catalog_http_postgres.rs), [revision tests](../../services/cantos-server/tests/revisions_postgres.rs), [OIDC tests](../../services/cantos-server/tests/studio_oidc_postgres.rs) | Retain. This is a shared-handler HTTP alias with independent cookie/deprecation behavior; source cleanup does not establish absence of deployed consumers. New clients use `/api/v1/studio`. |
| Active | [Catalog](../../services/cantos-server/src/contexts/catalog/mod.rs), [Content](../../packages/contracts/content-v1.md), [ingestion](../../services/cantos-server/src/contexts/ingestion/mod.rs), [editorial](../../services/cantos-server/src/contexts/editorial/mod.rs), [Media](../../services/cantos-server/src/contexts/media/mod.rs), auth and listener contexts | Current metadata, source, revision, private asset, publication and identity authority | Preserve their runtime paths and permissions. The same SQL database remains authoritative; no business query moves to DBSP. |
| Active optional platform, no current consumer | [Generic incremental engine](../../services/cantos-server/src/platform/incremental/mod.rs) and `dbsp-experimental` | Signed changes, grouped-count example circuit, transaction/freshness pure helpers | Keep isolated and opt-in. It is neither a PostgreSQL extension nor a replacement authority. No PostgreSQL business adapter or production activation is added. |
| Active external producer boundary | [Forge](../../services/cantos-forge-functions/README.md), Theatre Web/CMP and [adaptation tools](../../packages/contracts/adaptation-tools-v1.json) | No old provider-run route caller found; Forge remains an external producer, Theatre consumes public releases | Preserve module names and distinct authorization. No new worker or model transport is introduced into the API. |

The runtime caller audit searched Rust/Kotlin application source and the current tool scripts for
old table names, provider/start/load/cancel symbols and old shared DTOs. Historical migrations,
source-bound evidence, private-storage target `legacy-filesystem`, identity legacy actors and
unprefixed HTTP aliases have different owners; their spelling alone does not make them dead.

## Read/performance decision

Current context, proposal, source and receipt reads remain PostgreSQL owner-filtered point lookups;
frequency and live cardinality are unknown. Existing synthetic application-role index and bounded
response tests remain in [adaptations_postgres.rs](../../services/cantos-server/tests/adaptations_postgres.rs).
Removing retired reads does not change the current query algorithm or establish a speedup.
DBSP adoption is deferred: no new expensive aggregate, consumer, adapter or measured benefit is
identified. Existing benchmark history is not reused as current workload acceptance.

## Regression evidence and deferred acceptance

Added route coverage: `retired_provider_adaptation_routes_are_not_mounted_on_any_surface` exercises
the real Axum router without dialing PostgreSQL. Contract tests reject retired status spellings,
legacy review variants and provider-execution request fields; the current caller context shape
remains admitted. Studio's negative wire fixture rejects a legacy workflow while existing actor,
session, revision, review, retry and IME guards remain covered by its reducer tests.

The disposable PostgreSQL adaptation suite retains current context → valid/invalid submission →
review/accept/reopen, duplicate/race, ownership/session revocation, stale head, immutable evidence,
invalid binding and source-preservation tests. Historical `a1` seeding/read tests are removed;
count/grant fixtures now name current tables/columns only.

Execution outcomes belong to the parent task's final-diff gate ledger and PR. At inventory-authoring
time no Cargo or PostgreSQL gate was run by this helper; formatting and scoped diff checks were run.
No production database, VPS, Firebase setting, provider, manuscript or media was accessed by this
runtime-cleanup work. Live external-producer, browser interaction and production cutover acceptance
remain separate and require their named environments and authorization.
