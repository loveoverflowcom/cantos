# Architecture overview

Status: target runtime design. The [Script IR domain/CLI](../../services/cantos-api/README.md) is
implemented and locally tested. Local Axum/PostgreSQL revision persistence and a minimal Leptos
Studio consumer have [bounded evidence](../evidence/script-revision-persistence.md).
[Casting/input planning](../evidence/casting-production-inputs.md) and the
[local/fake durable worker](../evidence/durable-tts-jobs.md) reuse that authority; production
adoption, Theatre listening and native clients remain planned. The
user-selected stack and modular starting point are recorded in
[decision 0001](../decisions/0001-modular-monolith.md).

## Runtime and ownership

Use one Rust backend with Axum API handlers, domain/application modules and infrastructure adapters. Production workers may be a second process from the same backend package so expensive audio/AI work does not block request handling. Do not introduce independently deployed microservices, a broker or a workflow platform before the first production slice needs them.

| Logical boundary | Owns | Must not own |
| --- | --- | --- |
| Script/domain | Work/adaptation hierarchy, stable dialogue identities, voice/cast revisions and rights references | HTTP, provider SDKs or UI |
| Application | Permission checks, revision transitions, production planning, cost limits and publication gates | Provider-specific request shapes |
| Production | Durable step execution, TTS/mixing/QC adapters and asset manifests | Listener navigation or public permissions bypasses |
| Infrastructure | PostgreSQL transactions, object storage, AI providers, media tools and delivery adapters | Product state-transition policy |
| Web | Studio editor (`apps/cantos-studio/web`) and Theatre player (`apps/cantos-theatre/web`), as separate applications | Credentials for AI/object storage or authoritative approvals |
| Mobile | CMP screens/state plus native playback/download adapters for Theatre only (`apps/cantos-theatre/mobile`) | Server orchestration or in-app TTS for published episodes |

Cantos Studio has Web as its sole deployment target. Studio Mobile is out of scope / not
implemented; follow the [platform scope](repository-layout.md#platform-scope) before adding a
client or build target. Shared libraries may serve Theatre Mobile and Studio Web.

These are module responsibilities, not requirements to create one crate per row. Create packages only when a real build target or independent consumer justifies them.

## Data and delivery

PostgreSQL owns accounts/roles, script revisions, casting, provenance/rights, job attempts, cost reservations, asset metadata, approvals, publication manifests and listener progress. Media bytes live in S3-compatible storage. Retain immutable object keys and checksums; an asset row is ready only after upload completion and verification.

Keep private source manuscripts, voice samples, dialogue renders, scene previews and unpublished mixes separate from approved delivery assets by authorization and storage policy. Publishing must not make an entire production bucket public. CDN delivery serves only approved rendition references, using public or access-controlled delivery according to the episode's policy.

Recommended recovery model: a transactional job table with bounded retries, expiring leases and fencing tokens; a PostgreSQL outbox for external upload/publication work; workers that tolerate repeated messages. This is a proposal to validate in implementation, not an existing job API. External provider timeouts can leave an unknown billing/generation outcome; store the provider attempt identifier and reconcile before an automatic repeat where possible.

Publication is an atomic metadata transition to a complete immutable manifest after rights, editorial and QC gates pass. A failed replacement keeps the current release playable. Cache purges are a delivery optimization rather than the source of publication truth.

## Interfaces

- Version Script IR independently of Narrative Forge; exchange files/data through a compatibility-tested adapter. No path dependency or direct access to another project's database.
- HTTP contracts are hand-authored JSON Schemas with serialized fixtures and raw Rust DTOs shared by the server and the Studio (`cantos-contracts`); no OpenAPI or generated client exists. Kotlin types, when CMP needs them, are generated or drift-checked against the same fixtures. JSON is the default representation; a route may also negotiate MessagePack only as described in the [wire codec](wire-codec.md). Do not share UI implementation across frameworks.
- Keep a provider-neutral TTS request and capability model. Record which emotional/pronunciation controls the chosen provider actually supports; unsupported controls require an explicit fallback or correction.
- Separate audio render settings from script content. A mixing-profile revision can invalidate mixes without regenerating unchanged speech.

<a id="initial-source-layout"></a>

## Implemented source layout

The [repository layout](repository-layout.md) names every app, service and package. The root Cargo
workspace contains `services/cantos-api/`, `apps/cantos-studio/web/`, the `apps/cantos-theatre/web/`
Theatre skeleton, `packages/contracts/api/` and `packages/ui-system/leptos/`. The API has five
[implemented business contexts](api-contexts.md): Editorial, Ingestion, Adaptation, Production
and Synthesis. Each separates pure domain decisions from private application use cases and
PostgreSQL persistence; Script IR is the versioned shared kernel. HTTP surfaces live under
`interfaces/http/`, and connection/session/migration/operator plumbing under `platform/`.
Temporary crate-root re-exports preserve existing CLI/library paths.

Adaptation exports frozen caller context and admits proposals without inference or a revision
change before explicit acceptance. Production resolves casting/pronunciation/performance,
estimates cost and binds frozen input identity/current rights/approval. Its immutable catalog
contracts are pure data; Synthesis owns provider execution, attempts/receipts, clip reuse,
regeneration and the local/fake worker. Transaction extraction preserves locked current facts,
idempotency, session expiry and dispatch fences. The explicit local, Google and research-only
neural probes retain their existing permission and rights boundaries
([ADR 0008](../decisions/0008-local-tts-inspection.md),
[ADR 0011](../decisions/0011-google-cloud-tts-adapter.md),
[ADR 0010](../decisions/0010-local-neural-tts-research.md)).
The durable job summary later gained an opt-in experimental DBSP read model behind the
`dbsp-experimental` Cargo feature: an engine-independent `incremental` core, a trigger-written
outbox with an exact `xid8` committed position, and a fenced consumer that never answers before
PostgreSQL authorization ([ADR 0017](../decisions/0017-experimental-dbsp-job-summary.md)).
PostgreSQL remains the authority and the default path.
The Studio WASM consumer imports only `cantos-contracts`, never server
internals. This shared DTO crate has two concrete consumers. Migration and HTTP binaries live
in the same `cantos-api` package. `apps/cantos-theatre/mobile/` remains a guide; its Gradle app
belongs to the native listening slice. `apps/cantos-studio/mobile/` retains only an out-of-scope notice. [ADR 0003](../decisions/0003-script-revision-persistence.md) is proposed for
production adoption, not an accepted deployment decision.

## First verification targets

Use an original short two-character scene and synthetic/fake providers for deterministic tests. Then exercise a real provider behind explicit cost limits. Verify editing one dialogue, crashing/restarting a worker, retrying an ambiguous provider attempt, failing upload, failing QC, rejecting stale approval and publishing a replacement while listeners retain a playable prior release.

Do not claim production readiness from document checks or a simulated provider. Deployment, authentication provider, concrete TTS provider, media presets and storage/CDN vendor remain implementation decisions.
