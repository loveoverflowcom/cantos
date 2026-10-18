# Evidence records

This directory contains source-bound observations, test reports, fixtures and assets. It is
**not the work queue or an implementation specification**. Keep existing paths and binary/
JSON fixture bytes stable; age alone is not a reason to delete evidence.

Read the [current product/architecture map](../README.md), [decision lifecycle](../decisions/README.md)
and owning contract first. Then choose the relevant record and check its source revision,
inputs, tool/environment, live-vs-mocked boundary and deferred acceptance. An older FAIL or
NOT_RUN is retained history; an older PASS does not establish current-code or production status.

## Reading paths

| Capability | Records and limits |
| --- | --- |
| Catalog/Studio consumers | [API](studio-catalog-api.md), [browser follow-up](live-client-browser-acceptance.md), [finding recheck](live-client-finding-fixes.md); #66 completion and remaining QA live in GitHub |
| Brief/private audio | [Brief](production-brief.md), [Audio Lifecycle](audio-lifecycle.md); technical admission is not rights/listening approval |
| Private storage/artwork | [Storage](provider-independent-storage.md), [provider blocker](longvan-s3-acceptance.md), [artwork](private-work-artwork.md), [Studio UI](studio-private-artwork-ui.md); keep real-provider and fixture/browser acceptance separate |
| Publication/delivery | [Core](publication-core.md), [Range transport](playback-range-transport.md), [origin delivery](public-media-delivery-core.md); composition-gated cores are not live playback |
| Theatre catalog integration | [Step-1 production catalog/API/storage](theatre-live-catalog-step1.md); [earlier seeded Web/CMP evidence](theatre-seeded-catalog-integration.md) retains its original scope. Native runtime/playback require separate evidence |
| Identity | [Studio OIDC](studio-oidc.md), [Theatre activation](theatre-auth-activation.md), [Theatre identity](theatre-identity.md), [Firebase runtime](firebase-enrollment-runtime.md), [Theatre live sign-in](theatre-live-auth-206.md); live mail/provider/device gates stay explicit |
| Behavior | [Quint foundation](quint-behavior-foundation.md), [pure-core observations](quint-code-conformance.md); draft model PASS is not accepted behavior or DB/API/client conformance |
| Optional incremental platform | [DBSP review](dbsp-platform-review.md); current grouped-count core and forward-only legacy capture retirement, with future-consumer and production gates kept separate |
| Develop consistency | [Eight-commit review](develop-eight-commit-review-20261016.md); pinned merge-aware scope, receipt-wait expiry repair and independently reported local gates |
| Localization ownership | [17 October audit](i18n-audit-20261017/README.md); baseline-addressed five-scope inventory, existing-owner repairs, bounded changed-source guard and separate renderer/runtime acceptance |
| Runtime cleanup | [Current/retired SQL caller inventory](legacy-runtime-inventory.md); `a1` routes/DTOs are removed together with their Studio branch, while controlled schema cutover and live acceptance remain separate |
| Deployment tooling | [Historical Hosting tool evidence](issue199-firebase-hosting.md); current topology/operations live only in the [production runbook](../deployment/vps-production.md), never a dated deployment diary |

## Retired backend experiments

[ADR 0018](../decisions/0018-external-audio-production.md) supersedes backend TTS/provider/job
execution. Retained [eSpeak](tts-receipt-inspection.md), [provider prerequisites](tts-provider-prerequisites.md),
[Google adapter](google-tts-adapter.md), [neural research](local-neural-tts-research.md),
[neural samples](local-neural-tts.md) and [DBSP job summary](dbsp-job-summary.md) describe
the original experiments. They do not authorize restoring a worker or claim a live provider.
Forge's producer-side work is independently scoped by #203.

Session-specific planning instructions are in the [archive](../archive/README.md). Record a new
result only for a concrete verification claim with provenance; put routine docs-only check
results in the PR instead of adding another session ledger.
