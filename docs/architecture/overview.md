# Architecture overview

Status: target runtime design. The [Script IR domain/CLI](../../services/cantos-api/README.md) is
implemented and locally tested. Local Axum/PostgreSQL revision persistence and a minimal Leptos
Studio consumer have [bounded evidence](../evidence/script-revision-persistence.md).
[Casting/input planning](../evidence/casting-production-inputs.md) reuses that authority.
Cantos does not synthesize audio: external AI producers read its guidance and submit audio back
([ADR 0018](../decisions/0018-external-audio-production.md)), an API that is proposed and not yet
implemented. Production adoption, Theatre listening and native clients remain planned. The
user-selected stack and modular starting point are recorded in
[decision 0001](../decisions/0001-modular-monolith.md).

## Runtime and ownership

Use one Rust backend with Axum API handlers, domain/application modules and infrastructure adapters. Audio and AI generation run outside Cantos, in producers that call the API; Cantos itself does no expensive audio or AI work. Do not introduce independently deployed microservices, a broker or a workflow platform before a concrete slice needs them.

| Logical boundary | Owns | Must not own |
| --- | --- | --- |
| Script/domain | Work/adaptation hierarchy, stable dialogue identities, voice/cast revisions and rights references | HTTP, provider SDKs or UI |
| Application | Permission checks, revision transitions, production guidance and approval, audio admission and publication gates | Provider-specific request shapes |
| Production guidance and media | Casting and frozen inputs, standards, submitted-audio inspection and asset manifests | TTS or mixing execution, listener navigation or public permissions bypasses |
| Infrastructure | PostgreSQL transactions, object storage, media inspection tools and delivery adapters | Product state-transition policy |
| Web | Studio editor (`apps/cantos-studio/web`) and Theatre player (`apps/cantos-theatre/web`), as separate applications | Credentials for AI/object storage or authoritative approvals |
| Mobile | CMP screens/state plus native playback/download adapters for Theatre only (`apps/cantos-theatre/mobile`) | Server orchestration or in-app TTS for published episodes |

Cantos Studio has Web as its sole deployment target. Studio Mobile is out of scope / not
implemented; follow the [platform scope](repository-layout.md#platform-scope) before adding a
client or build target. Shared libraries may serve Theatre Mobile and Studio Web.

These are module responsibilities, not requirements to create one crate per row. Create packages only when a real build target or independent consumer justifies them.

## Data and delivery

PostgreSQL owns accounts/roles, script revisions, casting, provenance/rights, submitted-audio metadata, approvals, publication manifests and listener progress. Media bytes live in S3-compatible storage. Retain immutable object keys and checksums; an asset row is ready only after upload completion and verification.

Keep private source manuscripts, voice samples, dialogue renders, scene previews and unpublished mixes separate from approved delivery assets by authorization and storage policy. Publishing must not make an entire production bucket public. CDN delivery serves only approved rendition references, using public or access-controlled delivery according to the episode's policy.

Recommended recovery model for the external work Cantos still triggers (upload verification, publication, cache purge): a PostgreSQL outbox with consumers that tolerate repeated messages. This is a proposal to validate in implementation, not an existing API. Submissions must be idempotent, so a producer that repeats an upload after a timeout cannot create a second accepted result.

Publication is an atomic metadata transition to a complete immutable manifest after rights, editorial and QC gates pass. A failed replacement keeps the current release playable. Cache purges are a delivery optimization rather than the source of publication truth.

## Interfaces

- Version Script IR independently of Narrative Forge; exchange files/data through a compatibility-tested adapter. No path dependency or direct access to another project's database.
- HTTP contracts are hand-authored JSON Schemas with serialized fixtures and raw Rust DTOs shared by the server and the Studio (`cantos-contracts`); no OpenAPI or generated client exists. Kotlin types, when CMP needs them, are generated or drift-checked against the same fixtures. JSON is the default representation; a route may also negotiate MessagePack only as described in the [wire codec](wire-codec.md). Do not share UI implementation across frameworks.
- Give producers a provider-neutral description of each line (text, voice guidance, pronunciation, delivery). Record which controls a voice actually supports; unsupported controls require an explicit fallback or correction before approval.
- Separate audio render settings from script content. A mixing-profile revision can invalidate mixes without regenerating unchanged speech.

<a id="initial-source-layout"></a>

## Implemented source layout

The [repository layout](repository-layout.md) names every app, service and package. The root Cargo
workspace contains `services/cantos-api/`, `apps/cantos-studio/web/`, the `apps/cantos-theatre/web/`
Theatre skeleton, `packages/contracts/api/` and `packages/ui-system/leptos/`. The API has four
[business contexts](api-contexts.md): Editorial, Ingestion, Adaptation and Production, plus a Media
context for inspecting externally produced audio. Each separates pure domain decisions from private application use cases and
PostgreSQL persistence; Script IR is the versioned shared kernel. HTTP surfaces live under
`interfaces/http/`, and connection/session/migration/operator plumbing under `platform/`.
Temporary crate-root re-exports preserve existing CLI/library paths.

Adaptation exports frozen caller context and admits proposals without inference or a revision
change before explicit acceptance. Production resolves casting/pronunciation/performance,
estimates cost and binds frozen input identity/current rights/approval; its immutable catalog
contracts are pure data, and an approved snapshot is guidance for an external producer. Media
holds the pure, read-only WAV inspection that submitted audio will pass through. The in-process
TTS runtime that once sat beside them was removed
([ADR 0018](../decisions/0018-external-audio-production.md)). Transaction extraction preserves
locked current facts, idempotency and session expiry. An experimental, now consumer-less
`dbsp-experimental` feature still compiles an engine-independent `incremental` core.
PostgreSQL remains the authority and the default path.
The Studio WASM consumer imports only `cantos-contracts`, never server
internals. This shared DTO crate has two concrete consumers. Migration and HTTP binaries live
in the same `cantos-api` package. `apps/cantos-theatre/mobile/` remains a guide; its Gradle app
belongs to the native listening slice. `apps/cantos-studio/mobile/` retains only an out-of-scope notice. [ADR 0003](../decisions/0003-script-revision-persistence.md) is proposed for
production adoption, not an accepted deployment decision.

## First verification targets

Use an original short two-character scene and a synthetic audio fixture for deterministic tests. Then exercise a real external producer end to end. Verify editing one dialogue, an interrupted or repeated upload, a submission for a superseded revision, failing upload, failing QC, rejecting stale approval and publishing a replacement while listeners retain a playable prior release.

Do not claim production readiness from document checks or a synthetic fixture. Deployment, authentication provider, producer credential model, media presets and storage/CDN vendor remain implementation decisions.
