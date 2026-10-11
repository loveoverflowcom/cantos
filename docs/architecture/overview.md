# Architecture overview

Status: target runtime design. The [Script IR domain/CLI](../../services/cantos-api/README.md) is
implemented and locally tested. Local Axum/PostgreSQL revision persistence and a minimal Leptos
Studio consumer have [bounded evidence](../evidence/script-revision-persistence.md).
[Casting/input planning](../evidence/casting-production-inputs.md) reuses that authority;
workers, Theatre and native clients remain planned. The
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
| Web | Studio editor (`apps/cantos-studio/web`) and Theatre player (`apps/cantos/web`), as separate applications | Credentials for AI/object storage or authoritative approvals |
| Mobile | CMP screens/state plus native playback/download adapters for Theatre (`apps/cantos/mobile`); a Studio client is reserved (`apps/cantos-studio/mobile`) | Server orchestration or in-app TTS for published episodes |

These are module responsibilities, not requirements to create one crate per row. Create packages only when a real build target or independent consumer justifies them.

## Data and delivery

PostgreSQL owns accounts/roles, script revisions, casting, provenance/rights, job attempts, cost reservations, asset metadata, approvals, publication manifests and listener progress. Media bytes live in S3-compatible storage. Retain immutable object keys and checksums; an asset row is ready only after upload completion and verification.

Keep private source manuscripts, voice samples, dialogue renders, scene previews and unpublished mixes separate from approved delivery assets by authorization and storage policy. Publishing must not make an entire production bucket public. CDN delivery serves only approved rendition references, using public or access-controlled delivery according to the episode's policy.

Recommended recovery model: a transactional job table with bounded retries, expiring leases and fencing tokens; a PostgreSQL outbox for external upload/publication work; workers that tolerate repeated messages. This is a proposal to validate in implementation, not an existing job API. External provider timeouts can leave an unknown billing/generation outcome; store the provider attempt identifier and reconcile before an automatic repeat where possible.

Publication is an atomic metadata transition to a complete immutable manifest after rights, editorial and QC gates pass. A failed replacement keeps the current release playable. Cache purges are a delivery optimization rather than the source of publication truth.

## Interfaces

- Version Script IR independently of Narrative Forge; exchange files/data through a compatibility-tested adapter. No path dependency or direct access to another project's database.
- Define HTTP/OpenAPI and serialized contract fixtures when the first endpoints are implemented. Generate or validate TypeScript/Rust/Kotlin types where practical; do not share UI implementation across frameworks.
- Keep a provider-neutral TTS request and capability model. Record which emotional/pronunciation controls the chosen provider actually supports; unsupported controls require an explicit fallback or correction.
- Separate audio render settings from script content. A mixing-profile revision can invalidate mixes without regenerating unchanged speech.

## Initial source layout

The [repository layout](repository-layout.md) names every app, service and package. The root Cargo
workspace contains `services/cantos-api/`, `apps/cantos-studio/web/`, the `apps/cantos/web/` Theatre
skeleton and `packages/contracts/api/`. Server
`script_ir`, `revisions` and the adaptation admission kernel are pure rules; `postgres` owns
transactions and `http` maps the shared wire contract to Axum. Adaptation tools export frozen context and admit caller-created proposals over the same HTTP
service and a thin CLI. Generation belongs to the user-controlled AI host; the domain and
store perform no inference. Proposals never move a revision head until explicit acceptance.
The pure `production` module resolves reference-only casting/pronunciation/performance, computes
integer planning estimates and exact frozen input identity, then reevaluates current claims and
approval. PostgreSQL production adapters append settings/claims/candidates/approvals on the same
identity/evidence/revision authority; the HTTP layer and Studio share raw DTOs. No generation,
accounting engine, provider transport, service or DBSP read model is introduced by #5.
The focused #6 local inspection slice adds the pure `tts::local_contract` descriptor/text
predicate and receipt/audio/recovery rules inside the server package. Its explicit
`probe-local-tts` CLI invokes one installed eSpeak engine and writes private sample evidence;
HTTP remains planning-only. This is not a production worker, cache or audio-acceptance path.
[ADR 0008](../decisions/0008-local-tts-inspection.md) records the accepted local inspection choice
and proposed production adoption. Existing PostgreSQL queries/schema and publication/listening
boundaries are unchanged; no new read model or DBSP consumer is justified.
The optional neural research slice adds one concrete `tts::neural` process adapter and pure
`neural_contract`, with a fixed local Python helper and its own PCM inspection profile. It
reuses complete frozen-document/receipt admission and refuses production before any model or
runtime I/O. The eSpeak adapter/probe, Script IR, production-p1, PostgreSQL authority and listener
boundaries remain intact. See [ADR 0010](../decisions/0010-local-neural-tts-research.md).
The Studio WASM consumer imports only `cantos-contracts`, never server
internals. This shared DTO crate has two concrete consumers. Migration and HTTP binaries live
in the same `cantos-api` package. `apps/cantos/mobile/` and `apps/cantos-studio/mobile/` remain guides;
Gradle belongs to the native listening slice. [ADR 0003](../decisions/0003-script-revision-persistence.md) is proposed for
production adoption, not an accepted deployment decision.

## First verification targets

Use an original short two-character scene and synthetic/fake providers for deterministic tests. Then exercise a real provider behind explicit cost limits. Verify editing one dialogue, crashing/restarting a worker, retrying an ambiguous provider attempt, failing upload, failing QC, rejecting stale approval and publishing a replacement while listeners retain a playable prior release.

Do not claim production readiness from document checks or a simulated provider. Deployment, authentication provider, concrete TTS provider, media presets and storage/CDN vendor remain implementation decisions.
