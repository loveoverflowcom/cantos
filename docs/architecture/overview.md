# Architecture overview

Status: the [script revision backend](../../apps/server/README.md) implements the first Script IR/editorial, HTTP and PostgreSQL boundaries; other runtime sections remain proposed. The user-selected stack and modular starting point are recorded in [decision 0001](../decisions/0001-modular-monolith.md).

## Runtime and ownership

Use one Rust backend with Axum API handlers, domain/application modules and infrastructure adapters. Production workers may be a second process from the same backend package so expensive audio/AI work does not block request handling. Do not introduce independently deployed microservices, a broker or a workflow platform before the first production slice needs them.

| Logical boundary | Owns | Must not own |
| --- | --- | --- |
| Script/domain | Work/adaptation hierarchy, stable dialogue identities, voice/cast revisions and rights references | HTTP, provider SDKs or UI |
| Application | Permission checks, revision transitions, production planning, cost limits and publication gates | Provider-specific request shapes |
| Production | Durable step execution, TTS/mixing/QC adapters and asset manifests | Listener navigation or public permissions bypasses |
| Infrastructure | PostgreSQL transactions, object storage, AI providers, media tools and delivery adapters | Product state-transition policy |
| Web | Studio editor and Theatre player | Credentials for AI/object storage or authoritative approvals |
| Mobile | Shared CMP screens/state plus native playback/download adapters | Server orchestration or in-app TTS for published episodes |

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

`apps/server/` is a runnable Cargo package with migrations and tests. `apps/web/` and `apps/mobile/` still contain guides; their manifests and verification commands will arrive with their consumers. Keep shared contracts in `contracts/`; add shared Rust modules/packages only as their consumers appear.

## First verification targets

Use an original short two-character scene and synthetic/fake providers for deterministic tests. Then exercise a real provider behind explicit cost limits. Verify editing one dialogue, crashing/restarting a worker, retrying an ambiguous provider attempt, failing upload, failing QC, rejecting stale approval and publishing a replacement while listeners retain a playable prior release.

Do not claim production readiness from document checks or a simulated provider. Deployment, browser identity provider, concrete TTS provider, media presets and storage/CDN vendor remain implementation decisions. The initial operator-provisioned creator-token choice is recorded in [ADR 0002](../decisions/0002-creator-authentication.md).
